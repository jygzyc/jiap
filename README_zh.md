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
- **Python**: 3.10+ （用于 MCP Server）
- **JADX**: 支持插件的 JADX 反编译器
- **Python 依赖**: `requests`, `fastmcp`

### 安装

```bash
# 1. 在 JADX GUI 中安装插件
# JADX -> Settings -> Plugins -> Install JIAP

# 或使用命令行安装
jadx plugins --install-jar <path-to-jiap.jar>

# 2. 安装 MCP Server 依赖
cd mcp_server
pip install requests fastmcp  # 或者使用 uv sync
```

### 使用说明

* 启动 JIAP 插件

  - 启动 JADX 并启用 JIAP 插件
  - 确认服务器运行在 `http://127.0.0.1:25419`

* 启动 MCP 服务器

```bash
cd mcp_server
python jiap_mcp_server.py
```

MCP 服务器将在 `http://0.0.0.0:25420` 启动。
* 验证连接

使用 `health_check()` 验证 MCP 服务器与 JIAP 插件之间的连接

* 可用工具
  - `get_all_classes(page=1)` - 获取所有可用类，支持分页
  - `search_class(class_name, page=1)` - 按名称搜索类
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

```bash
# 环境变量
export JIAP_URL="http://192.168.1.100:25419"
python jiap_mcp_server.py

# 命令行参数
python jiap_mcp_server.py --jiap-host 192.168.1.100 --jiap-port 25419
```

---

## 开发

### 从源码构建

```bash
# 构建 JIAP Core
cd jiap_core
chmod +x gradlew
./gradlew dist

# 安装 MCP Server 依赖
cd mcp_server
pip install requests fastmcp  # 或者使用 uv sync
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

在`jadx/plugins/jiap/JiapConfig.kt`中注册自定义`service`，并在`routeMap`中添加路由映射即可实现自定义接口功能

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