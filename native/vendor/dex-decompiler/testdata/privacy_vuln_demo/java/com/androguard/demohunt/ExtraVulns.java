package com.androguard.demohunt;

import android.app.Activity;
import android.content.Context;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebView;
import android.webkit.WebViewClient;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/**
 * Remaining VF detector trips. One method per remaining category that did
 * not already live on VulnFlows / PrivacyLeaks.
 */
public final class ExtraVulns {
    private ExtraVulns() {}

    /** ZipEntry.getName → File / FileOutputStream (zip_slip). */
    public static void zipSlipExtract(Context ctx) {
        try {
            File outDir = ctx.getFilesDir();
            ZipInputStream zis = new ZipInputStream(new FileInputStream(new File(outDir, "demo.zip")));
            ZipEntry entry = zis.getNextEntry();
            String name = entry.getName();
            File dest = new File(outDir, name);
            FileOutputStream fos = new FileOutputStream(dest);
            fos.write(1);
            fos.close();
            zis.close();
        } catch (Throwable ignored) {
        }
    }

    /** Runtime.exec("logcat -d >> /sdcard/demo.log") (logcat_external_storage). */
    public static void dumpLogcatToSdcard() {
        try {
            Runtime.getRuntime().exec("logcat -d >> /sdcard/demo.log");
        } catch (Throwable ignored) {
        }
    }

    /** Class.forName(CertificatePinner) + getDeclaredMethod + bypass/unpin. */
    public static void pinningBypassReflect() {
        try {
            String pin = "okhttp3.CertificatePinner";
            String hint = "bypass";
            String unpin = "unpin";
            Class<?> c = Class.forName(pin);
            java.lang.reflect.Method m = c.getDeclaredMethod(hint);
            m.setAccessible(true);
            m.invoke(null);
            if (unpin.isEmpty()) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }

    /** SQLCipher open with const passphrase (sqlcipher_hardcoded_passphrase). */
    public static void sqlcipherHardcoded(Context ctx) {
        try {
            String pass = "passphrase-secret";
            net.sqlcipher.database.SQLiteDatabase.openOrCreateDatabase(
                    "demo.db", pass, null);
            if (pass.isEmpty()) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }

    /** BiometricPrompt.authenticate without CryptoObject. */
    public static void biometricWithoutCrypto(Context ctx) {
        try {
            androidx.biometric.BiometricPrompt prompt = new androidx.biometric.BiometricPrompt();
            prompt.authenticate(new androidx.biometric.BiometricPrompt.PromptInfo());
        } catch (Throwable ignored) {
        }
    }

    /** KeyGenParameterSpec.Builder without setUserAuthenticationRequired. */
    public static void keystoreNoUserAuth() {
        try {
            KeyGenParameterSpec spec = new KeyGenParameterSpec.Builder(
                    "demo", KeyProperties.PURPOSE_ENCRYPT).build();
            if (spec == null) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }

    /** WebViewClient.shouldInterceptRequest serves FileInputStream (webview_resource_response_file). */
    public static void installFileIntercept(WebView webView) {
        webView.setWebViewClient(new FileInterceptClient());
    }

    public static final class FileInterceptClient extends WebViewClient {
        @Override
        public WebResourceResponse shouldInterceptRequest(WebView view, WebResourceRequest request) {
            try {
                String name = request.getUrl().getLastPathSegment();
                FileInputStream fis = new FileInputStream(new File(name));
                return new WebResourceResponse("text/plain", "utf-8", fis);
            } catch (Throwable ignored) {
                return null;
            }
        }
    }

    public static void installFileInterceptOn(Activity activity) {
        try {
            WebView webView = new WebView(activity);
            installFileIntercept(webView);
        } catch (Throwable ignored) {
        }
    }
}
