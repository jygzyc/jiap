package com.androguard.demohunt;

import android.content.ClipboardManager;
import android.content.Context;
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
 * Intra-procedural privacy leaks. Source and sink stay in the same method so
 * default_config() + the taint solver can match {@code callable} to the method name.
 *
 * Network sinks use {@code Writer.write(String)} (modeled as argument index 1) so the
 * tainted string is the sink operand. {@code URL.openConnection()} is still invoked
 * so dest haystacks contain the host const-string and the openConnection symbol.
 */
public final class PrivacyLeaks {
    private PrivacyLeaks() {}

    /** DeviceId → Log.d (rule 10 DeviceId→Logging). */
    public static void leakDeviceIdToLog(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            Log.d("DemoHunt", id);
        } catch (Throwable ignored) {
        }
    }

    /**
     * DeviceId → Writer.write + URL.openConnection (rule 10 DeviceId→Network).
     * Host const-string lives in this method for later dest recovery.
     */
    public static void leakDeviceIdToFirstParty(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            String dest = "https://api.demohunt.androguard.com/v1/id";
            String urlStr = dest + "?id=" + id;
            URL url = new URL(urlStr);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(id);
            w.close();
        } catch (Throwable ignored) {
        }
    }

    /**
     * Location (getLatitude) → Network + FirebaseAnalytics.logEvent.
     * logEvent is not a default Network sink; Writer.write / openConnection is.
     */
    public static void leakLocationToFirebase(Context ctx) {
        try {
            LocationManager lm = (LocationManager) ctx.getSystemService(Context.LOCATION_SERVICE);
            Location loc = lm.getLastKnownLocation(LocationManager.GPS_PROVIDER);
            double lat = loc.getLatitude();
            String dest = "https://app-analytics.firebase.google.com/log";
            String latStr = String.valueOf(lat);
            String urlStr = dest + "?lat=" + latStr;
            URL url = new URL(urlStr);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(latStr);
            w.close();
            FirebaseAnalytics analytics = FirebaseAnalytics.getInstance(ctx);
            Bundle bundle = new Bundle();
            bundle.putDouble("lat", lat);
            analytics.logEvent("loc", bundle);
        } catch (Throwable ignored) {
        }
    }

    /** ClipboardManager.getText() → Log.i (rule 10 Clipboard→Logging). */
    public static void leakClipboardToLog(Context ctx) {
        try {
            ClipboardManager cm = (ClipboardManager) ctx.getSystemService(Context.CLIPBOARD_SERVICE);
            CharSequence clip = cm.getText();
            String text = String.valueOf(clip);
            Log.i("DemoHunt", text);
        } catch (Throwable ignored) {
        }
    }

    /**
     * DeviceId → getBytes → Cipher.doFinal → OutputStream.write.
     * The original id is never written; only the ciphertext is.
     * default_config() models Cipher.doFinal but not String.getBytes, so this
     * flow needs a getBytes TITO merge (see expected.json needs_cipher_tito).
     */
    public static void leakDeviceIdThroughCipher(Context ctx) {
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
            String dest = "https://api.demohunt.androguard.com/v1/enc";
            URL url = new URL(dest);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(leaked);
            w.close();
        } catch (Throwable ignored) {
        }
    }

    /**
     * DeviceId → world-readable file / SharedPreferences.
     * {@code Context.MODE_WORLD_READABLE} is a compile-time int (d8 inlines it to 1),
     * so keep the field name as a const-string for VF / world_readable_storage.
     */
    public static void leakDeviceIdToWorldReadablePrefs(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            String modeName = "MODE_WORLD_READABLE";
            int mode = Context.MODE_WORLD_READABLE;
            java.io.FileOutputStream fos = ctx.openFileOutput("device_id.txt", mode);
            fos.write(id.getBytes());
            fos.close();
            ctx.getSharedPreferences("demohunt", mode)
                    .edit()
                    .putString("device_id", id)
                    .apply();
            if (modeName.isEmpty()) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }
}
