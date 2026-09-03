package com.androguard.demohunt;

import android.app.Activity;
import android.os.Bundle;
import android.webkit.WebView;

/**
 * Exported VIEW/BROWSABLE deeplink: getDataString → loadUrl + addJavascriptInterface
 * (deeplink_webview_js_bridge + webview_js_bridge_user_url).
 */
public class DeeplinkActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        try {
            String url = getIntent().getDataString();
            WebView webView = new WebView(this);
            webView.addJavascriptInterface(new InsecureWebView.JsBridge(), "Android");
            webView.loadUrl(url);
        } catch (Throwable ignored) {
        }
    }
}
