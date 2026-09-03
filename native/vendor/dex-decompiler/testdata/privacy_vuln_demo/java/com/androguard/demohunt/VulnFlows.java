package com.androguard.demohunt;

import android.app.Activity;
import android.content.Intent;
import android.database.sqlite.SQLiteDatabase;
import android.net.Uri;
import android.webkit.WebView;

import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.Writer;
import java.net.HttpURLConnection;
import java.net.URL;

import dalvik.system.DexClassLoader;

/**
 * Intra-procedural vuln flows. Each method reads getIntent().getStringExtra
 * and reaches the sink in the same method.
 */
public final class VulnFlows {
    private VulnFlows() {}

    /**
     * ActivityUserInput → Runtime.exec (detector) + DexClassLoader.&lt;init&gt; (taint).
     * Runtime.exec is modeled as argument 0 ({@code this}); DexClassLoader.&lt;init&gt;
     * is argument 1 (the path), which is the extra.
     */
    public static void execFromExtra(Activity activity) {
        try {
            String cmd = activity.getIntent().getStringExtra("cmd");
            Runtime.getRuntime().exec(cmd);
            DexClassLoader loader = new DexClassLoader(
                    cmd, activity.getCacheDir().getAbsolutePath(), null, activity.getClassLoader());
            if (loader == null) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }

    /** ActivityUserInput → SQLiteDatabase.rawQuery (rule 4). */
    public static void sqlFromExtra(Activity activity) {
        try {
            String sql = activity.getIntent().getStringExtra("cmd");
            SQLiteDatabase db = activity.openOrCreateDatabase("demohunt.db", 0, null);
            db.rawQuery(sql, null);
        } catch (Throwable ignored) {
        }
    }

    /** ActivityUserInput → WebView.loadUrl (rule 5 + detector webview_unsafe_url). */
    public static void webviewFromExtra(Activity activity) {
        try {
            String url = activity.getIntent().getStringExtra("url");
            WebView webView = new WebView(activity);
            webView.loadUrl(url);
        } catch (Throwable ignored) {
        }
    }

    /**
     * ActivityUserInput → startActivity (rule 3).
     * startActivity sink port is argument 1 (the Intent). getIntent() already
     * taints that object; setData also builds a VIEW intent from the extra.
     */
    public static void launchFromExtra(Activity activity) {
        try {
            Intent incoming = activity.getIntent();
            String extra = incoming.getStringExtra("url");
            Uri uri = Uri.parse(extra);
            incoming.setData(uri);
            activity.startActivity(incoming);
            Intent view = new Intent(Intent.ACTION_VIEW);
            view.setData(uri);
            activity.startActivity(view);
        } catch (Throwable ignored) {
        }
    }

    /** ActivityUserInput → Writer.write + openConnection (rule 11). */
    public static void networkFromExtra(Activity activity) {
        try {
            String extra = activity.getIntent().getStringExtra("url");
            URL url = new URL(extra);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            Writer w = new OutputStreamWriter(conn.getOutputStream());
            w.write(extra);
            w.close();
        } catch (Throwable ignored) {
        }
    }

    /** ActivityUserInput → FileOutputStream (rule 12). */
    public static void fileWriteFromExtra(Activity activity) {
        try {
            String path = activity.getIntent().getStringExtra("cmd");
            FileOutputStream fos = new FileOutputStream(path);
            fos.write(1);
            fos.close();
        } catch (Throwable ignored) {
        }
    }

    /** Nested Intent extra → startActivity (intent_redirect_nested). */
    public static void nestedIntentRedirect(Activity activity) {
        try {
            Intent nested = activity.getIntent().getParcelableExtra("next");
            activity.startActivity(nested);
        } catch (Throwable ignored) {
        }
    }

    /**
     * Nested Intent + FLAG_GRANT_READ_URI_PERMISSION
     * (intent_redirect_nested + intent_redirect_grant_smuggle).
     */
    public static void nestedIntentGrantSmuggle(Activity activity) {
        try {
            Intent nested = activity.getIntent().getParcelableExtra("next");
            String flagName = "FLAG_GRANT_READ_URI_PERMISSION";
            nested.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            activity.startActivity(nested);
            if (flagName.isEmpty()) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }

    /** extra class name → Class.forName + getMethod + invoke (reflection_rce). */
    public static void reflectionFromExtra(Activity activity) {
        try {
            String cls = activity.getIntent().getStringExtra("class");
            Class<?> c = Class.forName(cls);
            java.lang.reflect.Method m = c.getMethod("run");
            m.invoke(null);
        } catch (Throwable ignored) {
        }
    }

    /** extra package → createPackageContext + loadClass, no checkSignatures. */
    public static void packageContextFromExtra(Activity activity) {
        try {
            String pkg = activity.getIntent().getStringExtra("pkg");
            android.content.Context other = activity.createPackageContext(
                    pkg, android.content.Context.CONTEXT_INCLUDE_CODE);
            other.getClassLoader().loadClass("x.Plugin");
        } catch (Throwable ignored) {
        }
    }

    /** extra path → File("/sdcard", extra) + FileOutputStream (path_traversal). */
    public static void pathTraversalFromExtra(Activity activity) {
        try {
            String extra = activity.getIntent().getStringExtra("path");
            java.io.File f = new java.io.File("/sdcard", extra);
            java.io.FileOutputStream fos = new java.io.FileOutputStream(f);
            fos.write(1);
            fos.close();
        } catch (Throwable ignored) {
        }
    }

    /** extra uri → grantUriPermission + setResult FLAG_GRANT (uri_permission_grant_flow). */
    public static void grantUriFromExtra(Activity activity) {
        try {
            String extra = activity.getIntent().getStringExtra("uri");
            android.net.Uri uri = android.net.Uri.parse(extra);
            activity.grantUriPermission(
                    "com.evil", uri, Intent.FLAG_GRANT_READ_URI_PERMISSION);
            Intent result = new Intent();
            result.setData(uri);
            result.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            activity.setResult(Activity.RESULT_OK, result);
        } catch (Throwable ignored) {
        }
    }

    /** extra → ObjectInputStream.readObject (unsafe_deserialization). */
    public static void deserializeFromExtra(Activity activity) {
        try {
            java.io.Serializable ser = activity.getIntent().getSerializableExtra("obj");
            String path = activity.getIntent().getStringExtra("path");
            java.io.ObjectInputStream ois = new java.io.ObjectInputStream(
                    new java.io.FileInputStream(path));
            Object obj = ois.readObject();
            ois.close();
            if (ser == obj) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }

    /** extra → CustomTabsIntent.launchUrl (custom_tabs_intent_url). */
    public static void customTabsFromExtra(Activity activity) {
        try {
            String extra = activity.getIntent().getStringExtra("url");
            androidx.browser.customtabs.CustomTabsIntent tabs =
                    new androidx.browser.customtabs.CustomTabsIntent.Builder().build();
            tabs.launchUrl(activity, android.net.Uri.parse(extra));
        } catch (Throwable ignored) {
        }
    }

    /** extra url → loadUrl + addJavascriptInterface (webview_js_bridge_user_url). */
    public static void webviewJsBridgeFromExtra(Activity activity) {
        try {
            String url = activity.getIntent().getStringExtra("url");
            android.webkit.WebView webView = new android.webkit.WebView(activity);
            webView.addJavascriptInterface(new InsecureWebView.JsBridge(), "Android");
            webView.loadUrl(url);
        } catch (Throwable ignored) {
        }
    }

    /** extra url → loadUrl + CookieManager (webview_cookie_exfil). */
    public static void webviewCookieFromExtra(Activity activity) {
        try {
            String url = activity.getIntent().getStringExtra("url");
            android.webkit.WebView webView = new android.webkit.WebView(activity);
            webView.loadUrl(url);
            android.webkit.CookieManager cm = android.webkit.CookieManager.getInstance();
            String cookie = cm.getCookie(url);
            cm.setCookie(url, cookie);
        } catch (Throwable ignored) {
        }
    }

    /**
     * extra url + contains("androguard.com") + getHost + loadUrl
     * (webview_weak_host_check).
     */
    public static void webviewWeakHostFromExtra(Activity activity) {
        try {
            String url = activity.getIntent().getStringExtra("url");
            String host = android.net.Uri.parse(url).getHost();
            if (url.contains("androguard.com")) {
                android.webkit.WebView webView = new android.webkit.WebView(activity);
                webView.loadUrl(url);
            }
            if (host == null) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }

    /** putExtra password/token → sendBroadcast (credential_broadcast + implicit_intent_sensitive). */
    public static void credentialBroadcast(Activity activity) {
        try {
            Intent intent = new Intent("com.androguard.demohunt.CREDENTIAL");
            intent.putExtra("password", "demo-password");
            intent.putExtra("token", "demo-token");
            activity.sendBroadcast(intent);
        } catch (Throwable ignored) {
        }
    }

    /** getDeviceId → sendBroadcast (sensitive_broadcast). */
    public static void sensitiveIdBroadcast(Activity activity) {
        try {
            android.telephony.TelephonyManager tm =
                    (android.telephony.TelephonyManager) activity.getSystemService(
                            android.content.Context.TELEPHONY_SERVICE);
            String id = tm.getDeviceId();
            String imsi = tm.getSubscriberId();
            Intent intent = new Intent("com.androguard.demohunt.ID");
            intent.putExtra("imei", id);
            intent.putExtra("imsi", imsi);
            activity.sendBroadcast(intent);
        } catch (Throwable ignored) {
        }
    }

    /** sendOrderedBroadcast + sendStickyBroadcast (sticky_ordered_broadcast). */
    public static void stickyOrderedBroadcast(Activity activity) {
        try {
            Intent intent = new Intent("com.androguard.demohunt.STICKY");
            activity.sendOrderedBroadcast(intent, null);
            activity.sendStickyBroadcast(intent);
        } catch (Throwable ignored) {
        }
    }

    /** ACTION_PICK + openInputStream (pick_file_theft). */
    public static void pickFileFromExtra(Activity activity) {
        try {
            String hint = "ACTION_PICK";
            String extra = activity.getIntent().getStringExtra("uri");
            Intent pick = new Intent(Intent.ACTION_PICK);
            pick.setType("image/*");
            activity.startActivityForResult(pick, 1);
            android.net.Uri uri = android.net.Uri.parse(extra);
            java.io.InputStream in = activity.getContentResolver().openInputStream(uri);
            if (in != null) {
                in.close();
            }
            if (hint.isEmpty()) {
                return;
            }
        } catch (Throwable ignored) {
        }
    }
}
