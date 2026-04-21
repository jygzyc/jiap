# WebView - Scan Result - Injection

Apps often feed QR, barcode, or external-link scan results directly into a WebView. If that result is treated as trusted, a physical or local trigger can become arbitrary navigation, bridge invocation, or credential theft.

**Risk: MEDIUM**

## Exploit Prerequisites

The app receives a scan result from a scanner SDK, camera flow, browser handoff, or activity-result callback and uses that value to build a WebView navigation or JavaScript execution path without strict validation.

**Android Version Scope:** Relevant on modern Android because scanner integrations increasingly rely on `ActivityResultLauncher`, third-party SDKs, and app-link handoffs rather than custom camera code.

## Bypass Conditions / Uncertainties

- If the app validates the full URL against an exact HTTPS domain allowlist before calling `loadUrl()`, reject the finding
- Scheme-only checks are bypassable if attacker-controlled hosts, paths, query strings, or `intent://` payloads still reach the WebView
- If the scan result is first opened in a browser or another app and only later returned, confirm the return path is still attacker-controlled
- If the scanner result comes from a trusted in-app parser that strips dangerous schemes and enforces host/path rules, reject the finding

## Visible Impact

Visible impact must be concrete, for example:

- attacker-controlled page opens inside the trusted WebView and steals cookies or tokens
- attacker-controlled `intent://` or custom scheme triggers native navigation
- attacker-controlled HTML reaches a JavaScript bridge with sensitive methods

If the result only opens a harmless external browser page with no trusted-app privileges, do not rate it as a WebView bug.

## Attack Flow

```text
1. decx code search-global "WebView" --max-results 50 -P <port>
2. decx code search-method "onActivityResult" -P <port>
3. Inspect ActivityResultLauncher callbacks and scanner SDK handlers
4. Trace scan-result strings into:
   -> WebView.loadUrl
   -> loadDataWithBaseURL
   -> evaluateJavascript
5. Confirm whether exact host/path/scheme validation exists before navigation
```

## Key Code Patterns

- `result.getStringExtra(...)` flows directly into `loadUrl()`
- scanner callback concatenates a base URL with attacker-controlled path or query
- result handler accepts `javascript:`, `data:`, `file:`, `intent:`, or untrusted HTTPS domains

```java
@Override
protected void onActivityResult(int requestCode, int resultCode, Intent data) {
    super.onActivityResult(requestCode, resultCode, data);
    if (requestCode == REQ_SCAN && resultCode == RESULT_OK) {
        String scanned = data.getStringExtra("SCAN_RESULT");
        webView.loadUrl(scanned); // vulnerable if scanned is attacker-controlled
    }
}
```

## Secure Pattern

```java
private static final Set<String> ALLOWED_HOSTS = Set.of("pay.example.com");

private boolean isAllowedScanUrl(Uri uri) {
    return uri != null
        && "https".equals(uri.getScheme())
        && ALLOWED_HOSTS.contains(uri.getHost());
}

private void handleScanResult(String scanned) {
    Uri uri = Uri.parse(scanned);
    if (!isAllowedScanUrl(uri)) {
        return;
    }
    webView.loadUrl(uri.toString());
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + JavaScript bridge | scan result drives attacker content into sensitive bridge methods | [[app-webview-js-bridge]] |
| + cookie theft | trusted WebView session cookies leak to attacker-controlled content | [[app-webview-cookie-theft]] |
| + URL validation bypass | weak allowlist turns scan payload into arbitrary navigation | [[app-webview-url-bypass]] |
| + `intent://` injection | scan payload escapes the WebView and triggers native components | [[app-webview-intent-scheme]] |

## Related

[[app-webview]]
[[app-webview-js-bridge]]
[[app-webview-cookie-theft]]
[[app-webview-url-bypass]]
[[app-webview-intent-scheme]]
