// Licence: MIT OR Apache-2.0. Publisher signatures of plugin packages (PLAN phase 7, decision O4), the Java twin of signed_message and
// check_signature in crates/suite-core/src/plugins.rs: the same message bytes, the same SIGNATURE grammar, the same messages. No android.* import,
// so packaging/android/sigcheck.sh runs it on a desktop JVM against the Rust-signed fixture. Ed25519 is written from RFC 8032 section 5.1
// (java.security has it only from API 33; minSdk is 26). It verifies public data only and never holds a secret, so it is not constant time.
// Only Java 8 / API 26 library calls (no BigInteger.TWO, which Android has from API 33).
package office.android;

import java.io.IOException;
import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.HashMap;
import java.util.Map;

final class Signatures {
    private Signatures() {}

    static final String MALFORMED = "the SIGNATURE file is malformed", MISMATCH = "the signature does not match the package";

    static String hex(byte[] b) {
        StringBuilder s = new StringBuilder();
        for (byte x : b) s.append(Character.forDigit((x >> 4) & 15, 16)).append(Character.forDigit(x & 15, 16));
        return s.toString();
    }

    /** Bytes from an even number of hex digits (the caller checks the digits with isHex). */
    static byte[] unhex(String s) {
        byte[] b = new byte[s.length() / 2];
        for (int i = 0; i < b.length; i++) b[i] = (byte) Integer.parseInt(s.substring(2 * i, 2 * i + 2), 16);
        return b;
    }

    static boolean isHex(String s, int n) {
        if (s.length() != n) return false;
        for (int i = 0; i < n; i++) { char c = s.charAt(i); if (!(c >= '0' && c <= '9' || c >= 'a' && c <= 'f')) return false; }
        return true;
    }

    static byte[] digest(String alg, byte[]... parts) {
        try {
            MessageDigest d = MessageDigest.getInstance(alg);
            for (byte[] p : parts) d.update(p);
            return d.digest();
        } catch (Exception e) { throw new IllegalStateException(e); }
    }

    /** What a publisher signs: "plugin-package-signature-v1\n", then "<name> <sha256 hex>\n" for plugin.toml, plugin.wasm and strings.tsv (only when present). */
    static byte[] signedMessage(byte[] toml, byte[] wasm, byte[] stringsOrNull) {
        StringBuilder m = new StringBuilder("plugin-package-signature-v1\n");
        m.append("plugin.toml ").append(hex(digest("SHA-256", toml))).append('\n');
        m.append("plugin.wasm ").append(hex(digest("SHA-256", wasm))).append('\n');
        if (stringsOrNull != null) m.append("strings.tsv ").append(hex(digest("SHA-256", stringsOrNull))).append('\n');
        return m.toString().getBytes(StandardCharsets.US_ASCII);
    }

    // shortcut: the cache is cleared when it holds 256 results; plenty for one phone's packages
    private static final Map<String, String> CACHE = new HashMap<>();

    /** A SIGNATURE file checked against the key it names; returns that key (64 lower-case hex). Whether it is trusted is the caller's test. */
    static String checkSignature(byte[] file, byte[] msg) throws IOException {
        String key = new String(file, StandardCharsets.ISO_8859_1) + "|" + hex(digest("SHA-256", msg));
        String r;
        synchronized (CACHE) { r = CACHE.get(key); }
        if (r == null) {
            r = check(file, msg);
            synchronized (CACHE) { if (CACHE.size() >= 256) CACHE.clear(); CACHE.put(key, r); }
        }
        if (!isHex(r, 64)) throw new IOException(r);
        return r;
    }

    /** The signer, or the error text. Grammar: "ed25519 <key: 64 hex> <signature: 128 hex>", then optionally "\n" or "\r\n". */
    private static String check(byte[] file, byte[] msg) {
        String t;
        try {
            t = StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT).onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(file)).toString();
        } catch (CharacterCodingException e) { return MALFORMED; }
        if (t.endsWith("\n")) {
            t = t.substring(0, t.length() - 1);
            if (t.endsWith("\r")) t = t.substring(0, t.length() - 1);
        }
        String[] p = t.split(" ", -1);
        if (p.length != 3 || !p[0].equals("ed25519") || !isHex(p[1], 64) || !isHex(p[2], 128)) return MALFORMED;
        return verify(unhex(p[1]), msg, unhex(p[2])) ? p[1] : MISMATCH;
    }

    // ---- Ed25519 verification, RFC 8032 section 5.1 ----

    static final BigInteger P = BigInteger.ONE.shiftLeft(255).subtract(BigInteger.valueOf(19));
    static final BigInteger L = BigInteger.ONE.shiftLeft(252).add(new BigInteger("27742317777372353535851937790883648493"));
    static final BigInteger D = BigInteger.valueOf(-121665).multiply(BigInteger.valueOf(121666).modInverse(P)).mod(P);
    static final BigInteger D2 = D.shiftLeft(1).mod(P);
    static final BigInteger SQRT_M1 = BigInteger.valueOf(2).modPow(P.subtract(BigInteger.ONE).shiftRight(2), P);
    static final BigInteger[] B;
    static {
        BigInteger y = BigInteger.valueOf(4).multiply(BigInteger.valueOf(5).modInverse(P)).mod(P);
        BigInteger x = recoverX(y, false);
        B = new BigInteger[] {x, y, BigInteger.ONE, x.multiply(y).mod(P)};
    }

    /** 5.1.3: x from y and the sign bit; null when y >= p, there is no square root, or x == 0 with the sign bit set. */
    static BigInteger recoverX(BigInteger y, boolean sign) {
        if (y.compareTo(P) >= 0) return null;
        BigInteger y2 = y.multiply(y).mod(P);
        BigInteger x2 = y2.subtract(BigInteger.ONE).multiply(D.multiply(y2).add(BigInteger.ONE).modInverse(P)).mod(P);
        if (x2.signum() == 0) return sign ? null : BigInteger.ZERO;
        BigInteger x = x2.modPow(P.add(BigInteger.valueOf(3)).shiftRight(3), P);
        if (!x.multiply(x).subtract(x2).mod(P).equals(BigInteger.ZERO)) x = x.multiply(SQRT_M1).mod(P);
        if (!x.multiply(x).subtract(x2).mod(P).equals(BigInteger.ZERO)) return null;
        if (x.testBit(0) != sign) x = P.subtract(x);
        return x;
    }

    /** 5.1.4, extended coordinates (X, Y, Z, T): X3 = E*F, Y3 = G*H, T3 = E*H, Z3 = F*G. */
    static BigInteger[] add(BigInteger[] p, BigInteger[] q) {
        BigInteger a = p[1].subtract(p[0]).multiply(q[1].subtract(q[0])).mod(P), b = p[1].add(p[0]).multiply(q[1].add(q[0])).mod(P);
        BigInteger c = p[3].multiply(D2).multiply(q[3]).mod(P), d = p[2].shiftLeft(1).multiply(q[2]).mod(P);
        BigInteger e = b.subtract(a), f = d.subtract(c), g = d.add(c), h = b.add(a);
        return new BigInteger[] {e.multiply(f).mod(P), g.multiply(h).mod(P), f.multiply(g).mod(P), e.multiply(h).mod(P)};
    }

    static BigInteger[] mul(BigInteger[] p, BigInteger k) {
        BigInteger[] r = {BigInteger.ZERO, BigInteger.ONE, BigInteger.ONE, BigInteger.ZERO};
        for (int i = k.bitLength() - 1; i >= 0; i--) {
            r = add(r, r);
            if (k.testBit(i)) r = add(r, p);
        }
        return r;
    }

    static BigInteger le(byte[] b, int off, int len) {
        byte[] be = new byte[len + 1];   // a leading zero keeps it positive
        for (int i = 0; i < len; i++) be[len - i] = b[off + i];
        return new BigInteger(be);
    }

    static byte[] encode(BigInteger[] pt) {
        BigInteger zi = pt[2].modInverse(P), x = pt[0].multiply(zi).mod(P), y = pt[1].multiply(zi).mod(P);
        byte[] out = new byte[32], be = y.toByteArray();
        for (int i = 0; i < 32 && i < be.length; i++) out[i] = be[be.length - 1 - i];
        if (x.testBit(0)) out[31] |= (byte) 0x80;
        return out;
    }

    /** 5.1.7 as the Rust verifier (ring) does it: s < L, A decodes, and encode([s]B - [h]A) equals the R bytes, h = SHA-512(R || A || M) mod L. */
    static boolean verify(byte[] key, byte[] msg, byte[] sig) {
        if (key.length != 32 || sig.length != 64) return false;
        BigInteger s = le(sig, 32, 32);
        if (s.compareTo(L) >= 0) return false;
        byte[] a = key.clone();
        boolean sign = (a[31] & 0x80) != 0;
        a[31] &= 0x7f;
        BigInteger y = le(a, 0, 32), x = recoverX(y, sign);
        if (x == null) return false;
        BigInteger nx = P.subtract(x).mod(P);
        BigInteger[] minusA = {nx, y, BigInteger.ONE, nx.multiply(y).mod(P)};
        byte[] r = Arrays.copyOfRange(sig, 0, 32);
        BigInteger h = le(digest("SHA-512", r, key, msg), 0, 64).mod(L);
        return Arrays.equals(encode(add(mul(B, s), mul(minusA, h))), r);
    }
}
