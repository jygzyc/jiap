# Activity 安全审计

Activity 是 Android 应用最常见的攻击面。导出 Activity 可被任意应用启动，Intent 重定向、Fragment 注入、路径遍历等漏洞均以 Activity 为入口。

## 风险清单

| 风险 | 等级 | 详情 |
|------|------|------|
| 组件导出越权访问 | MEDIUM | [[app-activity-exported-access]] |
| Intent 重定向（LaunchAnyWhere） | HIGH | [[app-activity-intent-redirect]] |
| Fragment 注入 | MEDIUM | [[app-activity-fragment-injection]] |
| 路径遍历 | HIGH | [[app-activity-path-traversal]] |
| PendingIntent 滥用 | HIGH | [[app-activity-pendingintent-abuse]] |
| setResult 数据泄露 | MEDIUM | [[app-activity-setresult-leak]] |
| Task 劫持 | MEDIUM | [[app-activity-task-hijack]] |
| 点击劫持 | LOW | [[app-activity-clickjacking]] |
| 生命周期问题 | MEDIUM | [[app-activity-lifecycle]] |

## 分析流程

```
1. jiap ard exported-components -P <port>         → 列出所有导出 Activity
2. jiap ard main-activity -P <port>               → 定位主入口
3. jiap ard app-deeplinks -P <port>               → 检查 Deep Link 入口
4. 对每个导出 Activity：
   a. jiap code class-source "<Activity>" -P <port> → 获取源码
   b. 检查 onCreate/onNewIntent 中是否从 Intent 提取嵌套 Intent
   c. 检查是否使用 getParcelableExtra("...*intent*...")
   d. jiap code xref-method "<methodSig>" -P <port> → 追踪数据流（签名格式：pkg.Class.method(paramType):returnType）
5. 重点关注：
   - getParcelableExtra 提取 Intent 后 startActivity（重定向）
   - getIntent().getData() 的 URI 处理（路径遍历）
   - setResult 返回敏感数据
   - extra 中传递的 Fragment 名（Fragment 注入）
```

## 关键追踪模式

- **Intent 重定向**：`getParcelableExtra()` → `startActivity()` 的嵌套 Intent 转发
- **Fragment 注入**：extra 中传递 Fragment 类名，未校验白名单
- **路径遍历**：`getIntent().getData()` 的 URI 未做路径规范化
- **PendingIntent**：从 extra 中提取 PendingIntent 并 send()
- **setResult 泄露**：`setResult()` 返回敏感数据给调用者
- **Task 劫持**：`taskAffinity` + `allowTaskReparenting` 组合

## Related

[[app-intent]]
[[app-webview]]
[[app-service]]
[[app-provider]]
[[app-broadcast]]
[[framework-service]]
[[risk-rating]]
