# WebView - Overview - Security Review

Modern Android WebView analysis must cover more than `loadUrl()`. Real attack chains now frequently involve QR-scan results, browser bridges, URI grants, file chooser callbacks, and message channels such as `postWebMessage`, `WebMessagePort`, or `addWebMessageListener` that feed untrusted content into trusted native actions.

## Risk Catalog

| Risk | Rating | Reference |
|------|--------|-----------|
| JavaScript bridge exposure | MEDIUM to CRITICAL | [[app-webview-js-bridge]] |
| File-access leakage | HIGH | [[app-webview-file-access]] |
| URL validation bypass | MEDIUM | [[app-webview-url-bypass]] |
| SSL bypass | MEDIUM | [[app-webview-ssl-bypass]] |
| Cookie theft | MEDIUM | [[app-webview-cookie-theft]] |
| `intent://` scheme injection | MEDIUM | [[app-webview-intent-scheme]] |
| Scan / QR result injection | MEDIUM | [[app-webview-scan-result-inject]] |

## Analysis Flow

```text
1. decx code search-global "WebView" --limit 50 -P <port>
   -> locate WebView hosts
2. decx code class-context "<WebViewHost>" -P <port>
   -> quick overview of all methods (bridge, handlers, callbacks)
3. decx code class-source "<WebViewHost>" -P <port>
   -> inspect:
      - addJavascriptInterface
      - postWebMessage / WebMessagePort / addWebMessageListener
      - loadUrl / loadData / loadDataWithBaseURL / evaluateJavascript
      - onActivityResult / ActivityResultLauncher callbacks
      - shouldOverrideUrlLoading / shouldInterceptRequest
      - onShowFileChooser / file chooser result handling
      - CookieManager / WebSettings
3. Trace attacker-controlled sources:
   -> Intent extras, deep links, scan results, QR parser output
   -> externally supplied URLs or HTML
   -> file chooser URIs
4. Confirm whether URL/domain allowlists, scheme checks, and bridge exposure rules are non-bypassable
```

## Key Trace Patterns

- Scan or QR payload flows directly into `loadUrl()` or bridge-triggering JavaScript
- `intent://` or custom schemes reach native navigation without allowlist enforcement
- `addJavascriptInterface()` or message-channel bridges expose sensitive native actions to attacker-controlled content
- File chooser or picker results are trusted without URI-owner validation
- Cookie-setting logic accepts attacker-controlled domains or redirect targets

## Related

[[app-activity]]
[[app-intent]]
[[app-provider]]
[[risk-rating]]
