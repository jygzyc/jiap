---
name: jiapcli-vulnhunt
description: Android 漏洞挖掘专用 skill。基于 JIAP CLI + JADX，提供组件级攻击面分析、可利用性评估、漏洞链构造方法论和风险评级参考。当用户提到漏洞挖掘、vulnerability hunting、exploit chain、攻击面分析、attack surface、安全审计、security audit、风险评级时使用。
metadata:
  requires:
    bins: ["jiap"]
---

# JIAP CLI — Android 漏洞挖掘

Android 组件级漏洞挖掘方法论。漏洞发现和评估由本 skill 完成，PoC 构造交由 `jiapcli-poc`。

命令参考见 `jiapcli`。

## 命令格式强制规则

> 来自 `jiapcli` skill，必须遵守：
> - 所有 `jiap code` / `jiap ard` 命令必须携带 `-P <port>`
> - 方法签名格式：`"package.Class.methodName(paramType1,paramType2):returnType"`，**禁止 `...`**，**必须用引号包裹**（括号/冒号会导致 shell 解析错误）
> - 不确定命令时先 `--help`，不要猜测

## 漏洞挖掘流程

### Phase 1：环境准备

```
jiap process open "<apk-path>" -P <port>  → 打开 APK（端口建议 25000–65535）
jiap process status -P <port>              → 确认服务正常
```

分析完成后不要自动关闭 session，告知用户：`jiap process close "<name>" -P <port>`

### Phase 2：攻击面枚举（侦察 Subagent）

创建一个侦察 Subagent 完成全部信息收集，主 agent **不执行任何 `jiap` 命令**，仅接收结构化输出。

#### 2.1 侦察 Subagent 输入

```json
{
  "port": 31234,
  "task": "枚举攻击面，返回需要分析的组件与入口"
}
```

#### 2.2 侦察 Subagent 执行步骤

按顺序执行以下命令，收集全部攻击面信息：

```
jiap ard exported-components -P <port>  → 导出组件列表
jiap ard app-deeplinks -P <port>        → Deep Link 入口
jiap ard app-receivers -P <port>        → 动态广播接收器
jiap ard app-manifest -P <port>         → 完整 Manifest
jiap ard get-aidl -P <port>            → AIDL 接口
jiap ard strings -P <port>              → 硬编码字符串
```

#### 2.3 侦察 Subagent 内部处理

1. **权限保护判定** — 回查 Manifest 中权限的 `protectionLevel`：

   | protectionLevel | 判定 |
   |----------------|------|
   | `normal` | **无保护**（等同于无权限） |
   | `dangerous` | **弱保护**（需用户动态授权） |
   | `signature` / `signatureOrSystem` | **受保护**（第三方不可满足） |

2. **过滤** — 排除 `signature` / `signatureOrSystem` 权限保护的组件（第三方不可达）

3. **分类** — 按 Activity / Service / ContentProvider / BroadcastReceiver / WebView / AIDL 接口 / 系统服务 / Deep Link 入口归类

#### 2.4 侦察 Subagent 输出

返回结构化 JSON，**禁止在输出中粘贴原始命令输出**：

```json
{
  "appInfo": {
    "packageName": "com.example.app",
    "versionName": "1.0.0",
    "minSdk": 21,
    "targetSdk": 33,
    "usesPermissions": ["android.permission.INTERNET", "..."]
  },
  "components": [
    {
      "className": "com.example.app.MyActivity",
      "type": "Activity",
      "exported": true,
      "permission": null,
      "protectionLevel": null,
      "intentFilters": ["android.intent.action.VIEW"],
      "deepLinks": ["example://callback"],
      "needsAnalysis": true,
      "reason": "导出 Activity，无权限保护，接受外部 Intent"
    },
    {
      "className": "com.example.app.InternalService",
      "type": "Service",
      "exported": true,
      "permission": "com.example.app.INTERNAL",
      "protectionLevel": "signature",
      "needsAnalysis": false,
      "reason": "signature 权限保护，第三方不可达"
    }
  ],
  "aidlInterfaces": [
    {
      "interfaceName": "com.example.app.IRemoteService",
      "implementingClass": "com.example.app.RemoteServiceImpl",
      "needsAnalysis": true,
      "reason": "通过 onBind 暴露的 AIDL 接口"
    }
  ],
  "deepLinkEntries": [
    {
      "scheme": "example",
      "host": "callback",
      "targetActivity": "com.example.app.DeepLinkActivity",
      "needsAnalysis": true,
      "reason": "Deep Link 入口，接受外部 URI 数据"
    }
  ],
  "notableStrings": [
    "http://api.example.com/debug",
    "file:///data/data/com.example.app/"
  ],
  "summary": "共发现 12 个导出组件，其中 8 个需要分析（4 个被 signature 权限排除）。重点关注：3 个无保护导出 Activity、2 个 ContentProvider（1 个无权限）、1 个暴露的 AIDL 接口。"
}
```

- `needsAnalysis`：仅 `true` 的组件进入 Phase 3 分析
- `needsAnalysis` 为 `false` 的组件附带排除原因，主 agent 据此跳过

#### 2.5 行为规则

- 侦察 subagent 执行全部 `jiap` 命令并完成分析，主 agent **不直接调用任何 Phase 2 命令**
- 所有命令携带 `-P <port>`
- 输出仅返回结构化 JSON，**禁止粘贴原始命令输出或源码**
- 主 agent 收到输出后直接进入 Phase 3

### Phase 3：逐组件深度分析（Subagent 链）

主 agent 接收 Phase 2 侦察 Subagent 的输出，仅对 `needsAnalysis: true` 的组件启动分析，多个组件可并发。主 agent 仅收集结构化摘要，不堆积原始源码。

#### 3.1 组件类型配置

| 组件类型 | 总览文件 | 入口方法 |
|---------|---------|---------|
| Activity | `references/app-activity.md` | `onCreate` / `onNewIntent` |
| Service | `references/app-service.md` | `onBind` / `onStartCommand` / `handleMessage` |
| ContentProvider | `references/app-provider.md` | `query` / `insert` / `update` / `delete` / `openFile` / `call` |
| BroadcastReceiver | `references/app-broadcast.md` | `onReceive` |
| WebView | `references/app-webview.md` | 宿主 Activity 的 `onCreate` |
| 系统服务 | `references/framework-service.md` | `system-service-impl` 定位的实现类方法 |

主 agent 根据组件类型选择总览文件，从总览的风险清单确定 `vulnTypes`。

#### 3.2 两级 Subagent 职责

**侦察 Subagent**（每组件一个，负责全局判断）：

读取总览文件 + 组件源码，识别该组件涉及哪些漏洞模式，为每种模式确定入口方法和初始追踪方向。输出每条链的起始 `currentMethod` 和 `target`（SOURCE/SINK/BOTH）。

**链式 Subagent**（每条链多个，负责逐方法追踪）：

分析 `currentMethod` 指定的单个方法，判断 Source/Sink/SAFE，输出 `nextMethods`。主 agent 据此创建下一个链式 subagent，逐步展开整条调用链。

#### 3.3 链式 Subagent 输入

```json
{
  "port": 31234,
  "currentMethod": "com.example.MyActivity.extractNestedIntent(android.content.Intent):android.content.Intent",
  "chain": [
    {"method": "...onCreate(...):void", "finding": "调用 extractNestedIntent"},
    {"method": "...extractNestedIntent(...):Intent", "finding": "getParcelableExtra(\"forward_intent\")，无校验返回"}
  ],
  "target": "BOTH"
}
```

- `chain`：已追踪的方法及发现，逐步累积
- `target`：`"SOURCE"`（找攻击者可控入口）/ `"SINK"`（找危险操作）/ `"BOTH"`（双向追踪）

#### 3.4 链式 Subagent 执行步骤

```
1. jiap code method-source "<currentMethod>" -P <port>  → 获取该方法源码
2. 分析源码，执行以下判断：
```

**Source 判断**（`target` 为 SOURCE 或 BOTH 时）：
方法中是否存在攻击者可控的数据获取？
- `getIntent().get*Extra()` — Intent 参数
- `getIntent().getData()` — URI 参数
- `getContentResolver().query()` — Provider 查询参数
- `onBind()` 返回的 Binder 接口参数 — IPC 输入
- `Messenger.handleMessage()` 的 msg 参数 — 消息参数
- 方法参数本身来自上游 tainted data

**Sink 判断**（`target` 为 SINK 或 BOTH 时）：
方法中是否存在危险操作？
- `startActivity()` / `sendBroadcast()` / `startService()` — 组件启动
- `Runtime.exec()` / `ProcessBuilder` — 命令执行
- `openFileOutput()` / `deleteFile()` / `renameTo()` — 文件操作
- `setResult()` — 数据返回
- `PendingIntent.send()` — 身份借用
- `WebView.loadUrl()` — URL 加载（配合可控输入）
- `execSQL()` — SQL 执行

**SAFE 判断**（Source → Sink 路径上是否存在有效校验）：
- 包名/签名/UID 白名单校验且不可绕过
- 完整性校验、HMAC 签名验证
- 类型强校验（如 `instanceof` + 白名单类列表）
- `checkSignatures` / `checkCallingPermission` 等系统校验

**分支选择**（方法中有多个调用时，仅追踪与污点数据相关的调用）：
- 追踪：接收 tainted data 作为参数的方法调用
- 跳过：日志（`Log.*`）、UI 操作（`setText`/`setVisibility`）、纯工具方法（`toString`/`hashCode`）、框架回调（`super.*`）

#### 3.5 链式 Subagent 输出

```json
{
  "method": "com.example.MyActivity.extractNestedIntent(android.content.Intent):android.content.Intent",
  "analysis": "getParcelableExtra(\"forward_intent\") 提取嵌套 Intent，无任何校验直接返回",
  "result": "SOURCE",
  "nextMethods": [
    {
      "method": "com.example.MyActivity.startForwardActivity(android.content.Intent):void",
      "reason": "接收 extractNestedIntent 返回值并调用 startActivity"
    }
  ],
  "terminates": false
}
```

- `result`：`"SOURCE"` / `"SINK"` / `"SAFE"` / `"PASS_THROUGH"`（透传，需继续）/ `"DEAD_END"`（无进一步调用）
- `nextMethods`：需继续追踪的方法，为空则分支终止
- `terminates`：到达 SINK/SAFE/DEAD_END 时为 true

**终止条件**（任一满足）：到达 Sink / 到达 SAFE / 方法无进一步调用

#### 3.6 主 agent 聚合

链式追踪完成后，主 agent 将完整 chain 合并为组件级输出：

```json
{
  "component": "com.example.MyActivity",
  "type": "Activity",
  "exported": true,
  "permission": null,
  "findings": [
    {
      "vulnType": "Intent 重定向",
      "risk": "HIGH",
      "source": "getIntent().getParcelableExtra(\"forward_intent\") at onCreate:42",
      "sink": "startActivity(intent) at startForwardActivity:15",
      "callChain": [
        "com.example.MyActivity.onCreate(android.os.Bundle):void",
        "com.example.MyActivity.extractNestedIntent(android.content.Intent):android.content.Intent",
        "com.example.MyActivity.startForwardActivity(android.content.Intent):void"
      ],
      "exploitPath": "第三方 app 发送含嵌套 Intent → 提取后无校验直接 startActivity → 访问任意未导出组件",
      "exploitable": "YES"
    }
  ],
  "excluded": [
    {"vulnType": "Fragment 注入", "reason": "未从 extra 提取 Fragment 类名"}
  ]
}
```

#### 3.7 行为规则

- 链式 subagent 仅分析 `currentMethod`，使用 `method-source` 获取源码
- 侦察 subagent 读取 reference 文件指导分析方向
- 所有命令携带 `-P <port>`，方法签名完整，禁止 `...`
- **禁止在输出中粘贴原始源码**

### Phase 4：跨组件追踪（Subagent 链延续）

Phase 3 的链式追踪在组件内部完成。Phase 4 对以下情况**延续链式追踪到目标组件**：

- 调用链跨组件（如 Activity A 调用 Activity B 的方法 / Service onBind 返回的接口被外部调用）
- 调用链涉及系统 API（如 `Context.startActivity()` 需确认目标组件是否导出）
- PendingIntent / URI 权限跨组件传递

执行方式：将 Phase 3 链的最后一个 `nextMethod`（目标组件的方法）作为新的链式 subagent 入口，`chain` 携带已有上下文，继续追踪直到终止条件满足。

输出格式同 Phase 3 的组件级聚合 JSON。无新增跨组件发现时返回空 `findings`。

### Phase 5：可利用性评估（强制过滤）

**第一步：快速排除** — 符合任一条件立即排除，不写入任何文档：

| 排除条件 | 检查方法 |
|---------|---------|
| `signature`/`signatureOrSystem` 权限 | Manifest protectionLevel |
| 签名校验 | `checkSignatures`、`Binder.getCallingUid()` + 白名单 |
| 包名/UID 白名单 | 硬编码包名数组，不可绕过 |
| 需要 root/system 权限 | `su` 调用或 system_server 独占 API |
| Source → Sink 间不可绕过的校验 | 完整性校验、加密、类型检查 |

**第二步：三要素评估**（详见 `references/risk-rating.md`）：

1. **可达** — 组件导出？权限可满足？
2. **可控** — 攻击者能控制 Source → Sink 的关键数据？
3. **有影响** — 敏感数据泄露 / 权限提升 / 代码执行？

三项全满足 → 构造攻击路径。任一不满足 → **不报告**。

**权限校验效力**：
- 组件启动阶段：`signature` 权限由系统调度时校验，第三方不可绕过
- 运行时交互阶段（Provider query / Service onBind）：Manifest 权限仅控制谁能调用，**不控制组件内部数据处理**。组件以自身身份执行特权操作时，必须确认有无 UID/签名校验

### Phase 6：报告生成（报告 Subagent）

主 agent 将 Phase 3–5 的聚合结果提交给报告 Subagent，由其独立完成报告撰写。主 agent **不参与报告内容生成**，仅接收最终报告。

#### 6.1 报告 Subagent 输入

```json
{
  "appInfo": { "...来自 Phase 2 输出" },
  "componentFindings": [
    { "component": "...", "type": "...", "findings": [...] }
  ],
  "crossComponentFindings": [
    { "...来自 Phase 4 输出" }
  ],
  "port": 31234
}
```

#### 6.2 执行步骤

1. 读取 `assets/report-template.md` 和 `references/risk-rating.md`
2. 仅对存在 `findings` 的组件生成报告条目
3. 使用 `jiap code method-source "<method>" -P <port>` 回取关键代码片段（仅 Source / Sink / 校验缺失位置）
4. **严格按模板结构逐项填充，生成报告**

#### 6.3 格式约束（强制，零容忍）

> **报告必须与 `assets/report-template.md` 结构完全一致。不得增删改章节、不得合并、不得调整顺序。**

**报告骨架**（每层标题级别、字段名、分隔符均不可更改）：

```
# 模块安全分析报告
## 基本信息
  表格：Target App / Target Version / Android Version / Analysis Date
---
## 问题X：[Risk] 漏洞标题          ← Risk 取自 risk-rating.md，编号从一连续
  ### 1. 漏洞分析
    #### 漏洞背景                   ← 利用方案 + 最终影响
    #### 完整调用链                  ← 见下方格式
    #### 漏洞代码分析                ← 见下方格式
  ### 2. 攻击路径
    #### 目标组件                    ← 表格：Package / Class / Action or URI / IPC 接口
    #### 攻击步骤                    ← 编号列表，每步为第三方 app 可执行操作
  ### 3. 危害
  ### 4. 修复方案
---
（下一条漏洞，同上结构）
```

**调用链格式**（完整函数签名 + 箭头 + 缩进表示层级）：
```
com.target.EntryActivity.onCreate(android.os.Bundle):void  （入口）
  → getIntent().getParcelableExtra("forward_intent")
  → startActivity(nestedIntent)
    → com.target.InternalActivity.onCreate(android.os.Bundle):void
      → handleIntent(intent)
        → vulnerableOperation(data)
```

**代码分析格式**（每条严格三要素）：
```
1. **粗体问题描述**

\`\`\`java
// 关键代码片段
\`\`\`

缺陷说明。
```

**禁止出现在报告中**：不可利用的发现 / 安全组件列表 / 缺乏攻击路径的发现 / 中间 JSON / 任何模板外章节

#### 6.4 自检清单

输出前必须逐项通过：

- [ ] `# 模块安全分析报告` 一级标题
- [ ] `## 基本信息` 含全部 4 字段
- [ ] 每条漏洞含完整 4 个 `###` + 3 个 `####`（漏洞分析下）
- [ ] 调用链：完整函数签名 + 箭头 + 缩进
- [ ] 代码分析：编号 + 粗体 + 代码块 + 说明
- [ ] 攻击路径：目标组件表（4 字段）+ 攻击步骤列表
- [ ] 无模板外内容、无中间数据

#### 6.5 输出与后续

通过自检后返回完整 Markdown 报告。主 agent 收到后：

1. 传递给 `jiapcli-poc` 构造 PoC 验证（session 需保持运行）
2. 告知用户可通过 `jiap process close "<name>" -P <port>` 手动关闭 session，或直接使用 `jiapcli-poc` skill

## References

| 文件 | 组件 |
|------|------|
| `references/app-activity.md` | Activity |
| `references/app-intent.md` | Intent |
| `references/app-broadcast.md` | Broadcast |
| `references/app-provider.md` | ContentProvider |
| `references/app-service.md` | Service |
| `references/app-webview.md` | WebView |
| `references/framework-service.md` | 系统服务 |
| `references/risk-rating.md` | 风险评级 |
