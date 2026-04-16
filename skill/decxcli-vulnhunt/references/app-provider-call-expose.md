# Provider - Call API - Exposure

`ContentProvider.call()` exposes custom IPC methods outside the standard CRUD path. If those methods lack explicit caller validation, the provider gains a hidden high-impact attack surface.

**Risk: HIGH**

## Exploit Prerequisites

The provider is externally reachable and overrides `call()` to perform sensitive reads, writes, file operations, or privileged helper actions without strong caller validation. Do not assume provider-level read/write permissions automatically make `call()` safe unless the implementation enforces equivalent checks itself.

**Android Version Scope:** Relevant across Android versions.

## Bypass Conditions / Uncertainties

- If `call()` enforces a strong permission and exact method allowlist, reject the finding
- If the exposed methods perform only benign public operations, reject the finding
- If the provider is exported but `call()` reuses the same strong permission and caller validation as the protected CRUD path, reject the finding
- Do not claim code execution unless the `call()` path actually loads or executes attacker-controlled code

## Visible Impact

Visible impact must be concrete, such as:

- deleting files
- resetting protected state
- returning privileged handles or sensitive data

## Attack Flow

```text
1. decx ard exported-components -P <port>
2. decx code class-source "<ProviderClass>" -P <port>
3. Inspect whether call() is overridden
4. Enumerate supported method names and their effects
5. Confirm whether caller validation exists before the sensitive branch
```

## Key Code Patterns

- custom switch on `method` with no caller validation

```java
@Override
public Bundle call(String method, String arg, Bundle extras) {
    switch (method) {
        case "deleteFile":
            String path = extras.getString("path");
            new File(path).delete();
            return Bundle.EMPTY;
    }
    return super.call(method, arg, extras);
}
```

## Secure Pattern

```java
if (!ALLOWED_METHODS.contains(method)) {
    return null;
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + provider data leak | hidden methods reveal extra data outside query() | [[app-provider-data-leak]] |
| + path traversal | attacker-controlled paths reach file operations | [[app-provider-path-traversal]] |
| + mutable `PendingIntent` | privileged handles or actions are returned to the attacker | [[app-intent-pendingintent-escalation]] |

## Related

[[app-provider]]
[[app-provider-data-leak]]
[[app-provider-path-traversal]]
[[app-intent-pendingintent-escalation]]
