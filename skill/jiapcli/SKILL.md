---
name: jiapcli
description: Android APK/DEX/JAR 通用分析 CLI。基于 JADX 反编译器，支持代码检索、交叉引用、组件分析。当用户提到 jiap、jadx、android analysis、APK 分析、反编译、decompile、代码审查、code review 时使用。
metadata:
  requires:
    bins: ["jiap"]
---

# JIAP CLI — 通用分析

基于 JADX 的 Android 通用分析工具。用于 APK/DEX/JAR 的反编译、代码检索、交叉引用和组件分析。

## 命令参考

### process - 进程管理

| 命令 | 说明 |
|------|------|
| `jiap process check [--install]` | 检查环境状态，`--install` 安装缺失组件 |
| `jiap process open <file> [options]` | 打开文件分析 |
| `jiap process close [name] [--all]` | 关闭指定 session 或全部 |
| `jiap process list` | 列出运行 session（NAME/PORT/PID/PATH） |
| `jiap process status [name]` | 检查服务器状态 |
| `jiap process install [-p]` | 安装 jiap-server.jar（`-p` 安装 prerelease） |

**open 选项：**

| 选项 | 说明 |
|------|------|
| `-P, --port <port>` | 服务器端口 |
| `--force` | 强制启动（忽略已有 session） |
| `-n, --name <name>` | 自定义 session 名 |

所有标准 jadx-cli 选项直接透传，常用：`--deobf`、`--no-res`、`--show-bad-code`、`-j`/`--threads-count`、`--no-imports`、`--no-debug-info`、`--escape-unicode`、`--log-level` 等。

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
| `jiap ard app-receivers` | 动态广播接收器 |
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
3. jiap code xref-method <sig>      → 追踪调用链
4. jiap code implement <Interface>  → 查找所有实现

必要时可使用 `search-class` 搜索全局关键字，`search-method` 按名称搜索方法。
找到具体实现后，再查找接口的所有实现类。
```

## 常见分析场景

### 理解应用结构
```
jiap ard app-manifest              → 了解组件注册和权限声明
jiap ard exported-components       → 查看对外暴露的组件
jiap ard app-deeplinks             → 查看 Deep Link 入口
jiap code all-classes              → 浏览包结构
```

### 追踪特定功能
```
jiap code search-method "login"    → 定位登录相关方法
jiap code class-source <Class>     → 阅读实现
jiap code xref-method <sig>        → 追踪谁调用了它
jiap code xref-field <field>       → 追踪字段读写
```

### 分析继承和实现
```
jiap code subclass <BaseClass>     → 查找所有子类
jiap code implement <Interface>    → 查找接口实现
jiap code class-info <Class>       → 查看类的方法和字段列表
```

### 检查资源和配置
```
jiap ard all-resources             → 浏览资源文件
jiap ard resource-file <res>       → 查看具体资源内容
jiap ard strings                   → 查看字符串资源
```
