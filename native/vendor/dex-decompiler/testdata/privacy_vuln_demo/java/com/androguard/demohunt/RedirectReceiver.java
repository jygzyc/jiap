package com.androguard.demohunt;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/**
 * Exported receiver: getParcelableExtra → startActivity
 * (broadcast_intent_redirect + exported_receiver_intent_redirect).
 */
public class RedirectReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context context, Intent intent) {
        try {
            Intent nested = intent.getParcelableExtra("next");
            context.startActivity(nested);
        } catch (Throwable ignored) {
        }
    }
}
