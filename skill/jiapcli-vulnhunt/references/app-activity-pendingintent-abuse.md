# PendingIntent 滥用

导出 Activity 未校验 PendingIntent 来源或创建 PendingIntent 时未限制目标组件，攻击者可以受害者身份执行操作。

**Risk: HIGH**


## 利用前提

目标应用创建 PendingIntent 时使用 `FLAG_MUTABLE` 且 base Intent 未指定目标组件，攻击者可填充 action/component 以受害者身份执行操作。

**Android 版本范围：Android 13 及以下可利用** — Android 12+ 强制指定 IMMUTABLE/MUTABLE，Android 14+ 禁止通过未指定组件的 intent 创建可变 PendingIntent。


## 攻击流程

```
1. jiap code class-source <Activity> → 获取源码，搜索 PendingIntent.getActivity/getService/getBroadcast
2. jiap code xref-method "android.app.PendingIntent.getActivity(...)" → 交叉引用追踪 PendingIntent 操作
3. jiap code xref-method "android.app.PendingIntent.getService(...)" → 追踪 getService 调用
4. jiap code xref-method "android.app.PendingIntent.getBroadcast(...)" → 追踪 getBroadcast 调用
5. jiap code xref-method "android.app.PendingIntent.send()" → 追踪 send 调用
6. 在反编译源码中搜索 flag 值：
   → 0x2000000 = FLAG_MUTABLE，0x4000000 = FLAG_IMMUTABLE
   → 0x8000000 = FLAG_UPDATE_CURRENT
   → 0xa000000 = FLAG_UPDATE_CURRENT | FLAG_MUTABLE
7. jiap code class-source <Activity> → 检查是否使用 FLAG_MUTABLE 且未指定目标组件
8. 确认导出 Activity 是否接收并执行外部传入的 PendingIntent
9. 构造恶意 Intent 填充 action/component 指向敏感操作
10. adb shell am start -n com.target/.ExportedActivity --el callback ...
11. 目标 Activity 执行 PendingIntent，以受害者身份完成攻击
```


## 关键特征与代码

- 创建 PendingIntent 时未指定目标组件（空 Intent），flag 含 `0x2000000`（FLAG_MUTABLE）而非 `0x4000000`（FLAG_IMMUTABLE）
- 导出 Activity 接收并执行外部传入的 PendingIntent

```java
// 漏洞1：空 Intent + FLAG_MUTABLE
Intent intent = new Intent(); // 空 Intent，未指定目标
PendingIntent pi = PendingIntent.getActivity(this, 0, intent,
    0xa000000);  // 0x8000000|0x2000000 = FLAG_UPDATE_CURRENT|FLAG_MUTABLE
// 恶意应用可填充 action/component 以受害者身份执行操作

// 漏洞2：导出 Activity 执行外部 PendingIntent
PendingIntent pi = getIntent().getParcelableExtra("callback");
pi.send(); // 以受害者身份执行
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2025-43977** | 韩国 SK Telecom 拨号 Activity 导出，接收 PendingIntent 后以受害者身份拨打电话 |
| **CVE-2023-47889** | 通过导出 Activity 传递 PendingIntent 实现远程关机 |


## 安全写法

```java
// FLAG_IMMUTABLE + 显式指定目标组件
Intent intent = new Intent(this, TargetActivity.class);
PendingIntent pi = PendingIntent.getActivity(this, 0, intent,
    0xc000000);  // 0x8000000|0x4000000 = FLAG_UPDATE_CURRENT|FLAG_IMMUTABLE
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Intent 重定向 | PendingIntent 指向非导出特权 Activity，以受害者身份绕过访问控制 | → [[app-activity-intent-redirect]] |
| + Framework 权限绕过 | 以受害者（可能是系统应用）身份调用系统服务接口 | → [[framework-service-permission-missing]] |
| + 点击劫持 | 覆盖权限请求对话框后，用户无意授权 PendingIntent 操作 | → [[app-activity-clickjacking]] |
| + setResult 泄露 | PendingIntent 执行结果通过 setResult 返回，泄露敏感操作结果 | → [[app-activity-setresult-leak]] |
| + 任务栈劫持 | 劫持任务栈后注入携带 PendingIntent 的 Activity，以受害者身份静默执行 | → [[app-activity-task-hijack]] |


## Related

- [[app-activity-clickjacking]]
- [[app-provider-fileprovider-misconfig]]
- [[app-webview-intent-scheme]]

- [[app-activity]]
- [[app-activity-intent-redirect]]
- [[app-intent-pendingintent-escalation]]
- [[framework-service-permission-missing]]
