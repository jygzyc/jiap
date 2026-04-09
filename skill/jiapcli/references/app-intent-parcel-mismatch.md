# Parcel 反序列化不匹配

利用 `Bundle` 序列化/反序列化过程中的读写不对称，构造恶意 Parcel 数据绕过安全校验（如 `checkKeyIntent()`），实现权限绕过。

**Risk: HIGH**


## 利用前提

需要前置条件。系统服务中存在 Bundle 校验→启动 Intent 的链路，且自定义 Parcelable 的 `writeToParcel`/`createFromParcel` 存在读写不对称。攻击者通过 AIDL 或导出组件传入恶意 Bundle 绕过 `checkKeyIntent` 校验。

**Android 版本范围：Android 10 ~ 11 可利用** — Android 12+ 修复了多个 Bundle Mismatch 问题，checkKeyIntent 校验大幅加强。


## 攻击流程

```
1. jiap code xref-method "com.android.server.am.ActivityManagerService.checkKeyIntent(...)" → 搜索 Bundle 安全校验
2. jiap code xref-method "android.content.Intent.filterEquals(...)" → 追踪 filterEquals 调用
3. jiap code xref-method "android.content.Intent.filterHasType(...)" → 追踪 filterHasType 调用
4. jiap code implement android.os.Parcelable$Creator → 搜索 Bundle 的 Parcelable 实现
5. jiap code xref-method "android.os.Parcel.writeBundle(...)" → 追踪 writeBundle 调用
6. jiap code class-source <Class> → 获取源码，分析 Bundle 读写路径
7. jiap code implement android.os.Parcelable$Creator → 枚举所有 Parcelable 实现
8. 对比 writeToParcel / createFromParcel 字段顺序和类型是否对称
9. 发现读写不对称的 Parcelable 后，构造恶意 Bundle
10. Bundle 在系统服务中经过 checkKeyIntent 校验时，利用偏移绕过校验
11. 校验通过后，系统服务以 system 权限启动被篡改的 Intent
```


## 关键特征

- 使用 `checkKeyIntent()` / `checkCallingPackage()` 等校验 Intent 安全性
- 校验通过 `Intent.filterEquals()` 或类型检查进行，但实际 Bundle 中包含额外恶意键值
- 自定义 Parcelable 实现中 `writeToParcel` 和 `CREATOR.createFromParcel` 字段顺序不一致
- `Intent.getExtras()` 返回的 Bundle 与原始 Bundle 内容不同（读写不对称）


## 代码模式

```java
// Bundle 读写不对称原理
// 写入时（正常流程）：
Bundle bundle = new Bundle();
bundle.putParcelable("intent", safeIntent);
bundle.putParcelable("extra_intent", maliciousIntent); // ⚠️ 额外的恶意 Intent
// writeToParcel 按内部 Map 顺序写入所有键值

// 读取时（系统服务校验）：
Intent intent = bundle.getParcelable("intent");
checkKeyIntent(intent); // ✅ 校验通过，safeIntent 是安全的
// 但 bundle 中还有 extra_intent 未被校验

// 攻击者利用 Parcel 偏移构造恶意 Bundle：
// 使 readFromParcel 时 "intent" 读取到 safeIntent（通过校验），
// 但后续读取到恶意数据（被代码信任）

// 经典利用：LaunchAnyWhere 绕过
// AccountManagerService.checkKeyIntent() 校验通过后，
// Settings 以 system 权限启动了被篡改的 Intent
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **LaunchAnyWhere (Bug 7699048)** | AccountManager Bundle Mismatch 绕过 checkKeyIntent，以 system 权限启动任意 Activity |
| **CVE-2023-20963** | WorkSource Parcelable 读写不对称 + Settings root-path FileProvider，实现 system 代码执行 (in-the-wild) |
| **CVE-2024-40676** | content URI 的 getType() 在不同时间返回不同 MIME 类型，绕过 checkKeyIntent 类型检查 |
| **CVE-2017-13286** | OutputConfiguration Parcelable mismatch 导致 system_server 任意组件启动 |
| **CVE-2017-13288** | PeriodicAdvertisingReport Parcelable mismatch 绕过权限校验 |


## 安全写法

```java
// 白名单过滤 Bundle 键，拒绝未预期的键值
public Bundle sanitizeBundle(Bundle input) {
    Bundle safe = new Bundle();
    Set<String> allowedKeys = Set.of("key1", "key2");
    for (String key : input.keySet()) {
        if (allowedKeys.contains(key)) {
            safe.putParcelable(key, input.getParcelable(key));
        }
    }
    return safe;
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Service AIDL | 通过 AIDL 发送恶意 Bundle 到系统服务，绕过参数校验 | → [[app-service-aidl-expose]] |
| + Framework 权限绕过 | Bundle 偏移跳过 enforcePermission，以 system 身份执行操作 | → [[framework-service-permission-missing]] |
| + ClassLoader 注入 | Bundle 读写不对称导致 readSerializable 加载恶意类 | → [[app-intent-classloader-inject]] |


## Related

- [[app-provider-call-expose]]
- [[app-provider-fileprovider-misconfig]]
- [[app-provider-gettype-infoleak]]
- [[app-service-bind-escalation]]
- [[app-service-messenger-abuse]]
- [[framework-service-intent-redirect]]

- [[app-intent]]
- [[app-service]]
- [[app-service-aidl-expose]]
- [[framework-service-permission-missing]]
- [[app-intent-classloader-inject]]

