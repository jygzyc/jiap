//! Simplify pass tests grouped by topic.

use super::cleanup::*;
use super::conditions::*;
use super::expr::*;
use super::format::*;
use super::loops::*;
use super::try_catch::*;
use super::util::*;
use super::{
    java_string_hash_code, merge_duplicate_finally, normalize_java_indent, restore_string_switch,
    simplify_method_body, simplify_synchronized_blocks,
};

mod parsing {
    use super::*;
}

mod pipeline {
    use super::*;

    use super::*;

    #[test]
    fn strip_comment() {
        let line = "        return v0;  // // 1234: return-object v0";
        assert_eq!(strip_trailing_comment(line).trim(), "return v0;");
    }

    #[test]
    fn parse_invoke() {
        let line = "        invoke-static( v2, v3, Class.method(A, B) );  // comment";
        let (args, method_name) = parse_invoke_args_and_method(line).unwrap();
        assert_eq!(args, "v2, v3");
        assert_eq!(method_name, "Class.method");
    }

    #[test]
    fn parse_invoke_no_args() {
        let line = "        invoke-static( Runtime.getRuntime() );  // x";
        let (args, method_name) = parse_invoke_args_and_method(line).unwrap();
        assert_eq!(args, "");
        assert_eq!(method_name, "Runtime.getRuntime");
    }

    #[test]
    fn parse_move_result() {
        assert_eq!(
            parse_move_result_line("        v0 = <result>;  // x"),
            Some("v0".to_string())
        );
        assert_eq!(
            parse_move_result_line("        v5 = <result>;"),
            Some("v5".to_string())
        );
        assert_eq!(
            parse_move_result_line("        e = <result>;"),
            Some("e".to_string())
        );
        assert_eq!(parse_move_result_line("        v0 = v1;"), None);
    }

    #[test]
    fn parse_return_reg() {
        assert_eq!(
            parse_return_reg_line("        return v0;  // x"),
            Some("v0".to_string())
        );
        assert_eq!(
            parse_return_reg_line("        return v3;"),
            Some("v3".to_string())
        );
        assert_eq!(
            parse_return_reg_line("        return e;"),
            Some("e".to_string())
        );
        assert_eq!(parse_return_reg_line("        return;"), None);
    }

    #[test]
    fn simplify_invoke_move_result_return() {
        let body = "        invoke-static( v2, v3, Foo.bar(A, B) );  // comment\n        v0 = <result>;  // move-result\n        return v0;  // return";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("return Foo.bar(v2, v3);"),
            "expected 'return Foo.bar(v2, v3);' in {:?}",
            simplified
        );
        assert!(!simplified.contains("<result>"));
    }

    #[test]
    fn simplify_invoke_move_result_only() {
        // Resolved invoke has "Receiver, MethodRef" - method ref is last (e.g. Class.method(Params)).
        let body =
            "        invoke-virtual( v0, Foo.bar(A, B) );  // x\n        v2 = <result>;  // y";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("v2 = Foo.bar(v0);"),
            "expected 'v2 = Foo.bar(v0);' in {:?}",
            simplified
        );
        assert!(!simplified.contains("<result>"));
    }

    #[test]
    fn simplify_invoke_no_arg_static_named_result() {
        let body = "        invoke-static(Runtime.getRuntime());\n        e = <result>;\n        e = e.exec(\"logcat -d\");";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("e = Runtime.getRuntime();"),
            "expected folded getRuntime in {:?}",
            simplified
        );
        assert!(!simplified.contains("invoke-static"));
        assert!(!simplified.contains("<result>"));
        assert!(simplified.contains("e = e.exec(\"logcat -d\");"));
    }

    #[test]
    fn simplify_invoke_return_void() {
        // invoke-static + return; → normal Java call then return;
        let body = "        invoke-static( v1, ViewCompatJB.postInvalidateOnAnimation(android.view.View) );  // x\n        return;  // y";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("ViewCompatJB.postInvalidateOnAnimation(v1);"),
            "expected Java-style call in {:?}",
            simplified
        );
        assert!(simplified.contains("return;"));
        assert!(!simplified.contains("invoke-static"));
    }

    #[test]
    fn simplify_if_return_else_return_to_ternary() {
        let body = "        if (n0 > 0) {\n            return n0;\n        } else {\n            return 0;\n        }";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("return n0 > 0 ? n0 : 0;"),
            "expected ternary in {:?}",
            simplified
        );
        assert!(!simplified.contains("} else {"));
    }

    #[test]
    fn simplify_stringbuilder_append_to_concat() {
        let body = "        sb = new StringBuilder();\n        sb.append(a);\n        sb.append(b);\n        s = sb.toString();";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("s = a + b;"),
            "expected 's = a + b;' in {:?}",
            simplified
        );
        assert!(!simplified.contains("StringBuilder"));
    }

    #[test]
    fn simplify_stringbuilder_return_to_string() {
        let body = "        sb = new StringBuilder(x);\n        sb.append(y);\n        return sb.toString();";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("return x + y;"),
            "expected 'return x + y;' in {:?}",
            simplified
        );
    }

    #[test]
    fn simplify_stringbuilder_ssa_aliased_with_init_and_println() {
        let body = "\
                        local2 = System.out;\n\
                        StringBuilder sb0 = new StringBuilder();\n\
                        str0 = \"test2 \";\n\
                        v3.<init>(str0);\n\
                        local3 = v3.append(n0);\n\
                        local4 = v3.toString();\n\
                        v2.println(local4);";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("System.out.println(\"test2 \" + n0);"),
            "expected 'System.out.println(\"test2 \" + n0);' in {:?}",
            simplified
        );
        assert!(
            !simplified.contains("StringBuilder"),
            "StringBuilder should be gone: {:?}",
            simplified
        );
    }

    #[test]
    fn simplify_stringbuilder_dest_assign_append() {
        let body = "\
                sb = new StringBuilder();\n\
                local0 = sb.append(a);\n\
                local1 = sb.append(b);\n\
                s = sb.toString();";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("s = a + b;"),
            "expected 's = a + b;' in {:?}",
            simplified
        );
        assert!(!simplified.contains("StringBuilder"));
    }

    #[test]
    fn simplify_remove_move_exception() {
        let body = "\
                local0; /* move-exception */\n\
                n0 = 12;\n\
                System.out.println(n0);";
        let simplified = simplify_method_body(body, false);
        assert!(
            !simplified.contains("move-exception"),
            "move-exception should be removed: {:?}",
            simplified
        );
        assert!(
            simplified.contains("System.out.println(12);"),
            "single-use numeric should be inlined: {:?}",
            simplified
        );
        assert!(
            !simplified.contains("n0 = 12"),
            "n0 assignment should be removed: {:?}",
            simplified
        );
    }

    #[test]
    fn simplify_inline_class_literal_and_index_zero() {
        // Mirrors reflection Class[] / Object[] setup from OVAA invokePlugins.
        // Use i0 (not bare `j`) so multi-use cheap literals still inline without
        // colliding with merge's live `i`/`j`/`k` loop-index rule.
        let body = "\
                Class[] arr0 = new Class[1];\n\
                local1 = android.content.Context.class;\n\
                i0 = 0;\n\
                arr0[i0] = local1;\n\
                Object[] arr1 = new Object[1];\n\
                arr1[i0] = this;\n\
                m.invoke(null, arr1);";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("arr0[0] = android.content.Context.class")
                || simplified.contains("arr0[0]=android.content.Context.class"),
            "expected Context.class inlined at arr0[0]: {:?}",
            simplified
        );
        assert!(
            !simplified.contains("local1"),
            "local1 should be gone: {:?}",
            simplified
        );
        assert!(
            !simplified.contains("i0"),
            "index temp should be inlined: {:?}",
            simplified
        );
    }

    #[test]
    fn simplify_inline_single_use_string_constant() {
        let body = "\
                str0 = \"test\";\n\
                System.out.println(str0);";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("System.out.println(\"test\");"),
            "expected inlined string constant: {:?}",
            simplified
        );
        assert!(
            !simplified.contains("str0"),
            "str0 assignment should be removed: {:?}",
            simplified
        );
    }

    /// OVAA LoginActivity.onCreate pattern: reuse `i0` for layout then button id across an if.
    #[test]
    fn simplify_ovaa_login_i0_reuse_across_if() {
        let body = "\
        super.onCreate(bundle);\n\
        int i0 = 2131361820;\n\
        this.setContentView(i0);\n\
        oversecured.ovaa.utils.LoginUtils v0 = oversecured.ovaa.utils.LoginUtils.getInstance(this);\n\
        this.loginUtils = v0;\n\
        boolean z0 = v0.isLoggedIn();\n\
        if (!z0) {\n\
            i0 = 2131165294;\n\
            View view0 = this.findViewById(i0);\n\
            oversecured.ovaa.activities.LoginActivity$1 v1 = new oversecured.ovaa.activities.LoginActivity$1(this);\n\
            view0.setOnClickListener(v1);\n\
            return;\n\
        } else {\n\
            this.onLoginFinished();\n\
            return;\n\
        }";
        let once = cleanup_decompiler_artifacts_once(body);
        assert!(
            once.contains("2131165294"),
            "first cleanup pass must keep button id:\n{}",
            once
        );
        let cleaned = cleanup_decompiler_artifacts(body);
        assert!(
            cleaned.contains("2131165294"),
            "cleanup loop must keep button id:\n{}",
            cleaned
        );
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("2131165294"),
            "button resource id must survive simplify:\ncleaned={:?}\nsimplified={:?}",
            cleaned,
            simplified
        );
        assert!(
            !simplified.contains("findViewById(i0)"),
            "dangling i0 after removing button assign: {:?}",
            simplified
        );
        assert!(
            !simplified.contains("findViewById(2131361820)"),
            "layout id must not replace button id: {:?}",
            simplified
        );
    }

    #[test]
    fn simplify_inline_findview_cast_gettext_tostring_chain() {
        let body = "\
                view0 = this.findViewById(2131165271);\n\
                view0 = (TextView) view0;\n\
                view0 = view0.getText();\n\
                email = view0.toString();\n\
                i1 = android.text.TextUtils.isEmpty(email);";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("this.findViewById(2131165271)")
                && simplified.contains("(TextView)")
                && simplified.contains("getText()")
                && simplified.contains("toString()")
                && simplified.contains("email ="),
            "expected chained findViewById into email, got:\n{}",
            simplified
        );
        assert!(
            simplified.contains("isEmpty(email)"),
            "email should remain for isEmpty, got:\n{}",
            simplified
        );
        assert!(
            !simplified.contains("view0 ="),
            "intermediate view0 assigns should be gone: {}",
            simplified
        );
    }

    #[test]
    fn simplify_inline_single_use_numeric_in_call() {
        let body = "\
                Intent pickerIntent = new Intent();\n\
                s0 = \"image/*\";\n\
                pickerIntent.setType(s0);\n\
                s0 = this.this$0;\n\
                int i0 = 1001;\n\
                s0.startActivityForResult(pickerIntent, i0);";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("pickerIntent.setType(\"image/*\");"),
            "expected inlined MIME type: {:?}",
            simplified
        );
        assert!(
            simplified.contains("this.this$0.startActivityForResult(pickerIntent, 1001);"),
            "expected inlined receiver + request code: {:?}",
            simplified
        );
        assert!(
            !simplified.contains("int i0"),
            "i0 should be removed: {:?}",
            simplified
        );
        assert!(
            !simplified.contains("s0 ="),
            "s0 temps should be removed: {:?}",
            simplified
        );
    }

    #[test]
    fn simplify_inline_multi_use_cheap_literal() {
        let body = "\
                int i0 = 1001;\n\
                foo(i0);\n\
                bar(i0);";
        let simplified = simplify_method_body(body, false);
        assert!(
            !simplified.contains("int i0") && !simplified.contains("i0 ="),
            "cheap multi-use temp should be inlined: {:?}",
            simplified
        );
        assert!(
            simplified.contains("foo(1001);") && simplified.contains("bar(1001);"),
            "{:?}",
            simplified
        );
    }

    #[test]
    fn simplify_inline_webview_boolean_settings() {
        let body = "\
    private void setupWebView(android.webkit.WebView p0) {\n\
        webView.setWebChromeClient(new android.webkit.WebChromeClient());\n\
        webView.setWebViewClient(new android.webkit.WebViewClient());\n\
        int i0 = 1;\n\
        webView.getSettings().setJavaScriptEnabled(i0);\n\
        webView.getSettings().setAllowFileAccessFromFileURLs(i0);\n\
        return;\n\
    }";
        let simplified = simplify_method_body(body, false);
        assert!(
            !simplified.contains("int i0") && !simplified.contains("i0 ="),
            "i0 should be removed: {:?}",
            simplified
        );
        assert!(
            simplified.contains("setJavaScriptEnabled(true)"),
            "expected boolean true: {:?}",
            simplified
        );
        assert!(
            simplified.contains("setAllowFileAccessFromFileURLs(true)"),
            "expected boolean true: {:?}",
            simplified
        );
    }

    #[test]
    fn simplify_keep_multi_use_non_temp_numeric() {
        // Meaningful names stay as locals even when the RHS is a cheap literal.
        let body = "\
                int requestCode = 1001;\n\
                foo(requestCode);\n\
                bar(requestCode);";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("requestCode = 1001")
                || simplified.contains("int requestCode = 1001"),
            "named local should keep assignment: {:?}",
            simplified
        );
        assert!(
            simplified.contains("foo(requestCode);") && simplified.contains("bar(requestCode);"),
            "{:?}",
            simplified
        );
    }

    #[test]
    fn simplify_dead_sysout_assignment_removed() {
        let body = "\
                local0 = System.out;\n\
                str0 = \"hello\";\n\
                v0.println(str0);";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("System.out.println(\"hello\");"),
            "expected System.out.println(\"hello\") in {:?}",
            simplified
        );
        assert!(
            !simplified.contains("local0 = System.out"),
            "dead assignment should be removed: {:?}",
            simplified
        );
    }

    #[test]
    fn simplify_inline_return_string_constant() {
        let body = "                        String result = \"bad_name\";\n                        return result;";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("return \"bad_name\";"),
            "expected return \"bad_name\"; in {:?}",
            simplified
        );
        assert!(
            !simplified.contains("String result"),
            "assignment should be removed: {:?}",
            simplified
        );
        assert!(
            !simplified.contains("return result"),
            "return result should be folded: {:?}",
            simplified
        );
    }

    #[test]
    fn simplify_assign_return_call() {
        let body = "                result = Foo.bar(x);\n                return result;";
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains("return Foo.bar(x);"),
            "expected return Foo.bar(x); in {:?}",
            simplified
        );
    }

    #[test]
    fn normalize_over_indented_constructor_body() {
        let body = "                super();\n                return;\n";
        let out = normalize_java_indent(body);
        assert!(
            out.contains("        super();"),
            "expected 8-space indent, got {:?}",
            out
        );
        assert!(
            !out.contains("                super"),
            "16-space indent should be gone: {:?}",
            out
        );
    }

    #[test]
    fn simplify_normalizes_indent() {
        let body = "                super();\n                return;\n";
        let simplified = simplify_method_body(body, true);
        let super_line = simplified.lines().find(|l| l.contains("super();")).unwrap();
        let spaces = super_line.len() - super_line.trim_start().len();
        assert_eq!(
            spaces, 8,
            "body statements should be 8 spaces, got {:?}: {:?}",
            spaces, simplified
        );
    }
}

mod string_switch {
    use super::*;

    use super::*;

    #[test]
    fn restores_string_case_label() {
        let body = r#"        switch (s.hashCode()) {
        case 97:
            if (!s.equals("a")) break;
            return 1;
        }
"#;
        let out = restore_string_switch(body);
        assert!(out.contains("switch (s)"));
        assert!(out.contains("case \"a\":"));
    }

    /// jadx `switches/TestSwitchOverStrings` — multi-case + no leftover hash discriminant.
    #[test]
    fn jadx_restores_multiple_string_cases() {
        let body = r#"        switch (str.hashCode()) {
        case -603257287:
            if (!str.equals("frewhyh")) break;
            return 1;
        case 3556498:
            if (!str.equals("test")) break;
            return 3;
        default:
            return 0;
        }
"#;
        let out = restore_string_switch(body);
        assert!(out.contains("switch (str)"));
        assert!(out.contains("case \"frewhyh\":"));
        assert!(out.contains("case \"test\":"));
        assert!(!out.contains("case -603257287:") || out.contains("// was"));
    }

    #[test]
    fn restores_literal_first_equals() {
        let body = r#"        switch (s.hashCode()) {
        case 97:
            if (!"a".equals(s)) break;
            return 1;
        }
"#;
        let out = restore_string_switch(body);
        assert!(out.contains("switch (s)"), "{out}");
        assert!(out.contains("case \"a\":"), "{out}");
    }

    #[test]
    fn folds_hashcode_temp_disc() {
        let body = r#"        int h = s.hashCode();
        switch (h) {
        case 97:
            if (!s.equals("a")) break;
            return 1;
        }
"#;
        let out = restore_string_switch(body);
        assert!(out.contains("switch (s)"), "{out}");
        assert!(out.contains("case \"a\":"), "{out}");
        assert!(!out.contains("switch (h)"), "{out}");
    }

    #[test]
    fn rejects_wrong_hash_keeps_numeric() {
        let body = r#"        switch (s.hashCode()) {
        case 99:
            if (!s.equals("a")) break;
            return 1;
        }
"#;
        let out = restore_string_switch(body);
        // "a".hashCode() == 97, not 99 — keep numeric case
        assert!(out.contains("case 99:"), "{out}");
        assert!(!out.contains("case \"a\":"), "{out}");
    }

    #[test]
    fn java_string_hash_matches_known() {
        assert_eq!(java_string_hash_code("a"), 97);
        assert_eq!(java_string_hash_code("test"), 3556498);
    }

    #[test]
    fn merge_duplicate_finally_peels_close() {
        let body = r#"        try {
            work();
            r.close();
        } catch (IOException e) {
            log(e);
            r.close();
        }
"#;
        let out = merge_duplicate_finally(body);
        assert!(out.contains("} finally {"), "{out}");
        assert!(out.contains("r.close();"), "{out}");
        assert_eq!(out.matches("r.close();").count(), 1, "{out}");
        assert!(out.contains("log(e);"), "{out}");
        assert!(out.contains("work();"), "{out}");
    }

    #[test]
    fn inline_array_string_temps() {
        let body = r#"        String s0 = "android.permission.READ_EXTERNAL_STORAGE";
        String s1 = "android.permission.WRITE_EXTERNAL_STORAGE";
        String[] permissions = new String[]{ s0, s1 };
        int length = permissions.length;
        int i0 = 0;
        while (i0 < length) {
            String permission = permissions[i0];
            i0 = i0 + 1;
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("READ_EXTERNAL_STORAGE") && out.contains("WRITE_EXTERNAL_STORAGE"),
            "should inline strings: {}",
            out
        );
        assert!(
            !out.contains("s0") && !out.contains("s1 ="),
            "string temps should be gone: {}",
            out
        );
    }

    /// OVAA MainActivity.processDeeplink-style nested inverted equals → else-if chain.
    #[test]
    fn simplify_ovaa_process_deeplink_else_if_chain() {
        let body = r#"        String path = uri.getScheme();
        if ("oversecured".equals(path)) {
            path = uri.getHost();
            url = "ovaa";
            if ("ovaa".equals(path)) {
                path = uri.getPath();
                url = "/logout";
                if (!("/logout".equals(path))) {
                    url = "/login";
                    if (!("/login".equals(path))) {
                        url = "/grant_uri_permissions";
                        if (!("/grant_uri_permissions".equals(path))) {
                            url = "/webview";
                            if ("/webview".equals(path)) {
                                this.startActivity(i);
                            }
                        } else {
                            this.startActivityForResult(url, i1);
                        }
                    } else {
                        this.loginUtils.setLoginUrl(url);
                        this.startActivity(intent0);
                    }
                } else {
                    this.loginUtils.logout();
                    this.startActivity(url);
                }
            }
        }
        return;
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("\"oversecured\".equals(uri.getScheme())")
                && out.contains("\"ovaa\".equals(uri.getHost())"),
            "scheme/host should merge into && :\n{}",
            out
        );
        assert!(
            out.contains("else if") || out.matches("else if").count() >= 1,
            "expected else-if chain:\n{}",
            out
        );
        assert!(
            out.contains("\"/logout\".equals(path)")
                && !out.contains("!(\"/logout\".equals(path))"),
            "logout check should be positive:\n{}",
            out
        );
        assert!(
            !out.contains("url = \"/logout\"")
                && !out.contains("url = \"ovaa\"")
                && !out.contains("url = \"/login\""),
            "folded string lits should be dropped:\n{}",
            out
        );
        assert!(
            out.contains("loginUtils.logout()")
                && out.contains("setLoginUrl")
                && out.contains("startActivityForResult"),
            "branch bodies must survive:\n{}",
            out
        );
    }

    #[test]
    fn simplify_demo_foreach_arrays_as_list_shape() {
        let body = r#"        int i = 1;
        int j = 2;
        int[] arr0 = new int[]{1, 2, 3};
        int i0 = ControlFlowFixtures.forEachArray(arr0);
        Integer[] arr1 = new Integer[j];
        int i2 = 4;
        String s0 = Integer.valueOf(4);
        int k = 0;
        arr1[k] = s0;
        i2 = 5;
        s0 = Integer.valueOf(5);
        arr1[i] = s0;
        List list0 = Arrays.asList(arr1);
        i = ControlFlowFixtures.forEachList(list0);
        return i0 + i;
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("forEachArray(new int[]{1, 2, 3})")
                || out.contains("forEachArray(new int[]{1,2,3})"),
            "int[] literal should inline into forEachArray:\n{out}"
        );
        assert!(
            out.contains("Arrays.asList(4, 5)") || out.contains("Arrays.asList(4,5)"),
            "Arrays.asList should take boxed literals directly:\n{out}"
        );
        assert!(
            !out.contains("new Integer[") && !out.contains("arr1"),
            "Integer[] materialization should be gone:\n{out}"
        );
    }

    #[test]
    fn flip_and_flatten_simple_negated_equals() {
        let body = r#"        if (!("a".equals(path))) {
            if (!("b".equals(path))) {
                foo();
            } else {
                bar();
            }
        } else {
            baz();
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("if (\"a\".equals(path))") && out.contains("else if (\"b\".equals(path))"),
            "expected positive else-if:\n{}",
            out
        );
        assert!(
            out.contains("baz();") && out.contains("bar();") && out.contains("foo();"),
            "{}",
            out
        );
    }

    #[test]
    fn polish_instanceof_eq_null_and_zero() {
        assert_eq!(
            polish_instanceof_in_condition("o instanceof Number == null"),
            "!(o instanceof Number)"
        );
        assert_eq!(
            polish_instanceof_in_condition("o instanceof Number == 0"),
            "!(o instanceof Number)"
        );
        assert_eq!(
            polish_instanceof_in_condition("o instanceof Number != 0"),
            "o instanceof Number"
        );
        assert_eq!(
            polish_instanceof_in_condition("!o instanceof Number"),
            "!(o instanceof Number)"
        );
    }

    #[test]
    fn simplify_casts_and_instanceof_from_d8_shape() {
        let body = r#"        boolean z0 = o instanceof Number;
        if (obj2 == null) {
            z0 = o instanceof CharSequence;
            if (obj2 == null) {
                return -1;
            } else {
                obj2 = o;
                obj2 = (CharSequence) o;
                return obj2.length();
            }
        } else {
            obj2 = o;
            obj2 = (Number) o;
            return obj2.intValue();
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("if (o instanceof Number)")
                && out.contains("if (o instanceof CharSequence)")
                && out.contains("return ((Number) o).intValue();")
                && out.contains("return ((CharSequence) o).length();")
                && out.contains("return -1;"),
            "expected sequential instanceof tests:\n{out}"
        );
        assert!(
            !out.contains("== null") && !out.contains("== 0") && !out.contains("obj2"),
            "should not keep inverted null tests or check-cast temps:\n{out}"
        );
    }

    #[test]
    fn jadx_conditions_short_circuit_and() {
        let body = r#"        if (a) {
            if (b) {
                foo();
            }
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("if (a && b)"),
            "expected short-circuit && merge:\n{}",
            out
        );
        assert!(out.contains("foo();"), "{}", out);
        assert!(
            !out.contains("if (a) {") || out.contains("if (a && b)"),
            "outer-only if should be gone:\n{}",
            out
        );
    }

    #[test]
    fn jadx_conditions_short_circuit_or() {
        let body = r#"        if (a) {
            foo();
        } else {
            if (b) {
                foo();
            }
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("if (a || b)"),
            "expected short-circuit || merge:\n{}",
            out
        );
        assert!(out.contains("foo();"), "{}", out);
        assert!(
            !out.contains("else if"),
            "else-if should collapse into ||:\n{}",
            out
        );
    }

    #[test]
    fn jadx_conditions_else_if_emit() {
        // Text-level flatten (emit-time else-if is covered by region emit; this asserts simplify path).
        let body = r#"        if (a) {
            foo();
        } else {
            if (b) {
                bar();
            } else {
                baz();
            }
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(out.contains("else if (b)"), "expected else if:\n{}", out);
        assert!(
            !out.contains("} else {\n            if (b)"),
            "should not nest else/if:\n{}",
            out
        );
        assert!(
            out.contains("foo();") && out.contains("bar();") && out.contains("baz();"),
            "{}",
            out
        );
    }

    #[test]
    fn jadx_ternary_assign() {
        let body = r#"        if (c) {
            x = a;
        } else {
            x = b;
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("x = c ? a : b;"),
            "expected assign ternary:\n{}",
            out
        );
        assert!(!out.contains("} else {"), "else should be gone:\n{}", out);
    }

    #[test]
    fn jadx_loops_foreach_iterator() {
        let body = r#"        it = list.iterator();
        while (it.hasNext()) {
            String s = (String) it.next();
            System.out.println(s);
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("for (String s : list)"),
            "expected for-each:\n{}",
            out
        );
        assert!(out.contains("System.out.println(s);"), "{}", out);
        assert!(
            !out.contains("hasNext()") && !out.contains("iterator()"),
            "iterator while should be gone:\n{}",
            out
        );
    }

    #[test]
    #[allow(non_snake_case)] // jadx-style name
    fn jadx_strip_requireNonNull() {
        let body = r#"        x = Objects.requireNonNull(y);
        Objects.requireNonNull(z);
        Intrinsics.checkNotNullParameter(arg, "arg");
        kotlin.jvm.internal.Intrinsics.checkNotNull(obj);
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("x = y;"),
            "requireNonNull assign should unwrap:\n{}",
            out
        );
        assert!(
            !out.contains("requireNonNull") && !out.contains("checkNotNull"),
            "null-check calls should be stripped:\n{}",
            out
        );
    }

    #[test]
    fn jadx_trycatch_multi_catch() {
        let body = r#"        try {
            foo();
        } catch (IOException e) {
            log(e);
        } catch (RuntimeException e) {
            log(e);
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("} catch (IOException | RuntimeException e)"),
            "expected multi-catch:\n{}",
            out
        );
        assert_eq!(
            out.matches("log(e);").count(),
            1,
            "body should appear once:\n{}",
            out
        );
    }

    #[test]
    fn simplify_fill_array_new_type() {
        let body = r#"        v0 = { 1, 2, 3 };
        v1 = { 1L, 2L };
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("v0 = new int[]{ 1, 2, 3 };"),
            "expected typed int array:\n{}",
            out
        );
        assert!(
            out.contains("v1 = new long[]{ 1L, 2L };"),
            "expected typed long array:\n{}",
            out
        );
    }

    #[test]
    fn jadx_loops_do_while_text() {
        let body = r#"        while (true) {
            work();
            if (!done) {
                break;
            }
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("do {") && out.contains("} while (done);"),
            "expected do-while:\n{}",
            out
        );
        assert!(out.contains("work();"), "{}", out);
        assert!(
            !out.contains("while (true)"),
            "while(true) should be gone:\n{}",
            out
        );
    }

    #[test]
    fn jadx_loops_do_while_break_positive() {
        let body = r#"        while (true) {
            work();
            if (done) {
                break;
            }
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("do {") && out.contains("} while (!(done));"),
            "expected do-while with negated cond:\n{}",
            out
        );
        assert!(!out.contains("while (true)"), "{}", out);
    }

    #[test]
    fn jadx_loops_do_while_break_oneliner() {
        let body = r#"        while (true) {
            work();
            if (done) break;
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("do {") && out.contains("} while (!(done));"),
            "expected oneliner do-while:\n{}",
            out
        );
    }

    #[test]
    fn jadx_loops_do_while_break_in_else() {
        let body = r#"        while (true) {
            work();
            if (ok) {
            } else {
                break;
            }
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("do {") && out.contains("} while (ok);"),
            "expected else-break do-while:\n{}",
            out
        );
    }

    #[test]
    fn jadx_switches_enum_switchmap() {
        let body = r#"        Outer.$SwitchMap$com$example$Color[Color.RED.ordinal()] = 1;
        Outer.$SwitchMap$com$example$Color[Color.BLUE.ordinal()] = 2;
        switch (Outer.$SwitchMap$com$example$Color[color.ordinal()]) {
            case 1:
                return "red";
            case 2:
                return "blue";
            default:
                return "?";
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("switch (color)"),
            "expected switch on enum expr:\n{}",
            out
        );
        assert!(
            out.contains("case Color.RED:") || out.contains("case com.example.Color.RED:"),
            "expected enum case label:\n{}",
            out
        );
        assert!(
            out.contains("case Color.BLUE:") || out.contains("case com.example.Color.BLUE:"),
            "expected blue case:\n{}",
            out
        );
        assert!(
            !out.contains("$SwitchMap$") && !out.contains(".ordinal()"),
            "SwitchMap/ordinal should be gone:\n{}",
            out
        );

        let body2 = r#"        int[] map = Outer.$SwitchMap$com$example$Color;
        switch (map[color.ordinal()]) {
            case 1:
                use(1);
        }
"#;
        let out2 = simplify_method_body(body2, false);
        assert!(out2.contains("switch (color)"), "map form:\n{}", out2);
        assert!(
            !out2.contains("$SwitchMap$"),
            "map assign dropped:\n{}",
            out2
        );

        let body3 = r#"        switch (color.ordinal()) {
            case 0:
                break;
        }
"#;
        let out3 = simplify_method_body(body3, false);
        assert!(out3.contains("switch (color)"), "plain ordinal:\n{}", out3);
    }

    #[test]
    fn jadx_try_with_resources() {
        let body = r#"        FileInputStream r = new FileInputStream(path);
        try {
            use(r);
        } catch (IOException e) {
            log(e);
        } finally {
            if (r != null) {
                r.close();
            }
        }
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("try (FileInputStream r = new FileInputStream(path))"),
            "expected try-with-resources:\n{}",
            out
        );
        assert!(out.contains("use(r);"), "{}", out);
        assert!(
            out.contains("} catch (IOException e)"),
            "catch should remain:\n{}",
            out
        );
        assert!(
            !out.contains("finally") && !out.contains(".close()"),
            "finally/close should be gone:\n{}",
            out
        );
    }

    #[test]
    fn jadx_try_with_resources_multi() {
        let body = r#"        FileInputStream in = new FileInputStream(path);
        FileOutputStream out = new FileOutputStream(dest);
        try {
            copy(in, out);
        } finally {
            if (in != null) {
                in.close();
            }
            if (out != null) {
                out.close();
            }
        }
"#;
        let simplified = simplify_method_body(body, false);
        assert!(
            simplified.contains(
                "try (FileInputStream in = new FileInputStream(path); FileOutputStream out = new FileOutputStream(dest))"
            ),
            "expected multi try-with-resources:\n{}",
            simplified
        );
        assert!(simplified.contains("copy(in, out);"), "{}", simplified);
        assert!(
            !simplified.contains("finally") && !simplified.contains(".close()"),
            "finally/close should be gone:\n{}",
            simplified
        );
    }

    #[test]
    fn jadx_strip_redundant_cast() {
        let body = r#"        String s = (String) new String("x");
        Object o = (String) (String) y;
        Object n = (String) null;
        use((String) new String("z"));
"#;
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("String s = new String(\"x\")"),
            "cast of new should drop:\n{}",
            out
        );
        assert!(
            out.contains("Object o = (String) y") || out.contains("o = (String)y"),
            "double cast should collapse:\n{}",
            out
        );
        assert!(
            !out.contains("(String) (String)") && !out.contains("(String)(String)"),
            "double same cast gone:\n{}",
            out
        );
        assert!(
            out.contains("Object n = null") || out.contains("n = null"),
            "cast of null:\n{}",
            out
        );
        assert!(
            out.contains("use(new String(\"z\"))"),
            "cast of new in call:\n{}",
            out
        );
    }

    #[test]
    fn trim_trailing_stray_braces_skips_blank_lines() {
        use crate::decompile::simplify::try_catch::trim_trailing_stray_braces;
        let body = "        try (R r = new R()) {\n            return 7;\n        }\n    }\n\n}\n\n}\n\n}\n    }\n";
        let out = trim_trailing_stray_braces(body);
        assert_eq!(
            out,
            "        try (R r = new R()) {\n            return 7;\n        }\n"
        );
    }

    #[test]
    fn jadx_string_concat_indy() {
        let body = r#"        String s = /* invoke-custom makeConcat */ (a, b, c);
        String t = StringConcatFactory.makeConcat(x, y);
        return /* invoke-custom makeConcatWithConstants */ (p, q);
"#;
        let out = simplify_method_body(body, false);
        assert!(out.contains("a + b + c"), "makeConcat comment:\n{}", out);
        assert!(out.contains("x + y"), "StringConcatFactory:\n{}", out);
        assert!(out.contains("return p + q"), "return concat:\n{}", out);
        assert!(!out.contains("makeConcat"), "indy leftover gone:\n{}", out);
    }

    #[test]
    fn restore_while_true_nested_for_minimal() {
        let body = "        while (true) {\n\
            int j = arr.length - 1;\n\
            if (0 >= j) {\n\
                return;\n\
            } else {\n\
                j = 0;\n\
                do {\n\
                    int tmp = arr.length - 0;\n\
                    tmp = tmp - 1;\n\
                } while (j >= tmp);\n\
                j = j + 1;\n\
            }\n\
        }\n";
        let out = restore_while_true_nested_for(body);
        assert!(
            out.contains("for (int i = 0; i < n - 1; i++)"),
            "minimal outer for:\n{out}"
        );
    }

    #[test]
    fn restore_while_true_nested_for_bubble_sort() {
        let body = "        while (true) {\n\
            int j = arr.length - 1;\n\
            if (0 >= j) {\n\
                return;\n\
            } else {\n\
                j = 0;\n\
                do {\n\
                    int tmp = arr.length - 0;\n\
                    tmp = tmp - 1;\n\
                } while (j >= tmp);\n\
                tmp = arr[j];\n\
                int j_0 = j + 1;\n\
                j_0 = arr[j_0];\n\
                if (tmp > j_0) {\n\
                    tmp = arr[j];\n\
                    j_0 = j + 1;\n\
                    j_0 = arr[j_0];\n\
                    arr[j] = j_0;\n\
                    j_0 = j + 1;\n\
                    arr[j_0] = tmp;\n\
                }\n\
                j = j + 1;\n\
            }\n\
        }\n";
        let out = restore_while_true_nested_for(body);
        assert!(
            out.contains("for (int i = 0; i < n - 1; i++)"),
            "outer for:\n{out}"
        );
        assert!(
            out.contains("for (int j = 0; j < n - i - 1; j++)"),
            "inner for:\n{out}"
        );
    }

    #[test]
    fn repair_self_ref_preserves_for_loop_header() {
        use crate::decompile::simplify::repair::repair_self_referential_call_args;
        let body = "        int i = 0;\n\
        int n = arr.length;\n\
        for (int i = 0; i < n - 1; i++) {\n\
            for (int j = 0; j < n - i - 1; j++) {\n\
                if (arr[j] > arr[j + 1]) {\n\
                    int tmp = arr[j];\n\
                    arr[j] = arr[j + 1];\n\
                    arr[j + 1] = tmp;\n\
                }\n\
            }\n\
        }\n";
        let out = repair_self_referential_call_args(body);
        assert!(
            out.contains("for (int j = 0; j < n - i - 1; j++)"),
            "repair_self_ref must not mangle for header:\n{out}"
        );
    }

    #[test]
    fn repair_self_ref_uses_preceding_const_not_unrelated_literal() {
        use crate::decompile::simplify::repair::repair_self_referential_call_args;
        let body = "        int i4 = 18;\n\
        int gcd = gcdEuclid(48, i4);\n\
        i4 = 20;\n\
        int i9 = 2;\n\
        i9 = bfsShortestPath(g, 0, i9);\n";
        let out = repair_self_referential_call_args(body);
        assert!(
            out.contains("bfsShortestPath(g, 0, 2)"),
            "self-ref bfs arg must be the pre-call 2, not 18:\n{out}"
        );
        assert!(
            !out.contains("18);") && !out.contains("; i9)"),
            "must not splice leftover tokens:\n{out}"
        );
    }

    #[test]
    fn repair_register_reuse_preserves_for_loop_header() {
        use crate::decompile::simplify::repair::repair_register_reuse_scalars;
        let body = "        int i = 0;\n\
        int n = arr.length;\n\
        for (int i = 0; i < n - 1; i++) {\n\
            for (int j = 0; j < n - i - 1; j++) {\n\
                if (arr[j] > arr[j + 1]) {\n\
                    int tmp = arr[j];\n\
                    arr[j] = arr[j + 1];\n\
                    arr[j + 1] = tmp;\n\
                }\n\
            }\n\
        }\n";
        let out = repair_register_reuse_scalars(body);
        assert!(
            out.contains("for (int j = 0; j < n - i - 1; j++)"),
            "repair must not mangle for header:\n{out}"
        );
    }

    #[test]
    fn simplify_bubble_sort_live_emission_shape() {
        let body = "        int i = 0;\n\
        while (true) {\n\
            int j = arr.length - 1;\n\
            if (i >= j) {\n\
                return;\n\
            } else {\n\
                j = 0;\n\
                do {\n\
                    int tmp = arr.length - i - 1;\n\
                } while (j >= tmp);\n\
                tmp = arr[j];\n\
                int j_0 = j + 1;\n\
                j_0 = arr[j_0];\n\
                if (tmp > j_0) {\n\
                    tmp = arr[j];\n\
                    j_0 = j + 1;\n\
                    j_0 = arr[j_0];\n\
                    arr[j] = j_0;\n\
                    j_0 = j + 1;\n\
                    arr[j_0] = tmp;\n\
                }\n\
                j++;\n\
            }\n\
        }\n";
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("for (int i = 0; i < n - 1; i++)"),
            "outer for:\n{out}"
        );
        assert!(
            out.contains("for (int j = 0; j < n - i - 1; j++)"),
            "inner for:\n{out}"
        );
        assert!(out.contains("arr[j] > arr[j + 1]"), "swap if:\n{out}");
        assert!(
            !out.lines().any(|l| l.trim() == "int i = 0;"),
            "stale init removed:\n{out}"
        );
    }

    #[test]
    fn simplify_bubble_sort_from_fixture_dex_shape() {
        let body = "        while (true) {\n\
            int j = arr.length - 1;\n\
            if (0 >= j) {\n\
                return;\n\
            } else {\n\
                j = 0;\n\
                do {\n\
                    int tmp = arr.length - 0;\n\
                    tmp = tmp - 1;\n\
                } while (j >= tmp);\n\
                tmp = arr[j];\n\
                int j_0 = j + 1;\n\
                j_0 = arr[j_0];\n\
                if (tmp > j_0) {\n\
                    tmp = arr[j];\n\
                    j_0 = j + 1;\n\
                    j_0 = arr[j_0];\n\
                    arr[j] = j_0;\n\
                    j_0 = j + 1;\n\
                    arr[j_0] = tmp;\n\
                }\n\
                j = j + 1;\n\
            }\n\
        }\n";
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("for (int i = 0; i < n - 1; i++)"),
            "outer for:\n{out}"
        );
        assert!(
            out.contains("for (int j = 0; j < n - i - 1; j++)"),
            "inner for:\n{out}"
        );
        assert!(out.contains("arr[j] > arr[j + 1]"), "swap if:\n{out}");
        assert!(!out.contains("while (true)"), "while true gone:\n{out}");
    }

    #[test]
    fn simplify_d8_merge_copy_to_structured_whiles() {
        let body = "        int[] out = new int[left.length + right.length];\n\
        int i = 0;\n\
        int j = 0;\n\
        int k = 0;\n\
        while (true) {\n\
            int k_0 = left.length;\n\
            if (i >= k_0) {\n\
                while (true) {\n\
                    k_0 = left.length;\n\
                    if (i >= k_0) {\n\
                        while (j < k_0) {\n\
                            k_0 = right.length;\n\
                            k_0 = k + 1;\n\
                            int j_0 = j + 1;\n\
                            j = right[j];\n\
                            out[k] = j;\n\
                            k = k_0;\n\
                            j = j_0;\n\
                            continue;\n\
                        }\n\
                        return out;\n\
                    } else {\n\
                        k_0 = k + 1;\n\
                        j_0 = i + 1;\n\
                        i = left[i];\n\
                        out[k] = i;\n\
                        k = k_0;\n\
                        i = j_0;\n\
                        continue;\n\
                    }\n\
                }\n\
            } else {\n\
                k_0 = right.length;\n\
                if (j < k_0) {\n\
                    k_0 = left[i];\n\
                    j_0 = right[j];\n\
                    if (k_0 > j_0) {\n\
                        k_0 = k + 1;\n\
                        j_0 = j + 1;\n\
                        j = right[j];\n\
                        out[k] = j;\n\
                        k = k_0;\n\
                        j = j_0;\n\
                        continue;\n\
                    } else {\n\
                        k_0 = k + 1;\n\
                        j_0 = i + 1;\n\
                        i = left[i];\n\
                        out[k] = i;\n\
                        k = k_0;\n\
                        i = j_0;\n\
                        continue;\n\
                    }\n\
                }\n\
            }\n\
        }\n";
        let out = simplify_method_body(body, false);
        assert!(
            out.contains("while (i < left.length && j < right.length)"),
            "main merge loop:\n{out}"
        );
        assert!(
            out.contains("out[k++] = left[i++]") && out.contains("out[k++] = right[j++]"),
            "postincrement stores:\n{out}"
        );
        assert!(
            !out.contains("while (true)") && !out.contains("out[0]"),
            "no while(true) or out[0]:\n{out}"
        );
        assert!(
            out.contains("while (i < left.length)") && out.contains("while (j < right.length)"),
            "drain loops:\n{out}"
        );
        assert!(out.contains("return out"), "return:\n{out}");
        assert!(
            !out.contains("k_0") && !out.contains("j_0"),
            "compare temps should inline:\n{out}"
        );
    }
}
