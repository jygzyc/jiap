# 权限检查缺失

系统服务方法缺少权限校验，或校验逻辑存在缺陷（如：先执行操作后检查权限、检查的权限级别不够）。

**Risk: CRITICAL**

可执行特权操作（设备管理、包安装、权限授予）。system_server 权限级别下无需用户交互，直接 CRITICAL。

## 利用前提

独立可利用。系统服务的方法未调用 `enforceCallingPermission()` 即执行特权操作。任何应用可通过 Binder 调用该接口获得系统权限级别的功能。这是 Android 系统漏洞的典型模式，危害极高。

**Android 版本范围：视具体系统服务而定** — 每个 Android 版本修复不同，AI 扫描时应标注发现的 Android 版本，不做通用版本范围假设。

## 攻击流程

```
1. decx ard system-service-impl <Interface> → 定位系统服务实现
2. decx code class-source <ServiceImpl> → 获取实现类源码
3. 在源码中定位权限检查相关 API：
   → 搜索 enforceCallingPermission, enforcePermission
   → 搜索 checkCallingPermission, checkPermission, getContext().enforce
4. 对比每个公共方法：敏感操作 ↔ enforceCallingPermission 是否配对
5. 检查权限检查位置（操作前 vs 操作后）
6. 检查是否使用 checkCallingOrSelfPermission 而非 checkCallingPermission
7. 构造 Binder 调用触发无权限检查的接口
```


## 关键特征与代码

- 方法处理敏感操作但未调用 `enforceCallingPermission`（权限检查缺失）
- 权限检查在操作执行之后（TOCTOU）
- 使用 `checkCallingOrSelfPermission` 而非 `checkCallingPermission`（前者会检查自身权限，可能绕过调用者校验）

```java
// 漏洞：缺少权限校验
public int getDeviceId() {
    // 敏感操作，未校验调用者权限
    return TelephonyManager.getDefault().getDeviceId();
}

// 漏洞：权限检查在操作之后
public void resetUserConfig(int userId) {
    // 先执行
    resetConfig(userId);
    // 后检查 — 操作已经完成
    getContext().enforceCallingPermission(
        "android.permission.RESET_USER_CONFIG", null);
}

// 漏洞：使用 checkCallingOrSelf
public String getPassword() {
    // 如果 system_server 自身拥有权限，调用者也会通过
    getContext().enforceCallingOrSelfPermission(
        "android.permission.GET_PASSWORD", null);
    return passwordStore.get(callingUid);
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2023-20963** | 系统服务权限检查被 Parcel Mismatch 绕过，实现任意代码执行 |
| **CVE-2021-0921** | system_server 中的服务方法缺少权限检查，普通应用提权 |
| **CVE-2023-21241** | Telecom 服务缺少权限检查，普通应用可拨打电话 |
| **CVE-2022-20435** | MediaProjection 服务缺少调用者校验，可截屏任意应用 |


## 安全写法

```java
// ✅ 正确：使用 enforceCallingPermission（权限不足时抛 SecurityException）
public int getDeviceId() {
    getContext().enforceCallingPermission(
        "android.permission.READ_PHONE_STATE",
        "getDeviceId requires READ_PHONE_STATE");
    return TelephonyManager.getDefault().getDeviceId();
}

// ✅ 正确：权限检查在操作之前
public void resetUserConfig(int userId) {
    getContext().enforceCallingPermission(
        "android.permission.RESET_USER_CONFIG",
        "resetUserConfig requires RESET_USER_CONFIG");
    resetConfig(userId);  // 操作在权限校验之后
}

// ✅ 正确：使用 enforceCallingPermission（仅检查调用者）
public String getPassword() {
    // enforceCallingPermission 只检查 Binder 调用者的权限，不会因为
    // system_server 自身拥有权限而放行任意调用者
    getContext().enforceCallingPermission(
        "android.permission.GET_PASSWORD",
        "getPassword requires GET_PASSWORD");
    int callingUid = Binder.getCallingUid();
    return passwordStore.get(callingUid);
}
```

**核心原则：** 系统服务必须使用 `enforceCallingPermission`（权限不足时直接抛异常中断），而非 `checkCallingPermission`（仅返回 boolean，容易被忽略或误用）。权限检查必须在敏感操作之前执行。

## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + PendingIntent | 以系统应用身份通过 PendingIntent 调用无权限检查的方法 | → [[app-intent-pendingintent-escalation]] |
| + 身份混淆 | 伪造 userId 调用缺失权限检查的方法，跨用户操作 | → [[framework-service-identity-confusion]] |
| + clearIdentity 滥用 | clearCallingIdentity 后执行无权限检查的特权操作 | → [[framework-service-clear-identity]] |
| + Intent 重定向 | 以系统权限转发 Intent 到非导出特权组件 | → [[framework-service-intent-redirect]] |


## Related

- [[app-activity-pendingintent-abuse]]
- [[app-intent-classloader-inject]]
- [[app-intent-parcel-mismatch]]
- [[framework-service-clear-identity]]
- [[framework-service-data-leak]]
- [[framework-service-race-condition]]

- [[app-intent-pendingintent-escalation]]
- [[framework-service]]
- [[framework-service-identity-confusion]]
