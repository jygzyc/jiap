---
name: jiapcli
description: Android APK/DEX/JAR 安全审计 CLI。基于 JADX 反编译器，支持代码检索、交叉引用、组件分析、漏洞挖掘。当用户提到 jiap、jadx、android analysis、android security、APK audit、安全审计、漏洞挖掘时使用。
metadata:
  requires:
    bins: ["jiap"]
---

# JIAP CLI

基于 JADX 的 Android 安全分析工具。分析 APK/框架时使用此 skill 获取工具命令和漏洞挖掘方法论。

## 命令参考

### process - 进程管理

| 命令 | 说明 |
|------|------|
| `jiap process check [--install]` | 检查环境状态，`--install` 安装缺失组件 |
| `jiap process open <apk> [--name <name>]` | 打开 APK 分析（`--name` 自定义 session 名） |
| `jiap process close [name] [--all]` | 关闭指定 session 或全部 |
| `jiap process list` | 列出运行 session（NAME/PORT/PID/PATH） |

### code - 代码分析

| 命令 | 说明 |
|------|------|
| `jiap code all-classes` | 列出所有类 |
| `jiap code class-info <class>` | 获取类信息 |
| `jiap code class-source <class>` | 获取类源码（`--smali` 输出 Smali） |
| `jiap code method-source <sig>` | 获取方法源码（`--smali`） |
| `jiap code xref-method <sig>` | 方法交叉引用（谁调用了它） |
| `jiap code xref-class <class>` | 类交叉引用 |
| `jiap code xref-field <field>` | 字段交叉引用 |
| `jiap code implement <interface>` | 查找接口实现 |
| `jiap code subclass <class>` | 查找子类 |
| `jiap code search-class <keyword>` | 搜索类内容（占资源，必要场景使用，禁止批量调用） |
| `jiap code search-method <name>` | 搜索方法名（占资源，必要场景使用，禁止批量调用） |

### ard - Android 分析

| 命令 | 说明 |
|------|------|
| `jiap ard app-manifest` | AndroidManifest.xml |
| `jiap ard main-activity` | 主 Activity |
| `jiap ard app-application` | Application 类 |
| `jiap ard exported-components` | 导出组件列表 |
| `jiap ard app-deeplinks` | Deep Link 列表 |
| `jiap ard all-resources` | 所有资源文件名 |
| `jiap ard resource-file <res>` | 获取资源文件内容 |
| `jiap ard strings` | 获取 strings.xml 内容 |
| `jiap ard receivers` | 动态广播接收器 |
| `jiap ard system-service-impl <interface>` | 系统服务实现类 |

### 全局选项

适用于 `code`/`ard`：

| 选项 | 说明 |
|------|------|
| `-s, --session <name>` | 指定目标 session 名称（APK 文件名） |
| `-P, --port <port>` | 指定服务器端口（默认 25419） |
| `--json` | JSON 格式输出 |
| `--page <n>` | 分页参数（默认 1） |

## 方法签名格式

用于 `code method-source` 和 `code xref-method`：

```
package.Class.methodName(paramType1,paramType2):returnType
```

示例：`com.example.MainActivity.onCreate(android.os.Bundle):void`

## 分析策略

优先使用 `class-source` + `xref-*` + `implement` 定位代码。`search-*` 必要场景使用，禁止批量调用。

```
典型流程：
1. jiap ard exported-components     → 定位目标组件
2. jiap code class-source <Class>   → 获取源码，找到关键方法
3. jiap code xref-method <sig>      → 追踪数据流
4. jiap code implement <Interface>  → 查找所有实现

必要时可使用 `search-class` 搜索全局关键字， `search-method` 按名称搜索方法。找到具体实现后，再查找接口的所有实现类。
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
