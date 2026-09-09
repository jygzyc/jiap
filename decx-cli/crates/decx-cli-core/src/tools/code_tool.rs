//! `code` tool — decompiled code queries over the DECX HTTP API.

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::Value;

use crate::error::{DecxError, DecxResult};
use crate::params::{ClassFilter, ClassGrep, GlobalSearch, SourceFilter};

use super::{matches_many, matches_u64, target_args, Tool, ToolContext};

pub struct CodeTool;

fn page_arg() -> Arg {
    Arg::new("page").long("page").help("Result page number to fetch")
}

fn package_filter_args(cmd: Command) -> Command {
    cmd.args([
        Arg::new("limit").long("limit").help("Maximum number of returned items"),
        Arg::new("include-package")
            .long("include-package")
            .action(ArgAction::Append)
            .num_args(1)
            .help("Include only names matching this package pattern; repeatable"),
        Arg::new("exclude-package")
            .long("exclude-package")
            .action(ArgAction::Append)
            .num_args(1)
            .help("Exclude names matching this package pattern; repeatable"),
        Arg::new("regex")
            .long("regex")
            .action(ArgAction::SetTrue)
            .default_value("true")
            .help("Treat patterns as regular expressions (default) — use --no-regex for literal text"),
        Arg::new("no-regex")
            .long("no-regex")
            .action(ArgAction::SetTrue)
            .help("Treat package filter patterns as literal text"),
    ])
}

fn parse_class_filter(m: &ArgMatches) -> ClassFilter {
    ClassFilter {
        limit: matches_u64(m, "limit"),
        includes: matches_many(m, "include-package"),
        excludes: matches_many(m, "exclude-package"),
        regex: Some(!matches_flag_local(m, "no-regex")),
    }
}

fn matches_flag_local(m: &ArgMatches, id: &str) -> bool {
    m.get_flag(id)
}

fn parse_global_search(m: &ArgMatches) -> GlobalSearch {
    GlobalSearch {
        limit: matches_u64(m, "limit"),
        includes: matches_many(m, "include-package"),
        excludes: matches_many(m, "exclude-package"),
        case_sensitive: matches_flag_local(m, "case-sensitive"),
        regex: !matches_flag_local(m, "no-regex"),
    }
}

fn parse_class_grep(m: &ArgMatches) -> ClassGrep {
    ClassGrep {
        limit: matches_u64(m, "limit").unwrap_or(0),
        case_sensitive: matches_flag_local(m, "case-sensitive"),
        regex: !matches_flag_local(m, "no-regex"),
    }
}

fn parse_source_filter(m: &ArgMatches) -> SourceFilter {
    SourceFilter {
        limit: matches_u64(m, "limit"),
        language: m.get_one::<String>("language").cloned().filter(|s| !s.is_empty()),
    }
}

fn language_arg() -> Arg {
    Arg::new("language")
        .long("language")
        .value_parser(["java", "kotlin", "auto"])
        .help("Render source in this language (native engine; the JVM engine always serves Java)")
}

fn command() -> Command {
    Command::new("code")
        .about("Query decompiled classes, methods, source, control flow, and cross references")
        .subcommands([
            package_filter_args(Command::new("classes").about("List decompiled classes with optional package filters"))
                .arg(page_arg())
                .args(target_args()),
            package_filter_args(
                Command::new("search-global")
                    .about("Search globally across class names and decompiled class source")
                    .arg(Arg::new("keyword").required(true).value_name("KEYWORD")),
            )
            .arg(Arg::new("case-sensitive").long("case-sensitive").action(ArgAction::SetTrue).help("Match keyword with case sensitivity"))
            .arg(page_arg())
            .args(target_args()),
            Command::new("class-context")
                .about("Show one class with fields, methods, and inheritance context")
                .arg(Arg::new("class").required(true).value_name("CLASS"))
                .arg(page_arg())
                .args(target_args()),
            Command::new("class-source")
                .about("Return decompiled Java or smali source for one class")
                .arg(Arg::new("class").required(true).value_name("CLASS"))
                .arg(Arg::new("limit").long("limit").help("Maximum number of source lines to return"))
                .arg(Arg::new("smali").long("smali").action(ArgAction::SetTrue).help("Return smali output instead of Java source"))
                .arg(language_arg())
                .arg(page_arg())
                .args(target_args()),
            Command::new("method-source")
                .about("Return decompiled Java or smali source for one method")
                .arg(Arg::new("signature").required(true).value_name("SIGNATURE"))
                .arg(Arg::new("smali").long("smali").action(ArgAction::SetTrue).help("Return smali output instead of Java source"))
                .arg(Arg::new("limit").long("limit").help("Maximum number of source lines to return"))
                .arg(language_arg())
                .arg(page_arg())
                .args(target_args()),
            Command::new("method-context")
                .about("Show callers, callees, and metadata for one method")
                .arg(Arg::new("signature").required(true).value_name("SIGNATURE"))
                .arg(page_arg())
                .args(target_args()),
            Command::new("method-cfg")
                .about("Return the control-flow graph for one method")
                .arg(Arg::new("signature").required(true).value_name("SIGNATURE"))
                .arg(page_arg())
                .args(target_args()),
            Command::new("search-class")
                .about("Search source text inside one class (requires --limit)")
                .arg(Arg::new("class").required(true).value_name("CLASS"))
                .arg(Arg::new("pattern").required(true).value_name("PATTERN"))
                .arg(Arg::new("limit").long("limit").required(true).help("Maximum number of matching source lines to return"))
                .arg(Arg::new("case-sensitive").long("case-sensitive").action(ArgAction::SetTrue).help("Match pattern with case sensitivity"))
                .arg(Arg::new("no-regex").long("no-regex").action(ArgAction::SetTrue).help("Treat pattern as literal text"))
                .arg(page_arg())
                .args(target_args()),
            Command::new("search-method")
                .about("Find method signatures by simple or partial method name")
                .arg(Arg::new("name").required(true).value_name("NAME"))
                .arg(page_arg())
                .args(target_args()),
            Command::new("xref-method")
                .about("Find callers and references to one method")
                .arg(Arg::new("signature").required(true).value_name("SIGNATURE"))
                .arg(page_arg())
                .args(target_args()),
            Command::new("xref-class")
                .about("Find usages of one class")
                .arg(Arg::new("class").required(true).value_name("CLASS"))
                .arg(page_arg())
                .args(target_args()),
            Command::new("xref-field")
                .about("Find reads and writes of one field")
                .arg(Arg::new("field").required(true).value_name("FIELD"))
                .arg(page_arg())
                .args(target_args()),
            Command::new("implementations")
                .about("Find classes implementing one interface")
                .arg(Arg::new("interface").required(true).value_name("INTERFACE"))
                .arg(page_arg())
                .args(target_args()),
            Command::new("subclasses")
                .about("Find subclasses of one class")
                .arg(Arg::new("class").required(true).value_name("CLASS"))
                .arg(page_arg())
                .args(target_args()),
        ])
}

impl Tool for CodeTool {
    fn id(&self) -> &'static str {
        "code"
    }

    fn commands(&self) -> Vec<Command> {
        vec![command()]
    }

    fn run(&self, ctx: &ToolContext, matches: &ArgMatches) -> DecxResult<Value> {
        let Some((name, m)) = matches.subcommand() else {
            return Err(DecxError::usage(
                "No code subcommand given (classes | search-global | class-context | class-source | method-source | \
                 method-context | method-cfg | search-class | search-method | xref-method | xref-class | xref-field | \
                 implementations | subclasses)",
            ));
        };
        let client = super::analysis_client(ctx, m)?;
        let page = matches_u64(m, "page").unwrap_or(1);

        // Every endpoint routes through `call`: HTTP for server-engine
        // projects, the engine's registered command template otherwise.
        match name {
            "classes" => super::call(&client, "get_classes", None, |c| {
                c.get_classes(&parse_class_filter(m), page)
            }),
            "search-global" => {
                let keyword = m.get_one::<String>("keyword").cloned().unwrap_or_default();
                super::call(&client, "search_global_key", Some(&keyword), |c| {
                    c.search_global_key(&keyword, &parse_global_search(m), page)
                })
            }
            "class-context" => {
                let class = m.get_one::<String>("class").cloned().unwrap_or_default();
                super::call(&client, "get_class_context", Some(&class), |c| c.get_class_context(&class, page))
            }
            "class-source" => {
                let class = m.get_one::<String>("class").cloned().unwrap_or_default();
                super::call(&client, "get_class_source", Some(&class), |c| {
                    c.get_class_source(&class, matches_flag_local(m, "smali"), &parse_source_filter(m), page)
                })
            }
            "method-source" => {
                let sig = m.get_one::<String>("signature").cloned().unwrap_or_default();
                super::call(&client, "get_method_source", Some(&sig), |c| {
                    if matches_u64(m, "limit").is_some() || m.get_one::<String>("language").is_some() {
                        c.get_method_source_full(&sig, matches_flag_local(m, "smali"), &parse_source_filter(m), page)
                    } else {
                        c.get_method_source(&sig, matches_flag_local(m, "smali"), page)
                    }
                })
            }
            "method-context" => {
                let sig = m.get_one::<String>("signature").cloned().unwrap_or_default();
                super::call(&client, "get_method_context", Some(&sig), |c| c.get_method_context(&sig, page))
            }
            "method-cfg" => {
                let sig = m.get_one::<String>("signature").cloned().unwrap_or_default();
                super::call(&client, "get_method_cfg", Some(&sig), |c| c.get_method_cfg(&sig, page))
            }
            "search-class" => {
                let class = m.get_one::<String>("class").cloned().unwrap_or_default();
                let pattern = m.get_one::<String>("pattern").cloned().unwrap_or_default();
                super::call(&client, "search_class_key", Some(&pattern), |c| {
                    c.search_class_key(&class, &pattern, &parse_class_grep(m), page)
                })
            }
            "search-method" => {
                let method = m.get_one::<String>("name").cloned().unwrap_or_default();
                super::call(&client, "search_method", Some(&method), |c| c.search_method(&method, page))
            }
            "xref-method" => {
                let sig = m.get_one::<String>("signature").cloned().unwrap_or_default();
                super::call(&client, "get_method_xref", Some(&sig), |c| c.get_method_xref(&sig, page))
            }
            "xref-class" => {
                let class = m.get_one::<String>("class").cloned().unwrap_or_default();
                super::call(&client, "get_class_xref", Some(&class), |c| c.get_class_xref(&class, page))
            }
            "xref-field" => {
                let field = m.get_one::<String>("field").cloned().unwrap_or_default();
                super::call(&client, "get_field_xref", Some(&field), |c| c.get_field_xref(&field, page))
            }
            "implementations" => {
                let iface = m.get_one::<String>("interface").cloned().unwrap_or_default();
                super::call(&client, "get_implementations", Some(&iface), |c| {
                    c.get_implementations(&iface, page)
                })
            }
            "subclasses" => {
                let class = m.get_one::<String>("class").cloned().unwrap_or_default();
                super::call(&client, "get_subclasses", Some(&class), |c| c.get_subclasses(&class, page))
            }
            other => Err(DecxError::usage(format!("Unknown code subcommand '{other}'"))),
        }
    }
}
