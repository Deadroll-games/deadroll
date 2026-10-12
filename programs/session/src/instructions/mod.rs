// Anchor needs the glob re-exports (generated `__client_accounts_*`); every module has its own `handler`.
#![allow(ambiguous_glob_reexports)]

pub mod act;
pub mod claim_escrow;
pub mod claim_rewards;
pub mod consume_randomness;
pub mod end_turn;
pub mod flee;
pub mod join_session;
pub mod monster_act;
pub mod monster_move;
pub mod move_unit;
pub mod open_session;
pub mod set_ready;
pub mod start_session;

pub use act::*;
pub use claim_escrow::*;
pub use claim_rewards::*;
pub use consume_randomness::*;
pub use end_turn::*;
pub use flee::*;
pub use join_session::*;
pub use monster_act::*;
pub use monster_move::*;
pub use move_unit::*;
pub use open_session::*;
pub use set_ready::*;
pub use start_session::*;
