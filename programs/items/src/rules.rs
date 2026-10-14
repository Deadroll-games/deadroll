//! The balance table this program runs on (the same one the session program and
//! `tools/balance` use: `deadroll_rules::BalanceConfig`).
use deadroll_rules::BalanceConfig;

pub const BALANCE: &BalanceConfig = &BalanceConfig::DEFAULT;

/// Layout version of the accounts created by this program.
pub const ACCOUNT_VERSION: u8 = 1;
