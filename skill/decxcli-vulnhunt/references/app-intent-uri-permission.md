# Intent - URI Grant - Abuse

Intent-based URI grants delegate access to a `content://` URI across app boundaries. If the grant reaches an attacker-controlled recipient or is made persistable without tight control, the attacker inherits access they should not have.

**Risk: HIGH**

## Exploit Prerequisites

The app attaches a sensitive `content://` URI to an attacker-reachable Intent and includes `FLAG_GRANT_*`, `ClipData`, persistable grants, or equivalent delegated access behavior.

**Android Version Scope:** Relevant across modern Android. Platform changes affect some routing patterns, but the core trust-boundary flaw remains.

## Bypass Conditions / Uncertainties

- If the Intent is constrained to a trusted explicit package/component and no attacker-controlled relay exists, reject the finding
- Persistable-grant impact requires the recipient to be able to call `takePersistableUriPermission()`
- Prefix grants matter only when the shared subtree actually exposes more than a single intended object
- If the granted URI does not expose meaningful data or write capability, reject the finding

## Visible Impact

Visible impact must be concrete, such as:

- reading a private file through a granted URI
- writing to a protected file or record through a writable URI
- preserving long-term access through a persistable grant

## Attack Flow

```text
1. Trace grantUriPermission, Intent.addFlags/setFlags, and ClipData usage
2. Identify FLAG_GRANT_READ / WRITE / PERSISTABLE / PREFIX paths
3. Confirm the receiving flow is attacker-reachable:
   -> implicit Intent
   -> setResult()
   -> exported component
4. Confirm the URI grants meaningful read or write access
```

## Key Code Patterns

```java
Intent intent = new Intent("com.app.VIEW_FILE");
intent.setData(contentUri);
intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
startActivity(intent);
```

```java
grantIntent.setClipData(clip);
grantIntent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION
        | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
```

## Secure Pattern

```java
intent.setPackage("com.trusted.receiver");
revokeUriPermission(contentUri, Intent.FLAG_GRANT_READ_URI_PERMISSION);
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + provider traversal | granted URI exposes arbitrary files | [[app-provider-path-traversal]] |
| + `setResult()` leak | caller receives a reusable private URI | [[app-activity-setresult-leak]] |
| + FileProvider misconfig | delegated URI points into an over-broad file surface | [[app-provider-fileprovider-misconfig]] |

## Related

[[app-intent]]
[[app-provider-path-traversal]]
[[app-activity-setresult-leak]]
[[app-provider-fileprovider-misconfig]]
