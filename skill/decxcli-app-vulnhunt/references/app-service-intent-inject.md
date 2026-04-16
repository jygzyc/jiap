# Service - Intent - Command Injection

An exported service reads attacker-controlled command data from an incoming Intent. Without caller validation and strict action handling, the service performs sensitive work on attacker input.

**Risk: HIGH**

## Exploit Prerequisites

The service is externally reachable and processes untrusted extras in `onStartCommand()`, `onHandleIntent()`, or an equivalent handler. The attacker-controlled fields must influence a security-relevant action.

**Android Version Scope:** Relevant across modern Android, though background-start limits can affect some execution paths. Frontground-service and activity-launch restrictions change reliability, not the underlying trust-boundary flaw.

## Bypass Conditions / Uncertainties

- If a manifest permission protects the service but that permission is defined outside the current APK, treat bypassability as unconfirmed unless you can prove the permission is attacker-obtainable
- If the service validates the Binder caller, action name, and payload with a strict allowlist, reject the finding
- If the command reaches only benign or user-visible non-sensitive actions, reject the finding
- Shell-command injection requires either direct shell invocation or a similar command interpreter sink; do not infer it from plain string handling alone

## Visible Impact

Visible impact must be concrete, for example:

- delete or overwrite app files
- trigger an internal sync, upload, or account action
- start an internal component or broadcast with attacker-controlled parameters

If exploitation can only crash the service, do not report it.

## Attack Flow

```text
1. decx ard exported-components -P <port>
   -> locate exported services
2. decx code class-source "<ServiceClass>" -P <port>
   -> inspect onStartCommand / onHandleIntent
3. Trace extras into:
   -> switch/if action dispatchers
   -> file operations
   -> startActivity / startService / sendBroadcast
   -> Runtime.exec / ProcessBuilder
4. Confirm the final action is both attacker-controlled and security-relevant
```

## Key Code Patterns

- `action` or `target` extras dispatched directly
- service forwards a nested Intent from the caller
- shell or file operations built from unvalidated parameters

```java
@Override
protected void onHandleIntent(Intent intent) {
    String action = intent.getStringExtra("action");
    String target = intent.getStringExtra("target");

    switch (action) {
        case "delete":
            deleteFile(target);
            break;
        case "upload":
            uploadData(target);
            break;
    }
}
```

## Secure Pattern

```java
private static final Set<String> VALID_ACTIONS = Set.of("delete", "upload");

@Override
protected void onHandleIntent(Intent intent) {
    String action = intent.getStringExtra("action");
    if (!VALID_ACTIONS.contains(action)) {
        return;
    }

    if ("delete".equals(action)) {
        String target = intent.getStringExtra("target");
        if (isInAllowedDirectory(target)) {
            deleteFile(new File(target));
        }
    }
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + activity redirect | service reaches a non-exported component | [[app-activity-intent-redirect]] |
| + mutable `PendingIntent` | service creates victim-identity actions from attacker-controlled state | [[app-intent-pendingintent-escalation]] |
| + path traversal | turns an external command into arbitrary file access | [[app-activity-path-traversal]] |

## Related

[[app-service]]
[[app-activity-intent-redirect]]
[[app-intent-pendingintent-escalation]]
[[app-activity-path-traversal]]
