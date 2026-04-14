---
name: jiapcli
description: Android APK/DEX/JAR 通用分析 CLI。基于 JADX 反编译器，支持代码检索、交叉引用、组件分析。当用户提到 jiap、jadx、android analysis、APK 分析、反编译、decompile、代码审查、code review 时使用。
metadata:
  requires:
    bins: ["jiap"]
---

# JIAP CLI — 通用分析

基于 JADX 的 Android 通用分析工具。用于 APK/DEX/JAR 的反编译、代码检索、交叉引用和组件分析。

## 使用原则

不确定命令或选项时，优先通过 `--help` 确认，不要猜测。例如 `jiap process open --help`、`jiap code --help`。

**每个 `open` 任务结束后必须调用 `close` 释放资源。** 使用 `jiap process close "name"` 关闭指定 session，或 `jiap process close -a` 关闭全部。

**必须通过 `-P <port>` 指定端口。** 端口可随机选取，建议在 25000–65535 之间以避免与常见服务冲突。

**参数必须用引号包裹。** 含包名、类名、方法签名、文件路径的参数值必须用双引号包裹，防止括号、冒号、空格等字符被 shell 错误解析。

## 命令参考

### process - 进程管理

| 命令 | 说明 |
|------|------|
| `jiap process check [-P <port>] [--install] [--json]` | 检查环境状态，`--install` 安装缺失组件 |
| `jiap process open "<file>" [options]` | 打开文件分析（支持本地路径和 HTTP URL） |
| `jiap process close "[name]" [-a] [--json]` | 关闭指定 session 或全部 |
| `jiap process list [--json]` | 列出运行 session（NAME/PORT/PID/PATH） |
| `jiap process status "[name]" [-P <port>] [--json]` | 检查服务器状态 |
| `jiap process install [-p] [--json]` | 安装 jiap-server.jar（`-p` 安装 prerelease） |

**open 选项：**

| 选项 | 说明 |
|------|------|
| `-P, --port <port>` | 服务器端口 |
| `--force` | 强制启动（忽略已有 session） |
| `-n, --name "<name>"` | 自定义 session 名（默认：APK 文件名去扩展名） |
| `--json` | JSON 格式输出 |

`open` 的 `<file>` 参数支持本地文件路径和 HTTP/HTTPS URL。URL 会自动下载到 `~/.jiap/tmp/` 并缓存。

**open 的 session 管理行为：**
- 同名 + 同 hash + 存活 → 自动复用已有 session
- 同名 + 不同 hash → 报错，提示使用 `--force` 或 `--name`
- 不同名 + 同 hash → 报错，提示使用 `--force`
- 端口被占用 → 报错，提示使用 `--port` 指定其他端口或关闭占用 session

**jadx-cli 选项透传：**

所有标准 jadx-cli 选项直接透传，常用：`--deobf`、`--no-res`、`--show-bad-code`、`-j`/`--threads-count`、`--no-imports`、`--no-debug-info`、`--escape-unicode`、`--log-level` 等，推荐`--show-bad-code`和`--no-imports`用于完整分析。

### code - 代码分析

| 命令 | 说明 |
|------|------|
| `jiap code all-classes [--page <n>]` | 列出所有类 |
| `jiap code class-info "<class>" [--page <n>]` | 获取类信息 |
| `jiap code class-source "<class>" [--smali] [--page <n>]` | 获取类源码 |
| `jiap code method-source "<sig>" [--smali] [--page <n>]` | 获取方法源码 |
| `jiap code xref-method "<sig>" [--page <n>]` | 方法交叉引用（谁调用了它） |
| `jiap code xref-class "<class>" [--page <n>]` | 类交叉引用 |
| `jiap code xref-field "<field>" [--page <n>]` | 字段交叉引用 |
| `jiap code implement "<interface>" [--page <n>]` | 查找接口实现 |
| `jiap code subclass "<class>" [--page <n>]` | 查找子类 |
| `jiap code get-aidl [--page <n>]` | 获取所有 AIDL 接口及实现类 |
| `jiap code search-class "<keyword>" [--page <n>]` | 搜索类内容（占资源，必要场景使用，禁止批量调用） |
| `jiap code search-method "<name>" [--page <n>]` | 搜索方法名（占资源，必要场景使用，禁止批量调用） |

### ard - Android 分析

| 命令 | 说明 |
|------|------|
| `jiap ard app-manifest [--page <n>]` | AndroidManifest.xml |
| `jiap ard main-activity [--page <n>]` | 主 Activity |
| `jiap ard app-application [--page <n>]` | Application 类 |
| `jiap ard exported-components [--page <n>]` | 导出组件列表 |
| `jiap ard app-deeplinks [--page <n>]` | Deep Link 列表 |
| `jiap ard all-resources [--page <n>]` | 所有资源文件名 |
| `jiap ard resource-file "<res>" [--page <n>]` | 获取资源文件内容 |
| `jiap ard strings [--page <n>]` | 获取 strings.xml 内容 |
| `jiap ard app-receivers [--page <n>]` | 动态广播接收器 |
| `jiap ard system-service-impl "<interface>" [--page <n>]` | 系统服务实现类 |

### 全局选项

适用于 `code`/`ard`：

| 选项 | 说明 |
|------|------|
| `-s, --session "<name>"` | 指定目标 session 名称（APK 文件名） |
| `-P, --port <port>` | 指定服务器端口（默认 25419） |
| `--json` | JSON 格式输出 |

`--page <n>` 是各子命令的独立选项（默认 1），用于分页，不属于全局选项。

## 环境变量

| 变量 | 说明 |
|------|------|
| `JIAP_SERVER_HOME` | 自定义 jiap-server.jar 路径。可指向 jar 文件本身或包含 jar 的目录。默认 `~/.jiap/bin/jiap-server.jar` |

## 方法签名格式

用于 `code method-source` 和 `code xref-method`：

```
package.Class.methodName(paramType1,paramType2):returnType
```

示例：`"com.example.MainActivity.onCreate(android.os.Bundle):void"`（传给 CLI 时必须用引号包裹）

## 分析策略

优先使用 `class-source` + `xref-*` + `implement` 定位代码。`search-*` 必要场景使用，禁止批量调用。

```
典型流程：
1. jiap ard exported-components                   → 定位目标组件
2. jiap code class-source "com.example.MyClass"  → 获取源码，找到关键方法
3. jiap code xref-method "com.example.MyClass.myMethod(java.lang.String):void"  → 追踪调用链
4. jiap code implement "com.example.MyInterface"  → 查找所有实现

必要时可使用 `search-class` 搜索全局关键字，`search-method` 按名称搜索方法。
找到具体实现后，再查找接口的所有实现类。
```

## 常见分析场景

### 理解应用结构
```
jiap ard app-manifest               → 了解组件注册和权限声明
jiap ard exported-components        → 查看对外暴露的组件
jiap ard app-deeplinks              → 查看 Deep Link 入口
jiap code all-classes               → 浏览包结构
```

### 追踪特定功能
```
jiap code search-method "login"     → 定位登录相关方法
jiap code class-source "com.example.MyClass"  → 阅读实现
jiap code xref-method "com.example.MyClass.login(java.lang.String):void"  → 追踪谁调用了它
jiap code xref-field "com.example.MyClass.mToken"  → 追踪字段读写
```

### 分析继承和实现
```
jiap code subclass "com.example.BaseActivity"     → 查找所有子类
jiap code implement "com.example.MyInterface"     → 查找接口实现
jiap code get-aidl                                → 发现所有 AIDL 接口
jiap code class-info "com.example.MyClass"        → 查看类的方法和字段列表
```

### 检查资源和配置
```
jiap ard all-resources              → 浏览资源文件
jiap ard resource-file "res/xml/file_paths.xml"  → 查看具体资源内容
jiap ard strings                    → 查看字符串资源
```
