# Intent 安全审计总览

Intent 是 Android 组件间通信的核心机制，也是数据流追踪的关键。Intent 相关漏洞涵盖隐式劫持、PendingIntent 提权、URI 权限授予等，是组件漏洞的传播通道。

## 风险清单

| 风险 | 严重性 | 文件 |
|------|--------|------|
| PendingIntent 权限提升 | HIGH | [[app-intent-pendingintent-escalation]] |
| URI 权限授予滥用 | HIGH | [[app-intent-uri-permission]] |
| 隐式 Intent 劫持 | MEDIUM | [[app-intent-implicit-hijack]] |
| ClassLoader 注入 | HIGH | [[app-intent-classloader-inject]] |
| Parcel 不匹配攻击 | HIGH | [[app-intent-parcel-mismatch]] |

## 分析流程

```
1. 定位数据入口（Source）：
   jiap code xref-method "android.content.Intent.getParcelableExtra(...)"
   jiap code xref-method "android.content.Intent.getStringExtra(...)"
   jiap code xref-method "android.content.Intent.getData()"

2. 定位数据出口（Sink）：
   jiap code xref-method "android.content.Context.startActivity(...)"
   jiap code xref-method "android.content.Context.sendBroadcast(...)"
   jiap code xref-method "android.content.Context.startService(...)"

3. 追踪 PendingIntent：
   jiap code xref-method "android.app.PendingIntent.getActivity(...)"
   jiap code xref-method "android.app.PendingIntent.getService(...)"

4. 检查 URI 权限：
   搜索 FLAG_GRANT_READ_URI_PERMISSION / FLAG_GRANT_WRITE_URI_PERMISSION
   追踪 content:// URI 的传递路径
```

## 关键追踪模式

- **Source → Sink**：从 `getIntent().getXxxExtra()` 到 `startActivity()`/`sendBroadcast()`
- **Parcelable 反序列化**：`getParcelableExtra()` 提取的 Intent/Bundle 是否被直接使用
- **PendingIntent**：谁创建、谁发送、触发时以谁的身份执行
- **URI 权限**：content:// URI 是否跨组件传递，是否携带 GRANT flags

## 交叉引用

- Activity 入口 → [[app-activity]]
- Broadcast 劫持 → [[app-broadcast]]
- Service 注入 → [[app-service]]
- 系统服务 Intent 转发 → [[framework-service]]
