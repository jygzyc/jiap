# Activity - Intent - Redirect

An exported activity accepts an attacker-controlled nested Intent and forwards it onward. This lets the attacker cross an internal trust boundary and reach components that were not meant to be directly exposed.

**Risk: HIGH**

## Exploit Prerequisites

The activity is externally reachable and reads a nested Intent, `ClipData`, or equivalent routing payload from an untrusted source, then forwards it through `startActivity()`, `startService()`, or `sendBroadcast()` without target validation.

**Android Version Scope:** Relevant across all Android versions. App-layer forwarding bugs remain exploitable even though some framework redirect cases were hardened in Android 12 and later.

## Bypass Conditions / Uncertainties

- If the activity is protected only by a custom permission defined in another app, treat the protection as bypassable only when the permission is attacker-definable, `normal`, `dangerous`, or otherwise not provably signature-bound to the target
- If validation checks only the action or scheme but not the explicit component/package, the attacker can usually bypass it with an explicit component name
- If the forwarded Intent carries URI grants, the attacker may bypass otherwise private file boundaries even when the target component itself is not exported
- Reject the finding if the target component is allowlisted by exact class name and the caller is validated by signature or immutable UID allowlist

## Visible Impact

Only report concrete outcomes, for example:

- launching a non-exported internal admin or account screen
- triggering a non-exported service operation
- returning a privileged `content://` grant to the attacker

If the redirected target performs no security-relevant action, reject the finding.

## Attack Flow

```text
1. decx ard exported-components -P <port>
   -> locate exported activities
2. decx code class-source "<ActivityClass>" -P <port>
   -> inspect onCreate / onNewIntent
3. decx code search-method "getParcelableExtra" -P <port>
4. Trace the nested Intent into:
   -> startActivity
   -> startService
   -> sendBroadcast
5. Confirm whether the downstream target is actually sensitive and reachable through this redirect
```

## Key Code Patterns

- `getParcelableExtra("forward_intent")` followed by immediate forwarding
- `Intent` target validation based only on action or package prefix
- forwarding code that preserves `FLAG_GRANT_*` URI permissions

```java
@Override
protected void onCreate(Bundle savedInstanceState) {
    super.onCreate(savedInstanceState);
    Intent nested = getIntent().getParcelableExtra("forward_intent");
    startActivity(nested); // vulnerable: attacker controls the forwarded target
}
```

## Secure Pattern

```java
private static final Set<String> ALLOWED_TARGETS = Set.of(
    "com.app.TargetActivity1",
    "com.app.TargetActivity2"
);

@Override
protected void onCreate(Bundle savedInstanceState) {
    super.onCreate(savedInstanceState);
    Intent nested = getIntent().getParcelableExtra("forward_intent");
    if (nested == null) return;

    ComponentName target = nested.getComponent();
    if (target == null || !ALLOWED_TARGETS.contains(target.getClassName())) {
        return;
    }

    nested.setFlags(nested.getFlags()
        & ~Intent.FLAG_GRANT_READ_URI_PERMISSION
        & ~Intent.FLAG_GRANT_WRITE_URI_PERMISSION);

    startActivity(nested);
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + service command injection | reaches a non-exported service operation | [[app-service-intent-inject]] |
| + `PendingIntent` abuse | reuses victim identity for a downstream action | [[app-intent-pendingintent-escalation]] |
| + FileProvider grant abuse | turns redirect into private-file access | [[app-provider-fileprovider-misconfig]] |
| + WebView file access | opens an internal WebView host on attacker-controlled content | [[app-webview-file-access]] |

## Related

[[app-activity]]
[[app-intent]]
[[app-service-intent-inject]]
[[app-intent-pendingintent-escalation]]
[[app-provider-fileprovider-misconfig]]
[[app-webview-file-access]]
[[framework-service-intent-redirect]]
