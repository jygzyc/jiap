package com.androguard.demohunt;

import android.app.AlarmManager;
import android.app.Notification;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;

/**
 * Empty mutable PendingIntent handed to Notification / AlarmManager.
 * Not in run_all_detectors — scanned via scan_pending_intents_dex_parallel.
 */
public final class PendingIntentLeak {
    private PendingIntentLeak() {}

    public static void mutableEmpty(Context ctx) {
        try {
            Intent empty = new Intent();
            int flags = PendingIntent.FLAG_MUTABLE;
            String flagName = "FLAG_MUTABLE";
            PendingIntent pi = PendingIntent.getActivity(ctx, 0, empty, flags);
            Notification.Builder b = new Notification.Builder(ctx);
            b.setContentIntent(pi);
            NotificationManager nm =
                    (NotificationManager) ctx.getSystemService(Context.NOTIFICATION_SERVICE);
            nm.notify(1, b.build());
            AlarmManager am = (AlarmManager) ctx.getSystemService(Context.ALARM_SERVICE);
            am.set(AlarmManager.RTC, 0L, pi);
            if (flagName.isEmpty()) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }
}
