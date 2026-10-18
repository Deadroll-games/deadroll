use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use art::preview::{self, Img, Pal, SHEET_BG};
use art::validate::{validate_files, Compiled, Issue};

const DEFAULT_PALETTE: &str = "assets/palette/palette_1.pal";
const SPRITES_DIR: &str = "assets/sprites";
/// `SpriteBook` body limit in programs/render.
const BOOK_MAX: usize = 64 * 1024;

const USAGE: &str = "usage:
  art validate [files.dsl ...]                 default: every file in assets/sprites
  art <file.dsl> [--id N] [--scale 4] [--out DIR|FILE.png] [--material NAME] [--bg #rrggbb]
  art sheet characters [--scale 4] [--out FILE.png]
  art sheet contact [files.dsl ...] [--scale 2] [--out FILE.png]
  art pack [--out DIR]                          palette.bin + book.bin (book 1, every sprite) for upload
  art upscale [files.dsl ...]                  rewrite format 1 sources (16..64 px) for format 2, in place
options: --palette FILE.pal (default assets/palette/palette_1.pal)";

struct Opts {
    pos: Vec<String>,
    id: Option<u16>,
    scale: u32,
    out: Option<PathBuf>,
    material: Option<String>,
    bg: Option<[u8; 3]>,
    palette: PathBuf,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        pos: Vec::new(),
        id: None,
        scale: 0,
        out: None,
        material: None,
        bg: None,
        palette: PathBuf::from(DEFAULT_PALETTE),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |name: &str| it.next().cloned().ok_or(format!("{name} needs a value"));
        match a.as_str() {
            "--id" => o.id = Some(val("--id")?.parse().map_err(|_| "bad --id")?),
            "--scale" => o.scale = val("--scale")?.parse().map_err(|_| "bad --scale")?,
            "--out" => o.out = Some(PathBuf::from(val("--out")?)),
            "--material" => o.material = Some(val("--material")?),
            "--bg" => {
                o.bg = Some(preview::parse_hex(&val("--bg")?).ok_or("bad --bg, use #rrggbb")?)
            }
            "--palette" => o.palette = PathBuf::from(val("--palette")?),
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            _ => o.pos.push(a.clone()),
        }
    }
    Ok(o)
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn norm(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn repo_files() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(SPRITES_DIR)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "dsl"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Loads `files` and the rest of the repo sprites (for id and name uniqueness).
fn load_all(files: &[PathBuf]) -> Result<(Vec<Compiled>, Vec<Issue>), String> {
    let mut all: Vec<PathBuf> = files.to_vec();
    for r in repo_files() {
        let same = |a: &Path, b: &Path| match (fs::canonicalize(a), fs::canonicalize(b)) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        };
        if !all.iter().any(|f| same(f, &r)) {
            all.push(r);
        }
    }
    let mut texts = Vec::new();
    for p in &all {
        texts.push((norm(p), read(p)?));
    }
    Ok(validate_files(&texts))
}

fn cmd_validate(o: &Opts) -> Result<bool, String> {
    let files: Vec<PathBuf> = if o.pos.is_empty() {
        repo_files()
    } else {
        o.pos.iter().map(PathBuf::from).collect()
    };
    let (sprites, issues) = load_all(&files)?;
    // Only report issues of the files asked for; the rest only serve uniqueness.
    let wanted: Vec<String> = files.iter().map(|p| norm(p)).collect();
    let shown: Vec<&Issue> = issues
        .iter()
        .filter(|i| wanted.contains(&i.file) || o.pos.is_empty())
        .collect();
    for i in &shown {
        eprintln!("{i}");
    }
    let n = sprites.iter().filter(|s| wanted.contains(&s.file)).count();
    if shown.is_empty() {
        println!("ok: {n} sprites in {} files", files.len());
        Ok(true)
    } else {
        eprintln!("{} problems", shown.len());
        Ok(false)
    }
}

fn load_pal(o: &Opts) -> Result<Pal, String> {
    Pal::load(&read(&o.palette)?)
}

fn static_name(s: &str) -> Result<&'static str, String> {
    Ok(Box::leak(s.to_string().into_boxed_str()))
}

fn finish(mut img: Img, bg: Option<[u8; 3]>) -> Vec<u8> {
    if let Some(bg) = bg {
        preview::flatten(&mut img, bg);
    }
    preview::to_png(&img)
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn cmd_render(o: &Opts) -> Result<bool, String> {
    let file = PathBuf::from(&o.pos[0]);
    let (sprites, issues) = load_all(std::slice::from_ref(&file))?;
    let mine = norm(&file);
    let own: Vec<&Compiled> = sprites.iter().filter(|s| s.file == mine).collect();
    for i in issues.iter().filter(|i| i.file == mine) {
        eprintln!("warning: {i}");
    }
    let pal = load_pal(o)?;
    let book = preview::pack_book(&sprites)?;
    let scale = if o.scale == 0 { 4 } else { o.scale };
    let pick: Vec<&&Compiled> = own
        .iter()
        .filter(|c| o.id.is_none_or(|id| c.src.id == id))
        .collect();
    if pick.is_empty() {
        return Err("no sprite selected".into());
    }
    let mat = o.material.as_deref().map(static_name).transpose()?;
    let out = o.out.clone().unwrap_or_else(|| PathBuf::from("."));
    let single_file = out.extension().is_some_and(|e| e == "png");
    if single_file && pick.len() != 1 {
        return Err("--out FILE.png needs exactly one sprite, use --id or a directory".into());
    }
    for c in pick {
        let img = preview::render_sprite(&pal, &book, c, scale, mat)?;
        let path = if single_file {
            out.clone()
        } else {
            out.join(format!("{}_{}.png", c.src.id, c.src.name))
        };
        write(&path, &finish(img, o.bg))?;
        println!("{}", path.display());
    }
    Ok(true)
}

fn cmd_sheet(o: &Opts) -> Result<bool, String> {
    let kind = o.pos.get(1).map(String::as_str).unwrap_or("");
    let files: Vec<PathBuf> = if o.pos.len() > 2 {
        o.pos[2..].iter().map(PathBuf::from).collect()
    } else {
        repo_files()
    };
    let (sprites, _) = load_all(&files)?;
    let pal = load_pal(o)?;
    let book = preview::pack_book(&sprites)?;
    let bg = o.bg.unwrap_or(SHEET_BG);
    let (default_scale, default_out) = match kind {
        "characters" => (4, "target/art/characters.png"),
        "contact" => (2, "target/art/contact.png"),
        _ => return Err("sheet kind: characters | contact".into()),
    };
    let scale = if o.scale == 0 { default_scale } else { o.scale };
    let mut images = Vec::new();
    if kind == "characters" {
        for (_, layers) in preview::characters() {
            let mut img = preview::render_character(&pal, &book, &layers, scale)?;
            preview::flatten(&mut img, bg);
            images.push(img);
        }
    } else {
        let named: Vec<String> = files.iter().map(|p| norm(p)).collect();
        let only = o.pos.len() > 2;
        for c in sprites.iter().filter(|c| !only || named.contains(&c.file)) {
            let mut img = preview::render_sprite(&pal, &book, c, scale, None)?;
            preview::flatten(&mut img, bg);
            images.push(img);
        }
    }
    let cols = if kind == "characters" { 3 } else { 8 };
    let sheet = preview::grid(&images, cols, 4 * scale as usize, bg);
    let path = o.out.clone().unwrap_or_else(|| PathBuf::from(default_out));
    write(&path, &preview::to_png(&sheet))?;
    println!("{} ({} images)", path.display(), images.len());
    Ok(true)
}

/// Writes the upload inputs: the compiled palette and one sprite book (id 1) with every repo sprite.
fn cmd_pack(o: &Opts) -> Result<bool, String> {
    let (sprites, issues) = load_all(&repo_files())?;
    if !issues.is_empty() {
        for i in &issues {
            eprintln!("{i}");
        }
        return Err(format!("{} problems, run `art validate`", issues.len()));
    }
    let pal = load_pal(o)?;
    let book = preview::pack_book(&sprites)?;
    if book.len() > BOOK_MAX {
        return Err(format!("book is {} bytes, over {BOOK_MAX}", book.len()));
    }
    let dir = o
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from("target/render-assets"));
    write(&dir.join("palette.bin"), &pal.src.bytes)?;
    write(&dir.join("book.bin"), &book)?;
    println!(
        "{}: palette {} bytes, book {} bytes ({} sprites)",
        dir.display(),
        pal.src.bytes.len(),
        book.len(),
        sprites.len()
    );
    Ok(true)
}

/// Rewrites format 1 sources for format 2 and prints, per sprite, how many cells differ from the
/// old slot grid enlarged x2 (0 = the same pixels).
fn cmd_upscale(o: &Opts) -> Result<bool, String> {
    let files: Vec<PathBuf> = if o.pos.is_empty() {
        repo_files()
    } else {
        o.pos.iter().map(PathBuf::from).collect()
    };
    for p in &files {
        let old = read(p)?;
        let new = art::upscale::upscale_text(&old).map_err(|e| format!("{}: {e}", p.display()))?;
        let diff =
            art::upscale::compare(&old, &new).map_err(|e| format!("{}: {e}", p.display()))?;
        write(p, new.as_bytes())?;
        let changed: Vec<String> = diff
            .iter()
            .filter(|(_, n)| *n > 0)
            .map(|(name, n)| format!("{name} {n}"))
            .collect();
        println!(
            "{}: {} sprites, cells differing from x2: {}",
            norm(p),
            diff.len(),
            if changed.is_empty() {
                "none".to_string()
            } else {
                changed.join(", ")
            }
        );
    }
    Ok(true)
}

fn run() -> Result<bool, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        println!("{USAGE}");
        return Ok(true);
    }
    let o = parse(&args)?;
    match o.pos.first().map(String::as_str) {
        Some("validate") => cmd_validate(&Opts {
            pos: o.pos[1..].to_vec(),
            ..o
        }),
        Some("sheet") => cmd_sheet(&o),
        Some("pack") => cmd_pack(&o),
        Some("upscale") => cmd_upscale(&Opts {
            pos: o.pos[1..].to_vec(),
            ..o
        }),
        Some(_) => cmd_render(&o),
        None => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}
