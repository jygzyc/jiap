//! Emulator state: values, registers, heap, and snapshot types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    Unset,
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Null,
    /// Heap reference (index into Emulator.heap).
    Ref(usize),
    /// String value (resolved from const-string).
    Str(String),
    /// Unknown value with a textual description (e.g. static field, method result).
    Unknown(String),
}

impl Value {
    pub fn as_int(&self) -> Option<i32> {
        match self {
            Value::Int(v) => Some(*v),
            _ => None,
        }
    }
    pub fn as_long(&self) -> Option<i64> {
        match self {
            Value::Long(v) => Some(*v),
            _ => None,
        }
    }
    pub fn display_short(&self) -> String {
        match self {
            Value::Unset => "?".into(),
            Value::Int(v) => format!("{}", v),
            Value::Long(v) => format!("{}L", v),
            Value::Float(v) => format!("{}f", v),
            Value::Double(v) => format!("{}d", v),
            Value::Null => "null".into(),
            Value::Ref(idx) => format!("@{}", idx),
            Value::Str(s) => format!("\"{}\"", s),
            Value::Unknown(s) => format!("<{}>", s),
        }
    }

    /// Like `display_short` but integers are shown in hex (for VM state display).
    pub fn display_short_hex(&self) -> String {
        match self {
            Value::Unset => "?".into(),
            Value::Int(v) => format!("0x{:08x}", (*v as u32)),
            Value::Long(v) => format!("0x{:016x}L", (*v as u64)),
            Value::Float(v) => format!("{}f", v),
            Value::Double(v) => format!("{}d", v),
            Value::Null => "null".into(),
            Value::Ref(idx) => format!("@{}", idx),
            Value::Str(s) => format!("\"{}\"", s),
            Value::Unknown(s) => format!("<{}>", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeapObject {
    pub kind: HeapObjectKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HeapObjectKind {
    Array {
        element_type: String,
        values: Vec<Value>,
    },
    Instance {
        class: String,
        fields: HashMap<String, Value>,
    },
}

/// A decoded instruction with offset and disassembly for the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionInfo {
    pub index: usize,
    pub offset: u32,
    pub mnemonic: String,
    pub operands: String,
}

/// Full snapshot of emulator state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub pc: usize,
    pub registers: Vec<RegisterInfo>,
    pub heap: Vec<HeapSnapshot>,
    pub finished: bool,
    pub exception: Option<String>,
    pub return_value: Option<Value>,
    pub step_count: usize,
    pub console_output: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterInfo {
    pub index: usize,
    pub name: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeapSnapshot {
    pub index: usize,
    pub object: HeapObjectKind,
}

/// History entry: one step of execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRecord {
    pub step: usize,
    pub pc_before: usize,
    pub instruction: InstructionInfo,
    pub state_after: StateSnapshot,
    pub description: String,
}

/// Result of a single step, for use by callers that drive the emulator step-by-step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_count: usize,
    pub instruction: InstructionInfo,
    pub state_after: StateSnapshot,
    pub description: String,
    pub finished: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum EmulatorError {
    #[error("division by zero at pc={0}")]
    DivisionByZero(usize),
    #[error("null reference at pc={0}")]
    NullReference(usize),
    #[error("array index out of bounds at pc={0}: index={1}, length={2}")]
    ArrayIndexOutOfBounds(usize, i32, usize),
    #[error("unsupported instruction at pc={0}: {1}")]
    Unsupported(usize, String),
    #[error("invalid register at pc={0}: v{1}")]
    InvalidRegister(usize, u32),
    #[error("type error at pc={0}: {1}")]
    TypeError(usize, String),
    #[error("execution limit reached: {0} steps")]
    ExecutionLimit(usize),
}

pub struct Emulator {
    pub registers: Vec<Value>,
    pub heap: Vec<HeapObject>,
    pub pc: usize,
    pub finished: bool,
    pub exception: Option<String>,
    pub return_value: Option<Value>,
    pub step_count: usize,
    pub history: Vec<StepRecord>,
    pub instructions: Vec<InstructionInfo>,
    /// Map from instruction byte-offset to index in `instructions`.
    pub offset_to_index: HashMap<u32, usize>,
    pub registers_size: u32,
    pub ins_size: u32,
    pub is_static: bool,
    /// Resolved operands (with string/type/field/method names instead of indices).
    pub resolved_operands: Vec<String>,
    /// Maximum steps before forced stop.
    pub max_steps: usize,
    /// Result from the last invoke, consumed by the next `move-result*`.
    pub last_invoke_result: Option<Value>,
    /// Captured console output (from System.out.println, Log.d, etc.).
    pub console_output: Vec<String>,
}

impl Emulator {
    pub fn new(
        instructions: Vec<InstructionInfo>,
        resolved_operands: Vec<String>,
        registers_size: u32,
        ins_size: u32,
        is_static: bool,
        params: Vec<Value>,
    ) -> Self {
        Self::new_with_heap(
            instructions,
            resolved_operands,
            registers_size,
            ins_size,
            is_static,
            params,
            Vec::new(),
        )
    }

    /// Same as `new` but starts with pre-allocated heap objects (e.g. for array params).
    /// Params may contain `Value::Ref(i)` for indices 0..heap.len().
    pub fn new_with_heap(
        instructions: Vec<InstructionInfo>,
        resolved_operands: Vec<String>,
        registers_size: u32,
        ins_size: u32,
        is_static: bool,
        params: Vec<Value>,
        initial_heap: Vec<HeapObject>,
    ) -> Self {
        let mut registers = vec![Value::Unset; registers_size as usize];
        let param_base = registers_size.saturating_sub(ins_size) as usize;
        for (i, val) in params.into_iter().enumerate() {
            if param_base + i < registers.len() {
                registers[param_base + i] = val;
            }
        }

        let offset_to_index: HashMap<u32, usize> = instructions
            .iter()
            .enumerate()
            .map(|(i, ins)| (ins.offset, i))
            .collect();

        Emulator {
            registers,
            heap: initial_heap,
            pc: 0,
            finished: false,
            exception: None,
            return_value: None,
            step_count: 0,
            history: Vec::new(),
            instructions,
            offset_to_index,
            registers_size,
            ins_size,
            is_static,
            resolved_operands,
            max_steps: 10_000,
            last_invoke_result: None,
            console_output: Vec::new(),
        }
    }

    pub fn snapshot(&self) -> StateSnapshot {
        let param_base = self.registers_size.saturating_sub(self.ins_size) as usize;
        let registers = self
            .registers
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let name = if i >= param_base {
                    let pidx = i - param_base;
                    if !self.is_static && pidx == 0 {
                        "this".into()
                    } else {
                        let display = if self.is_static { pidx } else { pidx - 1 };
                        format!("p{}", display)
                    }
                } else {
                    format!("v{}", i)
                };
                RegisterInfo {
                    index: i,
                    name,
                    value: v.clone(),
                }
            })
            .collect();

        let heap = self
            .heap
            .iter()
            .enumerate()
            .map(|(i, h)| HeapSnapshot {
                index: i,
                object: h.kind.clone(),
            })
            .collect();

        StateSnapshot {
            pc: self.pc,
            registers,
            heap,
            finished: self.finished,
            exception: self.exception.clone(),
            return_value: self.return_value.clone(),
            step_count: self.step_count,
            console_output: self.console_output.clone(),
        }
    }

    pub fn reset(&mut self, params: Vec<Value>) {
        let param_base = self.registers_size.saturating_sub(self.ins_size) as usize;
        for r in self.registers.iter_mut() {
            *r = Value::Unset;
        }
        for (i, val) in params.into_iter().enumerate() {
            if param_base + i < self.registers.len() {
                self.registers[param_base + i] = val;
            }
        }
        self.heap.clear();
        self.pc = 0;
        self.finished = false;
        self.exception = None;
        self.return_value = None;
        self.step_count = 0;
        self.history.clear();
        self.last_invoke_result = None;
        self.console_output.clear();
    }

    pub(crate) fn get_reg(&self, reg: u32) -> Result<&Value, EmulatorError> {
        self.registers
            .get(reg as usize)
            .ok_or(EmulatorError::InvalidRegister(self.pc, reg))
    }

    pub(crate) fn set_reg(&mut self, reg: u32, val: Value) -> Result<(), EmulatorError> {
        let idx = reg as usize;
        if idx >= self.registers.len() {
            return Err(EmulatorError::InvalidRegister(self.pc, reg));
        }
        self.registers[idx] = val;
        Ok(())
    }

    pub(crate) fn alloc(&mut self, obj: HeapObject) -> usize {
        let idx = self.heap.len();
        self.heap.push(obj);
        idx
    }
}
