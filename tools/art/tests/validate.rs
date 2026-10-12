use art::preview::{self, Pal};
use art::validate::{validate_files, validate_text};

const OK: &str =
    "sprite 100 warrior_ok\nsize 32 32\nlayer mainhand\nRECT 21 5 4 12 P\nRECT 22 7 1 8 U\n";

fn msgs(text: &str) -> Vec<String> {
    validate_text("warrior_kit.dsl", text)
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
    let mut t = String::from("sprite 100 a\nsize 32 32\nlayer boots\n");
    for i in 0..25 {
        t += &format!("PIXEL {} 5 P\n", i + 2);
    }
    rejects(&t, "commands, at most 24");
}

#[test]
fn rejects_too_many_shades_and_sockets() {
    let mut t = String::from("sprite 100 a\nsize 32 32\nlayer boots\nRECT 2 2 20 20 P\n");
    for i in 0..5 {
        t += &format!("SHADE {} 5 P 1\n", i + 3);
    }
    rejects(&t, "SHADE commands");
    let mut t = String::from("sprite 100 a\nsize 32 32\nlayer boots\nRECT 2 2 28 28 P\n");
    for i in 0..5 {
        t += &format!("SOCKET {} 4\n", 2 + i * 3);
    }
    rejects(&t, "SOCKET commands");
}

#[test]
fn rejects_edge_contact_unless_no_outline() {
    let t = "sprite 100 a\nsize 32 32\nlayer boots\nRECT 0 5 3 3 P\n";
    rejects(t, "touches the canvas edge");
    let t = "sprite 100 a\nsize 32 32\nlayer shadow\nflags no_outline\nRECT 0 5 3 3 P\n";
    assert!(!msgs(t).iter().any(|s| s.contains("edge")));
}

#[test]
fn rejects_bad_line_slope() {
    rejects(
        "sprite 100 a\nsize 32 32\nlayer boots\nLINE 3 3 10 6 P\n",
        "slope",
    );
    let t = "sprite 100 a\nsize 32 32\nlayer boots\nLINE 3 3 11 7 P\nLINE 3 10 3 20 P\nLINE 5 10 9 14 P\n";
    assert!(!msgs(t).iter().any(|s| s.contains("slope")));
}

#[test]
fn rejects_missing_no_outline() {
    rejects(
        "sprite 100 a\nsize 32 32\nlayer shadow\nRECT 10 31 12 1 P\n",
        "no_outline",
    );
    rejects(
        "sprite 500 tile_floor\nsize 32 32\nlayer tile\nRECT 4 4 4 4 P\n",
        "no_outline",
    );
}

#[test]
fn rejects_wrong_size_for_layer() {
    rejects(
        "sprite 100 a\nsize 16 16\nlayer boots\nRECT 2 2 3 3 P\n",
        "not allowed for layer",
    );
    let ok = "sprite 400 big\nsize 48 48\nlayer monster\nRECT 4 4 3 3 K\n";
    assert!(!validate_text("monsters.dsl", ok)
        .1
        .iter()
        .any(|i| i.msg.contains("size")));
}

#[test]
fn rejects_wrong_slot_letters_for_layer_kind() {
    rejects(
        "sprite 100 a\nsize 32 32\nlayer boots\nRECT 2 2 3 3 K\n",
        "not allowed on a item",
    );
    rejects(
        "sprite 100 a\nsize 32 32\nlayer torso\nRECT 2 2 3 3 P\n",
        "not allowed on a body",
    );
}

#[test]
fn rejects_sockets_outside_p_and_rune_problems() {
    rejects(
        "sprite 100 a\nsize 32 32\nlayer boots\nRECT 2 2 6 6 S\nSOCKET 3 3\n",
        "P mask",
    );
    rejects(
        "sprite 100 a\nsize 32 32\nlayer mainhand\nRECT 22 5 2 12 P\nRECT 22 7 1 3 U\n",
        "at least 6",
    );
    rejects(
        "sprite 100 a\nsize 32 32\nlayer mainhand\nRECT 22 5 4 12 P\nPIXEL 22 5 U\n",
        "inside the P mask",
    );
}

#[test]
fn rejects_ids_out_of_range_and_duplicates() {
    rejects(
        "sprite 250 a\nsize 32 32\nlayer boots\nRECT 2 2 3 3 P\n",
        "outside the range 100-199",
    );
    rejects(
        "sprite 700 a\nsize 32 32\nlayer boots\nRECT 2 2 3 3 P\n",
        "1..=599",
    );
    let files = vec![
        ("warrior_kit.dsl".to_string(), OK.to_string()),
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
fn reports_compile_errors_with_a_line() {
    let (_, i) = validate_text("x.dsl", "sprite 1 a\nsize 32 32\nlayer boots\nBLOB 1\n");
    assert!(i.iter().any(|i| i.line == 4), "{i:?}");
}

#[test]
fn inline_comment_is_not_a_comment() {
    // `#` after a command is a token that is not at line start of a statement: the compiler decides.
    let (_, i) = validate_text(
        "x.dsl",
        "# whole line comment\nsprite 1 a\nsize 32 32\nlayer boots\nRECT 2 2 3 3 P\n",
    );
    assert!(i.iter().all(|i| !i.msg.contains("slot")), "{i:?}");
}

#[test]
fn all_repo_sprites_are_valid_and_render_deterministically() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    let mut paths: Vec<_> = std::fs::read_dir(root.join("assets/sprites"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "dsl"))
        .collect();
    paths.sort();
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
        Pal::load(&std::fs::read_to_string(root.join("assets/palette/palette_1.pal")).unwrap())
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
        let img = preview::render_character(&pal, &book, &layers, 1).unwrap();
        assert!(img.w == 32 && img.h == 32, "{name}");
    }
}

#[test]
fn tiles_use_tile_materials_and_support_is_leather() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pal =
        Pal::load(&std::fs::read_to_string(root.join("assets/palette/palette_1.pal")).unwrap())
            .unwrap();
    for (name, want) in [
        ("tile_wall", "tile_wall"),
        ("tile_water", "tile_water"),
        ("tile_door", "tile_door"),
        ("tile_floor", "tile_floor"),
        ("tile_trap", "tile_floor"),
    ] {
        let look = preview::default_look(name, 20);
        assert_eq!(look.material, want, "{name}");
        assert!(
            pal.src.material(look.material).is_some(),
            "{want} missing in palette"
        );
    }
    let (_, layers) = preview::characters()
        .into_iter()
        .find(|c| c.0 == "human_support")
        .unwrap();
    for (ty, id, look) in layers {
        if id >= 300 {
            assert_eq!(look.material, "leather", "layer {ty} sprite {id}");
        }
    }
}
