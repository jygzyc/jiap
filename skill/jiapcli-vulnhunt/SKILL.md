---
name: jiapcli-vulnhunt
description: Android 漏洞挖掘专用 skill。基于 JIAP CLI + JADX，提供组件级攻击面分析、可利用性评估、漏洞链构造方法论和风险评级参考。当用户提到漏洞挖掘、vulnerability hunting、exploit chain、攻击面分析、attack surface、安全审计、security audit、风险评级时使用。
metadata:
  requires:
    bins: ["jiap"]
---

# JIAP CLI — Android 漏洞挖掘

Android 组件级漏洞挖掘方法论。提供攻击面枚举、数据流追踪、可利用性判定和风险评级参考。

命令参考见通用 skill `jiapcli`。

## 漏洞挖掘流程

```
1. jiap ard exported-components     → 枚举攻击面（导出组件）
2. jiap ard app-deeplinks           → 检查 Deep Link 入口
3. jiap code class-source <Class>   → 阅读目标组件源码
4. jiap code xref-method <sig>      → 追踪 Source → Sink 数据流
5. jiap code implement <Interface>  → 查找接口所有实现
6. 对每个发现评估可利用性三要素
```

## 核心原则：可利用性

只报告满足以下全部条件的发现：

1. **可达** — 攻击者能触发该代码路径
2. **可控** — 攻击者能影响关键数据
3. **有影响** — 造成安全后果

任一条件不满足 → 不报告。

## 报告模板

```
[严重性] 漏洞标题
Risk: CRITICAL/HIGH/MEDIUM/LOW
Component: com.package.ClassName.vulnerableMethod(paramType):returnType
Cause: 缺陷简述

Evidence:
  // Source
  Intent intent = getIntent().getParcelableExtra("key");
  // Sink
  startActivity(intent);

Taint Flow:
  Source → Propagation → [NO SANITIZER] → Sink

Exploit Path:
  1. 攻击步骤...
  2. ...

Impact: 攻击者获得什么
Mitigation: 修复建议
```

## References

分析特定组件时，先阅读对应的组件总览文件，再根据目标组件类型定位具体风险文件。每个组件总览包含风险清单、分析流程和交叉引用。

| 文件 | 组件 | 适用场景 |
|------|------|----------|
| `references/app-activity.md` | Activity | 分析导出 Activity |
| `references/app-intent.md` | Intent | 追踪 Intent 数据流 |
| `references/app-broadcast.md` | Broadcast | 分析广播接收器 |
| `references/app-provider.md` | ContentProvider | 分析导出 Provider |
| `references/app-service.md` | Service | 分析导出 Service |
| `references/app-webview.md` | WebView | 分析 WebView 使用 |
| `references/framework-service.md` | 系统服务 | 审计框架服务 |
| `references/risk-rating.md` | 风险评级 | 判断漏洞严重性 |
