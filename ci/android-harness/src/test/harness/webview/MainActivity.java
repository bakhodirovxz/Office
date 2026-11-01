package test.harness.webview;

import android.app.Activity;
import android.os.Bundle;
import android.util.Log;
import android.webkit.JavascriptInterface;
import android.webkit.WebSettings;
import android.webkit.WebView;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;

/**
 * Does what the HRM mobile app does (docs/phase0/hrm-current-state.md, MOB): fetch the editor config natively, build inline HTML that
 * loads the Document Server's api.js and runs `new DocsAPI.DocEditor("editor", config)`, and show it in a WebView with
 * loadDataWithBaseURL(baseUrl = the Document Server). Extras: ds (baseUrl, no trailing slash), cfg (editor-config URL on the backend).
 * Everything the page reports goes to logcat tag HARNESS. Commands go through the WebView's devtools socket (cdp.mjs).
 */
public class MainActivity extends Activity {
    static final String TAG = "HARNESS";
    WebView wv;

    class Bridge {
        @JavascriptInterface public void event(String json) { Log.i(TAG, "event " + json); }
    }

    @Override protected void onCreate(Bundle b) {
        super.onCreate(b);
        WebView.setWebContentsDebuggingEnabled(true);
        wv = new WebView(this);
        WebSettings s = wv.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        s.setMediaPlaybackRequiresUserGesture(false);
        wv.addJavascriptInterface(new Bridge(), "HarnessHost");
        setContentView(wv);
        wv.requestFocus(); // a WebView without view focus never shows the IME
        final String ds = getIntent().getStringExtra("ds"), cfg = getIntent().getStringExtra("cfg");
        new Thread(() -> {
            try {
                String json = get(cfg);
                Log.i(TAG, "config fetched " + json.length() + " bytes from " + cfg.replaceAll("\\?.*", ""));
                runOnUiThread(() -> wv.loadDataWithBaseURL(ds, html(ds, json), "text/html", "utf-8", null));
            } catch (Exception e) { Log.e(TAG, "config fetch failed", e); }
        }).start();
    }

    static String get(String u) throws Exception {
        HttpURLConnection c = (HttpURLConnection) new URL(u).openConnection();
        try (InputStream in = c.getInputStream()) {
            ByteArrayOutputStream o = new ByteArrayOutputStream();
            byte[] buf = new byte[8192];
            for (int n; (n = in.read(buf)) > 0; ) o.write(buf, 0, n);
            return o.toString("UTF-8");
        }
    }

    static String html(String ds, String cfgJson) {
        return "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"
            + "<style>html,body{margin:0;height:100%}#editor{height:100%}</style></head><body><div id=\"editor\"></div>"
            + "<script src=\"" + ds + "/web-apps/apps/api/documents/api.js\"></script><script>"
            + "function post(o){HarnessHost.event(JSON.stringify(o));}"
            + "window.onerror=function(m){post({err:String(m)});};"
            + "var cfg=" + cfgJson.replace("</", "<\\/") + ";"
            + "cfg.events={onDocumentReady:function(){post({ev:'ready'});},onDocumentStateChange:function(e){post({ev:'dirty',v:e.data});},"
            + "onSaved:function(e){post({ev:'saved',v:e.data.version});},onClosed:function(e){post({ev:'closed',status:e.data.status});},"
            + "onWarning:function(e){post({ev:'warning',v:e.data});},onError:function(e){post({ev:'error',v:e.data});}};"
            + "post({ev:'boot',origin:location.origin,url:location.href,docOrigin:window.origin});"
            + "window.__editor=new DocsAPI.DocEditor('editor',cfg);</script></body></html>";
    }
}
