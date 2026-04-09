# 隐式 Intent 劫持

应用发送隐式 Intent（未指定目标组件），被恶意应用截获，获取其中的敏感数据或劫持操作。

**Risk: MEDIUM**


## 利用前提

独立可利用。目标应用发送隐式广播/启动隐式 Activity 时，恶意应用注册了高优先级的同名接收器/Activity。攻击者截获 Intent 获取其中的数据。如果使用 `setPackage()` / `setComponent()` 或 `LocalBroadcastManager` 则不可利用。

**Android 版本范围：Android 10 ~ 13 可利用** — Android 14 (API 34) 限制：隐式 intent 只能传送到导出的组件，不能传送到未导出的组件。应用必须使用显式 intent 访问未导出组件。这有效防止了通过隐式 intent 劫持非导出组件。


## 攻击流程

```
1. jiap ard exported-components → 定位组件和源码
2. jiap code xref-method "android.content.Context.startActivity(...)" → 交叉引用追踪 Intent 发送
3. jiap code xref-method "android.content.Context.sendBroadcast(...)" → 追踪 sendBroadcast 调用
4. jiap code xref-method "android.content.Context.startService(...)" → 追踪 startService 调用
5. jiap code xref-method "android.content.Context.bindService(...)" → 追踪 bindService 调用
6. jiap code class-source <Class> → 定位 sendBroadcast/startActivity/startService 调用
7. 确认 Intent 是否为隐式（未指定 Component/Package）
8. 确认 Intent 中是否携带敏感数据（Token、密码、用户信息）
9. 恶意应用注册同名 action 的接收器/Activity
10. 截获 Intent 获取敏感数据
```


## 关键特征

- 创建 Intent 时未调用 `setClassName()` 或 `setComponent()` 指定目标
- 发送包含敏感数据的广播使用隐式 Intent
- 启动 Service 使用隐式 Intent（Android 5.0+ 已禁止，但旧代码仍可能存在）


## 代码模式

```java
// 漏洞：隐式 Intent 携带敏感数据
Intent intent = new Intent("com.app.ACTION_LOGIN");
intent.putExtra("password", userPassword);
sendBroadcast(intent); // 任何注册了该 action 的应用都能接收
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2018-9489** | Android 系统通过隐式广播发送 WiFi 状态信息（SSID、BSSID、IP），任意应用可截获 |
| **CVE-2020-0108** | Android 系统组件通过隐式 Intent 启动 Service，恶意应用可劫持 |
| **Plaid CTF 2019** | 银行应用通过隐式广播发送交易确认信息，恶意应用截获交易详情 |


## 安全写法

```java
// 使用显式 Intent 指定目标组件
Intent intent = new Intent(this, TargetActivity.class);
intent.putExtra("password", userPassword);
startActivity(intent);
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Broadcast 泄露 | 劫持隐式广播，获取本应只在应用内传播的凭证 | → [[app-broadcast-local-leak]] |
| + PendingIntent 窃取 | 隐式 Intent 携带 PendingIntent，劫持后可篡改执行 | → [[app-intent-pendingintent-escalation]] |
| + Service 命令注入 | 劫持隐式 Intent 发送的 Service 启动请求，恶意应用伪装为目标 Service | → [[app-service-intent-inject]] |
| + URI 权限授予 | 隐式 Intent 携带 FLAG_GRANT + content:// URI，截获后获得文件访问权 | → [[app-intent-uri-permission]] |


## Related

- [[framework-service-data-leak]]

- [[app-broadcast]]
- [[app-intent]]

