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

use deadroll_combat::{AccuracyBuff, HitPreview, ModKind, TimedMod, TimedMods, MODS_MAX};
use deadroll_combat_wasm::{
    attack_input, hero_sheet, line, monster_sheet, unit_mods, AttackRequest, AuraInput,
    DefenseInput, HeroInput, HeroSheet, LineRequest, MonsterInput, PartyInput, SkillInput,
    TargetInput, UnitModsRequest,
};
use deadroll_rules::{items::starter_loadout, registry::Registry, BalanceConfig};
use deadroll_skillcheck::{derive, presets, Category, Purpose};
use deadroll_types::{
    ActiveState, Aspect, Cell, Class, Race, StateKind, MAP_CELLS, MAP_MAX, STATES_MAX,
};
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

fn defense_of(r: &Registry, id: u8) -> DefenseInput {
    let d = r.defense_of(id);
    DefenseInput {
        resist: d.resist,
        immune_aspects: d.immune_aspects,
        immune_states: d.immune_states,
    }
}

fn defense_json(d: &DefenseInput) -> Value {
    json!({
        "resist": d.resist,
        "immuneAspects": d.immune_aspects,
        "immuneStates": d.immune_states,
    })
}

fn preview_json(p: &HitPreview) -> Value {
    let tints: Vec<Value> = p.tints[..p.tint_count as usize]
        .iter()
        .map(|t| {
            json!({
                "base": t.base as u8,
                "tint": t.tint as u8,
                "carrier": t.carrier as u8,
                "tier": t.tier,
                "pure": t.pure,
                "chancePct": t.chance_pct,
                "id": t.id,
                "generated": t.generated,
                "conditionMet": t.condition_met,
                "min": t.min,
                "max": t.max,
            })
        })
        .collect();
    json!({
        "immune": p.immune,
        "hitBp": p.hit_bp,
        "critBp": p.crit_bp,
        "min": p.min,
        "max": p.max,
        "critMin": p.crit_min,
        "critMax": p.crit_max,
        "tints": tints,
    })
}

fn party_json(p: &PartyInput) -> Value {
    let auras: Vec<Value> = p
        .auras
        .iter()
        .map(|slots| {
            slots
                .iter()
                .map(|a| {
                    json!({
                        "kind": a.kind as u8,
                        "aspect": a.aspect as u8,
                        "powerBp": a.power_bp,
                        "radius": a.radius,
                        "turns": a.turns,
                        "hostile": a.hostile,
                        "passive": a.passive,
                        "selfOnly": a.self_only,
                    })
                })
                .collect::<Value>()
        })
        .collect();
    json!({
        "auras": auras,
        "cells": p.cells.iter().map(|c| json!([c.x, c.y])).collect::<Vec<Value>>(),
        "alive": p.alive,
    })
}

/// A party whose player `bearer` (at `at`) carries `aura` in its passive slot, everyone alive.
fn party_of(bearer: usize, at: Cell, aura: AuraInput) -> PartyInput {
    let mut p = PartyInput {
        alive: 0b1111,
        ..PartyInput::default()
    };
    for (i, c) in p.cells.iter_mut().enumerate() {
        *c = Cell {
            x: 1 + i as u8,
            y: 1,
        };
    }
    p.cells[bearer] = at;
    p.auras[bearer][0] = aura;
    p
}

fn aura_in(
    kind: ModKind,
    aspect: Aspect,
    power_bp: u32,
    hostile: bool,
    passive: bool,
) -> AuraInput {
    AuraInput {
        kind,
        aspect,
        power_bp,
        radius: 2,
        turns: if passive { u8::MAX } else { 3 },
        hostile,
        passive,
        self_only: false,
    }
}

/// `unit_mods` cases: a passive Protection aura, a debuff aura on monsters, a basic attack under
/// a passive Empowerment, a self-only passive, a unit out of reach.
fn unit_mods_cases() -> Vec<UnitModsRequest> {
    let at = Cell { x: 4, y: 4 };
    let mut own = NO_MODS;
    own[0] = TimedMod {
        kind: ModKind::Protection,
        aspect: Aspect::Fire,
        turns: 2,
        power_bp: 1_000,
        source: 1,
    };
    let protect = party_of(
        0,
        at,
        aura_in(ModKind::Protection, Aspect::Cold, 2_500, false, true),
    );
    let slow = party_of(
        2,
        at,
        aura_in(ModKind::Slow, Aspect::Cold, 3_000, true, true),
    );
    let empower = party_of(
        1,
        at,
        aura_in(ModKind::Empowerment, Aspect::Blood, 2_000, false, true),
    );
    let mut self_only = protect;
    self_only.auras[0][0].self_only = true;
    let req = |party, mods, monster, slot, at, basic_attack| UnitModsRequest {
        party,
        mods,
        monster,
        slot,
        at,
        basic_attack,
    };
    vec![
        req(protect, NO_MODS, false, 0, at, false),
        req(protect, own, false, 1, Cell { x: 5, y: 5 }, false),
        req(protect, NO_MODS, false, 1, Cell { x: 7, y: 4 }, false),
        req(slow, NO_MODS, true, 0, Cell { x: 5, y: 3 }, false),
        req(slow, NO_MODS, false, 1, Cell { x: 5, y: 3 }, false),
        req(empower, NO_MODS, false, 0, Cell { x: 4, y: 5 }, false),
        req(empower, NO_MODS, false, 0, Cell { x: 4, y: 5 }, true),
        req(self_only, NO_MODS, false, 1, at, false),
    ]
}

/// `line` cases: straight, diagonal, slanted, cut by a wall, cut by the map edge.
fn line_cases() -> Vec<LineRequest> {
    let base = LineRequest {
        from: Cell { x: 2, y: 2 },
        through: Cell { x: 4, y: 2 },
        reach: 6,
        width: 12,
        height: 12,
        blocked: [0; MAP_CELLS / 8],
    };
    let mut walled = base;
    let wall = 2 * MAP_MAX + 6;
    walled.blocked[wall / 8] |= 1 << (wall % 8);
    vec![
        base,
        LineRequest {
            through: Cell { x: 4, y: 4 },
            ..base
        },
        LineRequest {
            through: Cell { x: 5, y: 3 },
            reach: 1 + 5,
            ..base
        },
        walled,
        LineRequest { width: 5, ..base },
    ]
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

fn requests(heroes: &[HeroInput], registry: &Registry) -> Vec<AttackRequest> {
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
            let id = (m % 5) as u8;
            let accuracy = match m % 4 {
                1 => AccuracyBuff {
                    aspect: skill.map_or(Aspect::Fire, |s| s.blocks.aspects[0]),
                    turns: 2,
                    bonus: 3,
                },
                _ => AccuracyBuff::default(),
            };
            out.push(AttackRequest {
                hero: *h,
                hero_mods: if m % 3 == 1 { empowered } else { NO_MODS },
                skill,
                target: TargetInput {
                    bestiary_id: id,
                    level: 1 + (m % 3) as u8,
                    states: if m % 4 == 3 {
                        chill
                    } else {
                        [ActiveState::default(); STATES_MAX]
                    },
                    mods: if m % 5 == 4 { protected } else { NO_MODS },
                    defense: defense_of(registry, id),
                },
                accuracy,
                // Patch 5: auras over the attacker and the target.
                party: match m % 6 {
                    4 => party_of(
                        1,
                        Cell { x: 2, y: 3 },
                        aura_in(ModKind::Vulnerability, Aspect::Piercing, 4_000, true, true),
                    ),
                    5 => party_of(
                        0,
                        Cell { x: 2, y: 2 },
                        aura_in(
                            ModKind::Empowerment,
                            Aspect::Blood,
                            3_000,
                            false,
                            m % 4 != 1,
                        ),
                    ),
                    _ => PartyInput::default(),
                },
                slot: 0,
                target_at: Cell { x: 3, y: 3 },
            });
        }
    }
    out
}

fn build() -> Value {
    let registry = Registry::shipped().unwrap();
    let tables = [
        registry.aspects.clone(),
        registry.traits.clone(),
        registry.resonances.clone(),
    ]
    .concat();
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
    let attacks: Vec<Value> = requests(&heroes, &registry)
        .iter()
        .map(|r| {
            let p = deadroll_combat::preview_hit(
                &attack_input(r).unwrap(),
                &CFG.combat,
                &registry.tables,
            );
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
                        "defense": defense_json(&r.target.defense),
                    },
                    "accuracy": {
                        "aspect": r.accuracy.aspect as u8,
                        "turns": r.accuracy.turns,
                        "bonus": r.accuracy.bonus,
                    },
                    "party": party_json(&r.party),
                    "slot": r.slot,
                    "targetAt": [r.target_at.x, r.target_at.y],
                },
                "bytes": hex(&borsh::to_vec(r).unwrap()),
                "preview": preview_json(&p),
            })
        })
        .collect();
    let monsters: Vec<Value> = (0..5u8)
        .flat_map(|id| [1u8, 3].map(move |level| (id, level)))
        .map(|(id, level)| {
            let input = MonsterInput {
                bestiary_id: id,
                level,
                defense: defense_of(&registry, id),
            };
            let m = monster_sheet(&input).unwrap();
            json!({
                "input": {
                    "bestiaryId": id,
                    "level": level,
                    "defense": defense_json(&input.defense),
                },
                "bytes": hex(&borsh::to_vec(&input).unwrap()),
                "sheet": {
                    "weapon": { "aspect": m.weapon.aspect as u8, "min": m.weapon.min, "max": m.weapon.max },
                    "attackBonus": m.attack_bonus,
                    "evasion": m.evasion,
                    "armor": m.armor,
                    "resistBp": m.resist_bp,
                    "immuneAspects": m.immune_aspects,
                    "immuneStates": m.immune_states,
                    "resistCapBp": m.resist_cap_bp,
                },
            })
        })
        .collect();
    let unit_views: Vec<Value> = unit_mods_cases()
        .iter()
        .map(|r| {
            json!({
                "input": {
                    "party": party_json(&r.party),
                    "mods": mods_json(&r.mods),
                    "monster": r.monster,
                    "slot": r.slot,
                    "at": [r.at.x, r.at.y],
                    "basicAttack": r.basic_attack,
                },
                "bytes": hex(&borsh::to_vec(r).unwrap()),
                "mods": mods_json(&unit_mods(r)),
            })
        })
        .collect();
    let lines: Vec<Value> = line_cases()
        .iter()
        .map(|r| {
            let out = line(r);
            json!({
                "input": {
                    "from": [r.from.x, r.from.y],
                    "through": [r.through.x, r.through.y],
                    "reach": r.reach,
                    "width": r.width,
                    "height": r.height,
                    "blocked": hex(&r.blocked),
                },
                "bytes": hex(&borsh::to_vec(r).unwrap()),
                "cells": out.cells[..out.len as usize].iter().map(|c| json!([c.x, c.y])).collect::<Vec<Value>>(),
            })
        })
        .collect();
    json!({
        "abiVersion": deadroll_combat_wasm::ABI_VERSION,
        "tables": hex(&tables),
        "sheets": sheets,
        "attacks": attacks,
        "monsters": monsters,
        "unitMods": unit_views,
        "lines": lines,
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
