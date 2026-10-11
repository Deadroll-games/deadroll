//! C ABI over `deadroll-combat` for the website: replay a hit from the VRF seed in a
//! `HitResolved` event, or preview damage. See `README.md` next to this crate for the protocol.
//!
//! Static buffers, no imports. The page writes a borsh `HitInput` into the input buffer and
//! the 32-byte seed into the seed buffer, calls an export, and reads borsh bytes back from the
//! output buffer. Balance = `Balance::DEFAULT`.
//!
//! Built with `std`: `deadroll-types` pulls `solana-pubkey`, whose `atomic` feature links `std`
//! (`parking_lot`) on non-SBF targets, so `no_std` is not possible here. std on wasm32 aborts on
//! panic and brings its own allocator.

use borsh::{BorshDeserialize, BorshSerialize};
use deadroll_combat::{preview_range, resolve_hit, Balance, HitInput, VrfRoll};

/// Bumped on any change of the exports or the byte layouts.
pub const ABI_VERSION: u32 = 1;
/// Size of the input and output buffers.
pub const BUFFER_LEN: usize = 4096;

/// Export error codes (negative return values).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Error {
    /// `input_len` is larger than the input buffer.
    InputTooLong = -1,
    /// The input is not a valid borsh `HitInput` (or has trailing bytes).
    BadInput = -2,
    /// The result does not fit the output buffer.
    OutputTooSmall = -3,
}

/// Decodes a borsh `HitInput`, resolves it with `seed`, writes a borsh `HitResult` to `out`.
/// Returns the number of bytes written.
pub fn resolve_bytes(input: &[u8], seed: &[u8; 32], out: &mut [u8]) -> Result<usize, Error> {
    let input = HitInput::try_from_slice(input).map_err(|_| Error::BadInput)?;
    let result = resolve_hit(&input, &Balance::DEFAULT, &mut VrfRoll::new(*seed));
    write(&result, out)
}

/// Decodes a borsh `HitInput`, writes `min: u32 LE, max: u32 LE` (8 bytes) to `out`.
pub fn preview_bytes(input: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let input = HitInput::try_from_slice(input).map_err(|_| Error::BadInput)?;
    let (min, max) = preview_range(&input, &Balance::DEFAULT);
    write(&(min, max), out)
}

fn write(value: &impl BorshSerialize, out: &mut [u8]) -> Result<usize, Error> {
    let total = out.len();
    let mut cursor = out;
    value
        .serialize(&mut cursor)
        .map_err(|_| Error::OutputTooSmall)?;
    Ok(total - cursor.len())
}

fn code(result: Result<usize, Error>) -> i32 {
    match result {
        Ok(len) => len as i32,
        Err(e) => e as i32,
    }
}

mod abi {
    use core::cell::UnsafeCell;

    use super::*;

    struct Buffers {
        input: [u8; BUFFER_LEN],
        output: [u8; BUFFER_LEN],
        seed: [u8; 32],
    }

    /// wasm32 runs one thread: the exports never overlap.
    struct Shared(UnsafeCell<Buffers>);
    unsafe impl Sync for Shared {}

    static BUFFERS: Shared = Shared(UnsafeCell::new(Buffers {
        input: [0; BUFFER_LEN],
        output: [0; BUFFER_LEN],
        seed: [0; 32],
    }));

    fn buffers() -> *mut Buffers {
        BUFFERS.0.get()
    }

    #[no_mangle]
    pub extern "C" fn abi_version() -> u32 {
        ABI_VERSION
    }

    #[no_mangle]
    pub extern "C" fn buffer_len() -> u32 {
        BUFFER_LEN as u32
    }

    #[no_mangle]
    pub extern "C" fn input_ptr() -> *mut u8 {
        // SAFETY: only an address is taken; no reference outlives this call.
        unsafe { (*buffers()).input.as_mut_ptr() }
    }

    #[no_mangle]
    pub extern "C" fn output_ptr() -> *const u8 {
        // SAFETY: as above.
        unsafe { (*buffers()).output.as_ptr() }
    }

    #[no_mangle]
    pub extern "C" fn seed_ptr() -> *mut u8 {
        // SAFETY: as above.
        unsafe { (*buffers()).seed.as_mut_ptr() }
    }

    /// Resolves `input[..input_len]` with the seed buffer. Returns the output length or an
    /// [`Error`] code.
    #[no_mangle]
    pub extern "C" fn resolve(input_len: u32) -> i32 {
        // SAFETY: single-threaded, no other borrow of the buffers is alive during the call.
        let b = unsafe { &mut *buffers() };
        let Some(input) = b.input.get(..input_len as usize) else {
            return Error::InputTooLong as i32;
        };
        code(resolve_bytes(input, &b.seed, &mut b.output))
    }

    /// Damage range without dice: writes `min, max` as two `u32` LE. Returns 8 or an error.
    #[no_mangle]
    pub extern "C" fn preview(input_len: u32) -> i32 {
        // SAFETY: as in `resolve`.
        let b = unsafe { &mut *buffers() };
        let Some(input) = b.input.get(..input_len as usize) else {
            return Error::InputTooLong as i32;
        };
        code(preview_bytes(input, &mut b.output))
    }

    /// Roll number `index` of the seed buffer (`VrfRoll::roll_at`), for replay checks.
    #[no_mangle]
    pub extern "C" fn roll_u64(index: u32) -> u64 {
        // SAFETY: as in `resolve`.
        let seed = unsafe { &(*buffers()).seed };
        VrfRoll::roll_at(seed, index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadroll_combat::{Carrier, DamageRange, HitResult, Skill, Tint};
    use deadroll_types::{Aspect, Form, Source};

    fn input() -> HitInput {
        let mut input = HitInput::default();
        input.attacker.weapon = DamageRange::new(Aspect::Piercing, 4, 8);
        input.attacker.skill = Skill::basic(Source::Combat, Form::Strike, 0);
        input.attacker.tints[0] = Tint::new(Aspect::Cold, 3, Carrier::Race);
        input.defender.defense = 8;
        input.defender.armor = 15;
        input
    }

    #[test]
    fn resolve_round_trip_matches_direct_call() {
        let input = input();
        let bytes = borsh::to_vec(&input).unwrap();
        assert!(bytes.len() < BUFFER_LEN);
        let mut out = [0u8; BUFFER_LEN];
        for s in 0u8..32 {
            let seed = [s; 32];
            let len = resolve_bytes(&bytes, &seed, &mut out).unwrap();
            let decoded = HitResult::try_from_slice(&out[..len]).unwrap();
            let direct = resolve_hit(&input, &Balance::DEFAULT, &mut VrfRoll::new(seed));
            assert_eq!(decoded, direct);
        }
    }

    #[test]
    fn preview_bytes_layout() {
        let bytes = borsh::to_vec(&input()).unwrap();
        let mut out = [0u8; 16];
        assert_eq!(preview_bytes(&bytes, &mut out), Ok(8));
        let (min, max) = preview_range(&input(), &Balance::DEFAULT);
        assert_eq!(out[..4], min.to_le_bytes());
        assert_eq!(out[4..8], max.to_le_bytes());
    }

    #[test]
    fn errors() {
        let bytes = borsh::to_vec(&input()).unwrap();
        let mut out = [0u8; BUFFER_LEN];
        assert_eq!(
            resolve_bytes(&bytes[..10], &[0; 32], &mut out),
            Err(Error::BadInput)
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            resolve_bytes(&trailing, &[0; 32], &mut out),
            Err(Error::BadInput)
        );
        assert_eq!(
            resolve_bytes(&bytes, &[0; 32], &mut out[..16]),
            Err(Error::OutputTooSmall)
        );
        assert_eq!(code(Err(Error::InputTooLong)), -1);
    }

    #[test]
    fn exports_on_host() {
        let bytes = borsh::to_vec(&input()).unwrap();
        // SAFETY: test-only access to the export buffers; this is the only test using them.
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), abi::input_ptr(), bytes.len());
            core::ptr::write_bytes(abi::seed_ptr(), 7, 32);
        }
        let len = abi::resolve(bytes.len() as u32);
        assert!(len > 0);
        let out = unsafe { core::slice::from_raw_parts(abi::output_ptr(), len as usize) };
        let decoded = HitResult::try_from_slice(out).unwrap();
        assert_eq!(
            decoded,
            resolve_hit(&input(), &Balance::DEFAULT, &mut VrfRoll::new([7; 32]))
        );
        assert_eq!(abi::roll_u64(0), VrfRoll::roll_at(&[7; 32], 0));
        assert_eq!(
            abi::resolve(BUFFER_LEN as u32 + 1),
            Error::InputTooLong as i32
        );
        assert_eq!(abi::abi_version(), ABI_VERSION);
        assert_eq!(abi::buffer_len(), BUFFER_LEN as u32);
    }
}
