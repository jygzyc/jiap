//! `android` tool — app analysis over the DECX HTTP API, live device
//! inspection via adb, and framework jar project management.

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::Value;

use crate::error::{DecxError, DecxResult};
use crate::params::{ClassFilter, ComponentFilter};

use super::{
    adb::AdbClient,
    matches_flag, matches_many, matches_u64, target_args, Tool, ToolContext,
};

pub struct AndroidTool;

fn page_arg() -> Arg {
    Arg::new("page").long("page").help("Result page number to fetch")
}

fn parse_class_filter(m: &ArgMatches) -> ClassFilter {
    ClassFilter {
        limit: matches_u64(m, "limit"),
        includes: matches_many(m, "include-package"),
        excludes: matches_many(m, "exclude-package"),
        regex: if matches_flag(m, "no-regex") { Some(false) } else { None },
    }
}

fn command() -> Command {
    Command::new("android")
        .about("Android app, device, and framework analysis")
        .subcommand(
            Command::new("app")
                .about("Analyze the loaded Android app")
                .subcommands([
                    Command::new("manifest").about("Return the decoded AndroidManifest.xml").arg(page_arg()).args(target_args()),
                    Command::new("launcher-activity").about("Return the launcher activity component").arg(page_arg()).args(target_args()),
                    Command::new("application").about("Return the application class context").arg(page_arg()).args(target_args()),
                    Command::new("exported-components")
                        .about("List exported components from the manifest")
                        .args([
                            Arg::new("include").long("include").action(ArgAction::Append).num_args(1).help("Include names matching this pattern; repeatable"),
                            Arg::new("exclude").long("exclude").action(ArgAction::Append).num_args(1).help("Exclude names matching this pattern; repeatable"),
                            Arg::new("no-regex").long("no-regex").action(ArgAction::SetTrue).help("Treat patterns as literal text"),
                        ])
                        .args(target_args()),
                    Command::new("deep-links").about("List deep links declared by the app").arg(page_arg()).args(target_args()),
                    Command::new("dynamic-receivers").about("List dynamically registered receivers").arg(page_arg())
                        .args([
                            Arg::new("include-package").long("include-package").action(ArgAction::Append).num_args(1).help("Package filter; repeatable"),
                            Arg::new("exclude-package").long("exclude-package").action(ArgAction::Append).num_args(1).help("Package exclusion filter; repeatable"),
                            Arg::new("no-regex").long("no-regex").action(ArgAction::SetTrue).help("Treat patterns as literal text"),
                        ])
                        .args(target_args()),
                    Command::new("framework-service-implementation")
                        .about("Find the implementation of one framework system service")
                        .arg(Arg::new("interface").required(true).value_name("INTERFACE"))
                        .arg(page_arg())
                        .args(target_args()),
                    Command::new("resources")
                        .about("List resource files inside the app")
                        .args([Arg::new("include").long("include").action(ArgAction::Append).num_args(1).help("Resource file-name filter; repeatable"),
                               Arg::new("no-regex").long("no-regex").action(ArgAction::SetTrue).help("Treat patterns as literal text")])
                        .arg(page_arg())
                        .args(target_args()),
                    Command::new("resource-file")
                        .about("Return one resource file entry")
                        .arg(Arg::new("res").required(true).value_name("RES"))
                        .arg(page_arg())
                        .args(target_args()),
                    Command::new("strings").about("Return app string resources").arg(page_arg()).args(target_args()),
                    Command::new("aidl-interfaces")
                        .about("List AIDL interfaces declared by the app")
                        .args([
                            Arg::new("include-package").long("include-package").action(ArgAction::Append).num_args(1).help("Package filter; repeatable"),
                            Arg::new("exclude-package").long("exclude-package").action(ArgAction::Append).num_args(1).help("Package exclusion filter; repeatable"),
                            Arg::new("no-regex").long("no-regex").action(ArgAction::SetTrue).help("Treat patterns as literal text"),
                        ])
                        .arg(page_arg())
                        .args(target_args()),
                ]),
        )
        .subcommand(
            Command::new("device")
                .about("Inspect a connected Android device via adb")
                .subcommands([
                    Command::new("system-services")
                        .about("List live Binder/system services from the device")
                        .args([
                            Arg::new("serial").long("serial").num_args(1).help("adb device serial"),
                            Arg::new("adb-path").long("adb-path").num_args(1).help("Path to the adb binary"),
                            Arg::new("grep").long("grep").num_args(1).help("Case-insensitive substring filter over service names/interfaces"),
                        ]),
                    Command::new("permission-info")
                        .about("Return structured metadata for one Android permission")
                        .arg(Arg::new("permission").required(true).value_name("PERMISSION"))
                        .args([
                            Arg::new("serial").long("serial").num_args(1).help("adb device serial"),
                            Arg::new("adb-path").long("adb-path").num_args(1).help("Path to the adb binary"),
                        ]),
                ]),
        )
        .subcommand(
            Command::new("framework")
                .about("Framework jar collection and project management")
                .long_about(
                    "Framework `open` manages a framework jar as a regular analysis project. \
                     `collect`/`process`/`run` (device-side collection and image extraction) are \
                     not ported to the Rust CLI yet.",
                )
                .subcommands([
                    Command::new("open")
                        .about("Open a framework jar as a DECX analysis project")
                        .arg(Arg::new("jar").num_args(0..=1).value_name("JAR").help("Framework jar (default: auto-detected via device OEM)"))
                        .args([
                            Arg::new("engine").long("engine").help("Analysis engine backend (jvm | native)"),
                            Arg::new("port").long("port").help("DECX HTTP server port to bind"),
                            Arg::new("name").long("name").short('n').help("Project name"),
                            Arg::new("force").long("force").action(ArgAction::SetTrue).help("Restart matching projects first"),
                            Arg::new("timeout").long("timeout").help("Seconds to wait for server health (default 300)"),
                            Arg::new("serial").long("serial").num_args(1).help("adb device serial (for OEM auto-detection)"),
                        ])
                        .arg(
                            Arg::new("passthrough")
                                .value_name("JADX_ARGS")
                                .num_args(0..)
                                .trailing_var_arg(true)
                                .allow_hyphen_values(true)
                                .help("Everything after JAR is forwarded to jadx-cli"),
                        ),
                    Command::new("collect").about("Collect framework files from a device (not ported to Rust yet)"),
                    Command::new("process").about("Process collected framework files (not ported to Rust yet)"),
                    Command::new("run").about("Collect + process + open in one step (not ported to Rust yet)"),
                ]),
        )
}

fn require(m: &ArgMatches, id: &str) -> String {
    m.get_one::<String>(id).cloned().unwrap_or_default()
}

impl AndroidTool {
    fn run_app(&self, ctx: &ToolContext, m: &ArgMatches) -> DecxResult<Value> {
        let Some((name, m)) = m.subcommand() else {
            return Err(DecxError::usage(
                "No android app subcommand given (manifest | launcher-activity | application | \
                 exported-components | deep-links | dynamic-receivers | framework-service-implementation | \
                 resources | resource-file | strings | aidl-interfaces)",
            ));
        };
        let (port, _) = super::resolve_target(ctx, m)?;
        let client = crate::client::DecxClient::new(port);
        let page = matches_u64(m, "page").unwrap_or(1);
        match name {
            "manifest" => client.get_app_manifest(page),
            "launcher-activity" => client.get_main_activity(page),
            "application" => client.get_application(page),
            "exported-components" => {
                let filter = ComponentFilter {
                    includes: matches_many(m, "include"),
                    excludes: matches_many(m, "exclude"),
                    regex: if matches_flag(m, "no-regex") { Some(false) } else { None },
                };
                client.get_exported_components(&filter, page)
            }
            "deep-links" => client.get_deep_links(page),
            "dynamic-receivers" => client.get_dynamic_receivers(&parse_class_filter(m), page),
            "framework-service-implementation" => {
                client.get_system_service_impl(&require(m, "interface"), page)
            }
            "resources" => {
                let includes = matches_many(m, "include");
                let regex = if matches_flag(m, "no-regex") { Some(false) } else { None };
                client.get_all_resources(&includes, regex, page)
            }
            "resource-file" => client.get_resource_file(&require(m, "res"), page),
            "strings" => client.get_strings(page),
            "aidl-interfaces" => client.get_aidl_interfaces(&parse_class_filter(m), page),
            other => Err(DecxError::usage(format!("Unknown android app subcommand '{other}'"))),
        }
    }

    fn run_device(&self, _ctx: &ToolContext, m: &ArgMatches) -> DecxResult<Value> {
        let Some((name, m)) = m.subcommand() else {
            return Err(DecxError::usage("No android device subcommand given (system-services | permission-info)"));
        };
        let mut adb = AdbClient::new(
            m.get_one::<String>("adb-path").cloned(),
            m.get_one::<String>("serial").cloned(),
        );
        adb.ensure_available()?;
        match name {
            "system-services" => {
                let (total, services) = adb.list_system_services()?;
                Ok(super::adb::filter_system_services(
                    total,
                    &services,
                    m.get_one::<String>("grep").map(String::as_str),
                ))
            }
            "permission-info" => adb.permission_info(&require(m, "permission")),
            other => Err(DecxError::usage(format!("Unknown android device subcommand '{other}'"))),
        }
    }

    fn run_framework(&self, ctx: &ToolContext, m: &ArgMatches) -> DecxResult<Value> {
        let Some((name, m)) = m.subcommand() else {
            return Err(DecxError::usage("No android framework subcommand given (open | collect | process | run)"));
        };
        match name {
            "open" => {
                let jar = m.get_one::<String>("jar").map(String::as_str).filter(|s| !s.is_empty());
                let jar = match jar {
                    Some(jar) => jar.to_string(),
                    None => {
                        // No jar given: auto-detect the device OEM and use its
                        // collected framework jar when one exists.
                        let oem = detect_device_oem(m)?;
                        let path = ctx.home.join("framework").join(&oem).join("framework.jar");
                        if !path.exists() {
                            return Err(DecxError::file(
                                format!(
                                    "No processed framework jar for OEM '{oem}' at {}. Run the framework \
                                     collect/process pipeline (TypeScript CLI) or pass a jar explicitly.",
                                    path.display()
                                ),
                                Some(path.display().to_string()),
                            ));
                        }
                        path.display().to_string()
                    }
                };
                let req = crate::engine::launcher::OpenRequest {
                    file: jar,
                    engine_id: m.get_one::<String>("engine").cloned(),
                    port: m.get_one::<String>("port").cloned(),
                    name: m.get_one::<String>("name").cloned(),
                    force: matches_flag(m, "force"),
                    scripts: vec![],
                    passthrough: matches_many(m, "passthrough"),
                    timeout_secs: matches_u64(m, "timeout").unwrap_or(300),
                };
                crate::engine::launcher::open_analysis_target(&ctx.manager, &ctx.engines, &req, |msg| ctx.notice(msg))
            }
            "collect" | "process" | "run" => Err(DecxError::not_ported(format!("android framework {name}"))),
            other => Err(DecxError::usage(format!("Unknown android framework subcommand '{other}'"))),
        }
    }
}

/// Auto-detect the framework OEM from a connected device's brand property.
fn detect_device_oem(m: &ArgMatches) -> DecxResult<String> {
    let mut adb = AdbClient::new(None, m.get_one::<String>("serial").cloned());
    adb.ensure_available()?;
    let mut brand = String::new();
    for prop in ["ro.product.vendor.brand", "ro.product.brand", "ro.product.manufacturer"] {
        if let Ok(value) = adb.get_prop(prop) {
            if !value.is_empty() {
                brand = value;
                break;
            }
        }
    }
    super::adb::detect_framework_oem_from_brand(&brand)
}

impl Tool for AndroidTool {
    fn id(&self) -> &'static str {
        "android"
    }

    fn commands(&self) -> Vec<Command> {
        vec![command()]
    }

    fn run(&self, ctx: &ToolContext, matches: &ArgMatches) -> DecxResult<Value> {
        let Some((name, m)) = matches.subcommand() else {
            return Err(DecxError::usage("No android subcommand given (app | device | framework)"));
        };
        match name {
            "app" => self.run_app(ctx, m),
            "device" => self.run_device(ctx, m),
            "framework" => self.run_framework(ctx, m),
            other => Err(DecxError::usage(format!("Unknown android subcommand '{other}'"))),
        }
    }
}
