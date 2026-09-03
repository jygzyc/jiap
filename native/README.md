# native — Rust 重写 DECX 核心的可行性研究

分支 `native-dev`。目标:研究用 [asLody/dexdec](https://github.com/asLody/dexdec) 与
[androguard/dex-decompiler](https://github.com/androguard/dex-decompiler) 替换 JADX 核心
的可行性,用 Rust 达到最高性能,最终结构对齐现有 decx(core / server / cli,cli 通过
HTTP 获取 server 数据)。

结论先行:**结构可行性已验证(已跑通)**;但 androguard 反编译引擎的逐类性能与稳定性
不达标,替换 JADX 的 Java 生成层在当前引擎状态下不可行,详见 [RESEARCH.md](RESEARCH.md)。

## 目录结构(对齐现有仓库)

| 现有 (JVM) | 本目录 (Rust) | 说明 |
|---|---|---|
| `decx/decx-core` | `crates/decx-core` | 项目加载 + 类索引 + 懒反编译 + 字节上限 LRU 缓存(≈ DecompileGuard)+ API 分发(≈ RouteHandler)|
| `decx/decx-server` | `crates/decx-server` | axum HTTP 服务,`GET /health` + `POST /api/decx/<endpoint>`,错误封套 `{error, message}` 与 Kotlin `DecxError` 状态码一致 |
| `decx-cli` | `crates/decx-cli` | `process open/list/check/close` + `code *` 命令树;会话存 `~/.decx-native`(可用 `DECX_NATIVE_HOME` 重定向);stdout 只出 JSON |
| — | `vendor/` | 照抄的 androguard 三件套:`dex-parser`、`dex-bytecode`、`dex-decompiler`(Apache-2.0,补丁表上移到 workspace 根)|

## 构建(Windows + Git Bash)

本机无 MSVC 链接器,且 Git Bash 的 coreutils `link` 会遮蔽,故 `native/` 已设置
rustup 目录覆盖到 `stable-x86_64-pc-windows-gnu`(存于 `~/.rustup/settings.toml`,
未提交;新 clone 需执行 `rustup override set stable-x86_64-pc-windows-gnu`)。

windows-gnu 上 rustc 为 `windows-sys`(raw-dylib)调用外部 `dlltool.exe`,工具链自带版
缺 `as` 会失败;解决:把 Android NDK 的 `llvm-dlltool.exe` 拷为 `native/bin/dlltool.exe`
(已就位,未入库),构建前 `export PATH="/e/Code/decx/native/bin:$PATH"`。

```bash
cd native
export PATH="/e/Code/decx/native/bin:$PATH"   # dlltool
cargo build --release                          # fat LTO
cargo test --release
```

产物:`target/release/decx-native-server.exe`、`target/release/decx-native.exe`
(CLI 按同级目录查找 server 二进制)。

## 用法(对齐 decx-cli 体验)

```bash
./target/release/decx-native process open app.apk --name demo [--port N] [--warm]
./target/release/decx-native process list
./target/release/decx-native code get-classes --limit 20 --includes '^com\.foo\.'
./target/release/decx-native code get-class-source com.foo.Bar [--smali] [--limit N]
./target/release/decx-native code search-global-key 'pattern' [--regex] [--case-sensitive]
./target/release/decx-native code get-method-source 'com.foo.Bar.method'
./target/release/decx-native code get-method-cfg 'com.foo.Bar.method'
./target/release/decx-native code get-method-xref / get-field-xref / get-class-xref
./target/release/decx-native code get-implementations / get-subclasses
./target/release/decx-native process close demo
```

会话记录:`~/.decx-native/sessions.json`;server 日志:`~/.decx-native/logs/<name>.log`。

## 已实现端点(与 `DecxRoutes` 同名同路径)

`get_classes`、`get_class_source`(java/smali + filter.limit)、`get_class_context`、
`search_global_key`、`search_class_key`、`search_method`、`get_method_source`、
`get_method_context`、`get_method_cfg`、`get_method_xref`(跨 dex 汇总)、
`get_field_xref`(跨 dex 汇总)、`get_class_xref`(元数据级结构引用)、
`get_implementations`(直接实现者 + 一层子类)、`get_subclasses`、`get_app_manifest`
(仅 APK 内已解包 XML;二进制 AXML 解码未做)。

未实现(与 Kotlin 版差异):Android 资源/AXML 类端点(`get_all_resources` 等)、MCP、
jadx 脚本。见 RESEARCH.md 的差距清单。
