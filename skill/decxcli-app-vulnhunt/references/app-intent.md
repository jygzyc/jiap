# Intent - Overview - Security Review

Intents are the transport layer of Android trust boundaries. They carry data across activities, services, receivers, providers, and internal app handoff paths, so Intent review is usually chain review.

## Risk Catalog

| Risk | Rating | Reference |
|------|--------|-----------|
| Mutable `PendingIntent` privilege abuse | HIGH | [[app-intent-pendingintent-escalation]] |
| URI grant / persistable grant abuse | HIGH | [[app-intent-uri-permission]] |
| Implicit Intent hijack | MEDIUM | [[app-intent-implicit-hijack]] |
| ClassLoader / deserialization injection | HIGH | [[app-intent-classloader-inject]] |
| Parcel mismatch | HIGH | [[app-intent-parcel-mismatch]] |

## Analysis Flow

```text
1. Find sources:
   -> getIntent().get*Extra()
   -> getIntent().getData()
   -> getClipData()
   -> nested Intent extraction
   -> ActivityResult / scan-result callbacks
2. Find transfer points:
   -> startActivity / startService / sendBroadcast
   -> setResult
   -> PendingIntent.getActivity / getService / getBroadcast
   -> grantUriPermission / takePersistableUriPermission
3. Inspect protections:
   -> explicit component vs implicit routing
   -> caller validation
   -> package/signature allowlists
   -> `FLAG_MUTABLE`, `FLAG_GRANT_*`, persistable/prefix grants
4. Check whether the downstream effect is visible and matches risk-rating guidance
```

## Key Trace Patterns

- Nested Intent forwarding without target validation
- Mutable `PendingIntent` created from incomplete base intents
- `ClipData` and URI grants passed to attacker-reachable flows
- Activity results or scan results trusted as if they came from the app itself
- Custom parcelables or serializables parsed across trust boundaries

## Related

[[app-activity]]
[[app-service]]
[[app-provider]]
[[app-webview]]
[[risk-rating]]
