# Provider - getType() - Reconnaissance Leak

`getType()` can act as an oracle when its return value changes based on whether a file, row, or object exists. On its own this is usually reconnaissance, but it matters when it strengthens a practical attack path.

**Risk: LOW**

## Exploit Prerequisites

The provider is externally reachable and `getType()` returns distinguishable values based on the existence or state of a protected object.

**Android Version Scope:** Relevant across Android versions.

## Bypass Conditions / Uncertainties

- Reject the finding if `getType()` returns a constant value that does not reveal object existence or state
- Reject the finding if the provider is not externally reachable
- Keep severity low unless the oracle materially enables a higher-value follow-on attack
- Prefer not reporting it as a standalone issue unless the oracle clearly improves a practical attack path

## Visible Impact

Visible impact is usually limited to reconnaissance, such as:

- determining whether a private file exists
- mapping valid paths, record types, or internal object names

Do not overstate this as direct data disclosure unless content itself is revealed. In many audits this is better treated as supporting evidence for another bug than as a standalone issue.

## Attack Flow

```text
1. decx code class-source "<ProviderClass>" -P <port>
2. Inspect getType()
3. Confirm return values differ based on protected-object existence or state
4. Confirm the provider is externally reachable
```

## Key Code Patterns

```java
@Override
public String getType(Uri uri) {
    File file = new File(getContext().getFilesDir(), uri.getPath());
    if (file.exists()) {
        return "image/jpeg";
    }
    return null;
}
```

## Secure Pattern

```java
@Override
public String getType(Uri uri) {
    return "application/octet-stream";
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + provider traversal | attacker maps valid file targets before reading them | [[app-provider-path-traversal]] |
| + parcel mismatch | MIME-based oracle helps validation-bypass style chains | [[app-intent-parcel-mismatch]] |

## Related

[[app-provider]]
[[app-provider-path-traversal]]
[[app-intent-parcel-mismatch]]
