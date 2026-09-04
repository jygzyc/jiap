# 可行性研究报告:用 Rust 引擎替换 JADX 核心(最终版)

日期:2026-09-04 · 分支:`native-dev`
结论:**可行,且已落地**。Java 反编译核心采用 asLody/dexdec(照抄 vendor + 少量移植
补丁),解析/反汇编层采用 androguard 三件套;原生栈在同一台机器、同一个 dex 上的
实测表现全面优于现有 JVM/jadx 方案。

## 1. 决策与依据

核心诉求是 **Java 反编译的性能与稳定性**。对两个候选引擎做了同机实测(输入:
androguard 自带 testdata/classes.dex,5920 类 / 46185 方法 / 121 万指令,5.4MB):

| 指标 | androguard/dex-decompiler | **asLody/dexdec(采用)** | 现有 JVM/jadx 栈 |
|---|---|---|---|
| 打开+索引 | 60ms(仅索引) | **186ms(含平台符号库解码)** | — |
| 单类交互(最差类 `ActivityResultRegistry`) | 31s,且存在 28.8GB 分配直接 abort 进程的类 | **108ms(端到端含 HTTP)** | — |
| 全量批扫(单线程) | 5920 类 >25min 未完成(WSL) | **2682 顶层类 65s,0 失败** | — |
| 服务就绪(同 dex) | — | **0.59s(open→healthy)** | **33 分钟仍未就绪**(CLI 默认 300s 超时,实际即不可用) |
| 自带测试 | 112/113 | **618/618** | — |
| 输出质量 | switch/循环结构错乱、垃圾临时变量 | **jadx 级**:正确 import、类型、switch 分组合并、字段提升 | jadx 级 |

判定:androguard 的反编译核性能/健壮性不达标(详见表后附注);dexdec 成熟度与
性能满足"大幅提高"要求,采用为 Java 生成引擎;androguard 保留作解析器/反汇编器
(smali、方法 CFG、调用/字段交叉引用——这些是它的强项,且已在原生栈上跑通)。

## 2. 已落地架构

```
native/
├─ crates/
│  ├─ decx-core     Project: 类索引(andrg) + dexdec Java 引擎 + 字节上限 LRU 缓存
│  │                + api::dispatch(端点分发,镜像 RouteHandler)
│  ├─ decx-server   axum:GET /health + POST /api/decx/<endpoint>,
│  │                错误封套 {error,message} 与 Kotlin DecxError 状态码一致
│  └─ decx-cli      process open/list/check/close + code 查询命令树,
│                   stdout JSON-only,会话存 ~/.decx-native(DECX_NATIVE_HOME 可重定向)
└─ vendor/          照抄的上游引擎(Apache-2.0,LICENSE 保留)
   ├─ dexdec + rusty-dex        asLody/dexdec —— Java 生成核心
   └─ dex-parser + dex-bytecode + dex-decompiler   androguard —— smali/CFG/xref
```

端点(与 `DecxRoutes` 同名同路径):`get_classes`、`get_class_source`(java=dexdec,
smali=androguard,支持 filter.limit)、`get_class_context`、`search_global_key`
(dexdec 批量预热后 grep)、`search_class_key`、`search_method`、`get_method_source`
(java=dexdec / smali)、`get_method_context`、`get_method_cfg`(字节码 CFG)、
`get_method_xref`、`get_field_xref`、`get_class_xref`、`get_implementations`、
`get_subclasses`、`get_app_manifest`。

E2E 已验证(release):open(5920 类 0.59s)→ get-classes 正则过滤 →
get-class-source → get-method-source → get-method-cfg → xref 三件 →
search-global-key → close,stdout JSON-only,与现有 CLI 契约一致。

## 3. dexdec 移植补丁(vendor 内,均已注释标注)

windows-gnu 本机无 MSVC/C 工具链,故将 C 依赖 opt-in 化:

1. `mimalloc` → feature `mimalloc-allocator`(可选,库本身不依赖);
2. `.dexsym` 解码改用纯 Rust `ruzstd`(`symbol-codec`,运行时必需——内嵌 2MB
   `platform.dexsym` 平台符号库靠它解码);C zstd 压缩编码放 `symbol-encode`
   (可选,无它时按未压缩存储,读取端两种都认);
3. `rusty-dex` 的 zip 只保留纯 Rust `deflate`(APK 只需要 deflate/stored);
4. workspace `[patch]` 上移到根 `Cargo.toml`(cargo 规则:仅根生效)。

效果:dexdec 618/618 测试通过(含全部平台层级分析测试)。

## 4. 工具链备注(本机 Windows)

- 本机无 MSVC 链接器且 Git Bash coreutils `link` 会遮蔽 → `native/` 已 rustup
  override 到 `stable-x86_64-pc-windows-gnu`(新 clone 需
  `rustup override set stable-x86_64-pc-windows-gnu`);
- windows-gnu 下 rustc 需要外部 `dlltool.exe`(windows-sys raw-dylib):已把 NDK 的
  `llvm-dlltool.exe` 拷为 `native/bin/dlltool.exe`(gitignore,不入库),构建前
  `export PATH="/e/Code/decx/native/bin:$PATH"`;
- **fat LTO 误编译**:release + `lto="fat"` 时 server 二进制在 main 前段错误
  (`--help` 即崩);改 `lto="thin"` 后正常,性能差异可忽略。已固化到 profile。

## 5. 复现

```bash
cd native && export PATH="/e/Code/decx/native/bin:$PATH"
cargo build --release
cargo test --release -p decx-core -p decx-cli
cargo test -p dexdec --no-default-features --features symbol-codec --lib
# 引擎对比基准(ignored by default):
cargo test -p decx-core --release dexdec_bench -- --ignored --nocapture
# 端到端:
target/release/decx-native process open <apk|dex> --name demo
target/release/decx-native code get-class-source com.foo.Bar
```

## 6. 附:androguard 反编译核不达标的实证(保留备查)

- 每类均值 ~730ms(Windows)/ ~195ms(WSL;系统层差 ~4 倍,余为算法差距),
  最差单类 31s;构造 1.9ms、注解类 3.9ms——慢在内部类/匿名类合并路径;
- 某类触发 28.8GB 单次分配 → 进程 abort(catch_unwind 无法拦截),server 实证
  日志 `memory allocation of 30953970812 bytes failed`;
- 原因分析:构造与简单类都快,慢点集中在 `type_infer`/`ssa`/`value_flow` 对
  复杂类的逐方法处理,存在疑似 O(全 dex) 的每方法操作。未深修——被 dexdec 取代。

## 7. 已知差距 / 后续

- 与 Kotlin 版差异:`get_all_resources`/`get_resource_file`/`get_strings`(需
  resources.arsc/字符串表端点)、二进制 AXML 清单解码、AIDL、MCP、jadx 脚本;
- `get_class_xref` 为元数据级引用(父类/接口/字段类型/签名),代码级类型引用未扫;
- dexdec 批量管线为单线程(`Decompiler` 是 `&mut self`);并行化可用"每 worker 一个
  context"实现,属后续优化;
- mimalloc 在本机不可编译(无 C 工具链),Linux 部署时可开启
  (`--features mimalloc-allocator`)进一步降低分配开销。
