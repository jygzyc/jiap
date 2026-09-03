//! Per-instruction read/write sets (which registers are read/written).
//! Used for reaching definitions and value-flow / tainting.

use super::{
    parse_instance_field_operands, parse_one_reg, parse_static_field_operands, parse_three_regs,
    parse_two_regs, parse_two_regs_and_literal,
};

fn parse_vreg(token: &str) -> Option<u32> {
    token.trim().strip_prefix('v')?.parse().ok()
}

fn wide_pair(reg: u32) -> Vec<u32> {
    let mut regs = vec![reg];
    if let Some(high) = reg.checked_add(1) {
        regs.push(high);
    }
    regs
}

/// Parse the leading register operands, including decoder-emitted ranges such as
/// `v2 ... v5`. Parsing stops at the first reference/literal operand.
fn parse_reg_list(s: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for part in s.split(',').map(str::trim) {
        if !part.starts_with('v') {
            break;
        }
        let regs: Vec<u32> = part
            .split_whitespace()
            .filter_map(|token| parse_vreg(token.trim_matches(|c| c == '{' || c == '}')))
            .collect();
        if part.contains("..") && regs.len() >= 2 {
            if regs[0] <= regs[1] {
                out.extend(regs[0]..=regs[1]);
            }
        } else if let Some(&reg) = regs.first() {
            out.push(reg);
        }
    }
    out
}

/// Return (regs_read, regs_written) for an instruction given mnemonic and resolved operands.
/// Used for value-flow and tainting. Unknown opcodes return (empty, empty).
pub fn instruction_reads_writes(mnemonic: &str, ops_resolved: &str) -> (Vec<u32>, Vec<u32>) {
    let m = mnemonic;
    let ops = ops_resolved.trim();

    // move / move-object: dst = src
    if matches!(
        m,
        "move"
            | "move/from16"
            | "move/16"
            | "move-object"
            | "move-object/from16"
            | "move-object/16"
            | "move-wide"
            | "move-wide/from16"
            | "move-wide/16"
    ) {
        if let Some((dst, src)) = parse_two_regs(ops) {
            if m.starts_with("move-wide") {
                return (wide_pair(src), wide_pair(dst));
            }
            return (vec![src], vec![dst]);
        }
    }

    // move-result*: dst = result
    if m.starts_with("move-result") {
        if let Some(dst) = parse_one_reg(ops) {
            if m == "move-result-wide" {
                return (vec![], wide_pair(dst));
            }
            return (vec![], vec![dst]);
        }
    }

    // move-exception defines its sole destination from the exceptional edge.
    if m == "move-exception" {
        if let Some(dst) = parse_one_reg(ops) {
            return (vec![], vec![dst]);
        }
    }

    // const*: dst = literal
    if m.starts_with("const") {
        if let Some(dst) = parse_one_reg(ops) {
            if m.starts_with("const-wide") {
                return (vec![], wide_pair(dst));
            }
            return (vec![], vec![dst]);
        }
    }

    // return*: read reg
    if matches!(m, "return" | "return-wide" | "return-object") {
        if let Some(reg) = parse_one_reg(ops) {
            if m == "return-wide" {
                return (wide_pair(reg), vec![]);
            }
            return (vec![reg], vec![]);
        }
    }
    if m == "return-void" {
        return (vec![], vec![]);
    }

    // invoke-custom/polymorphic and all ordinary invoke forms read their argument registers.
    if m.starts_with("invoke-") {
        let regs = parse_invoke_arg_regs(ops);
        return (regs, vec![]);
    }

    // filled-new-array produces the result in the result pseudo-register; its explicit
    // registers are all reads and the following move-result-object is the definition.
    if matches!(m, "filled-new-array" | "filled-new-array/range") {
        return (parse_invoke_arg_regs(ops), vec![]);
    }

    // if-*: read one or two regs (filter branch offset)
    if m.starts_with("if-") {
        let regs: Vec<u32> = ops
            .split(',')
            .map(str::trim)
            .filter(|p| !(p.starts_with('+') || p.starts_with('-') || p.starts_with("0x")))
            .filter_map(|p| p.strip_prefix('v').and_then(|n| n.parse().ok()))
            .collect();
        if !regs.is_empty() {
            return (regs, vec![]);
        }
    }

    // packed-switch / sparse-switch: read one reg
    if matches!(m, "packed-switch" | "sparse-switch") {
        if let Some(reg) = parse_one_reg(ops) {
            return (vec![reg], vec![]);
        }
    }

    // Array access has operand roles unlike ordinary three-register arithmetic.
    if m.starts_with("aget") {
        if let Some((dst, array, index)) = parse_three_regs(ops) {
            let writes = if m == "aget-wide" {
                wide_pair(dst)
            } else {
                vec![dst]
            };
            return (vec![array, index], writes);
        }
    }
    if m.starts_with("aput") {
        if let Some((value, array, index)) = parse_three_regs(ops) {
            let mut reads = if m == "aput-wide" {
                wide_pair(value)
            } else {
                vec![value]
            };
            reads.extend([array, index]);
            return (reads, vec![]);
        }
    }

    // Binary ops: dst, src1, src2 or dst, src, lit
    if let Some((a, b, c)) = parse_three_regs(ops) {
        if matches!(
            m,
            "add-int"
                | "sub-int"
                | "mul-int"
                | "div-int"
                | "rem-int"
                | "and-int"
                | "or-int"
                | "xor-int"
                | "shl-int"
                | "shr-int"
                | "ushr-int"
                | "add-long"
                | "sub-long"
                | "mul-long"
                | "div-long"
                | "rem-long"
                | "and-long"
                | "or-long"
                | "xor-long"
                | "shl-long"
                | "shr-long"
                | "ushr-long"
                | "add-float"
                | "sub-float"
                | "mul-float"
                | "div-float"
                | "rem-float"
                | "add-double"
                | "sub-double"
                | "mul-double"
                | "div-double"
                | "rem-double"
        ) {
            if m.ends_with("-long") || m.ends_with("-double") {
                let mut reads = wide_pair(b);
                if matches!(m, "shl-long" | "shr-long" | "ushr-long") {
                    reads.push(c);
                } else {
                    reads.extend(wide_pair(c));
                }
                return (reads, wide_pair(a));
            }
            return (vec![b, c], vec![a]);
        }
    }
    if let Some((dst, src, _lit)) = parse_two_regs_and_literal(ops) {
        if matches!(
            m,
            "add-int/lit8"
                | "rsub-int/lit8"
                | "mul-int/lit8"
                | "div-int/lit8"
                | "rem-int/lit8"
                | "and-int/lit8"
                | "or-int/lit8"
                | "xor-int/lit8"
                | "shl-int/lit8"
                | "shr-int/lit8"
                | "ushr-int/lit8"
        ) {
            return (vec![src], vec![dst]);
        }
    }

    // array-length: dest, array -> read array, write dest
    if m == "array-length" {
        if let Some((dst, src)) = parse_two_regs(ops) {
            return (vec![src], vec![dst]);
        }
    }

    // instance-of: dst = src instanceof type.
    if let Some((dst, src)) = parse_two_regs(ops) {
        if m == "instance-of" {
            return (vec![src], vec![dst]);
        }
    }

    // check-cast reads and redefines the same register.
    if m == "check-cast" {
        if let Some(reg) = parse_one_reg(ops) {
            return (vec![reg], vec![reg]);
        }
    }

    // iget: dest, object, field -> read object, write dest
    if m.starts_with("iget") {
        if let Some((dest, object_reg, _)) = parse_instance_field_operands(ops) {
            let writes = if m == "iget-wide" {
                wide_pair(dest)
            } else {
                vec![dest]
            };
            return (vec![object_reg], writes);
        }
    }
    // iput: value, object, field -> read value, object
    if m.starts_with("iput") {
        if let Some((value_reg, object_reg, _)) = parse_instance_field_operands(ops) {
            let mut reads = if m == "iput-wide" {
                wide_pair(value_reg)
            } else {
                vec![value_reg]
            };
            reads.push(object_reg);
            return (reads, vec![]);
        }
    }
    // sget: reg, field -> write reg
    if m.starts_with("sget") {
        if let Some((reg, _)) = parse_static_field_operands(ops) {
            let writes = if m == "sget-wide" {
                wide_pair(reg)
            } else {
                vec![reg]
            };
            return (vec![], writes);
        }
    }
    // sput: reg, field -> read reg
    if m.starts_with("sput") {
        if let Some((reg, _)) = parse_static_field_operands(ops) {
            let reads = if m == "sput-wide" {
                wide_pair(reg)
            } else {
                vec![reg]
            };
            return (reads, vec![]);
        }
    }

    // throw: read reg
    if m == "throw" {
        if let Some(reg) = parse_one_reg(ops) {
            return (vec![reg], vec![]);
        }
    }

    // new-instance: write one reg
    if m == "new-instance" {
        if let Some(reg) = parse_one_reg(ops) {
            return (vec![], vec![reg]);
        }
    }

    // new-array: dest, size -> read size, write dest
    if m == "new-array" {
        let parts: Vec<&str> = ops.split(',').map(str::trim).collect();
        if parts.len() >= 2 {
            if let (Some(dst), Some(sz)) = (
                parts[0].strip_prefix('v').and_then(|n| n.parse().ok()),
                parts[1].strip_prefix('v').and_then(|n| n.parse().ok()),
            ) {
                return (vec![sz], vec![dst]);
            }
        }
    }

    // Monitors and fill-array-data consume an existing object/array register.
    if matches!(m, "monitor-enter" | "monitor-exit" | "fill-array-data") {
        if let Some(reg) = parse_one_reg(ops) {
            return (vec![reg], vec![]);
        }
    }

    // goto: no regs
    if m.starts_with("goto") || m == "nop" {
        return (vec![], vec![]);
    }

    (vec![], vec![])
}

/// From invoke operands "v0, v1, Lclass;->m(I)V" return [0, 1].
fn parse_invoke_arg_regs(ops: &str) -> Vec<u32> {
    let mut depth = 0u32;
    let mut last_comma = None;
    for (i, c) in ops.chars().enumerate() {
        match c {
            '(' => depth = depth.saturating_add(1),
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => last_comma = Some(i),
            _ => {}
        }
    }
    let args_str = match last_comma {
        Some(i) => ops[..i].trim(),
        None => ops.trim(),
    };
    parse_reg_list(args_str)
}

#[cfg(test)]
mod tests {
    use super::instruction_reads_writes;

    #[test]
    fn move_read_write() {
        let (r, w) = instruction_reads_writes("move", "v1, v0");
        assert_eq!(r, vec![0]);
        assert_eq!(w, vec![1]);
    }

    #[test]
    fn return_read() {
        let (r, w) = instruction_reads_writes("return", "v0");
        assert_eq!(r, vec![0]);
        assert!(w.is_empty());
    }

    #[test]
    fn const_write() {
        let (r, w) = instruction_reads_writes("const/4", "v0, 0");
        assert!(r.is_empty());
        assert_eq!(w, vec![0]);
    }

    #[test]
    fn move_object_from16_is_a_copy() {
        let (r, w) = instruction_reads_writes("move-object/from16", "v0, v3");
        assert_eq!(r, vec![3]);
        assert_eq!(w, vec![0]);
    }

    #[test]
    fn array_length_write() {
        let (r, w) = instruction_reads_writes("array-length", "v0, v3");
        assert_eq!(r, vec![3]);
        assert_eq!(w, vec![0]);
    }

    #[test]
    fn move_object16_and_move_wide16_are_copies() {
        let (r, w) = instruction_reads_writes("move-object/16", "v0, v3");
        assert_eq!(r, vec![3]);
        assert_eq!(w, vec![0]);
        let (r, w) = instruction_reads_writes("move-wide/16", "v4, v6");
        assert_eq!(r, vec![6, 7]);
        assert_eq!(w, vec![4, 5]);
    }

    #[test]
    fn exception_throw_monitor_and_fill_array_data() {
        assert_eq!(
            instruction_reads_writes("move-exception", "v3"),
            (vec![], vec![3])
        );
        assert_eq!(instruction_reads_writes("throw", "v3"), (vec![3], vec![]));
        assert_eq!(
            instruction_reads_writes("monitor-enter", "v4"),
            (vec![4], vec![])
        );
        assert_eq!(
            instruction_reads_writes("monitor-exit", "v4"),
            (vec![4], vec![])
        );
        assert_eq!(
            instruction_reads_writes("fill-array-data", "v5, +4h"),
            (vec![5], vec![])
        );
    }

    #[test]
    fn filled_array_and_invoke_ranges_expand_all_registers() {
        assert_eq!(
            instruction_reads_writes("filled-new-array", "v1, v3, v5, int[]"),
            (vec![1, 3, 5], vec![])
        );
        assert_eq!(
            instruction_reads_writes("filled-new-array/range", "v2 ... v5, int[]"),
            (vec![2, 3, 4, 5], vec![])
        );
        assert_eq!(
            instruction_reads_writes(
                "invoke-polymorphic/range",
                "v7 .. v9 java.lang.invoke.MethodHandle.invoke proto@1"
            ),
            (vec![7, 8, 9], vec![])
        );
        assert_eq!(
            instruction_reads_writes("invoke-custom", "v0, v2, callsite@3"),
            (vec![0, 2], vec![])
        );
    }

    #[test]
    fn array_access_distinguishes_value_array_and_index() {
        assert_eq!(
            instruction_reads_writes("aget-object", "v0, v1, v2"),
            (vec![1, 2], vec![0])
        );
        assert_eq!(
            instruction_reads_writes("aget-wide", "v4, v1, v2"),
            (vec![1, 2], vec![4, 5])
        );
        assert_eq!(
            instruction_reads_writes("aput", "v0, v1, v2"),
            (vec![0, 1, 2], vec![])
        );
        assert_eq!(
            instruction_reads_writes("aput-wide", "v4, v1, v2"),
            (vec![4, 5, 1, 2], vec![])
        );
    }

    #[test]
    fn wide_constants_results_and_returns_cover_register_pairs() {
        assert_eq!(
            instruction_reads_writes("const-wide/high16", "v2, 1"),
            (vec![], vec![2, 3])
        );
        assert_eq!(
            instruction_reads_writes("move-result-wide", "v4"),
            (vec![], vec![4, 5])
        );
        assert_eq!(
            instruction_reads_writes("return-wide", "v4"),
            (vec![4, 5], vec![])
        );
    }
}
