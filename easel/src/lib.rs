//! # easel — a drawing that can be edited and saved
//!
//! The editor, without the window. Everything here is arithmetic on a pointer
//! position and a list of marks, so all of it is tested in the dark — and a
//! second program could put a different window round it without touching any
//! of this.
//!
//! Same split as `world` and `live`.

pub mod board;
pub mod history;
pub mod ink;
pub mod mark;
pub mod sheet;

pub use board::{Board, Tool};
pub use history::History;
pub use ink::Ink;
pub use mark::Mark;
pub use sheet::Sheet;
