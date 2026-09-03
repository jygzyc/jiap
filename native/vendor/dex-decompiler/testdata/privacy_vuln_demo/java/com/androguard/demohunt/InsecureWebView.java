package com.androguard.demohunt;

import android.webkit.CookieManager;
import android.webkit.WebSettings;
import android.webkit.WebView;

/** JS bridge + file-access + cookies (webview_javascript_interface / webview_file_access). */
public final class InsecureWebView {
    private InsecureWebView() {}

    public static void enable(WebView webView) {
        WebSettings settings = webView.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setAllowFileAccess(true);
        settings.setAllowFileAccessFromFileURLs(true);
        webView.addJavascriptInterface(new JsBridge(), "Android");
        CookieManager.getInstance().setCookie("https://demohunt.androguard.com", "session=1");
    }

    public static final class JsBridge {
        @android.webkit.JavascriptInterface
        public String ping() {
            return "pong";
        }
    }
}
