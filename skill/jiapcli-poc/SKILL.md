---
name: jiapcli-poc
description: Android 漏洞 PoC 应用构造。生成可编译安装的 PoC Android 应用，在真实设备或模拟器上验证漏洞利用。当用户提到 PoC、proof of concept、漏洞验证、exploit、利用构造、复现漏洞、poc 验证时使用。
metadata: {}
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
2. **验证报告准确性**，**必须使用 subagent 执行**（防止大量源码数据污染主上下文）：
   - 为报告中的**每个问题创建独立的 subagent**，各 subagent 并发执行
   - 每个 subagent 的 prompt 包含：端口号、该问题的完整描述（组件名、调用链、声称的 Source/Sink）
   - subagent 内部执行以下验证并**仅返回结构化结论**（禁止返回原始源码）：
     - **目标组件是否存在且导出**：`jiap ard exported-components -P <port>` 确认组件在列表中
     - **调用链中的方法是否存在**：对调用链涉及的每个方法执行 `jiap code method-source "<sig>" -P <port>` 确认方法签名匹配且源码行为与报告描述一致
     - **Source/Sink 是否确实存在**：在方法源码中确认报告声称的 Source（如 `getIntent().get*Extra()`）和 Sink（如 `startActivity()`）是否真实存在
     - **权限校验是否被遗漏**：检查组件及调用链沿途是否有报告未提及的安全校验（如签名校验、UID 白名单），可能导致漏洞不可利用

   subagent 输出格式（每项一条）：
   ```
   - [PASS/FAIL] 组件 X 导出状态：...
   - [PASS/FAIL] 方法签名 Y：...
   - [PASS/FAIL] Source Z 存在性：...
   - [PASS/FAIL] Sink W 存在性：...
   - [PASS/FAIL] 遗漏校验检查：...
   ```

   主 agent 汇总所有 subagent 结论。如果验证发现报告描述与实际代码不符，**告知用户具体差异**，由用户决定是否继续构造 PoC 或先修正报告。

   > **前置条件**：验证步骤需要目标 APK 的 jiap session 处于运行状态。如果 session 未运行，提示用户先通过 `jiap process open "<apk-path>" -P <port>` 打开。

3. **确定组件类型**（Activity / Broadcast / Provider / Service / Intent / WebView / Framework），加载对应的 reference
4. **首次创建项目**：执行 `node scripts/setup-poc.mjs <target-app>`，自动解压模板并完成包名替换（`com.poc.targetapp` → `com.poc.<target-app>`，含目录名）
5. **编写 Exploit**：在 `app/src/main/java/com/poc/<target-app>/exploit/` 下创建 Exploit 类，从 reference 中选择匹配的漏洞模式模板，将报告中的攻击步骤填入 `execute()` 方法，替换包名/类名常量
6. **注册到 ExploitRegistry**：在 `ExploitRegistry.java` 的 `EXPLOITS` 数组中添加新 Exploit 类

## 项目模板

项目模板为 `assets/poc-template.zip`，是一个完整可编译的 Gradle Android 项目。

**首次创建项目**：执行 `node scripts/setup-poc.mjs <target-app>`，自动完成解压、包名替换和目录重命名。

```
poc-<target-app>/
├── settings.gradle              # 项目设置
├── gradle.properties            # JVM 参数
├── gradle/libs.versions.toml    # 依赖版本管理
├── gradle/wrapper/              # Gradle Wrapper
├── gradlew / gradlew.bat        # 构建脚本
└── app/
    ├── build.gradle             # app 模块配置（AGP 7.4.2, JDK 11+）
    └── src/main/
        ├── AndroidManifest.xml  # Manifest 模板（按攻击向量添加权限和组件声明）
        ├── res/values/styles.xml
        ├── res/layout/activity_poc.xml
        └── java/com/poc/targetapp/
            ├── Exploit.java          # Exploit 基类（提供 Activity 和 log 方法）
            ├── ExploitRegistry.java  # Exploit 注册表（新增 Exploit 在此注册）
            └── PoCActivity.java      # 主界面（自动为每个 Exploit 生成触发按钮）
```

模板已集成 `AndroidHiddenApiBypass` 和 `AppCompat` 依赖。

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

> **重要：以下步骤仅在用户明确要求编译或部署时才执行。** 代码编写阶段不要主动运行任何环境检测或编译命令。

### 环境检测

当用户要求编译时，先运行环境检测脚本：

```bash
node scripts/check-env.mjs
```

脚本会检测 ANDROID_HOME、build-tools、platforms、JDK 版本、adb 并输出结果。如果检测未通过（退出码非 0），**停止编译**，将脚本输出完整展示给用户，由用户自行解决环境问题。

### 编译

环境检测通过后执行编译，使用超时保护防止 Gradle daemon 卡死：

```bash
cd poc-<target-app> && timeout 300 ./gradlew assembleDebug --no-daemon
```

如果编译超时或失败，分析错误日志修复代码后重试。

### 部署（可选）

仅在用户明确要求部署且 `adb devices` 能检测到设备时执行：

```bash
adb install app/build/outputs/apk/debug/app-debug.apk
adb logcat -s PoC:I AndroidRuntime:E
adb uninstall com.poc.<target_app>
```
