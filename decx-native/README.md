# decx-native

DECX 原生 Rust 分析引擎。纯标准库（zero external deps），跨平台构建，独立 HTTP server，
契约兼容 JVM 版 `decx-server`（`POST /api/decx/<endpoint>` + `GET /health`）。

## 结构

```
crates/
  decx-json     # 自研 JSON 值类型 + 解析/序列化（插入序保持、深度上限、surrogate pair）
  decx-apk      # zip/inflate/axml(AXML)/manifest —— APK 容器层
  decx-core     # dex 解析、opcode 表、全局索引/xref、结构化 IR Java 发射
                # + sources.rs：decx-engine 源码恢复桥（失败回落结构化 IR）
  decx-taint    # 污点分析：MT 风格跨过程数据流（source/sink/propagator/sanitizer 规则）
  decx-server   # decx-native-server：手写 HTTP/1.1 + 27 端点路由 + envelope 兼容
  decx-engine/  # 自研反编译引擎（源码级反编译核心）
                # 核心改编自 asLody/dexdec v1.0.2 与 rusty-rs/rusty-dex 0.2.0
                # （均为 Apache-2.0，文件头标注来源，见 VENDORED.md）
                # 内含 rusty_dex/（DEX 解析器模块）与 log/rayon/zstd/crc32fast
                # 根模块（依赖垫片，自研代码，零外部依赖）
```

## 源码恢复策略

- `get_class_source` / `get_method_source` 优先走 **decx-engine** 完整管线
  （SSA→Region IR→类型/值恢复→typed AST→Java/Kotlin 打印器）。
- decx-engine 失败（或 `smali=true`）时回落 decx-core 自研结构化 IR。
- 响应 `meta.mode` = `dexdec` | `structural-ir`；`meta.language` = 实际输出语言。
- `language` 请求字段：`java`（默认）| `kotlin` | `auto`（按 kotlin.Metadata/SourceFile 推断）。

## 构建

```bash
cd decx-native
cargo build --release
cargo test
```

`crates/decx-engine/resources/symbols/platform.dexsym`（约 14MB）为平台 ABI 元数据，
随源码一起携带，编译时 `include_bytes!` 内嵌。
上游文件为 zstd 压缩（frame flag=1）；本仓版已用 `analysis/dexsym-repack`
一次性工具重写为未压缩存储（flag=0），因为引擎内 `zstd` 模块是恒等透传
（写入路径 encode_all 恒等 + 读取路径 decode_all 恒等，自读自写自洽）。

## 运行

```bash
target/release/decx-native-server E:/path/to/app.apk --port 25419
# ready: "DECX Server running at http://127.0.0.1:25419"

# 自定义污点规则（否则用内嵌默认规则集）
target/release/decx-native-server app.apk --taint-rules my-rules.json
# 或：DECX_TAINT_RULES=my-rules.json target/release/decx-native-server app.apk
```

## 污点分析（native-only：`taint_scan`）

`POST /api/decx/taint_scan` 是第 27 个端点，仅原生引擎提供（JVM 侧无对应实现）。

- 请求体：`{}`（全部可选字段：`maxRounds`（默认 8）、`maxFindings`（默认 50））。
- 规则内嵌于 `crates/decx-taint/resources/taint-rules.json`：source（如
  `TelephonyManager.getDeviceId`、`Build.SERIAL`）、sink（如
  `HttpURLConnection.setRequestProperty`、`SQLiteDatabase.execSQL`）、
  propagator（`StringBuilder.append`、`Intent.putExtra` 等四类传播模式）、
  sanitizer（`Integer.parseInt` 等 kill-ret）、exclude（androidx/support 等包前缀）。
- 规则可在启动时替换：`decx-native-server app.apk --taint-rules my-rules.json`
  （或环境变量 `DECX_TAINT_RULES`）。文件格式与内嵌规则一致，额外支持
  `"param": <n>` 字段把回调方法声明参数锚定为 source（AppShark 风格：如
  `BroadcastReceiver.onReceive` 的 `intent` 参数生而带污点，param 按含 this
  的参数序号计）；规则文件不可读/非法时启动即失败（fail-fast）。
- 分析是 Mariana-Trench 风格的跨过程数据流：过程内线性流不敏感求解器产出
  方法摘要（param→sink、param→静态字段、返回值污点），调用点采纳摘要并在
  fixpoint（≤ maxRounds）上迭代至不动点；每个 finding 带 source/sink 位置与
  途经跳点（`route`），并按 source×sink 类别定级 severity。
- 虚函数解析采用同键扇出：virtual/interface/super/polymorphic 调用点会把
  基类签名连同全部同名键 override（子类重写、接口实现）一起作为候选，
  sink 命中与摘要采纳都对候选集展开（保守但免类型推导）；sanitizer/propagator
  只按字面匹配，不做扇出，避免过宽清污。
- 响应 `summary`（DecxApiResult 契约位置）：`rounds_run`、`methods_analyzed`、`methods_total`、
  `static_fields_tracked`、`truncated`（达到 maxFindings 早停时为 true）。

## 许可

- decx-native 各 crate：Apache-2.0
- `crates/decx-engine`：自研包名，但核心改编自两个上游（均 Apache-2.0）——
  - [asLody/dexdec](https://github.com/asLody/dexdec) v1.0.2（`src/**`，见 `LICENSE.dexdec`）
  - [rusty-rs/rusty-dex](https://github.com/rusty-rs/rusty-dex) 0.2.0（`src/rusty_dex/**`，
    基线为 dexdec v1.0.2 workspace 内置副本，见 `LICENSE.rusty-dex`）
  - 来源、修改清单与上游同步/重比对方法统一记录在 `crates/decx-engine/VENDORED.md`

### 改编源码标注约定

decx-engine 内每个改编自上游的 `.rs` 文件头部都标明自研包名、上游来源、版本、
许可证，以及该文件是“与上游逐字节一致（仅多出头部）”还是“已被 decx-native
修改”（Apache-2.0 §4(b) 要求修改过的文件带显著声明）。后续改动这些文件时：
保持头部状态准确，并在改动处以 `decx-native:` 行内注释标记。
