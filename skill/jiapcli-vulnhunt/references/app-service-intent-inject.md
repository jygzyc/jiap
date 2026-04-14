# Intent 命令注入

导出的 Service 在 `onHandleIntent()` 或 `onStartCommand()` 中处理外部 Intent 数据，未校验命令内容。

**Risk: HIGH**


## 利用前提

独立可利用。Service 导出 + `onStartCommand()` 从 Intent extras 获取参数执行操作（如启动 Activity、发送广播、文件操作）但未校验来源。攻击者直接 `startService()` 传递恶意参数。

**Android 版本范围：Android 13 及以下可利用** — Android 12 (API 31) 引入前台服务启动限制，后台应用启动前台服务受限。Android 14 (API 34) 进一步限制从后台启动 Activity。Android 15 (API 35) 收紧 SYSTEM_ALERT_WINDOW 权限的前台服务启动豁免。


## 攻击流程

```
1. jiap ard exported-components → 定位导出 Service
2. jiap code class-source <ServiceClass> → 检查 onStartCommand/onHandleIntent
3. jiap code xref-method "package.Class.onHandleIntent(...):void" → 追踪 onHandleIntent 调用
4. jiap code xref-method "package.Class.onStartCommand(...):int" → 追踪 onStartCommand 调用
5. 从 class-source 中定位 onHandleIntent/onStartCommand 方法，检查 Intent extra 参数处理逻辑
6. 识别从 Intent extras 提取命令参数的模式
7. 分析 switch/if 分支中的敏感操作（文件删除、上传、执行）
8. adb shell am startservice -n <component> --es action "delete" --es target "/data/..."
9. 触发恶意操作
```


## 关键特征与代码

- `onHandleIntent()` / `onStartCommand()` 直接从 Intent 提取操作指令，命令参数来自外部可控的 extras，未校验调用者身份
- **Shell 命令注入**：Service 接收外部参数用于构建系统命令（`Runtime.exec()`），未对输入进行白名单过滤，攻击者注入 `;`、`|`、`&` 等 Shell 元字符
- **Intent 重定向**：Service 接收外部 Intent 并将其转发（startActivity/startService/sendBroadcast），参数来源于外部 Intent

### 模式 1：Intent 命令分派

```java
// 漏洞：直接执行外部传入的命令
@Override
protected void onHandleIntent(Intent intent) {
    String action = intent.getStringExtra("action");
    String target = intent.getStringExtra("target");

    switch (action) {
        case "delete":
            deleteFile(target); // 可删除任意文件
            break;
        case "upload":
            uploadData(target); // 可上传到任意 URL
            break;
    }
}
```

### 模式 2：Shell 命令注入

```java
// 漏洞：拼接外部参数构建 Shell 命令
@Override
public int onStartCommand(Intent intent, int flags, int startId) {
    String domain = intent.getStringExtra("domain");
    // 未过滤输入，攻击者可注入 "; cat /data/data/com.app/databases/db"
    String cmd = "ping -c 1 " + domain;
    Runtime.getRuntime().exec(cmd);
    return START_NOT_STICKY;
}
```

### 模式 3：Intent 重定向

```java
// 漏洞：转发外部可控的 Intent
@Override
protected void onHandleIntent(Intent intent) {
    Intent target = intent.getParcelableExtra("target");
    // 危险：转发外部可控的 Intent，可能启动私有 Activity 或授予 URI 权限
    startActivity(target);
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2019-2114** | Android NFC 服务接受外部 Intent 安装 APK，绕过用户确认 |
| **CVE-2020-0391** | 系统服务接受未验证的 Intent 参数执行特权操作 |
| **CVE-2023-21008** | Wi-Fi Supplicant Service 的 setWpsDeviceType 接口未校验数组长度，导致越界读写（Native 层代码执行） |
| **文件管理 APP** | 导出 IntentService 接受 delete/move 命令，可删除任意文件 |


## 安全写法

```java
// 通过 manifest 权限 + 白名单 action 保护
// AndroidManifest.xml 中为 Service 声明自定义权限：
// <service android:name=".ExportedService"
//          android:exported="true"
//          android:permission="com.app.PERMISSION_USE_SERVICE">
//     <intent-filter>
//         <action android:name="com.app.ExportedService" />
//     </intent-filter>
// </service>
// 权限定义（protectionLevel="signature" 仅限同签名应用）：
// <permission android:name="com.app.PERMISSION_USE_SERVICE"
//             android:protectionLevel="signature" />

@Override
protected void onHandleIntent(Intent intent) {
    // onStartCommand() 通过 Binder IPC 调用，Binder.getCallingUid() 返回调用方应用的 UID
    // 可在代码中校验 Binder.getCallingUid()，同时 manifest 的 android:permission 属性
    // 保护 bindService() 和 startService() 两种调用方式

    String action = intent.getStringExtra("action");
    // 白名单校验 action
    if (!VALID_ACTIONS.contains(action)) {
        return;
    }

    // 安全处理
    switch (action) {
        case "delete":
            String target = intent.getStringExtra("target");
            // 校验目标路径在允许范围内
            if (isInAllowedDirectory(target)) {
                deleteFile(new File(target));
            }
            break;
    }
}

private static final Set<String> VALID_ACTIONS = Set.of("delete", "upload");

// Shell 命令安全写法：使用 ProcessBuilder 数组形式，避免 Shell 解释器解析
// 不安全：Runtime.getRuntime().exec("ping -c 1 " + domain)
// 安全：
new ProcessBuilder("ping", "-c", "1", domain).start();
// 或使用正则白名单限制输入字符（仅允许字母、数字、点、横线）
if (!domain.matches("^[a-zA-Z0-9.\\-]+$")) {
    throw new SecurityException("Invalid domain");
}

// Intent 重定向安全写法：使用显式 Intent，校验目标包名
// 不安全：startActivity(target)  // target 来自外部
// 安全：
Intent safe = new Intent();
safe.setComponent(new ComponentName(getPackageName(), "com.app.InternalActivity"));
// 或校验目标
// if (target.getComponent() != null
//     && target.getComponent().getPackageName().equals(getPackageName())) {
//     startActivity(target);
// }
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Broadcast 动态接收器 | 广播接收器收到命令后转发给导出 Service | → [[app-broadcast-dynamic-abuse]] |
| + Activity Intent 重定向 | 重定向到非导出 Service，绕过访问控制 | → [[app-activity-intent-redirect]] |
| + PendingIntent 提权 | Service 创建 PendingIntent 时使用外部可控 Intent | → [[app-intent-pendingintent-escalation]] |
| + 路径遍历 | Service 处理文件路径参数未规范化，随意文件读写 | → [[app-activity-path-traversal]] |
| + 权限提升 | Service 持有敏感权限但未校验调用者，恶意应用绕过自身权限限制 | → [[app-service-bind-escalation]] |
| + 隐式 Intent 劫持 | Service 使用隐式 Intent 启动，攻击者注册高优先级同名 Action 拦截 | → [[app-intent-implicit-hijack]] |


## Related

- [[app-broadcast-ordered-hijack]]
- [[app-broadcast-permission-bypass]]
- [[app-intent-implicit-hijack]]
- [[app-service-foreground-leak]]

- [[app-activity-intent-redirect]]
- [[app-broadcast-dynamic-abuse]]
- [[app-intent]]
- [[app-service]]
