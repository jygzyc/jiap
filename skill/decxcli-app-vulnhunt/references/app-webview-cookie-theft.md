# WebView - Cookie - Theft

The app sets or exposes sensitive cookies in a way that attacker-controlled web content can read or receive them. This turns a trusted WebView session into a credential or session-token leak.

**Risk: MEDIUM**

## Exploit Prerequisites

The app uses `CookieManager` for a WebView flow and either sets cookies for attacker-controlled domains or exposes non-HttpOnly session data to attacker-controlled content.

**Android Version Scope:** Relevant across Android versions.

## Bypass Conditions / Uncertainties

- If cookies are only set for exact trusted domains and not exposed to attacker-controlled content, reject the finding
- Shared cookie storage alone is not a vulnerability without an attacker-controlled content path
- If the exposed cookies do not carry authentication or meaningful sensitive state, downgrade or reject

## Visible Impact

Visible impact must be concrete, such as:

- theft of session cookies
- reuse of authentication state
- forced authenticated requests under victim context

## Attack Flow

```text
1. decx code xref-method "android.webkit.CookieManager.setCookie(java.lang.String,java.lang.String):void" -P <port>
2. Inspect the URL/domain source for setCookie()
3. Confirm whether attacker-controlled content can read or receive the cookie
4. Confirm the cookie has real authentication or sensitive value
```

## Key Code Patterns

```java
String attackerUrl = getIntent().getDataString();
CookieManager.getInstance().setCookie(attackerUrl, "token=" + getUserToken());
webView.loadUrl(attackerUrl);
```

## Secure Pattern

```java
if (ALLOWED_DOMAINS.contains(host)) {
    manager.setCookie(url, "token=" + getUserToken() + "; HttpOnly; Secure");
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + URL validation bypass | attacker-controlled host receives cookies | [[app-webview-url-bypass]] |
| + JavaScript bridge | stolen session state is combined with native actions | [[app-webview-js-bridge]] |
| + scan-result injection | QR payload drives a trusted session into attacker content | [[app-webview-scan-result-inject]] |

## Related

[[app-webview]]
[[app-webview-url-bypass]]
[[app-webview-js-bridge]]
[[app-webview-scan-result-inject]]
