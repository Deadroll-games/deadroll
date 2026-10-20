//! Validator rules of `FORMAT.md` section 6. The decoder (`deadroll-render`) already rejects
//! malformed sprites; this adds the authoring rules.

use std::collections::BTreeMap;

use deadroll_render::raster::{rasterize, Grid};
use deadroll_render::sprite::{self, layer, slot, Cmd, Sprite, SOCKET_SIZE};
use deadroll_render::text::{compile_sprites, SpriteSrc};

pub const MAX_CMDS: usize = 48;
pub const MAX_SHADE: usize = 8;
pub const MAX_SOCKET: usize = 4;
pub const MIN_RUNE_WEAPON: usize = 12;
/// Margin of an item portrait with a rune zone: three 2 px glow bands (`deadroll_render::craft`).
pub const PORTRAIT_RUNE_MARGIN: i32 = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub file: String,
    pub line: usize,
    pub sprite: String,
    pub msg: String,
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.sprite.is_empty() {
            write!(f, "{}:{}: {}", self.file, self.line, self.msg)
        } else {
            write!(
                f,
                "{}:{}: [{}] {}",
                self.file, self.line, self.sprite, self.msg
            )
        }
    }
}

/// One compiled and decoded sprite with its origin.
pub struct Compiled {
    pub file: String,
    pub src: SpriteSrc,
    pub sprite: Sprite,
}

/// Id range of a known sprite file (`FORMAT.md` 1.5). Unknown files only get the global range.
pub fn file_range(file: &str) -> Option<(u16, u16)> {
    let base = file.rsplit(['/', '\\']).next()?;
    Some(match base.strip_suffix(".dsl")? {
        "bodies" => (1, 99),
        "items" => (100, 399),
        "monsters" => (400, 499),
        "tiles" => (500, 599),
        _ => return None,
    })
}

/// Cuts a comment: `#` at the start of a token followed by whitespace or the end of the line.
fn strip_comment(l: &str) -> &str {
    let b = l.as_bytes();
    for i in 0..b.len() {
        let starts = i == 0 || b[i - 1].is_ascii_whitespace();
        let ends = b.get(i + 1).is_none_or(|c| c.is_ascii_whitespace());
        if b[i] == b'#' && starts && ends {
            return &l[..i];
        }
    }
    l
}

/// Text-level rule: slot letters allowed for the layer type
/// (body `K H Y D R O _`, items `P S A E U O _`). Letters are aliases after compiling, so this
/// has to look at the source text.
fn check_letters(file: &str, text: &str, out: &mut Vec<Issue>) {
    let mut name = String::new();
    let mut body = false;
    // Rows left in the current BITMAP: their uppercase letters are slot letters too.
    let mut rows = 0usize;
    for (i, l) in text.lines().enumerate() {
        let t: Vec<&str> = strip_comment(l).split_whitespace().collect();
        let Some(&cmd) = t.first() else { continue };
        if rows > 0 {
            rows -= 1;
            let allowed = if body { "KHYDRO" } else { "PSAEUO" };
            if let Some(c) = cmd
                .chars()
                .find(|c| c.is_ascii_uppercase() && !allowed.contains(*c))
            {
                out.push(Issue {
                    file: file.into(),
                    line: i + 1,
                    sprite: name.clone(),
                    msg: format!(
                        "BITMAP slot `{c}` not allowed on a {} layer (allowed: {allowed})",
                        if body { "body" } else { "item" }
                    ),
                });
            }
            continue;
        }
        match cmd {
            "BITMAP" => rows = t.get(4).and_then(|h| h.parse().ok()).unwrap_or(0),
            "sprite" => {
                name = t.get(2).copied().unwrap_or("").to_string();
                body = false;
            }
            "layer" => {
                body = t
                    .get(1)
                    .and_then(|n| layer::NAMES.iter().position(|x| x == n))
                    .is_some_and(|p| layer::is_body(p as u8));
            }
            "RECT" | "PIXEL" | "LINE" | "ELLIPSE" | "RING" | "POLY" | "SHADE" => {
                let tok = match cmd {
                    "POLY" => t.get(1),
                    "SHADE" => t.get(3),
                    _ => t.last(),
                };
                let allowed = if body { "KHYDRO_" } else { "PSAEUO_" };
                if let Some(s) = tok {
                    if s.len() != 1 || !allowed.contains(*s) {
                        out.push(Issue {
                            file: file.into(),
                            line: i + 1,
                            sprite: name.clone(),
                            msg: format!(
                                "slot `{s}` not allowed on a {} layer (allowed: {allowed})",
                                if body { "body" } else { "item" }
                            ),
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

fn is_line_slope_ok(x0: u8, y0: u8, x1: u8, y1: u8) -> bool {
    let dx = (x1 as i32 - x0 as i32).abs();
    let dy = (y1 as i32 - y0 as i32).abs();
    dx == 0 || dy == 0 || dx == dy || dx == 2 * dy || dy == 2 * dx
}

/// Canvas of the character layers (`FORMAT.md` 1.4).
pub const CHARACTER_SIZE: u8 = 96;

fn size_ok(layer_type: u8, w: u8, h: u8) -> bool {
    match layer_type {
        0..=layer::CHARACTER_MAX => (w, h) == (CHARACTER_SIZE, CHARACTER_SIZE),
        layer::TILE => (w, h) == (64, 64),
        layer::MONSTER => w == h && matches!(w, 96 | 128),
        layer::ICON => (w, h) == (32, 32),
        layer::PORTRAIT => true,
        _ => false,
    }
}

fn must_no_outline(layer_type: u8, name: &str) -> bool {
    // The front fist repaints the body's fist over the weapon: an outline would draw a seam at the
    // wrist.
    matches!(
        layer_type,
        layer::SHADOW | layer::NECKLACE | layer::FIST_FRONT | layer::OVERLAY_ELEMENT
    ) || (layer_type == layer::TILE && name.contains("floor"))
}

fn slot_at(g: &Grid, x: i32, y: i32) -> u8 {
    g.get(x, y).map_or(0, |c| c.slot)
}

fn check_sprite(c: &Compiled, out: &mut Vec<Issue>) {
    let s = &c.sprite;
    let h = &s.header;
    let mut bad = |msg: String| {
        out.push(Issue {
            file: c.file.clone(),
            line: c.src.line,
            sprite: c.src.name.clone(),
            msg,
        })
    };
    if s.cmds.len() > MAX_CMDS {
        bad(format!("{} commands, at most {MAX_CMDS}", s.cmds.len()));
    }
    let shades = s
        .cmds
        .iter()
        .filter(|c| matches!(c, Cmd::Shade { .. }))
        .count();
    if shades > MAX_SHADE {
        bad(format!("{shades} SHADE commands, at most {MAX_SHADE}"));
    }
    let sockets = s
        .cmds
        .iter()
        .filter(|c| matches!(c, Cmd::Socket { .. }))
        .count();
    if sockets > MAX_SOCKET {
        bad(format!("{sockets} SOCKET commands, at most {MAX_SOCKET}"));
    }
    if !size_ok(h.layer_type, h.width, h.height) {
        bad(format!(
            "size {}x{} not allowed for layer `{}`",
            h.width,
            h.height,
            layer::NAMES[h.layer_type as usize]
        ));
    }
    if must_no_outline(h.layer_type, &c.src.name) && !h.no_outline() {
        bad("layer needs `flags no_outline`".into());
    }
    for cmd in &s.cmds {
        if let Cmd::Line { x0, y0, x1, y1, .. } = *cmd {
            if !is_line_slope_ok(x0, y0, x1, y1) {
                bad(format!(
                    "LINE {x0} {y0} {x1} {y1}: slope must be 0, 90, 45 degrees, 1:2 or 2:1"
                ));
            }
        }
    }

    let g = rasterize(s);
    let (w, hh) = (h.width as i32, h.height as i32);

    if !h.no_outline() {
        let edge = (0..hh)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .find(|&(x, y)| {
                (x == 0 || y == 0 || x == w - 1 || y == hh - 1) && slot_at(&g, x, y) != 0
            });
        if let Some((x, y)) = edge {
            bad(format!(
                "pixel ({x}, {y}) touches the canvas edge, keep a 1 px margin for the outline"
            ));
        }
    }

    // Sockets: SOCKET_SIZE squares inside the P mask, not overlapping each other.
    let n = SOCKET_SIZE as i32;
    let mut taken: Vec<(i32, i32)> = Vec::new();
    for cmd in &s.cmds {
        if let Cmd::Socket { x, y } = *cmd {
            let (x, y) = (x as i32, y as i32);
            let sq: Vec<(i32, i32)> = (0..n)
                .flat_map(|dy| (0..n).map(move |dx| (x + dx, y + dy)))
                .collect();
            if sq
                .iter()
                .any(|&(px, py)| slot_at(&g, px, py) != slot::PRIMARY)
            {
                bad(format!(
                    "SOCKET {x} {y}: the {n}x{n} square is not inside the P mask"
                ));
            }
            if sq.iter().any(|p| taken.contains(p)) {
                bad(format!("SOCKET {x} {y}: overlaps another socket"));
            }
            taken.extend(sq);
        }
    }

    // Rune zone `U` (item layers): inside the P mask, at least MIN_RUNE_WEAPON px on a weapon,
    // and a portrait with runes keeps PORTRAIT_RUNE_MARGIN from the edges for the glow bands.
    if !layer::is_body(h.layer_type) {
        let mut runes = 0usize;
        let mut outside = None;
        for y in 0..hh {
            for x in 0..w {
                if slot_at(&g, x, y) != slot::RUNE {
                    continue;
                }
                runes += 1;
                let inside = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|&(dx, dy)| {
                    matches!(slot_at(&g, x + dx, y + dy), slot::PRIMARY | slot::RUNE)
                });
                if !inside {
                    outside.get_or_insert((x, y));
                }
            }
        }
        if let Some((x, y)) = outside {
            bad(format!("U pixel ({x}, {y}) is not inside the P mask"));
        }
        if runes > 0 && h.layer_type == layer::MAINHAND && runes < MIN_RUNE_WEAPON {
            bad(format!(
                "rune zone has {runes} px, a weapon needs at least {MIN_RUNE_WEAPON}"
            ));
        }
        let m = PORTRAIT_RUNE_MARGIN;
        if runes > 0 && h.layer_type == layer::PORTRAIT {
            let near = (0..hh)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .find(|&(x, y)| {
                    slot_at(&g, x, y) != 0 && (x < m || y < m || x >= w - m || y >= hh - m)
                });
            if let Some((x, y)) = near {
                bad(format!(
                    "pixel ({x}, {y}): a portrait with a rune zone keeps {m} px from the edges"
                ));
            }
        }
    }
}

/// Compiles text and runs every per-sprite rule. Returns the compiled sprites too.
pub fn validate_text(file: &str, text: &str) -> (Vec<Compiled>, Vec<Issue>) {
    let mut issues = Vec::new();
    check_letters(file, text, &mut issues);
    let srcs = match compile_sprites(text) {
        Ok(v) => v,
        Err(e) => {
            issues.push(Issue {
                file: file.into(),
                line: e.line,
                sprite: String::new(),
                msg: e.msg,
            });
            return (Vec::new(), issues);
        }
    };
    let range = file_range(file);
    let mut compiled = Vec::new();
    for src in srcs {
        let sprite = match sprite::decode(&src.bytes) {
            Ok(s) => s,
            Err(e) => {
                issues.push(Issue {
                    file: file.into(),
                    line: src.line,
                    sprite: src.name.clone(),
                    msg: format!("decoder error {e:?}"),
                });
                continue;
            }
        };
        let c = Compiled {
            file: file.into(),
            src,
            sprite,
        };
        let id = c.src.id;
        let range_msg = if !(1..=599).contains(&id) {
            Some(format!("id {id} outside 1..=599 (600+ is reserved)"))
        } else {
            range
                .filter(|&(lo, hi)| id < lo || id > hi)
                .map(|(lo, hi)| format!("id {id} outside the range {lo}-{hi} of this file"))
        };
        if let Some(msg) = range_msg {
            issues.push(Issue {
                file: file.into(),
                line: c.src.line,
                sprite: c.src.name.clone(),
                msg,
            });
        }
        check_sprite(&c, &mut issues);
        // Determinism: rasterizing twice gives the same cells.
        if rasterize(&c.sprite) != rasterize(&c.sprite) {
            issues.push(Issue {
                file: file.into(),
                line: c.src.line,
                sprite: c.src.name.clone(),
                msg: "rasterizing twice gives different cells".into(),
            });
        }
        compiled.push(c);
    }
    (compiled, issues)
}

/// Validates several files together: per-sprite rules plus unique ids and names across all of them.
pub fn validate_files(files: &[(String, String)]) -> (Vec<Compiled>, Vec<Issue>) {
    let mut all = Vec::new();
    let mut issues = Vec::new();
    for (name, text) in files {
        let (c, i) = validate_text(name, text);
        all.extend(c);
        issues.extend(i);
    }
    let mut ids: BTreeMap<u16, (String, String, usize)> = BTreeMap::new();
    let mut names: BTreeMap<String, (u16, String, usize)> = BTreeMap::new();
    for c in &all {
        let here = |msg: String| Issue {
            file: c.file.clone(),
            line: c.src.line,
            sprite: c.src.name.clone(),
            msg,
        };
        if let Some(p) = ids.insert(c.src.id, (c.src.name.clone(), c.file.clone(), c.src.line)) {
            issues.push(here(format!(
                "id {} already used by `{}` ({}:{})",
                c.src.id, p.0, p.1, p.2
            )));
        }
        if let Some(p) = names.insert(c.src.name.clone(), (c.src.id, c.file.clone(), c.src.line)) {
            issues.push(here(format!(
                "name already used by id {} ({}:{})",
                p.0, p.1, p.2
            )));
        }
    }
    (all, issues)
}
