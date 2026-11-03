// Licence: MIT OR Apache-2.0. The Android plugin store (PLAN phase 7 WP9, section 8): first-party packages bundled in the APK assets
// (assets/web/plugin-pkg/<id>/, put there by build.sh from plugins/*) and packages the user installed from a file (files/plugins/<id>/ with an
// install.toml record, the same SHA-256 record as crates/suite-core/src/plugins.rs). On/off lives in SharedPreferences. There is no network grant
// on Android: the descriptors carry `grant: []`, and the app has no INTERNET permission at all.
// shortcut: this checks id, api, apps, sizes and the file names of a package, not every rule of plugin-api's manifest parser (the pack step of
// `cargo xt plugin pack` does that for bundled ones); the page host re-checks the descriptor and enforces the permissions at run time.
package office.android;

import android.content.Context;
import android.content.SharedPreferences;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.regex.Pattern;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;
import org.json.JSONArray;
import org.json.JSONObject;

final class Plugins {
    static final String ASSETS = "web/plugin-pkg";
    static final int MAX_WASM = 4 << 20, MAX_MANIFEST = 64 << 10, MAX_PACKAGE = 8 << 20, MAX_ENTRIES = 64;
    static final Set<String> FILES = new HashSet<>(Arrays.asList("plugin.toml", "plugin.wasm", "strings.tsv", "SIGNATURE"));
    static final Set<String> APPS = new HashSet<>(Arrays.asList("docs", "sheets", "slides", "pdf"));
    static final Pattern ID = Pattern.compile("[a-z0-9]([a-z0-9.-]{1,62})[a-z0-9]");

    static final class Item {
        String id = "", version = "", sha256 = "", problem = null;
        boolean bundled, enabled;
        JSONObject manifest = new JSONObject();
        byte[] wasm;
        String dirName = "";
    }

    final Context ctx;
    final SharedPreferences prefs;

    Plugins(Context c) { ctx = c; prefs = c.getSharedPreferences("plugins", Context.MODE_PRIVATE); }

    File root() { return new File(ctx.getFilesDir(), "plugins"); }

    static String hex(byte[] b) {
        StringBuilder s = new StringBuilder();
        for (byte x : b) s.append(String.format("%02x", x & 255));
        return s.toString();
    }

    static String sha(byte[] b) {
        try { return hex(MessageDigest.getInstance("SHA-256").digest(b)); } catch (Exception e) { throw new IllegalStateException(e); }
    }

    static byte[] readAll(InputStream in, int max) throws IOException {
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        byte[] buf = new byte[1 << 16];
        for (int n; (n = in.read(buf)) > 0; ) {
            o.write(buf, 0, n);
            if (o.size() > max) throw new IOException("a file in the package is too large");
        }
        return o.toByteArray();
    }

    /** Parses and checks one package; the Item carries the manifest as JSON and the digest of the wasm. */
    static Item parse(byte[] toml, byte[] wasm) throws IOException {
        if (wasm.length > MAX_WASM) throw new IOException("plugin.wasm is over 4 MiB");
        if (wasm.length < 8 || wasm[0] != 0 || wasm[1] != 'a' || wasm[2] != 's' || wasm[3] != 'm') throw new IOException("plugin.wasm is not a WebAssembly module");
        if (toml.length > MAX_MANIFEST) throw new IOException("plugin.toml is over 64 KiB");
        Item it = new Item();
        it.manifest = Toml.parse(new String(toml, StandardCharsets.UTF_8));
        it.id = it.manifest.optString("id");
        it.version = it.manifest.optString("version");
        if (!ID.matcher(it.id).matches() || it.id.contains("..")) throw new IOException("bad plugin id");
        if (it.manifest.optInt("api", 0) != 1) throw new IOException("API " + it.manifest.opt("api") + " is not supported");
        JSONArray apps = it.manifest.optJSONArray("apps");
        if (apps == null || apps.length() == 0) throw new IOException("the plugin names no app");
        for (int i = 0; i < apps.length(); i++) if (!APPS.contains(apps.optString(i))) throw new IOException("unknown app " + apps.optString(i));
        it.wasm = wasm;
        it.sha256 = sha(wasm);
        return it;
    }

    String record(Item it, byte[] toml) { return "sha256 = \"" + it.sha256 + "\"\nmanifest_sha256 = \"" + sha(toml) + "\"\n"; }

    /** Bundled first, then installed; an unusable package is listed with its problem and never served. */
    List<Item> list() {
        List<Item> out = new ArrayList<>();
        try {
            String[] ids = ctx.getAssets().list(ASSETS);
            for (String d : ids == null ? new String[0] : ids) {
                try (InputStream t = ctx.getAssets().open(ASSETS + "/" + d + "/plugin.toml"); InputStream w = ctx.getAssets().open(ASSETS + "/" + d + "/plugin.wasm")) {
                    Item it = parse(readAll(t, MAX_MANIFEST), readAll(w, MAX_WASM));
                    it.bundled = true;
                    out.add(it);
                } catch (Exception e) { out.add(broken(d, e)); }
            }
        } catch (IOException ignored) { /* no bundled plugins */ }
        File[] dirs = root().listFiles(File::isDirectory);
        if (dirs != null) {
            Arrays.sort(dirs);
            for (File d : dirs) {
                try {
                    byte[] toml = readAll(new java.io.FileInputStream(new File(d, "plugin.toml")), MAX_MANIFEST);
                    Item it = parse(toml, readAll(new java.io.FileInputStream(new File(d, "plugin.wasm")), MAX_WASM));
                    String rec = new String(readAll(new java.io.FileInputStream(new File(d, "install.toml")), 4096), StandardCharsets.UTF_8);
                    if (!rec.equals(record(it, toml))) it.problem = "the package changed since it was installed";
                    it.dirName = d.getName();
                    out.add(it);
                } catch (Exception e) { out.add(broken(d.getName(), e)); }
            }
        }
        for (Item it : out) it.enabled = it.problem == null && prefs.getBoolean("on." + it.id, it.bundled);
        return out;
    }

    static Item broken(String name, Exception e) {
        Item it = new Item();
        it.id = name; it.problem = String.valueOf(e.getMessage());
        return it;
    }

    void setEnabled(String id, boolean on) { prefs.edit().putBoolean("on." + id, on).apply(); }

    /** The page host's descriptors (docs/plugins.md 9a) of the enabled ones. `grant` is always empty: Android grants no network. */
    JSONArray descriptors() throws Exception {
        JSONArray a = new JSONArray();
        for (Item it : list()) {
            if (!it.enabled) continue;
            a.put(new JSONObject().put("id", it.id).put("version", it.version).put("sha256", it.sha256).put("wasm", "/plugins/" + it.sha256)
                .put("manifest", it.manifest).put("grant", new JSONArray()));
        }
        return a;
    }

    /** `/plugins/<sha256>`: the wasm of an enabled plugin, hashed again as it is read. null for anything else. */
    byte[] wasm(String sha) {
        if (!sha.matches("[0-9a-f]{64}")) return null;
        for (Item it : list()) if (it.enabled && it.sha256.equals(sha) && sha(it.wasm).equals(sha)) return it.wasm;
        return null;
    }

    /** Installs a package zip (plugin.toml + plugin.wasm [+ strings.tsv]); returns the id. The user turns it on afterwards. */
    String install(InputStream zip) throws IOException {
        byte[] toml = null, wasm = null, tsv = null;
        Set<String> seen = new HashSet<>();
        int total = 0;
        try (ZipInputStream z = new ZipInputStream(zip)) {
            for (ZipEntry e; (e = z.getNextEntry()) != null; ) {
                String n = e.getName();
                if (!FILES.contains(n) || !seen.add(n) || seen.size() > MAX_ENTRIES) throw new IOException("the package holds an entry that is not allowed: " + n);
                byte[] b = readAll(z, n.equals("plugin.wasm") ? MAX_WASM : MAX_MANIFEST);
                if (n.equals("plugin.toml")) toml = b; else if (n.equals("plugin.wasm")) wasm = b; else if (n.equals("strings.tsv")) tsv = b;
                if ((total += b.length) > MAX_PACKAGE) throw new IOException("the package is over 8 MiB");
            }
        }
        if (toml == null || wasm == null) throw new IOException("the package needs plugin.toml and plugin.wasm");
        Item it = parse(toml, wasm);
        for (Item o : list()) if (o.bundled && o.id.equals(it.id)) throw new IOException("a bundled plugin has the id " + it.id);
        File dir = new File(root(), it.id), tmp = new File(root(), it.id + ".tmp");
        deleteAll(tmp);
        if (!tmp.mkdirs()) throw new IOException("cannot create the plugin folder");
        put(tmp, "plugin.toml", toml); put(tmp, "plugin.wasm", wasm);
        if (tsv != null) put(tmp, "strings.tsv", tsv);
        put(tmp, "install.toml", record(it, toml).getBytes(StandardCharsets.UTF_8));
        deleteAll(dir);
        if (!tmp.renameTo(dir)) throw new IOException("cannot keep the plugin");
        return it.id;
    }

    static void put(File dir, String name, byte[] b) throws IOException {
        try (FileOutputStream o = new FileOutputStream(new File(dir, name))) { o.write(b); o.getFD().sync(); }
    }

    static void deleteAll(File f) {
        File[] k = f.listFiles();
        if (k != null) for (File c : k) deleteAll(c);
        f.delete();
    }

    /** The rows of the Plugins page (web/plugins/page.js). Network is never offered: `network` is empty and the policy says so. */
    JSONObject page(String installed, boolean restart) throws Exception {
        JSONArray items = new JSONArray();
        for (Item it : list()) {
            JSONObject p = it.manifest.optJSONObject("permissions");
            items.put(new JSONObject().put("id", it.id).put("name", it.manifest.opt("name") == null ? it.id : it.manifest.opt("name")).put("version", it.version)
                .put("publisher", it.manifest.optString("publisher")).put("document", p == null ? "none" : p.optString("document", "none"))
                .put("network", new JSONArray()).put("grant", new JSONArray()).put("enabled", it.enabled).put("machine", false)
                .put("problem", it.problem == null ? JSONObject.NULL : it.problem));
        }
        JSONObject o = new JSONObject().put("type", "plugins").put("ok", true).put("items", items)
            .put("policy", new JSONObject().put("user_install", true).put("user_grants", false)).put("restart", restart);
        if (installed != null) o.put("installed", installed);
        return o;
    }
}
