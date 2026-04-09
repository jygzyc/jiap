# PendingIntent 权限提升

**Risk: HIGH**


## 利用前提

需要前置条件。目标应用创建 PendingIntent 时使用 `FLAG_MUTABLE`，且填充了外部可控的数据（extras）。攻击者修改 extras 后触发 PendingIntent，以目标应用身份执行操作。如果使用 `FLAG_IMMUTABLE` 则不可利用。

应用创建 PendingIntent 后传递给其他应用（如通知管理器、AlarmManager），接收方可以该应用的身份执行操作。

**Android 版本范围：Android 10 ~ 13 可利用** — Android 10 (API 29) 默认创建可变 PendingIntent。Android 12 (API 31) 强制要求 PendingIntent 必须指定 FLAG_IMMUTABLE/FLAG_MUTABLE，但显式使用 FLAG_MUTABLE 的应用仍可被利用。Android 14 (API 34) 进一步限制：未指定组件的可变 PendingIntent 抛出异常。


## 攻击流程

```
1. jiap code xref-method "android.app.PendingIntent.getActivity(...)" → 定位 PendingIntent 创建点
2. jiap code xref-method "android.app.PendingIntent.getService(...)" → 追踪 getService
3. jiap code xref-method "android.app.PendingIntent.getBroadcast(...)" → 追踪 getBroadcast
4. jiap code xref-method "android.app.PendingIntent.send()" → 追踪 send 调用
5. 在反编译源码中搜索 flag 值：0x2000000（MUTABLE）或 0x4000000（IMMUTABLE）
6. 检查 base Intent 是否为空 / 未指定目标组件
7. 恶意应用获取 PendingIntent 后修改 action/component/extras
8. 调用 send() 以受害应用身份执行操作
```


## 关键特征

- 使用 `PendingIntent.getActivity()`、`getService()`、`getBroadcast()` 创建
- base Intent 未指定目标组件（空 action 或空 component）
- flag 含 `0x2000000`（FLAG_MUTABLE）而非 `0x4000000`（FLAG_IMMUTABLE）


## 代码模式

```java
// 漏洞：空 Intent + FLAG_MUTABLE
Intent intent = new Intent(); // 空 Intent
PendingIntent pi = PendingIntent.getActivity(this, 0, intent,
    0xa000000);  // 0x8000000|0x2000000 = FLAG_UPDATE_CURRENT|FLAG_MUTABLE
// 攻击者获取 pi 后，使用 Intent.fillIn() 填充空字段
// Intent fillIntent = new Intent();
// fillIntent.setAction("android.intent.action.MASTER_CLEAR");
// pi.send(context, 0, fillIntent);
// → 以受害者身份执行恢复出厂设置
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2025-43977** | 韩国 SK Telecom 导出 Activity 接收 PendingIntent，以受害者身份拨打电话 |
| **CVE-2023-47889** | 通过导出 Activity 传递 PendingIntent 实现远程关机 |
| **CVE-2020-0389** | Android AccessibilityService 创建 FLAG_MUTABLE PendingIntent，攻击者填充恶意 Intent |


## 安全写法

```java
// FLAG_IMMUTABLE + 显式指定目标组件
Intent intent = new Intent(this, TargetActivity.class);
PendingIntent pi = PendingIntent.getActivity(this, 0, intent,
    0xc000000);  // 0x8000000|0x4000000 = FLAG_UPDATE_CURRENT|FLAG_IMMUTABLE

// 如果必须使用 FLAG_MUTABLE，确保 Intent 已填充完整字段
// 避免 PendingIntent 传递给不可信的应用
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Activity Intent 重定向 | 填充 PendingIntent 指向非导出特权 Activity，绕过访问控制 | → [Activity Intent 重定向](./app-activity-intent-redirect.md) |
| + Provider 数据窃取 | 以受害者身份访问其 ContentProvider，读取数据库全量数据 | → [Provider 数据泄露](./app-provider-data-leak.md) |
| + Framework 权限绕过 | 以受害者（可能是系统应用）身份调用系统服务接口 | → [Framework 权限缺失](./framework-service-permission-missing.md) |
| + URI 权限写入 | 修改 PendingIntent 的 data 为攻击者 content:// URI + FLAG_GRANT_WRITE_URI_PERMISSION，以系统权限写入任意文件 | → [URI 权限授予](./app-intent-uri-permission.md) |


## Related

- [[app-activity-intent-redirect]]
- [[app-activity-pendingintent-abuse]]
- [[app-intent-implicit-hijack]]
- [[app-provider-data-leak]]
- [[app-service-intent-inject]]
- [[framework-service-intent-redirect]]
- [[framework-service-permission-missing]]

- [[app-activity]]
- [[app-intent]]

