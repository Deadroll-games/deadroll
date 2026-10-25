//! Pictures larger than one sprite canvas (buildings, loading screens, frames), put together from
//! sprites by a text layout and rendered by `crates/render` like every preview. Site art only: a
//! composed picture never goes into a sprite book.
//!
//! ```text
//! # comment (a `#` token on its own)
//! canvas <w> <h>                  picture size in pixels (first line)
//! bg #rrggbb                      fill the background (default transparent)
//! put  <sprite> <x> <y> [opts]    sprite canvas with its top left corner at (x, y)
//! cell <sprite> <cx> <cy> [opts]  bottom-center on the 64 px map cell (cx, cy), as the app draws
//! tile <sprite> <x> <y> <cols> <rows> [opts]   the sprite repeated in a grid of its own size
//! ```
//! `<sprite>` is a name or an id. Options: `material=<name>`, `accent=<ramp>`, `element=<ramp>`,
//! `flip` (mirror left to right). Later lines draw over earlier ones.

use std::collections::BTreeMap;

use crate::preview::{self, default_look, over, Img, Look, Pal, CELL};
use crate::validate::Compiled;

fn num(s: Option<&&str>, what: &str, line: usize) -> Result<i32, String> {
    s.and_then(|v| v.parse().ok())
        .ok_or(format!("line {line}: bad or missing {what}"))
}

fn flipped(img: &Img) -> Img {
    let mut rgba = vec![0; img.rgba.len()];
    for y in 0..img.h {
        for x in 0..img.w {
            let (s, d) = ((y * img.w + x) * 4, (y * img.w + img.w - 1 - x) * 4);
            rgba[d..d + 4].copy_from_slice(&img.rgba[s..s + 4]);
        }
    }
    Img {
        w: img.w,
        h: img.h,
        rgba,
    }
}

/// Renders a layout (module docs) at one pixel scale.
pub fn compose(
    pal: &Pal,
    book_bytes: &[u8],
    sprites: &[Compiled],
    layout: &str,
) -> Result<Img, String> {
    let by_name: BTreeMap<&str, &Compiled> =
        sprites.iter().map(|c| (c.src.name.as_str(), c)).collect();
    let ids = preview::by_id(sprites);
    let mut img: Option<Img> = None;
    for (i, raw) in layout.lines().enumerate() {
        let line = i + 1;
        // A comment is a `#` token on its own (`#rrggbb` is a color).
        let t: Vec<&str> = raw.split_whitespace().take_while(|w| *w != "#").collect();
        if t.is_empty() {
            continue;
        }
        if t[0] == "canvas" {
            let (w, h) = (
                num(t.get(1), "width", line)?,
                num(t.get(2), "height", line)?,
            );
            if w <= 0 || h <= 0 || w > 4096 || h > 4096 {
                return Err(format!("line {line}: canvas 1..4096"));
            }
            img = Some(Img {
                w: w as usize,
                h: h as usize,
                rgba: vec![0; w as usize * h as usize * 4],
            });
            continue;
        }
        let img = img
            .as_mut()
            .ok_or(format!("line {line}: `canvas` must come first"))?;
        if t[0] == "bg" {
            let c = t
                .get(1)
                .and_then(|s| preview::parse_hex(s))
                .ok_or(format!("line {line}: bg #rrggbb"))?;
            for px in img.rgba.chunks_exact_mut(4) {
                px.copy_from_slice(&[c[0], c[1], c[2], 255]);
            }
            continue;
        }
        let (args, nargs) = match t[0] {
            "put" | "cell" => (&t[1..], 3),
            "tile" => (&t[1..], 5),
            k => return Err(format!("line {line}: unknown `{k}`")),
        };
        let name = args.first().ok_or(format!("line {line}: no sprite"))?;
        let c = name
            .parse::<u16>()
            .ok()
            .and_then(|id| ids.get(&id).copied())
            .or_else(|| by_name.get(name).copied())
            .ok_or(format!("line {line}: no sprite `{name}`"))?;
        let mut look: Look = default_look(&c.src.name, c.sprite.header.layer_type);
        let mut flip = false;
        for o in args.iter().skip(nargs) {
            let leak = |v: &str| -> &'static str { Box::leak(v.to_string().into_boxed_str()) };
            match o.split_once('=') {
                Some(("material", v)) => look.material = leak(v),
                Some(("accent", v)) => look.accent = leak(v),
                Some(("element", v)) => look.element = leak(v),
                None if *o == "flip" => flip = true,
                _ => return Err(format!("line {line}: unknown option `{o}`")),
            }
        }
        let mut s = preview::render_look(pal, book_bytes, c, look)?;
        if flip {
            s = flipped(&s);
        }
        let (a, b) = (num(args.get(1), "x", line)?, num(args.get(2), "y", line)?);
        match t[0] {
            "put" => over(img, &s, a, b),
            "cell" => {
                let x = a * CELL as i32 + CELL as i32 / 2 - s.w as i32 / 2;
                let y = (b + 1) * CELL as i32 - s.h as i32;
                over(img, &s, x, y);
            }
            _ => {
                let (cols, rows) = (
                    num(args.get(3), "cols", line)?,
                    num(args.get(4), "rows", line)?,
                );
                for r in 0..rows {
                    for q in 0..cols {
                        over(img, &s, a + q * s.w as i32, b + r * s.h as i32);
                    }
                }
            }
        }
    }
    img.ok_or("empty layout, no `canvas`".into())
}
