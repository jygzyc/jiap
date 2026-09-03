package com.androguard.decompilefixtures;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.security.SecureRandom;
import java.util.Arrays;
import javax.crypto.Cipher;
import javax.crypto.Mac;
import javax.crypto.SecretKeyFactory;
import javax.crypto.spec.IvParameterSpec;
import javax.crypto.spec.PBEKeySpec;
import javax.crypto.spec.SecretKeySpec;

/** Crypto via standard JCA — byte buffers, try/catch, static calls, constants. */
public final class CryptoFixtures {
    private CryptoFixtures() {}

    public static byte[] sha256(byte[] input) throws NoSuchAlgorithmException {
        MessageDigest md = MessageDigest.getInstance("SHA-256");
        return md.digest(input);
    }

    public static byte[] sha256String(String text) throws NoSuchAlgorithmException {
        return sha256(text.getBytes(StandardCharsets.UTF_8));
    }

    public static byte[] hmacSha256(byte[] key, byte[] data) throws Exception {
        Mac mac = Mac.getInstance("HmacSHA256");
        mac.init(new SecretKeySpec(key, "HmacSHA256"));
        return mac.doFinal(data);
    }

    public static byte[] aesCbcEncrypt(byte[] key, byte[] iv, byte[] plaintext) throws Exception {
        Cipher cipher = Cipher.getInstance("AES/CBC/PKCS5Padding");
        cipher.init(Cipher.ENCRYPT_MODE, new SecretKeySpec(key, "AES"), new IvParameterSpec(iv));
        return cipher.doFinal(plaintext);
    }

    public static byte[] aesCbcDecrypt(byte[] key, byte[] iv, byte[] ciphertext) throws Exception {
        Cipher cipher = Cipher.getInstance("AES/CBC/PKCS5Padding");
        cipher.init(Cipher.DECRYPT_MODE, new SecretKeySpec(key, "AES"), new IvParameterSpec(iv));
        return cipher.doFinal(ciphertext);
    }

    public static byte[] pbkdf2Sha256(char[] password, byte[] salt, int iterations, int keyLen)
            throws Exception {
        PBEKeySpec spec = new PBEKeySpec(password, salt, iterations, keyLen * 8);
        SecretKeyFactory factory = SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256");
        return factory.generateSecret(spec).getEncoded();
    }

    public static byte[] secureRandomBytes(int len) {
        byte[] out = new byte[len];
        new SecureRandom().nextBytes(out);
        return out;
    }

    /** XOR stream cipher — tight loop over bytes (common in obfuscated apps). */
    public static byte[] xorStream(byte[] data, byte[] key) {
        byte[] out = new byte[data.length];
        for (int i = 0; i < data.length; i++) {
            out[i] = (byte) (data[i] ^ key[i % key.length]);
        }
        return out;
    }

    /** Constant-time-ish compare (security-sensitive branch pattern). */
    public static boolean constantTimeEquals(byte[] a, byte[] b) {
        if (a.length != b.length) {
            return false;
        }
        int diff = 0;
        for (int i = 0; i < a.length; i++) {
            diff |= a[i] ^ b[i];
        }
        return diff == 0;
    }

    public static int demoCrypto() throws Exception {
        byte[] msg = "hello-droid2web".getBytes(StandardCharsets.UTF_8);
        byte[] hash = sha256(msg);
        byte[] mac = hmacSha256("secret-key".getBytes(StandardCharsets.UTF_8), msg);
        byte[] key = Arrays.copyOf(hash, 16);
        byte[] iv = secureRandomBytes(16);
        byte[] enc = aesCbcEncrypt(key, iv, msg);
        byte[] dec = aesCbcDecrypt(key, iv, enc);
        byte[] derived = pbkdf2Sha256("password".toCharArray(), iv, 1000, 32);
        byte[] xored = xorStream(msg, key);
        boolean same = constantTimeEquals(dec, msg);
        return hash.length + mac.length + enc.length + derived.length + xored.length + (same ? 1 : 0);
    }
}
