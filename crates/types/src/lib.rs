//! Deadroll shared types (agents/CONTRACTS.md, sections 2-4).
//!
//! Frozen after L0: changes only through a lead-lane PR.
//!
//! Without features the crate is `no_std` and uses plain borsh. With `anchor` the same types
//! derive `AnchorSerialize`/`AnchorDeserialize`/`InitSpace`, so programs can embed them in
//! `#[account]` structs; `idl-build` also puts them into the program IDLs.
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
            /// Borsh size in bytes.
            pub const SIZE: usize = 1;
        }

        impl TryFrom<u8> for $name {
            type Error = $crate::InvalidValue;

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                Self::ALL.get(value as usize).copied().ok_or($crate::InvalidValue(value))
            }
        }

        impl From<$name> for u8 {
            fn from(value: $name) -> u8 {
                value as u8
            }
        }
    };
}

pub mod accounts;
pub mod consts;
pub mod enums;
pub mod render;
pub mod seeds;
// Not glob-exported: `Target`, `Effect` and `Category` are too generic for the crate root.
pub mod skill;

pub use accounts::*;
pub use consts::*;
pub use enums::*;
pub use render::*;
pub use solana_pubkey::Pubkey;

/// A raw byte that does not map to any variant (or is out of range for a bounded value).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidValue(pub u8);

impl core::fmt::Display for InvalidValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid value {}", self.0)
    }
}
