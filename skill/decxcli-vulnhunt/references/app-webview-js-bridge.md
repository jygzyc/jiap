# WebView - JavaScript Bridge - Exposure

`addJavascriptInterface()` and related message-channel bridges expose native functionality to web content. This becomes exploitable when attacker-controlled content can reach the WebView and invoke sensitive native actions.

**Risk: MEDIUM to CRITICAL**

## Exploit Prerequisites

The WebView registers a bridge object with sensitive `@JavascriptInterface` methods, or an equivalent web-to-native message bridge, and attacker-controlled content can execute inside that WebView.

**Android Version Scope:** Relevant across modern Android. The risk depends more on bridge sensitivity and content control than on platform version.

## Bypass Conditions / Uncertainties

- The bridge alone is not enough; reject the finding if attacker-controlled content cannot reach the WebView
- If the bridge exposes only harmless methods, reject the finding
- Escalate toward `CRITICAL` only when the bridge enables highly sensitive or privileged actions, not just generic data reads
- `evaluateJavascript()` injection must be proven as attacker-controlled code execution, not merely string interpolation with no executable effect

## Visible Impact

Visible impact must be concrete, such as:

- reading tokens or user data
- invoking sensitive in-app operations
- abusing device capabilities exposed by the bridge

## Attack Flow

```text
1. decx code search-class "WebView" -P <port>
2. decx code class-source "<WebViewHost>" -P <port>
3. Locate addJavascriptInterface, `postWebMessage`, `WebMessagePort`, or `addWebMessageListener`
4. Inspect each exposed bridge or message handler for sensitive reads or actions
5. Confirm attacker-controlled content can reach the WebView
```

## Key Code Patterns

- sensitive bridge methods or native message handlers
- attacker-controlled page or injected JS calling the bridge or sending crafted web messages

```java
webView.getSettings().setJavaScriptEnabled(true);
webView.addJavascriptInterface(new SensitiveBridge(), "Android");
```

```java
class SensitiveBridge {
    @JavascriptInterface
    public String getToken() {
        return authToken;
    }
}
```

## Secure Pattern

```java
webView.addJavascriptInterface(new MinimalBridge(), "bridge");
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + URL validation bypass | attacker-controlled content reaches the bridge | [[app-webview-url-bypass]] |
| + file access leakage | local files become readable and exfiltrated through the bridge | [[app-webview-file-access]] |
| + SSL bypass | injected content reaches the bridge over a MITM path | [[app-webview-ssl-bypass]] |
| + scan-result injection | QR payload drives malicious content into the trusted WebView | [[app-webview-scan-result-inject]] |

## Related

[[app-webview]]
[[app-webview-url-bypass]]
[[app-webview-file-access]]
[[app-webview-ssl-bypass]]
[[app-webview-scan-result-inject]]
