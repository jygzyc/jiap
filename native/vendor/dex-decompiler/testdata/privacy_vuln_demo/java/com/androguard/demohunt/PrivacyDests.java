package com.androguard.demohunt;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.location.Location;
import android.location.LocationManager;
import android.telephony.TelephonyManager;
import android.util.Log;
import android.webkit.CookieManager;
import android.widget.EditText;

import com.appsflyer.AppsFlyerLib;
import com.google.firebase.crashlytics.FirebaseCrashlytics;

import java.io.OutputStreamWriter;
import java.io.Writer;
import java.net.HttpURLConnection;
import java.net.URL;
import java.util.HashMap;

/**
 * Extra destination classes. DeviceId / Location / UserInput → dest sink
 * in the same method. ClipboardWrite / SharedPrefsWrite / CookieWrite are
 * asserted only in the merged extra-PII test.
 */
public final class PrivacyDests {
    private PrivacyDests() {}

    /** DeviceId → ClipboardManager.setText (ClipboardWrite, dest other_app). */
    public static void leakDeviceIdToClipboard(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            ClipboardManager cm = (ClipboardManager) ctx.getSystemService(Context.CLIPBOARD_SERVICE);
            cm.setText(id);
            cm.setPrimaryClip(ClipData.newPlainText("id", id));
        } catch (Throwable ignored) {
        }
    }

    /** DeviceId → SharedPreferences.Editor.putString (SharedPrefsWrite). */
    public static void leakDeviceIdToSharedPrefs(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            ctx.getSharedPreferences("demohunt", Context.MODE_PRIVATE)
                    .edit()
                    .putString("device_id", id)
                    .apply();
        } catch (Throwable ignored) {
        }
    }

    /** DeviceId → CookieManager.setCookie (CookieWrite). */
    public static void leakDeviceIdToCookie(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            CookieManager.getInstance().setCookie(id, "v=1");
        } catch (Throwable ignored) {
        }
    }

    /** DeviceId → Intent.putExtra + startActivity (LaunchingComponent, dest other_app). */
    public static void leakDeviceIdViaIntent(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            Intent launched = new Intent(Intent.ACTION_VIEW).putExtra(id, id);
            ctx.startActivity(launched);
        } catch (Throwable ignored) {
        }
    }

    /** Location → sendBroadcast with extra (LaunchingComponent, dest other_app). */
    public static void leakLocationViaBroadcast(Context ctx) {
        try {
            LocationManager lm = (LocationManager) ctx.getSystemService(Context.LOCATION_SERVICE);
            Location loc = lm.getLastKnownLocation(LocationManager.GPS_PROVIDER);
            double lat = loc.getLatitude();
            String latStr = String.valueOf(lat);
            Intent launched = new Intent("com.androguard.demohunt.LOC").putExtra(latStr, latStr);
            ctx.sendBroadcast(launched);
        } catch (Throwable ignored) {
        }
    }

    /** DeviceId → AppsFlyerLib.logEvent + openConnection (Network, default rule 10). */
    public static void leakDeviceIdToAppsFlyer(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            AppsFlyerLib.getInstance().logEvent(ctx, "id", new HashMap<String, Object>());
            String dest = "https://t.appsflyer.com";
            URL url = new URL(dest);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(id);
            w.close();
        } catch (Throwable ignored) {
        }
    }

    /** DeviceId → FirebaseCrashlytics.log + Log.e (Logging, dest logs). */
    public static void leakDeviceIdToCrashlytics(Context ctx) {
        try {
            TelephonyManager tm = (TelephonyManager) ctx.getSystemService(Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            FirebaseCrashlytics.getInstance().log(id);
            FirebaseCrashlytics.getInstance().recordException(new RuntimeException(id));
            Log.e("DemoHunt", id);
        } catch (Throwable ignored) {
        }
    }

    /** UserInput (EditText.getText) → Network (rule 11). */
    public static void leakEditTextToNetwork(Context ctx) {
        try {
            EditText et = new EditText(ctx);
            CharSequence raw = et.getText();
            String text = String.valueOf(raw);
            String dest = "https://api.demohunt.androguard.com/v1/input";
            URL url = new URL(dest);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(text);
            w.close();
        } catch (Throwable ignored) {
        }
    }
}
