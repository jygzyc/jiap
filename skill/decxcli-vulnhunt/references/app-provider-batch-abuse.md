# Provider - Batch - Operation Abuse

`applyBatch()` and `bulkInsert()` enable transactional bulk writes through a single provider call. If validation only covers the batch entry point and not each operation, an attacker can mass-write, mass-delete, or poison records.

**Risk: MEDIUM**

## Exploit Prerequisites

This usually requires an exported provider or another reachable grant chain. The provider exposes `applyBatch()` or `bulkInsert()` and fails to enforce caller authorization, target-URI validation, or per-row validation for each operation.

**Android Version Scope:** Affects all Android versions. This is an app-layer authorization design flaw.

## Bypass Conditions / Uncertainties

- If the provider-level permission is defined outside the current APK, treat the finding as bypassable only when one of the following is true:
  - the defining app is attacker-controlled
  - the permission is `normal` or `dangerous`
  - the permission ownership and signature binding cannot be confirmed statically
- If `applyBatch()` delegates into internal helpers that skip the checks present in public `insert()` or `update()`, the provider-level guard is effectively bypassed
- If every operation is revalidated per caller and per target URI, reject the finding

## Visible Impact

Visible impact must be concrete. Valid examples:

- attacker can insert or overwrite account/profile rows
- attacker can delete a large set of locally stored records
- attacker can poison configuration flags that change app behavior

If the only effect is malformed-input crashes or non-sensitive test data writes, do not report it.

## Attack Flow

```text
1. decx ard exported-components -P <port>
   -> locate exported providers and authority strings
2. decx code class-source "<ProviderClass>" -P <port>
   -> inspect applyBatch / bulkInsert
3. Check whether each ContentProviderOperation and each target URI is revalidated
4. Check whether the transaction touches sensitive tables or settings rows
5. Confirm that the resulting write/delete effect is visible and security-relevant
```

## Key Code Patterns

- `applyBatch()` validates only once before entering the transaction
- `ContentProviderOperation.apply()` is called without re-checking the caller or target URI
- `bulkInsert()` trusts all incoming `ContentValues` and writes directly into sensitive tables

```java
@Override
public ContentProviderResult[] applyBatch(ArrayList<ContentProviderOperation> operations)
        throws OperationApplicationException {
    db.beginTransaction();
    try {
        ContentProviderResult[] results = new ContentProviderResult[operations.size()];
        for (int i = 0; i < operations.size(); i++) {
            results[i] = operations.get(i).apply(this, results, i);
            // vulnerable: no per-operation authorization or payload validation
        }
        db.setTransactionSuccessful();
        return results;
    } finally {
        db.endTransaction();
    }
}
```

## Secure Pattern

```java
@Override
public ContentProviderResult[] applyBatch(ArrayList<ContentProviderOperation> operations)
        throws OperationApplicationException {
    final int callingUid = Binder.getCallingUid();
    for (ContentProviderOperation op : operations) {
        Uri uri = op.getUri();
        enforceOperationAllowed(callingUid, uri, op);
        validateOperationPayload(op);
    }
    return super.applyBatch(operations);
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + SQL injection | Escalates single-row injection into transactional bulk corruption | [[app-provider-sql-injection]] |
| + `call()` exposure | Uses a hidden IPC method to reach the batch path | [[app-provider-call-expose]] |
| + provider data leak | Poison data first, then read back the modified state | [[app-provider-data-leak]] |

## Related

[[app-provider]]
[[app-provider-call-expose]]
[[app-provider-data-leak]]
[[app-provider-sql-injection]]
