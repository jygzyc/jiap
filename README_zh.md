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

- **Java**: JDK 17+ （用于 JIAP Core）
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
cd jiap_core
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
  - 插件会自动启动 HTTP 服务器和 MCP 伴生进程
  - 确认服务器运行在 `http://127.0.0.1:25419`

**伴生进程机制：**
- JIAP 插件启动时，会自动启动 MCP 服务器作为**伴生进程**
- MCP 服务器作为侧车进程（sidecar）由插件管理
- 无需手动启动 MCP 服务器
- 伴生进程会在 JIAP 插件卸载或 JADX 退出时自动停止

**自动执行流程：**
1. JIAP 插件启动 HTTP 服务器（端口 `25419`）
2. 插件自动提取 MCP 脚本到 `~/.jiap/mcp/`
3. 伴生进程（MCP 服务器）自动启动（端口 `25420`）
4. 插件监控伴生进程健康状态
5. 两个进程在关闭时协同停止

* 验证连接

使用 `health_check()` 验证 MCP 服务器与 JIAP 插件之间的连接

* 可用工具
  - `get_all_classes(page=1)` - 获取所有可用类，支持分页
  - `search_class_key(key, page=1)` - 搜索源代码中包含指定关键字的类（不区分大小写）
  - `get_class_source(class_name, smali=False, page=1)` - 获取 Java 或 Smali 格式的类源代码
  - `search_method(method_name, page=1)` - 搜索匹配给定方法名的方法
  - `get_method_source(method_name, smali=False, page=1)` - 获取方法源代码
  - `get_class_info(class_name, page=1)` - 获取类信息，包括字段和方法
  - `get_method_xref(method_name, page=1)` - 查找方法使用位置
  - `get_class_xref(class_name, page=1)` - 查找类使用位置
  - `get_implement(interface_name, page=1)` - 获取接口实现
  - `get_sub_classes(class_name, page=1)` - 获取子类
  - `selected_text(page=1)` - 获取 JADX GUI 中当前选中的文本
  - `selected_class(page=1)` - 获取 JADX GUI 中当前选中的类
  - `get_app_manifest(page=1)` - 获取 Android 清单内容
  - `get_main_activity(page=1)` - 获取主 Activity 源代码
  - `get_application(page=1)` - 获取 Android 应用类及其信息
  - `get_system_service_impl(interface_name, page=1)` - 获取系统服务实现

### 配置说明

**端口配置：**
- **GUI 方式**：JIAP Server Status 菜单 → 设置新端口 → 自动重启
- **插件选项**：在 JADX 插件选项中设置 `jiap.port`
- **默认值**：`25419`（JIAP），`25420`（MCP 伴生进程）

**MCP 脚本路径：**
- **GUI 方式**：JIAP Server Status 菜单 → 浏览并选择自定义脚本
- **插件选项**：在 JADX 插件选项中设置 `jiap.mcp_path` 为自定义脚本路径
- **默认值**：自动提取到 `~/.jiap/mcp/jiap_mcp_server.py`

**伴生进程配置：**
```bash
# 自动检测执行器：uv、python3 或 python
# 自动提取脚本到 ~/.jiap/mcp/
# 自动启动并配置正确的 JIAP_URL 和 MCP_PORT
# 自动监控并在需要时重启
```

### 错误码说明

JIAP 使用结构化的错误码进行清晰的诊断：

| 错误码 | 描述 | 常见原因 |
|--------|------|----------|
| **E001** | 内部服务器错误 | 意外的服务器状态 |
| **E002** | 端口被占用 | 其他服务正在使用该端口 |
| **E003** | 服务器启动失败 | 端口绑定或初始化错误 |
| **E004** | 服务器停止失败 | 优雅关闭超时 |
| **E005** | 服务器重启失败 | 重启序列错误 |
| **E006** | JADX 不可用 | 反编译器未初始化 |
| **E007** | JADX 初始化失败 | 反编译器初始化错误 |
| **E008** | 未找到 Python | 未找到 Python/uv 可执行文件 |
| **E009** | 侧车脚本未找到 | MCP 脚本提取失败 |
| **E010** | 侧车启动失败 | 伴生进程启动失败 |
| **E011** | 侧车进程错误 | 伴生进程崩溃 |
| **E012** | 侧车停止失败 | 伴生进程无法停止 |
| **E013** | 服务错误 | 通用服务错误 |
| **E014** | 健康检查失败 | 无法连接 JIAP 服务器 |
| **E015** | 方法未找到 | 请求的方法不存在 |
| **E016** | 缺少参数 | 未提供必需参数 |
| **E017** | 参数无效 | 参数格式/值无效 |
| **E018** | 未知端点 | 请求的 API 端点不存在 |
| **E019** | 连接错误 | 网络/HTTP 连接失败 |

**错误响应格式：**
```json
{
  "error": "E010",
  "message": "侧车启动失败: 未找到 Python 可执行文件"
}
```

---

## 开发

### 从源码构建

```bash
# 构建 JIAP Core
cd jiap_core
chmod +x gradlew
./gradlew dist

# 可选：测试 MCP Server（用于开发）
cd mcp_server
python jiap_mcp_server.py --jiap-url "http://127.0.0.1:25419"
```

### 增加自定义功能

在`jadx/plugins/jiap/service`下创建自定义`service`，实现`JiapServiceInterface`, 其中接口的实现返回值均为`JiapResult`

```kotlin
class CustomService(override val pluginContext: JadxPluginContext) : JiapServiceInterface {
    fun doSomething(): JiapResult {
        //...
    }
}
```

在`jadx/plugins/jiap/core/JiapConfig.kt`中注册自定义`service`，并在`routeMap`中添加路由映射即可实现自定义接口功能

```kotlin
// Service instances
val commonService: CommonService = CommonService(pluginContext)
val androidFrameworkService: AndroidFrameworkService = AndroidFrameworkService(pluginContext)
val androidAppService: AndroidAppService = AndroidAppService(pluginContext)
val customService: CustomService = CustomService(pluginContext) // 注册自定义功能

// Route mappings
val routeMap: Map<String, RouteTarget>
get() = mapOf(
    // Common Service
    "/api/jiap/get_all_classes" to RouteTarget(
        service = commonService,
        methodName = "handleGetAllClasses",
        cacheable = true
    ),
    //...
    // Custom Service
    "/api/jiap/custom_service/do_something" to RouteTarget(
        service = customService,
        methodName = "doSomething",
    ),
    //...
)
```

### 故障排查

**伴生进程问题：**
- **查看日志**：在 JIAP 日志中查找 `[MCP Sidecar STDOUT]` 消息
- **验证 Python**：确保已安装 Python 3.10+ 或 `uv`
- **检查依赖**：插件会自动检查 `requests`、`fastmcp`、`pydantic`
- **手动路径**：如有需要，可通过 GUI 配置自定义脚本路径

**连接问题：**
- 使用 `health_check()` 验证两个服务器是否都在运行
- 检查端口冲突：`netstat -tlnp | grep 25419`
- 验证防火墙是否允许 localhost 连接

**常见错误：**
- **E008**：安装 Python 3.10+ 或 `uv`
- **E009/E010**：检查 `~/.jiap/mcp/` 权限
- **E002**：在 JIAP 设置 GUI 中更改端口
- **E014**：确保 JIAP 插件已启用并加载

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

- **[JADX](https://github.com/skylot/jadx)**: 强大的Android反编译器
- **[FastMCP](https://github.com/modelcontextprotocol/servers)**: MCP协议实现
- **[Javalin](https://javalin.io/)**: 轻量级Web框架
- **[jadx-ai-mcp](https://github.com/zinja-coder/jadx-ai-mcp/)**：Jadx AI 插件

---

<div align="center">

**⭐ 如果这个项目对您有帮助，请给一个Star！**

![Star History](https://img.shields.io/github/stars/jygzyc/jiap?style=social)

</div>