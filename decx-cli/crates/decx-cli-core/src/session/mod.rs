//! Independent project manager: tracking, supervision, and monitoring of
//! analysis projects (one DECX server per project).

pub mod lifecycle;
pub mod manager;
pub mod model;
pub mod monitor;
pub mod store;

pub use manager::SessionManager;
pub use model::{ObservedState, Session, SessionEvent, SessionState};
