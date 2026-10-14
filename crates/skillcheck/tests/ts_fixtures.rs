//! Golden fixtures for the TypeScript mirror of this crate (`app/src/chain/skillcheck.ts`).
//! Test-only: no crate logic lives here.
//!
//! The test builds the expected `validate`, `validate_starting` (every class), `derive` and
//! `check_loadout` results for the presets, plus a seeded set of random blocks, loadouts and map
//! selections (`check_map_selection`), and
//! compares them with the committed JSON. When a rule changes, regenerate it and update the TS mirror:
//!
//! ```text
//! UPDATE_TS_FIXTURES=1 cargo test -p deadroll-skillcheck --test ts_fixtures
//! ```
//!
//! Blocks are encoded as discriminants in `SkillBlocks` field order:
//! `[category, purpose, source, form, target, a0, a1, a2, aspect_count, effect, trigger, aura_radius]`.

use std::fmt::Write as _;
use std::path::PathBuf;

use deadroll_skillcheck::{
    check_loadout, check_map_selection, derive, presets, validate, validate_starting, Category,
    Effect, MapSelection, Purpose, SkillBlocks, SkillError, Target, Trigger, NO_SKILL,
    SELECTION_SLOTS, SKILL_ASPECTS_MAX, STARTING_MAX,
};
use deadroll_types::{Aspect, Class, Form, Source};

const RANDOM_SKILLS: usize = 2_000;
const RANDOM_LOADOUTS: usize = 200;
const RANDOM_SELECTIONS: usize = 400;
/// Fixed seed; changing it rewrites the fixture.
const SEED: u64 = 0xD3AD_0011_5EED_0001;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../app/src/chain/__fixtures__/skillcheck.json")
}

/// Small deterministic generator (SplitMix64), so the fixture is stable across runs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// Random blocks biased toward valid shapes, so every rule is hit both ways.
fn random_blocks(rng: &mut Rng) -> SkillBlocks {
    let category = Category::ALL[rng.below(Category::COUNT)];
    let purpose = Purpose::ALL[rng.below(Purpose::COUNT)];
    let aspect_count = match rng.below(20) {
        0 => 0,
        1 => 4,
        n => 1 + (n % 3) as u8,
    };
    let mut aspects = [Aspect::Fire; SKILL_ASPECTS_MAX];
    for (i, a) in aspects.iter_mut().enumerate() {
        // Unused slots stay at the default most of the time (else UnusedAspectSet dominates).
        if i < aspect_count as usize || rng.chance(5) {
            *a = Aspect::ALL[rng.below(Aspect::COUNT)];
        }
    }
    let effect = if rng.chance(40) {
        Effect::None
    } else {
        Effect::ALL[rng.below(Effect::COUNT)]
    };
    let trigger = if category == Category::Reactive {
        if rng.chance(10) {
            Trigger::None
        } else {
            Trigger::ALL[rng.below(Trigger::COUNT)]
        }
    } else if rng.chance(10) {
        Trigger::ALL[rng.below(Trigger::COUNT)]
    } else {
        Trigger::None
    };
    let aura_radius = if rng.chance(75) {
        0
    } else {
        rng.below(5) as u8
    };
    SkillBlocks {
        category,
        purpose,
        source: Source::ALL[rng.below(Source::COUNT)],
        form: Form::ALL[rng.below(Form::COUNT)],
        target: Target::ALL[rng.below(Target::COUNT)],
        aspects,
        aspect_count,
        effect,
        trigger,
        aura_radius,
    }
}

fn encode(s: &SkillBlocks) -> String {
    let v = [
        s.category as u8,
        s.purpose as u8,
        s.source as u8,
        s.form as u8,
        s.target as u8,
        s.aspects[0] as u8,
        s.aspects[1] as u8,
        s.aspects[2] as u8,
        s.aspect_count,
        s.effect as u8,
        s.trigger as u8,
        s.aura_radius,
    ];
    format!("[{}]", v.map(|x| x.to_string()).join(","))
}

fn result(r: Result<(), SkillError>) -> String {
    match r {
        Ok(()) => "\"Ok\"".into(),
        Err(e) => format!("\"{e:?}\""),
    }
}

fn skill_line(s: &SkillBlocks) -> String {
    let d = derive(s);
    let starting: Vec<String> = Class::ALL
        .iter()
        .map(|c| result(validate_starting(s, *c)))
        .collect();
    format!(
        "{{\"blocks\":{},\"validate\":{},\"starting\":[{}],\"derive\":{{\"sharesBp\":[{},{},{}],\"powerBp\":{},\"fusion\":{},\"cooldown\":{}}}}}",
        encode(s),
        result(validate(s)),
        starting.join(","),
        d.shares_bp[0],
        d.shares_bp[1],
        d.shares_bp[2],
        d.power_bp,
        d.fusion,
        d.cooldown,
    )
}

/// Builds the whole fixture and the set of errors it covers.
fn build() -> (String, Vec<String>) {
    let mut covered = Vec::new();
    let mut note = |r: Result<(), SkillError>| {
        if let Err(e) = r {
            let name = format!("{e:?}");
            if !covered.contains(&name) {
                covered.push(name);
            }
        }
    };

    let mut out = String::from(
        "{\n\"generatedBy\":\"crates/skillcheck/tests/ts_fixtures.rs\",\n\"presets\":[\n",
    );
    let mut lines = Vec::new();
    for class in Class::ALL {
        for p in presets::for_class(*class) {
            lines.push(format!(
                "{{\"class\":{},\"name\":\"{}\",\"skill\":{}}}",
                *class as u8,
                p.name,
                skill_line(&p.blocks)
            ));
        }
    }
    out += &lines.join(",\n");

    out += "\n],\n\"skills\":[\n";
    let mut rng = Rng(SEED);
    let mut lines = Vec::with_capacity(RANDOM_SKILLS);
    for _ in 0..RANDOM_SKILLS {
        let s = random_blocks(&mut rng);
        note(validate(&s));
        for c in Class::ALL {
            note(validate_starting(&s, *c));
        }
        lines.push(skill_line(&s));
    }
    out += &lines.join(",\n");

    out += "\n],\n\"loadouts\":[\n";
    let mut lines = Vec::with_capacity(RANDOM_LOADOUTS);
    for _ in 0..RANDOM_LOADOUTS {
        let n = rng.below(8);
        let cats: Vec<Category> = (0..n)
            .map(|_| Category::ALL[rng.below(Category::COUNT)])
            .collect();
        let blocks: Vec<SkillBlocks> = cats
            .iter()
            .map(|c| SkillBlocks {
                category: *c,
                ..Default::default()
            })
            .collect();
        let r = check_loadout(blocks.iter());
        note(r);
        let list: Vec<String> = cats.iter().map(|c| (*c as u8).to_string()).collect();
        lines.push(format!(
            "{{\"categories\":[{}],\"result\":{}}}",
            list.join(","),
            result(r)
        ));
    }
    out += &lines.join(",\n");
    out += "\n],\n\"selections\":[\n";
    let mut lines = Vec::with_capacity(RANDOM_SELECTIONS);
    for _ in 0..RANDOM_SELECTIONS {
        let owned: Vec<Category> = (0..rng.below(10))
            .map(|_| Category::ALL[rng.below(Category::COUNT)])
            .collect();
        // A valid selection (the first skills of each category), then maybe a mutation.
        let mut sel: MapSelection = [NO_SKILL; STARTING_MAX];
        for (category, start, slots) in SELECTION_SLOTS {
            let mine = owned.iter().enumerate().filter(|(_, c)| **c == category);
            for (slot, (index, _)) in sel[start..start + slots].iter_mut().zip(mine) {
                *slot = index as u8;
            }
        }
        if rng.chance(60) {
            let a = rng.below(STARTING_MAX);
            if rng.chance(50) {
                sel.swap(a, rng.below(STARTING_MAX));
            } else {
                let v = rng.below(owned.len() + 3);
                sel[a] = if v == owned.len() + 2 {
                    NO_SKILL
                } else {
                    v as u8
                };
            }
        }
        let r = check_map_selection(&owned, &sel);
        note(r);
        let list: Vec<String> = owned.iter().map(|c| (*c as u8).to_string()).collect();
        let picked: Vec<String> = sel.iter().map(|i| i.to_string()).collect();
        lines.push(format!(
            "{{\"categories\":[{}],\"selection\":[{}],\"result\":{}}}",
            list.join(","),
            picked.join(","),
            result(r)
        ));
    }
    out += &lines.join(",\n");
    out += "\n]\n}\n";
    (out, covered)
}

#[test]
fn ts_fixtures_are_current() {
    let (json, covered) = build();

    // Every rule must be hit at least once, or the TS mirror is not fully checked.
    let all = [
        "AspectCount",
        "RepeatedAspect",
        "UnusedAspectSet",
        "ThirdAspectTarget",
        "EffectNeedsOneAspect",
        "EffectKind",
        "MissingEffect",
        "HardControlAspect",
        "MissingTrigger",
        "UnexpectedTrigger",
        "PassiveDamage",
        "AuraCategory",
        "AuraEffect",
        "AuraRadius",
        "ClassSource",
        "ClassPurpose",
        "TooManyActive",
        "TooManyReactive",
        "TooManyPassive",
        "SelectionUnknownSkill",
        "SelectionRepeated",
        "SelectionCategory",
        "SelectionCount",
    ];
    let missing: Vec<&str> = all
        .iter()
        .copied()
        .filter(|e| !covered.iter().any(|c| c == e))
        .collect();
    assert!(missing.is_empty(), "random cases never hit: {missing:?}");

    let path = fixture_path();
    if std::env::var_os("UPDATE_TS_FIXTURES").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &json).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    let mut hint = String::new();
    let _ = write!(hint, "{} is stale or missing: run UPDATE_TS_FIXTURES=1 cargo test -p deadroll-skillcheck --test ts_fixtures, then update app/src/chain/skillcheck.ts", path.display());
    assert!(committed == json, "{hint}");
}
