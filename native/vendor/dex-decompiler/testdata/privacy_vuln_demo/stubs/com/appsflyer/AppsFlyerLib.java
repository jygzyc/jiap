package com.appsflyer;

import android.content.Context;
import java.util.Map;

/** In-tree stub: AppsFlyerLib.logEvent (no AppsFlyer AAR). */
public class AppsFlyerLib {
    public static AppsFlyerLib getInstance() {
        return new AppsFlyerLib();
    }

    public void logEvent(Context context, String name, Map<String, Object> values) {
    }
}
