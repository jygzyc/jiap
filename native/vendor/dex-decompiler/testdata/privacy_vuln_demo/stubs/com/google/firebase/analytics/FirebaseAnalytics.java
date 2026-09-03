package com.google.firebase.analytics;

import android.content.Context;
import android.os.Bundle;

/**
 * Tiny in-tree stub so dest haystack contains FirebaseAnalytics + logEvent
 * without pulling Play / Firebase AARs.
 */
public class FirebaseAnalytics {
    public static FirebaseAnalytics getInstance(Context context) {
        return new FirebaseAnalytics();
    }

    public void logEvent(String name, Bundle params) {
    }
}
