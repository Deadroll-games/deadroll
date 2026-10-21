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
    attack_input, hero_sheet, line, monster_attack_input, monster_plan, monster_sheet,
    monster_skill, spawn_skills, unit_mods, AiUnitInput, AttackRequest, AuraInput, BearerInput,
    HeroInput, HeroSheet, LineRequest, MonsterAttackRequest, MonsterInput, MonsterPlanRequest,
    PartyInput, SkillCopy, SkillInput, SpawnSkillsRequest, TargetInput, UnitModsRequest,
};
use deadroll_rules::{items::starter_loadout, registry::Registry, BalanceConfig};
use deadroll_skillcheck::{derive, monster::POOL_MAX, presets, Category, Purpose};
use deadroll_types::{
    ActiveState, Aspect, Cell, Class, Race, StateKind, MAP_CELLS, MAP_MAX, MONSTERS_MAX,
    MONSTER_SLOTS, PARTY_MAX, SPAWN_ELITE, STATES_MAX,
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
    weights_pct: [0; Aspect::COUNT],
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
                "weightsPct": t.weights_pct,
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
        "resistBp": s.resist_bp,
        "stateResistBp": s.state_resist_bp,
        "stateResistByKindBp": s.state_resist_by_kind_bp,
    })
}

/// The shipped bestiary record of `id`, as `MonsterState.records` holds it.
fn record_of(r: &Registry, id: u8) -> [u8; 48] {
    r.record(id).unwrap()
}

/// The session copy of the shipped monster skill named `name`.
fn skill_copy(r: &Registry, name: &str) -> SkillCopy {
    let i = r.skill_names.iter().position(|n| n == name).unwrap();
    r.skills[i].unwrap().encode_session()
}

fn ai_unit_json(u: &AiUnitInput) -> Value {
    json!({
        "cell": [u.cell.x, u.cell.y],
        "hp": u.hp,
        "hpMax": u.hp_max,
        "alive": u.alive,
        "states": states_json(&u.states),
        "mods": mods_json(&u.mods),
        "kin": u.kin,
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

fn aura_json(a: &AuraInput) -> Value {
    json!({
        "kind": a.kind as u8,
        "aspect": a.aspect as u8,
        "powerBp": a.power_bp,
        "radius": a.radius,
        "turns": a.turns,
        "hostile": a.hostile,
        "passive": a.passive,
        "selfOnly": a.self_only,
        "weightsPct": a.weights_pct,
    })
}

fn party_json(p: &PartyInput) -> Value {
    let auras: Vec<Value> = p
        .auras
        .iter()
        .map(|slots| slots.iter().map(aura_json).collect::<Value>())
        .collect();
    // Only the used bearers: the encoder pads the rest with zeros.
    let bearers: Vec<Value> = p.bearers[..p.bearer_count as usize]
        .iter()
        .map(|b| {
            json!({
                "index": b.index,
                "cell": [b.cell.x, b.cell.y],
                "kin": b.kin,
                "auras": b.auras.iter().map(|(a, k)| json!({ "aura": aura_json(a), "kinOnly": k })).collect::<Vec<Value>>(),
            })
        })
        .collect();
    json!({
        "auras": auras,
        "cells": p.cells.iter().map(|c| json!([c.x, c.y])).collect::<Vec<Value>>(),
        "alive": p.alive,
        "bearers": bearers,
    })
}

/// A party with no auras and monster `index` (kin `kin`, at `at`) bearing `aura`.
fn with_bearer(
    mut p: PartyInput,
    index: u8,
    at: Cell,
    kin: u8,
    aura: AuraInput,
    kin_only: bool,
) -> PartyInput {
    let n = p.bearer_count as usize;
    p.bearers[n] = BearerInput {
        index,
        cell: at,
        kin,
        auras: [(aura, kin_only), (AuraInput::default(), false)],
    };
    p.bearer_count += 1;
    p
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
        weights_pct: [0; Aspect::COUNT],
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
        weights_pct: [0; Aspect::COUNT],
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
    // Patch 9: a passive Resistance aura over Piercing (65) and Cold (35), the bearer's Blood
    // predisposition at the base share; an own Resistance it outweighs.
    let mut ward = aura_in(ModKind::Resistance, Aspect::Piercing, 3_000, false, true);
    ward.weights_pct[Aspect::Piercing as usize] = 65;
    ward.weights_pct[Aspect::Cold as usize] = 35;
    ward.weights_pct[Aspect::Blood as usize] = 65;
    let resist = party_of(0, at, ward);
    let mut own_resist = NO_MODS;
    own_resist[ModKind::Resistance as usize] = TimedMod {
        kind: ModKind::Resistance,
        aspect: Aspect::Fire,
        turns: 2,
        power_bp: 1_000,
        source: 1,
        weights_pct: [0, 100, 0, 0, 0, 0],
    };
    // Patch 6: a goblin chieftain's Leader (kin-only Empowerment) over two monsters, a monster's
    // Slow aura over a player.
    let leader = with_bearer(
        PartyInput::default(),
        0,
        at,
        1,
        aura_in(ModKind::Empowerment, Aspect::Piercing, 3_000, false, true),
        true,
    );
    let monster_slow = with_bearer(
        protect,
        3,
        Cell { x: 6, y: 4 },
        0,
        aura_in(ModKind::Slow, Aspect::Cold, 2_000, true, true),
        false,
    );
    let req = |party, mods, monster, slot, at, basic_attack, kin| UnitModsRequest {
        party,
        mods,
        monster,
        slot,
        at,
        basic_attack,
        kin,
    };
    vec![
        req(protect, NO_MODS, false, 0, at, false, 0),
        req(protect, own, false, 1, Cell { x: 5, y: 5 }, false, 0),
        req(protect, NO_MODS, false, 1, Cell { x: 7, y: 4 }, false, 0),
        req(slow, NO_MODS, true, 0, Cell { x: 5, y: 3 }, false, 0),
        req(slow, NO_MODS, false, 1, Cell { x: 5, y: 3 }, false, 0),
        req(empower, NO_MODS, false, 0, Cell { x: 4, y: 5 }, false, 0),
        req(empower, NO_MODS, false, 0, Cell { x: 4, y: 5 }, true, 0),
        req(self_only, NO_MODS, false, 1, at, false, 0),
        req(resist, NO_MODS, false, 1, Cell { x: 5, y: 5 }, false, 0),
        req(resist, own_resist, false, 1, Cell { x: 5, y: 5 }, false, 0),
        req(leader, NO_MODS, true, 1, Cell { x: 5, y: 4 }, false, 1),
        req(leader, NO_MODS, true, 1, Cell { x: 5, y: 4 }, true, 1),
        req(leader, NO_MODS, true, 2, Cell { x: 5, y: 4 }, false, 0),
        req(
            monster_slow,
            NO_MODS,
            false,
            0,
            Cell { x: 5, y: 4 },
            false,
            0,
        ),
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
        weights_pct: [0; Aspect::COUNT],
    };
    let mut protected = NO_MODS;
    protected[0] = TimedMod {
        kind: ModKind::Protection,
        aspect: Aspect::Blunt,
        turns: 2,
        power_bp: 2_500,
        source: 5,
        weights_pct: [0; Aspect::COUNT],
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
                    record: record_of(registry, id),
                    level: 1 + (m % 3) as u8,
                    states: if m % 4 == 3 {
                        chill
                    } else {
                        [ActiveState::default(); STATES_MAX]
                    },
                    mods: if m % 5 == 4 { protected } else { NO_MODS },
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
                    // Patch 6: the target's ally covers it with a Protection aura.
                    3 => with_bearer(
                        PartyInput::default(),
                        1,
                        Cell { x: 4, y: 3 },
                        registry.template(id).unwrap().kin,
                        aura_in(ModKind::Protection, Aspect::Blunt, 2_000, false, false),
                        false,
                    ),
                    _ => PartyInput::default(),
                },
                slot: 0,
                target_at: Cell { x: 3, y: 3 },
                target_index: 0,
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
                        "record": hex(&r.target.record),
                        "level": r.target.level,
                        "states": states_json(&r.target.states),
                        "mods": mods_json(&r.target.mods),
                    },
                    "accuracy": {
                        "aspect": r.accuracy.aspect as u8,
                        "turns": r.accuracy.turns,
                        "bonus": r.accuracy.bonus,
                    },
                    "party": party_json(&r.party),
                    "slot": r.slot,
                    "targetAt": [r.target_at.x, r.target_at.y],
                    "targetIndex": r.target_index,
                },
                "bytes": hex(&borsh::to_vec(r).unwrap()),
                "preview": preview_json(&p),
            })
        })
        .collect();
    let monsters: Vec<Value> = (0..5u8)
        .flat_map(|id| [(1u8, 0u8), (3, 0), (1, SPAWN_ELITE)].map(move |(l, f)| (id, l, f)))
        .map(|(id, level, flags)| {
            let input = MonsterInput {
                record: record_of(&registry, id),
                level,
                flags,
            };
            let m = monster_sheet(&input).unwrap();
            json!({
                "input": {
                    "record": hex(&input.record),
                    "level": level,
                    "flags": flags,
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
                    "hpMax": m.hp_max,
                    "xp": m.xp,
                    "boss": m.boss,
                    "role": m.role,
                    "size": m.size,
                    "minTier": m.min_tier,
                    "speed": m.speed,
                    "initiative": m.initiative,
                    "source": m.source as u8,
                    "form": m.form as u8,
                    "range": m.range as u8,
                    "poolStart": m.pool_start,
                    "poolLen": m.pool_len,
                    "kin": m.kin,
                    "slots": m.slots,
                    "tagResistBp": m.tag_resist_bp,
                    "immuneTags": m.immune_tags,
                },
            })
        })
        .collect();
    // Patch 6: every shipped monster skill, as the session copy.
    let monster_skills: Vec<Value> = registry
        .skills
        .iter()
        .flatten()
        .map(|k| {
            let copy = k.encode_session();
            let sheet = monster_skill(&copy).unwrap();
            let b = &sheet.blocks;
            json!({
                "copy": hex(&copy),
                "sheet": {
                    "blocks": [
                        b.category as u8, b.purpose as u8, b.source as u8, b.form as u8, b.target as u8,
                        b.aspects[0] as u8, b.aspects[1] as u8, b.aspects[2] as u8, b.aspect_count,
                        b.effect as u8, b.trigger as u8, b.aura_radius,
                    ],
                    "ext": [sheet.ext.range as u8, sheet.ext.timing as u8, sheet.ext.hp_below_pct],
                    "kinOnly": sheet.kin_only,
                    "sharesBp": sheet.shares_bp,
                    "powerBp": sheet.power_bp,
                    "cooldown": sheet.cooldown,
                    "apCost": sheet.ap_cost,
                },
            })
        })
        .collect();
    // Every shipped template with its pool, a few masks and both ranks.
    let mut spawn_cases = Vec::new();
    for id in 0..5u8 {
        let t = registry.template(id).unwrap();
        let mut categories = [0u8; POOL_MAX];
        for (c, k) in categories.iter_mut().zip(registry.pool(&t)) {
            *c = k.unwrap().blocks.category as u8;
        }
        for (mask, flags) in [
            (0u16, 0u8),
            (0b111, 0),
            (0xFFFF, 0),
            (0b1111, SPAWN_ELITE),
            (0b1_1111, 0),
        ] {
            let r = SpawnSkillsRequest {
                record: record_of(&registry, id),
                flags,
                categories,
                mask,
            };
            let out = spawn_skills(&r).unwrap();
            spawn_cases.push(json!({
                "input": {
                    "record": hex(&r.record),
                    "flags": flags,
                    "categories": categories,
                    "mask": mask,
                },
                "bytes": hex(&borsh::to_vec(&r).unwrap()),
                "result": {
                    "error": out.error,
                    "slots": out.slots,
                    "defaultMask": out.default_mask,
                },
            }));
        }
    }
    // A monster's natural attack and skills on the warrior.
    let warrior = heroes[0];
    let mut monster_attacks = Vec::new();
    for (id, skill) in [
        (0u8, None),
        (0, Some("Hamstring")),
        (2, Some("Dagger Throw")),
        (2, None),
        (4, Some("Spark")),
        (1, Some("Thrown Stone")),
    ] {
        let mut party = party_of(
            0,
            Cell { x: 2, y: 2 },
            aura_in(ModKind::Protection, Aspect::Cold, 2_000, false, true),
        );
        if id == 2 {
            party = with_bearer(
                party,
                0,
                Cell { x: 3, y: 2 },
                1,
                aura_in(ModKind::Empowerment, Aspect::Piercing, 3_000, false, true),
                true,
            );
        }
        let r = MonsterAttackRequest {
            monster: TargetInput {
                record: record_of(&registry, id),
                level: 1,
                states: [ActiveState::default(); STATES_MAX],
                mods: NO_MODS,
            },
            index: 0,
            at: Cell { x: 3, y: 2 },
            skill: skill.map(|n| skill_copy(&registry, n)),
            hero: warrior,
            hero_states: [ActiveState::default(); STATES_MAX],
            hero_mods: NO_MODS,
            slot: 0,
            party,
        };
        let p = deadroll_combat::preview_hit(
            &monster_attack_input(&r).unwrap(),
            &CFG.combat,
            &registry.tables,
        );
        monster_attacks.push(json!({
            "input": {
                "monster": {
                    "record": hex(&r.monster.record),
                    "level": r.monster.level,
                    "states": states_json(&r.monster.states),
                    "mods": mods_json(&r.monster.mods),
                },
                "index": r.index,
                "at": [r.at.x, r.at.y],
                "skill": r.skill.map(|c| hex(&c)),
                "hero": hero_json(&r.hero),
                "heroStates": states_json(&r.hero_states),
                "heroMods": mods_json(&r.hero_mods),
                "slot": r.slot,
                "party": party_json(&r.party),
            },
            "bytes": hex(&borsh::to_vec(&r).unwrap()),
            "preview": preview_json(&p),
        }));
    }
    // The monster AI: the shaman with a hurt ally or not, every skill on cooldown, out of reach.
    let mut plans = Vec::new();
    let shaman = registry.template(4).unwrap();
    for (hurt, cooldown, far) in [
        (2u32, 0u8, false),
        (10, 0, false),
        (10, 1, false),
        (10, 1, true),
    ] {
        let unit = |x: u8, hp: u32, kin: u8| AiUnitInput {
            cell: Cell { x, y: 5 },
            hp,
            hp_max: 10,
            alive: true,
            kin,
            ..AiUnitInput::default()
        };
        let mut skills = [None; MONSTER_SLOTS];
        for (slot, name) in ["Spark", "Mending", "Quicken"].iter().enumerate() {
            skills[slot] = Some(skill_copy(&registry, name));
        }
        let mut r = MonsterPlanRequest {
            record: record_of(&registry, 4),
            level: 1,
            index: 0,
            ap: 3,
            skills,
            cooldowns: [cooldown; MONSTER_SLOTS],
            heroes: [AiUnitInput::default(); PARTY_MAX],
            hero_count: 1,
            monsters: [AiUnitInput::default(); MONSTERS_MAX],
            monster_count: 2,
        };
        r.heroes[0] = unit(if far { 31 } else { 9 }, 10, 0xFF);
        r.monsters[0] = unit(5, 10, shaman.kin);
        r.monsters[1] = unit(6, hurt, shaman.kin);
        let out = monster_plan(&r).unwrap();
        plans.push(json!({
            "input": {
                "record": hex(&r.record),
                "level": r.level,
                "index": r.index,
                "ap": r.ap,
                "skills": r.skills.iter().map(|k| k.map(|c| hex(&c))).collect::<Vec<_>>(),
                "cooldowns": r.cooldowns,
                "heroes": r.heroes[..r.hero_count as usize].iter().map(ai_unit_json).collect::<Vec<_>>(),
                "monsters": r.monsters[..r.monster_count as usize].iter().map(ai_unit_json).collect::<Vec<_>>(),
            },
            "bytes": hex(&borsh::to_vec(&r).unwrap()),
            "plan": {
                "found": out.found,
                "slot": out.slot,
                "ally": out.ally,
                "target": out.target,
                "range": out.range as u8,
                "ap": out.ap,
            },
        }));
    }
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
                    "kin": r.kin,
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
        "monsterSkills": monster_skills,
        "spawnSkills": spawn_cases,
        "monsterAttacks": monster_attacks,
        "monsterPlans": plans,
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
