---
name: poc-framework-service
description: Framework-service PoC reference covering clearCallingIdentity misuse, missing permission enforcement, identity confusion, intent redirect, data exposure, and race conditions.
---

# Framework Service PoC Reference

Use this reference for Binder calls into `system_server` or another privileged framework process. Framework-service PoCs almost always fit `binder-caller` mode.

## Hidden API Rule

Framework-service PoCs usually need hidden API access for `ServiceManager` and related Binder plumbing.

```java
import org.lsposed.hiddenapibypass.HiddenApiBypass;

HiddenApiBypass.addHiddenApiExemptions("");
```

Use hidden API access only for framework paths. Do not mix it into ordinary app-component PoCs.

## Construction Matrix

| Vulnerability | Exploit mode | Typical support | Visible success |
|---------------|--------------|-----------------|-----------------|
| `clearCallingIdentity()` misuse | `binder-caller` | hidden API access | privileged work runs after attacker-triggered call |
| missing permission enforcement | `binder-caller` | hidden API access | privileged Binder method succeeds without required permission |
| identity confusion | `binder-caller` | hidden API access | attacker can act for another user or caller identity |
| intent redirect | `binder-caller` | hidden API access | privileged service forwards attacker-controlled Intent |
| data exposure | `binder-caller` | hidden API access | privileged data is returned |
| race condition | `binder-caller` | hidden API access plus concurrency helper | repeated concurrent calls trigger inconsistent state |

## Pattern 1 - Single Binder Caller

Use for permission-missing, identity-confusion, intent-redirect, and data-leak cases.

```java
public final class FrameworkPermissionMissingExploit extends Exploit {
    public FrameworkPermissionMissingExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        try {
            HiddenApiBypass.addHiddenApiExemptions("");
            Class<?> smClass = Class.forName("android.os.ServiceManager");
            Method getService = HiddenApiBypass.getDeclaredMethod(smClass, "getService", String.class);
            IBinder binder = (IBinder) getService.invoke(null, "vulnerable_service");
            log("Obtained Binder for vulnerable_service. Replace this placeholder with the verified privileged method call.");
        } catch (Exception e) {
            log("Framework Binder setup failed: " + e.getMessage());
        }
    }
}
```

Fill with:

- real framework service name
- real interface wrapper or Stub conversion
- one verified vulnerable method only

Success signal:

- actual returned privileged data
- actual privileged state change
- actual privileged component launch

## Pattern 2 - Concurrency Driver

Use for race-condition findings.

```java
public final class FrameworkRaceConditionExploit extends Exploit {
    public FrameworkRaceConditionExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        int threadCount = 10;
        ExecutorService executor = Executors.newFixedThreadPool(threadCount);
        CountDownLatch latch = new CountDownLatch(threadCount);

        for (int i = 0; i < threadCount; i++) {
            executor.submit(() -> {
                try {
                    HiddenApiBypass.addHiddenApiExemptions("");
                    log("Send one concurrent Binder request here");
                } finally {
                    latch.countDown();
                }
            });
        }
    }
}
```

Use when:

- the verified finding depends on a timing window

Do not use the concurrency driver for ordinary single-call authorization bugs.

## Construction Notes

- Framework-service PoCs may still require special device state, root, or privileged test conditions
- Keep the exploit focused on proving the verified path, not enumerating the whole service
- If a required interface wrapper is not yet reconstructed, do not overclaim `build-ready`
