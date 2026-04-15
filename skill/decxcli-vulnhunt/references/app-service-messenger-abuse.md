# Messenger 消息滥用

Service 通过 `Messenger` 暴露消息处理接口，Handler 未校验调用者身份，任意应用可发送消息触发非预期操作。**Risk: HIGH**


## 利用前提

独立可利用。Service 导出 + onBind 返回 Messenger + Handler 未校验 `Binder.getCallingUid()`。攻击者绑定后发送 Message 触发任意操作。

**Android 版本范围：所有版本可利用** — 应用层配置问题。


## 攻击流程

```
1. decx ard exported-components → 定位导出 Service
2. decx code class-source <ServiceClass> → 检查 onBind 返回 Messenger.getBinder()
3. decx code subclass android.os.Messenger → 查找 Messenger 实现类
4. decx code subclass android.os.Handler → 定位 Handler 子类
5. decx code xref-method "package.Class.send(...):void" → xref 追踪 send 调用
6. decx code xref-method "package.Class.replyTo(...):void" → xref 追踪 replyTo 调用
7. 从 class-source 中定位 handleMessage 方法，检查 msg.what 分发逻辑
8. 分析 handleMessage() 中 msg.what 各分支的操作
9. 编写攻击应用绑定服务并发送恶意 Message
```


## 关键特征与代码

- `onBind()` 返回 `Messenger(handler).getBinder()`，`handleMessage()` 根据 `msg.what` 分发操作，Handler 中未调用 `Binder.getCallingUid()` 校验调用者身份，消息内容来自外部且未做输入校验

```java
// 漏洞：Messenger 处理外部消息，未校验调用者
class IncomingHandler extends Handler {
    @Override
    public void handleMessage(Message msg) {
        switch (msg.what) {
            case MSG_GET_DATA:
                // 直接返回敏感数据给调用者
                Message reply = Message.obtain(null, MSG_DATA_RESPONSE);
                reply.obj = sensitiveData;
                msg.replyTo.send(reply);
                break;
            case MSG_EXECUTE:
                String cmd = msg.getData().getString("command");
                Runtime.getRuntime().exec(cmd); // 命令注入
                break;
        }
    }
}

public class MyService extends Service {
    private final Messenger mMessenger = new Messenger(new IncomingHandler());

    @Override
    public IBinder onBind(Intent intent) {
        return mMessenger.getBinder();
    }
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2014-7911** | Android 系统服务通过 Messenger 接受消息时未校验调用者，可触发反序列化执行代码 |
| **智能家居 APP** | 导出 Service Messenger 接口控制设备开关，无身份校验 |


## 安全写法

```java
class SecureIncomingHandler extends Handler {
    @Override
    public void handleMessage(Message msg) {
        int callingUid = Binder.getCallingUid();
        if (callingUid != Process.myUid()) {
            // 校验调用者身份
            if (checkCallingOrSelfPermission("com.example.PERMISSION")
                    != PackageManager.PERMISSION_GRANTED) {
                Log.w(TAG, "Rejected message from uid=" + callingUid);
                return;
            }
        }
        switch (msg.what) {
            case MSG_GET_DATA:
                // 校验通过后再返回数据
                Message reply = Message.obtain(null, MSG_DATA_RESPONSE);
                reply.obj = getSanitizedData();
                msg.replyTo.send(reply);
                break;
        }
    }
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Service 绑定提权 | 绕过 Messenger 校验直接绑定 Service 调用敏感方法 | → [[app-service-bind-escalation]] |
| + 反序列化 | Message Bundle 中携带恶意序列化对象 | → [[app-intent-parcel-mismatch]] |
| + AIDL 接口暴露 | 同一 Service 同时暴露 Messenger 和 AIDL 接口 | → [[app-service-aidl-expose]] |
| + Intent 重定向 | Handler 处理消息时将外部数据作为 Intent 参数启动组件 | → [[app-activity-intent-redirect]] |


## Related

- [[app-intent]]
- [[app-service]]
