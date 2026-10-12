use anchor_lang::prelude::*;
use deadroll_types::URI_BASE_LEN;

use super::set_programs::EditConfig;
use crate::events::UriBaseSet;

pub fn handler(ctx: Context<EditConfig>, uri_base: [u8; URI_BASE_LEN]) -> Result<()> {
    ctx.accounts.config.core.uri_base = uri_base;
    emit!(UriBaseSet { uri_base });
    Ok(())
}
