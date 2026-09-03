package com.androguard.demohunt;

import android.app.Activity;
import android.net.Uri;
import android.os.Bundle;

/**
 * Exported VIEW/BROWSABLE deeplink: getEncodedPath forwarded to LynxHost.
 * No WebView / JS interface in this class (split-class CVE-2024-45240 shape).
 */
public class LynxDeepLinkActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        try {
            Uri data = getIntent().getData();
            String path = data.getEncodedPath();
            LynxHost.open(this, path);
        } catch (Throwable ignored) {
        }
    }
}
