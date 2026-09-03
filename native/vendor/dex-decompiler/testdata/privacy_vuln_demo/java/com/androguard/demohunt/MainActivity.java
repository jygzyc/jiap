package com.androguard.demohunt;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.webkit.WebView;

/** Launcher. Calls every leak / vuln method so the APK is a real runnable demo. */
public class MainActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        try {
            PrivacyLeaks.leakDeviceIdToLog(this);
            PrivacyLeaks.leakDeviceIdToFirstParty(this);
            PrivacyLeaks.leakDeviceIdThroughCipher(this);
            PrivacyLeaks.leakDeviceIdToWorldReadablePrefs(this);
        } catch (Throwable ignored) {
        }
        try {
            PrivacyLeaks.leakLocationToFirebase(this);
        } catch (Throwable ignored) {
        }
        try {
            PrivacyLeaks.leakClipboardToLog(this);
        } catch (Throwable ignored) {
        }
        try {
            PrivacySources.leakPhoneToNetwork(this);
            PrivacySources.leakContactsToNetwork(this);
            PrivacySources.leakSmsToLog(this);
            PrivacySources.leakAccountsToNetwork(this);
            PrivacySources.leakAdvertisingIdToFirebase(this);
            PrivacySources.leakMediaToNetwork(this);
            PrivacySources.leakCalendarToLog(this);
            PrivacySources.leakEmailToNetwork(this);
        } catch (Throwable ignored) {
        }
        try {
            PrivacyLayers.leakDeviceIdViaHelper(this);
            PrivacyLayers.leakDeviceIdViaLocalHelper(this);
            PrivacyLayers.leakDeviceIdViaBaseConcat(this);
            PrivacyLayers.leakDeviceIdViaFieldHop(this);
            PrivacyLayers.leakDeviceIdViaFieldUrl(this);
            PrivacyLayers.leakDeviceIdCipherThenHelper(this);
            PrivacyLayers.leakLocationViaExtraHop(this);
            PrivacyLayers.leakDeviceIdViaBuilder(this);
            PrivacyLayers.leakDeviceIdViaSdkWrapper(this);
        } catch (Throwable ignored) {
        }
        try {
            PrivacyDests.leakDeviceIdToClipboard(this);
            PrivacyDests.leakDeviceIdToSharedPrefs(this);
            PrivacyDests.leakDeviceIdToCookie(this);
            PrivacyDests.leakDeviceIdViaIntent(this);
            PrivacyDests.leakLocationViaBroadcast(this);
            PrivacyDests.leakDeviceIdToAppsFlyer(this);
            PrivacyDests.leakDeviceIdToCrashlytics(this);
            PrivacyDests.leakEditTextToNetwork(this);
        } catch (Throwable ignored) {
        }
        VulnFlows.execFromExtra(this);
        VulnFlows.sqlFromExtra(this);
        VulnFlows.webviewFromExtra(this);
        VulnFlows.launchFromExtra(this);
        VulnFlows.networkFromExtra(this);
        VulnFlows.fileWriteFromExtra(this);
        try {
            VulnFlows.nestedIntentRedirect(this);
            VulnFlows.nestedIntentGrantSmuggle(this);
            VulnFlows.reflectionFromExtra(this);
            VulnFlows.packageContextFromExtra(this);
            VulnFlows.pathTraversalFromExtra(this);
            VulnFlows.grantUriFromExtra(this);
            VulnFlows.deserializeFromExtra(this);
            VulnFlows.customTabsFromExtra(this);
            VulnFlows.webviewJsBridgeFromExtra(this);
            VulnFlows.webviewCookieFromExtra(this);
            VulnFlows.webviewWeakHostFromExtra(this);
            VulnFlows.credentialBroadcast(this);
            VulnFlows.sensitiveIdBroadcast(this);
            VulnFlows.stickyOrderedBroadcast(this);
            VulnFlows.pickFileFromExtra(this);
        } catch (Throwable ignored) {
        }
        try {
            ExtraVulns.zipSlipExtract(this);
            ExtraVulns.dumpLogcatToSdcard();
            ExtraVulns.pinningBypassReflect();
            ExtraVulns.sqlcipherHardcoded(this);
            ExtraVulns.biometricWithoutCrypto(this);
            ExtraVulns.keystoreNoUserAuth();
            ExtraVulns.installFileInterceptOn(this);
            PendingIntentLeak.mutableEmpty(this);
        } catch (Throwable ignored) {
        }
        TrustAll.install();
        WeakCrypto.useDes(new byte[] { 1, 2, 3, 4, 5, 6, 7, 8 });
        WebView webView = new WebView(this);
        InsecureWebView.enable(webView);
        HardcodedSecret.token();
    }

    /** Forwards the result Intent (uri_permission_result_forward). */
    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        setResult(resultCode, data);
    }
}
