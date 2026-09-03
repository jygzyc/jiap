package com.androguard.demohunt;

import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.Writer;
import java.net.HttpURLConnection;
import java.net.URL;

/**
 * Invoke-based hardcoded_secrets_review: write a token into FileOutputStream
 * and HttpURLConnection body (not just a string constant).
 */
public final class HardcodedSecret {
    public static final String API_KEY = "sk_live_demohunt_not_real";
    public static final String AWS_ACCESS_KEY = "AKIADEMOHUNTNOTREAL0";

    private HardcodedSecret() {}

    public static String token() {
        String key = API_KEY;
        String aws = AWS_ACCESS_KEY;
        try {
            FileOutputStream fos = new FileOutputStream("/sdcard/demo_token.txt");
            fos.write(key.getBytes());
            fos.close();
            URL url = new URL("https://api.demohunt.androguard.com/v1/token");
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(key + aws);
            w.close();
        } catch (Throwable ignored) {
        }
        return key + aws;
    }
}
