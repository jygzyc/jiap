//! Dalvik bytecode emulator: interprets instructions step-by-step,
//! tracking register values, heap objects, and execution history.

pub mod interpret;
pub mod state;
pub mod stubs;

pub use state::{
    Emulator, EmulatorError, HeapObject, HeapObjectKind, RegisterInfo, StateSnapshot, StepResult,
    Value,
};
