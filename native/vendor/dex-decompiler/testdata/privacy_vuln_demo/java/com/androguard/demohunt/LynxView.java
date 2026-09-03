package com.androguard.demohunt;

import android.webkit.WebView;

/** Thin wrapper around WebView (TikTok addJavascriptInterfaceOut / LynxView shape). */
public final class LynxView {
    private final WebView webView;

    public LynxView(WebView webView) {
        this.webView = webView;
    }

    public void addJavascriptInterfaceOut(Object o, String name) {
        webView.addJavascriptInterface(o, name);
    }

    public void loadUrl(String url) {
        webView.loadUrl(url);
    }
}
