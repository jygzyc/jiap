# DECX - Decompiler + X

<div align="center">

![DECX Logo](https://img.shields.io/badge/DECX-Decompiler%20%2B%20X-blue?style=for-the-badge&logo=java&logoColor=white)
![Release](https://img.shields.io/github/v/release/jygzyc/decx?style=for-the-badge&logo=github&color=green)
![License](https://img.shields.io/github/license/jygzyc/decx?style=for-the-badge&logo=gnu&color=orange)

**基于 JADX 的 Decompiler + X — 为 AI 辅助代码分析而设计**

</div>

---

## 项目概述

DECX (Decompiler + X) 是一个基于JADX反编译器的智能代码分析平台，专门为AI辅助代码分析而设计。该平台通过HTTP API和MCP (Model Context Protocol) 协议，为AI助手提供强大的Java代码分析能力。

---

## 快速开始

### 环境要求

- **Java**: JDK 11+ （用于 DECX Core）
- **Python**: 3.10+ （可选 - 自动管理）
- **JADX**: v1.5.2+ 支持插件的 JADX 反编译器
- **Python 依赖**: `requests`, `fastmcp`, `pydantic`（自动安装）

### 安装

```bash
# 1. 在 JADX GUI 中安装插件
# JADX -> Settings -> Plugins -> Install Decx

# 或使用命令行安装
jadx plugins --install-jar <path-to-decx.jar>

# 2. 构建 DECX Core（从源码）
cd decx
chmod +x gradlew
./gradlew dist
```

**MCP 服务器自动管理：**
- 插件会自动提取并管理 MCP 服务器脚本
- 无需手动安装 Python 依赖或启动 MCP 服务器
- 无需手动配置环境变量

### 使用说明

* 启动 DECX 插件

  - 启动 JADX 并启用 DECX 插件
  - 插件会自动启动 HTTP 服务器，MCP 服务器可手动确认启动
  - 确认服务器运行在 `http://127.0.0.1:25419`

**自动执行流程：**
1. DECX 插件启动 HTTP 服务器（端口 `25419`）
2. 插件自动提取 MCP 脚本到 `~/.decx/mcp/`
3. 若启用自动启动，伴生进程（MCP 服务器）将自动启动（端口 `25419 + 1`）
4. 两个进程在关闭时协同停止

* 验证连接

使用 `health_check()` 验证 MCP 服务器与 DECX 插件之间的连接

* 可用工具

所有工具均支持通过 `page` 参数进行分页。

**代码分析**
- `get_all_classes(first=None, include_packages=None, exclude_packages=None, page=1)` - 获取类列表，支持包过滤
- `search_global_key(key, first=None, max_results, include_packages=None, exclude_packages=None, case_sensitive=False, regex=True, page=1)` - 按候选项、包和结果上限搜索所有类源码
- `search_class_key(class_name, key, max_results, case_sensitive=False, regex=True, page=1)` - 在单个类中 grep，并返回命中行和方法签名
- `get_class_source(class_name, smali=False, page=1)` - 获取 Java 或 Smali 格式的类源代码
- `search_method(method_name, page=1)` - 搜索匹配给定方法名的方法
- `get_method_source(method_name, smali=False, page=1)` - 获取方法源代码
- `get_class_context(class_name, page=1)` - 获取类信息，包括字段和方法
- `get_method_context(method_name, page=1)` - 获取方法签名、caller 和 callee
- `get_method_cfg(method_name, page=1)` - 获取方法控制流图摘要
- `get_method_xref(method_name, page=1)` - 查找方法使用位置
- `get_field_xref(field_name, page=1)` - 查找字段使用位置
- `get_class_xref(class_name, page=1)` - 查找类使用位置
- `get_implement(interface_name, page=1)` - 获取接口实现
- `get_sub_classes(class_name, page=1)` - 获取子类

**UI 集成**
- `selected_text(page=1)` - 获取 JADX GUI 中当前选中的文本
- `selected_class(page=1)` - 获取 JADX GUI 中当前选中的类

**Android 分析**
- `get_app_manifest(page=1)` - 获取 Android 清单内容
- `get_main_activity(page=1)` - 获取主 Activity 源代码
- `get_application(page=1)` - 获取 Android 应用类及其信息
- `get_exported_components(component_types=None, regex=True, page=1)` - 获取导出组件，支持按组件类型正则过滤
- `get_deep_links(page=1)` - 获取应用的 URL Schemes 和 Intent Filters
- `get_all_resources(page=1)` - 列出所有资源文件名（包括 resources.arsc 子文件）
- `get_resource_file(resource_name, page=1)` - 按名称获取资源文件内容
- `get_strings(page=1)` - 获取应用 strings.xml 内容
- `get_dynamic_receivers(first=None, include_packages=None, exclude_packages=None, regex=True, page=1)` - 获取动态注册的 BroadcastReceivers，支持包过滤
- `get_aidl(first=None, include_packages=None, exclude_packages=None, regex=True, page=1)` - 获取 AIDL 接口及其实现类，支持包过滤
- `get_system_service_impl(interface_name, page=1)` - 获取系统服务实现

**系统**
- `health_check()` - 验证服务器连接状态

### 配置说明

**端口配置：**
- **GUI 方式**：DECX Server Status 菜单 → 设置新端口 → 自动重启
- **插件选项**：在 JADX 插件选项中设置 `decx.port`
- **默认值**：`25419`（Decx）

**MCP 脚本路径：**
- **GUI 方式**：DECX Server Status 菜单 → 浏览并选择自定义脚本
- **插件选项**：在 JADX 插件选项中设置 `decx.mcp_path` 为自定义脚本路径
- **默认值**：自动提取到 `~/.decx/mcp/decx_mcp_server.py`

**伴生进程配置：**
```bash
# 自动检测执行器：uv、python3 或 python
# 自动提取脚本到 ~/.decx/mcp/
# 自动启动并配置正确的 DECX_URL 和 MCP_PORT
```

**缓存配置：**
DECX 支持两种缓存模式以提升性能：
- **disk**（默认）：将反编译缓存持久化到磁盘（`~/.decx/cache/`）
- **memory**：仅在内存中保留缓存，适用于小型项目

**配置方式：**
- **插件选项**：在 JADX 插件选项中设置 `decx.cache` 为 `disk` 或 `memory`
- **默认值**：`disk`，以在后续运行中提供更好的性能

**性能优化：**
DECX 包含自动性能优化：

**反编译器预热：**
- DECX 启动时自动预热反编译器引擎
- 过滤掉 SDK 包（android.*、androidx.*、java.*、javax.*、kotlin.*）
- 随机采样最多 15,000 个应用类
- 确保后续查询的最佳性能

**磁盘缓存：**
- 反编译的代码缓存到磁盘以实现更快检索
- 缓存跨 JADX 会话持久化
- 大幅缩短大型项目的分析时间

### 错误码说明

DECX 使用结构化的错误码进行清晰的诊断：

| 错误码 | 描述 | HTTP 状态码 |
|--------|------|-------------|
| **INTERNAL_ERROR** | 内部服务器错误 | 500 |
| **SERVICE_ERROR** | 服务错误 | 503 |
| **HEALTH_CHECK_FAILED** | 健康检查失败 | 500 |
| **UNKNOWN_ENDPOINT** | 未知端点 | 404 |
| **INVALID_PARAMETER** | 参数无效 | 400 |
| **METHOD_NOT_FOUND** | 方法未找到 | 404 |
| **CLASS_NOT_FOUND** | 类未找到 | 404 |
| **RESOURCE_NOT_FOUND** | 资源未找到 | 404 |
| **MANIFEST_NOT_FOUND** | AndroidManifest 未找到 | 404 |
| **FIELD_NOT_FOUND** | 字段未找到 | 404 |
| **INTERFACE_NOT_FOUND** | 接口未找到 | 404 |
| **SERVICE_IMPL_NOT_FOUND** | 服务实现未找到 | 404 |
| **NO_STRINGS_FOUND** | 未找到 strings.xml 资源 | 404 |
| **NO_MAIN_ACTIVITY** | 未找到 MAIN/LAUNCHER Activity | 404 |
| **NO_APPLICATION** | 未找到 Application 类 | 404 |
| **EMPTY_SEARCH_KEY** | 搜索关键字不能为空 | 400 |
| **NOT_GUI_MODE** | 非 GUI 模式 | 503 |

**错误响应格式：**
```json
{
  "ok": false,
  "error": {
    "code": "CLASS_NOT_FOUND",
    "message": "Class not found: com.example.Foo"
  }
}
```

---

## CLI 命令行工具

DECX 提供了一个 TypeScript CLI 工具，用于通过命令行访问分析平台。

**安装：**

```bash
npm install -g @jygzyc/decx-cli
```

**进程管理：**
- `decx process check` - 检查 DECX 服务器状态
- `decx process open <file>` - 打开并分析文件（APK、DEX、JAR 等）；默认启用 `--show-bad-code`
- `decx process close [name]` - 按会话名停止 DECX 服务器
- `decx process list` - 列出运行中的进程

**代码分析：**
- `decx code all-classes` - 获取类列表（支持 `--first`、`--include-package`、`--exclude-package`）
- `decx code class-context <class>` - 获取类信息
- `decx code class-source <class>` - 获取类源代码（`--smali` 输出 Smali 格式）
- `decx code search-global <keyword> --max-results <n>` - 全局搜索（支持 `--first`、`--include-package`、`--exclude-package`、`--no-regex`、`--case-sensitive`）
- `decx code search-class <class> <pattern> --max-results <n>` - 在单个类中 grep（支持 `--no-regex`、`--case-sensitive`）
- `decx code search-method <name>` - 按名称搜索方法
- `decx code method-source <signature>` - 获取方法源代码（`--smali` 输出 Smali 格式）
- `decx code method-context <signature>` - 获取方法签名、caller 和 callee
- `decx code method-cfg <signature>` - 获取方法控制流图摘要
- `decx code xref-method <signature>` - 查找方法调用者
- `decx code xref-class <class>` - 查找类使用位置
- `decx code xref-field <field>` - 查找字段使用位置
- `decx code implement <interface>` - 查找接口实现
- `decx code subclass <class>` - 查找子类

**Android 分析：**
- `decx ard app-manifest` - 获取 AndroidManifest.xml
- `decx ard main-activity` - 获取主 Activity 名称
- `decx ard app-application` - 获取 Application 类名
- `decx ard exported-components [--type <pattern>] [--no-regex]` - 列出导出组件，可按类型过滤
- `decx ard app-deeplinks` - 列出深度链接
- `decx ard app-receivers [--first <n>] [--include-package <pattern>] [--exclude-package <pattern>] [--no-regex]` - 列出动态广播接收器，支持包过滤
- `decx ard system-service-impl <interface>` - 查找系统服务实现
- `decx ard system-services [--serial <serial>] [--grep <keyword>]` - 以结构化 JSON 列出当前设备上的系统服务
- `decx ard perm-info <permission> [--serial <serial>]` - 查看结构化权限详情
- `decx ard all-resources` - 列出所有资源文件名
- `decx ard resource-file <res>` - 按名称获取资源文件内容
- `decx ard strings` - 获取 strings.xml 内容
- `decx ard get-aidl [--first <n>] [--include-package <pattern>] [--exclude-package <pattern>] [--no-regex]` - 获取 AIDL 接口，支持包过滤

**自管理：**
- `decx self install` - 安装或更新 decx-server.jar（`-p` 安装预发布版）
- `decx self update` - 更新 decx-server.jar 和当前已安装的 npm CLI 包（`-p` 目前只影响 server JAR 更新路径）

所有基于会话的 `code` 和 `ard` 命令都支持 `--page <n>` 分页。
基于 adb 的 `system-services` 和 `perm-info` 不使用 `-P <port>`，而是使用 `--serial` / `--adb-path`。

`system-services` 返回结构化 JSON，包含 `total` 和 `services[]`；每个服务对象含 `index`、`name`、`interfaces`。
`perm-info` 返回单个已解析的权限对象，字段包括 `permission`、`package`、`label`、`description`、`protectionLevel` 等。

---

## AI Agent 技能

`skill/` 目录下包含 AI Agent 技能定义文件，支持自动化 Android 逆向分析、漏洞挖掘和 PoC 构造。

### 可用技能

| 技能 | 说明 | 依赖 |
|------|------|------|
| **decxcli** | 通用分析：代码导航、交叉引用、Manifest/资源检查 | `decx` |
| **decxcli-app-vulnhunt** | App 漏洞挖掘：APK 攻击面枚举、组件/WebView/IPC 追踪、可利用性评估、中英文报告生成 | `decx` |
| **decxcli-framework-vulnhunt** | Framework 漏洞挖掘：Binder 服务枚举、framework JAR 追踪、权限门审计、可利用性评估、中英文报告生成 | `decx` |
| **decxcli-poc** | PoC 构造：漏洞归一化、Exploit 类实现、可选编译部署 | `decx`, `node` |

技能按顺序协作：`decxcli`（分析） → `decxcli-app-vulnhunt` 或 `decxcli-framework-vulnhunt`（漏洞挖掘） → `decxcli-poc`（PoC 构造）。

### 前置依赖

使用任何技能前，请先安装 DECX CLI：

```bash
npm install -g @jygzyc/decx-cli
```

### 安装

__注意：__ 安装方式因平台而异。

**Claude Code**

```bash
cp -r skill/decxcli ~/.claude/skills/
cp -r skill/decxcli-app-vulnhunt ~/.claude/skills/
cp -r skill/decxcli-framework-vulnhunt ~/.claude/skills/
cp -r skill/decxcli-poc ~/.claude/skills/
```

**Cursor**

```bash
cp skill/decxcli/SKILL.md .cursor/rules/decxcli.md
cp skill/decxcli-app-vulnhunt/SKILL.md .cursor/rules/decxcli-app-vulnhunt.md
cp skill/decxcli-framework-vulnhunt/SKILL.md .cursor/rules/decxcli-framework-vulnhunt.md
cp skill/decxcli-poc/SKILL.md .cursor/rules/decxcli-poc.md
```

**Cline**

```bash
cp skill/decxcli/SKILL.md .clinerules-decxcli
cp skill/decxcli-app-vulnhunt/SKILL.md .clinerules-decxcli-app-vulnhunt
cp skill/decxcli-framework-vulnhunt/SKILL.md .clinerules-decxcli-framework-vulnhunt
cp skill/decxcli-poc/SKILL.md .clinerules-decxcli-poc
```

**Windsurf**

```bash
cp skill/decxcli/SKILL.md .windsurfrules-decxcli
cp skill/decxcli-app-vulnhunt/SKILL.md .windsurfrules-decxcli-app-vulnhunt
cp skill/decxcli-framework-vulnhunt/SKILL.md .windsurfrules-decxcli-framework-vulnhunt
cp skill/decxcli-poc/SKILL.md .windsurfrules-decxcli-poc
```

---

## 开发

### 从源码构建

```bash
# 构建 DECX Core
cd decx
chmod +x gradlew
./gradlew dist

# 构建 CLI
cd cli
npm install
npm run build
```

### 增加自定义功能

DECX 的架构分为三层：`DecxApi` 接口定义 → `DecxApiImpl` 实现 → `RouteHandler` HTTP 路由分发。

**1. 在 `DecxApi` 接口中添加方法**

文件：`decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxApi.kt`

```kotlin
interface DecxApi {
    // ... 已有方法 ...

    fun doSomething(param: String): DecxApiResult
}
```

**2. 在 `DecxApiImpl` 中实现方法**

文件：`decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxApiImpl.kt`

```kotlin
override fun doSomething(param: String): DecxApiResult {
    val result = // 业务逻辑
    return DecxApiResult.ok(mapOf("data" to result))
}
```

**3. 在 `DecxRoutes` 中注册路由**

文件：`decx/decx-core/src/main/kotlin/jadx/plugins/decx/api/DecxApiContract.kt`

```kotlin
DecxRoute("/api/decx/do_something", "do_something") { api, params ->
    api.doSomething(params.requireString("param"))
}
```

### 故障排查

**伴生进程问题：**
- **查看日志**：在 DECX 日志中查找 `[MCP]` 消息
- **验证 Python**：确保已安装 Python 3.10+ 或 `uv`
- **检查依赖**：插件会自动检查 `requests`、`fastmcp`、`pydantic`
- **手动路径**：如有需要，可通过 GUI 配置自定义脚本路径

**连接问题：**
- 使用 `health_check()` 验证两个服务器是否都在运行
- 检查端口冲突：`lsof -i :25419`
- 验证防火墙是否允许 localhost 连接

**常见错误：**
- **INTERNAL_ERROR**：查看 DECX 日志中的内部错误信息
- **HEALTH_CHECK_FAILED**：确保 DECX 插件已启用并加载
- **INVALID_PARAMETER**：检查参数格式和值

## 贡献

1. Fork 本仓库
2. 创建功能分支
3. 进行更改
4. 如适用，添加测试
5. 提交 Pull Request

---

## 许可证

本项目采用 [GNU许可证](LICENSE) - 详见 [LICENSE](LICENSE) 文件。

---

## 致谢

- **[skylot/jadx](https://github.com/skylot/jadx)** - 本项目的基础，强大的 JADX 反编译器，提供插件支持
- **[zinja-coder/jadx-ai-mcp](https://github.com/zinja-coder/jadx-ai-mcp)** - 为本项目提供了很多思路和灵感，关于 JADX MCP 集成的优秀实践
- **[FastMCP](https://github.com/modelcontextprotocol/servers)**: MCP协议实现
- **[Javalin](https://javalin.io/)**: 轻量级Web框架

---

<div align="center">

**⭐ 如果这个项目对您有帮助，请给一个Star！**

![Star History](https://img.shields.io/github/stars/jygzyc/decx?style=social)

</div>
