use anchor_lang::prelude::*;
use deadroll_types::ParticipationData;

/// `["participation", session, character]`: one character in one party.
#[account]
#[derive(InitSpace)]
pub struct Participation {
    /// Frozen contract, first after the discriminator.
    pub core: ParticipationData,
    /// Main wallet (character owner) that joined and paid.
    pub player: Pubkey,
    /// Action points granted at the start of each own turn (from speed).
    pub max_ap: u8,
    /// `GameSession.turn_seq` when `core.action_points` was last granted.
    pub ap_seq: u32,
    /// Lamports paid into the escrow, for refunds.
    pub contribution: u64,
}

impl Participation {
    /// Refills action points if this is the first action of a new own turn.
    pub fn refresh_ap(&mut self, turn_seq: u32) {
        if self.ap_seq != turn_seq {
            self.ap_seq = turn_seq;
            self.core.action_points = self.max_ap;
        }
    }
}
