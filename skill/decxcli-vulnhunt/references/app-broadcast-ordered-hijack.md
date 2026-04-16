# Broadcast - Ordered - Hijack

Ordered broadcasts let earlier receivers observe, modify, or abort the payload before the intended consumer runs. Without strict receiver restrictions, an attacker can tamper with the flow.

**Risk: MEDIUM**

## Exploit Prerequisites

The app sends an ordered broadcast containing sensitive data or control fields and does not restrict which receivers may participate.

**Android Version Scope:** Historically stronger on older Android patterns, but still relevant wherever ordered broadcasts are used without strong receiver constraints.

## Bypass Conditions / Uncertainties

- If the ordered broadcast specifies a non-bypassable receiver permission, reject the finding
- If modifying or aborting the broadcast cannot change a security-relevant outcome, reject the finding
- If only static internal receivers can consume the broadcast, reject the finding

## Visible Impact

Visible impact must be concrete, such as:

- changing a command or target before the intended receiver handles it
- stealing OTPs or other sensitive message content
- aborting a security-relevant broadcast so a victim workflow never completes

## Attack Flow

```text
1. decx code xref-method "android.content.Context.sendOrderedBroadcast(android.content.Intent,java.lang.String):void" -P <port>
2. Inspect the payload and permission arguments
3. Confirm whether a high-priority external receiver can observe or modify the result
4. Confirm the downstream effect is security-relevant
```

## Key Code Patterns

- ordered broadcast with `null` permission
- downstream receiver trusting mutable result data

```java
context.sendOrderedBroadcast(intent, null);
```

## Secure Pattern

```java
context.sendOrderedBroadcast(intent, "com.example.PERMISSION_SMS");
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + intent redirect | tampered broadcast injects a new target component | [[app-activity-intent-redirect]] |
| + broadcast data leak | attacker reads and then modifies the same sensitive flow | [[app-broadcast-local-leak]] |
| + service command injection | changed fields reach a service launch path | [[app-service-intent-inject]] |

## Related

[[app-broadcast]]
[[app-activity-intent-redirect]]
[[app-service-intent-inject]]
