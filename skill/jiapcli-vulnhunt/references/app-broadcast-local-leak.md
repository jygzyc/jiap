# 本地广播泄露

**Risk: MEDIUM**


## 利用前提

独立可利用。目标应用使用 `sendBroadcast()` 发送含敏感数据的广播，未指定接收权限或包名。同设备任何应用均可注册接收器截获。

**Android 版本范围：Android 13 及以下可利用** — 同 broadcast-dynamic-abuse，Android 14 要求运行时注册的接收器显式声明导出行为，默认不导出。


## 攻击流程

```
1. jiap ard app-manifest → 检查广播接收器声明和权限配置
2. jiap code class-source <ReceiverClass> → 获取 Receiver 源码，检查是否调用 sendBroadcast 系列方法
3. jiap code xref-method "package.ReceiverClass.sendBroadcast(android.content.Intent):void" → 追踪 sendBroadcast 调用
4. jiap code xref-method "package.ReceiverClass.sendOrderedBroadcast(android.content.Intent,java.lang.String):void" → 追踪 sendOrderedBroadcast 调用
5. jiap code class-source <Class> → 定位 sendBroadcast 调用，检查 Intent 中的敏感数据
6. 确认发送广播未指定包名/权限/未使用 LocalBroadcastManager
7. 恶意应用注册同名 action 的接收器
8. 截获广播获取 Token、凭证、用户数据等敏感信息
```


## 关键特征

- 使用 `sendBroadcast()` 发送包含敏感数据的 Intent
- 未调用 `intent.setPackage()` 限定接收者
- 未使用 `LocalBroadcastManager`


## 代码模式

```java
// 漏洞：通过全局广播发送敏感信息
Intent intent = new Intent("com.app.ACTION_TOKEN_READY");
intent.putExtra("auth_token", secretToken);
sendBroadcast(intent); // 任何应用可注册接收
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2018-9489** | Android WiFi 状态广播包含 SSID/BSSID/IP 地址，任意应用可截获 |
| **CVE-2019-2198** | Android 蓝牙状态广播泄露 MAC 地址 |
| **银行 APP** | 部分银行 APP 通过全局广播发送交易确认 Token，同设备恶意应用可截获 |


## 安全写法

```java
// 使用 setPackage 限定接收方
Intent intent = new Intent("com.app.ACTION_TOKEN_READY");
intent.putExtra("auth_token", secretToken);
intent.setPackage(getPackageName());
sendBroadcast(intent);

// 或使用 LocalBroadcastManager
LocalBroadcastManager.getInstance(this).sendBroadcast(intent);
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + 隐式 Intent 劫持 | 同一条链：sendBroadcast 隐式发送 → 恶意应用截获 → 获取内部通信数据 | → [[app-intent-implicit-hijack]] |
| + Provider 数据窃取 | 广播泄露的 URI/Authority 用于直接访问 ContentProvider | → [[app-provider-data-leak]] |
| + PendingIntent 窃取 | 广播中携带的 PendingIntent 被截获后篡改执行 | → [[app-intent-pendingintent-escalation]] |
| + 有序广播劫持 | 截获敏感数据后篡改并转发给后续接收器 | → [[app-broadcast-ordered-hijack]] |


## Related

- [[app-broadcast-ordered-hijack]]
- [[app-intent-implicit-hijack]]
- [[app-service-foreground-leak]]
- [[framework-service-data-leak]]

- [[app-broadcast]]
- [[app-intent]]

