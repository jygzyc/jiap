# native — Rust 原生 DECX core + server

分支 `native-dev`。最终架构分工:

- **Rust(本目录)= `decx-core` + `decx-server`**:分析核心与 HTTP 服务,替换 JVM/JADX 栈;
- **TypeScript `decx-cli`(仓库 `decx-cli/`,保持原样)= 唯一客户端**:会话管理、
  查询命令、skill 工作流全部走现有 TS CLI,不做 Rust 化。

**dexdec 不是外挂依赖,而是这个核心的内在引擎**:类索引来自它的
`ArchiveCatalog`,成员/继承来自 `ClassOutline`,Java 与 smali(IRDump)输出来自
它的反编译器与 `visualizer`,交叉引用来自它的 `references` 扫描器,批量预热走
它的 `ClassBatch` 管线。`decx-core` 只是把这套能力包装成 DECX 的
`Project + api::dispatch` 形态。

- 引擎:[asLody/dexdec](https://github.com/asLody/dexdec) + `rusty-dex`(vendor 照抄,Apache-2.0)
- 自研两层:`decx-core`(引擎融合 + 端点分发)、`decx-server`(axum HTTP)

同机实测(5920 类 dex):open **0.59s** 就绪(JVM/jadx 栈 33 分钟未就绪)、
单类交互 **76–160ms**、dexdec 自测 **618/618**。真机案例(vivo 全局搜索,
42MB/48431 类/R8 混淆)全端点验证通过。数据与结论见 [RESEARCH.md](RESEARCH.md)。

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
cargo test --release -p decx-core -p decx-server
```

产物:`target/release/decx-native-server[.exe]`。TS CLI 自动发现它:
`DECX_NATIVE_SERVER` 环境变量(文件或目录)> `DECX_HOME/bin` > 本仓库 dev 构建。

## 用法:TS CLI 驱动 Rust 引擎

```bash
cd decx-cli && npm run build   # 一次性

node dist/index.js process open app.apk --engine native --name demo
# 或 export DECX_ENGINE=native 后省略 --engine

node dist/index.js process list
node dist/index.js code classes --limit 20 --includes '^com\.foo\.'
node dist/index.js code class-source com.foo.Bar [--smali]
node dist/index.js code search-global 'pattern' --includes '^com\.foo\.'
node dist/index.js code method-source 'com.foo.Bar#method'
node dist/index.js code method-cfg 'com.foo.Bar#method'
node dist/index.js code xref-method / xref-class / xref-field
node dist/index.js code implementations / subclasses
node dist/index.js process close demo
```

约束(与 JVM 引擎的差异):`--script`/`--mcp` 需要 JVM,`--engine native` 下
直接报错;jadx 透传参数被忽略;会话按 engine 区分,同名/同 hash 不同引擎的
复用会报错(用 `--force` 替换)。

环境变量:`DECX_NATIVE_REQUEST_TIMEOUT_SECS`(server 请求超时,默认 120,冷启
全库搜索/预热调大)、`DECX_NATIVE_BATCH_WORKERS`(批量/层级并行度,默认
min(4, 核数))、`DECX_NATIVE_CACHE_MAX_BYTES`(源码缓存,默认 1GiB)。

## HTTP 契约(与 DecxRoutes 同名同路径)

`GET /health`;`POST /api/decx/<endpoint>`,成功返回 items 信封
(`{ok, kind, query, summary:{total,returned,truncated}, items:[{id,kind,title,content,meta}], page}`),
错误 `{ "error": { "code", "message" } }` 及 400/404/503/504 映射与 Kotlin
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
| 其余 manifest 类端点 | 解码后的 manifest 解析(导出组件/深链/receiver 等) |

真机验证案例:vivo 全局搜索系统应用(`com.vivo.globalsearch`,42MB,48431 类,
targetSdk 36,R8 混淆)——打开 1.1s,manifest/深链/导出组件/方法源码/IR/交叉引用
(带源码行)全部可用,详见 RESEARCH.md §6。

### 标准 jar 与 android.jar

内容为 JVM `.class` 的 jar(库 jar、SDK 的 `android.jar`)同样可加载:
`process open <jar> --engine native` 后按内容自动识别。能力范围:
类清单、javap 风格骨架源码(层级+签名+常量)、类上下文、方法签名块、
成员搜索、implementations/subclasses——对 API 面分析足够
(`android.jar` 本就是空体 stub)。JVM 方法体不做反编译,dex 专属端点
(cfg/xref/字符串表)返回 `UNSUPPORTED_FOR_TARGET`。实测:SDK 35 的
android.jar(27MB)打开 2.2s,`android.app.Activity` 骨架/成员/层级查询正常。

差距清单(RESEARCH.md §7):AIDL、MCP、jadx 脚本、resources.arsc 资源表端点。
