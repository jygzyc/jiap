# WebView - File Access - Leakage

Unsafe WebView file or content access settings let web content reach local files, provider data, or picker-returned URIs. If attacker-controlled content can load in that WebView, the browser surface crosses into local app data.

**Risk: HIGH**

## Exploit Prerequisites

The WebView enables unsafe local-file behavior such as `setAllowFileAccessFromFileURLs(true)` or otherwise lets attacker-controlled content reach local files, `content://` URIs, or unvalidated file chooser results.

**Android Version Scope:** Relevant across Android versions when the unsafe settings or trust-boundary mistakes are present.

## Bypass Conditions / Uncertainties

- Reject the finding if attacker-controlled content cannot reach the WebView
- `setAllowFileAccess(true)` alone is not enough; the path to attacker-controlled navigation or JS must exist
- File chooser issues require an untrusted result URI to be accepted without validation
- `content://` impact requires the loaded provider path to expose meaningful data or side effects

## Visible Impact

Visible impact must be concrete, such as:

- reading shared preferences, databases, or tokens
- loading private provider-backed content into a trusted renderer
- accepting a malicious file chooser result that exposes protected data

## Attack Flow

```text
1. decx code search-class "WebView" -P <port>
2. decx code class-source "<WebViewHost>" -P <port>
3. Inspect WebSettings:
   -> setAllowFileAccess
   -> setAllowFileAccessFromFileURLs
   -> setAllowUniversalAccessFromFileURLs
   -> setAllowContentAccess
4. Confirm attacker-controlled content or file chooser results can reach the WebView
5. Confirm the resulting local or provider access is meaningful
```

## Key Code Patterns

- unsafe file-access settings
- unvalidated file chooser callback results

```java
WebSettings settings = webView.getSettings();
settings.setJavaScriptEnabled(true);
settings.setAllowFileAccess(true);
settings.setAllowFileAccessFromFileURLs(true);
```

## Secure Pattern

```java
settings.setAllowFileAccess(false);
settings.setAllowFileAccessFromFileURLs(false);
settings.setAllowUniversalAccessFromFileURLs(false);
settings.setAllowContentAccess(false);
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + URL validation bypass | attacker navigation reaches a local file target | [[app-webview-url-bypass]] |
| + JavaScript bridge | local data is exfiltrated through native methods | [[app-webview-js-bridge]] |
| + provider traversal | content loading reaches an unsafe file provider path | [[app-provider-path-traversal]] |
| + activity redirect | trusted host is navigated into a local file flow | [[app-activity-intent-redirect]] |

## Related

[[app-webview]]
[[app-webview-url-bypass]]
[[app-webview-js-bridge]]
[[app-provider-path-traversal]]
[[app-activity-intent-redirect]]
