# Framework Service - Overview - Security Review

Framework services run in `system_server` or other privileged processes. The same data-flow bugs seen in apps become far more severe here because the caller crosses into privileged identity.

## Risk Catalog

| Risk | Rating | Reference |
|------|--------|-----------|
| `clearCallingIdentity()` misuse | HIGH | [[framework-service-clear-identity]] |
| Missing permission enforcement | CRITICAL | [[framework-service-permission-missing]] |
| Identity confusion | HIGH to CRITICAL | [[framework-service-identity-confusion]] |
| Intent redirect | HIGH | [[framework-service-intent-redirect]] |
| Sensitive data leak | HIGH | [[framework-service-data-leak]] |
| Race condition | MEDIUM to HIGH | [[framework-service-race-condition]] |

## Analysis Flow

```text
1. decx ard system-service-impl "<Interface>" -P <port> [--page <n>]
2. decx code class-context "<ServiceImpl>" -P <port>
   -> quick overview of all Binder-exposed methods
3. decx code class-source "<ServiceImpl>" -P <port>
4. Inspect Binder-exposed methods for:
   -> enforceCallingPermission / checkCallingPermission
   -> Binder.getCallingUid / UserHandle.getCallingUserId
   -> clearCallingIdentity / restoreCallingIdentity
   -> nested Intent or PendingIntent handling
4. Confirm the visible consequence:
   -> privileged action
   -> privileged data read
   -> persistent state change
```

## Key Trace Patterns

- Privileged operations before permission enforcement
- Caller identity inferred from user-supplied parameters
- Identity cleared too early or for too broad a scope
- Binder-exposed methods returning privileged data directly
- Intent forwarding from untrusted IPC into privileged launches

## Related

[[risk-rating]]
