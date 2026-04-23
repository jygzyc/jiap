# WebView - SSL - Bypass

The app overrides `onReceivedSslError()` and proceeds despite certificate failures. This lets an active network attacker inject or observe traffic that the WebView should have rejected.

**Risk: MEDIUM**

## Exploit Prerequisites

The WebView uses a custom `WebViewClient` that calls `handler.proceed()` or otherwise accepts untrusted certificates.

**Android Version Scope:** Relevant across Android versions when custom SSL-error handling is present.

## Bypass Conditions / Uncertainties

- This finding requires an active MITM position; state that explicitly
- If the app cancels on SSL errors or only proceeds after a trusted recovery path, reject the finding
- If the loaded content has no path to a meaningful security outcome, keep the impact bounded

## Visible Impact

Visible impact must be concrete, such as:

- stealing credentials in transit
- injecting attacker-controlled web content
- reaching bridge or local-file chains through injected content

## Attack Flow

```text
1. decx code search-global "onReceivedSslError" --limit 50 -P <port>
2. decx code class-source "<WebViewClientImpl>" -P <port>
3. Inspect onReceivedSslError()
4. Confirm whether handler.proceed() is called unconditionally
5. Confirm the affected WebView handles meaningful authenticated content
```

## Key Code Patterns

```java
@Override
public void onReceivedSslError(WebView view, SslErrorHandler handler, SslError error) {
    handler.proceed();
}
```

## Secure Pattern

```java
handler.cancel();
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + JavaScript bridge | injected content calls native methods | [[app-webview-js-bridge]] |
| + file-access leakage | injected page reads local resources | [[app-webview-file-access]] |
| + URL validation bypass | MITM-delivered page drives a broader navigation chain | [[app-webview-url-bypass]] |

## Related

[[app-webview]]
[[app-webview-js-bridge]]
[[app-webview-file-access]]
[[app-webview-url-bypass]]
