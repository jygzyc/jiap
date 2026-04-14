---
name: jiapcli-poc
description: Android 漏洞 PoC 应用构造。生成可编译安装的 PoC Android 应用，在真实设备或模拟器上验证漏洞利用。当用户提到 PoC、proof of concept、漏洞验证、exploit、利用构造、复现漏洞、poc 验证时使用。
metadata:
  requires:
    bins: ["gradle", "javac"]
---

# JIAP CLI — 漏洞 PoC 应用构造

将漏洞发现转化为可编译通过的 PoC Android 应用。核心产出是**可编译的 PoC 项目代码**，adb 部署验证为可选项。

**输入来源**：接收 skill `jiapcli-vulnhunt` 输出的漏洞分析报告，根据报告构造 PoC。

命令参考见通用 skill `jiapcli`。

## 核心原则

- **PoC 必须是一个可编译的 Android 应用项目。** 编译通过是最低标准。
- **同一项目 session 中只创建一个 PoC 应用**，多个漏洞问题通过各自的 Exploit 组件实现。
- **`allowBackup` 设为 false，`applicationId` 使用 `com.poc.*` 命名空间。**
- **涉及隐藏 API / 框架函数调用时，使用 [AndroidHiddenApiBypass](https://github.com/LSPosed/AndroidHiddenApiBypass) 库。** 项目模板已集成该依赖，通过 `HiddenApiBypass.getDeclaredMethod()` / `HiddenApiBypass.invoke()` 调用隐藏 API，无需反射 stub JAR。

## 从报告到 PoC

```
报告的"问题一" → 一个 Exploit 类
报告的"问题二" → 另一个 Exploit 类
... → 全部注册到同一个 PoC 应用的 EXPLOITS 数组
```

构造步骤：

1. **读取报告**，从"攻击路径 → 目标组件"获取包名、类名、Action/URI、IPC 接口
2. **确定组件类型**（Activity / Broadcast / Provider / Service / Intent / WebView / Framework），加载对应的 reference
3. **首次创建项目**：基于 `assets/` 下的模板文件搭建 `poc-<target-app>/` 项目结构
4. **编写 Exploit**：从 reference 中选择匹配的漏洞模式模板，将报告中的攻击步骤填入 `execute()` 方法，替换包名/类名常量
5. **注册到 PoCActivity**：在 `EXPLOITS` 数组中添加新 Exploit 类
6. **编译通过**

## 项目模板

项目结构和模板文件见 `assets/` 目录：

| 文件 | 说明 |
|------|------|
| `assets/build.gradle.root` | 项目根 build.gradle |
| `assets/build.gradle.app` | app 模块 build.gradle（namespace 和 applicationId 替换为 `com.poc.<target-app>`） |
| `assets/AndroidManifest.xml` | Manifest 模板（按攻击向量添加权限和组件声明） |
| `assets/PoCActivity.java` | 主界面模板（新增 Exploit 后在 `EXPLOITS` 数组中注册） |
| `assets/activity_poc.xml` | 布局模板 |

## 按组件加载 Reference

| 组件 | Reference 文件 | 覆盖漏洞数 |
|------|---------------|-----------|
| Activity | `references/poc-app-activity.md` | 9 种 |
| Broadcast | `references/poc-app-broadcast.md` | 4 种 |
| ContentProvider | `references/poc-app-provider.md` | 6 种 |
| Service | `references/poc-app-service.md` | 5 种 |
| Intent | `references/poc-app-intent.md` | 5 种 |
| WebView | `references/poc-app-webview.md` | 6 种 |
| 系统服务 | `references/poc-framework-service.md` | 6 种 |

所有 Exploit 继承自 `references/poc-base.md` 中的基类。

## 构建与部署

```bash
# 编译（必须）
cd poc-<target-app> && ./gradlew assembleDebug

# 以下为可选步骤，需要 adb 环境：
# 安装到设备
adb install app/build/outputs/apk/debug/app-debug.apk
# 查看执行日志
adb logcat -s PoC:I AndroidRuntime:E
# 卸载
adb uninstall com.poc.<target_app>
```
