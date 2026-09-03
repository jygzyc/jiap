//! Instruction interpreter: executes one Dalvik instruction at a time.

use super::state::*;

impl Emulator {
    /// Execute the instruction at `self.pc`, advance PC, and record history.
    /// Returns a `StepResult` with the executed instruction, state after, and description,
    /// so callers can drive the emulator step-by-step (e.g. debugger UI, scripts).
    pub fn step(&mut self) -> Result<StepResult, EmulatorError> {
        if self.finished {
            let snapshot = self.snapshot();
            return Ok(StepResult {
                step_count: self.step_count,
                instruction: InstructionInfo {
                    index: self.pc.min(self.instructions.len().saturating_sub(1)),
                    offset: self
                        .instructions
                        .get(self.pc)
                        .map(|i| i.offset)
                        .unwrap_or(0),
                    mnemonic: "nop".into(),
                    operands: String::new(),
                },
                state_after: snapshot,
                description: "(finished)".into(),
                finished: true,
            });
        }
        if self.step_count >= self.max_steps {
            return Err(EmulatorError::ExecutionLimit(self.max_steps));
        }
        if self.pc >= self.instructions.len() {
            self.finished = true;
            let snapshot = self.snapshot();
            return Ok(StepResult {
                step_count: self.step_count,
                instruction: InstructionInfo {
                    index: self.pc.saturating_sub(1),
                    offset: 0,
                    mnemonic: "nop".into(),
                    operands: String::new(),
                },
                state_after: snapshot,
                description: "(past end of instructions)".into(),
                finished: true,
            });
        }

        let ins = self.instructions[self.pc].clone();
        let resolved = self.resolved_operands[self.pc].clone();
        let pc_before = self.pc;

        let desc = self.execute_instruction(&ins, &resolved)?;

        self.step_count += 1;
        let snapshot = self.snapshot();
        self.history.push(StepRecord {
            step: self.step_count,
            pc_before,
            instruction: ins.clone(),
            state_after: snapshot.clone(),
            description: desc.clone(),
        });

        Ok(StepResult {
            step_count: self.step_count,
            instruction: ins,
            state_after: snapshot,
            description: desc,
            finished: self.finished,
        })
    }

    /// Run until finished or error, up to max_steps.
    pub fn run_to_end(&mut self) -> Result<(), EmulatorError> {
        while !self.finished {
            self.step()?;
        }
        Ok(())
    }

    fn execute_instruction(
        &mut self,
        ins: &InstructionInfo,
        resolved: &str,
    ) -> Result<String, EmulatorError> {
        let m = ins.mnemonic.as_str();
        match m {
            "nop" => {
                self.pc += 1;
                Ok("nop".into())
            }

            "const/4" | "const/16" | "const" | "const/high16" => self.exec_const(resolved),
            "const-wide/16" | "const-wide/32" | "const-wide" | "const-wide/high16" => {
                self.exec_const_wide(resolved)
            }
            "const-string" | "const-string/jumbo" => self.exec_const_string(resolved),

            "move" | "move/from16" | "move/16" | "move-wide" | "move-wide/from16"
            | "move-wide/16" | "move-object" | "move-object/from16" | "move-object/16" => {
                self.exec_move(resolved)
            }
            "move-result" | "move-result-wide" | "move-result-object" => {
                self.exec_move_result(resolved)
            }
            "move-exception" => self.exec_move_exception(resolved),

            "return-void" => {
                self.finished = true;
                self.return_value = None;
                Ok("return void".into())
            }
            "return" | "return-wide" | "return-object" => self.exec_return(resolved),

            "add-int" | "sub-int" | "mul-int" | "div-int" | "rem-int" | "and-int" | "or-int"
            | "xor-int" | "shl-int" | "shr-int" | "ushr-int" => self.exec_binop_int(m, resolved),
            "add-int/2addr" | "sub-int/2addr" | "mul-int/2addr" | "div-int/2addr"
            | "rem-int/2addr" | "and-int/2addr" | "or-int/2addr" | "xor-int/2addr"
            | "shl-int/2addr" | "shr-int/2addr" | "ushr-int/2addr" => {
                self.exec_binop_int_2addr(m, resolved)
            }
            "add-int/lit8" | "add-int/lit16" | "rsub-int" | "rsub-int/lit8" | "mul-int/lit8"
            | "mul-int/lit16" | "div-int/lit8" | "div-int/lit16" | "rem-int/lit8"
            | "rem-int/lit16" | "and-int/lit8" | "and-int/lit16" | "or-int/lit8"
            | "or-int/lit16" | "xor-int/lit8" | "xor-int/lit16" | "shl-int/lit8"
            | "shr-int/lit8" | "ushr-int/lit8" => self.exec_binop_int_lit(m, resolved),

            "add-long" | "sub-long" | "mul-long" | "div-long" | "rem-long" | "and-long"
            | "or-long" | "xor-long" | "add-long/2addr" | "sub-long/2addr" | "mul-long/2addr"
            | "div-long/2addr" | "rem-long/2addr" | "and-long/2addr" | "or-long/2addr"
            | "xor-long/2addr" => self.exec_binop_long(m, resolved),

            "add-float" | "sub-float" | "mul-float" | "div-float" | "rem-float"
            | "add-float/2addr" | "sub-float/2addr" | "mul-float/2addr" | "div-float/2addr"
            | "rem-float/2addr" => self.exec_binop_float(m, resolved),

            "add-double" | "sub-double" | "mul-double" | "div-double" | "rem-double"
            | "add-double/2addr" | "sub-double/2addr" | "mul-double/2addr" | "div-double/2addr"
            | "rem-double/2addr" => self.exec_binop_double(m, resolved),

            "neg-int" => self.exec_unary_int(resolved, |a| -a, "neg"),
            "not-int" => self.exec_unary_int(resolved, |a| !a, "not"),
            "int-to-long" => self.exec_int_to_long(resolved),
            "int-to-float" => self.exec_int_to_float(resolved),
            "int-to-double" => self.exec_int_to_double(resolved),
            "long-to-int" => self.exec_long_to_int(resolved),
            "float-to-int" => self.exec_float_to_int(resolved),
            "double-to-int" => self.exec_double_to_int(resolved),
            "int-to-byte" => self.exec_unary_int(resolved, |a| (a as i8) as i32, "i2b"),
            "int-to-char" => self.exec_unary_int(resolved, |a| (a as u16) as i32, "i2c"),
            "int-to-short" => self.exec_unary_int(resolved, |a| (a as i16) as i32, "i2s"),

            "if-eq" | "if-ne" | "if-lt" | "if-ge" | "if-gt" | "if-le" => {
                self.exec_if_two(m, &ins.operands)
            }
            "if-eqz" | "if-nez" | "if-ltz" | "if-gez" | "if-gtz" | "if-lez" => {
                self.exec_if_zero(m, &ins.operands)
            }

            "goto" | "goto/16" | "goto/32" => self.exec_goto(m, &ins.operands),

            "new-array" => self.exec_new_array(resolved),
            "aget" | "aget-wide" | "aget-object" | "aget-boolean" | "aget-byte" | "aget-char"
            | "aget-short" => self.exec_aget(resolved),
            "aput" | "aput-wide" | "aput-object" | "aput-boolean" | "aput-byte" | "aput-char"
            | "aput-short" => self.exec_aput(resolved),
            "array-length" => self.exec_array_length(resolved),

            "new-instance" => self.exec_new_instance(resolved),

            "sget" | "sget-wide" | "sget-object" | "sget-boolean" | "sget-byte" | "sget-char"
            | "sget-short" => self.exec_sget(resolved),
            "sput" | "sput-wide" | "sput-object" | "sput-boolean" | "sput-byte" | "sput-char"
            | "sput-short" => {
                self.pc += 1;
                Ok(format!("sput {}", resolved))
            }
            "iget" | "iget-wide" | "iget-object" | "iget-boolean" | "iget-byte" | "iget-char"
            | "iget-short" => self.exec_iget(resolved),
            "iput" | "iput-wide" | "iput-object" | "iput-boolean" | "iput-byte" | "iput-char"
            | "iput-short" => {
                self.pc += 1;
                Ok(format!("iput {}", resolved))
            }

            m if m.starts_with("invoke-") => self.exec_invoke(resolved),

            "fill-array-data" | "filled-new-array" | "filled-new-array/range" => {
                self.pc += 1;
                Ok(format!("{} (stubbed)", m))
            }

            "throw" => {
                let reg = parse_single_reg(&ins.operands)?;
                let val = self.get_reg(reg)?.clone();
                self.exception = Some(format!("throw {}", val.display_short()));
                self.finished = true;
                Ok(format!("throw v{}", reg))
            }

            "check-cast" | "instance-of" | "monitor-enter" | "monitor-exit" | "packed-switch"
            | "sparse-switch" | "cmpg-float" | "cmpl-float" | "cmpg-double" | "cmpl-double"
            | "cmp-long" => self.exec_compare_or_stub(m, &ins.operands, resolved),

            _ => {
                self.pc += 1;
                Ok(format!("{} {} (not emulated)", m, resolved))
            }
        }
    }

    // ---- const ----
    fn exec_const(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let (reg, val) = parse_reg_and_literal(&ops)?;
        let v = val as i32;
        self.set_reg(reg, Value::Int(v))?;
        self.pc += 1;
        Ok(format!("v{} = {}", reg, v))
    }

    fn exec_const_wide(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let (reg, val) = parse_reg_and_literal(&ops)?;
        self.set_reg(reg, Value::Long(val))?;
        self.pc += 1;
        Ok(format!("v{} = {}L", reg, val))
    }

    fn exec_const_string(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let parts: Vec<&str> = ops.splitn(2, ',').collect();
        if parts.len() < 2 {
            self.pc += 1;
            return Ok(format!("const-string (parse error: {})", ops));
        }
        let reg = parse_reg(parts[0].trim())?;
        let s = parts[1].trim().trim_matches('"').to_string();
        self.set_reg(reg, Value::Str(s.clone()))?;
        self.pc += 1;
        Ok(format!("v{} = \"{}\"", reg, s))
    }

    // ---- move ----
    fn exec_move(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let (dst, src) = parse_two_regs(&ops)?;
        let val = self.get_reg(src)?.clone();
        self.set_reg(dst, val)?;
        self.pc += 1;
        Ok(format!("v{} = v{}", dst, src))
    }

    fn exec_move_result(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let reg = parse_single_reg(&ops)?;
        let val = self
            .last_invoke_result
            .take()
            .unwrap_or(Value::Unknown("move-result".into()));
        let desc = format!("v{} = {}", reg, val.display_short());
        self.set_reg(reg, val)?;
        self.pc += 1;
        Ok(desc)
    }

    fn exec_move_exception(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let reg = parse_single_reg(&ops)?;
        self.set_reg(reg, Value::Unknown("exception".into()))?;
        self.pc += 1;
        Ok(format!("v{} = <exception>", reg))
    }

    // ---- return ----
    fn exec_return(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let reg = parse_single_reg(&ops)?;
        let val = self.get_reg(reg)?.clone();
        self.return_value = Some(val.clone());
        self.finished = true;
        Ok(format!("return v{} ({})", reg, val.display_short()))
    }

    // ---- binary int ops ----
    fn exec_binop_int(&mut self, mnemonic: &str, ops: &str) -> Result<String, EmulatorError> {
        let (dst, a, b) = parse_three_regs_str(&ops)?;
        let va = self.get_reg(a)?.as_int().unwrap_or(0);
        let vb = self.get_reg(b)?.as_int().unwrap_or(0);
        let result = self.int_op(mnemonic, va, vb)?;
        self.set_reg(dst, Value::Int(result))?;
        self.pc += 1;
        Ok(format!(
            "v{} = {} {} {} = {}",
            dst,
            va,
            op_symbol(mnemonic),
            vb,
            result
        ))
    }

    fn exec_binop_int_2addr(&mut self, mnemonic: &str, ops: &str) -> Result<String, EmulatorError> {
        let (dst, src) = parse_two_regs(&ops)?;
        let va = self.get_reg(dst)?.as_int().unwrap_or(0);
        let vb = self.get_reg(src)?.as_int().unwrap_or(0);
        let base = mnemonic.trim_end_matches("/2addr");
        let result = self.int_op(base, va, vb)?;
        self.set_reg(dst, Value::Int(result))?;
        self.pc += 1;
        Ok(format!(
            "v{} = {} {} {} = {}",
            dst,
            va,
            op_symbol(base),
            vb,
            result
        ))
    }

    fn exec_binop_int_lit(&mut self, mnemonic: &str, ops: &str) -> Result<String, EmulatorError> {
        let (dst, src, lit) = parse_two_regs_and_literal(&ops)?;
        let va = self.get_reg(src)?.as_int().unwrap_or(0);
        let lit_i = lit as i32;
        let base_op = mnemonic.split('/').next().unwrap_or(mnemonic);
        let result = if base_op == "rsub-int" {
            lit_i.wrapping_sub(va)
        } else {
            self.int_op(base_op, va, lit_i)?
        };
        self.set_reg(dst, Value::Int(result))?;
        self.pc += 1;
        Ok(format!(
            "v{} = {} {} {} = {}",
            dst,
            va,
            op_symbol(base_op),
            lit_i,
            result
        ))
    }

    fn int_op(&self, mnemonic: &str, a: i32, b: i32) -> Result<i32, EmulatorError> {
        Ok(match mnemonic {
            "add-int" => a.wrapping_add(b),
            "sub-int" => a.wrapping_sub(b),
            "mul-int" => a.wrapping_mul(b),
            "div-int" => {
                if b == 0 {
                    return Err(EmulatorError::DivisionByZero(self.pc));
                }
                a.wrapping_div(b)
            }
            "rem-int" => {
                if b == 0 {
                    return Err(EmulatorError::DivisionByZero(self.pc));
                }
                a.wrapping_rem(b)
            }
            "and-int" => a & b,
            "or-int" => a | b,
            "xor-int" => a ^ b,
            "shl-int" => a.wrapping_shl(b as u32 & 0x1f),
            "shr-int" => a.wrapping_shr(b as u32 & 0x1f),
            "ushr-int" => ((a as u32).wrapping_shr(b as u32 & 0x1f)) as i32,
            _ => a,
        })
    }

    // ---- binary long ops ----
    fn exec_binop_long(&mut self, mnemonic: &str, ops: &str) -> Result<String, EmulatorError> {
        let base = mnemonic.replace("/2addr", "").replace("-long", "-int");
        let is_2addr = mnemonic.contains("2addr");
        if is_2addr {
            let (dst, src) = parse_two_regs(&ops)?;
            let va = self.get_reg(dst)?.as_long().unwrap_or(0);
            let vb = self.get_reg(src)?.as_long().unwrap_or(0);
            let result = long_op(&base, va, vb, self.pc)?;
            self.set_reg(dst, Value::Long(result))?;
            self.pc += 1;
            Ok(format!("v{} = {}L", dst, result))
        } else {
            let (dst, a, b) = parse_three_regs_str(&ops)?;
            let va = self.get_reg(a)?.as_long().unwrap_or(0);
            let vb = self.get_reg(b)?.as_long().unwrap_or(0);
            let result = long_op(&base, va, vb, self.pc)?;
            self.set_reg(dst, Value::Long(result))?;
            self.pc += 1;
            Ok(format!("v{} = {}L", dst, result))
        }
    }

    // ---- binary float ops ----
    fn exec_binop_float(&mut self, mnemonic: &str, ops: &str) -> Result<String, EmulatorError> {
        let is_2addr = mnemonic.contains("2addr");
        if is_2addr {
            let (dst, src) = parse_two_regs(&ops)?;
            let a = get_float(&self.registers, dst);
            let b = get_float(&self.registers, src);
            let r = float_op(mnemonic, a, b);
            self.set_reg(dst, Value::Float(r))?;
            self.pc += 1;
            Ok(format!("v{} = {}f", dst, r))
        } else {
            let (dst, ra, rb) = parse_three_regs_str(&ops)?;
            let a = get_float(&self.registers, ra);
            let b = get_float(&self.registers, rb);
            let r = float_op(mnemonic, a, b);
            self.set_reg(dst, Value::Float(r))?;
            self.pc += 1;
            Ok(format!("v{} = {}f", dst, r))
        }
    }

    // ---- binary double ops ----
    fn exec_binop_double(&mut self, mnemonic: &str, ops: &str) -> Result<String, EmulatorError> {
        let is_2addr = mnemonic.contains("2addr");
        if is_2addr {
            let (dst, src) = parse_two_regs(&ops)?;
            let a = get_double(&self.registers, dst);
            let b = get_double(&self.registers, src);
            let r = double_op(mnemonic, a, b);
            self.set_reg(dst, Value::Double(r))?;
            self.pc += 1;
            Ok(format!("v{} = {}d", dst, r))
        } else {
            let (dst, ra, rb) = parse_three_regs_str(&ops)?;
            let a = get_double(&self.registers, ra);
            let b = get_double(&self.registers, rb);
            let r = double_op(mnemonic, a, b);
            self.set_reg(dst, Value::Double(r))?;
            self.pc += 1;
            Ok(format!("v{} = {}d", dst, r))
        }
    }

    // ---- unary / conversion ----
    fn exec_unary_int<F: Fn(i32) -> i32>(
        &mut self,
        ops: &str,
        f: F,
        label: &str,
    ) -> Result<String, EmulatorError> {
        let (dst, src) = parse_two_regs(&ops)?;
        let va = self.get_reg(src)?.as_int().unwrap_or(0);
        let r = f(va);
        self.set_reg(dst, Value::Int(r))?;
        self.pc += 1;
        Ok(format!("v{} = {}(v{}) = {}", dst, label, src, r))
    }

    fn exec_int_to_long(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let (dst, src) = parse_two_regs(&ops)?;
        let v = self.get_reg(src)?.as_int().unwrap_or(0) as i64;
        self.set_reg(dst, Value::Long(v))?;
        self.pc += 1;
        Ok(format!("v{} = (long)v{} = {}L", dst, src, v))
    }

    fn exec_int_to_float(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let (dst, src) = parse_two_regs(&ops)?;
        let v = self.get_reg(src)?.as_int().unwrap_or(0) as f32;
        self.set_reg(dst, Value::Float(v))?;
        self.pc += 1;
        Ok(format!("v{} = (float)v{} = {}f", dst, src, v))
    }

    fn exec_int_to_double(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let (dst, src) = parse_two_regs(&ops)?;
        let v = self.get_reg(src)?.as_int().unwrap_or(0) as f64;
        self.set_reg(dst, Value::Double(v))?;
        self.pc += 1;
        Ok(format!("v{} = (double)v{} = {}d", dst, src, v))
    }

    fn exec_long_to_int(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let (dst, src) = parse_two_regs(&ops)?;
        let v = self.get_reg(src)?.as_long().unwrap_or(0) as i32;
        self.set_reg(dst, Value::Int(v))?;
        self.pc += 1;
        Ok(format!("v{} = (int)v{} = {}", dst, src, v))
    }

    fn exec_float_to_int(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let (dst, src) = parse_two_regs(&ops)?;
        let v = get_float(&self.registers, src) as i32;
        self.set_reg(dst, Value::Int(v))?;
        self.pc += 1;
        Ok(format!("v{} = (int)v{} = {}", dst, src, v))
    }

    fn exec_double_to_int(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let (dst, src) = parse_two_regs(&ops)?;
        let v = get_double(&self.registers, src) as i32;
        self.set_reg(dst, Value::Int(v))?;
        self.pc += 1;
        Ok(format!("v{} = (int)v{} = {}", dst, src, v))
    }

    // ---- branches ----
    fn exec_if_two(&mut self, mnemonic: &str, ops: &str) -> Result<String, EmulatorError> {
        let parts: Vec<&str> = ops.split(',').map(str::trim).collect();
        if parts.len() < 3 {
            self.pc += 1;
            return Ok(format!("{} (parse error)", mnemonic));
        }
        let ra = parse_reg(parts[0])?;
        let rb = parse_reg(parts[1])?;
        let offset: i64 = branch_offset_signed(mnemonic, parse_literal(parts[2]));
        let va = self.get_reg(ra)?.as_int().unwrap_or(0);
        let vb = self.get_reg(rb)?.as_int().unwrap_or(0);
        let taken = match mnemonic {
            "if-eq" => va == vb,
            "if-ne" => va != vb,
            "if-lt" => va < vb,
            "if-ge" => va >= vb,
            "if-gt" => va > vb,
            "if-le" => va <= vb,
            _ => false,
        };
        let desc = format!(
            "if v{}({}) {} v{}({}) → {}",
            ra,
            va,
            cond_symbol(mnemonic),
            rb,
            vb,
            taken
        );
        if taken {
            self.jump_by_offset(offset);
        } else {
            self.pc += 1;
        }
        Ok(desc)
    }

    fn exec_if_zero(&mut self, mnemonic: &str, ops: &str) -> Result<String, EmulatorError> {
        let parts: Vec<&str> = ops.split(',').map(str::trim).collect();
        if parts.len() < 2 {
            self.pc += 1;
            return Ok(format!("{} (parse error)", mnemonic));
        }
        let ra = parse_reg(parts[0])?;
        let offset: i64 = branch_offset_signed(mnemonic, parse_literal(parts[1]));
        let va = self.get_reg(ra)?.as_int().unwrap_or(0);
        let taken = match mnemonic {
            "if-eqz" => va == 0,
            "if-nez" => va != 0,
            "if-ltz" => va < 0,
            "if-gez" => va >= 0,
            "if-gtz" => va > 0,
            "if-lez" => va <= 0,
            _ => false,
        };
        let desc = format!("if v{}({}) {} 0 → {}", ra, va, cond_symbol(mnemonic), taken);
        if taken {
            self.jump_by_offset(offset);
        } else {
            self.pc += 1;
        }
        Ok(desc)
    }

    fn exec_goto(&mut self, mnemonic: &str, ops: &str) -> Result<String, EmulatorError> {
        let raw = parse_literal(ops.trim());
        let offset = branch_offset_signed(mnemonic, raw);
        let desc = format!("goto {:+}", offset);
        self.jump_by_offset(offset);
        Ok(desc)
    }

    /// Apply branch/goto offset. Dalvik encodes branch offsets in 16-bit code units;
    /// instruction offsets in our stream are in bytes, so we multiply by 2.
    fn jump_by_offset(&mut self, code_unit_offset: i64) {
        let current_byte_offset = self.instructions[self.pc].offset;
        let byte_delta = code_unit_offset * 2;
        let target_byte = (current_byte_offset as i64 + byte_delta) as u32;
        if let Some(&idx) = self.offset_to_index.get(&target_byte) {
            self.pc = idx;
        } else {
            // Fallback: find nearest instruction at or after target.
            let nearest = self
                .instructions
                .iter()
                .position(|i| i.offset >= target_byte);
            self.pc = nearest.unwrap_or(self.instructions.len());
            if self.pc >= self.instructions.len() {
                self.finished = true;
            }
        }
    }

    // ---- arrays ----
    fn exec_new_array(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let parts: Vec<&str> = ops.splitn(3, ',').map(str::trim).collect();
        if parts.len() < 3 {
            self.pc += 1;
            return Ok(format!("new-array (parse error: {})", ops));
        }
        let dst = parse_reg(parts[0])?;
        let size_reg = parse_reg(parts[1])?;
        let type_name = parts[2].to_string();
        let size = self.get_reg(size_reg)?.as_int().unwrap_or(0).max(0) as usize;
        let heap_idx = self.alloc(HeapObject {
            kind: HeapObjectKind::Array {
                element_type: type_name.clone(),
                values: vec![Value::Int(0); size],
            },
        });
        self.set_reg(dst, Value::Ref(heap_idx))?;
        self.pc += 1;
        Ok(format!(
            "v{} = new {}[{}] → @{}",
            dst, type_name, size, heap_idx
        ))
    }

    fn exec_aget(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let (dst, arr_reg, idx_reg) = parse_three_regs_str(&ops)?;
        let idx = self.get_reg(idx_reg)?.as_int().unwrap_or(0);
        let arr_val = self.get_reg(arr_reg)?.clone();
        match arr_val {
            Value::Ref(heap_idx) => {
                if let Some(obj) = self.heap.get(heap_idx) {
                    if let HeapObjectKind::Array { ref values, .. } = obj.kind {
                        if idx < 0 || idx as usize >= values.len() {
                            return Err(EmulatorError::ArrayIndexOutOfBounds(
                                self.pc,
                                idx,
                                values.len(),
                            ));
                        }
                        let val = values[idx as usize].clone();
                        self.set_reg(dst, val.clone())?;
                        self.pc += 1;
                        return Ok(format!(
                            "v{} = @{}[{}] = {}",
                            dst,
                            heap_idx,
                            idx,
                            val.display_short()
                        ));
                    }
                }
                self.set_reg(dst, Value::Unknown("aget".into()))?;
                self.pc += 1;
                Ok(format!("v{} = aget (non-array ref)", dst))
            }
            _ => {
                self.set_reg(dst, Value::Unknown("aget".into()))?;
                self.pc += 1;
                Ok(format!(
                    "v{} = aget (non-ref: {})",
                    dst,
                    arr_val.display_short()
                ))
            }
        }
    }

    fn exec_aput(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let (val_reg, arr_reg, idx_reg) = parse_three_regs_str(&ops)?;
        let idx = self.get_reg(idx_reg)?.as_int().unwrap_or(0);
        let val = self.get_reg(val_reg)?.clone();
        let arr_val = self.get_reg(arr_reg)?.clone();
        match arr_val {
            Value::Ref(heap_idx) => {
                if let Some(obj) = self.heap.get_mut(heap_idx) {
                    if let HeapObjectKind::Array { ref mut values, .. } = obj.kind {
                        if idx < 0 || idx as usize >= values.len() {
                            return Err(EmulatorError::ArrayIndexOutOfBounds(
                                self.pc,
                                idx,
                                values.len(),
                            ));
                        }
                        values[idx as usize] = val.clone();
                        self.pc += 1;
                        return Ok(format!("@{}[{}] = {}", heap_idx, idx, val.display_short()));
                    }
                }
                self.pc += 1;
                Ok(format!("aput (non-array ref @{})", heap_idx))
            }
            _ => {
                self.pc += 1;
                Ok(format!("aput (non-ref: {})", arr_val.display_short()))
            }
        }
    }

    fn exec_array_length(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let (dst, src) = parse_two_regs(&ops)?;
        let arr_val = self.get_reg(src)?.clone();
        let len = match arr_val {
            Value::Ref(heap_idx) => {
                if let Some(obj) = self.heap.get(heap_idx) {
                    if let HeapObjectKind::Array { ref values, .. } = obj.kind {
                        values.len() as i32
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
            _ => 0,
        };
        self.set_reg(dst, Value::Int(len))?;
        self.pc += 1;
        Ok(format!("v{} = array-length(v{}) = {}", dst, src, len))
    }

    // ---- objects ----
    fn exec_new_instance(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let parts: Vec<&str> = ops.splitn(2, ',').map(str::trim).collect();
        if parts.len() < 2 {
            self.pc += 1;
            return Ok(format!("new-instance (parse error: {})", ops));
        }
        let dst = parse_reg(parts[0])?;
        let class = parts[1].to_string();
        let heap_idx = self.alloc(HeapObject {
            kind: HeapObjectKind::Instance {
                class: class.clone(),
                fields: std::collections::HashMap::new(),
            },
        });
        self.set_reg(dst, Value::Ref(heap_idx))?;
        self.pc += 1;
        Ok(format!("v{} = new {} → @{}", dst, class, heap_idx))
    }

    fn exec_sget(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let parts: Vec<&str> = ops.splitn(2, ',').map(str::trim).collect();
        if parts.len() < 2 {
            self.pc += 1;
            return Ok(format!("sget (parse error: {})", ops));
        }
        let dst = parse_reg(parts[0])?;
        let field = parts[1].trim().to_string();
        self.set_reg(dst, Value::Unknown(format!("sget {}", field)))?;
        self.pc += 1;
        Ok(format!("v{} = {}", dst, field))
    }

    fn exec_iget(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let parts: Vec<&str> = ops.splitn(3, ',').map(str::trim).collect();
        if parts.len() < 3 {
            self.pc += 1;
            return Ok(format!("iget (parse error: {})", ops));
        }
        let dst = parse_reg(parts[0])?;
        let _obj_reg = parse_reg(parts[1])?;
        let field = parts[2].trim().to_string();
        self.set_reg(dst, Value::Unknown(format!("iget {}", field)))?;
        self.pc += 1;
        Ok(format!("v{} = .{}", dst, field))
    }

    fn exec_invoke(&mut self, ops: &str) -> Result<String, EmulatorError> {
        let (arg_regs, method_sig) = parse_invoke_operands(ops);

        if let Some(stub_result) = super::stubs::try_stub(self, &method_sig, &arg_regs) {
            self.last_invoke_result = stub_result.result;
            if let Some(line) = stub_result.console_line {
                self.console_output.push(line);
            }
            self.pc += 1;
            return Ok(stub_result.description);
        }

        self.last_invoke_result = Some(Value::Unknown(format!("result of {}", method_sig)));
        self.pc += 1;
        Ok(format!("invoke {} (no stub)", method_sig))
    }

    fn exec_compare_or_stub(
        &mut self,
        mnemonic: &str,
        ops: &str,
        _resolved: &str,
    ) -> Result<String, EmulatorError> {
        match mnemonic {
            "cmp-long" | "cmpl-float" | "cmpg-float" | "cmpl-double" | "cmpg-double" => {
                let (dst, a, b) = parse_three_regs_str(ops)?;
                let result = match mnemonic {
                    "cmp-long" => {
                        let va = self.get_reg(a)?.as_long().unwrap_or(0);
                        let vb = self.get_reg(b)?.as_long().unwrap_or(0);
                        va.cmp(&vb) as i32
                    }
                    "cmpl-float" | "cmpg-float" => {
                        let va = get_float(&self.registers, a);
                        let vb = get_float(&self.registers, b);
                        if va < vb {
                            -1
                        } else if va > vb {
                            1
                        } else {
                            0
                        }
                    }
                    "cmpl-double" | "cmpg-double" => {
                        let va = get_double(&self.registers, a);
                        let vb = get_double(&self.registers, b);
                        if va < vb {
                            -1
                        } else if va > vb {
                            1
                        } else {
                            0
                        }
                    }
                    _ => 0,
                };
                self.set_reg(dst, Value::Int(result))?;
                self.pc += 1;
                Ok(format!("v{} = cmp(v{}, v{}) = {}", dst, a, b, result))
            }
            _ => {
                self.pc += 1;
                Ok(format!("{} {} (stubbed)", mnemonic, ops))
            }
        }
    }
}

// ---- helpers ----

fn parse_reg(s: &str) -> Result<u32, EmulatorError> {
    let s = s.trim();
    s.strip_prefix('v')
        .and_then(|n| n.parse().ok())
        .ok_or(EmulatorError::Unsupported(
            0,
            format!("expected register, got '{}'", s),
        ))
}

fn parse_single_reg(ops: &str) -> Result<u32, EmulatorError> {
    let s = ops.split(',').next().unwrap_or(ops).trim();
    parse_reg(s)
}

fn parse_two_regs(ops: &str) -> Result<(u32, u32), EmulatorError> {
    let parts: Vec<&str> = ops.split(',').map(str::trim).collect();
    if parts.len() < 2 {
        return Err(EmulatorError::Unsupported(
            0,
            format!("expected 2 regs: '{}'", ops),
        ));
    }
    Ok((parse_reg(parts[0])?, parse_reg(parts[1])?))
}

fn parse_three_regs_str(ops: &str) -> Result<(u32, u32, u32), EmulatorError> {
    let parts: Vec<&str> = ops.split(',').map(str::trim).collect();
    if parts.len() < 3 {
        return Err(EmulatorError::Unsupported(
            0,
            format!("expected 3 regs: '{}'", ops),
        ));
    }
    Ok((
        parse_reg(parts[0])?,
        parse_reg(parts[1])?,
        parse_reg(parts[2])?,
    ))
}

fn parse_literal(s: &str) -> i64 {
    let s = s.trim();
    // 0x / 0X prefix
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return i64::from_str_radix(hex, 16).unwrap_or(0);
    }
    if let Some(hex) = s.strip_prefix("-0x").or_else(|| s.strip_prefix("-0X")) {
        return -i64::from_str_radix(hex, 16).unwrap_or(0);
    }
    // +NNh / -NNh or +0xNN / -0xNN (disassembler-style hex)
    let rest = s.strip_prefix('+').unwrap_or(s);
    let (negative, rest) = if let Some(r) = rest.strip_prefix('-') {
        (true, r)
    } else {
        (false, rest)
    };
    let rest = rest.trim_end_matches('h').trim_end_matches('H');
    if let Ok(n) = i64::from_str_radix(rest, 16) {
        return if negative { -n } else { n };
    }
    s.parse().unwrap_or(0)
}

/// Dalvik branch/goto offsets are signed; the disassembler may emit them as unsigned hex.
/// Convert raw parsed value to the correct signed code-unit offset for the given mnemonic.
fn branch_offset_signed(mnemonic: &str, raw: i64) -> i64 {
    match mnemonic {
        "goto" => {
            // 10t: 8-bit signed
            if (128..=255).contains(&raw) {
                (raw as i8) as i64
            } else {
                raw
            }
        }
        "goto/16" => {
            // 20t: 16-bit signed
            if (32768..=65535).contains(&raw) {
                raw - 65536
            } else {
                raw
            }
        }
        "goto/32" => {
            // 30t: 32-bit signed
            if raw >= 0x8000_0000 && raw <= 0xFFFF_FFFF {
                (raw as i32) as i64
            } else {
                raw
            }
        }
        _ => {
            // if-eq, if-ne, if-eqz, etc.: 21t/22t use 16-bit signed
            if (32768..=65535).contains(&raw) {
                raw - 65536
            } else {
                raw
            }
        }
    }
}

fn parse_reg_and_literal(ops: &str) -> Result<(u32, i64), EmulatorError> {
    let parts: Vec<&str> = ops.splitn(2, ',').map(str::trim).collect();
    if parts.len() < 2 {
        return Err(EmulatorError::Unsupported(
            0,
            format!("expected reg,lit: '{}'", ops),
        ));
    }
    Ok((parse_reg(parts[0])?, parse_literal(parts[1])))
}

fn parse_two_regs_and_literal(ops: &str) -> Result<(u32, u32, i64), EmulatorError> {
    let parts: Vec<&str> = ops.splitn(3, ',').map(str::trim).collect();
    if parts.len() < 3 {
        return Err(EmulatorError::Unsupported(
            0,
            format!("expected reg,reg,lit: '{}'", ops),
        ));
    }
    Ok((
        parse_reg(parts[0])?,
        parse_reg(parts[1])?,
        parse_literal(parts[2]),
    ))
}

fn op_symbol(m: &str) -> &str {
    match m {
        "add-int" | "add-long" | "add-float" | "add-double" => "+",
        "sub-int" | "sub-long" | "sub-float" | "sub-double" => "-",
        "mul-int" | "mul-long" | "mul-float" | "mul-double" => "*",
        "div-int" | "div-long" | "div-float" | "div-double" => "/",
        "rem-int" | "rem-long" | "rem-float" | "rem-double" => "%",
        "and-int" | "and-long" => "&",
        "or-int" | "or-long" => "|",
        "xor-int" | "xor-long" => "^",
        "shl-int" => "<<",
        "shr-int" => ">>",
        "ushr-int" => ">>>",
        "rsub-int" => "rsub",
        _ => "?",
    }
}

fn cond_symbol(m: &str) -> &str {
    match m {
        "if-eq" | "if-eqz" => "==",
        "if-ne" | "if-nez" => "!=",
        "if-lt" | "if-ltz" => "<",
        "if-ge" | "if-gez" => ">=",
        "if-gt" | "if-gtz" => ">",
        "if-le" | "if-lez" => "<=",
        _ => "?",
    }
}

fn long_op(base: &str, a: i64, b: i64, pc: usize) -> Result<i64, EmulatorError> {
    Ok(match base {
        "add-int" => a.wrapping_add(b),
        "sub-int" => a.wrapping_sub(b),
        "mul-int" => a.wrapping_mul(b),
        "div-int" => {
            if b == 0 {
                return Err(EmulatorError::DivisionByZero(pc));
            }
            a.wrapping_div(b)
        }
        "rem-int" => {
            if b == 0 {
                return Err(EmulatorError::DivisionByZero(pc));
            }
            a.wrapping_rem(b)
        }
        "and-int" => a & b,
        "or-int" => a | b,
        "xor-int" => a ^ b,
        _ => a,
    })
}

fn get_float(regs: &[Value], r: u32) -> f32 {
    match regs.get(r as usize) {
        Some(Value::Float(v)) => *v,
        Some(Value::Int(v)) => *v as f32,
        _ => 0.0,
    }
}

fn get_double(regs: &[Value], r: u32) -> f64 {
    match regs.get(r as usize) {
        Some(Value::Double(v)) => *v,
        Some(Value::Long(v)) => *v as f64,
        _ => 0.0,
    }
}

fn float_op(m: &str, a: f32, b: f32) -> f32 {
    let base = m.split('/').next().unwrap_or(m);
    match base {
        "add-float" => a + b,
        "sub-float" => a - b,
        "mul-float" => a * b,
        "div-float" => a / b,
        "rem-float" => a % b,
        _ => a,
    }
}

fn double_op(m: &str, a: f64, b: f64) -> f64 {
    let base = m.split('/').next().unwrap_or(m);
    match base {
        "add-double" => a + b,
        "sub-double" => a - b,
        "mul-double" => a * b,
        "div-double" => a / b,
        "rem-double" => a % b,
        _ => a,
    }
}

/// Parse resolved invoke operands like "v2, v3, java.io.PrintStream.println(java.lang.String)"
/// into register list and method signature.
/// Registers are comma-separated `vN` tokens at the start; the method ref follows and may
/// itself contain commas inside its `(...)` parameter list.
fn parse_invoke_operands(ops: &str) -> (Vec<u32>, String) {
    let parts: Vec<&str> = ops.split(',').map(str::trim).collect();
    let mut regs = Vec::new();
    let mut method_start_idx = parts.len();
    for (i, part) in parts.iter().enumerate() {
        if part.starts_with('v') {
            if let Ok(r) = parse_reg(part) {
                regs.push(r);
                continue;
            }
        }
        method_start_idx = i;
        break;
    }
    let method_sig = parts[method_start_idx..].join(", ");
    (regs, method_sig)
}
