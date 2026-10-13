//! MagicBlock VRF seam (CLAUDE.md hard rule 5): one VRF value per action, every roll of the
//! action derived from it.
//!
//! `act` / `monster_act` call [`request`]; the VRF program later calls `consume_randomness` with
//! [`identity`] as the first signer and the accounts from [`callback_metas`] after it.
//!
//! `mock-vrf` (localnet only, agents/TASKS.md decision block, rule 2): the shared validator has no
//! VRF oracle, so the request is skipped and the test signs `consume_randomness` itself with
//! [`MOCK_VRF_IDENTITY`]. The callback body is the same code path.
use anchor_lang::prelude::*;
use deadroll_combat::RollSource;
use ephemeral_vrf_sdk::types::SerializableAccountMeta;

/// `scoped_vrf_identity(&crate::ID)`: `PDA(["identity", session program], VRF program)`, the
/// signer of every real callback (checked against the SDK in the tests).
pub const VRF_IDENTITY: Pubkey = pubkey!("Bj7XBktwDmAg6J4FohS6oRHbevD6MMd8jnCZKvzKPiKV");

/// Public key of `Keypair.fromSeed(sha256("deadroll:mock-vrf"))`. Not a secret: it is accepted
/// only by `mock-vrf` builds, which never go to devnet.
pub const MOCK_VRF_IDENTITY: Pubkey = pubkey!("5RotSMue7vx9xBPp3L8iQEKbKSc3ezbY4GqiJxDEe6sN");

/// Our `["identity"]` PDA, which signs the VRF request (SDK `create_request_randomness_ix`).
pub const PROGRAM_IDENTITY_SEED: &[u8] = b"identity";

/// The session `["authority"]` PDA and its bump, so the callback signs CPI without a bump search.
pub const SESSION_AUTHORITY: Pubkey = pubkey!("3o8dkZ2aXofZQpeTjHHDvWNCahXrfqvY3GodqKtVMomB");
pub const SESSION_AUTHORITY_BUMP: u8 = 254;

/// Registry `GameConfig` (`["config"]`), forwarded to the character program.
pub const GAME_CONFIG: Pubkey = pubkey!("HYGYnuyssMfrHr2gT3AC3hkQpD6xt37a5hrCc2Z97JHN");

pub fn vrf_program_id() -> Pubkey {
    Pubkey::new_from_array(ephemeral_vrf_sdk::consts::VRF_PROGRAM_ID.to_bytes())
}

/// Oracle queue for requests (MagicBlock's default devnet queue).
pub fn oracle_queue() -> Pubkey {
    Pubkey::new_from_array(ephemeral_vrf_sdk::consts::DEFAULT_QUEUE.to_bytes())
}

pub fn slot_hashes() -> Pubkey {
    Pubkey::new_from_array(ephemeral_vrf_sdk::compat::slot_hashes::ID.to_bytes())
}

/// The signer `consume_randomness` accepts.
pub fn identity() -> Pubkey {
    #[cfg(feature = "mock-vrf")]
    {
        MOCK_VRF_IDENTITY
    }
    #[cfg(not(feature = "mock-vrf"))]
    {
        VRF_IDENTITY
    }
}

/// The production [`RollSource`]: roll `i` = first 8 bytes (LE) of `sha256(seed ‖ i as u32 LE)`,
/// exactly `deadroll_combat::VrfRoll`, but through the SHA-256 syscall on-chain.
#[derive(Clone, Copy, Debug)]
pub struct SyscallRoll {
    pub seed: [u8; 32],
    pub index: u32,
}

impl SyscallRoll {
    pub const fn new(seed: [u8; 32]) -> Self {
        Self { seed, index: 0 }
    }

    pub const fn at(seed: [u8; 32], index: u32) -> Self {
        Self { seed, index }
    }
}

impl RollSource for SyscallRoll {
    fn next_u64(&mut self) -> u64 {
        let digest = solana_sha256_hasher::hashv(&[&self.seed, &self.index.to_le_bytes()]);
        self.index = self.index.wrapping_add(1);
        let mut first = [0u8; 8];
        first.copy_from_slice(&digest.to_bytes()[..8]);
        u64::from_le_bytes(first)
    }
}

/// Unique per request: `sha256(session ‖ action_counter LE)`.
pub fn caller_seed(session: &Pubkey, action_counter: u32) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[session.as_ref(), &action_counter.to_le_bytes()]).to_bytes()
}

/// The accounts the callback gets after the VRF identity, in `ConsumeRandomness` order.
pub struct CallbackAccounts {
    pub session: Pubkey,
    pub monsters: Pubkey,
    pub participation: Pubkey,
    pub character: Pubkey,
    /// The attacking player's `SkillSet`; any key when a monster attacks (not read).
    pub skill_set: Pubkey,
    /// The ally of a heal or buff that is not the caster (`GameSession.pending_target`).
    pub target_participation: Option<Pubkey>,
}

pub fn callback_metas(a: &CallbackAccounts) -> Vec<SerializableAccountMeta> {
    let meta = |key: &Pubkey, is_writable: bool| SerializableAccountMeta {
        pubkey: ephemeral_vrf_sdk::compat::Pubkey::new_from_array(key.to_bytes()),
        is_signer: false,
        is_writable,
    };
    vec![
        meta(&a.session, true),
        meta(&a.monsters, true),
        meta(&a.participation, true),
        meta(&a.character, true),
        meta(&a.skill_set, false),
        meta(&SESSION_AUTHORITY, false),
        meta(&GAME_CONFIG, false),
        meta(&deadroll_character::ID, false),
        // Anchor reads this program's id as `None`.
        match &a.target_participation {
            Some(key) => meta(key, true),
            None => meta(&crate::ID, false),
        },
    ]
}

/// Accounts of the VRF request CPI.
pub struct RequestAccounts<'a, 'info> {
    /// Pays the VRF fee: the acting wallet or session key (docs/GAME_COMMISSIONS.md, 6.3).
    pub payer: &'a AccountInfo<'info>,
    pub program_identity: &'a AccountInfo<'info>,
    pub oracle_queue: &'a AccountInfo<'info>,
    pub system_program: &'a AccountInfo<'info>,
    pub slot_hashes: &'a AccountInfo<'info>,
}

/// Requests one VRF value whose callback is `consume_randomness` with `callback`.
#[cfg(not(feature = "mock-vrf"))]
pub fn request(
    accounts: RequestAccounts,
    identity_bump: u8,
    caller_seed: [u8; 32],
    callback: &CallbackAccounts,
) -> Result<()> {
    use anchor_lang::{solana_program::program::invoke_signed, Discriminator};
    use ephemeral_vrf_sdk::instructions::{create_request_randomness_ix, RequestRandomnessParams};

    let sdk = |key: &Pubkey| ephemeral_vrf_sdk::compat::Pubkey::new_from_array(key.to_bytes());
    let ix = create_request_randomness_ix(RequestRandomnessParams {
        payer: sdk(accounts.payer.key),
        oracle_queue: sdk(accounts.oracle_queue.key),
        callback_program_id: sdk(&crate::ID),
        callback_discriminator: crate::instruction::ConsumeRandomness::DISCRIMINATOR.to_vec(),
        accounts_metas: Some(callback_metas(callback)),
        caller_seed,
        callback_args: None,
    });
    invoke_signed(
        &ix,
        &[
            accounts.payer.clone(),
            accounts.program_identity.clone(),
            accounts.oracle_queue.clone(),
            accounts.system_program.clone(),
            accounts.slot_hashes.clone(),
        ],
        &[&[PROGRAM_IDENTITY_SEED, &[identity_bump]]],
    )
    .map_err(Into::into)
}

/// `mock-vrf`: no oracle on localnet; the test calls `consume_randomness` itself.
#[cfg(feature = "mock-vrf")]
pub fn request(
    _accounts: RequestAccounts,
    _identity_bump: u8,
    _caller_seed: [u8; 32],
    _callback: &CallbackAccounts,
) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::ToAccountMetas;
    use deadroll_combat::VrfRoll;
    use deadroll_types::seeds;

    #[test]
    fn syscall_roll_is_vrf_roll() {
        for seed_byte in [0u8, 1, 7, 0xAB, 0xFF] {
            let seed = [seed_byte; 32];
            let mut ours = SyscallRoll::new(seed);
            let mut theirs = VrfRoll::new(seed);
            for _ in 0..40 {
                assert_eq!(ours.next_u64(), theirs.next_u64());
            }
            assert_eq!(
                SyscallRoll::at(seed, 1_000).next_u64(),
                VrfRoll::roll_at(&seed, 1_000)
            );
        }
    }

    #[test]
    fn constants_match_their_derivation() {
        let scoped = ephemeral_vrf_sdk::consts::scoped_vrf_identity(
            &ephemeral_vrf_sdk::compat::Pubkey::new_from_array(crate::ID.to_bytes()),
        );
        assert_eq!(VRF_IDENTITY.to_bytes(), scoped.to_bytes());
        assert_eq!(
            Pubkey::find_program_address(&[seeds::AUTHORITY], &crate::ID),
            (SESSION_AUTHORITY, SESSION_AUTHORITY_BUMP)
        );
        assert_eq!(
            Pubkey::find_program_address(&[seeds::CONFIG], &deadroll_registry::ID).0,
            GAME_CONFIG
        );
        assert_ne!(MOCK_VRF_IDENTITY, VRF_IDENTITY);
        #[cfg(not(feature = "mock-vrf"))]
        assert_eq!(identity(), VRF_IDENTITY);
        #[cfg(feature = "mock-vrf")]
        assert_eq!(identity(), MOCK_VRF_IDENTITY);
    }

    #[test]
    fn callback_metas_follow_consume_randomness() {
        let keys: Vec<Pubkey> = (0..6).map(|_| Pubkey::new_unique()).collect();
        for ally in [None, Some(keys[5])] {
            let ours = callback_metas(&CallbackAccounts {
                session: keys[0],
                monsters: keys[1],
                participation: keys[2],
                character: keys[3],
                skill_set: keys[4],
                target_participation: ally,
            });
            let anchors = crate::accounts::ConsumeRandomness {
                vrf_program_identity: VRF_IDENTITY,
                session: keys[0],
                monsters: keys[1],
                participation: keys[2],
                character: keys[3],
                skill_set: keys[4],
                session_authority: SESSION_AUTHORITY,
                config: GAME_CONFIG,
                character_program: deadroll_character::ID,
                target_participation: ally,
            }
            .to_account_metas(None);
            assert_eq!(anchors.len(), ours.len() + 1);
            assert!(anchors[0].is_signer && anchors[0].pubkey == VRF_IDENTITY);
            for (a, o) in anchors[1..].iter().zip(&ours) {
                assert_eq!(a.pubkey.to_bytes(), o.pubkey.to_bytes());
                assert_eq!(a.is_writable, o.is_writable, "{}", a.pubkey);
                assert!(!a.is_signer && !o.is_signer);
            }
            assert_eq!(
                ours.last().unwrap().pubkey.to_bytes(),
                ally.unwrap_or(crate::ID).to_bytes()
            );
        }
    }
}
