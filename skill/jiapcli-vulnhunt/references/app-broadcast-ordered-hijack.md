# 有序广播劫持

有序广播（`sendOrderedBroadcast`）按优先级传递给各接收器，高优先级接收器可修改或拦截广播数据。

**Risk: MEDIUM**


## 利用前提

独立可利用。目标应用使用 `sendOrderedBroadcast()` 且未指定接收权限。恶意应用注册高优先级接收器，可截获、篡改或丢弃广播数据。

**Android 版本范围：Android 13 及以下可利用** — Android 14 要求运行时注册的接收器必须指定 `RECEIVER_EXPORTED`/`RECEIVER_NOT_EXPORTED`。仅接收系统广播的接收器无需指定。manifest 声明的静态接收器不受影响。


## 攻击流程

```
1. jiap code class-source <SendingComponentClass> → 定位发送有序广播的组件
2. jiap code xref-method "package.Class.sendOrderedBroadcast(android.content.Intent,java.lang.String):void" → 交叉引用追踪
3. 检查 sendOrderedBroadcast 是否指定 permission 参数
4. 检查广播中携带的敏感数据和操作指令
5. 恶意应用注册高优先级 (priority=999) 接收器
6. 截获/篡改 setResultData 或 abortBroadcast 拦截广播
```


## 关键特征

- 应用发送有序广播携带敏感操作指令
- 恶意应用注册高优先级（`android:priority="999"`）的接收器
- 恶意接收器修改 `getResultData()` 或调用 `setResultData()` 替换数据
- 恶意接收器可调用 `abortBroadcast()` 完全拦截广播，阻止后续接收器收到数据
- 未对有序广播设置 `permission` 参数限制接收者


## 代码模式

```java
// 漏洞：发送有序广播未设置权限，接收数据未校验
public class SmsManager {
    public void sendSms(String number, String text) {
        Intent intent = new Intent("com.example.ACTION_SEND_SMS");
        intent.putExtra("number", number);
        intent.putExtra("text", text);
        // 未指定 permission，任意应用可接收并篡改
        context.sendOrderedBroadcast(intent, null);
    }
}

// 接收端：直接使用广播数据未校验
public class SmsReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context context, Intent intent) {
        String number = intent.getStringExtra("number");
        String text = intent.getStringExtra("text");
        // 直接使用数据，可能已被中间接收器篡改
        SmsManager.getDefault().sendTextMessage(number, null, text, null, null);
    }
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2022-20010** | Android 框架层有序广播未设置权限保护，高优先级接收器可篡改系统广播数据 |
| **银行 APP SMS 劫持** | 部分银行 APP 通过有序广播传递交易验证码，恶意应用注册高优先级接收器截获 OTP |


## 安全写法

```java
// 发送有序广播时指定 signature 权限
context.sendOrderedBroadcast(intent, "com.example.PERMISSION_SMS");

// AndroidManifest.xml
// <permission android:name="com.example.PERMISSION_SMS"
//     android:protectionLevel="signature" />
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Activity Intent 重定向 | 修改有序广播中的 Intent 数据，注入恶意目标组件 | → [[app-activity-intent-redirect]] |
| + 本地广播泄露 | 截获有序广播中的敏感数据后修改结果 | → [[app-broadcast-local-leak]] |
| + Service 命令注入 | 篡改广播数据后触发后续接收器中的 Service 启动 | → [[app-service-intent-inject]] |


## Related

- [[app-broadcast]]

