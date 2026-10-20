use anchor_lang::prelude::*;
use deadroll_registry::{cpi, cpi::accounts};
use deadroll_types::seeds;

/// `registry::set_rasterizer_from_render()` at the end of `finalize_rasterizer`: points
/// `GameConfig` at the rasterizer just finalized (`docs/STORAGE_AND_DELIVERY.md` 3.2). Signed by
/// the render `["authority"]` PDA, which the registry checks against `GameConfig.programs[Render]`.
pub fn set_rasterizer_from_render<'info>(
    render_authority: &AccountInfo<'info>,
    bump: u8,
    config: &AccountInfo<'info>,
    rasterizer: &AccountInfo<'info>,
) -> Result<()> {
    let bump = [bump];
    let signer: &[&[u8]] = &[seeds::AUTHORITY, &bump];
    cpi::set_rasterizer_from_render(CpiContext::new_with_signer(
        deadroll_registry::ID,
        accounts::SetRasterizerFromRender {
            render_authority: render_authority.clone(),
            config: config.clone(),
            blob: rasterizer.clone(),
        },
        &[signer],
    ))
}
