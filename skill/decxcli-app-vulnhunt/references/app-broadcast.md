# Broadcast - Overview - Security Review

Broadcasts are often treated as low-risk plumbing, but exported receivers, ordered broadcasts, and weak permission design can expose command-injection and data-leak paths.

## Risk Catalog

| Risk | Rating | Reference |
|------|--------|-----------|
| Dynamic receiver abuse | MEDIUM | [[app-broadcast-dynamic-abuse]] |
| Ordered broadcast hijack | MEDIUM | [[app-broadcast-ordered-hijack]] |
| Broadcast permission bypass | MEDIUM | [[app-broadcast-permission-bypass]] |
| Broadcast data leak | MEDIUM | [[app-broadcast-local-leak]] |

## Analysis Flow

```text
1. decx ard app-receivers --exclude-package "androidx\\..*" --exclude-package "android\\.support\\..*" -P <port>
2. decx code search-method "registerReceiver" -P <port>
3. decx code class-source "<ReceiverClass>" -P <port>
4. Inspect:
   -> onReceive() inputs
   -> permission arguments on sendBroadcast / sendOrderedBroadcast / registerReceiver
   -> exported behavior for runtime-registered receivers
5. Check whether the receiver triggers:
   -> startActivity / startService / sendBroadcast
   -> file or database actions
   -> credential or token handling
```

## Key Trace Patterns

- Dynamic receivers that trust arbitrary actions or extras
- Ordered broadcasts with no receiver permission
- Custom permissions defined as `normal` or otherwise attacker-obtainable
- Broadcast payloads containing tokens, account info, or internal state

## Related

[[app-intent]]
[[app-service]]
[[risk-rating]]
