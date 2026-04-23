# WebView - URL Validation - Bypass

The app tries to restrict what a WebView can load, but the validation is too weak. Attacker-controlled content still reaches the trusted WebView despite the intended allowlist.

**Risk: MEDIUM**

## Exploit Prerequisites

The WebView accepts attacker-controlled URLs or HTML, and its scheme/host/path validation is missing or bypassable.

**Android Version Scope:** Relevant across Android versions.

## Bypass Conditions / Uncertainties

- If exact HTTPS host/path allowlisting blocks all attacker-controlled navigation, reject the finding
- Scheme-only or host-only checks are often bypassable; treat them as insufficient unless the remaining fields are irrelevant
- If the loaded page has no path to a security-relevant action, downgrade or reject the finding

## Visible Impact

Visible impact must be concrete, such as:

- loading attacker-controlled content into a trusted WebView
- reaching a JavaScript bridge
- navigating into file, content, or custom-scheme flows

## Attack Flow

```text
1. decx code search-global "WebView" --limit 50 -P <port>
2. decx code class-context "<WebViewHost>" -P <port>
   -> identify WebViewClient and URL handling methods
3. decx code search-class "<WebViewHost>" "loadUrl|loadData|loadDataWithBaseURL|shouldOverrideUrlLoading" --limit 20 -P <port>
   -> locate all URL loading entry points in one shot
4. decx code class-source "<WebViewHost>" -P <port>
5. Confirm whether attacker-controlled URLs can bypass validation
6. Confirm the loaded content can cause a real security effect
```

## Key Code Patterns

```java
String url = getIntent().getStringExtra("url");
webView.loadUrl(url);
```

```java
@Override
public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
    view.loadUrl(request.getUrl().toString());
    return true;
}
```

## Secure Pattern

```java
if (ALLOWED_SCHEMES.contains(scheme) && ALLOWED_HOSTS.contains(host)) {
    return false;
}
return true;
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + JavaScript bridge | attacker page invokes native methods | [[app-webview-js-bridge]] |
| + file-access leakage | attacker page reaches local files or providers | [[app-webview-file-access]] |
| + `intent://` injection | attacker page pivots into native navigation | [[app-webview-intent-scheme]] |
| + cookie theft | attacker-controlled host receives session cookies | [[app-webview-cookie-theft]] |

## Related

[[app-webview]]
[[app-webview-js-bridge]]
[[app-webview-file-access]]
[[app-webview-intent-scheme]]
[[app-webview-cookie-theft]]
