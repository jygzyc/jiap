# Intent - PendingIntent - Privilege Abuse

A mutable `PendingIntent` lets another party fill missing fields or rewrite extras before execution. When the base Intent is incomplete, the attacker effectively chooses what the victim app does.

**Risk: HIGH**

## Exploit Prerequisites

The target app creates a `PendingIntent` with `FLAG_MUTABLE` and leaves meaningful fields attacker-fillable, such as the action, component, data URI, or security-relevant extras.

**Android Version Scope:** Still relevant on modern Android. Android 12 and later reduced accidental misuse by requiring explicit mutability, and Android 14 further restricts unsafe patterns, but explicit mutable use with incomplete base Intents remains dangerous.

## Bypass Conditions / Uncertainties

- If the base Intent already pins the exact component, action, data, and relevant extras, mutability alone is not enough
- If the mutable `PendingIntent` is never exposed to an untrusted receiver, reject the finding
- If protection depends on a custom permission defined in another app, keep the finding at `candidate` unless the bypass condition is explicit
- URI-grant abuse becomes possible if the attacker can inject `content://` data plus `FLAG_GRANT_*`

## Visible Impact

Only report a visible effect, for example:

- launching an internal or protected component with victim identity
- reading or writing provider data as the victim app
- sending a privileged broadcast or performing a dangerous app action

If the attacker can only append irrelevant extras without changing behavior, reject the finding.

## Attack Flow

```text
1. decx code xref-method "android.app.PendingIntent.getActivity(android.content.Context,int,android.content.Intent,int):android.app.PendingIntent" -P <port>
2. decx code xref-method "android.app.PendingIntent.getService(android.content.Context,int,android.content.Intent,int):android.app.PendingIntent" -P <port>
3. decx code xref-method "android.app.PendingIntent.getBroadcast(android.content.Context,int,android.content.Intent,int):android.app.PendingIntent" -P <port>
4. Inspect flags for FLAG_MUTABLE vs FLAG_IMMUTABLE
5. Confirm which fields in the base Intent stay attacker-fillable
6. Confirm the downstream action is visible and security-relevant
```

## Key Code Patterns

- empty or partially filled base Intent
- mutable flag combined with externally exposed `PendingIntent`
- downstream sink using victim identity to access internal surfaces

```java
Intent intent = new Intent(); // incomplete base intent
PendingIntent pi = PendingIntent.getActivity(
    this,
    0,
    intent,
    PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_MUTABLE
);
```

## Secure Pattern

```java
Intent intent = new Intent(this, TargetActivity.class);
intent.setAction("com.app.EXPECTED_ACTION");

PendingIntent pi = PendingIntent.getActivity(
    this,
    0,
    intent,
    PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE
);
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + activity redirect | victim identity launches a non-exported target | [[app-activity-intent-redirect]] |
| + provider data leak | victim identity reads provider data otherwise denied to the attacker | [[app-provider-data-leak]] |
| + URI grant abuse | victim identity grants attacker file access | [[app-intent-uri-permission]] |

## Related

[[app-intent]]
[[app-activity-intent-redirect]]
[[app-provider-data-leak]]
[[app-intent-uri-permission]]
