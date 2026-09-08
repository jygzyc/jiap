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
        let (port, _session) = super::resolve_target(ctx, m)?;
        let client = crate::client::DecxClient::new(port);
        let page = matches_u64(m, "page").unwrap_or(1);

        match name {
            "classes" => client.get_classes(&parse_class_filter(m), page),
            "search-global" => client.search_global_key(
                m.get_one::<String>("keyword").map(String::as_str).unwrap_or_default(),
                &parse_global_search(m),
                page,
            ),
            "class-context" => {
                client.get_class_context(m.get_one::<String>("class").map(String::as_str).unwrap_or_default(), page)
            }
            "class-source" => client.get_class_source(
                m.get_one::<String>("class").map(String::as_str).unwrap_or_default(),
                matches_flag_local(m, "smali"),
                &parse_source_filter(m),
                page,
            ),
            "method-source" => {
                if matches_u64(m, "limit").is_some() || m.get_one::<String>("language").is_some() {
                    client.get_method_source_full(
                        m.get_one::<String>("signature").map(String::as_str).unwrap_or_default(),
                        matches_flag_local(m, "smali"),
                        &parse_source_filter(m),
                        page,
                    )
                } else {
                    client.get_method_source(
                        m.get_one::<String>("signature").map(String::as_str).unwrap_or_default(),
                        matches_flag_local(m, "smali"),
                        page,
                    )
                }
            }
            "method-context" => client.get_method_context(
                m.get_one::<String>("signature").map(String::as_str).unwrap_or_default(),
                page,
            ),
            "method-cfg" => {
                client.get_method_cfg(m.get_one::<String>("signature").map(String::as_str).unwrap_or_default(), page)
            }
            "search-class" => client.search_class_key(
                m.get_one::<String>("class").map(String::as_str).unwrap_or_default(),
                m.get_one::<String>("pattern").map(String::as_str).unwrap_or_default(),
                &parse_class_grep(m),
                page,
            ),
            "search-method" => {
                client.search_method(m.get_one::<String>("name").map(String::as_str).unwrap_or_default(), page)
            }
            "xref-method" => client.get_method_xref(
                m.get_one::<String>("signature").map(String::as_str).unwrap_or_default(),
                page,
            ),
            "xref-class" => {
                client.get_class_xref(m.get_one::<String>("class").map(String::as_str).unwrap_or_default(), page)
            }
            "xref-field" => {
                client.get_field_xref(m.get_one::<String>("field").map(String::as_str).unwrap_or_default(), page)
            }
            "implementations" => client.get_implementations(
                m.get_one::<String>("interface").map(String::as_str).unwrap_or_default(),
                page,
            ),
            "subclasses" => {
                client.get_subclasses(m.get_one::<String>("class").map(String::as_str).unwrap_or_default(), page)
            }
            other => Err(DecxError::usage(format!("Unknown code subcommand '{other}'"))),
        }
    }
}
