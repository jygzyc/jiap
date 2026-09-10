//! `android` tool — app analysis over the DECX contract, live device
//! inspection via adb, and framework session management.

use serde_json::Value;

use crate::error::{DecxError, DecxResult};
use crate::iface::{ArgSpec as A, Args, CommandSpec, Interface};
use crate::params::{ClassFilter, ComponentFilter};
use crate::tools::{
    adb::AdbClient, ToolContext,
};

pub fn interface() -> Interface {
    Interface::new("android", "Android app, device, and framework analysis").commands(vec![
        CommandSpec::group(
            "app",
            "Analyze the loaded Android app",
            app_commands(),
        ),
        CommandSpec::group("device", "Inspect a connected Android device via adb", device_commands()),
        CommandSpec::group(
            "framework",
            "Framework jar session management",
            framework_commands(),
        ),
    ])
}

fn app_commands() -> Vec<CommandSpec> {
    let package_filter = || -> Vec<A> {
        vec![
            A::opt("limit", "limit", "Maximum number of returned items"),
            A::multi("include-package", "include-package", "Include only names matching this pattern; repeatable"),
            A::multi("exclude-package", "exclude-package", "Exclude names matching this pattern; repeatable"),
            A::flag("no-regex", "no-regex", "Treat patterns as literal text"),
        ]
    };
    vec![
        CommandSpec::leaf("manifest", "Return the decoded AndroidManifest.xml", vec![], |ctx, a| {
            page_query(ctx, a, |c, page| c.get_app_manifest(page))
        }),
        CommandSpec::leaf("launcher-activity", "Return the launcher activity component", vec![], |ctx, a| {
            page_query(ctx, a, |c, page| c.get_main_activity(page))
        }),
        CommandSpec::leaf("application", "Return the application class context", vec![], |ctx, a| {
            page_query(ctx, a, |c, page| c.get_application(page))
        }),
        CommandSpec::leaf(
            "exported-components",
            "List exported components from the manifest",
            vec![
                A::multi("include", "include", "Include names matching this pattern; repeatable"),
                A::multi("exclude", "exclude", "Exclude names matching this pattern; repeatable"),
                A::flag("no-regex", "no-regex", "Treat patterns as literal text"),
            ],
            |ctx, a| {
                let filter = ComponentFilter {
                    includes: a.strs("include").to_vec(),
                    excludes: a.strs("exclude").to_vec(),
                    regex: if a.flag("no-regex") { Some(false) } else { None },
                };
                let page = page_of(a);
                query(ctx, a, |c| {
                    c.get_exported_components(&filter, page)
                })
            },
        ),
        CommandSpec::leaf("deep-links", "List deep links declared by the app", vec![], |ctx, a| {
            page_query(ctx, a, |c, page| c.get_deep_links(page))
        }),
        CommandSpec::leaf(
            "dynamic-receivers",
            "List dynamically registered receivers",
            package_filter(),
            |ctx, a| {
                let filter = class_filter(a);
                let page = page_of(a);
                query(ctx, a, |c| {
                    c.get_dynamic_receivers(&filter, page)
                })
            },
        ),
        CommandSpec::leaf(
            "framework-service-implementation",
            "Find the implementation of one framework system service",
            vec![A::positional("interface", "System service interface name")],
            |ctx, a| {
                let key = a.str("interface").to_string();
                let page = page_of(a);
                query(ctx, a, |c| {
                    c.get_system_service_impl(&key, page)
                })
            },
        ),
        CommandSpec::leaf(
            "resources",
            "List resource files inside the app",
            vec![
                A::multi("include", "include", "Resource file-name filter; repeatable"),
                A::flag("no-regex", "no-regex", "Treat patterns as literal text"),
            ],
            |ctx, a| {
                let includes = a.strs("include").to_vec();
                let regex = if a.flag("no-regex") { Some(false) } else { None };
                let page = page_of(a);
                query(ctx, a, |c| {
                    c.get_all_resources(&includes, regex, page)
                })
            },
        ),
        CommandSpec::leaf(
            "resource-file",
            "Return one resource file entry",
            vec![A::positional("res", "Resource file path")],
            |ctx, a| {
                let key = a.str("res").to_string();
                let page = page_of(a);
                query(ctx, a, |c| {
                    c.get_resource_file(&key, page)
                })
            },
        ),
        CommandSpec::leaf("strings", "Return app string resources", vec![], |ctx, a| {
            page_query(ctx, a, |c, page| c.get_strings(page))
        }),
        CommandSpec::leaf(
            "aidl-interfaces",
            "List AIDL interfaces declared by the app",
            package_filter(),
            |ctx, a| {
                let filter = class_filter(a);
                let page = page_of(a);
                query(ctx, a, |c| {
                    c.get_aidl_interfaces(&filter, page)
                })
            },
        ),
    ]
}

fn device_commands() -> Vec<CommandSpec> {
    let adb_args = || -> Vec<A> {
        vec![
            A::opt("serial", "serial", "adb device serial"),
            A::opt("adb-path", "adb-path", "Path to the adb binary"),
        ]
    };
    vec![
        CommandSpec::leaf(
            "system-services",
            "List live Binder/system services from the device",
            {
                let mut args = adb_args();
                args.push(A::opt("grep", "grep", "Case-insensitive substring filter over service names/interfaces"));
                args
            },
            |_ctx, a| {
                let mut adb = adb_client(a);
                adb.ensure_available()?;
                let (total, services) = adb.list_system_services()?;
                Ok(crate::tools::adb::filter_system_services(
                    total,
                    &services,
                    a.opt_str("grep"),
                ))
            },
        ),
        CommandSpec::leaf(
            "permission-info",
            "Return structured metadata for one Android permission",
            {
                let mut args = adb_args();
                args.push(A::positional("permission", "Full permission name (android.permission.*)"));
                args
            },
            |_ctx, a| {
                let mut adb = adb_client(a);
                adb.ensure_available()?;
                adb.permission_info(a.str("permission"))
            },
        ),
    ]
}

fn framework_commands() -> Vec<CommandSpec> {
    vec![
        CommandSpec::leaf(
            "open",
            "Open a framework jar as an analysis session",
            vec![
                A::pos_opt("jar", "Framework jar (default: auto-detected via device OEM)"),
                A::one_of("engine", "engine", &["jvm", "native", "kuna"], "Engine backend (default: config / DECX_ENGINE / jvm)"),
                A::opt("port", "port", "DECX HTTP server port to bind"),
                A::opt("name", "name", "Session name"),
                A::flag("force", "force", "Restart matching sessions first"),
                A::opt("timeout", "timeout", "Seconds to wait for server health"),
                A::opt("serial", "serial", "adb device serial (for OEM auto-detection)"),
                A::trailing("jadx", "Everything after the jar is forwarded to jadx-cli"),
            ],
            run_framework_open,
        ),
        CommandSpec::leaf("collect", "Collect framework files from a device (not ported to Rust yet)", vec![], |_, _| {
            Err(DecxError::not_ported("android framework collect"))
        }),
        CommandSpec::leaf("process", "Process collected framework files (not ported to Rust yet)", vec![], |_, _| {
            Err(DecxError::not_ported("android framework process"))
        }),
        CommandSpec::leaf("run", "Collect + process + open in one step (not ported to Rust yet)", vec![], |_, _| {
            Err(DecxError::not_ported("android framework run"))
        }),
    ]
}

// ── shared handler helpers ──────────────────────────────────────────────────

fn page_of(a: &Args) -> u64 {
    a.u64("page").unwrap_or(1)
}

fn class_filter(a: &Args) -> ClassFilter {
    ClassFilter {
        limit: a.u64("limit"),
        includes: a.strs("include-package").to_vec(),
        excludes: a.strs("exclude-package").to_vec(),
        regex: if a.flag("no-regex") { Some(false) } else { None },
    }
}

fn page_query(
    ctx: &ToolContext,
    a: &Args,
    http: impl FnOnce(&crate::client::DecxClient, u64) -> DecxResult<Value>,
) -> DecxResult<Value> {
    http(&super::analysis_client(ctx, a)?, page_of(a))
}

fn query(
    ctx: &ToolContext,
    a: &Args,
    http: impl FnOnce(&crate::client::DecxClient) -> DecxResult<Value>,
) -> DecxResult<Value> {
    http(&super::analysis_client(ctx, a)?)
}

fn adb_client(a: &Args) -> AdbClient {
    AdbClient::new(
        a.opt_str("adb-path").map(str::to_string),
        a.opt_str("serial").map(str::to_string),
    )
}

fn run_framework_open(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let jar = match a.opt_str("jar") {
        Some(jar) => jar.to_string(),
        None => {
            let oem = detect_device_oem(a)?;
            let path = ctx.home.join("framework").join(&oem).join("framework.jar");
            if !path.exists() {
                return Err(DecxError::file(
                    format!(
                        "No processed framework jar for OEM '{oem}' at {}. Run the framework collect/process \
                         pipeline (TypeScript CLI) or pass a jar explicitly.",
                        path.display()
                    ),
                    Some(path.display().to_string()),
                ));
            }
            path.display().to_string()
        }
    };
    let config = crate::config::Config::load(&ctx.home);
    let req = crate::session::lifecycle::OpenRequest {
        file: jar,
        engine_id: Some(config.effective_engine(a.opt_str("engine"))),
        port: a.opt_str("port").map(str::to_string),
        name: a.opt_str("name").map(str::to_string),
        force: a.flag("force"),
        scripts: vec![],
        passthrough: a.strs("jadx").to_vec(),
        timeout_secs: a.u64("timeout").unwrap_or(config.session.open_timeout_secs),
        origin: "decx android framework open".into(),
    };
    crate::session::lifecycle::open_session(&ctx.manager, &ctx.engines, &req, |msg| ctx.notice(msg))
}

/// Auto-detect the framework OEM from a connected device's brand property.
fn detect_device_oem(a: &Args) -> DecxResult<String> {
    let mut adb = AdbClient::new(None, a.opt_str("serial").map(str::to_string));
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
    crate::tools::adb::detect_framework_oem_from_brand(&brand)
}
