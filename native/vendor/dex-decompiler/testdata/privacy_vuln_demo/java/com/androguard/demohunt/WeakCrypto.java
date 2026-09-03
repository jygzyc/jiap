package com.androguard.demohunt;

import javax.crypto.Cipher;
import javax.crypto.spec.SecretKeySpec;

/** Weak DES + SecretKeySpec (detector weak_crypto). */
public final class WeakCrypto {
    private WeakCrypto() {}

    public static byte[] useDes(byte[] input) {
        try {
            Cipher cipher = Cipher.getInstance("DES");
            SecretKeySpec key = new SecretKeySpec(new byte[] { 1, 2, 3, 4, 5, 6, 7, 8 }, "DES");
            cipher.init(Cipher.ENCRYPT_MODE, key);
            byte[] out = cipher.doFinal(input);
            return out;
        } catch (Throwable ignored) {
            return input;
        }
    }
}
