# 动态广播滥用

应用在运行时动态注册广播接收器，但未限制接收范围或校验数据来源，导致外部恶意应用可注入命令或窃取数据。

**Risk: MEDIUM**


## 利用前提

需要前置条件。目标应用动态注册的广播接收器处理外部 Intent 数据未校验来源，或应用发送的广播未指定接收权限。恶意应用可注册同名接收器监听敏感广播，或向目标接收器发送恶意数据。如果广播指定了 signature 级别权限则不可利用。

**Android 版本范围：Android 13 及以下可利用** — Android 14 (API 34) 要求运行时注册的广播接收器必须指定 `RECEIVER_EXPORTED` 或 `RECEIVER_NOT_EXPORTED` 标志，默认不再导出到所有应用。仅接收系统广播的接收器无需指定标志。


## 攻击流程

```
1. decx ard app-receivers → 列出动态注册的广播接收器
2. decx code class-source <ReceiverClass> → 分析 onReceive() 处理逻辑
3. 确认接收器处理外部 Intent 数据未校验
4. 构造恶意广播发送到目标接收器
5. 触发靮期操作（命令执行、数据写入、流程控制）
```


## 关键特征与代码

- 使用 `registerReceiver()` 注册接收器，IntentFilter 匹配范围过广，接收器处理外部 Intent 数据未做校验
- 在 `onResume()` 中注册但未在 `onPause()` 中注销，扩大了接收窗口

```java
// 漏洞：动态注册的接收器处理外部数据未校验
IntentFilter filter = new IntentFilter("com.app.ACTION");
registerReceiver(new BroadcastReceiver() {
    @Override
    public void onReceive(Context context, Intent intent) {
        String cmd = intent.getStringExtra("command");
        // 直接执行外部传入的命令
        executeCommand(cmd);
    }
}, filter);
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2020-0391** | Android 系统动态注册的广播接收器未限制发送方，导致权限绕过 |
| **第三方 SDK** | 多款广告 SDK 动态注册接收器处理 URL 参数，恶意广播可注入任意 URL |


## 安全写法

```java
// Android 14+: 注册时指定 RECEIVER_NOT_EXPORTED
registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED);

// 低版本: 使用 LocalBroadcastManager
LocalBroadcastManager.getInstance(this).registerReceiver(receiver, filter);
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Service 命令注入 | 接收器收到命令后转发给导出 Service，扩大攻击面 | → [[app-service-intent-inject]] |
| + URI 权限授予 | 广播 Intent 携带 content:// URI + FLAG_GRANT_READ，授予文件访问权 | → [[app-intent-uri-permission]] |
| + 权限绕过 | 动态接收器未指定 RECEIVER_NOT_EXPORTED，外部恶意应用可直接发送 | → [[app-broadcast-permission-bypass]] |
| + WebView URL 加载 | 接收器处理 URL 参数后传递给 WebView 加载 | → [[app-webview-url-bypass]] |


## Related

- [[app-broadcast-permission-bypass]]
- [[app-service-intent-inject]]

- [[app-broadcast]]
