//! Deadroll skill checks (docs/SKILL_BUILDER.md).
//!
//! One crate for the builder on the website and the character program, so the limits cannot
//! be bypassed by going around the website (SKILL_BUILDER.md, section 1.4).
//!
//! - [`SkillBlocks`]: the mechanics of a skill, only blocks (category, purpose, tags, aspects,
//!   effect, target, trigger, aura). Name and description are text and live in the program.
//! - [`validate`]: the builder rules of SKILL_BUILDER.md, section 13.
//! - [`validate_starting`]: the class limits on the first six skills (section 10.1).
//! - [`check_loadout`]: at most 4 active, 1 reactive, 1 passive (section 3.4).
//! - [`check_map_selection`]: the skills taken onto a map, 4 + 1 + 1 (sections 3.4, 13).
//! - [`derive`]: shares, power, fusion and cooldown, computed once at creation and stored.
//! - [`presets`]: starter sets per class for the demo.
//!
//! Integers only, no allocation, `no_std` without the `std`/`anchor` features.
#![cfg_attr(not(feature = "std"), no_std)]

/// Applies the serialization derives for the active feature set to every item inside.
macro_rules! shared {
    ($($item:item)*) => {$(
        #[cfg_attr(
            feature = "anchor",
            derive(
                anchor_lang::prelude::AnchorSerialize,
                anchor_lang::prelude::AnchorDeserialize,
                anchor_lang::prelude::InitSpace
            )
        )]
        #[cfg_attr(not(feature = "anchor"), derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
        $item
    )*};
}

/// A `#[repr(u8)]` enum with sequential discriminants from 0, `TryFrom<u8>` and `ALL`.
/// Discriminants are part of the on-chain format: append only.
macro_rules! u8_enum {
    ($(#[$meta:meta])* $name:ident { $first:ident $(, $rest:ident)* $(,)? }) => {
        shared! {
            $(#[$meta])*
            #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
            #[repr(u8)]
            pub enum $name {
                #[default]
                $first,
                $($rest,)*
            }
        }

        impl $name {
            /// Every variant in discriminant order.
            pub const ALL: &'static [$name] = &[$name::$first, $($name::$rest,)*];
            pub const COUNT: usize = Self::ALL.len();
        }

        impl TryFrom<u8> for $name {
            type Error = deadroll_types::InvalidValue;

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                Self::ALL.get(value as usize).copied().ok_or(deadroll_types::InvalidValue(value))
            }
        }
    };
}

pub mod blocks;
pub mod derive;
pub mod presets;
pub mod rules;

pub use blocks::*;
pub use derive::{derive, derive_with, fusion_id, Derived, SkillTuning};
pub use rules::{
    check_loadout, check_map_selection, validate, validate_starting, MapSelection, SkillError,
    ACTIVE_MAX, AURA_RADIUS_MAX, NO_SKILL, PASSIVE_MAX, REACTIVE_MAX, SELECTION_SLOTS,
    STARTING_MAX,
};
