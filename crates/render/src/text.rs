//! Text DSL and palette text compilers (`FORMAT.md` 1 and 3.2.1). Feature `text`, tools only.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::palette;
use crate::sprite::{self, bitmap, layer, op, FLAG_NO_OUTLINE, POLY_MAX, POLY_MIN, RAMP_COLORS};

/// A compile error with a 1-based line number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextError {
    pub line: usize,
    pub msg: String,
}

impl core::fmt::Display for TextError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

type TResult<T> = core::result::Result<T, TextError>;

fn err<T>(line: usize, msg: impl Into<String>) -> TResult<T> {
    Err(TextError {
        line,
        msg: msg.into(),
    })
}

/// One compiled sprite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteSrc {
    pub id: u16,
    pub name: String,
    /// Line of the `sprite` statement.
    pub line: usize,
    /// Binary sprite (`FORMAT.md` 3.1), already validated by the decoder.
    pub bytes: Vec<u8>,
}

/// Slot letter -> value (`FORMAT.md` 1.3).
pub fn slot_value(s: &str) -> Option<u8> {
    Some(match s {
        "_" => 0,
        "O" => 1,
        "P" | "K" => 2,
        "S" | "H" => 3,
        "A" | "Y" => 4,
        "E" | "D" => 5,
        "U" | "R" => 6,
        _ => return None,
    })
}

/// `BITMAP` row character -> pixel value (`FORMAT.md` 1.2). `.` draws nothing, `_` erases,
/// a slot letter is auto shaded, and a digit or lowercase letter is a slot with an explicit
/// ramp color: `0-5` slot 2 (`P`/`K`), `a-f` 3 (`S`/`H`), `g-l` 4 (`A`/`Y`),
/// `m-r` 5 (`E`/`D`), `s-x` 6 (`U`/`R`).
pub fn bitmap_char(c: char) -> Option<u8> {
    let n = RAMP_COLORS;
    Some(match c {
        '.' => bitmap::value(bitmap::SKIP, 0),
        '0'..='5' => bitmap::value(2, 1 + (c as u8 - b'0')),
        'a'..='x' => {
            let i = c as u8 - b'a';
            bitmap::value(3 + i / n, 1 + i % n)
        }
        _ => {
            let mut buf = [0u8; 4];
            bitmap::value(slot_value(c.encode_utf8(&mut buf))?, 0)
        }
    })
}

fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Cuts a comment: a `#` at the start of a token followed by whitespace or the end of the line
/// (`#rrggbb` colors are not comments).
fn strip_comment(l: &str) -> &str {
    let b = l.as_bytes();
    for i in 0..b.len() {
        let starts_token = i == 0 || b[i - 1].is_ascii_whitespace();
        let ends = b.get(i + 1).is_none_or(|c| c.is_ascii_whitespace());
        if b[i] == b'#' && starts_token && ends {
            return &l[..i];
        }
    }
    l
}

fn lines(src: &str) -> impl Iterator<Item = (usize, Vec<&str>)> {
    src.lines().enumerate().filter_map(|(i, l)| {
        let l = strip_comment(l);
        let t: Vec<&str> = l.split_whitespace().collect();
        (!t.is_empty()).then_some((i + 1, t))
    })
}

fn num(line: usize, t: &str) -> TResult<u8> {
    t.parse::<u8>()
        .or_else(|_| err(line, format!("expected a number 0..255, got `{t}`")))
}

fn slot(line: usize, t: &str) -> TResult<u8> {
    slot_value(t).map_or_else(|| err(line, format!("unknown slot `{t}`")), Ok)
}

struct Builder {
    id: u16,
    name: String,
    line: usize,
    width: Option<u8>,
    height: u8,
    layer_type: Option<u8>,
    flags: u8,
    count: usize,
    cmds: Vec<u8>,
    /// A `BITMAP` waiting for its rows: `(x, y, w, h, pixels so far)`.
    bitmap: Option<(u8, u8, u8, u8, Vec<u8>)>,
}

impl Builder {
    fn finish(self, end_line: usize) -> TResult<SpriteSrc> {
        if self.bitmap.is_some() {
            return err(end_line, "`BITMAP` is missing rows");
        }
        let (Some(width), Some(layer_type)) = (self.width, self.layer_type) else {
            return err(self.line, "sprite needs `size` and `layer`");
        };
        if self.count > u8::MAX as usize {
            return err(self.line, "more than 255 commands");
        }
        let mut bytes = Vec::with_capacity(sprite::HEADER_LEN + self.cmds.len());
        bytes.extend_from_slice(&[
            sprite::FORMAT_VERSION,
            width,
            self.height,
            layer_type,
            self.flags,
        ]);
        bytes.push(self.count as u8);
        bytes.extend_from_slice(&self.cmds);
        if let Err(e) = sprite::decode(&bytes) {
            return err(
                end_line,
                format!("sprite `{}` rejected by the decoder: {e:?}", self.name),
            );
        }
        Ok(SpriteSrc {
            id: self.id,
            name: self.name,
            line: self.line,
            bytes,
        })
    }
}

/// Compiles a `.dsl` file. Also checks that ids and names are unique within the file.
pub fn compile_sprites(src: &str) -> TResult<Vec<SpriteSrc>> {
    let mut out: Vec<SpriteSrc> = Vec::new();
    let mut cur: Option<Builder> = None;
    let mut last_line = 0;
    for (ln, t) in lines(src) {
        last_line = ln;
        let argc = |n: usize| {
            if t.len() == n + 1 {
                Ok(())
            } else {
                err(ln, format!("`{}` takes {n} arguments", t[0]))
            }
        };
        if t[0] == "sprite" {
            argc(2)?;
            if let Some(b) = cur.take() {
                out.push(b.finish(ln)?);
            }
            let id = t[1].parse::<u16>().ok().filter(|&i| i != 0);
            let Some(id) = id else {
                return err(ln, "sprite id must be 1..65535");
            };
            if !valid_name(t[2]) {
                return err(ln, "sprite name must be [a-z0-9_]{1,32}");
            }
            if out.iter().any(|s| s.id == id || s.name == t[2]) {
                return err(ln, "duplicate sprite id or name");
            }
            cur = Some(Builder {
                id,
                name: t[2].to_string(),
                line: ln,
                width: None,
                height: 0,
                layer_type: None,
                flags: 0,
                count: 0,
                cmds: Vec::new(),
                bitmap: None,
            });
            continue;
        }
        let Some(b) = cur.as_mut() else {
            return err(ln, "expected `sprite` first");
        };
        if let Some((x, y, w, h, px)) = b.bitmap.as_mut() {
            if t.len() != 1 || t[0].chars().count() != *w as usize {
                return err(ln, format!("a `BITMAP` row is {w} characters, no spaces"));
            }
            for c in t[0].chars() {
                let Some(v) = bitmap_char(c) else {
                    return err(ln, format!("unknown `BITMAP` character `{c}`"));
                };
                px.push(v);
            }
            if px.len() == *w as usize * *h as usize {
                b.cmds.extend_from_slice(&[op::BITMAP << 4, *x, *y, *w, *h]);
                b.cmds.extend(bitmap::encode(px));
                b.count += 1;
                b.bitmap = None;
            }
            continue;
        }
        match t[0] {
            "size" => {
                argc(2)?;
                if b.width.is_some() || b.count > 0 {
                    return err(ln, "`size` must come once, right after `sprite`");
                }
                let (w, h) = (num(ln, t[1])?, num(ln, t[2])?);
                if !sprite::valid_size(w) || !sprite::valid_size(h) {
                    return err(ln, "size must be 32, 64, 96 or 128");
                }
                b.width = Some(w);
                b.height = h;
            }
            "layer" => {
                argc(1)?;
                if b.width.is_none() || b.layer_type.is_some() {
                    return err(ln, "`layer` must come once, after `size`");
                }
                let Some(lt) = layer::NAMES.iter().position(|n| *n == t[1]) else {
                    return err(ln, format!("unknown layer `{}`", t[1]));
                };
                b.layer_type = Some(lt as u8);
            }
            "flags" => {
                argc(1)?;
                if b.layer_type.is_none() || b.count > 0 {
                    return err(ln, "`flags` must come after `layer`, before commands");
                }
                if t[1] != "no_outline" {
                    return err(ln, "only `flags no_outline` exists");
                }
                b.flags |= FLAG_NO_OUTLINE;
            }
            "copy" => {
                argc(1)?;
                if b.layer_type.is_none() {
                    return err(ln, "`copy` must come after `size` and `layer`");
                }
                let Some(src) = out.iter().find(|s| s.name == t[1]) else {
                    return err(
                        ln,
                        format!("`copy`: no earlier sprite `{}` in this file", t[1]),
                    );
                };
                b.cmds.extend_from_slice(&src.bytes[sprite::HEADER_LEN..]);
                b.count += src.bytes[sprite::HEADER_LEN - 1] as usize;
            }
            "BITMAP" => {
                argc(4)?;
                if b.layer_type.is_none() {
                    return err(ln, "commands must come after `size` and `layer`");
                }
                let (x, y) = (num(ln, t[1])?, num(ln, t[2])?);
                let (w, h) = (num(ln, t[3])?, num(ln, t[4])?);
                if w == 0 || h == 0 {
                    return err(ln, "`BITMAP` needs a width and a height of at least 1");
                }
                b.bitmap = Some((x, y, w, h, Vec::new()));
            }
            cmd => {
                if b.layer_type.is_none() {
                    return err(ln, "commands must come after `size` and `layer`");
                }
                encode_cmd(ln, cmd, &t[1..], &mut b.cmds)?;
                b.count += 1;
            }
        }
    }
    if let Some(b) = cur.take() {
        out.push(b.finish(last_line)?);
    }
    Ok(out)
}

fn encode_cmd(ln: usize, cmd: &str, a: &[&str], out: &mut Vec<u8>) -> TResult<()> {
    let need = |n: usize| {
        if a.len() == n {
            Ok(())
        } else {
            err(ln, format!("`{cmd}` takes {n} arguments"))
        }
    };
    match cmd {
        "RECT" | "LINE" | "ELLIPSE" | "RING" => {
            need(5)?;
            let o = match cmd {
                "RECT" => op::RECT,
                "LINE" => op::LINE,
                "ELLIPSE" => op::ELLIPSE,
                _ => op::RING,
            };
            out.push(o << 4 | slot(ln, a[4])?);
            for t in &a[..4] {
                out.push(num(ln, t)?);
            }
        }
        "PIXEL" => {
            need(3)?;
            out.extend_from_slice(&[
                op::PIXEL << 4 | slot(ln, a[2])?,
                num(ln, a[0])?,
                num(ln, a[1])?,
            ]);
        }
        "POLY" => {
            let n = a.len().saturating_sub(1) / 2;
            if a.len() < 1 + 2 * POLY_MIN as usize
                || a.len().is_multiple_of(2)
                || n > POLY_MAX as usize
            {
                return err(ln, "`POLY` takes a slot and 3..8 vertex pairs");
            }
            out.extend_from_slice(&[op::POLY << 4 | slot(ln, a[0])?, n as u8]);
            for t in &a[1..] {
                out.push(num(ln, t)?);
            }
        }
        "MIRROR_X" | "MIRROR_Y" => {
            need(1)?;
            let o = if cmd == "MIRROR_X" {
                op::MIRROR_X
            } else {
                op::MIRROR_Y
            };
            out.extend_from_slice(&[o << 4, num(ln, a[0])?]);
        }
        "SHADE" => {
            need(4)?;
            out.extend_from_slice(&[
                op::SHADE << 4 | slot(ln, a[2])?,
                num(ln, a[0])?,
                num(ln, a[1])?,
                num(ln, a[3])?,
            ]);
        }
        "SOCKET" => {
            need(2)?;
            out.extend_from_slice(&[op::SOCKET << 4, num(ln, a[0])?, num(ln, a[1])?]);
        }
        _ => return err(ln, format!("unknown statement `{cmd}`")),
    }
    Ok(())
}

/// A compiled palette with the names of its ramps and materials (index = position).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteSrc {
    pub palette_version: u16,
    pub ramps: Vec<String>,
    pub materials: Vec<String>,
    /// Palette body (`FORMAT.md` 3.2), already validated.
    pub bytes: Vec<u8>,
}

impl PaletteSrc {
    pub fn ramp(&self, name: &str) -> Option<u8> {
        self.ramps.iter().position(|r| r == name).map(|i| i as u8)
    }

    pub fn material(&self, name: &str) -> Option<u8> {
        self.materials
            .iter()
            .position(|m| m == name)
            .map(|i| i as u8)
    }
}

fn hex(ln: usize, t: &str) -> TResult<[u8; 3]> {
    let h = t
        .strip_prefix('#')
        .filter(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()));
    let Some(h) = h else {
        return err(ln, format!("expected #rrggbb, got `{t}`"));
    };
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0);
    Ok([byte(0), byte(2), byte(4)])
}

/// Compiles a `.pal` file.
pub fn compile_palette(src: &str) -> TResult<PaletteSrc> {
    let mut version = None;
    let mut outline = None;
    let mut dark: Option<(usize, String)> = None;
    let mut curse: Option<(usize, String)> = None;
    let mut cold: Option<(usize, String)> = None;
    let mut ramps: Vec<(String, Vec<[u8; 3]>)> = Vec::new();
    let mut materials: Vec<(usize, String, String, String, u8)> = Vec::new();
    for (ln, t) in lines(src) {
        let argc = |n: usize| {
            if t.len() == n + 1 {
                Ok(())
            } else {
                err(ln, format!("`{}` takes {n} arguments", t[0]))
            }
        };
        match t[0] {
            "palette" => {
                argc(1)?;
                version = Some(
                    t[1].parse::<u16>()
                        .or_else(|_| err(ln, "palette version must be 0..65535"))?,
                );
            }
            "outline" => {
                argc(1)?;
                outline = Some(hex(ln, t[1])?);
            }
            "dark" | "curse" | "cold" => {
                argc(1)?;
                let v = Some((ln, t[1].to_string()));
                match t[0] {
                    "dark" => dark = v,
                    "curse" => curse = v,
                    _ => cold = v,
                }
            }
            "ramp" => {
                argc(1 + RAMP_COLORS as usize)?;
                if !valid_name(t[1]) || ramps.iter().any(|r| r.0 == t[1]) {
                    return err(ln, "ramp name must be unique [a-z0-9_]{1,32}");
                }
                let colors = t[2..].iter().map(|c| hex(ln, c)).collect::<TResult<_>>()?;
                ramps.push((t[1].to_string(), colors));
            }
            "material" => {
                let flags = match t.len() {
                    4 => 0,
                    5 if t[4] == "dither" => palette::MATERIAL_DITHER,
                    _ => return err(ln, "`material <name> <primary> <secondary> [dither]`"),
                };
                if !valid_name(t[1]) || materials.iter().any(|m| m.1 == t[1]) {
                    return err(ln, "material name must be unique [a-z0-9_]{1,32}");
                }
                let (name, p, s) = (t[1].to_string(), t[2].to_string(), t[3].to_string());
                materials.push((ln, name, p, s, flags));
            }
            other => return err(ln, format!("unknown statement `{other}`")),
        }
    }
    let (Some(version), Some(outline), Some(dark), Some(curse), Some(cold)) =
        (version, outline, dark, curse, cold)
    else {
        return err(
            0,
            "palette needs `palette`, `outline`, `dark`, `curse` and `cold`",
        );
    };
    if ramps.is_empty() || ramps.len() > 255 || materials.is_empty() || materials.len() > 255 {
        return err(0, "palette needs 1..255 ramps and 1..255 materials");
    }
    let ramp_ix = |ln: usize, name: &str| match ramps.iter().position(|r| r.0 == name) {
        Some(i) => Ok(i as u8),
        None => err(ln, format!("unknown ramp `{name}`")),
    };
    let mut bytes = alloc::vec![palette::FORMAT_VERSION];
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&outline);
    for (ln, name) in [&dark, &curse, &cold] {
        bytes.push(ramp_ix(*ln, name)?);
    }
    bytes.push(ramps.len() as u8);
    for (_, colors) in &ramps {
        for c in colors {
            bytes.extend_from_slice(c);
        }
    }
    bytes.push(materials.len() as u8);
    for (ln, _, p, s, flags) in &materials {
        bytes.extend_from_slice(&[ramp_ix(*ln, p)?, ramp_ix(*ln, s)?, *flags]);
    }
    if palette::Palette::parse(&bytes).is_err() {
        return err(0, "compiled palette rejected by the decoder");
    }
    Ok(PaletteSrc {
        palette_version: version,
        ramps: ramps.into_iter().map(|r| r.0).collect(),
        materials: materials.into_iter().map(|m| m.1).collect(),
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_format_example() {
        let src = "# Warrior sword\nsprite 199 example_sword\nsize 32 32\nlayer mainhand\n\
                   RECT 22 5 2 12 P\nPIXEL 22 4 P\nRECT 20 17 6 1 A\nRECT 22 18 2 2 S\n\
                   LINE 22 7 22 14 U\nSOCKET 22 17\nSHADE 22 5 P 3\n";
        let s = compile_sprites(src).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].id, 199);
        assert_eq!(&s[0].bytes[..6], &[2, 32, 32, 16, 0, 7]);
        assert_eq!(&s[0].bytes[6..11], &[0x12, 22, 5, 2, 12]);
        assert_eq!(&s[0].bytes[s[0].bytes.len() - 4..], &[0x92, 22, 5, 3]);
    }

    #[test]
    fn bitmap_rows_legend_and_copy() {
        let src = "sprite 1 a\nsize 32 32\nlayer head\nRECT 1 1 2 2 K\n\
                   BITMAP 2 3 4 2\n.K_O\n5ajx\n\
                   sprite 2 b\nsize 32 32\nlayer head\ncopy a\nPIXEL 9 9 H\n";
        let s = compile_sprites(src).unwrap();
        assert_eq!(s[0].bytes[5], 2, "RECT + BITMAP");
        let d = sprite::decode(&s[0].bytes).unwrap();
        let v = bitmap::value;
        assert_eq!(
            d.bitmaps[0],
            [
                v(7, 0),
                v(2, 0),
                v(0, 0),
                v(1, 0),
                v(2, 6),
                v(3, 1),
                v(4, 4),
                v(6, 6)
            ]
        );
        // `copy` takes the commands of `a`, then its own.
        let c = sprite::decode(&s[1].bytes).unwrap();
        assert_eq!(c.cmds.len(), 3);
        assert_eq!(c.bitmaps, d.bitmaps);
        let want = [&s[0].bytes[6..], &[0x23, 9, 9][..]].concat();
        assert_eq!(&s[1].bytes[6..], &want[..]);
        // Every legend character maps to a value the decoder accepts.
        for c in ".PSAEUOKHYDR_012345abcdefghijklmnopqrstuvwx".chars() {
            assert!(bitmap_char(c).is_some(), "{c}");
        }
        for c in ['6', 'y', 'z', '#', 'B'] {
            assert!(bitmap_char(c).is_none(), "{c}");
        }
    }

    #[test]
    fn all_commands_and_aliases() {
        let src = "sprite 1 a\nsize 32 32\nlayer head\nflags no_outline\n\
                   POLY K 1 1 5 1 3 4\nMIRROR_X 16\nMIRROR_Y 16\nELLIPSE 1 1 4 4 H\nRING 1 1 4 4 Y\n\
                   PIXEL 2 2 D\nPIXEL 3 3 R\nPIXEL 4 4 O\nPIXEL 5 5 _\n\
                   sprite 2 b\nsize 32 32\nlayer tile\nRECT 0 0 32 32 E\n";
        let s = compile_sprites(src).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(&s[0].bytes[..6], &[2, 32, 32, 11, 1, 9]);
        assert_eq!(&s[0].bytes[6..14], &[0x62, 3, 1, 1, 5, 1, 3, 4]);
        assert_eq!(s[1].bytes[6], 0x15);
    }

    #[test]
    fn errors_with_lines() {
        let cases = [
            ("RECT 1 1 1 1 P", 1),                                     // no sprite
            ("sprite 0 a\nsize 32 32\nlayer head", 1),                 // id 0
            ("sprite 1 A\nsize 32 32\nlayer head", 1),                 // name
            ("sprite 1 a\nsize 20 16\nlayer head", 2),                 // size
            ("sprite 1 a\nsize 32 32\nlayer hat", 3),                  // layer name
            ("sprite 1 a\nlayer head", 2),                             // layer before size
            ("sprite 1 a\nsize 32 32\nRECT 1 1 1 1 P", 3),             // command before layer
            ("sprite 1 a\nsize 32 32\nlayer head\nRECT 1 1 1 P", 4),   // argc
            ("sprite 1 a\nsize 32 32\nlayer head\nRECT 1 1 1 1 Q", 4), // slot
            ("sprite 1 a\nsize 32 32\nlayer head\nRECT 1 1 1 999 P", 4),
            ("sprite 1 a\nsize 32 32\nlayer head\nPOLY P 1 1 2 2", 4), // 2 vertices
            ("sprite 1 a\nsize 32 32\nlayer head\nBOX 1 1", 4),        // unknown
            ("sprite 1 a\nsize 32 32\nlayer head\nRECT 30 1 10 1 P", 4), // decoder: past edge
            (
                "sprite 1 a\nsize 32 32\nlayer head\nsprite 1 b\nsize 32 32\nlayer head",
                4,
            ), // dup id
            ("sprite 1 a\nsize 32 32\nlayer head\nflags shiny", 4),
            ("sprite 1 a\nsize 32 32\nlayer head\nBITMAP 0 0 2 1\nK", 5), // row width
            ("sprite 1 a\nsize 32 32\nlayer head\nBITMAP 0 0 2 1\nKz", 5), // character
            ("sprite 1 a\nsize 32 32\nlayer head\nBITMAP 0 0 2 2\nKK", 5), // missing row
            ("sprite 1 a\nsize 32 32\nlayer head\nBITMAP 31 0 2 1\nKK", 5), // past edge
            ("sprite 1 a\nsize 32 32\nlayer head\nBITMAP 0 0 0 1", 4),    // zero width
            ("sprite 1 a\nsize 32 32\nlayer head\ncopy b", 4),            // no such sprite
        ];
        for (src, line) in cases {
            let e = compile_sprites(src).unwrap_err();
            assert_eq!(e.line, line, "{src:?}: {e}");
        }
        // Missing layer at end of file.
        assert!(compile_sprites("sprite 1 a\nsize 32 32").is_err());
    }

    #[test]
    fn palette_text() {
        let six = "#101010 #202020 #404040 #808080 #c0c0c0 #f0f0f0";
        let src = format!(
            "palette 3\noutline #120c16\n\
             ramp gray {six} # neutral\n#\n\
             ramp steel #3a4250 #5c6676 #8a94a4 #c8d0dc #d8e0e8 #f0f4f8\n\
             ramp leather {six}\n\
             dark gray\ncurse steel\ncold leather\n\
             material sword steel leather\nmaterial cloth leather leather dither\n"
        );
        let p = compile_palette(&src).unwrap();
        assert_eq!(p.palette_version, 3);
        assert_eq!(p.ramp("leather"), Some(2));
        assert_eq!(p.material("cloth"), Some(1));
        let parsed = palette::Palette::parse(&p.bytes).unwrap();
        assert_eq!(parsed.material(0), (1, 2));
        assert_eq!((parsed.dithers(0), parsed.dithers(1)), (false, true));
        assert_eq!((parsed.curse_ramp, parsed.cold_ramp), (1, 2));
        assert_eq!(parsed.color(1, 5), [0xf0, 0xf4, 0xf8]);
        // 4-color ramps (palette format 1), a missing curse line, an unknown material flag.
        let four = "#000000 #000000 #000000 #000000";
        let base = "palette 1\noutline #000000\ndark g\ncurse g\ncold g\n";
        assert!(compile_palette(&format!("{base}ramp g {four}\nmaterial m g g")).is_err());
        assert!(compile_palette(&format!("{base}ramp g {six}\nmaterial m g g")).is_ok());
        assert!(compile_palette(&format!("{base}ramp g {six}\nmaterial m g g shiny")).is_err());
        let no_curse = "palette 1\noutline #000000\ndark g\ncold g\n";
        assert!(compile_palette(&format!("{no_curse}ramp g {six}\nmaterial m g g")).is_err());
        assert!(compile_palette("palette 1\noutline #0000\n").is_err());
    }
}
