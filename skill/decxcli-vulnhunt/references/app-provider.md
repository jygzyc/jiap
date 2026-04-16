# Provider - Overview - Security Review

ContentProviders expose one of the clearest app-data boundaries. Export mistakes, weak per-operation checks, and URI/file handling bugs often lead directly to real data access.

## Risk Catalog

| Risk | Rating | Reference |
|------|--------|-----------|
| Provider data leak | HIGH | [[app-provider-data-leak]] |
| SQL injection | HIGH | [[app-provider-sql-injection]] |
| Path traversal | HIGH | [[app-provider-path-traversal]] |
| `call()` exposure | HIGH | [[app-provider-call-expose]] |
| Batch-operation abuse | MEDIUM | [[app-provider-batch-abuse]] |
| `getType()` reconnaissance leak | LOW | [[app-provider-gettype-infoleak]] |
| FileProvider misconfiguration | HIGH | [[app-provider-fileprovider-misconfig]] |

## Analysis Flow

```text
1. decx ard exported-components -P <port>
   -> locate exported providers and authorities
2. decx code class-source "<ProviderClass>" -P <port>
   -> inspect query / insert / update / delete / openFile / call / applyBatch / bulkInsert
3. Check:
   -> manifest permission on the provider
   -> per-method caller validation
   -> per-URI and per-row validation
   -> path normalization and root confinement
4. Track whether the provider can expose:
   -> account rows, tokens, chat history, files, config data
   -> attacker-controlled writes into sensitive tables
```

## Key Trace Patterns

- Exported provider with no caller check in `query()`
- SQL assembled from untrusted `selection` or path fragments
- File paths derived from URI segments without canonical-path validation
- `call()` used as a hidden privileged IPC surface
- `applyBatch()` or `bulkInsert()` validating the batch only once

## Common False Positives

- Provider is exported but all sensitive methods enforce a non-bypassable signature permission
- `getType()` only reveals a generic MIME type with no file-existence oracle
- Batch support exists but every operation is revalidated per caller and per target URI

## Related

[[app-intent]]
[[risk-rating]]
