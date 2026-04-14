# Service 安全审计

Service 是 Android 后台执行组件。导出 Service 可被任意应用绑定或启动，AIDL 接口暴露、Messenger 滥用、Intent 注入等漏洞可导致权限提升和敏感操作执行。

## 风险清单

| 风险 | 等级 | 详情 |
|------|------|------|
| AIDL 接口暴露 | HIGH | [[app-service-aidl-expose]] |
| Messenger 消息滥用 | HIGH | [[app-service-messenger-abuse]] |
| Intent 命令注入 | HIGH | [[app-service-intent-inject]] |
| 绑定提权 | HIGH | [[app-service-bind-escalation]] |
| 前台服务泄露 | MEDIUM | [[app-service-foreground-leak]] |

## 分析流程

```
1. jiap ard exported-components          → 列出导出 Service
2. 对每个导出 Service：
   a. jiap code class-source <Service>   → 获取源码
   b. 检查 onBind() 返回的 IBinder 接口
   c. 检查 onHandleIntent() / onStartCommand() 对 Intent 的处理
   d. 检查 Messenger 实现（handleMessage）
3. 追踪 AIDL 接口：
   jiap code search-class "Stub"         → 定位 AIDL Stub 实现
   jiap code class-source <AidlClass.Stub> → 获取接口方法
4. 检查权限保护：
   搜索 enforcePermission / checkPermission 调用
   检查 AndroidManifest 中 Service 的 android:permission 属性
```

## 关键追踪模式

- **AIDL 暴露**：`onBind()` 返回的 `Stub` 实现，检查接口方法的权限校验
- **Messenger 消息**：`handleMessage()` 中 `msg.what` 的分发，是否处理攻击者构造的消息
- **Intent 注入**：`onStartCommand()` / `onHandleIntent()` 对传入 Intent 的处理
- **绑定提权**：恶意应用通过 `bindService()` 获取高权限接口

## Related

[[app-activity]]
[[app-intent]]
[[app-broadcast]]
[[framework-service]]
[[risk-rating]]
