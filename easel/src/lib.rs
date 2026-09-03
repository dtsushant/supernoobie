//! # easel — a drawing that can be edited and saved
//!
//! The editor, without the window. Everything here is arithmetic on a pointer
//! position and a list of marks, so all of it is tested in the dark — and a
//! second program could put a different window round it without touching any
//! of this.
//!
//! Same split as `world` and `live`.

pub mod action;
pub mod bar;
pub mod board;
pub mod history;
pub mod ink;
pub mod mark;
pub mod rule;
pub mod script;
pub mod sheet;
pub mod track;
pub mod tree;
pub mod wire;

pub use action::{Act, Action, Step};
pub use bar::{Bar, Button, Cmd};
pub use board::{Board, Tool};
pub use history::History;
pub use ink::Ink;
pub use mark::Mark;
pub use rule::{Rule, Tally};
pub use script::{Row, Script};
pub use sheet::Sheet;
pub use track::{Ease, Key, Track};
pub use tree::{Node, Poke, Tree};
pub use wire::{scene, Look};
