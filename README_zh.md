# JIAP - Java智能分析平台

<div align="center">

![JIAP Logo](https://img.shields.io/badge/JIAP-Java%20Intelligence%20Analysis%20Platform-blue?style=for-the-badge&logo=java&logoColor=white)
![Release](https://img.shields.io/github/v/release/jygzyc/jiap?style=for-the-badge&logo=github&color=green)
![License](https://img.shields.io/github/license/jygzyc/jiap?style=for-the-badge&logo=gnu&color=orange)

**基于JADX的Java智能分析平台 - 为AI辅助代码分析而设计**

</div>

---

## 项目概述

JIAP (Java Intelligence Analysis Platform) 是一个基于JADX反编译器的智能代码分析平台，专门为AI辅助代码分析而设计。该平台通过HTTP API和MCP (Model Context Protocol) 协议，为AI助手提供强大的Java代码分析能力。

---

## 快速开始

### 环境要求

- **Java**: JDK 11+ （用于 JIAP Core）
- **Python**: 3.10+ （可选 - 自动管理）
- **JADX**: v1.5.2+ 支持插件的 JADX 反编译器
- **Python 依赖**: `requests`, `fastmcp`, `pydantic`（自动安装）

### 安装

```bash
# 1. 在 JADX GUI 中安装插件
# JADX -> Settings -> Plugins -> Install JIAP

# 或使用命令行安装
jadx plugins --install-jar <path-to-jiap.jar>

# 2. 构建 JIAP Core（从源码）
cd jiap
chmod +x gradlew
./gradlew dist
```

**MCP 服务器自动管理：**
- 插件会自动提取并管理 MCP 服务器脚本
- 无需手动安装 Python 依赖或启动 MCP 服务器
- 无需手动配置环境变量

### 使用说明

* 启动 JIAP 插件

  - 启动 JADX 并启用 JIAP 插件
  - 插件会自动启动 HTTP 服务器，MCP 服务器可手动确认启动
  - 确认服务器运行在 `http://127.0.0.1:25419`

**自动执行流程：**
1. JIAP 插件启动 HTTP 服务器（端口 `25419`）
2. 插件自动提取 MCP 脚本到 `~/.jiap/mcp/`
3. 若启用自动启动，伴生进程（MCP 服务器）将自动启动（端口 `25419 + 1`）
4. 两个进程在关闭时协同停止

* 验证连接

使用 `health_check()` 验证 MCP 服务器与 JIAP 插件之间的连接

* 可用工具

所有工具均支持通过 `page` 参数进行分页。

**代码分析**
- `get_all_classes(page=1)` - 获取所有可用类
- `search_class_key(key, page=1)` - 搜索源代码中包含指定关键字的类（不区分大小写）
- `get_class_source(class_name, smali=False, page=1)` - 获取 Java 或 Smali 格式的类源代码
- `search_method(method_name, page=1)` - 搜索匹配给定方法名的方法
- `get_method_source(method_name, smali=False, page=1)` - 获取方法源代码
- `get_class_info(class_name, page=1)` - 获取类信息，包括字段和方法
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
- `get_exported_components(page=1)` - 获取导出的组件（Activity、Service、Receiver、Provider）及权限
- `get_deep_links(page=1)` - 获取应用的 URL Schemes 和 Intent Filters
- `get_all_resources(page=1)` - 列出所有资源文件名（包括 resources.arsc 子文件）
- `get_resource_file(resource_name, page=1)` - 按名称获取资源文件内容
- `get_strings(page=1)` - 获取应用 strings.xml 内容
- `get_dynamic_receivers(page=1)` - 获取动态注册的 BroadcastReceivers
- `get_system_service_impl(interface_name, page=1)` - 获取系统服务实现

**系统**
- `health_check()` - 验证服务器连接状态

### 配置说明

**端口配置：**
- **GUI 方式**：JIAP Server Status 菜单 → 设置新端口 → 自动重启
- **插件选项**：在 JADX 插件选项中设置 `jiap.port`
- **默认值**：`25419`（JIAP）

**MCP 脚本路径：**
- **GUI 方式**：JIAP Server Status 菜单 → 浏览并选择自定义脚本
- **插件选项**：在 JADX 插件选项中设置 `jiap.mcp_path` 为自定义脚本路径
- **默认值**：自动提取到 `~/.jiap/mcp/jiap_mcp_server.py`

**伴生进程配置：**
```bash
# 自动检测执行器：uv、python3 或 python
# 自动提取脚本到 ~/.jiap/mcp/
# 自动启动并配置正确的 JIAP_URL 和 MCP_PORT
```

**缓存配置：**
JIAP 支持两种缓存模式以提升性能：
- **disk**（默认）：将反编译缓存持久化到磁盘（`~/.jiap/cache/`）
- **memory**：仅在内存中保留缓存，适用于小型项目

**配置方式：**
- **插件选项**：在 JADX 插件选项中设置 `jiap.cache` 为 `disk` 或 `memory`
- **默认值**：`disk`，以在后续运行中提供更好的性能

**性能优化：**
JIAP 包含自动性能优化：

**反编译器预热：**
- JIAP 启动时自动预热反编译器引擎
- 过滤掉 SDK 包（android.*、androidx.*、java.*、javax.*、kotlin.*）
- 随机采样最多 15,000 个应用类
- 确保后续查询的最佳性能

**磁盘缓存：**
- 反编译的代码缓存到磁盘以实现更快检索
- 缓存跨 JADX 会话持久化
- 大幅缩短大型项目的分析时间

### 错误码说明

JIAP 使用结构化的错误码进行清晰的诊断：

| 错误码 | 描述 | 常见原因 |
|--------|------|----------|
| **E001** | 内部服务器错误 | 意外的服务器状态 |
| **E002** | 服务错误 | 通用服务错误 |
| **E003** | 健康检查失败 | 无法连接 JIAP 服务器 |
| **E004** | 方法未找到 | 请求的方法不存在 |
| **E005** | 参数无效 | 参数格式/值无效 |

**错误响应格式：**
```json
{
  "error": "E001",
  "message": "内部错误: 启动失败"
}
```

---

## CLI 命令行工具

JIAP 提供了一个 TypeScript CLI 工具，用于通过命令行访问分析平台。

**安装：**

```bash
cd cli
npm install
npm run build
npm link  # 将 'jiap' 命令注册为全局命令
```

**进程管理：**
- `jiap process check` - 检查 JIAP 服务器状态
- `jiap process open <file>` - 打开并分析文件（APK、DEX、JAR 等）
- `jiap process close [name]` - 按会话名停止 JIAP 服务器
- `jiap process list` - 列出运行中的进程
- `jiap process install` - 安装或更新 jiap-server.jar

**代码分析：**
- `jiap code all-classes` - 获取所有类
- `jiap code class-info <class>` - 获取类信息
- `jiap code class-source <class>` - 获取类源代码
- `jiap code search-class <keyword>` - 搜索类内容
- `jiap code search-method <name>` - 按名称搜索方法
- `jiap code method-source <signature>` - 获取方法源代码
- `jiap code xref-method <signature>` - 查找方法调用者
- `jiap code xref-class <class>` - 查找类使用位置
- `jiap code xref-field <field>` - 查找字段使用位置
- `jiap code implement <interface>` - 查找接口实现
- `jiap code subclass <class>` - 查找子类

**Android 分析：**
- `jiap ard app-manifest` - 获取 AndroidManifest.xml
- `jiap ard main-activity` - 获取主 Activity 名称
- `jiap ard app-application` - 获取 Application 类名
- `jiap ard exported-components` - 列出导出组件
- `jiap ard app-deeplinks` - 列出深度链接
- `jiap ard receivers` - 列出动态广播接收器
- `jiap ard system-service-impl <interface>` - 查找系统服务实现
- `jiap ard all-resources` - 列出所有资源文件名
- `jiap ard resource-file <res>` - 按名称获取资源文件内容
- `jiap ard strings` - 获取 strings.xml 内容

---

## 开发

### 从源码构建

```bash
# 构建 JIAP Core
cd jiap
chmod +x gradlew
./gradlew dist

# 构建 CLI
cd cli
npm install
npm run build
```

### 增加自定义功能

JIAP 的架构分为三层：`JiapApi` 接口定义 → `JiapApiImpl` 实现 → `RouteHandler` HTTP 路由分发。

**1. 在 `JiapApi` 接口中添加方法**

文件：`jiap/jiap-core/src/main/kotlin/jadx/plugins/jiap/api/JiapApi.kt`

```kotlin
interface JiapApi {
    // ... 已有方法 ...

    fun doSomething(param: String): JiapApiResult
}
```

**2. 在 `JiapApiImpl` 中实现方法**

文件：`jiap/jiap-core/src/main/kotlin/jadx/plugins/jiap/api/JiapApiImpl.kt`

```kotlin
override fun doSomething(param: String): JiapApiResult {
    val result = // 业务逻辑
    return JiapApiResult.ok(mapOf("data" to result))
}
```

**3. 在 `JiapServer.ALL_ROUTES` 中注册路由，在 `RouteHandler.dispatch()` 中添加分发**

文件：`jiap/jiap-core/src/main/kotlin/jadx/plugins/jiap/http/JiapServer.kt`

```kotlin
val ALL_ROUTES = setOf(
    // ... 已有路由 ...
    "/api/jiap/do_something",
)
```

文件：`jiap/jiap-core/src/main/kotlin/jadx/plugins/jiap/http/RouteHandler.kt`

```kotlin
"/api/jiap/do_something" -> requireParam(payload, "param") { api.doSomething(it) }
```

### 故障排查

**伴生进程问题：**
- **查看日志**：在 JIAP 日志中查找 `[MCP]` 消息
- **验证 Python**：确保已安装 Python 3.10+ 或 `uv`
- **检查依赖**：插件会自动检查 `requests`、`fastmcp`、`pydantic`
- **手动路径**：如有需要，可通过 GUI 配置自定义脚本路径

**连接问题：**
- 使用 `health_check()` 验证两个服务器是否都在运行
- 检查端口冲突：`lsof -i :25419`
- 验证防火墙是否允许 localhost 连接

**常见错误：**
- **E001**：查看 JIAP 日志中的内部错误信息
- **E003**：确保 JIAP 插件已启用并加载
- **E005**：检查参数格式和值

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

![Star History](https://img.shields.io/github/stars/jygzyc/jiap?style=social)

</div>