package com.google.android.gms.ads.identifier;

import android.content.Context;

/** In-tree stub: AdvertisingIdClient.getAdvertisingIdInfo (no Play AAR). */
public class AdvertisingIdClient {
    public static Info getAdvertisingIdInfo(Context context) {
        return new Info();
    }

    public static final class Info {
        public String getId() {
            return "00000000-0000-0000-0000-000000000000";
        }

        public boolean isLimitAdTrackingEnabled() {
            return false;
        }
    }
}
