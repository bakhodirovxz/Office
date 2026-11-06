// Licence: MIT OR Apache-2.0. Desktop-JVM check that office.android.Signatures agrees with crates/suite-core/src/plugins.rs (not part of the APK).
// Usage: SigCheck <fixture-pkg-dir> <test-a.pub>
package office.android;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;

public class SigCheck {
    static int pass = 0, fail = 0;

    static void ok(boolean c, String what) { if (c) pass++; else { fail++; System.out.println("FAIL " + what); } }

    /** Expects check to throw exactly this message (null: succeed with signer). */
    static void expect(String what, byte[] file, byte[] msg, String err, String signer) {
        try {
            String s = Signatures.checkSignature(file, msg);
            ok(err == null && s.equals(signer), what + " (got signer " + s + ")");
        } catch (Exception e) { ok(err != null && e.getMessage().equals(err), what + " (got " + e.getMessage() + ")"); }
    }

    public static void main(String[] a) throws Exception {
        String[][] rfc = {
            {"d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a", "", "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"},
            {"3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c", "72", "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00"},
            {"fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025", "af82", "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a"}};
        for (String[] v : rfc) {
            byte[] m = Signatures.unhex(v[1]), m2 = Arrays.copyOf(m, m.length + 1);
            byte[] f = ("ed25519 " + v[0] + " " + v[2] + "\n").getBytes(StandardCharsets.UTF_8);
            expect("rfc8032 " + v[0].substring(0, 6), f, m, null, v[0]);
            expect("rfc8032 +1 byte " + v[0].substring(0, 6), f, m2, Signatures.MISMATCH, null);
        }
        Path pkg = Paths.get(a[0]);
        byte[] toml = Files.readAllBytes(pkg.resolve("plugin.toml")), wasm = Files.readAllBytes(pkg.resolve("plugin.wasm")), tsv = Files.readAllBytes(pkg.resolve("strings.tsv"));
        byte[] sig = Files.readAllBytes(pkg.resolve("SIGNATURE"));
        String pub = new String(Files.readAllBytes(Paths.get(a[1])), StandardCharsets.UTF_8).trim();
        byte[] msg = Signatures.signedMessage(toml, wasm, tsv);
        expect("fixture verifies", sig, msg, null, pub);
        byte[] w2 = wasm.clone(); w2[w2.length / 2] ^= 1;
        expect("changed wasm byte", sig, Signatures.signedMessage(toml, w2, tsv), Signatures.MISMATCH, null);
        expect("strings.tsv dropped", sig, Signatures.signedMessage(toml, wasm, null), Signatures.MISMATCH, null);
        String t = new String(sig, StandardCharsets.UTF_8).trim();
        int last = t.length() - 1;
        String flipped = t.substring(0, last) + (t.charAt(last) == '0' ? '1' : '0');
        expect("flipped signature digit", flipped.getBytes(StandardCharsets.UTF_8), msg, Signatures.MISMATCH, null);
        String[] p = t.split(" ");
        String[] bad = {"", "ed25519", "rsa " + p[1] + " " + p[2], t.toUpperCase(), t + "x", p[0] + "  " + p[1] + " " + p[2], t + "\n\n", " " + t, t + "\r"};
        for (String b : bad) expect("malformed [" + b + "]", b.getBytes(StandardCharsets.UTF_8), msg, Signatures.MALFORMED, null);
        expect("invalid utf-8", new byte[] {(byte) 0xff, (byte) 0xfe}, msg, Signatures.MALFORMED, null);
        expect("CRLF ending accepted", (t + "\r\n").getBytes(StandardCharsets.UTF_8), msg, null, pub);
        expect("no ending accepted", t.getBytes(StandardCharsets.UTF_8), msg, null, pub);
        // s >= L is refused, and a key that is not on the curve (y = 2 has no square root) does not verify
        byte[] big = Signatures.unhex(p[2]); Arrays.fill(big, 32, 64, (byte) 0xff);
        expect("s >= L", ("ed25519 " + p[1] + " " + Signatures.hex(big) + "\n").getBytes(StandardCharsets.UTF_8), msg, Signatures.MISMATCH, null);
        expect("off-curve key", ("ed25519 02" + "00".repeat(31) + " " + p[2] + "\n").getBytes(StandardCharsets.UTF_8), msg, Signatures.MISMATCH, null);
        // timing: uncached verifies of the raw primitive
        byte[] key = Signatures.unhex(pub), sg = Signatures.unhex(p[2]);
        double[] ms = new double[20];
        for (int i = 0; i < 20; i++) { long t0 = System.nanoTime(); ok(Signatures.verify(key, msg, sg), "timed verify"); ms[i] = (System.nanoTime() - t0) / 1e6; }
        Arrays.sort(ms);
        System.out.printf("median verify: %.2f ms (min %.2f, max %.2f, 20 runs)%n", (ms[9] + ms[10]) / 2, ms[0], ms[19]);
        System.out.println("passed " + pass + ", failed " + fail);
        System.exit(fail == 0 ? 0 : 1);
    }
}
