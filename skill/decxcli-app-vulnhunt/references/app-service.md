# Service - Overview - Security Review

Services expose long-lived IPC and background execution surfaces. Exported binders, Messenger handlers, and start-command paths frequently become privilege-misuse or unauthorized-action bugs.

## Risk Catalog

| Risk | Rating | Reference |
|------|--------|-----------|
| AIDL exposure | HIGH | [[app-service-aidl-expose]] |
| Messenger abuse | HIGH | [[app-service-messenger-abuse]] |
| Intent command injection | HIGH | [[app-service-intent-inject]] |
| Bound-service privilege abuse | HIGH | [[app-service-bind-escalation]] |
| Foreground-notification leakage | LOW | [[app-service-foreground-leak]] |

## Analysis Flow

```text
1. decx ard exported-components -P <port>
2. decx ard get-aidl --exclude-package "androidx\\..*" --exclude-package "android\\.support\\..*" -P <port>
3. decx code class-context "<ServiceClass>" -P <port>
   -> quick overview of onBind, onStartCommand, handlers, AIDL stubs
4. decx code class-source "<ServiceClass>" -P <port>
   -> inspect Binder / Messenger / Intent handling logic
5. Confirm whether the service enforces:
   -> manifest permission
   -> Binder caller validation
   -> package/signature allowlists
   -> action and parameter allowlists
```

## Key Trace Patterns

- Exported service returning a binder with no caller validation
- `onStartCommand()` switching on attacker-controlled actions
- Shell or file operations built from extras
- Messenger handlers trusting `msg.what`, `replyTo`, or Bundle contents
- Service-created `PendingIntent` objects that stay mutable

## Related

[[app-intent]]
[[risk-rating]]
