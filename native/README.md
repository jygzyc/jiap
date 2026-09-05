# native — Rust 原生 DECX(core / server / cli)

分支 `native-dev`。用纯 Rust 替换现有 JVM/JADX 方案的核心。

**dexdec 不是外挂依赖,而是这个核心的内在引擎**:类索引来自它的
`ArchiveCatalog`,成员/继承来自 `ClassOutline`,Java 与 smali(IRDump)输出来自
它的反编译器与 `visualizer`,交叉引用来自它的 `references` 扫描器,批量预热走
它的 `ClassBatch` 管线。`decx-core` 只是把这套能力包装成 DECX 的
`Project + api::dispatch` 形态。

- 引擎:[asLody/dexdec](https://github.com/asLody/dexdec) + `rusty-dex`(vendor 照抄,Apache-2.0)
- 自研三层:`decx-core`(引擎融合 + 端点分发)、`decx-server`(axum HTTP)、`decx-cli`(会话 + 查询)

同机实测(5920 类 dex):open **0.59s** 就绪(JVM/jadx 栈 33 分钟未就绪)、
单类交互 **76–160ms**、全量批扫 65s 零失败、dexdec 自测 **618/618**。
数据与结论见 [RESEARCH.md](RESEARCH.md)。

## 构建(Windows + Git Bash)

本机无 MSVC,`native/` 使用 GNU 工具链(新 clone 先执行
`rustup override set stable-x86_64-pc-windows-gnu`);三个本机必需的构建前提
(都已固化,详见 RESEARCH.md §4):

1. PATH 加 `native/bin`(内含 NDK llvm-dlltool 拷贝,供 windows-sys raw-dylib 使用);
2. 链接器必须用 **rust-lld**(`.cargo/config.toml` 已配置):windows-gnu 自带
   GNU ld 对这个依赖图会间歇性产出启动即段错误的坏二进制;
3. release 为 **opt-level 2,不开 LTO**(LTO 与过高的 opt 在 windows-gnu 上
   误编译,曾导致 `--help` 都段错误)。

```bash
cd native
export PATH="/e/Code/decx/native/bin:$PATH"
cargo build --release
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

环境变量:`DECX_NATIVE_REQUEST_TIMEOUT_SECS`(默认 120,冷启全库搜索/预热调大)、
`DECX_NATIVE_BATCH_WORKERS`(默认 min(4, 核数),批量/层级构建并行度)。

`code call <endpoint> [--arg k=v]... [--json '{...}']` 可直达全部端点
(含 `get_strings`、`get_app_manifest`、`get_exported_components`、
`get_deep_links` 等 25 个 Kotlin 同名端点)。

## HTTP 契约(与 DecxRoutes 同名同路径)

`GET /health`;`POST /api/decx/<endpoint>`,成功返回 items 信封
(`{ok, kind, query, summary:{total,returned,truncated}, items:[{id,kind,title,content,meta}], page}`),
错误 `{ "error": "<CODE>", "message": "..." }` 及 400/404/503/504 映射与 Kotlin
`DecxError` 一致。

| 端点 | 引擎能力 |
|---|---|
| `get_classes` / `search_method` | `ArchiveCatalog` / `member_catalog` 元数据索引 |
| `get_class_source` / `get_method_source`(java) | dexdec 反编译器 |
| `--smali` 输出 | dexdec IR visualizer(语义块列表,非 dalvik 原文) |
| `get_method_cfg` | dexdec `decode_method` 的 CFG(块/边/EdgeKind)+ IR 文本 |
| `get_method_xref` / `get_field_xref` / `get_class_xref` | dexdec `references` 字节码级引用扫描(带指令偏移) |
| `get_app_manifest` | abxml 解码二进制 AXML(resources.arsc 资源名还原) |
| `get_strings` | dex 字符串表(分页 + 正则过滤) |
| `get_implementations` / `get_subclasses` | 全部 `ClassOutline` 并行构建的层级索引(缓存后毫秒级) |
| `search_global_key` | 并行 dexdec 批量预热 + 正则 grep |

真机验证案例:vivo 全局搜索系统应用(`com.vivo.globalsearch`,42MB,48431 类,
targetSdk 36,R8 混淆)——打开 1.1s,manifest/深链/导出组件/方法源码/IR/交叉引用
(带源码行)全部可用,详见 RESEARCH.md §6。

差距清单(RESEARCH.md §7):AIDL、MCP、jadx 脚本、resources.arsc 资源表端点。
