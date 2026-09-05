# 可行性研究报告:用 Rust 引擎替换 JADX 核心(最终版)

日期:2026-09-04 · 分支:`native-dev`
结论:**可行且已落地**。Java 反编译核心 = asLody/dexdec(融合为内在引擎);
同机同 dex 实测全面优于现有 JVM/jadx 方案。androguard 三件套已完成使命并从
vendor 中移除(其解析/反汇编能力被 dexdec 全覆盖)。

## 1. 引擎选型(同机实测,输入:testdata/classes.dex,5920 类 / 46185 方法 / 121 万指令)

| 指标 | androguard/dex-decompiler | **asLody/dexdec(采用)** | 现有 JVM/jadx 栈 |
|---|---|---|---|
| 打开+索引 | 60ms(仅索引) | **186ms(含平台符号库解码)** | — |
| 单类交互(最差类 `ActivityResultRegistry`) | 31s;存在 28.8GB 分配直接 abort 进程的类 | **108ms(端到端含 HTTP)** | — |
| 全量批扫(单线程) | 5920 类 >25min 未完成 | **2682 顶层类 65s,0 失败** | — |
| 服务就绪(同 dex) | — | **0.59s(open→healthy)** | **33 分钟仍未就绪**(CLI 默认 300s 超时,实际不可用) |
| 自带测试 | 112/113 | **618/618** | — |
| 输出质量 | switch/循环结构错乱、垃圾临时变量 | **jadx 级**:正确 import、类型、switch 分组合并 | jadx 级 |

判定:dexdec 满足"性能与稳定性大幅提高"的要求,采用为唯一引擎。

## 2. 落地架构:dexdec 融合为内在引擎

```
native/
├─ crates/
│  ├─ decx-core     Project: dexdec ArchiveCatalog 索引 + ClassOutline 成员
│  │                + 反编译/IRDump/引用扫描/批量管线 + 字节上限 LRU 缓存
│  │                + api::dispatch(端点分发,镜像 RouteHandler)
│  ├─ decx-server   axum:GET /health + POST /api/decx/<endpoint>,
│  │                错误封套 {error,message} 与 Kotlin DecxError 状态码一致
│  └─ decx-cli      process open/list/check/close + code 查询命令树
├─ vendor/          dexdec + rusty-dex(照抄,Apache-2.0;约 22 万行引擎代码)
├─ testdata/        测试用 dex(5920 类)
└─ bin/             llvm-dlltool(不入库,见 §4)
```

自研代码约 2400 行,引擎按依赖引入(不是重写、不是 fork 修改)。

### 引擎能力 → DECX 端点映射

| 端点 | dexdec 能力 |
|---|---|
| `get_classes` / `search_method` | `ArchiveCatalog` / `member_catalog`(元数据索引) |
| `get_class_source` / `get_method_source`(java) | `Decompiler::class` / `method`(MethodRequest 支持重载) |
| `--smali` | `decode_method` + `visualizer::method_to_text`(语义 IR 块列表) |
| `get_method_cfg` | CFG 块/边(`EdgeKind`)+ IR 文本 |
| `get_method_xref` / `get_field_xref` / `get_class_xref` | `references(ReferenceTarget)` 字节码级引用(带指令偏移) |
| `get_implementations` / `get_subclasses` | 全量 `ClassOutline` 层级索引(懒构建缓存) |
| `search_global_key` | `ClassSelector::Listed` 批量预热 + grep |

相比上一版(androguard 为辅)的改进:`get_class_xref` 从元数据级近似升级为
dexdec 的**字节码级引用**(带指令偏移);smali/CFG 从 dalvik 文本切换为语义 IR;
方法解析支持重载(descriptor 消歧)。

### 性能修复(融合过程中发现并解决)

1. **主线程栈溢出**:dexdec 递归解析在 opt≥2 的内联大栈帧下超出 Windows 主线程
   1MB 栈 → server 以 0xC0000005 随机崩溃(之前误判为 LTO/工具链 bug)。
   修复:`main` 整体运行在 256MB 栈线程,tokio worker/blocking 线程 64MB 栈。
2. **嵌套类缓存穿透**:批量预热缓存的是顶层所有者 key,嵌套类条目逐个未命中会
   反复重渲染父类(5920 类搜索从 45s 劣化到 10 分钟)。修复:缓存查找先走
   owner key。修复后冷启全库搜索 ~45s,命中后毫秒级。

## 3. dexdec 移植补丁(vendor 内,均已注释标注)

windows-gnu 本机无 C 工具链,C 依赖 opt-in 化:

1. `mimalloc` → feature `mimalloc-allocator`(可选);
2. `.dexsym` 解码改纯 Rust `ruzstd`(`symbol-codec`,运行时必需——内嵌 2MB
   `platform.dexsym` 靠它);C zstd 压缩放 `symbol-encode`(可选,无它时按未压缩
   存储,读取端两种都认);
3. `rusty-dex` 的 zip 只保留纯 Rust `deflate`。

## 4. 工具链备注(本机 Windows,踩坑实录)

- 本机无 MSVC → `native/` rustup override 到 `stable-x86_64-pc-windows-gnu`
  (新 clone 需手动执行一次);
- windows-gnu 下 rustc 需要外部 `dlltool.exe`(windows-sys raw-dylib):NDK 的
  `llvm-dlltool.exe` 拷为 `native/bin/dlltool.exe`,构建前
  `export PATH="/e/Code/decx/native/bin:$PATH"`;
- **链接器必须用 rust-lld**(`.cargo/config.toml` 已配置):windows-gnu 自带 GNU ld
  对此依赖图会间歇性产出启动即段错误的坏二进制(同一源码、同一 opt 有时 5/5 正常
  有时 0/5 崩溃,与是否触及引擎代码无关),换 lld 后彻底稳定;
- **release = opt-level 2,不开 LTO**:fat/thin LTO 与 opt≥3 在 windows-gnu 上
  误编译(`--help` 即段错误);opt2 实测稳定。rustc 1.93 → 1.98.1 复测结论一致;
- dexdec 递归解析需要大栈:main 线程 256MB + tokio 线程 64MB(见 §2)。

## 5. 复现

```bash
cd native && export PATH="/e/Code/decx/native/bin:$PATH"
cargo build --release
cargo test --release -p decx-core -p decx-cli          # 6 + 1 通过
cargo test -p dexdec --no-default-features --features symbol-codec --lib  # 618 通过
cargo test -p decx-core --release dexdec_bench -- --ignored --nocapture
# 端到端:
target/release/decx-native process open <apk|dex> --name demo
target/release/decx-native code get-class-source com.foo.Bar
```

## 6. 真机案例:vivo 全局搜索系统应用(com.vivo.globalsearch)

从已连接设备(`adb pull`)拉取真实系统 APK:42MB,targetSdk 36(Android 16),
R8 混淆,48431 个类。打开 **1.1s**(仅索引,懒反编译)。

功能面板(全部通过):

| 端点 | 真机结果 |
|---|---|
| `get_app_manifest` | 真实二进制 AXML → 文本(v8.80.31.1, targetSdk 36, 权限清单完整) |
| `get_deep_links` / `get_exported_components` / `get_application` | `vglobalsearch://` 深链、`SearchActivity` 导出组件与 intent-filter、`SearchApplication` |
| `get_main_activity` | 正确返回 404 `NO_MAIN_ACTIVITY`——该应用确实无 LAUNCHER 入口(adb resolve-activity 交叉验证) |
| `get_class_source` / `get_class_context` | 真实混淆类 `SearchApplication` 完整 Java(0.41s,import 正确) |
| `get_method_source`(java / --smali) | 混淆方法还原 + 语义 IR 双输出 |
| `get_method_cfg` | CFG + IR 文本,还原 lambda 调用 |
| `search_method`(裸名,48431 类) | **0.42s**(member_catalog 元数据通道) |
| `get_method_xref` | 真实调用点带源码行(`e1.j(SearchApplication.getApplication())`,MemoryPressureMonitor.f:84),8.1s |
| `get_strings` | 真实资源字符串(“智慧桌面”等,resources.arsc 解析) |
| `search_global_key`(带类名过滤) | 50s 扫完 `com.vivo.globalsearch.*` 包并命中 |

规模成本(诚实数据):冷启**全库**批量反编译(无类名过滤)在 48431 类上超过
60 分钟(opt2,8 worker)——真实 R8 应用的混淆大方法远贵于 androidx 测试类;
带 `filter.includes` 的范围搜索是推荐用法(上表 50s)。层级索引
(`get_subclasses`/`get_implementations` 首查)并行构建 36s/48431 类,缓存后毫秒级。
批量/层级 worker 数:`DECX_NATIVE_BATCH_WORKERS`;请求超时:`DECX_NATIVE_REQUEST_TIMEOUT_SECS`。

## 7. 已知差距 / 后续

- 未实现端点:`get_all_resources`/`get_resource_file`/`get_strings`、二进制 AXML
  清单解码、AIDL、MCP、jadx 脚本;
- `get_subclasses`/`get_implementations` 首次查询需构建层级索引(5920 类 ~14s,
  之后毫秒级);`search_global_key` 冷启需全库批量反编译(~45s,之后走缓存);
- dexdec 批量管线单线程(`Decompiler` 为 `&mut self`);并行化可按"每 worker 一个
  context"推进;
- Linux 部署时可开启 `--features mimalloc-allocator` 进一步降低分配开销。

## 8. 附:androguard 反编译核不达标实证(已移除,保留备查)

每类均值 ~730ms(Windows)/ ~195ms(WSL),最差单类 31s;某类触发 28.8GB 单次
分配 → 进程 abort(server 日志实证)。构造 1.9ms、注解类 3.9ms——慢点集中在
`type_infer`/`ssa`/`value_flow` 对内部类/匿名类合并路径的逐方法处理。2026-09-04
从 vendor 移除(其解析能力由 dexdec/rusty-dex 接管,测试 fixture 移至
`native/testdata/`)。
