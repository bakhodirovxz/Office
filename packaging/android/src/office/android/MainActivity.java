// Licence: MIT OR Apache-2.0. The thin Android host (PLAN phase 6 WP8): one WebView that serves the shell and the four editor dists from the APK's assets on a
// virtual https origin, plus the three things a page cannot do: receive a file from VIEW/SEND/the picker, write it back to its content:// URI, and Save as.
// No androidx, no network permission. Written from scratch; no product name here (the label comes from brand.toml through res/values/strings.xml).
package office.android;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.ClipData;
import android.content.Intent;
import android.content.pm.ApplicationInfo;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.util.Base64;
import android.view.View;
import android.view.WindowInsets;
import android.webkit.JavascriptInterface;
import android.webkit.JsResult;
import android.webkit.RenderProcessGoneDetail;
import android.webkit.ValueCallback;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.FrameLayout;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.json.JSONObject;

public class MainActivity extends Activity {
    static final String HOST = "appassets.androidplatform.net";
    static final String ORIGIN = "https://" + HOST + "/";
    static final int REQ_OPEN = 1, REQ_SAVE_AS = 2, REQ_CHOOSER = 3;
    static final long MAX_BYTES = 256L << 20;   // shortcut: the whole document is held in memory (the editors do too); a streamed bridge if a real phone says otherwise
    static final String[] MIMES = {
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation", "application/pdf", "application/msword", "application/vnd.ms-excel",
        "application/vnd.ms-powerpoint", "application/vnd.oasis.opendocument.text", "application/vnd.oasis.opendocument.spreadsheet",
        "application/vnd.oasis.opendocument.presentation"};
    // route -> dist directory under assets/web (the same table as services/suite-serve EDITORS)
    static final Map<String, String> DISTS = new HashMap<>();
    static { DISTS.put("docs", "docs-web"); DISTS.put("sheets", "sheets-web"); DISTS.put("slides", "slides-web"); DISTS.put("pdf", "pdf-web"); }
    // the shell's own response rules, as convert-api's baseline_headers sets them
    static final String SHELL_CSP = "default-src 'self'; script-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";

    WebView web;
    FrameLayout root;
    final Object lock = new Object();
    // a document waiting for the page to fetch it from /__open (guarded by lock)
    String inName; byte[] inBytes; Uri inUri;
    // the document the page has open: where "save" writes back
    Uri curUri; String curName;
    // a save waiting for the Save as dialog
    byte[] saveBytes; String saveName;
    ValueCallback<Uri[]> chooser;
    final Map<String, String> csp = new HashMap<>();

    @Override protected void onCreate(Bundle b) {
        super.onCreate(b);
        root = new FrameLayout(this);
        root.setBackgroundColor(0xFFFFFFFF);
        web = new WebView(this);
        root.addView(web, new FrameLayout.LayoutParams(-1, -1));
        setContentView(root);
        edgeToEdge();
        setUp();
        if (b == null) {
            web.loadUrl(ORIGIN);
            take(getIntent());
        } else {
            web.restoreState(b);   // a recreated activity (process death): the page restarts; the shell reopens the recovery file (WP12)
        }
    }

    void setUp() {
        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        s.setAllowFileAccess(false);
        s.setAllowContentAccess(false);
        s.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        s.setSupportMultipleWindows(false);
        if ((getApplicationInfo().flags & ApplicationInfo.FLAG_DEBUGGABLE) != 0) WebView.setWebContentsDebuggingEnabled(true);
        web.addJavascriptInterface(new Bridge(), "SuiteHost");
        web.setWebViewClient(new Client());
        web.setWebChromeClient(new Chrome());
        web.setFocusable(true);
        web.setFocusableInTouchMode(true);
    }

    @Override protected void onSaveInstanceState(Bundle b) { super.onSaveInstanceState(b); web.saveState(b); }

    // The page lays out under the system bars (edge to edge); the root pads for the bars and the keyboard, so the keyboard shrinks the viewport
    // (adjustResize alone does nothing once the window does not fit the system windows).
    void edgeToEdge() {
        if (Build.VERSION.SDK_INT >= 30) getWindow().setDecorFitsSystemWindows(false);
        else root.setSystemUiVisibility(View.SYSTEM_UI_FLAG_LAYOUT_STABLE | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
        root.setOnApplyWindowInsetsListener((v, in) -> {
            int l, t, r, bt;
            if (Build.VERSION.SDK_INT >= 30) {
                android.graphics.Insets i = in.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.ime() | WindowInsets.Type.displayCutout());
                l = i.left; t = i.top; r = i.right; bt = i.bottom;
            } else {
                l = in.getSystemWindowInsetLeft(); t = in.getSystemWindowInsetTop(); r = in.getSystemWindowInsetRight(); bt = in.getSystemWindowInsetBottom();
            }
            v.setPadding(l, t, r, bt);
            return Build.VERSION.SDK_INT >= 30 ? WindowInsets.CONSUMED : in.consumeSystemWindowInsets();
        });
    }

    @Override protected void onResume() { super.onResume(); web.requestFocus(); }   // Gboard needs the WebView to be the focused view (docs/phase6/webview-embed.md)

    @Override protected void onNewIntent(Intent i) { super.onNewIntent(i); setIntent(i); take(i); }

    @Override public void onBackPressed() {
        // the shell answers true when it went back to its home page
        web.evaluateJavascript("window.hostBack ? hostBack() : false", v -> { if (!"true".equals(v)) finish(); });
    }

    // ---- receiving a file ----
    void take(Intent i) {
        if (i == null) return;
        Uri u = null;
        if (Intent.ACTION_VIEW.equals(i.getAction())) u = i.getData();
        else if (Intent.ACTION_SEND.equals(i.getAction())) u = i.getParcelableExtra(Intent.EXTRA_STREAM);
        if (u != null) load(u);
    }

    /** Reads the bytes off the main thread, parks them for /__open, then tells the page. */
    void load(Uri u) {
        new Thread(() -> {
            try {
                String name = displayName(u);
                byte[] bytes = readAll(u);
                synchronized (lock) { inName = name; inBytes = bytes; inUri = u; }
                runOnUiThread(() -> web.evaluateJavascript("window.hostOpen && hostOpen()", null));
            } catch (Exception e) {
                runOnUiThread(() -> reply("hostOpenFailed", false, "", String.valueOf(e.getMessage())));
            }
        }).start();
    }

    byte[] readAll(Uri u) throws IOException {
        try (InputStream in = getContentResolver().openInputStream(u)) {
            if (in == null) throw new IOException("cannot open " + u);
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[1 << 16];
            for (int n; (n = in.read(buf)) > 0; ) {
                out.write(buf, 0, n);
                if (out.size() > MAX_BYTES) throw new IOException("file too large");
            }
            return out.toByteArray();
        }
    }

    String displayName(Uri u) {
        try (Cursor c = getContentResolver().query(u, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (c != null && c.moveToFirst() && c.getString(0) != null) return c.getString(0);
        } catch (Exception ignored) { /* fall back to the path */ }
        String p = u.getLastPathSegment();
        return p == null ? "document" : p.substring(p.lastIndexOf('/') + 1);
    }

    static String ext(String name) { int i = name.lastIndexOf('.'); return i < 0 ? "" : name.substring(i + 1).toLowerCase(Locale.ROOT); }

    // ---- saving ----
    /** Called from the page (JS thread). Writes back to the opened URI when the name's extension still matches and the URI is writable, else Save as. */
    void save(String name, byte[] bytes) {
        Uri u; String opened;
        synchronized (lock) { u = curUri; opened = curName; }
        String err = null;
        if (u != null && opened != null && ext(opened).equals(ext(name))) {
            try (OutputStream out = getContentResolver().openOutputStream(u, "wt")) {
                if (out == null) throw new IOException("no output stream");
                out.write(bytes);
                done(true, opened, "");
                return;
            } catch (Exception e) { err = String.valueOf(e.getMessage()); }
        }
        saveBytes = bytes; saveName = name;
        runOnUiThread(() -> {
            Intent i = new Intent(Intent.ACTION_CREATE_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType(mimeOf(name)).putExtra(Intent.EXTRA_TITLE, name);
            try { startActivityForResult(i, REQ_SAVE_AS); }
            catch (Exception e) { saveBytes = null; done(false, name, "cannot write " + opened + " and no Save as dialog: " + e.getMessage()); }
        });
    }

    static String mimeOf(String name) {
        switch (ext(name)) {
            case "docx": return MIMES[0]; case "xlsx": return MIMES[1]; case "pptx": return MIMES[2]; case "pdf": return MIMES[3];
            case "doc": return MIMES[4]; case "xls": return MIMES[5]; case "ppt": return MIMES[6];
            case "odt": return MIMES[7]; case "ods": return MIMES[8]; case "odp": return MIMES[9];
            default: return "application/octet-stream";
        }
    }

    void done(boolean ok, String name, String error) { runOnUiThread(() -> reply("hostSaved", ok, name, error)); }

    void reply(String fn, boolean ok, String name, String error) {
        try {
            JSONObject j = new JSONObject().put("ok", ok).put("name", name).put("error", error);
            web.evaluateJavascript("window." + fn + " && " + fn + "(" + j + ")", null);
        } catch (Exception ignored) { /* JSONObject.put only throws on NaN */ }
    }

    // ---- recovery (WP12): the editor's autosave of an unsaved document, one file per editor, app-private; the shell deletes it once saved or discarded ----
    /** files/recover/<app>: the document's name, a newline, its bytes. null for an unknown editor. */
    File recoverFile(String app) { return DISTS.containsKey(app) ? new File(new File(getFilesDir(), "recover"), app) : null; }

    /** Written beside and renamed over the old one, so a kill mid-write leaves the previous autosave whole. */
    void autosave(String app, String name, byte[] bytes) throws IOException {
        File f = recoverFile(app);
        String n = name.substring(Math.max(name.lastIndexOf('/'), name.lastIndexOf('\\')) + 1).replace('\n', ' ');
        if (f == null || n.isEmpty()) throw new IOException("bad autosave " + app);
        File tmp = new File(f.getParentFile(), app + ".tmp");
        f.getParentFile().mkdirs();
        try (FileOutputStream out = new FileOutputStream(tmp)) {
            out.write((n + "\n").getBytes(StandardCharsets.UTF_8));
            out.write(bytes);
            out.getFD().sync();
        }
        if (!tmp.renameTo(f)) throw new IOException("cannot keep the autosave of " + n);
    }

    /** GET /__recover/<app>: 200 with the bytes and the name in X-Name, or 204. */
    WebResourceResponse recovered(String app) {
        File f = recoverFile(app);
        if (f == null || !f.isFile()) return status(204, "No Content");
        try (InputStream in = new java.io.FileInputStream(f)) {
            ByteArrayOutputStream name = new ByteArrayOutputStream();
            for (int c; (c = in.read()) >= 0 && c != '\n'; ) name.write(c);
            ByteArrayOutputStream body = new ByteArrayOutputStream();
            byte[] buf = new byte[1 << 16];
            for (int n; (n = in.read(buf)) > 0; ) body.write(buf, 0, n);
            Map<String, String> h = new HashMap<>();
            h.put("X-Name", Uri.encode(name.toString("UTF-8"))); h.put("Cache-Control", "no-store"); h.put("Cross-Origin-Resource-Policy", "same-origin");
            return new WebResourceResponse("application/octet-stream", null, 200, "OK", h, new ByteArrayInputStream(body.toByteArray()));
        } catch (IOException e) { return status(204, "No Content"); }
    }

    @Override protected void onActivityResult(int req, int res, Intent data) {
        Uri u = res == RESULT_OK && data != null ? data.getData() : null;
        if (req == REQ_OPEN) { if (u != null) load(u); }
        else if (req == REQ_SAVE_AS) {
            byte[] bytes = saveBytes; String name = saveName; saveBytes = null;
            if (bytes == null) return;
            if (u == null) { done(false, name, "Save as was cancelled"); return; }
            new Thread(() -> {
                try (OutputStream out = getContentResolver().openOutputStream(u, "wt")) {
                    if (out == null) throw new IOException("no output stream");
                    out.write(bytes);
                    String shown = displayName(u);
                    synchronized (lock) { curUri = u; curName = shown; }
                    done(true, shown, "");
                } catch (Exception e) { done(false, name, String.valueOf(e.getMessage())); }
            }).start();
        } else if (req == REQ_CHOOSER && chooser != null) {
            Uri[] r = null;
            if (res == RESULT_OK && data != null) {
                ClipData c = data.getClipData();
                if (c != null) { r = new Uri[c.getItemCount()]; for (int i = 0; i < r.length; i++) r[i] = c.getItemAt(i).getUri(); }
                else if (u != null) r = new Uri[] {u};
            }
            chooser.onReceiveValue(r); chooser = null;
        }
    }

    // ---- what the page may call ----
    class Bridge {
        /** The shell's Open buttons. app: "" = any supported type, "pdf" = PDF only. The result comes back through /__open like a VIEW. */
        @JavascriptInterface public void pick(String app) {
            runOnUiThread(() -> {
                Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("*/*");
                i.putExtra(Intent.EXTRA_MIME_TYPES, "pdf".equals(app) ? new String[] {MIMES[3]} : MIMES);
                i.addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION | Intent.FLAG_GRANT_READ_URI_PERMISSION);
                startActivityForResult(i, REQ_OPEN);
            });
        }
        /** The editor's autosave (base64), kept in an app-private file until discard(). A failure is only logged: the document stays open and dirty. */
        @JavascriptInterface public void autosave(String app, String name, String base64) {
            try { MainActivity.this.autosave(app, name, Base64.decode(base64, Base64.DEFAULT)); }
            catch (Exception e) { android.util.Log.w("SuiteHost", "autosave: " + e.getMessage()); }
        }
        /** The document was saved or the user discarded its unsaved changes. */
        @JavascriptInterface public void discard(String app) {
            File f = recoverFile(app);
            if (f != null) f.delete();
        }
        /** bytes as base64 (the call crosses the JS bridge as a string). The answer is hostSaved({ok, name, error}) in the page. */
        @JavascriptInterface public void save(String name, String base64) {
            try { MainActivity.this.save(name, Base64.decode(base64, Base64.DEFAULT)); }
            catch (Exception e) { done(false, name, String.valueOf(e.getMessage())); }
        }
    }

    // ---- the virtual origin: shell and dists from assets ----
    class Client extends WebViewClient {
        @Override public void onPageFinished(WebView v, String url) { v.requestFocus(); }
        // A crashed or killed renderer would take the app down: a new WebView loads the shell, which reopens the recovery file.
        @Override public boolean onRenderProcessGone(WebView v, RenderProcessGoneDetail d) {
            if (v != web) return true;
            root.removeView(web);
            web.destroy();
            web = new WebView(MainActivity.this);
            root.addView(web, new FrameLayout.LayoutParams(-1, -1));
            setUp();
            web.loadUrl(ORIGIN);
            return true;
        }
        @Override public boolean shouldOverrideUrlLoading(WebView v, WebResourceRequest r) { return !HOST.equals(r.getUrl().getHost()); }   // no other page, ever
        @Override public WebResourceResponse shouldInterceptRequest(WebView v, WebResourceRequest r) {
            Uri u = r.getUrl();
            if (!HOST.equals(u.getHost()) || !"GET".equals(r.getMethod())) return status(404, "Not Found");
            String p = u.getPath() == null ? "/" : u.getPath();
            if (p.equals("/__open")) return opened();
            if (p.startsWith("/__recover/")) return recovered(p.substring(11));
            String asset;
            if (p.equals("/")) asset = "shell/index.html";
            else if (p.equals("/shell-config.json")) asset = "shell-config.json";
            else if (p.startsWith("/shell/")) asset = "shell/" + p.substring(7);
            else {
                int s = p.indexOf('/', 1);
                String dist = s < 0 ? null : DISTS.get(p.substring(1, s));
                if (dist == null) return status(404, "Not Found");
                String rest = p.substring(s + 1);
                asset = dist + "/" + (rest.isEmpty() ? "index.html" : rest);
            }
            if (asset.contains("..")) return status(404, "Not Found");
            try {
                InputStream in = getAssets().open("web/" + asset);
                return respond(asset, in, asset.endsWith("/index.html") && !asset.startsWith("shell/") ? editorCsp(asset) : SHELL_CSP);
            } catch (IOException e) { return status(404, "Not Found"); }
        }
    }

    WebResourceResponse status(int code, String reason) {
        return new WebResourceResponse("text/plain", "utf-8", code, reason, new HashMap<>(), new ByteArrayInputStream(new byte[0]));
    }

    /** The document parked by take()/load(), once: 200 with the bytes and the name in X-Name, or 204. */
    WebResourceResponse opened() {
        String n; byte[] b;
        synchronized (lock) {
            n = inName; b = inBytes;
            if (b != null) { curUri = inUri; curName = n; inName = null; inBytes = null; inUri = null; }
        }
        if (b == null) return status(204, "No Content");
        Map<String, String> h = new HashMap<>();
        h.put("X-Name", Uri.encode(n)); h.put("Cache-Control", "no-store"); h.put("Cross-Origin-Resource-Policy", "same-origin");
        return new WebResourceResponse("application/octet-stream", null, 200, "OK", h, new ByteArrayInputStream(b));
    }

    WebResourceResponse respond(String asset, InputStream in, String policy) {
        Map<String, String> h = new HashMap<>();
        h.put("Content-Security-Policy", policy);
        h.put("X-Content-Type-Options", "nosniff");
        h.put("Referrer-Policy", "no-referrer");
        h.put("Cross-Origin-Resource-Policy", "same-origin");
        h.put("Cache-Control", "no-cache");
        String type = type(asset);
        return new WebResourceResponse(type, type.startsWith("text/") || type.endsWith("json") || type.endsWith("javascript") ? "utf-8" : null, 200, "OK", h, in);
    }

    static String type(String f) {
        switch (ext(f)) {
            case "html": return "text/html"; case "js": case "mjs": return "text/javascript"; case "css": return "text/css";
            case "json": return "application/json"; case "wasm": return "application/wasm"; case "svg": return "image/svg+xml";
            case "png": return "image/png"; case "ico": return "image/x-icon"; case "ttf": return "font/ttf";
            case "webmanifest": return "application/manifest+json";
            default: return "application/octet-stream";
        }
    }

    /** The editor page's CSP, as suite-serve's editor_csp builds it: 'self', wasm, and the SHA-256 of each inline script and style. Cached per page. */
    synchronized String editorCsp(String asset) {
        String c = csp.get(asset);
        if (c != null) return c;
        try (InputStream in = getAssets().open("web/" + asset)) {
            ByteArrayOutputStream o = new ByteArrayOutputStream();
            byte[] buf = new byte[8192];
            for (int n; (n = in.read(buf)) > 0; ) o.write(buf, 0, n);
            String html = o.toString("UTF-8");
            c = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'" + hashes(html, "script") + "; style-src 'self'" + hashes(html, "style")
                + "; img-src 'self' data: blob:; object-src 'none'; frame-ancestors 'self'; base-uri 'none'; form-action 'self'";
        } catch (Exception e) { c = SHELL_CSP; }
        csp.put(asset, c);
        return c;
    }

    static String hashes(String html, String tag) throws Exception {
        Matcher m = Pattern.compile("<" + tag + "(\\s[^>]*)?>(.*?)</" + tag + ">", Pattern.DOTALL).matcher(html);
        StringBuilder sb = new StringBuilder();
        while (m.find()) {
            if (m.group(1) != null && m.group(1).contains("src=")) continue;
            byte[] d = MessageDigest.getInstance("SHA-256").digest(m.group(2).getBytes(StandardCharsets.UTF_8));
            sb.append(" 'sha256-").append(Base64.encodeToString(d, Base64.NO_WRAP)).append("'");
        }
        return sb.toString();
    }

    // ---- what a WebView does not do for itself ----
    class Chrome extends WebChromeClient {
        @Override public boolean onJsConfirm(WebView v, String url, String msg, JsResult r) {
            new AlertDialog.Builder(MainActivity.this).setMessage(msg).setCancelable(false)
                .setPositiveButton(android.R.string.ok, (d, w) -> r.confirm()).setNegativeButton(android.R.string.cancel, (d, w) -> r.cancel()).show();
            return true;
        }
        @Override public boolean onJsAlert(WebView v, String url, String msg, JsResult r) {
            new AlertDialog.Builder(MainActivity.this).setMessage(msg).setCancelable(false).setPositiveButton(android.R.string.ok, (d, w) -> r.confirm()).show();
            return true;
        }
        // <input type=file> inside an editor (insert image)
        @Override public boolean onShowFileChooser(WebView v, ValueCallback<Uri[]> cb, FileChooserParams p) {
            if (chooser != null) chooser.onReceiveValue(null);
            chooser = cb;
            try { startActivityForResult(p.createIntent(), REQ_CHOOSER); }
            catch (Exception e) { chooser = null; return false; }
            return true;
        }
    }
}
