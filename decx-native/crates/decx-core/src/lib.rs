//! decx-core — pure-std DEX analysis core: parse, index, xref, emit.

pub mod code;
pub mod dex;
pub mod java;
pub mod leb128;
pub mod opcodes;
pub mod project;
pub mod regex;
pub mod sources;
pub mod xref;

pub use dex::Dex;
pub use project::Project;
pub use xref::XrefIndex;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage_dex() {
        assert!(Dex::parse(b"garbage!".to_vec()).is_err());
    }

    #[test]
    fn unsupported_input() {
        assert!(Project::load("x.txt", b"hi".to_vec()).is_err());
    }
}
