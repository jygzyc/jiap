# AIDL 接口暴露

通过 AIDL 定义的接口在导出 Service 中暴露，攻击者可绑定服务并调用任意接口方法。

**Risk: HIGH**


## 利用前提

独立可利用。Service 为 `exported="true"` 且 AIDL 接口方法无权限校验。攻击者通过 `bindService()` 获取 Binder 后直接调用接口方法。危害取决于暴露接口的功能。

**Android 版本范围：所有版本可利用** — 应用层配置问题。


## 攻击流程

```
1. jiap ard exported-components → 定位导出 Service
2. jiap code class-source <ServiceClass> → 检查 onBind() 返回的 Binder/Stub
3. 在 class-source 中查看 IMyService.Stub 匿名类实现（AIDL Stub 实现）
4. jiap code implement <AIDL_Interface> → 查找 AIDL 接口的所有实现
5. jiap code xref-method "package.Class.onBind(...):IBinder" → 检查权限校验缺失
6. 分析每个接口方法的功能（数据读取、文件操作、命令执行）
7. 检查是否存在 Binder.getCallingUid() 校验
8. 编写攻击应用 bindService() 调用敏感接口
```


## 关键特征与代码

- Service 导出（`android:exported="true"`）且 `onBind()` 返回 AIDL 接口的 `Stub` 实现，接口方法未做调用者身份校验（`Binder.getCallingUid()`）
- 接口返回敏感数据（用户信息、Token、Cookie、配置文件）且未根据调用者身份做数据脱敏，泄露的信息可用于后续账号劫持

```java
// 漏洞：导出 Service 暴露 AIDL 接口，无权限校验
public class ExportedService extends Service {
    private final IMyService.Stub binder = new IMyService.Stub() {
        @Override
        public String getToken(String userId) {
            // 任何应用可调用，返回敏感 Token
            return TokenManager.getToken(userId);
        }

        @Override
        public void executeAction(String action, Bundle params) {
            // 任何应用可触发操作
            ActionProcessor.process(action, params);
        }
    };

    @Override
    public IBinder onBind(Intent intent) {
        return binder; // 无条件返回
    }
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2022-20007** | Android 系统服务 AIDL 接口未校验调用者，允许普通应用执行特权操作 |
| **CVE-2021-0313** | 系统服务 Binder 接口处理异常输入导致 DoS |
| **CVE-2024-43090** | Keyboard shortcuts helper 通过 AIDL 接口泄露 Icon 对象 |
| **第三方支付 SDK** | 导出服务暴露订单查询接口，无权限校验可查询所有订单 |


## 安全写法

```java
@Override
public IBinder onBind(Intent intent) {
    // 校验调用者
    int callingUid = Binder.getCallingUid();
    if (!isTrustedCaller(callingUid)) {
        return null;
    }
    return binder;
}

// 或在接口方法中校验（Stub 内部类通过外部 Service 引用访问 Context）
@Override
public String getToken(String userId) {
    ExportedService.this.enforceCallingPermission("com.app.PERMISSION_GET_TOKEN", null);
    return TokenManager.getToken(userId);
}

// 数据脱敏：根据调用者身份返回最小必要数据
@Override
public Bundle getUserInfo(String userId) {
    int callingUid = Binder.getCallingUid();
    enforceCallingPermission("com.app.PERMISSION_READ_USER", null);

    Bundle info = new Bundle();
    if (isSameUser(callingUid, userId)) {
        // 同一用户：返回完整信息
        info.putAll(getFullUserInfo(userId));
    } else {
        // 跨用户：仅返回非敏感信息
        info.putString("displayName", getDisplayName(userId));
        // 不返回 Token、Cookie 等敏感字段
    }
    return info;
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Parcel 反序列化 | 通过 AIDL 发送恶意 Parcelable 绕过参数校验 | → [[app-intent-parcel-mismatch]] |
| + Activity Intent 重定向 | AIDL 接口返回的数据用于构造重定向 Intent | → [[app-activity-intent-redirect]] |
| + Messenger 滥用 | AIDL 接口和 Messenger 同时暴露，多路径攻击 | → [[app-service-messenger-abuse]] |
| + Framework 身份混淆 | AIDL 接口受理请求时未检查 userId 参数 | → [[framework-service-identity-confusion]] |
| + 账号劫持 | 泄露的 Token/Cookie 用于伪造请求，接管用户账号 | → [[framework-service-data-leak]] |


## Related

- [[app-intent-classloader-inject]]
- [[app-service-bind-escalation]]

- [[app-activity-intent-redirect]]
- [[app-intent]]
- [[app-intent-parcel-mismatch]]
- [[app-service]]
