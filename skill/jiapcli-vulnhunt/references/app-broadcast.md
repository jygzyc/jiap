# Broadcast 安全审计

Broadcast 是 Android 消息传递机制。动态注册的广播接收器、有序广播劫持、本地广播泄露等漏洞可导致敏感数据泄露和权限绕过。

## 风险清单

| 风险 | 等级 | 详情 |
|------|------|------|
| 动态广播滥用 | MEDIUM | [[app-broadcast-dynamic-abuse]] |
| 有序广播劫持 | MEDIUM | [[app-broadcast-ordered-hijack]] |
| 权限绕过 | HIGH | [[app-broadcast-permission-bypass]] |
| 本地广播泄露 | MEDIUM | [[app-broadcast-local-leak]] |

## 分析流程

```
1. jiap ard receivers -P <port>                    → 列出动态广播接收器
2. jiap code search-method "registerReceiver" -P <port> → 定位动态注册点
3. 对每个接收器：
   a. jiap code class-source "<Receiver>" -P <port>  → 获取源码
   b. 检查 onReceive 中是否处理敏感数据
   c. 检查是否使用 LocalBroadcastManager（安全）或全局 sendBroadcast（危险）
4. jiap code xref-method "android.content.Context.sendBroadcast(android.content.Intent):void" -P <port> → 追踪广播发送
5. jiap code xref-method "android.content.Context.sendOrderedBroadcast(android.content.Intent,java.lang.String):void" -P <port> → 有序广播
6. 检查广播是否携带敏感 extra 数据
7. 检查是否设置 receiverPermission 参数
```

## 关键追踪模式

- **动态注册**：`registerReceiver()` 注册的接收器，检查 IntentFilter 的 action
- **有序广播**：`sendOrderedBroadcast()` 的优先级劫持和结果篡改
- **权限保护**：发送和接收端是否设置 `receiverPermission`
- **本地广播**：是否误用全局广播传递本应局限于应用内的数据

## Related

[[app-activity]]
[[app-intent]]
[[app-service]]
[[framework-service]]
[[risk-rating]]
