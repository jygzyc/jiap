---
name: jiapcli-vulnhunt
description: Android 漏洞挖掘专用 skill。基于 JIAP CLI + JADX，提供组件级攻击面分析、可利用性评估、漏洞链构造方法论和风险评级参考。当用户提到漏洞挖掘、vulnerability hunting、exploit chain、攻击面分析、attack surface、安全审计、security audit、风险评级时使用。
metadata:
  requires:
    bins: ["jiap"]
---

# JIAP CLI — Android 漏洞挖掘

Android 组件级漏洞挖掘方法论。提供攻击面枚举、数据流追踪、可利用性判定和风险评级参考。

**职责边界**：本 skill 负责漏洞发现和可利用性评估。PoC 应用构造和编译验证交由 skill `jiapcli-poc` 完成。

命令参考见通用 skill `jiapcli`。

## 漏洞挖掘流程

### Phase 1：环境准备

```
jiap process open <apk-path> -P <port>  → 打开 APK 启动分析服务
jiap process status -P <port>           → 确认服务运行正常
```

分析完成后释放资源：`jiap process close <name> -P <port>`

### Phase 2：攻击面枚举

```
jiap ard exported-components         → 列出所有导出组件（攻击入口）
jiap ard app-deeplinks               → 列出 Deep Link 入口（远程可达）
jiap ard app-receivers               → 列出动态注册的广播接收器
jiap ard app-manifest                → 获取完整 Manifest（权限声明、组件配置）
jiap ard strings                     → 硬编码字符串（URL、Token、密钥）
```

输出攻击面清单：每个导出组件的包名、类名、导出方式（`exported=true` / `intent-filter`）、权限保护情况。

**权限保护判定规则**：对每个导出组件声明的权限（`android:permission` / `android:readPermission` / `android:writePermission`），必须回查 Manifest 中该权限的 `protectionLevel`：

| protectionLevel | 判定 | 说明 |
|----------------|------|------|
| `normal` | **无保护** | 任何 app 可自动获得，等同于无权限声明 |
| `dangerous` | **弱保护** | 需用户动态授权，评估用户授权门槛 |
| `signature` | **受保护** | 仅同签名 app 可获得，第三方 app 不可满足 |
| `signatureOrSystem` | **受保护** | 系统签名或预装 app 可获得 |

> 若组件使用自定义权限但 protectionLevel 为 `normal`，必须将其视为**无保护**。

### Phase 3：逐组件深度分析

对 Phase 2 输出的每个导出组件，加载对应组件的 references 总览文件，按其中的分析流程执行：

| 组件类型 | 总览文件 | 核心分析命令 |
|---------|---------|------------|
| Activity | `references/app-activity.md` | `class-source` → 检查 `onCreate`/`onNewIntent` 中的 Intent 处理 |
| Service | `references/app-service.md` | `class-source` → 检查 `onBind` 返回的接口 / `onStartCommand` 的 Intent 处理 |
| ContentProvider | `references/app-provider.md` | `class-source` → 检查 `query`/`insert`/`update`/`delete`/`openFile`/`call`/`applyBatch`/`bulkInsert` 方法 |
| BroadcastReceiver | `references/app-broadcast.md` | `class-source` → 检查 `onReceive` 的 Intent 处理 |
| WebView | `references/app-webview.md` | `search-class WebView` → 追踪 `loadUrl`/`addJavascriptInterface`/`setAllowFileAccess` |
| 系统服务 | `references/framework-service.md` | `system-service-impl` → 追踪 `clearCallingIdentity`/`enforceCallingPermission` |

对每个组件执行 Source → Sink 数据流追踪：

```
jiap code class-source <Component>       → 阅读组件完整源码
jiap code xref-method <methodSig>        → 追踪方法调用链
jiap code implement <Interface>          → 查找接口所有实现
jiap code subclass <BaseClass>           → 查找子类
```

**Source 识别**（攻击者可控的数据入口）：
- `getIntent().get*Extra()` — Intent Extra 参数
- `getIntent().getData()` — URI 参数
- `getContentResolver().query()` — ContentProvider 查询参数
- `onBind()` 返回的 Binder 接口 — IPC 输入
- `Messenger.handleMessage()` — 消息参数
- `addJavascriptInterface()` — JS Bridge 输入
- URL 加载 `loadUrl()` / `loadData()` — 外部输入

**Sink 识别**（危险操作）：
- `startActivity()` / `sendBroadcast()` / `startService()` — 组件启动
- `Runtime.exec()` / `ProcessBuilder` — 命令执行
- `openFileOutput()` / `deleteFile()` — 文件操作
- `setResult()` — 数据返回
- `PendingIntent.send()` — 身份借用执行
- `WebView.loadUrl()` — URL 加载（配合可控输入）

### Phase 4：跨组件追踪

追踪数据在组件间的流转，寻找组合利用链：

```
# Intent 重定向：导出组件是否转发嵌套 Intent
jiap code search-method "getParcelableExtra"
jiap code xref-method "...startActivity(android.content.Intent):void"

# PendingIntent 滥用：是否传递未标记 IMMUTABLE 的 PendingIntent
jiap code search-method "PendingIntent.getActivity"
jiap code search-method "PendingIntent.send"

# URI 权限授予：是否跨组件传递带 GRANT flag 的 content URI
jiap code search-method "FLAG_GRANT_READ_URI_PERMISSION"
jiap code search-method "FLAG_GRANT_WRITE_URI_PERMISSION"

# WebView URL 来源：外部输入如何流入 loadUrl
jiap code xref-method "...WebView.loadUrl(java.lang.String):void"
jiap code xref-method "...WebView.loadDataWithBaseURL(...):void"
```

### Phase 5：可利用性评估

对每个发现逐项检查三要素（详见 `references/risk-rating.md`）：

1. **可达** — 组件是否导出？是否需要攻击者不具备的权限？
2. **可控** — 攻击者能否控制从 Source 到 Sink 的关键数据？中间是否有校验？
3. **有影响** — 能否造成敏感数据泄露、权限提升、代码执行等实际后果？

三项全部满足 → 构造攻击路径，进入 Phase 6。
任一条件不满足 → **不报告**。

### Phase 6：风险评级与报告

按 `references/risk-rating.md` 标准评定风险等级，按 `assets/report-template.md` 格式输出漏洞分析报告。

报告生成后，传递给 skill `jiapcli-poc` 构造可编译的 PoC 应用进行验证。

## 核心原则：可实现的利用

**唯一判定标准：能否构造出一条完整、可复现的攻击路径。** 只报告能达成可实现的利用的发现。

**不可利用的情况（不报告）：**

- 需要 root 权限才能完成的攻击
- 需要系统签名 / `privileged` 权限才能触发的路径
- 存在系统级安全校验（如 `checkKeyIntent`、SELinux 策略、`signature` 级别权限）且无法绕过
- 需要物理接触设备、解锁 Bootloader 等非软件层面条件
- 第三方 app 无法满足的前置条件

**核心判断标准：任何第三方 app（无特殊权限、无需 root）能否完成整个攻击链。** 如果答案是否定的，则不报告。

**权限校验的实际效力**：
- **组件启动阶段**（Activity/Service/BroadcastReceiver 的拉起）：系统在组件调度时自动校验 Manifest 声明的权限。`signature` / `signatureOrSystem` 级别的权限声明在此阶段有效，第三方 app 不可绕过。
- **运行时交互阶段**（Provider 的 query/insert/call、Service 的 onBind 返回接口、接收方处理 Intent 数据）：Manifest 权限声明仅控制谁能调用该组件，**不控制组件内部如何处理传入数据**。如果组件收到数据后以自身身份执行特权操作（如以 app 身份读文件、以 app 身份发请求），必须进入代码确认是否有 UID/签名校验。若组件将传入数据直接用于特权操作而无内部校验，即使声明了权限也可能存在风险。

评估利用可行性时检查：

1. **可达** — 攻击者能否到达该代码路径
2. **可控** — 攻击者能否控制关键数据
3. **有影响** — 能否造成实际安全后果

三项全部满足 ≠ 自动有利用，还需在报告中描述攻击路径。无法构造攻击路径 → 不报告。

确认漏洞后，使用 skill `jiapcli-poc` 构造可编译的 PoC 应用进行验证。

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
