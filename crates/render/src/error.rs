//! Error codes of `FORMAT.md` section 5. The numeric value is the WASM `last_error()` code.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Error {
    /// Truncated input or trailing bytes.
    Truncated = 1,
    /// Unsupported format version.
    Version = 2,
    /// Canvas size, layer type or reserved flag bits.
    Header = 3,
    /// Unknown opcode.
    Opcode = 4,
    /// Invalid slot for the command.
    Slot = 5,
    /// Argument out of range.
    Arg = 6,
    /// Sprite id not found, or a malformed sprite book.
    Sprite = 7,
    /// Malformed palette, version mismatch or index out of range.
    Palette = 8,
    /// Malformed render request, output mode or scale.
    Request = 9,
    /// More than 256 colors in the image: an indexed PNG cannot hold it.
    Colors = 10,
}

impl Error {
    pub const fn code(self) -> u32 {
        self as u32
    }
}

pub type Result<T> = core::result::Result<T, Error>;
