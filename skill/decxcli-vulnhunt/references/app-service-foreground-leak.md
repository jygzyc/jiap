# 前台服务通知泄露

Service 启动为前台服务时，通知可能泄露敏感信息。

**Risk: LOW**


## 利用前提

独立可利用（危害低）。前台服务通知栏对用户可见。如果通知包含敏感信息（Token、密码、通信内容），同设备用户或截屏即可获取。不需要其他应用配合。

**Android 版本范围：所有版本可利用** — 前台服务通知栏信息对用户可见，无版本限制。Android 13+ 需要前台通知权限 (POST_NOTIFICATIONS)。


## 攻击流程

```
1. decx ard exported-components → 定位导出 Service
2. decx code class-source <ServiceClass> → 获取 Service 源码，定位 startForeground 调用
3. 从 class-source 中定位 Notification 构建代码
4. decx code class-source <ServiceClass> → 定位 startForeground 调用
5. 追踪 Notification.Builder 构造过程
6. 检查 setContentText/setContentTitle/setStyle 是否包含敏感变量
7. 检查 setVisibility 是否设置为 VISIBILITY_SECRET
8. 截屏或同设备观察通知内容
```


## 关键特征与代码

- 前台服务通知中使用 `NotificationCompat.Builder` 直接拼入敏感数据（位置、文件名、通信内容），未设置 `VISIBILITY_SECRET`，锁屏状态下仍然可见

```java
// 漏洞：前台通知中包含敏感信息
public class LocationService extends Service {
    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String location = intent.getStringExtra("current_location");
        // 通知中暴露精确位置
        Notification notification = new NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("正在追踪位置")
            .setContentText("当前位置: " + location)  // 敏感信息泄露
            .setSmallIcon(R.drawable.ic_location)
            .build();
        startForeground(1, notification);
        return START_STICKY;
    }
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2022-20342** | Android System UI 前台服务通知在锁屏界面泄露剪贴板内容，本地攻击者可读取 |
| **CVE-2021-0683** | 系统服务前台通知包含敏感操作状态，未设置 VISIBILITY_SECRET 导致锁屏泄露 |
| **健康追踪 APP** | 前台服务通知栏实时显示用户心率、血压等健康数据，锁屏状态可见 |


## 安全写法

```java
public class SecureLocationService extends Service {
    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        Notification notification = new NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("位置服务运行中")
            .setContentText("正在为您提供位置服务")  // 不暴露具体信息
            .setSmallIcon(R.drawable.ic_location)
            .setVisibility(NotificationCompat.VISIBILITY_SECRET)  // 锁屏不显示
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build();
        startForeground(1, notification);
        return START_STICKY;
    }
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + 点击劫持 | 通知栏显示敏感信息后被覆盖层截获，放大信息泄露 | → [[app-activity-clickjacking]] |
| + Service 命令注入 | 导出 Service 接受外部 Intent 触发前台服务，通知含敏感参数 | → [[app-service-intent-inject]] |
| + Broadcast 本地泄露 | 前台服务通过广播更新状态，广播内容也含敏感信息 | → [[app-broadcast-local-leak]] |


## Related

- [[app-service]]
