# Provider - Query - Data Leak

An exported provider returns sensitive rows or columns to any caller. This happens when `query()` lacks meaningful caller validation or response filtering.

**Risk: HIGH**

## Exploit Prerequisites

The provider is externally reachable and `query()` returns sensitive data without a strong read permission or explicit caller validation.

**Android Version Scope:** Relevant across Android versions.

## Bypass Conditions / Uncertainties

- If a non-bypassable signature permission protects reads, reject the finding
- If only non-sensitive data is returned, reject the finding
- If permissions are asymmetric, note whether write-only access can still leak structure or state, but do not overstate it as a read leak without evidence
- If a custom permission is defined outside the current APK, keep the bypass condition explicit

## Visible Impact

Visible impact must be concrete, such as:

- reading tokens, passwords, account rows, chat history, or private configuration
- enumerating sensitive records that should stay inside the app sandbox

## Attack Flow

```text
1. decx ard exported-components -P <port>
2. decx code class-source "<ProviderClass>" -P <port>
3. Inspect query() for caller validation and returned columns
4. Confirm the returned data is genuinely sensitive
```

## Key Code Patterns

- exported provider with no strong read permission
- `query()` returning sensitive columns directly

```java
@Override
public Cursor query(Uri uri, String[] projection, String selection,
                    String[] selectionArgs, String sortOrder) {
    return db.query("users", projection, selection, selectionArgs,
                    null, null, sortOrder);
}
```

## Secure Pattern

```java
String[] safeProjection = filterProjection(projection, ALLOWED_COLUMNS);
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + mutable `PendingIntent` | victim identity is reused to query the provider | [[app-intent-pendingintent-escalation]] |
| + SQL injection | leak escalates from row-level access to arbitrary queries | [[app-provider-sql-injection]] |
| + path traversal | provider file APIs expose even broader data | [[app-provider-path-traversal]] |

## Related

[[app-provider]]
[[app-intent-pendingintent-escalation]]
[[app-provider-sql-injection]]
[[app-provider-path-traversal]]
