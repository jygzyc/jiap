# Intent 重定向（LaunchAnyWhere）

攻击者通过导出 Activity（或系统中间组件）传递恶意 Intent，由其转发到未导出的内部组件，绕过访问控制。这是 Android 历史上危害最大的漏洞类型之一。

**Risk: HIGH**

## 利用前提

独立可利用。导出 Activity 从外部 Intent 获取目标组件名并启动。攻击者直接构造 Intent 指定任意目标组件即可。危害程度取决于目标组件（非导出 Activity → 越权访问；系统组件 → 提权）。

**Android 版本范围：所有版本可利用** — 应用层逻辑漏洞。但 framework-service-intent-redirect 在 Android 12+ 因 checkKeyIntent 加强而大幅缓解。

## 攻击流程

```
1. jiap ard exported-components → 定位导出 Activity
2. jiap code class-source <Activity> → 检查是否从 Intent 读取嵌套 Intent
3. jiap code class-source <targetClass> → 搜索 getParcelableExtra 的调用
4. jiap code xref-method "package.Class.onCreate(android.os.Bundle):void" → 从生命周期追踪数据流
5. jiap code xref-method "com.android.server...checkKeyIntent(...)" → 系统服务 Intent 中转
6. jiap code xref-method "package.Class.startActivity(android.content.Intent):void" → 追踪转发调用
7. 构造恶意 Intent，设置目标组件为非导出敏感组件
8. adb shell am start -n com.target/.ExportedActivity --el forward_intent ...
9. 导出 Activity 转发 Intent，访问目标未导出组件
```

## 关键特征与代码

- 导出 Activity 调用 `getIntent().getParcelableExtra("key")` 提取嵌套 Intent，直接传递给 `startActivity()` / `startService()` / `sendBroadcast()`，缺少目标组件的白名单校验或签名校验
- **权限差异**：转发组件（受害者）通常拥有比攻击者更高的权限（如系统权限），或能访问攻击者无法直接访问的私有组件/资源
- **FLAG 授权**：重定向的 Intent 携带 `FLAG_GRANT_READ_URI_PERMISSION` 可窃取私有文件

```java
// 典型漏洞：从外部 Intent 提取嵌套 Intent 并转发
public class ExportedActivity extends Activity {
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Intent forwardIntent = getIntent().getParcelableExtra("forward_intent");
        startActivity(forwardIntent); // 攻击者可控制目标组件
    }
}

// 系统级变体：AccountManagerService 未校验 authenticator 返回的 Intent
// Bug 7699048 (LaunchAnyWhere)
public Bundle addAccount(String accountType, String authTokenType,
        String[] requiredFeatures, Bundle options) {
    // 调用恶意 authenticator 的 addAccount 方法
    Bundle result = authenticator.addAccount(response, accountType, ...);
    // result 中包含攻击者控制的 Intent，未做签名校验直接返回给 Settings
    Intent intent = result.getParcelable(AccountManager.KEY_INTENT);
    // Settings 以 system 权限启动该 Intent
}
```

## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **LaunchAnyWhere (Bug 7699048)** | 恶意 APP → AccountManager.addAccount → 恶意 authenticator 返回包含恶意 Intent 的 Bundle → Settings 以 system 权限启动 Intent → 访问任意未导出组件 |
| **Bundle Mismatch** | 利用 `writeToParcel`/`createFromParcel` 读写不对称，构造恶意 Bundle 绕过 `checkKeyIntent` 检查，注入额外键值对 |
| **CVE-2024-40676** | content URI 的 `getType()` 在不同时间返回不同 MIME 类型 → 两次隐式 Intent 解析结果不一致 → 绕过 `checkKeyIntent` 的类型检查 |
| **CVE-2022-20550** | 特权应用接收不可信的 ComponentName 并直接创建 Intent 启动，导致越权访问 |
| **CVE-2024-0015** | 特权应用接受不可信输入构造 Intent 启动组件，绕过访问控制 |

## 安全写法

```java
// 白名单校验目标组件 + 源校验 + 清除敏感 Flags
private static final Set<String> ALLOWED_TARGETS = Set.of(
    "com.app.TargetActivity1",
    "com.app.TargetActivity2"
);

@Override
protected void onCreate(Bundle savedInstanceState) {
    super.onCreate(savedInstanceState);
    Intent forwardIntent = getIntent().getParcelableExtra("forward_intent");
    if (forwardIntent == null) return;

    // 校验来源：使用 getCallingActivity() 或 Binder.getCallingUid() 验证调用者
    ComponentName caller = getCallingActivity();
    if (caller == null || !caller.getPackageName().equals(getPackageName())) {
        Log.w(TAG, "Blocked redirect from external caller");
        return;
    }

    // 校验目标：使用 resolveActivity() 检查目标组件
    ComponentName target = forwardIntent.getComponent();
    if (target == null || !ALLOWED_TARGETS.contains(target.getClassName())) {
        Log.w(TAG, "Blocked redirect to: " + target);
        return;
    }

    // 清除敏感 Flags
    forwardIntent.setFlags(forwardIntent.getFlags()
        & ~Intent.FLAG_GRANT_READ_URI_PERMISSION
        & ~Intent.FLAG_GRANT_WRITE_URI_PERMISSION);

    startActivity(forwardIntent);
}
```

## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Service 命令注入 | 重定向到非导出 Service，通过 `onHandleIntent` 执行任意操作 | → [[app-service-intent-inject]] |
| + WebView 加载 | 重定向到内部 WebView Activity，加载 file:// 读取私有文件 | → [[app-webview-file-access]] |
| + PendingIntent 填充 | 重定向 Intent 包含 PendingIntent，利用受害者身份执行操作 | → [[app-intent-pendingintent-escalation]] |
| + Broadcast 权限绕过 | 转发 Intent 到受保护的 BroadcastReceiver，绕过权限隔离 | → [[app-broadcast-permission-bypass]] |
| + 组件导出越权 | 重定向到导出敏感 Activity，绕过内部访问控制 | → [[app-activity-exported-access]] |
| + Fragment 注入 | 重定向到导出 Activity 加载攻击者指定的 Fragment | → [[app-activity-fragment-injection]] |
| + 路径遍历 | 重定向到文件操作 Activity，通过路径遍历读取任意文件 | → [[app-activity-path-traversal]] |
| + setResult 泄露 | 重定向触发目标 Activity，截获通过 setResult 返回的敏感数据 | → [[app-activity-setresult-leak]] |
| + PendingIntent 滥用 | PendingIntent 指向非导出 Activity，以受害者身份绕过控制 | → [[app-activity-pendingintent-abuse]] |
| + FileProvider 组合 | 重定向携带 FLAG_GRANT_READ_URI_PERMISSION 到导出 Activity，通过 setResult 返回给攻击者授予私有文件访问权 | → [[app-provider-fileprovider-misconfig]] |

## Related

- [[app-broadcast-ordered-hijack]]
- [[app-provider-fileprovider-misconfig]]
- [[app-provider-gettype-infoleak]]
- [[app-provider-sql-injection]]
- [[app-service-aidl-expose]]
- [[app-service-bind-escalation]]
- [[app-service-messenger-abuse]]
- [[app-webview-intent-scheme]]
- [[framework-service-intent-redirect]]

- [[app-activity]]
- [[app-activity-exported-access]]
- [[app-activity-fragment-injection]]
- [[app-activity-path-traversal]]
- [[app-activity-pendingintent-abuse]]
- [[app-activity-setresult-leak]]
- [[app-broadcast-permission-bypass]]
- [[app-intent]]
- [[app-intent-pendingintent-escalation]]
- [[app-service]]
- [[app-service-intent-inject]]
- [[app-webview-file-access]]
