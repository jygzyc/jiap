# WebView - Intent Scheme - Injection

The WebView processes `intent://` or equivalent custom-scheme URLs and converts web content into native launches. Without a strict target allowlist, attacker-controlled pages can drive trusted component invocations.

**Risk: MEDIUM**

## Exploit Prerequisites

Attacker-controlled web content reaches a WebView that parses `intent://` URLs and launches the resulting Intent without strict validation.

**Android Version Scope:** Relevant across Android versions because the flaw is in app-side scheme handling.

## Bypass Conditions / Uncertainties

- If the WebView blocks all non-allowlisted schemes, reject the finding
- If parsed Intents are stripped of explicit component, selector, and dangerous flags before trusted resolution, reject the finding
- If the resulting native target performs no meaningful action, reject the finding

## Visible Impact

Visible impact must be concrete, such as:

- launching an internal screen
- triggering a privileged native workflow
- forwarding URI grants or dangerous flags into another component

## Attack Flow

```text
1. decx code search-global "WebView" --limit 50 -P <port>
2. Inspect shouldOverrideUrlLoading
3. Confirm whether intent:// or custom schemes are parsed into Intents
4. Confirm whether the parsed Intent is launched without strict validation
```

## Key Code Patterns

```java
if (url.startsWith("intent://")) {
    Intent intent = Intent.parseUri(url, Intent.URI_INTENT_SCHEME);
    startActivity(intent);
    return true;
}
```

## Secure Pattern

```java
intent.addCategory(Intent.CATEGORY_BROWSABLE);
intent.setComponent(null);
intent.setSelector(null);
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + activity redirect | web content reaches a non-exported internal target | [[app-activity-intent-redirect]] |
| + URL bypass | weak navigation validation allows attacker content to reach scheme parsing | [[app-webview-url-bypass]] |
| + mutable `PendingIntent` | parsed Intent preserves dangerous victim-identity behavior | [[app-intent-pendingintent-escalation]] |

## Related

[[app-webview]]
[[app-activity-intent-redirect]]
[[app-webview-url-bypass]]
[[app-intent-pendingintent-escalation]]
