//! `art upscale`: moves format 1 sprite sources to format 2 by doubling every coordinate, so a
//! sprite keeps its shape on a canvas twice as large (`FORMAT.md` 1.1).
//!
//! RECT, PIXEL, axis-aligned LINE, MIRROR and SOCKET map to exactly the same pixels (each old
//! pixel becomes a 2x2 block). POLY, ELLIPSE and RING keep their geometry and are sampled at the
//! finer grid. A diagonal LINE becomes two parallel lines 2 px thick, a SHADE becomes its 2x2
//! block with the explicit shade on the top left pixel.

use deadroll_render::raster::rasterize;
use deadroll_render::sprite;
use deadroll_render::text::compile_sprites;

/// Code part and comment part (with its leading spaces) of a DSL line.
fn split_comment(l: &str) -> (&str, &str) {
    let b = l.as_bytes();
    for i in 0..b.len() {
        let starts_token = i == 0 || b[i - 1].is_ascii_whitespace();
        let ends = b.get(i + 1).is_none_or(|c| c.is_ascii_whitespace());
        if b[i] == b'#' && starts_token && ends {
            let code = l[..i].trim_end();
            return (code, &l[code.len()..]);
        }
    }
    (l.trim_end(), "")
}

fn num(t: &str, line: usize) -> Result<u32, String> {
    t.parse::<u32>()
        .map_err(|_| format!("line {line}: expected a number, got `{t}`"))
}

/// `2 * v`, as a DSL number (at most 255).
fn twice(v: u32, line: usize) -> Result<String, String> {
    let d = 2 * v;
    if d > u8::MAX as u32 {
        return Err(format!("line {line}: {v} doubled is {d}, over 255"));
    }
    Ok(d.to_string())
}

/// Converts the commands of one line; `None` keeps the line as is.
fn convert(t: &[&str], line: usize) -> Result<Option<Vec<String>>, String> {
    let n = |i: usize| num(t[i], line);
    let d = |i: usize| twice(num(t[i], line)?, line);
    let argc = |k: usize| {
        if t.len() == k {
            Ok(())
        } else {
            Err(format!("line {line}: `{}` takes {} arguments", t[0], k - 1))
        }
    };
    let out = match t[0] {
        "size" => {
            argc(3)?;
            vec![format!("size {} {}", d(1)?, d(2)?)]
        }
        "RECT" | "ELLIPSE" => {
            argc(6)?;
            vec![format!(
                "{} {} {} {} {} {}",
                t[0],
                d(1)?,
                d(2)?,
                d(3)?,
                d(4)?,
                t[5]
            )]
        }
        "RING" => {
            argc(6)?;
            let (x, y, w, h) = (n(1)? * 2, n(2)? * 2, n(3)? * 2, n(4)? * 2);
            let mut v = vec![format!(
                "RING {} {} {} {} {}",
                d(1)?,
                d(2)?,
                d(3)?,
                d(4)?,
                t[5]
            )];
            if w > 2 && h > 2 {
                v.push(format!(
                    "RING {} {} {} {} {}",
                    x + 1,
                    y + 1,
                    w - 2,
                    h - 2,
                    t[5]
                ));
            }
            v
        }
        "PIXEL" => {
            argc(4)?;
            vec![format!("RECT {} {} 2 2 {}", d(1)?, d(2)?, t[3])]
        }
        "LINE" => {
            argc(6)?;
            let (x0, y0, x1, y1) = (n(1)?, n(2)?, n(3)?, n(4)?);
            if x0 == x1 || y0 == y1 {
                let (x, y) = (x0.min(x1), y0.min(y1));
                let (w, h) = (x0.abs_diff(x1) + 1, y0.abs_diff(y1) + 1);
                vec![format!(
                    "RECT {} {} {} {} {}",
                    twice(x, line)?,
                    twice(y, line)?,
                    twice(w, line)?,
                    twice(h, line)?,
                    t[5]
                )]
            } else {
                // Same slope; the copy is offset across the main direction, inside the old extent.
                let (ox, oy) = if x0.abs_diff(x1) >= y0.abs_diff(y1) {
                    (0, 1)
                } else {
                    (1, 0)
                };
                let (a, b, c, e) = (2 * x0, 2 * y0, 2 * x1, 2 * y1);
                for v in [a + ox, b + oy, c + ox, e + oy] {
                    if v > u8::MAX as u32 {
                        return Err(format!("line {line}: LINE doubled goes over 255"));
                    }
                }
                vec![
                    format!("LINE {a} {b} {c} {e} {}", t[5]),
                    format!("LINE {} {} {} {} {}", a + ox, b + oy, c + ox, e + oy, t[5]),
                ]
            }
        }
        "POLY" => {
            if t.len() < 2 + 6 || !(t.len() - 2).is_multiple_of(2) {
                return Err(format!("line {line}: POLY needs a slot and vertex pairs"));
            }
            let pts: Result<Vec<String>, String> = (2..t.len()).map(d).collect();
            vec![format!("POLY {} {}", t[1], pts?.join(" "))]
        }
        "MIRROR_X" | "MIRROR_Y" => {
            argc(2)?;
            vec![format!("{} {}", t[0], d(1)?)]
        }
        "SHADE" => {
            argc(5)?;
            vec![
                format!("RECT {} {} 2 2 {}", d(1)?, d(2)?, t[3]),
                format!("SHADE {} {} {} {}", d(1)?, d(2)?, t[3], t[4]),
            ]
        }
        "SOCKET" => {
            argc(3)?;
            vec![format!("SOCKET {} {}", d(1)?, d(2)?)]
        }
        _ => return Ok(None),
    };
    Ok(Some(out))
}

/// Rewrites a format 1 `.dsl` text for format 2. Comments and blank lines are kept; comments
/// that name coordinates need a manual look.
pub fn upscale_text(src: &str) -> Result<String, String> {
    let mut out = String::with_capacity(src.len() + src.len() / 4);
    for (i, l) in src.lines().enumerate() {
        let (code, comment) = split_comment(l);
        let t: Vec<&str> = code.split_whitespace().collect();
        match if t.is_empty() {
            None
        } else {
            convert(&t, i + 1)?
        } {
            None => out.push_str(l),
            Some(lines) => {
                let last = lines.len() - 1;
                for (k, c) in lines.iter().enumerate() {
                    out.push_str(c);
                    if k == 0 {
                        out.push_str(comment);
                    }
                    if k != last {
                        out.push('\n');
                    }
                }
            }
        }
        out.push('\n');
    }
    Ok(out)
}

/// Per sprite of a converted file: `(name, cells whose slot differs from the old slot grid
/// enlarged x2)`. `old` is the format 1 text; its canvases are padded to a format 2 size only
/// to compile it, which does not move any pixel.
pub fn compare(old: &str, new: &str) -> Result<Vec<(String, usize)>, String> {
    let mut padded = String::new();
    for l in old.lines() {
        let (code, _) = split_comment(l);
        let t: Vec<&str> = code.split_whitespace().collect();
        if t.first() == Some(&"size") && t.len() == 3 {
            fn pad(v: &str) -> &str {
                match v {
                    "16" => "32",
                    "48" => "64",
                    v => v,
                }
            }
            padded.push_str(&format!("size {} {}\n", pad(t[1]), pad(t[2])));
        } else {
            padded.push_str(l);
            padded.push('\n');
        }
    }
    let old = compile_sprites(&padded).map_err(|e| format!("old: {e}"))?;
    let new = compile_sprites(new).map_err(|e| format!("new: {e}"))?;
    let mut res = Vec::new();
    for (o, n) in old.iter().zip(&new) {
        let go = rasterize(&sprite::decode(&o.bytes).map_err(|e| format!("{e:?}"))?);
        let gn = rasterize(&sprite::decode(&n.bytes).map_err(|e| format!("{e:?}"))?);
        let mut diff = 0;
        for y in 0..gn.height as i32 {
            for x in 0..gn.width as i32 {
                let a = go.get(x / 2, y / 2).map_or(0, |c| c.slot);
                let b = gn.get(x, y).map_or(0, |c| c.slot);
                diff += usize::from(a != b);
            }
        }
        res.push((n.name.clone(), diff));
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "# sword\nsprite 1 a\nsize 16 48   # canvas\nlayer portrait\n\
                       RECT 2 3 4 5 P\nPIXEL 7 8 S # tip\nLINE 1 1 1 9 S\nLINE 1 1 4 4 A\n\
                       ELLIPSE 1 1 6 6 E\nRING 2 2 8 8 P\nPOLY P 1 1 5 1 3 4\nMIRROR_X 16\n\
                       SHADE 3 3 P 3\nSOCKET 4 10\n\n";

    #[test]
    fn doubles_every_command() {
        let new = upscale_text(OLD).unwrap();
        let want = "# sword\nsprite 1 a\nsize 32 96   # canvas\nlayer portrait\n\
                    RECT 4 6 8 10 P\nRECT 14 16 2 2 S # tip\nRECT 2 2 2 18 S\n\
                    LINE 2 2 8 8 A\nLINE 2 3 8 9 A\nELLIPSE 2 2 12 12 E\n\
                    RING 4 4 16 16 P\nRING 5 5 14 14 P\nPOLY P 2 2 10 2 6 8\nMIRROR_X 32\n\
                    RECT 6 6 2 2 P\nSHADE 6 6 P 3\nSOCKET 8 20\n\n";
        assert_eq!(new, want);
        compile_sprites(&new).unwrap();
    }

    #[test]
    fn exact_commands_give_the_same_pixels() {
        let old = "sprite 1 a\nsize 32 32\nlayer mainhand\nRECT 2 3 4 5 P\nPIXEL 9 9 S\n\
                   LINE 12 2 12 9 A\nLINE 3 20 9 20 S\nPOLY P 10 10 14 10 14 14 10 14\n\
                   MIRROR_X 32\n";
        let diff = compare(old, &upscale_text(old).unwrap()).unwrap();
        assert_eq!(diff, vec![("a".to_string(), 0)]);
    }

    #[test]
    fn rejects_overflow_and_bad_lines() {
        assert!(upscale_text("RECT 200 0 1 1 P\n").is_err());
        assert!(upscale_text("MIRROR_X 128\n").is_err());
        assert!(upscale_text("RECT 1 1 P\n").is_err());
        assert!(upscale_text("POLY P 1 1 2 2\n").is_err());
    }
}
