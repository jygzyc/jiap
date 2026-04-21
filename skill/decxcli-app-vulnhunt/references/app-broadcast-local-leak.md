# Broadcast - Global Delivery - Data Leak

An app sends sensitive data through a global broadcast without restricting receivers. Any app on the device can register for the action and read the payload.

**Risk: MEDIUM**

## Exploit Prerequisites

The app uses `sendBroadcast()` or an equivalent global broadcast API to send tokens, identifiers, account state, URIs, or other sensitive fields without `setPackage()`, receiver permission, or an internal-only mechanism.

**Android Version Scope:** Most relevant to broader broadcast exposure patterns on older Android, but the core flaw is still unrestricted sensitive broadcast delivery.

## Bypass Conditions / Uncertainties

- If the broadcast is package-restricted or protected by a non-bypassable signature permission, reject the finding
- If the payload contains no sensitive or security-relevant data, reject the finding
- If the flow already uses an internal-only mechanism, such as a package-locked path, do not report it

## Visible Impact

Visible impact must be concrete, such as:

- leaking tokens or credentials
- leaking sensitive URIs or authority names that unlock other surfaces
- exposing user or device state that supports account takeover or further exploitation

## Attack Flow

```text
1. decx code xref-method "android.content.Context.sendBroadcast(android.content.Intent):void" -P <port>
2. For each call site, decx code method-context "<sendingMethod>" -P <port>
   -> callees show what data flows into the broadcast Intent
3. Inspect the payload placed into the broadcast Intent
4. Confirm there is no package restriction or strong receiver permission
5. Confirm the payload is actually sensitive
```

## Key Code Patterns

- global broadcast of auth token or equivalent secret

```java
Intent intent = new Intent("com.app.ACTION_TOKEN_READY");
intent.putExtra("auth_token", secretToken);
sendBroadcast(intent);
```

## Secure Pattern

```java
intent.setPackage(getPackageName());
sendBroadcast(intent);
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + implicit Intent hijack | the same unrestricted routing leaks internal communications | [[app-intent-implicit-hijack]] |
| + provider data leak | leaked authority or URI is reused against a provider | [[app-provider-data-leak]] |
| + `PendingIntent` abuse | leaked object or handle becomes a victim-identity action | [[app-intent-pendingintent-escalation]] |

## Related

[[app-broadcast]]
[[app-intent-implicit-hijack]]
[[app-provider-data-leak]]
[[app-intent-pendingintent-escalation]]
