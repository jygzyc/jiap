# Provider - Query - SQL Injection

A provider builds SQL from attacker-controlled input such as `selection`, `sortOrder`, URI parameters, or dynamic table clauses. This lets the caller change query semantics and reach data that filters should have blocked.

**Risk: HIGH**

## Exploit Prerequisites

The provider is externally reachable and builds SQL through concatenation or unsafe query-builder configuration.

**Android Version Scope:** Relevant across Android versions.

## Bypass Conditions / Uncertainties

- If the provider uses proper parameterization and strict query-builder modes, reject the finding
- `sortOrder` control is only meaningful if it changes access to sensitive rows or enables injection semantics
- If the provider is not externally reachable, keep the issue only as a chain element

## Visible Impact

Visible impact must be concrete, such as:

- reading secrets from other tables
- bypassing row filters
- enumerating protected schema or record content

## Attack Flow

```text
1. decx ard exported-components -P <port>
2. decx code class-source "<ProviderClass>" -P <port>
3. Inspect query(), rawQuery(), execSQL(), and SQLiteQueryBuilder usage
4. Confirm untrusted input reaches SQL structure rather than only bound parameters
5. Confirm the resulting data exposure or modification is meaningful
```

## Key Code Patterns

- string concatenation in SQL construction
- table or join fragments derived from URI parameters
- `SQLiteQueryBuilder` without strict mode

```java
String sql = "SELECT * FROM users WHERE " + selection;
return db.rawQuery(sql, selectionArgs);
```

## Secure Pattern

```java
String selection = "name = ?";
String[] selectionArgs = new String[]{ userInput };
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + provider data leak | arbitrary query access broadens the data leak | [[app-provider-data-leak]] |
| + `call()` exposure | hidden provider methods expose even richer primitives | [[app-provider-call-expose]] |
| + URI-grant abuse | extracted content is later re-shared | [[app-intent-uri-permission]] |

## Related

[[app-provider]]
[[app-provider-data-leak]]
[[app-provider-call-expose]]
[[app-intent-uri-permission]]
