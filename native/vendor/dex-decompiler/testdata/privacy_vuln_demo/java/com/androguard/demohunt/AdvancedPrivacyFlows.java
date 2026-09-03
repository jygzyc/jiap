package com.androguard.demohunt;

import android.app.Notification;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.os.IBinder;
import android.telephony.TelephonyManager;
import android.util.Log;
import android.webkit.WebView;

/**
 * Regression shapes for heap, exceptional, callback, and privacy destination flow.
 *
 * The clean array slot is deliberately sent to the same API as the tainted slot so
 * index-insensitive propagation produces an observable false positive.
 */
public final class AdvancedPrivacyFlows extends Service {
    private static String pendingDeviceId;

    private static String deviceId(Context context) {
        TelephonyManager tm =
                (TelephonyManager) context.getSystemService(Context.TELEPHONY_SERVICE);
        return tm == null ? null : tm.getDeviceId();
    }

    public static void leakThroughException(Context context) {
        String id = deviceId(context);
        try {
            if (id != null) {
                throw new IllegalStateException("handoff");
            }
        } catch (IllegalStateException expected) {
            Log.w("privacy-exception", id);
        }
    }

    public static void leakThroughIndexedArray(Context context) {
        String[] values = new String[2];
        values[0] = deviceId(context);
        values[1] = "public-constant";
        Log.i("privacy-array-clean", values[1]);
        Log.w("privacy-array-tainted", values[0]);
    }

    public static void storeStatic(Context context) {
        pendingDeviceId = deviceId(context);
    }

    public static void flushStatic() {
        Log.w("privacy-static", pendingDeviceId);
    }

    public static void leakToNotification(Context context) {
        String id = deviceId(context);
        Notification notification = new Notification.Builder(context)
                .setContentTitle("Account")
                .setContentText(id)
                .build();
        NotificationManager manager =
                (NotificationManager) context.getSystemService(Context.NOTIFICATION_SERVICE);
        if (manager != null) {
            manager.notify(42, notification);
        }
    }

    public static void leakToWebViewJavascript(Context context, WebView webView) {
        String id = deviceId(context);
        webView.evaluateJavascript("receiveId('" + id + "')", null);
    }

    private static void secondHop(String value) {
        Log.w("privacy-multihop", value);
    }

    private static void firstHop(String value) {
        secondHop(value);
    }

    public static void leakThroughTwoHelpers(Context context) {
        firstHop(deviceId(context));
    }

    private static void postCallbackValue(String value) {
        Log.w("privacy-callback", value);
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String controlled = intent == null ? null : intent.getStringExtra("payload");
        postCallbackValue(controlled);
        return START_NOT_STICKY;
    }

    public void onMessage(String externallyControlledMessage) {
        Log.w("privacy-callback-direct", externallyControlledMessage);
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
