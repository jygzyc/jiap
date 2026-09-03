package com.androguard.demohunt;

import java.security.cert.X509Certificate;

import javax.net.ssl.HostnameVerifier;
import javax.net.ssl.HttpsURLConnection;
import javax.net.ssl.SSLContext;
import javax.net.ssl.SSLSession;
import javax.net.ssl.TrustManager;
import javax.net.ssl.X509TrustManager;

/** Trust-all SSL. Class / strings contain TRUST_ALL + ALLOW_ALL so ssl_trust_all hints fire. */
public final class TrustAll {
    public static final String TRUST_ALL = "TRUST_ALL";
    public static final String ALLOW_ALL = "ALLOW_ALL";

    private TrustAll() {}

    public static void install() {
        X509TrustManager tm = new TrustAllManager();
        HostnameVerifier hv = new TRUST_ALL_HOSTNAME();
        try {
            SSLContext ctx = SSLContext.getInstance("TLS");
            ctx.init(null, new TrustManager[] { tm }, new java.security.SecureRandom());
            HttpsURLConnection.setDefaultSSLSocketFactory(ctx.getSocketFactory());
        } catch (Throwable ignored) {
        }
        HttpsURLConnection.setDefaultHostnameVerifier(hv);
        // Keep the hint strings live so they land in the method's const-string pool.
        String hints = TRUST_ALL + ALLOW_ALL;
        if (hints.isEmpty()) {
            return;
        }
    }

    public static final class TrustAllManager implements X509TrustManager {
        @Override
        public void checkClientTrusted(X509Certificate[] chain, String authType) {
        }

        @Override
        public void checkServerTrusted(X509Certificate[] chain, String authType) {
        }

        @Override
        public X509Certificate[] getAcceptedIssuers() {
            return new X509Certificate[0];
        }
    }

    public static final class TRUST_ALL_HOSTNAME implements HostnameVerifier {
        @Override
        public boolean verify(String hostname, SSLSession session) {
            return true;
        }
    }
}
