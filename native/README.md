# native — Rust 原生 DECX(core / server / cli)

分支 `native-dev`。用纯 Rust 替换现有 JVM/JADX 方案的核心:

- **Java 反编译核心 = [asLody/dexdec](https://github.com/asLody/dexdec)**(vendor 照抄 + 移植补丁)
- **解析 / smali / CFG / 交叉引用 = [androguard](https://github.com/androguard/dex-decompiler) 三件套**(vendor 照抄)
- 自研三层结构对齐现有仓库:`decx-core`(引擎 glue + 端点分发)、`decx-server`(axum HTTP)、`decx-cli`(会话 + 查询)

同机实测(5920 类 dex):服务就绪 **0.59s**(JVM/jadx 栈 33 分钟未就绪)、
单类交互 **76–160ms**、全量批扫 **65s 零失败**、dexdec 自测 **618/618**。
数据与结论见 [RESEARCH.md](RESEARCH.md)。

## 构建(Windows + Git Bash)

本机无 MSVC,`native/` 使用 GNU 工具链(新 clone 先执行
`rustup override set stable-x86_64-pc-windows-gnu`);windows-gnu 需要 dlltool,
已备好 NDK 的 llvm-dlltool 副本(不入库):

```bash
cd native
export PATH="/e/Code/decx/native/bin:$PATH"   # dlltool
cargo build --release                          # thin LTO(fat LTO 在 windows-gnu 有误编译,勿改)
cargo test --release -p decx-core -p decx-cli
```

产物:`target/release/decx-native-server.exe`、`target/release/decx-native.exe`
(CLI 按同级目录查找 server)。

## 用法(对齐 decx-cli 体验)

```bash
./target/release/decx-native process open app.apk --name demo [--port N] [--warm]
./target/release/decx-native process list
./target/release/decx-native code get-classes --limit 20 --includes '^com\.foo\.'
./target/release/decx-native code get-class-source com.foo.Bar [--smali] [--limit N]
./target/release/decx-native code search-global-key 'pattern' [--regex] [--case-sensitive]
./target/release/decx-native code get-method-source 'com.foo.Bar.method' [--smali]
./target/release/decx-native code get-method-cfg 'com.foo.Bar.method'
./target/release/decx-native code get-method-xref / get-field-xref / get-class-xref
./target/release/decx-native code get-implementations / get-subclasses
./target/release/decx-native process close demo
```

会话:`~/.decx-native/sessions.json`;server 日志:`~/.decx-native/logs/<name>.log`;
源码缓存默认 1GiB,`DECX_NATIVE_CACHE_MAX_BYTES` 可调;stdout 只出 JSON。

## HTTP 契约(与 DecxRoutes 同名同路径)

`GET /health`;`POST /api/decx/<endpoint>`,错误 `{ "error": "<CODE>", "message": "..." }`
及 400/404/503/504 映射与 Kotlin `DecxError` 一致。已实现:`get_classes`、
`get_class_source`、`get_class_context`、`search_global_key`、`search_class_key`、
`search_method`、`get_method_source`、`get_method_context`、`get_method_cfg`、
`get_method_xref`、`get_field_xref`、`get_class_xref`、`get_implementations`、
`get_subclasses`、`get_app_manifest`(文本 XML)。

Java 输出走 dexdec;`--smali`、方法 CFG、三类 xref 走 androguard;差距清单见
[RESEARCH.md](RESEARCH.md) 第 7 节。
