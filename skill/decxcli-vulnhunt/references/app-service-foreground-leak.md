# Service - Foreground Notification - Leak

A foreground service notification may expose sensitive information on screen or lock screen. The issue matters only when the visible content reveals data a nearby attacker should not learn.

**Risk: LOW**

## Exploit Prerequisites

The notification text, title, style, or expanded content includes sensitive values such as location, message content, identifiers, or account state.

**Android Version Scope:** Relevant across Android versions.

## Bypass Conditions / Uncertainties

- If the notification contains only generic status text, reject the finding
- If the data is hidden on lock screen and not otherwise exposed, lower or reject the finding depending on the remaining visibility
- This is low severity unless the leaked content is genuinely sensitive and visible

## Visible Impact

Visible impact must be concrete, such as:

- exposing exact location
- exposing message or clipboard content
- exposing token-like or account-related state

## Attack Flow

```text
1. decx code class-source "<ServiceClass>" -P <port>
2. Locate startForeground() and notification builder code
3. Confirm whether sensitive variables flow into visible notification fields
4. Check visibility settings for lock-screen exposure
```

## Key Code Patterns

```java
Notification notification = new NotificationCompat.Builder(this, CHANNEL_ID)
    .setContentTitle("Tracking location")
    .setContentText("Current location: " + location)
    .build();
```

## Secure Pattern

```java
.setContentText("Location service is running")
.setVisibility(NotificationCompat.VISIBILITY_SECRET)
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + clickjacking | visible content helps social-engineering flows | [[app-activity-clickjacking]] |
| + service command injection | attacker controls parameters that appear in the notification | [[app-service-intent-inject]] |
| + broadcast data leak | same sensitive state is also exposed through broadcasts | [[app-broadcast-local-leak]] |

## Related

[[app-service]]
[[app-activity-clickjacking]]
[[app-service-intent-inject]]
[[app-broadcast-local-leak]]
