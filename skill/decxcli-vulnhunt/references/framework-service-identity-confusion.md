# 身份混淆（Identity Confusion）

系统服务信任调用者传递的 UID/PID 参数，而非使用 `Binder.getCallingUid()` 获取真实调用者身份。

**Risk: HIGH → CRITICAL**

跨用户操作或以 system 身份执行。可访问其他用户的数据、修改其他用户的配置。


## 利用前提

需要前置条件。系统服务信任调用者传入的 userId 参数而非使用 `Binder.getCallingUid()` 获取真实身份。攻击者需要能调用该服务接口并传入伪造的 userId。如果服务正确使用 `UserHandle.getCallingUserId()` 则不受影响。

**Android 版本范围：视具体系统服务而定** — 取决于哪个系统服务信任传入的 userId 参数而非使用 Binder.getCallingUid()。AI 扫描时应标注发现的 Android 版本和具体服务。


## 攻击流程

```
1. decx ard system-service-impl <Interface> → 定位系统服务实现
2. decx code class-source <ServiceImpl> → 获取实现类源码
3. 在源码中定位身份验证相关 API：
   → 搜索 Binder.getCallingUid, Binder.getCallingPid, getCallingUid
4. decx code xref-method "<package.Class.processForUser(...)>" → 对特定方法进行交叉引用追踪
5. decx code xref-method "<package.Class.handleIncomingUser(...)>" → 追踪 handleIncomingUser 调用
6. decx code class-source <ServiceImpl> → 搜索接受 userId/uid 参数的方法
7. 检查是否使用 Binder.getCallingUid() 校验
8. 确认机器是否为多用户设备（工作 profile、餞宽模式）
9. 构造 Binder 调用传入伪造 userId 访问其他用户数据
```


## 关键特征与代码

- 方法参数包含 `int userId` 或 `int uid`，直接使用而非校验是否与 `Binder.getCallingUid()` 匹配
- 使用 `Binder.getCallingUid()` 但未正确映射到 userId（如：在 `clearCallingIdentity()` 后使用缓存的 UID）

```java
// 漏洞：信任调用者传入的 userId
public void deleteUserFile(int userId, String filename) {
    // 未校验 userId 是否与 Binder.getCallingUid() 匹配
    File file = new File(getUserDir(userId), filename);
    file.delete();
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2020-0108** | system_server 中多个服务信任传入的 userId，普通应用可跨用户操作 |
| **CVE-2021-0314** | 系统服务未校验 userId 参数，可访问工作 profile 数据 |
| **CVE-2023-21126** | ActivityManagerService 身份混淆，普通 APP 以系统身份执行操作 |


## 安全写法

```java
// ✅ 正确：用 Binder API 获取真实调用者身份
public void deleteUserFile(int userId, String filename) {
    int callingUid = Binder.getCallingUid();
    int callingUserId = UserHandle.getUserId(callingUid);
    if (callingUserId != userId) {
        getContext().enforceCallingPermission(
            "android.permission.INTERACT_ACROSS_USERS",
            "Caller uid=" + callingUid + " requests access to userId=" + userId);
    }
    File file = new File(getUserDir(userId), filename);
    file.delete();
}

// ✅ 正确：clearCallingIdentity 后不信任缓存的 UID
public void performAsSystem(int userId, Runnable action) {
    long token = Binder.clearCallingIdentity();
    try {
        // clearCallingIdentity 之后，Binder.getCallingUid() 返回 Process.myUid()
        // 此时应使用显式传入且已校验的 userId，而非重新调用 Binder API
        action.run();
    } finally {
        Binder.restoreCallingIdentity(token);
    }
}

// ✅ 正确：服务端完全不接受调用者传入的 userId
public UserInfo getUserInfo() {
    int callingUid = Binder.getCallingUid();
    int callingUserId = UserHandle.getUserId(callingUid);
    return mUserManager.getUserInfo(callingUserId);
}
```

**核心原则：** 服务端必须通过 `Binder.getCallingUid()` / `UserHandle.getCallingUserId()` 获取调用者真实身份，绝不信任调用者传入的 `userId`/`uid` 参数。如业务需要跨用户操作，必须通过 `enforceCallingPermission` 校验 `INTERACT_ACROSS_USERS` 等权限。


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + 权限检查缺失 | 伪造身份调用无权限检查的方法，双重绕过 | → [[framework-service-permission-missing]] |
| + 绑定服务越权 | 绑定系统服务后传入伪造 userId，跨用户操作 | → [[app-service-bind-escalation]] |
| + clearIdentity 滥用 | clearIdentity 后使用缓存 UID 导致身份判断错误 | → [[framework-service-clear-identity]] |
| + 数据泄露 | 伪造 userId 获取其他用户的敏感数据 | → [[framework-service-data-leak]] |


## Related

- [[app-service-aidl-expose]]
- [[framework-service-race-condition]]

- [[app-service-bind-escalation]]
- [[framework-service]]
- [[framework-service-permission-missing]]
