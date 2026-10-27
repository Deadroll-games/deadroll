use art::preview::{self, Pal};
use art::validate::{validate_files, validate_text};

const OK: &str =
    "sprite 100 warrior_ok\nsize 96 96\nlayer mainhand\nRECT 21 5 4 16 P\nRECT 22 7 1 12 U\n";

fn msgs(text: &str) -> Vec<String> {
    validate_text("items.dsl", text)
        .1
        .into_iter()
        .map(|i| i.msg)
        .collect()
}

fn rejects(text: &str, needle: &str) {
    let m = msgs(text);
    assert!(
        m.iter().any(|s| s.contains(needle)),
        "wanted `{needle}` in {m:?}"
    );
}

#[test]
fn accepts_a_good_sprite() {
    assert_eq!(msgs(OK), Vec::<String>::new());
}

#[test]
fn rejects_too_many_commands() {
    let mut t = String::from("sprite 100 a\nsize 96 96\nlayer boots\n");
    for i in 0..49 {
        t += &format!("PIXEL {} 5 P\n", i + 2);
    }
    rejects(&t, "commands, at most 48");
}

#[test]
fn rejects_too_many_shades_and_sockets() {
    let mut t = String::from("sprite 100 a\nsize 96 96\nlayer boots\nRECT 2 2 20 20 P\n");
    for i in 0..9 {
        t += &format!("SHADE {} 5 P 1\n", i + 3);
    }
    rejects(&t, "SHADE commands");
    let mut t = String::from("sprite 100 a\nsize 96 96\nlayer boots\nRECT 2 2 28 28 P\n");
    for i in 0..5 {
        t += &format!("SOCKET {} 4\n", 2 + i * 5);
    }
    rejects(&t, "SOCKET commands");
}

#[test]
fn rejects_edge_contact_unless_no_outline() {
    let t = "sprite 100 a\nsize 96 96\nlayer boots\nRECT 0 5 3 3 P\n";
    rejects(t, "touches the canvas edge");
    let t = "sprite 100 a\nsize 96 96\nlayer shadow\nflags no_outline\nRECT 0 5 3 3 P\n";
    assert!(!msgs(t).iter().any(|s| s.contains("edge")));
}

#[test]
fn rejects_bad_line_slope() {
    rejects(
        "sprite 100 a\nsize 96 96\nlayer boots\nLINE 3 3 10 6 P\n",
        "slope",
    );
    let t = "sprite 100 a\nsize 96 96\nlayer boots\nLINE 3 3 11 7 P\nLINE 3 10 3 20 P\nLINE 5 10 9 14 P\n";
    assert!(!msgs(t).iter().any(|s| s.contains("slope")));
}

#[test]
fn rejects_missing_no_outline() {
    rejects(
        "sprite 100 a\nsize 96 96\nlayer shadow\nRECT 10 63 12 1 P\n",
        "no_outline",
    );
    rejects(
        "sprite 500 tile_floor\nsize 64 64\nlayer tile\nRECT 4 4 4 4 P\n",
        "no_outline",
    );
}

#[test]
fn rejects_wrong_size_for_layer() {
    rejects(
        "sprite 100 a\nsize 32 32\nlayer boots\nRECT 2 2 3 3 P\n",
        "not allowed for layer",
    );
    let ok = "sprite 400 big\nsize 96 96\nlayer monster\nRECT 4 4 3 3 K\n";
    assert!(!validate_text("monsters.dsl", ok)
        .1
        .iter()
        .any(|i| i.msg.contains("size")));
}

#[test]
fn rejects_wrong_slot_letters_for_layer_kind() {
    rejects(
        "sprite 100 a\nsize 96 96\nlayer boots\nRECT 2 2 3 3 K\n",
        "not allowed on a item",
    );
    rejects(
        "sprite 100 a\nsize 96 96\nlayer body\nRECT 2 2 3 3 P\n",
        "not allowed on a body",
    );
}

#[test]
fn rejects_sockets_outside_p_and_rune_problems() {
    rejects(
        "sprite 100 a\nsize 96 96\nlayer boots\nRECT 2 2 6 6 S\nSOCKET 3 3\n",
        "P mask",
    );
    // The socket square is 4x4: 3x3 of it inside P is not enough.
    rejects(
        "sprite 100 a\nsize 96 96\nlayer boots\nRECT 2 2 4 4 P\nSOCKET 3 3\n",
        "4x4 square is not inside the P mask",
    );
    assert!(msgs("sprite 100 a\nsize 96 96\nlayer boots\nRECT 2 2 6 6 P\nSOCKET 3 3\n").is_empty());
    rejects(
        "sprite 100 a\nsize 96 96\nlayer boots\nRECT 2 2 10 10 P\nSOCKET 3 3\nSOCKET 6 6\n",
        "overlaps another socket",
    );
    // 11 px of U on a weapon is too short, 12 is enough (OK).
    rejects(
        "sprite 100 a\nsize 96 96\nlayer mainhand\nRECT 22 5 3 16 P\nRECT 23 7 1 11 U\n",
        "at least 12",
    );
    rejects(
        "sprite 100 a\nsize 96 96\nlayer mainhand\nRECT 22 5 4 12 P\nPIXEL 22 5 U\n",
        "inside the P mask",
    );
}

#[test]
fn portrait_with_runes_keeps_the_glow_margin() {
    rejects(
        "sprite 100 a\nsize 64 64\nlayer portrait\nRECT 5 10 6 30 P\nRECT 7 12 1 12 U\n",
        "keeps 6 px from the edges",
    );
    // 6 px from the edge is fine, and without a rune zone the margin is the outline's 1 px.
    assert!(
        msgs("sprite 100 a\nsize 64 64\nlayer portrait\nRECT 6 10 6 30 P\nRECT 8 12 1 12 U\n")
            .is_empty()
    );
    assert!(msgs("sprite 100 a\nsize 64 64\nlayer portrait\nRECT 1 10 6 30 P\n").is_empty());
}

#[test]
fn rejects_ids_out_of_range_and_duplicates() {
    rejects(
        "sprite 450 a\nsize 96 96\nlayer boots\nRECT 2 2 3 3 P\n",
        "outside the range 100-399",
    );
    rejects(
        "sprite 1200 a\nsize 96 96\nlayer boots\nRECT 2 2 3 3 P\n",
        "outside the range 100-399",
    );
    let (_, i) = validate_text(
        "x.dsl",
        "sprite 1200 a\nsize 96 96\nlayer boots\nRECT 2 2 3 3 P\n",
    );
    assert!(i.iter().any(|i| i.msg.contains("1..=999")), "{i:?}");
    let files = vec![
        ("items.dsl".to_string(), OK.to_string()),
        ("other.dsl".to_string(), OK.to_string()),
    ];
    let m: Vec<String> = validate_files(&files)
        .1
        .into_iter()
        .map(|i| i.msg)
        .collect();
    assert!(m.iter().any(|s| s.contains("id 100 already used")), "{m:?}");
    assert!(m.iter().any(|s| s.contains("name already used")), "{m:?}");
}

#[test]
fn id_ranges_follow_the_category_folder() {
    use art::validate::file_range;
    let tile = "sprite {id} a\nsize 64 64\nlayer tile\nRECT 2 2 3 3 P\n";
    let ok = |file: &str, id: u16| {
        validate_text(file, &tile.replace("{id}", &id.to_string()))
            .1
            .iter()
            .all(|i| !i.msg.contains("outside"))
    };
    assert!(ok("assets/sprites/tiles/caves/walls.dsl", 520));
    assert!(ok("assets/sprites/tiles/crypt/floor.dsl", 810));
    assert!(ok("assets/sprites/points/chest.dsl", 580));
    assert!(!ok("assets/sprites/tiles/forest/floor.dsl", 700));
    assert!(!ok("assets/sprites/points/chest.dsl", 1000));
    assert!(ok("assets/site_sprites/buildings/tavern.dsl", 1000));
    assert!(!ok("assets/site_sprites/buildings/tavern.dsl", 999));
    assert_eq!(
        file_range("assets/sprites/resources/ores.dsl"),
        Some(&[(600, 799)][..])
    );
    assert_eq!(
        file_range("assets/sprites/monsters/monsters.dsl"),
        Some(&[(400, 499)][..])
    );
    assert_eq!(file_range("x.dsl"), None);
}

#[test]
fn reports_compile_errors_with_a_line() {
    let (_, i) = validate_text("x.dsl", "sprite 1 a\nsize 96 96\nlayer boots\nBLOB 1\n");
    assert!(i.iter().any(|i| i.line == 4), "{i:?}");
}

#[test]
fn inline_comment_is_not_a_comment() {
    // `#` after a command is a token that is not at line start of a statement: the compiler decides.
    let (_, i) = validate_text(
        "x.dsl",
        "# whole line comment\nsprite 1 a\nsize 96 96\nlayer boots\nRECT 2 2 3 3 P\n",
    );
    assert!(i.iter().all(|i| !i.msg.contains("slot")), "{i:?}");
}

#[test]
fn all_repo_sprites_are_valid_and_render_deterministically() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    let paths = art::validate::dsl_files(&root.join("assets/sprites"));
    for p in &paths {
        files.push((
            p.to_string_lossy().replace('\\', "/"),
            std::fs::read_to_string(p).unwrap(),
        ));
    }
    let (sprites, issues) = validate_files(&files);
    assert!(
        issues.is_empty(),
        "{:#?}",
        issues.iter().map(|i| i.to_string()).collect::<Vec<_>>()
    );
    let pal =
        Pal::load(&std::fs::read_to_string(root.join("assets/palette/palette_2.pal")).unwrap())
            .unwrap();
    let book = preview::pack_book(&sprites).unwrap();
    for c in &sprites {
        let a = preview::render_sprite(&pal, &book, c, 2, None).unwrap();
        let b = preview::render_sprite(&pal, &book, c, 2, None).unwrap();
        assert_eq!(a.rgba, b.rgba, "{} not deterministic", c.src.name);
        assert!(
            a.rgba.chunks(4).any(|p| p[3] != 0),
            "{} renders empty",
            c.src.name
        );
    }
    for (name, layers) in preview::characters() {
        for dead in [false, true] {
            let img = preview::render_character(&pal, &book, &layers, 1, dead).unwrap();
            assert!(img.w == img.h && matches!(img.w, 64 | 96), "{name}");
        }
    }
    // The craft matrix renders for every item sprite.
    for c in sprites.iter().filter(|c| {
        let lt = c.sprite.header.layer_type;
        !deadroll_render::sprite::layer::is_body(lt)
            && lt <= deadroll_render::sprite::layer::CHARACTER_MAX
    }) {
        let row = preview::craft_row(&pal, &book, c, 1).unwrap();
        assert_eq!(row.len(), preview::MATRIX_COLUMNS.len(), "{}", c.src.name);
    }
}

#[test]
fn tiles_use_tile_materials_and_figures_wear_the_starter_kits() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pal =
        Pal::load(&std::fs::read_to_string(root.join("assets/palette/palette_2.pal")).unwrap())
            .unwrap();
    // The current map's tiles are the Caves and Ruins set (TILE_LOOKS); the old trap keeps its look.
    for (name, want) in [
        ("tile_wall", "cave_wall"),
        ("tile_water", "murk"),
        ("tile_door", "cave_door"),
        ("tile_floor", "cave_floor"),
        ("cave_wall_5", "cave_wall"),
        ("cave_dirt_1", "cave_floor"),
        ("tile_trap", "tile_floor"),
    ] {
        let look = preview::default_look(name, 20);
        assert_eq!(look.material, want, "{name}");
        assert!(
            pal.src.material(look.material).is_some(),
            "{want} missing in palette"
        );
    }
    // A figure is its body and hair and the items it wears: the class starter kit of Gear::starter,
    // one sprite per ItemBase (id 100 + base) in its slot's layer.
    use deadroll_render::sprite::layer;
    for (figure, items) in [
        (
            "human_warrior",
            [
                (layer::ARMOR_BODY, 103, "leather"),
                (layer::MAINHAND, 100, "steel"),
            ],
        ),
        (
            "elf_mage",
            [
                (layer::ARMOR_BODY, 104, "robe"),
                (layer::MAINHAND, 101, "leather"),
            ],
        ),
        (
            "human_support",
            [
                (layer::ARMOR_BODY, 105, "leather"),
                (layer::MAINHAND, 102, "leather"),
            ],
        ),
    ] {
        let (_, layers) = preview::characters()
            .into_iter()
            .find(|c| c.0 == figure)
            .unwrap();
        let worn: Vec<(u8, u16, &str)> = layers
            .iter()
            .filter(|l| l.1 >= 100)
            .map(|l| (l.0, l.1, l.2.material))
            .collect();
        assert_eq!(worn, items.to_vec(), "{figure}");
        assert_eq!(
            layers.len(),
            6,
            "{figure}: shadow, body, hair, fist and two items"
        );
        for (_, _, m) in items {
            assert!(pal.src.material(m).is_some(), "{m} missing in palette");
        }
    }
}

#[test]
fn format_2_sizes_per_layer() {
    // Format 1 sizes no longer compile at all.
    rejects(
        "sprite 400 a\nsize 48 48\nlayer monster\nRECT 4 4 3 3 K\n",
        "size must be 32, 64, 96 or 128",
    );
    for bad in [
        "sprite 400 a\nsize 32 32\nlayer monster\nRECT 4 4 3 3 K\n",
        "sprite 400 a\nsize 64 128\nlayer monster\nRECT 4 4 3 3 K\n",
        "sprite 500 a\nsize 32 32\nlayer tile\nRECT 4 4 3 3 P\n",
        "sprite 100 a\nsize 64 64\nlayer icon\nRECT 4 4 3 3 P\n",
    ] {
        let file = if bad.contains("tile") {
            "tiles.dsl"
        } else {
            "monsters.dsl"
        };
        let issues = validate_text(file, bad).1;
        assert!(
            issues
                .iter()
                .any(|i| i.msg.contains("not allowed for layer")),
            "{bad}: {issues:?}"
        );
    }
    for ok in [
        "sprite 400 a\nsize 128 128\nlayer monster\nRECT 4 4 3 3 K\n",
        "sprite 500 a\nsize 64 64\nlayer tile\nRECT 4 4 3 3 P\n",
        "sprite 100 a\nsize 32 32\nlayer icon\nRECT 4 4 3 3 P\n",
        "sprite 100 a\nsize 32 96\nlayer portrait\nRECT 4 4 3 3 P\n",
    ] {
        let file = if ok.contains("tile") {
            "tiles.dsl"
        } else {
            "monsters.dsl"
        };
        let issues = validate_text(file, ok).1;
        assert!(
            !issues.iter().any(|i| i.msg.contains("size")),
            "{ok}: {issues:?}"
        );
    }
}

#[test]
fn bitmap_rows_use_the_layer_slot_letters() {
    let item = "sprite 100 a\nsize 96 96\nlayer boots\nBITMAP 10 10 3 2\nPS.\n04h\n";
    assert_eq!(msgs(item), Vec::<String>::new());
    rejects(
        "sprite 100 a\nsize 96 96\nlayer boots\nBITMAP 10 10 3 1\nPK.\n",
        "BITMAP slot `K` not allowed on a item layer",
    );
    let body = "sprite 2 a\nsize 96 96\nlayer body\nBITMAP 10 10 2 1\nKY\n";
    let m = validate_text("bodies.dsl", body).1;
    assert!(m.is_empty(), "{m:?}");
    // Character layers are 96 (or 64 until redrawn), never 128.
    rejects(
        "sprite 100 a\nsize 128 128\nlayer boots\nRECT 10 10 4 4 P\n",
        "size 128x128 not allowed",
    );
}

#[test]
fn pack_splits_books_at_64_kib() {
    // Nine different 96 px sprites of about 8.8 KB each: a checkerboard defeats the run-length
    // coding, the accent pixel at x = i keeps them from sharing bytes.
    let mut text = String::new();
    for i in 0..9 {
        text += &format!(
            "sprite {} s{i}\nsize 96 96\nlayer boots\nBITMAP 1 1 94 94\n",
            100 + i
        );
        for y in 0..94 {
            let row: String = (0..94)
                .map(|x| match (x, y) {
                    (x, 0) if x == i => 'A',
                    _ if (x + y) % 2 == 0 => 'P',
                    _ => 'S',
                })
                .collect();
            text += &row;
            text += "\n";
        }
    }
    let (sprites, issues) = validate_text("items.dsl", &text);
    assert!(issues.is_empty(), "{issues:?}");
    let books = preview::pack_books(&sprites, preview::BOOK_ID).unwrap();
    assert!(books.len() >= 2, "{} books", books.len());
    for (i, b) in books.iter().enumerate() {
        assert!(b.len() <= preview::BOOK_MAX);
        let book = deadroll_render::Book::parse(b).unwrap();
        assert_eq!(book.book_id, preview::BOOK_ID + i as u16);
    }
    let total: usize = books
        .iter()
        .map(|b| deadroll_render::Book::parse(b).unwrap().len())
        .sum();
    assert_eq!(total, 9);
    // A later first id (a new book after a frozen one) numbers the same split from there.
    let later = preview::pack_books(&sprites, 5).unwrap();
    assert_eq!(later.len(), books.len());
    for (i, b) in later.iter().enumerate() {
        assert_eq!(
            deadroll_render::Book::parse(b).unwrap().book_id,
            5 + i as u16
        );
    }
}

#[test]
fn every_frame_fits_an_indexed_png() {
    // Error 10: a frame with more than 256 colors has no indexed PNG. Characters alive and dead,
    // and every crafted item column on its own canvas.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for p in art::validate::dsl_files(&root.join("assets/sprites")) {
        files.push((
            p.to_string_lossy().replace('\\', "/"),
            std::fs::read_to_string(&p).unwrap(),
        ));
    }
    let (sprites, _) = validate_files(&files);
    let pal =
        Pal::load(&std::fs::read_to_string(root.join("assets/palette/palette_2.pal")).unwrap())
            .unwrap();
    let book = preview::pack_book(&sprites).unwrap();
    let colors = |img: &preview::Img| {
        let mut set: Vec<&[u8]> = img.rgba.chunks_exact(4).collect();
        set.sort();
        set.dedup();
        set.len()
    };
    let mut most = (0, String::new());
    for (name, layers) in preview::characters() {
        for dead in [false, true] {
            let img = preview::render_character(&pal, &book, &layers, 1, dead).unwrap();
            let n = colors(&img);
            assert!(n <= 256, "{name} dead={dead}: {n} colors");
            if n > most.0 {
                most = (n, format!("{name} dead={dead}"));
            }
        }
    }
    for c in sprites.iter().filter(|c| {
        let lt = c.sprite.header.layer_type;
        !deadroll_render::sprite::layer::is_body(lt)
            && lt <= deadroll_render::sprite::layer::CHARACTER_MAX
    }) {
        for (i, img) in preview::craft_row(&pal, &book, c, 1)
            .unwrap()
            .iter()
            .enumerate()
        {
            let n = colors(img);
            assert!(n <= 256, "{} column {i}: {n} colors", c.src.name);
            if n > most.0 {
                most = (n, format!("{} column {i}", c.src.name));
            }
        }
    }
    println!("most colors in one frame: {} ({})", most.0, most.1);
}

#[test]
fn front_fist_repeats_the_fist_of_every_body() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(root.join("assets/sprites/bodies/bodies.dsl")).unwrap();
    let (sprites, issues) = validate_text("bodies.dsl", &text);
    assert!(issues.is_empty(), "{issues:?}");
    let grid = |name: &str| {
        let c = sprites.iter().find(|c| c.src.name == name).unwrap();
        (c, deadroll_render::raster::rasterize(&c.sprite))
    };
    let (fist, fg) = grid("fist_front");
    assert!(fist.sprite.header.no_outline());
    let cells: Vec<_> = fg
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| c.slot != 0)
        .collect();
    assert!(cells.len() > 20, "the fist is drawn");
    for body in ["human_body", "elf_body"] {
        let (_, bg) = grid(body);
        for &(i, c) in &cells {
            assert_eq!(
                bg.cells[i],
                *c,
                "{body}: fist pixel ({}, {})",
                i % 96,
                i / 96
            );
        }
    }
    // Without no_outline the validator refuses a front fist.
    let bad = "sprite 14 fist_front\nsize 96 96\nlayer fist_front\nRECT 60 60 4 4 K\n";
    assert!(validate_text("bodies.dsl", bad)
        .1
        .iter()
        .any(|i| i.msg.contains("no_outline")));
}

#[test]
fn every_character_layer_and_monster_is_redrawn_at_96() {
    // The 64 px canvases of format 2 before the 96 px characters are no longer accepted.
    rejects(
        "sprite 100 a\nsize 64 64\nlayer boots\nRECT 10 10 4 4 P\n",
        "size 64x64 not allowed",
    );
    rejects(
        "sprite 400 a\nsize 64 64\nlayer monster\nRECT 10 10 4 4 K\n",
        "size 64x64 not allowed",
    );
    let big = "sprite 400 a\nsize 128 128\nlayer monster\nRECT 10 10 4 4 K\n";
    assert!(validate_text("monsters.dsl", big).1.is_empty());
}
