# AndroHunt Privacy + Vuln Demo

Small compile-able Android app (`com.androguard.demohunt`) with **intra-procedural**
taint flows and detector-shaped methods. `privacy_vuln_demo.apk` and `classes.dex`
are checked in so `dex-decompiler` tests do not need the Android SDK.

Most taint / VF flows keep the source invoke and the sink invoke in the **same
method**. `PrivacyLayers` adds 1-hop helpers (`HttpSink`, `collectDeviceId`,
`forwardLocation`, `AnalyticsWrapper`) so dest recovery and summaries have a
cross-method case. `MainActivity.onCreate` calls every method inside try/catch so
the APK still runs on a device without the listed permissions.

## Privacy methods

| Method | Source → sink | Notes |
|---|---|---|
| `PrivacyLeaks.leakDeviceIdToLog` | `getDeviceId` → `Log.d` | rule 10 DeviceId→Logging |
| `PrivacyLeaks.leakDeviceIdToFirstParty` | `getDeviceId` → `openConnection` | host `https://api.demohunt.androguard.com/v1/id` |
| `PrivacyLeaks.leakLocationToFirebase` | `getLatitude` → `openConnection` + stub `FirebaseAnalytics.logEvent` | host `https://app-analytics.firebase.google.com/log` |
| `PrivacyLeaks.leakClipboardToLog` | `ClipboardManager.getText` → `Log.i` | rule 10 |
| `PrivacyLeaks.leakDeviceIdThroughCipher` | `getDeviceId` → `getBytes` → `Cipher.doFinal` → `Writer.write` | needs **getBytes + Cipher TITO** |
| `PrivacyLeaks.leakDeviceIdToWorldReadablePrefs` | `getDeviceId` → `openFileOutput(..., MODE_WORLD_READABLE)` | VF / `world_readable_storage` |
| `PrivacySources.leakPhoneToNetwork` | `getLine1Number` → `Writer.write` | PhoneNumber, extra rule 19, host `/v1/phone` |
| `PrivacySources.leakContactsToNetwork` | `ContactsContract.Contacts.getLookupUri` → network | Contacts, extra rule 19 |
| `PrivacySources.leakSmsToLog` | `SmsManager.getDefault` → `Log.d` | Sms, extra rule 20 |
| `PrivacySources.leakAccountsToNetwork` | `AccountManager.getAccounts` → network | Account, extra rule 19 |
| `PrivacySources.leakAdvertisingIdToFirebase` | stub `AdvertisingIdClient.getAdvertisingIdInfo` → network | AdvertisingId, extra rule 19 |
| `PrivacySources.leakMediaToNetwork` | `MediaStore.Images.Media.getContentUri` → network | Media, extra rule 19 |
| `PrivacySources.leakCalendarToLog` | `readCalendarContract` → `Log.i` | Calendar, extra rule 20 |
| `PrivacySources.leakEmailToNetwork` | `getEmail` + `Profile.CONTENT_URI` → network | Email, extra rule 19 |
| `PrivacyDests.leakDeviceIdToClipboard` | `getDeviceId` → `ClipboardManager.setText` | ClipboardWrite, extra rule 21 |
| `PrivacyDests.leakDeviceIdToSharedPrefs` | `getDeviceId` → `Editor.putString` | SharedPrefsWrite, extra rule 21 |
| `PrivacyDests.leakDeviceIdToCookie` | `getDeviceId` → `CookieManager.setCookie` | CookieWrite, extra rule 21 |
| `PrivacyDests.leakDeviceIdViaIntent` | `getDeviceId` → fluent `putExtra` → `startActivity` | LaunchingComponent, extra rule 21 |
| `PrivacyDests.leakLocationViaBroadcast` | `getLatitude` → fluent `putExtra` → `sendBroadcast` | LaunchingComponent, extra rule 21 |
| `PrivacyDests.leakDeviceIdToAppsFlyer` | `getDeviceId` → stub `AppsFlyerLib.logEvent` + `https://t.appsflyer.com` | default rule 10 |
| `PrivacyDests.leakDeviceIdToCrashlytics` | `getDeviceId` → stub `FirebaseCrashlytics.log` + `Log.e` | default rule 10 |
| `PrivacyDests.leakEditTextToNetwork` | `EditText.getText` → network | UserInput, rule 11 |
| `PrivacyLayers.leakDeviceIdViaHelper` | `collectDeviceId` → `HttpSink.post` / `sendHop` | 2-hop; dest `/v1/hop` |
| `PrivacyLayers.leakDeviceIdViaLocalHelper` | `getDeviceId` → same-class `sendHop` | 1-hop the solver handles; dest `/v1/localhop` |
| `PrivacyLayers.leakDeviceIdViaFieldUrl` | `getDeviceId` → `Writer.write` + `new URL(API)` | field URL `/v1/field` |
| `PrivacyLayers.leakDeviceIdCipherThenHelper` | `getDeviceId` → `getBytes` → `Cipher.doFinal` → `HttpSink.post` | needs getBytes TITO |
| `PrivacyLayers.leakLocationViaExtraHop` | `getLatitude` → `putExtra("lat")` → `forwardLocation` + `Log` | field-sensitive extra |
| `PrivacyLayers.leakDeviceIdViaBuilder` | `getDeviceId` → `StringBuilder` → `HttpSink.post` | dest `/v1/builder` |
| `PrivacyLayers.leakDeviceIdViaSdkWrapper` | `getDeviceId` → `AnalyticsWrapper.log` (stub Firebase + firebase host) | dest firebase host |

Extra kinds / dest sinks are locked by `extra_pii_kinds_and_dest_sinks` (merged
catalog sources + ClipboardWrite / SharedPrefsWrite / CookieWrite). Default-config
tests skip `needs_extra_pii` and `needs_cipher_tito`.

## Vuln methods

| Method | Detector category |
|---|---|
| `VulnFlows.execFromExtra` | `rce_process_exec` / `rce_dynamic_loading` (DexClassLoader) |
| `VulnFlows.sqlFromExtra` | `sql_injection` |
| `VulnFlows.webviewFromExtra` | `webview_unsafe_url` |
| `VulnFlows.launchFromExtra` | `intent_spoofing` / `ipc_intent_validation` / `implicit_intent_launch` |
| `VulnFlows.networkFromExtra` | rule 11 |
| `VulnFlows.fileWriteFromExtra` | rule 12 |
| `VulnFlows.nestedIntentRedirect` | `intent_redirect_nested` |
| `VulnFlows.nestedIntentGrantSmuggle` | `intent_redirect_grant_smuggle` |
| `VulnFlows.reflectionFromExtra` | `reflection_rce` |
| `VulnFlows.packageContextFromExtra` | `rce_package_context` |
| `VulnFlows.pathTraversalFromExtra` | `path_traversal` |
| `VulnFlows.grantUriFromExtra` | `uri_permission_grant_flow` |
| `VulnFlows.deserializeFromExtra` | `unsafe_deserialization` |
| `VulnFlows.customTabsFromExtra` | `custom_tabs_intent_url` |
| `VulnFlows.webviewJsBridgeFromExtra` | `webview_js_bridge_user_url` |
| `VulnFlows.webviewCookieFromExtra` | `webview_cookie_exfil` |
| `VulnFlows.webviewWeakHostFromExtra` | `webview_weak_host_check` |
| `VulnFlows.credentialBroadcast` | `credential_broadcast` / `implicit_intent_sensitive` |
| `VulnFlows.sensitiveIdBroadcast` | `sensitive_broadcast` |
| `VulnFlows.stickyOrderedBroadcast` | `sticky_ordered_broadcast` |
| `VulnFlows.pickFileFromExtra` | `pick_file_theft` |
| `ExtraVulns.zipSlipExtract` | `zip_slip` |
| `ExtraVulns.dumpLogcatToSdcard` | `logcat_external_storage` |
| `ExtraVulns.pinningBypassReflect` | `pinning_bypass` |
| `ExtraVulns.sqlcipherHardcoded` | `sqlcipher_hardcoded_passphrase` |
| `ExtraVulns.biometricWithoutCrypto` | `biometric_without_crypto` |
| `ExtraVulns.keystoreNoUserAuth` | `keystore_no_user_auth` |
| `ExtraVulns.FileInterceptClient.shouldInterceptRequest` | `webview_resource_response_file` |
| `TrustAll.install` | `ssl_trust_all` |
| `WeakCrypto.useDes` | `weak_crypto` |
| `InsecureWebView.enable` | `webview_javascript_interface` / `webview_file_access` |
| `HardcodedSecret.token` | `hardcoded_secrets_review` (FileOutputStream + body write) |
| `CmdReceiver.onReceive` | `command_receiver` |
| `RedirectReceiver.onReceive` | `broadcast_intent_redirect` / `exported_receiver_intent_redirect` |
| `DeeplinkActivity.onCreate` | `deeplink_webview_js_bridge` |
| `DemoProvider.query` | `provider_sql_injection` |
| `DemoProvider.openFile` | `provider_path_traversal` |
| `MainActivity.onActivityResult` | `uri_permission_result_forward` |
| `ResultPassthroughActivity.onCreate` | `uri_permission_setresult_passthrough` |
| `PendingIntentLeak.mutableEmpty` | PendingIntent scan (not in `run_all_detectors`) |

`TaskAffinityActivity` is StrandHogg bait (`taskAffinity=com.evil.phish`,
`allowTaskReparenting=true`). Manifest also exports the deeplink activity,
`DemoProvider` (`grantUriPermissions=true`), and both receivers. Permissions
include `READ_SMS`, `GET_ACCOUNTS`, `READ_EXTERNAL_STORAGE`, `READ_CALENDAR`.

In-tree stubs live under `stubs/` (Firebase Analytics / Crashlytics,
AdvertisingIdClient, AppsFlyer, CustomTabsIntent, BiometricPrompt, SQLCipher).
No Play / OkHttp AARs. FileProvider / network-security-config XML omitted
(flags live on the manifest: `usesCleartextTraffic`).

See `expected.json` for the machine-readable expected issue list.

## Rebuild

Needs the Android SDK (`d8`, `aapt2`, `zipalign`, `apksigner`) and a JDK.

```bash
bash testdata/privacy_vuln_demo/build.sh
```

SDK is `$ANDROID_HOME` or `$ANDROID_SDK_ROOT` or `/Users/toto/Library/Android/sdk`.
`javac` comes from `$JAVA_HOME` or Android Studio's JBR. `debug.keystore` is
created once next to this README (store/key pass `android`). Intermediate files
land in `build/` and can be deleted.

## Scan

```bash
# dex-decompiler regression (no SDK required):
cargo test --test decompiler_tests privacy_vuln_demo

# optional AndroHunt (user's other tree):
# androhunt run --apk testdata/privacy_vuln_demo/privacy_vuln_demo.apk --out /tmp/demo
```
