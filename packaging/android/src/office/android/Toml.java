// Licence: MIT OR Apache-2.0. The TOML subset a plugin.toml uses, as a JSONObject (the Android side has no TOML library; the Rust side converts with
// toml -> serde_json, and scripts/mobile/toml-check.mjs-style comparison of the two outputs is the test, see docs/phase7/android.md).
// Supported: comments, bare and "quoted" keys, dotted keys, [table], [[array of tables]], basic and 'literal' strings, integers, floats, booleans,
// arrays (multi-line), inline tables. Not supported (refused): multi-line strings, dates, a table defined twice.
package office.android;

import java.io.IOException;
import org.json.JSONArray;
import org.json.JSONObject;

final class Toml {
    final String s;
    int i;

    Toml(String s) { this.s = s; }

    static JSONObject parse(String text) throws IOException {
        try { return new Toml(text).doc(); }
        catch (IOException e) { throw e; }
        catch (Exception e) { throw new IOException("plugin.toml: " + e.getMessage()); }
    }

    IOException bad(String what) { return new IOException("plugin.toml: " + what + " near offset " + i); }

    JSONObject doc() throws Exception {
        JSONObject root = new JSONObject(), cur = root;
        for (;;) {
            blank(true);
            if (i >= s.length()) return root;
            if (s.charAt(i) == '[') {
                boolean arr = s.startsWith("[[", i);
                i += arr ? 2 : 1;
                String[] k = path();
                if (!s.startsWith(arr ? "]]" : "]", i)) throw bad("unclosed table header");
                i += arr ? 2 : 1;
                cur = root;
                for (int n = 0; n < k.length; n++) {
                    boolean last = n == k.length - 1;
                    Object o = cur.opt(k[n]);
                    if (last && arr) {
                        JSONArray a = o == null ? new JSONArray() : o instanceof JSONArray ? (JSONArray) o : null;
                        if (a == null) throw bad("table " + k[n] + " is defined twice");
                        if (o == null) cur.put(k[n], a);
                        a.put(cur = new JSONObject());
                    } else if (o == null) { cur.put(k[n], cur = new JSONObject()); if (last) continue; }
                    else if (o instanceof JSONArray) { JSONArray a = (JSONArray) o; cur = a.getJSONObject(a.length() - 1); }
                    else if (o instanceof JSONObject) { if (last) throw bad("table " + k[n] + " is defined twice"); cur = (JSONObject) o; }
                    else throw bad("bad table " + k[n]);
                }
            } else {
                String[] k = path();
                blank(false);
                if (i >= s.length() || s.charAt(i++) != '=') throw bad("expected =");
                blank(false);
                JSONObject t = cur;
                for (int n = 0; n < k.length - 1; n++) {
                    JSONObject nx = t.optJSONObject(k[n]);
                    if (nx == null) { if (t.has(k[n])) throw bad("key " + k[n] + " is not a table"); t.put(k[n], nx = new JSONObject()); }
                    t = nx;
                }
                if (t.has(k[k.length - 1])) throw bad("key " + k[k.length - 1] + " is set twice");
                t.put(k[k.length - 1], value());
            }
            blank(false);
            if (i < s.length() && s.charAt(i) != '\n' && s.charAt(i) != '\r') throw bad("unexpected text after a value");
        }
    }

    /** Skips spaces, tabs and comments; with `nl` also line ends. */
    void blank(boolean nl) {
        while (i < s.length()) {
            char c = s.charAt(i);
            if (c == ' ' || c == '\t' || (nl && (c == '\n' || c == '\r'))) i++;
            else if (c == '#') while (i < s.length() && s.charAt(i) != '\n') i++;
            else return;
        }
    }

    String[] path() throws IOException {
        java.util.ArrayList<String> k = new java.util.ArrayList<>();
        for (;;) {
            blank(false);
            if (i < s.length() && s.charAt(i) == '"') k.add(basic());
            else {
                int a = i;
                while (i < s.length() && (Character.isLetterOrDigit(s.charAt(i)) && s.charAt(i) < 128 || s.charAt(i) == '_' || s.charAt(i) == '-')) i++;
                if (a == i) throw bad("bad key");
                k.add(s.substring(a, i));
            }
            blank(false);
            if (i < s.length() && s.charAt(i) == '.') i++; else return k.toArray(new String[0]);
        }
    }

    String basic() throws IOException {
        if (s.startsWith("\"\"\"", i)) throw bad("multi-line strings are not supported");
        StringBuilder b = new StringBuilder();
        i++;
        for (;;) {
            if (i >= s.length() || s.charAt(i) == '\n') throw bad("unclosed string");
            char c = s.charAt(i++);
            if (c == '"') return b.toString();
            if (c != '\\') { b.append(c); continue; }
            char e = i < s.length() ? s.charAt(i++) : 0;
            switch (e) {
                case 'n': b.append('\n'); break; case 't': b.append('\t'); break; case 'r': b.append('\r'); break; case 'b': b.append('\b'); break;
                case 'f': b.append('\f'); break; case '"': b.append('"'); break; case '\\': b.append('\\'); break;
                case 'u': case 'U': {
                    int n = e == 'u' ? 4 : 8;
                    if (i + n > s.length()) throw bad("bad escape");
                    b.appendCodePoint(Integer.parseInt(s.substring(i, i + n), 16)); i += n; break;
                }
                default: throw bad("bad escape");
            }
        }
    }

    Object value() throws Exception {
        if (i >= s.length()) throw bad("missing value");
        char c = s.charAt(i);
        if (c == '"') return basic();
        if (c == '\'') {
            if (s.startsWith("'''", i)) throw bad("multi-line strings are not supported");
            int e = s.indexOf('\'', i + 1);
            if (e < 0 || s.substring(i, e).indexOf('\n') >= 0) throw bad("unclosed string");
            String v = s.substring(i + 1, e); i = e + 1; return v;
        }
        if (c == '[') {
            JSONArray a = new JSONArray();
            i++;
            for (;;) {
                blank(true);
                if (i < s.length() && s.charAt(i) == ']') { i++; return a; }
                a.put(value());
                blank(true);
                if (i < s.length() && s.charAt(i) == ',') i++;
                else if (i >= s.length() || s.charAt(i) != ']') throw bad("bad array");
            }
        }
        if (c == '{') {
            JSONObject o = new JSONObject();
            i++;
            for (;;) {
                blank(false);
                if (i < s.length() && s.charAt(i) == '}') { i++; return o; }
                String[] k = path();
                blank(false);
                if (i >= s.length() || s.charAt(i++) != '=') throw bad("expected =");
                blank(false);
                JSONObject t = o;
                for (int n = 0; n < k.length - 1; n++) { JSONObject nx = t.optJSONObject(k[n]); if (nx == null) t.put(k[n], nx = new JSONObject()); t = nx; }
                t.put(k[k.length - 1], value());
                blank(false);
                if (i < s.length() && s.charAt(i) == ',') i++;
                else if (i >= s.length() || s.charAt(i) != '}') throw bad("bad inline table");
            }
        }
        int a = i;
        while (i < s.length() && ",]}\n\r #\t".indexOf(s.charAt(i)) < 0) i++;
        String w = s.substring(a, i).replace("_", "");
        if (w.equals("true")) return true;
        if (w.equals("false")) return false;
        try { return w.contains(".") || w.contains("e") || w.contains("E") ? (Object) Double.valueOf(w) : (Object) Long.valueOf(w); }
        catch (NumberFormatException e) { throw bad("bad value " + w); }
    }
}
