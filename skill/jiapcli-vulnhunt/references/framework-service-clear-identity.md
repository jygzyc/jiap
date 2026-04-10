# clearCallingIdentity 滥用

`Binder.clearCallingIdentity()` 清除调用者身份信息，在恢复之前执行的代码以 `system` 身份运行。

**Risk: HIGH**

扩大了以 system 身份执行的代码范围，本应受调用者权限限制的操作被跳过。


## 利用前提

独立可利用。系统服务使用 `clearCallingIdentity()` 清除调用者身份后执行操作，但操作范围过大或未正确 restore。任何应用通过 Binder 触发该服务接口即可获得清除身份后的权限提升效果。

**Android 版本范围：Android 13 及以下可利用** — Android 14 (API 34) 引入 `Binder.withCleanCallingIdentity()` (API 34)，提供受限范围的 clearCallingIdentity 替代方案，减少滥用风险。


## 攻击流程

```
1. jiap ard system-service-impl <Interface> → 定位系统服务实现
2. jiap code class-source <ServiceImpl> → 获取实现类源码
3. jiap code xref-method "android.os.Binder.clearCallingIdentity()" → 追踪 clearCallingIdentity 调用
4. jiap code xref-method "android.os.Binder.restoreCallingIdentity(...)" → 追踪 restoreCallingIdentity 调用
5. 分析 clear 和 restore 之间的代码范围
6. 识别 clear 块内的敏感操作（文件读写、权限授予、包管理）
7. 检查 clear 之前是否缺少 enforceCallingPermission
8. 构造 Binder 调用触发目标服务接口
```


## 关键特征

- `clearCallingIdentity()` 和 `restoreCallingIdentity()` 之间的代码块以 system 身份执行
- 在 clear/restore 之间执行的操作本应受调用者权限限制
- clear 后未在正确的时机 restore


## 代码模式

```java
// 漏洞：clearCallingIdentity 范围过大
public void performOperation(String action) {
    // 清除身份
    long token = Binder.clearCallingIdentity();
    try {
        // 这段代码以 system 身份执行
        // 本应检查调用者权限的操作被跳过
        if (action.equals("delete_all")) {
            deleteAllUserData(); // 任何调用者可触发
        }
    } finally {
        Binder.restoreCallingIdentity(token);
    }
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2021-0313** | 系统服务 clearCallingIdentity 后执行了未校验的操作，导致本地提权 |
| **CVE-2022-20127** | NFC 服务 clearCallingIdentity 范围过大，允许普通应用以 system 身份执行操作 |
| **LaunchAnyWhere** | AccountManagerService clearIdentity 后启动任意 Activity，以 system 身份绕过导出限制 |


## 安全写法

```java
// 1. 缩小 clearCallingIdentity 范围到最小必要操作
// 2. API 34+ 使用 Binder.withCleanCallingIdentity()
// 3. 始终使用 try-finally 保证 restore

public void performOperation(String action) {
    // 在 clear 之前先做权限校验
    enforceCallingPermission(android.Manifest.permission.MANAGE_USERS, "Need MANAGE_USERS");

    // 方案一：API 34+ 使用 withCleanCallingIdentity（自动 restore）
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
        Binder.withCleanCallingIdentity(() -> {
            // 仅执行必须以 system 身份的操作
            doPrivilegedOperation(action);
        });
    } else {
        // 方案二：手动 clear/restore，范围最小化
        long token = Binder.clearCallingIdentity();
        try {
            doPrivilegedOperation(action);
        } finally {
            Binder.restoreCallingIdentity(token);
        }
    }
}

// 非 system 身份的操作放在 clear/restore 之外
private void doPrivilegedOperation(String action) {
    // 仅包含必须以 system 身份执行的代码
    // 不要在此做业务逻辑判断
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + 权限检查缺失 | clear 后调用其他无权限检查的服务方法，以 system 身份执行 | → [[framework-service-permission-missing]] |
| + 身份混淆 | clearIdentity 后使用缓存的 UID 导致身份判断错误 | → [[framework-service-identity-confusion]] |
| + Intent 重定向 | 以 system 身份启动任意组件，绕过导出限制 | → [[framework-service-intent-redirect]] |
| + 竞态条件 | clearIdentity 范围内存在竞态，操作身份混乱 | → [[framework-service-race-condition]] |


## Related

- [[app-activity]]
- [[framework-service]]
- [[framework-service-permission-missing]]

