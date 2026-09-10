# decx-cli 重构笔记(工作状态,非最终文档)

仓库:E:/Code/decx-cli-dev/decx-cli(单一 crate,旧 crates/ 已删)。

## 架构 v4(当前生效,来自用户 4 轮指示)

1. CLI 内部管理命令(session/engine/settings/tools/self/android device+framework)在 Rust 侧注册,**不进 config.json**。
2. config.json 注册**工具域(tools)× 引擎(engines)**:
   - `tools[]`:如 `java`(engines: jvm+native,APK/DEX/JAR 的 code/android 分析命令)、`binary`(engines: kuna,ELF/PE/Mach-O;将来 ida 也挂 binary 域)。命令平铺在工具下:`decx java classes`、`decx java manifest`、`decx binary search-global`。
   - `engines[]`:纯引擎(id/description/docs/binary{kind,path,env,exe_suffix,search_sibling,search_dirs}/launch{command 占位符 {binary}{target}{port}{java_heap},scripts:"positional"可选,trailing_args,params[]})。**引擎不携带命令树、无 extends**。
   - 命令级 `engines` 支持列表(空=工具全部引擎)表达引擎专属命令:java 域的 taint-scan 声明 `"engines":["native"]`。
3. jadx 相关逻辑全部移出 CLI(jadx 归 jvm server 自己);jvm 引擎声明通用 `trailing_args: true`,`session open -- <args>` 原样透传给 server(server 负责归一化)。删除 src/engine/jadx.rs。
4. 不考虑任何向后兼容。

## config.json(v4,已写盘)

common_args: session/port/page(编译期烘进每个工具叶子,CLI 拥有的传输/分页关注点)。
tools.java(engines jvm+native)命令(全部带 page always→1 映射):
classes(get_classes;limit/include-package/exclude-package/no-regex→filter.*)、search-global(search_global_key;keyword→key,limit/case-sensitive/include-package/exclude-package/no-regex→search.*,regex invert)、class-context(get_class_context;class→cls)、class-source(get_class_source;class→cls,limit→filter.limit,language→language(java|kotlin|auto),smali bool default false)、method-source(get_method_source;signature→mth,同上)、method-context/method-cfg(get_method_context/get_method_cfg;signature→mth)、search-class(search_class_key;class→cls+pattern→key+limit required→grep.limit default 0+case-sensitive+no-regex invert)、search-method(search_method;name→mth)、xref-method/xref-class/xref-field(get_*_xref;signature/class/field→mth/cls/fld)、implementations(get_implementations;interface→iface)、subclasses(get_subclasses;class→cls)、taint-scan(taint_scan;engines[native];max-rounds→maxRounds,max-findings→maxFindings,when:set)、manifest(get_app_manifest)、launcher-activity(get_main_activity)、application(get_application)、exported-components(get_exported_components;include/exclude→includes/excludes,no-regex→regex invert)、deep-links(get_deep_links)、dynamic-receivers(get_dynamic_receivers;filter.*)、framework-service-implementation(get_system_service_impl;interface→iface)、resources(get_all_resources;include→filter.includes,no-regex→filter.regex)、resource-file(get_resource_file;res→res)、strings(get_strings)、aidl-interfaces(get_aidl_interfaces;filter.*)。
tools.binary(engines kuna):classes(get_classes)、class-source(get_class_source)、method-source(get_method_source;function→mth,positional"Function name")、search-method(search_method;name→mth)、search-global(search_global_key;keyword→key)、strings(get_strings)。
engines: jvm(java-jar,decx-server.jar,env DECX_SERVER_HOME,launch java -Xmx{java_heap} -jar {binary} {target} --port {port},scripts positional,trailing_args,params port/log-level)、native(program,decx-native-server,env DECX_NATIVE_SERVER,exe_suffix,search_sibling,search_dirs[decx-native/target/release],launch {binary} {target} --port {port},params port/warm/taint-rules)、kuna(program,decx-kuna-server,env DECX_KUNA_SERVER,exe_suffix,search_sibling,launch {binary} {target} --port {port},params port)。kuna docs 注明 decx-kuna-server 在 decx 主仓库构建,不在此 CLI workspace。

## 本轮已重写的文件

- src/schema.rs(新):UnifiedConfig{version,common_args,tools,engines};ToolDef{name,about,engines,commands};CommandDef 增 engines 字段;EngineDef 删 commands/commands_extends;LaunchDef.passthrough→trailing_args:bool;validate()(引擎 id 规则/binary.kind/占位符校验/scripts 校验/params 校验;工具名规则/engines 非空⊆引擎表/命令重名/命令级 engines⊆工具 engines/common arg 撞名/request 映射类型检查;孤儿引擎报"serves no tool")。测试:repo config 有效、java/binary 工具断言、taint-scan engines==["native"]、非法占位符/未声明映射参数/未知工具引擎/命令 engines 越界/孤儿引擎/common 撞名。
- src/spec.rs(新):ArgKind/ArgSpecS/MapType/MapWhen/MapDefault/FieldMap/Route/CmdSpec{name,about,args,subs,engines,route}+leaf_paths/find_leaf(要求 self.subs.is_empty 才返回 Some)/supports_engine;ToolSpec{name,about,engines,commands}+find_leaf(command 相对路径)/command_paths/command_paths_for(engine);find_tool(name)/find_leaf("java.classes")走 crate::engines_gen::TOOLS;ParamSpec/ParamKind/BinaryKind/BinarySpec/LaunchSpec{command,scripts,trailing_args,params};EngineSpec{id,description,docs,binary,launch,tools}+find_param。注意 CmdSpec.find_leaf 以自身 name 为第一段,ToolSpec.find_leaf 传入不含工具名的相对路径。

## 待办(按序)

1. ~~build.rs~~ ✅ 已重写:TOOLS + ENGINES, // 行注释。
2. ~~iface.rs~~ ✅ CommandSpec{handler,static_leaf};tools_interfaces(TOOLS)→Interface 列表;run_command 分发到 run_route(ctx,tool,leaf,args) 或 handler。
3. ~~main.rs~~ ✅ 注册 = internal interfaces + TOOLS(名字互斥,平铺拼接);manager=SessionManager::open(&home)(返回 Arc);外部工具 passthrough 保留。
4. ~~src/engine/~~ ✅ jadx.rs 已删;build_launch_command 用 trailing_args 原样透传(不做 jadx 归一化,归 jvm server)。
5. ~~src/commands/~~ ✅ trailing id jadx→server-args(session_cmd + android_cmd framework open);engine_cmd show/list 改用 tools_of(id)+command_paths_for(id)+tool.find_leaf(rel)列命令,summary 带 tools+command_count。
6. ~~lifecycle.rs~~ ✅ TargetSpec.engine_args=Vec<String>(render_engine_args 烘结)。
7. ✅ cargo check + cargo test 全绿(82 passed);冒烟:--help 顶层=session/engine/android/settings/tools/self+java/binary;java 27 叶子;engine show native 含 java.taint-scan;binary method-source 含烘入的 session/port/page + positional function;session open 尾参 server-args;engine list 输出 tools+command_count(jvm25/native26/kuna6)。
8. ~~v5/v6 android 重构~~ ✅:android 顶层命令组取消,设备巡检 + framework 处理作为 java 工具域的 LOCAL 命令承接。机制:config.json 叶子可声明 `"local": "<handler-id>"`(与 endpoint 互斥,禁 request/engines,不烘入 common args);schema 校验;build.rs 编入 CmdSpec.local;iface::from_static 经 `commands::local::lookup` 表绑 handler(未知 id 启动即 panic);ToolSpec::command_paths_for 排除 local 叶子(engine show/list 不列)。
9. ~~framework 处理回归(用户 m00641:"别把 framework 处理的部分丢了啊")~~ ✅:TS `decx-cli/src/android/framework*.ts` 全量移植到 `src/android_sdk/`:`framework.rs`(分层采集 collector、dex 提取/apex 载荷 processor、packer、collect/process/pack/run 四流程,`decx java android framework collect|process|pack|run`;run 不自动开 session,产出后 `decx session open <jar>`),`zip_util.rs`(bsdtar/unzip 外部工具),`ext4.rs`(纯 std ext4 载荷读取,夹具 `tests/fixtures/apex_payload_ext4.img`),adb.rs 增 pull/framework_oem/device_model。真机验证(vivo V2324A/Android 16):collect 847MB→process 171 dex→pack 115.6MB jar 全通;该机 `/apex` find 无权限→tier2 正确回退拉全量 .apex,载荷为 ext4 走纯 Rust 解析。~~WSL 路径翻译 + debugfs/erofs 惰性解析保留~~ → WSL 委托已按用户要求删除(用户 m00763:"wsl 那条路子删了,不需要"):外部工具仅 PATH 查找(command_exists),Windows 上 EROFS/不支持 ext4 特性直接报错引导去 Linux/macOS 跑;未移植 TS 的 packaged bin.tar.gz 分发。~~Windows 上 EROFS/不支持 ext4 特性直接报错引导去 Linux/macOS 跑~~ → 已补齐纯 Rust EROFS 读取器 `erofs.rs`(superblock/inode/dirent/zmap 解码, LZ4 解压, fragment dedupe/ztailpacking/big pcluster/SHIFTED plain, 6 张真机 vivo 镜像与 fsck.erofs 参考输出逐字节交叉验证);`framework.rs` 的 `extract_payload_natively` 现同时覆盖 ext4+erofs,外部 erofs-utils 仅在Unsupported 特性时兜底(PATH 查找)。集成测试 `processes_erofs_payload_apex_natively` 在 Windows 上端到端验证 EROFS .apex 原生解包。`.artifact.json` 元数据/OEM 目录分层/apex 模块命名空间与 TS 一致。
10. 剩余收尾:settings.rs 删 legacy config.json 回退(v4 无兼容);README 命令示例改 decx java/binary 形态;REFACTOR_NOTES.md 全部完成后删除。

## 历史要点(防回归)

- workspace 曾是 crates/{decx-cli,decx-cli-core},已合并为单 crate;decx-kuna/dex 不在本 workspace。
- 曾有 v3 设计(config.json 带 internal handler id 注册内部命令)被用户否决:v4 内部命令回归 Rust 注册。
- 上轮 cargo check 遗留错误(部分已过时):main.rs settings use 冲突;engines_gen.rs //! 注释 E0753;lifecycle.rs engine_args 类型不匹配;这些在新架构下逐个验证。
- settings.rs 已有 legacy config.json 回退逻辑(读旧格式)——v4 无兼容要求,应删除该回退。
- 旧 src/commands/{config_tool,engine_tool,self_tool,session_tool,tools_tool,android_tool}.rs 已 git rm。
