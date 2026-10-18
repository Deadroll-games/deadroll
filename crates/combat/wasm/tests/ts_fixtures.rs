//! Golden fixtures for the website's combat WASM wrapper (`app/src/chain/combatWasm.ts`).
//! Test-only: no crate logic lives here.
//!
//! Each case holds a structured input (numbers in field order), its borsh bytes and what the
//! exports return for it. The app test checks that its encoder writes the same bytes and that the
//! committed `combat.wasm` returns the same results. When a rule or a layout changes, regenerate:
//!
//! ```text
//! UPDATE_TS_FIXTURES=1 cargo test -p deadroll-combat-wasm --test ts_fixtures
//! ```
//!
//! Then rebuild the shipped module (`bash crates/combat/wasm/build.sh`).

use std::path::PathBuf;

use deadroll_combat::{ModKind, TimedMod, TimedMods, MODS_MAX};
use deadroll_combat_wasm::{
    attack_input, hero_sheet, AttackRequest, HeroInput, HeroSheet, SkillInput, TargetInput,
};
use deadroll_rules::{items::starter_loadout, BalanceConfig};
use deadroll_skillcheck::{derive, presets, Category, Purpose};
use deadroll_types::{ActiveState, Aspect, Class, Race, StateKind, STATES_MAX};
use serde_json::{json, Value};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../app/src/chain/__fixtures__/combat-wasm.json")
}

const CFG: &BalanceConfig = &BalanceConfig::DEFAULT;

const NO_MODS: TimedMods = [TimedMod {
    kind: ModKind::Haste,
    aspect: Aspect::Fire,
    turns: 0,
    power_bp: 0,
    source: 0,
}; MODS_MAX];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hero_json(h: &HeroInput) -> Value {
    json!({
        "race": h.race as u8,
        "class": h.class as u8,
        "level": h.level,
        "stats": h.stats,
        "predisposition": h.predisposition as u8,
        "predispositionTier": h.predisposition_tier,
        "classTier": h.class_tier,
        "equipment": h.equipment,
    })
}

fn mods_json(m: &TimedMods) -> Value {
    m.iter()
        .map(|t| {
            json!({
                "kind": t.kind as u8,
                "aspect": t.aspect as u8,
                "turns": t.turns,
                "powerBp": t.power_bp,
                "source": t.source,
            })
        })
        .collect()
}

fn states_json(s: &[ActiveState; STATES_MAX]) -> Value {
    s.iter()
        .map(|a| json!({ "kind": a.kind as u8, "turns": a.turns, "power": a.power, "source": a.source }))
        .collect()
}

fn skill_json(s: &SkillInput) -> Value {
    let b = &s.blocks;
    json!({
        "blocks": [
            b.category as u8, b.purpose as u8, b.source as u8, b.form as u8, b.target as u8,
            b.aspects[0] as u8, b.aspects[1] as u8, b.aspects[2] as u8, b.aspect_count,
            b.effect as u8, b.trigger as u8, b.aura_radius,
        ],
        "sharesBp": s.shares_bp,
        "powerBp": s.power_bp,
    })
}

fn sheet_json(s: &HeroSheet) -> Value {
    json!({
        "accuracy": s.accuracy,
        "evasion": s.evasion,
        "armor": s.armor,
        "critBp": s.crit_bp,
        "hpMax": s.hp_max,
        "actionPoints": s.action_points,
        "initiative": s.initiative,
        "martialStrikeBp": s.martial_strike_bp,
        "martialProjectileBp": s.martial_projectile_bp,
        "magicalBp": s.magical_bp,
        "martialStateBp": s.martial_state_bp,
        "magicalStateBp": s.magical_state_bp,
        "tempoStateBp": s.tempo_state_bp,
        "selfBuffTurns": s.self_buff_turns,
        "critRollBp": s.crit_roll_bp,
    })
}

fn heroes() -> Vec<HeroInput> {
    let mut out = Vec::new();
    for (i, class) in Class::ALL.iter().enumerate() {
        for (j, race) in Race::ALL.iter().enumerate() {
            let predisposition = match race {
                Race::Human => Aspect::Blood,
                Race::ForestElf => Aspect::Cold,
            };
            let kit = starter_loadout(*class, &CFG.items).equipment().0;
            let n = (i * Race::COUNT + j) as u16;
            out.push(HeroInput {
                race: *race,
                class: *class,
                level: 1 + n % 4,
                stats: [5 + n * 2 % 11, 7 + n % 5, 6 + n * 3 % 9, 5 + n % 7],
                predisposition,
                predisposition_tier: 14 - (n as u8 % 3) * 5,
                class_tier: 14 - (n as u8 % 4) * 4,
                equipment: if n % 3 == 2 { [0xFF; 11] } else { kit },
            });
        }
    }
    out
}

fn requests(heroes: &[HeroInput]) -> Vec<AttackRequest> {
    let mut chill = [ActiveState::default(); STATES_MAX];
    chill[0] = ActiveState {
        kind: StateKind::Chill,
        turns: 2,
        power: 5_000,
        source: 1,
    };
    let mut empowered = NO_MODS;
    empowered[0] = TimedMod {
        kind: ModKind::Empowerment,
        aspect: Aspect::Blood,
        turns: 2,
        power_bp: 3_000,
        source: 0,
    };
    let mut protected = NO_MODS;
    protected[0] = TimedMod {
        kind: ModKind::Protection,
        aspect: Aspect::Blunt,
        turns: 2,
        power_bp: 2_500,
        source: 5,
    };
    let mut out = Vec::new();
    for (n, h) in heroes.iter().enumerate() {
        let damage_skills = presets::for_class(h.class)
            .iter()
            .filter(|p| {
                p.blocks.category == Category::Active && p.blocks.purpose == Purpose::Damage
            })
            .map(|p| {
                let d = derive(&p.blocks);
                Some(SkillInput {
                    blocks: p.blocks,
                    shares_bp: d.shares_bp,
                    power_bp: d.power_bp,
                })
            });
        for (k, skill) in core::iter::once(None).chain(damage_skills).enumerate() {
            let m = n + k;
            out.push(AttackRequest {
                hero: *h,
                hero_mods: if m % 3 == 1 { empowered } else { NO_MODS },
                skill,
                target: TargetInput {
                    bestiary_id: (m % 5) as u8,
                    level: 1 + (m % 3) as u8,
                    states: if m % 4 == 3 {
                        chill
                    } else {
                        [ActiveState::default(); STATES_MAX]
                    },
                    mods: if m % 5 == 4 { protected } else { NO_MODS },
                },
            });
        }
    }
    out
}

fn build() -> Value {
    let heroes = heroes();
    let sheets: Vec<Value> = heroes
        .iter()
        .map(|h| {
            json!({
                "input": hero_json(h),
                "bytes": hex(&borsh::to_vec(h).unwrap()),
                "sheet": sheet_json(&hero_sheet(&h.hero())),
            })
        })
        .collect();
    let attacks: Vec<Value> = requests(&heroes)
        .iter()
        .map(|r| {
            let p = deadroll_combat::preview_hit(&attack_input(r).unwrap(), &CFG.combat);
            json!({
                "input": {
                    "hero": hero_json(&r.hero),
                    "heroMods": mods_json(&r.hero_mods),
                    "skill": r.skill.as_ref().map(skill_json),
                    "target": {
                        "bestiaryId": r.target.bestiary_id,
                        "level": r.target.level,
                        "states": states_json(&r.target.states),
                        "mods": mods_json(&r.target.mods),
                    },
                },
                "bytes": hex(&borsh::to_vec(r).unwrap()),
                "preview": {
                    "immune": p.immune,
                    "hitBp": p.hit_bp,
                    "critBp": p.crit_bp,
                    "min": p.min,
                    "max": p.max,
                    "critMin": p.crit_min,
                    "critMax": p.crit_max,
                },
            })
        })
        .collect();
    json!({
        "abiVersion": deadroll_combat_wasm::ABI_VERSION,
        "sheets": sheets,
        "attacks": attacks,
    })
}

#[test]
fn ts_fixtures_are_current() {
    let path = fixture_path();
    let text = serde_json::to_string_pretty(&build()).unwrap() + "\n";
    if std::env::var_os("UPDATE_TS_FIXTURES").is_some() {
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        committed == text,
        "{} is stale or missing: run UPDATE_TS_FIXTURES=1 cargo test -p deadroll-combat-wasm \
         --test ts_fixtures, then rebuild combat.wasm (bash crates/combat/wasm/build.sh)",
        path.display()
    );
}
