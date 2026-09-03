# 可行性研究报告:用 Rust 引擎替换 JADX 核心

日期:2026-09-04 · 分支:`native-dev` · 结论:**部分可行** — 工程结构与服务契约 100% 可
平移且已跑通;但受调研的两个引擎中可直接落地的 androguard 反编译核在性能与健壮性上
不达标,Java 生成层暂不建议替换;解析/反汇编层可以立即采用。

## 1. 调研对象

| 仓库 | 语言/规模 | 定位 | 可借用性 |
|---|---|---|---|
| [asLody/dexdec](https://github.com/asLody/dexdec) | Rust,`rusty-dex` 6.3k 行解析 + `dexdec` 核心 **21.6 万行** | 商业级:懒反编译、故障隔离、Tauri GUI、MCP、可逆重命名 | 引擎成熟度高,但体量过大(21.6 万行),整体 vendor 进本仓库不现实;可作为日后"整库依赖"候选(Cargo git 依赖,不进源码树) |
| [androguard/dex-decompiler](https://github.com/androguard/dex-decompiler) | Rust,解析 2.6k + 反汇编 4.2k + 反编译 4.7 万行 | 研究级:CFG→SSA IR→region 结构化→Java,附污点分析/semgrep/模拟器 | 全链路可 vendor、管线清晰,本次采用;`dex-parser`/`dex-bytecode` 质量好 |

两者均为 Apache-2.0,允许 vendor(保留 LICENSE 与出处)。

## 2. 已落地(可运行)

workspace `native/`:vendor androguard 三件套 + 自研三层:

```
crates/decx-core    Project(索引/缓存) + api::dispatch(端点分发)
crates/decx-server  axum,GET /health + POST /api/decx/<endpoint>,错误封套与
                    Kotlin DecxError 同构(400/404/503/504 映射一致)
crates/decx-cli     clap 命令树 process/code,会话落盘 + 心跳等待 + 零依赖 HTTP 客户端
vendor/             dex-parser、dex-bytecode、dex-decompiler(照抄,仅挪走 [patch])
```

验证结果(release,fat LTO):

- 引擎自带测试 112/113 通过(1 个 crypto fixture 上游即失败);
- 自研单测:decx-core 4、decx-cli 96(含 chunked HTTP 解码)全过;
- E2E(Windows,Git Bash):`process open`(5.4MB dex,索引 5920 类 0.06s)→
  `get-classes` 正则过滤 → `get-class-source`(Java + smali + filter.limit 截断)→
  `search-method` → `get-method-source` → `get-method-cfg`(CFG 节点/边 + 字节码)→
  `get-class-xref`(结构引用)→ `get-subclasses` → `search-global-key`(340 类批量
  反编译 + grep,16.3s)→ `process close`。stdout JSON-only、错误 `{error,message}`
  与现有 CLI 契约一致。

与 Kotlin 版对齐的关键设计:字节上限 LRU 源码缓存(≈ `BoundedCodeCache`,默认 1GiB,
`DECX_NATIVE_CACHE_MAX_BYTES` 可调);端点由单一 `api::dispatch` 分发(≈ `RouteHandler`);
重活进 `spawn_blocking`,不占异步运行时。

## 3. 关键问题:反编译核性能与健壮性不达标

测试输入:vendored 自带 `testdata/classes.dex`(androidx/androidx.activity 等,5920 类,
5.4MB)。全部为 release + 单实例复用 Decompiler 的顺序/批量计时:

| 环境 | 每类均值(Restructure) | 参照 |
|---|---|---|
| Windows GNU 工具链 | ~730ms/类;最差单类 31s | jadx 同等代码 ~1–10ms/类 |
| WSL2 Linux(同机) | ~195ms/类 | 系统层差 ~4 倍(分配器),其余为算法层差距 |

定位结论:

1. **不是**构造开销(构造 1.9ms)也**不是**简单类(注解类 3.9ms);慢在带内部类/
   匿名类合并的复杂类,逐方法成本失控,疑似若干 O(全 dex) 的每方法操作
   (`type_infer`/`value_flow`/`ssa` 路径),未深修——那是引擎作者的问题域。
2. **健壮性**:该 dex 上存在触发单次 **28.8GB 分配 → 进程 abort** 的类(服务端日志
   实证)。abort 无法被 `catch_unwind` 拦截,当前以批量路径 + `catch_unwind` 隔离普通
   panic,但 OOM 类崩溃无法在进程内防御(多进程/单类隔离才有效,又是 jadx 场景不会
   遇到的额外复杂度)。
3. 全量扫描 5920 类:WSL 下 25 分钟未跑完(timeout);jadx 通常 < 1 分钟。

**判定**:以"替换 JADX 核心、达到最高性能"为标准,androguard 反编译核现状不可用。
它适合做静态扫描/污点分析类批处理(其 semgrep/污点/检测器部分反而是亮点),不适合
作为交互式分析服务的 Java 生成层。

## 4. 建议(按投入产出排序)

1. **采纳解析层**:`dex-parser` + `dex-bytecode` 快而稳(索引 5920 类 60ms;smali/CFG/
   交叉引用端点已全部基于它跑通),可先替换 CLI 侧轻量分析,不必动 JADX。
2. **反编译层维持 JADX**:DECX 的 `DecompileGuard`/`BoundedCodeCache` 已解决 jadx 的
   内存问题;jadx 的逐类毫秒级性能 Rust 侧目前无现成替代。
3. **观察 asLody/dexdec**:成熟度最高且本身就是库 + CLI + MCP 形态,若引入,应以
   Cargo git 依赖整库引用(不 vendor 源码),在我们 `Project` 层后面对齐同一套端点
   即可热插拔——本次的 `crates/decx-core` 接口设计已按此预留。
4. **如果要继续自研**:照抄 asLody 的懒反编译与故障隔离架构,逐 pass 移植;按当前
   测得差距,工作量以人月计,不属于一次会话可完成项。

## 5. 与现有 DECX 的功能差距清单

未实现:`get_all_resources`/`get_resource_file`/`get_strings`(需 resources.arsc 解析)、
二进制 AXML 清单解码、AIDL 端点、MCP 传输、jadx 脚本、`process open` 的 JVM 参数族
(`-Xmx`/jadx-cli 透传在 native 形态下语义不同)。
已实现但为 v1 语义:`get_class_xref` 为元数据级(父类/接口/字段类型/签名),未扫代码级
类型引用;`get_implementations` 只展开一层;分页 `page` 仅对 `get_classes`/搜索生效。

## 6. 复现实验数据

```bash
cargo test -p decx-core --release warm_bench -- --ignored --nocapture  # 模式对比/微基准
cargo test -p decx-core --release batch_all  -- --ignored --nocapture  # 全量失败枚举
```

服务器 OOM 实证日志:`memory allocation of 30953970812 bytes failed`(见正文 §3.2)。
