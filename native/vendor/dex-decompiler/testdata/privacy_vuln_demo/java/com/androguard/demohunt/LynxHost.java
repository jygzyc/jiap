package com.androguard.demohunt;

import android.content.Context;
import android.webkit.WebView;

/** Lynx-style host: JS interface + file URL load. No Intent/getData in this class. */
public final class LynxHost {
    private LynxHost() {}

    public static void open(Context ctx, String path) {
        WebView webView = new WebView(ctx);
        LynxView lynx = new LynxView(webView);
        lynx.addJavascriptInterfaceOut(new InsecureWebView.JsBridge(), "Android");
        lynx.loadUrl("file:///android_asset" + path);
    }
}
