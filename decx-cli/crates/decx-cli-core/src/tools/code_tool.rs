//! `code` tool — decompiled code queries over the DECX contract.

use serde_json::Value;

use crate::error::DecxResult;
use crate::iface::{ArgSpec as A, Args, CommandSpec, Interface};
use crate::params::{ClassFilter, ClassGrep, GlobalSearch, SourceFilter};
use crate::tools::{target_args, ToolContext};

pub fn interface() -> Interface {
    let mut query_args = target_args();
    query_args.push(A::opt("page", "page", "Result page number to fetch"));
    Interface::new(
        "code",
        "Query decompiled classes, methods, source, control flow, and cross references",
    )
    .common(query_args)
    .commands(vec![
        CommandSpec::leaf(
            "classes",
            "List decompiled classes with optional package filters",
            filter_args(vec![]),
            |ctx, a| {
                let page = page(a);
                query(ctx, a, |c| {
                    c.get_classes(&class_filter(a), page)
                })
            },
        ),
        CommandSpec::leaf(
            "search-global",
            "Search globally across class names and decompiled class source",
            filter_args(vec![
                A::positional("keyword", "Search keyword (e.g. a class or method name fragment)"),
                A::flag("case-sensitive", "case-sensitive", "Match keyword with case sensitivity"),
            ]),
            |ctx, a| {
                let page = page(a);
                let key = a.str("keyword").to_string();
                query(ctx, a, |c| {
                    c.search_global_key(
                        &key,
                        &GlobalSearch {
                            limit: a.u64("limit"),
                            includes: a.strs("include-package").to_vec(),
                            excludes: a.strs("exclude-package").to_vec(),
                            case_sensitive: a.flag("case-sensitive"),
                            regex: !a.flag("no-regex"),
                        },
                        page,
                    )
                })
            },
        ),
        CommandSpec::leaf(
            "class-context",
            "Show one class with fields, methods, and inheritance context",
            vec![A::positional("class", "Fully qualified class name")],
            |ctx, a| single(ctx, a, "class", |c, key: &str| {
                c.get_class_context(key, page(a))
            }),
        ),
        CommandSpec::leaf(
            "class-source",
            "Return decompiled source for one class",
            vec![
                A::positional("class", "Fully qualified class name"),
                A::opt("limit", "limit", "Maximum number of source lines to return"),
                A::flag("smali", "smali", "Return smali output instead of source"),
                A::one_of("language", "language", &["java", "kotlin", "auto"], "Render language (native engine)"),
            ],
            |ctx, a| single_src(ctx, a, "class", |c, key, f| {
                c.get_class_source(key, a.flag("smali"), &f, page(a))
            }),
        ),
        CommandSpec::leaf(
            "method-source",
            "Return decompiled source for one method",
            vec![
                A::positional("signature", "Method signature (Lpkg/Cls;->name(args)ret)"),
                A::flag("smali", "smali", "Return smali output instead of source"),
                A::opt("limit", "limit", "Maximum number of source lines to return"),
                A::one_of("language", "language", &["java", "kotlin", "auto"], "Render language (native engine)"),
            ],
            |ctx, a| single_src(ctx, a, "signature", |c, key, f| {
                c.get_method_source_full(key, a.flag("smali"), &f, page(a))
            }),
        ),
        CommandSpec::leaf(
            "method-context",
            "Show callers, callees, and metadata for one method",
            vec![A::positional("signature", "Method signature")],
            |ctx, a| single(ctx, a, "signature", |c, key: &str| {
                c.get_method_context(key, page(a))
            }),
        ),
        CommandSpec::leaf(
            "method-cfg",
            "Return the control-flow graph for one method",
            vec![A::positional("signature", "Method signature")],
            |ctx, a| single(ctx, a, "signature", |c, key: &str| {
                c.get_method_cfg(key, page(a))
            }),
        ),
        CommandSpec::leaf(
            "search-class",
            "Search source text inside one class",
            vec![
                A::positional("class", "Fully qualified class name"),
                A::positional("pattern", "Text or regex to find"),
                A::required_opt("limit", "limit", "Maximum number of matching source lines"),
                A::flag("case-sensitive", "case-sensitive", "Match pattern with case sensitivity"),
                A::flag("no-regex", "no-regex", "Treat pattern as literal text"),
            ],
            |ctx, a| {
                let class = a.str("class").to_string();
                let pattern = a.str("pattern").to_string();
                query(ctx, a, |c| {
                    c.search_class_key(
                        &class,
                        &pattern,
                        &ClassGrep {
                            limit: a.u64("limit").unwrap_or(0),
                            case_sensitive: a.flag("case-sensitive"),
                            regex: !a.flag("no-regex"),
                        },
                        page(a),
                    )
                })
            },
        ),
        CommandSpec::leaf(
            "search-method",
            "Find method signatures by simple or partial method name",
            vec![A::positional("name", "Method name fragment")],
            |ctx, a| single(ctx, a, "name", |c, key| {
                c.search_method(key, page(a))
            }),
        ),
        CommandSpec::leaf(
            "xref-method",
            "Find callers and references to one method",
            vec![A::positional("signature", "Method signature")],
            |ctx, a| single(ctx, a, "signature", |c, key: &str| {
                c.get_method_xref(key, page(a))
            }),
        ),
        CommandSpec::leaf(
            "xref-class",
            "Find usages of one class",
            vec![A::positional("class", "Fully qualified class name")],
            |ctx, a| single(ctx, a, "class", |c, key: &str| {
                c.get_class_xref(key, page(a))
            }),
        ),
        CommandSpec::leaf(
            "xref-field",
            "Find reads and writes of one field",
            vec![A::positional("field", "Field signature or name")],
            |ctx, a| single(ctx, a, "field", |c, key: &str| {
                c.get_field_xref(key, page(a))
            }),
        ),
        CommandSpec::leaf(
            "implementations",
            "Find classes implementing one interface",
            vec![A::positional("interface", "Fully qualified interface name")],
            |ctx, a| single(ctx, a, "interface", |c, key: &str| {
                c.get_implementations(key, page(a))
            }),
        ),
        CommandSpec::leaf(
            "subclasses",
            "Find subclasses of one class",
            vec![A::positional("class", "Fully qualified class name")],
            |ctx, a| single(ctx, a, "class", |c, key: &str| {
                c.get_subclasses(key, page(a))
            }),
        ),
    ])
}

fn filter_args(extra: Vec<A>) -> Vec<A> {
    let mut args = vec![
        A::opt("limit", "limit", "Maximum number of returned items"),
        A::multi("include-package", "include-package", "Include only names matching this package pattern; repeatable"),
        A::multi("exclude-package", "exclude-package", "Exclude names matching this package pattern; repeatable"),
        A::flag("no-regex", "no-regex", "Treat patterns as literal text instead of regular expressions"),
    ];
    args.extend(extra);
    args
}

fn page(a: &Args) -> u64 {
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

fn source_filter(a: &Args) -> SourceFilter {
    SourceFilter {
        limit: a.u64("limit"),
        language: a.opt_str("language").map(str::to_string),
    }
}

// ── handler helpers ─────────────────────────────────────────────────────────

fn query(
    ctx: &ToolContext,
    a: &Args,
    http: impl FnOnce(&crate::client::DecxClient) -> DecxResult<Value>,
) -> DecxResult<Value> {
    http(&super::analysis_client(ctx, a)?)
}

/// One-key endpoint: resolve the client, pass the declared key argument.
fn single(
    ctx: &ToolContext,
    a: &Args,
    key_id: &str,
    http: impl FnOnce(&crate::client::DecxClient, &str) -> DecxResult<Value>,
) -> DecxResult<Value> {
    let key = a.str(key_id).to_string();
    http(&super::analysis_client(ctx, a)?, &key)
}

/// One-key source endpoint carrying a source filter.
fn single_src(
    ctx: &ToolContext,
    a: &Args,
    key_id: &str,
    http: impl FnOnce(&crate::client::DecxClient, &str, SourceFilter) -> DecxResult<Value>,
) -> DecxResult<Value> {
    let key = a.str(key_id).to_string();
    let filter = source_filter(a);
    http(&super::analysis_client(ctx, a)?, &key, filter)
}
