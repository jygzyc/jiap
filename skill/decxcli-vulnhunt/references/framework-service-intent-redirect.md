# 系统服务 Intent 重定向

系统服务作为中转传递 Intent，未校验 Intent 目标组件，攻击者可利用系统权限启动任意非导出特权组件。**Risk: HIGH**

与 [[app-activity-intent-redirect]] 的区别：发生在系统服务进程中，攻击者以系统身份执行操作，权限远高于普通应用。


## 利用前提

独立可利用。系统服务作为 Intent 中转，从外部获取 Intent 参数后直接启动。攻击者通过 AIDL 接口传入指向非导出特权组件的 Intent，以系统权限启动该组件。典型利用链：Settings → FileProvider root-path → 任意文件读写。

**Android 版本范围：Android 11 及以下可利用（典型利用链）** — Android 12+ 引入 intent 启动安全检查，LaunchAnyWhere 类攻击基本被修复。Android 14+ 限制隐式 intent 只能传送到导出组件。


## 攻击流程

```bash
# Step 1: 定位系统服务实现
decx ard system-service-impl <Interface>

# Step 2: 获取实现类源码
decx code class-source <ServiceImpl>

# Step 3: 对 Intent 相关方法进行交叉引用追踪
decx code xref-method "<ServiceImpl.startActivity(...)>"
decx code xref-method "<ServiceImpl.sendBroadcast(...)>"
decx code xref-method "android.content.Intent.fillIn(...)"

# Step 4: 检查 Intent 来源是否为外部 Binder 调用
# Step 5: 确认是否缺少 checkKeyIntent/Component 白名单校验
# Step 6: 通过 AIDL 接口传入指向非导出特权组件的 Intent
```


## 关键特征与代码

- 系统服务接收外部传入的 Intent（通过 Binder 调用），未调用 `checkKeyIntent()` 或未校验 Intent 的 Component/Package
- 直接使用传入的 Intent 调用 `startActivity()` / `sendBroadcast()` 等，系统服务持有 `INTERACT_ACROSS_USERS` 等高权限
- **服务劫持**变体：Service 使用隐式 Intent 启动（配置了 intent-filter 且未限制 exported），攻击者注册了更高优先级的同名 Action，拦截发往目标 Service 的 Intent

```java
// 漏洞：系统服务中转 Intent 未校验目标
public class NotificationService extends INotificationService.Stub {
    @Override
    public void notify(String tag, int id, Notification notification) {
        // 直接使用 notification.contentIntent，未校验目标
        if (notification.contentIntent != null) {
            notification.contentIntent.send();
            // 攻击者可构造 Notification 包含恶意 PendingIntent
            // 以系统身份启动任意组件
        }
    }
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2023-20963** | Parcel Mismatch + Settings Intent 重定向 + FileProvider root-path，完整提权链 |
| **CVE-2021-0653** | Settings 应用 Intent 重定向，通过系统服务中转启动任意非导出 Activity |
| **CVE-2022-20356** | 系统服务未校验 Intent 目标组件，可启动非导出设置页面泄露敏感信息 |
| **CVE-2020-0108** | system_server Intent 重定向启动任意组件，提权到 system |
| **LaunchAnyWhere** | 经典 Intent 重定向漏洞，通过 AccountManagerService 转发 Intent 启动任意 Activity |
| **CVE-2024-31317** | 通过 WRITE_SECURE_SETTINGS 权限修改设置，利用系统服务向 Zygote 进程注入命令，本质是参数传递过程中的命令注入/重定向 |


## 安全写法

```java
public class SecureNotificationService extends INotificationService.Stub {
    @Override
    public void notify(String tag, int id, Notification notification) {
        if (notification.contentIntent != null) {
            // 校验 PendingIntent 的目标 Intent
            Intent intent = notification.contentIntent.getIntent();
            // 方案 1：使用 checkKeyIntent 校验
            if (intent != null && !isAllowedPackage(intent.getPackage())) {
                Log.w(TAG, "Blocked PendingIntent to: " + intent.getComponent());
                return;
            }
            // 方案 2：限制 PendingIntent 的发送者
            notification.contentIntent.send();
        }
    }

    private boolean isAllowedPackage(String pkg) {
        return ALLOWED_PACKAGES.contains(pkg);
    }
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Activity Intent 重定向 | 系统服务转发恶意 Intent 到非导出的特权 Activity | → [[app-activity-intent-redirect]] |
| + PendingIntent 滥用 | 篡改系统服务传递的 PendingIntent，以 system 身份执行操作 | → [[app-intent-pendingintent-escalation]] |
| + Parcel 反序列化 | 通过 Parcel Mismatch 绕过 checkKeyIntent 检查 | → [[app-intent-parcel-mismatch]] |
| + FileProvider 配置错误 | 重定向到 Settings + root-path FileProvider 实现任意文件访问 | → [[app-provider-fileprovider-misconfig]] |
| + clearIdentity 滥用 | clearIdentity 后启动组件，以 system 身份绕过导出限制 | → [[framework-service-clear-identity]] |


## Related

- [[framework-service-data-leak]]

- [[app-activity]]
- [[app-activity-intent-redirect]]
- [[app-intent-pendingintent-escalation]]
- [[framework-service]]
