//! Patch 9 resistances in the rules: the hero's from items and Endurance, the Resistance effect
//! (the skill's aspects by share plus the bearer's predisposition), state resistance on effect
//! chances, immunity to the state of an effect, and the bestiary's defenses by tag
//! (docs/CHARACTER.md 2-3, COMBAT_MATH.md 3 and 5, DAMAGE_TYPES_AND_MIXES.md 9).
#![cfg(feature = "serde")]
use deadroll_combat::{
    form_bit, modifiers::ratio, source_bit, states::state_bit, Applied, HitInput, HitResult,
    ModKind, Outcome, SeededRoll, TimedMods,
};
use deadroll_rules::{
    aura::passive_aura,
    input::{
        effect_chance, effect_immune, effect_resist, hero, land_effect, monster_defender,
        monster_effect_chance, player_defender, resistance_weights, EffectAction, HeroStats,
        SkillStats,
    },
    items::starter_loadout,
    monster::{MonsterSkillStats, MonsterTemplate},
    registry::{encode_bestiary, Registry},
    resolve::{debuff_monster, monster_attack, RiderOutcome},
    BalanceConfig,
};
use deadroll_skillcheck::{Category, Effect, Purpose, SkillBlocks, SkillExt};
use deadroll_types::{
    ActiveState, Aspect, Class, Form, ItemBase, Monster, Race, Source, Stat, StateKind, STATES_MAX,
};

const CFG: BalanceConfig = BalanceConfig::DEFAULT;
const WOLF: u8 = 0;

fn hero_with(endurance: u16, predisposition: Aspect) -> HeroStats {
    HeroStats {
        race: Race::Human,
        class: Class::Warrior,
        level: 1,
        stats: [6, 6, 6, endurance],
        predisposition,
        predisposition_tier: 14,
        class_tier: 14,
        equipment: starter_loadout(Class::Warrior, &CFG.items).equipment(),
    }
}

fn skill(category: Category, purpose: Purpose, aspects: &[Aspect], effect: Effect) -> SkillStats {
    let mut list = [Aspect::default(); 3];
    list[..aspects.len()].copy_from_slice(aspects);
    let blocks = SkillBlocks {
        category,
        purpose,
        source: Source::Magical,
        form: Form::Strike,
        aspects: list,
        aspect_count: aspects.len() as u8,
        effect,
        ..SkillBlocks::default()
    };
    SkillStats::derive(blocks, SkillExt::MELEE, &CFG.skills)
}

fn no_states() -> [ActiveState; STATES_MAX] {
    [ActiveState::default(); STATES_MAX]
}

#[test]
fn a_hero_resists_through_items_and_endurance_never_through_race() {
    let h = hero_with(10, Aspect::Fire);
    let bare = player_defender(&CFG, &h, &no_states(), &TimedMods::default());
    // No item resists in the shipped bases and the race gives nothing on its own.
    assert_eq!(bare.resist_bp, [0; Aspect::COUNT]);
    let per = CFG.players.end_state_resist_bp;
    assert!(per > 0);
    assert_eq!(bare.state_resist_bp, [10 * per; StateKind::COUNT]);

    // Worn items add theirs: per aspect (physical too) and to the state resistance.
    let mut cfg = CFG;
    let armor = &mut cfg.items.bases[ItemBase::LeatherArmor as usize];
    armor.resist_bp[Aspect::Fire as usize] = 2_000;
    armor.resist_bp[Aspect::Piercing as usize] = 1_000;
    armor.state_resist_bp = 500;
    armor.stats[Stat::Endurance as usize] = 2;
    let d = player_defender(&cfg, &h, &no_states(), &TimedMods::default());
    assert_eq!(d.resist_bp[Aspect::Fire as usize], 2_000);
    assert_eq!(d.resist_bp[Aspect::Piercing as usize], 1_000);
    assert_eq!(hero::state_resist(&cfg, &h), 12 * per + 500);
    assert_eq!(
        d.state_resist_bp[StateKind::Bleeding as usize],
        12 * per + 500
    );
}

#[test]
fn a_resistance_weighs_the_skill_aspects_by_share_and_adds_the_predisposition() {
    // The builder allows one aspect on a buff today (SKILL_BUILDER 13); more come with skill
    // upgrades, and the weights already follow the shares.
    let ward = skill(
        Category::Passive,
        Purpose::Buff,
        &[Aspect::Piercing, Aspect::Cold],
        Effect::Resistance,
    );
    let [first, second, _] = ward.shares_bp;
    let w = resistance_weights(&ward, Some(Aspect::Blood));
    assert_eq!(w[Aspect::Piercing as usize] as u32, first / 100);
    assert_eq!(w[Aspect::Cold as usize] as u32, second / 100);
    // The predisposition at the base aspect's share.
    assert_eq!(w[Aspect::Blood as usize] as u32, first / 100);
    assert_eq!(w[Aspect::Fire as usize], 0);
    // A predisposition already in the skill keeps its own (larger) weight.
    let same = resistance_weights(&ward, Some(Aspect::Cold));
    assert_eq!(same[Aspect::Cold as usize] as u32, first / 100);
    // Any other effect weighs nothing.
    let guard = skill(
        Category::Passive,
        Purpose::Buff,
        &[Aspect::Piercing],
        Effect::Protection,
    );
    assert_eq!(
        resistance_weights(&guard, Some(Aspect::Blood)),
        [0; Aspect::COUNT]
    );
}

#[test]
fn a_passive_resistance_cuts_its_aspects_on_the_bearer() {
    let h = hero_with(6, Aspect::Blood);
    let ward = skill(
        Category::Passive,
        Purpose::Buff,
        &[Aspect::Piercing],
        Effect::Resistance,
    );
    let aura = passive_aura(&CFG, &h, &ward).unwrap();
    assert_eq!(aura.kind, ModKind::Resistance);
    assert!(aura.self_only);
    assert_eq!(aura.weights_pct[Aspect::Piercing as usize], 100);
    assert_eq!(aura.weights_pct[Aspect::Blood as usize], 100);

    // Landed as a buff: the same weights reach the defender.
    let mut states = no_states();
    let mut mods = TimedMods::default();
    let applied = land_effect(
        &CFG,
        EffectAction::Mod(ModKind::Resistance),
        3_000,
        Aspect::Piercing,
        aura.weights_pct,
        0,
        0,
        &mut states,
        &mut mods,
    );
    assert_eq!(applied, Applied::Added);
    let d = player_defender(&CFG, &h, &states, &mods);
    assert_eq!(d.resist_bp[Aspect::Piercing as usize], 3_000);
    assert_eq!(d.resist_bp[Aspect::Blood as usize], 3_000);
    assert_eq!(d.resist_bp[Aspect::Fire as usize], 0);
}

#[test]
fn state_resistance_divides_effect_chances() {
    let h = hero_with(6, Aspect::Piercing);
    let slow = skill(
        Category::Active,
        Purpose::Damage,
        &[Aspect::Cold],
        Effect::Slow,
    );
    let open = effect_chance(&CFG, &h, &slow, 0);
    let held = effect_chance(&CFG, &h, &slow, 5_000);
    assert!(held < open);
    let bonus = hero::state_chance(&CFG, &h, Stat::Dexterity);
    let base = CFG.support.rider_chance_pct as u64;
    assert_eq!(held as u64, ratio(base, bonus, 5_000));
    assert_eq!(monster_effect_chance(&CFG, 0), CFG.support.rider_chance_pct);
    assert_eq!(
        monster_effect_chance(&CFG, 10_000) as u64,
        ratio(base, 0, 10_000)
    );

    // Hard control meets the resistance to its own state, a modifier the strongest one.
    let mut d = player_defender(&CFG, &h, &no_states(), &TimedMods::default());
    d.state_resist_bp[StateKind::Stun as usize] = 9_000;
    assert_eq!(effect_resist(&d, EffectAction::Stun), 9_000);
    assert_eq!(effect_resist(&d, EffectAction::Mod(ModKind::Slow)), 9_000);
    assert_eq!(
        effect_resist(&d, EffectAction::Freeze),
        d.state_resist_bp[StateKind::Freeze as usize]
    );
}

#[test]
fn a_monster_rider_meets_the_player_state_resistance() {
    let r = Registry::shipped().unwrap();
    let t = r.template(WOLF).unwrap();
    let p = t.profile(1, &CFG.monsters);
    let i = r.skill_names.iter().position(|n| n == "Hamstring").unwrap();
    let hamstring = MonsterSkillStats::new(&r.skills[i].unwrap(), &CFG.skills);
    let tough = hero_with(15, Aspect::Piercing);
    let resist = hero::state_resist(&CFG, &tough);
    let expected = CFG.combat.clamp_chance(monster_effect_chance(&CFG, resist));
    let mut seen = false;
    for seed in 0..200 {
        let mut hp = 50;
        let mut states = no_states();
        let mut own = TimedMods::default();
        let mut input = Box::<HitInput>::default();
        let mut result = Box::<HitResult>::default();
        let out = monster_attack(
            &CFG,
            &r.tables,
            4,
            &p,
            Some(&hamstring.stats),
            &TimedMods::default(),
            &tough,
            &mut hp,
            &mut states,
            &mut own,
            &TimedMods::default(),
            &mut input,
            &mut result,
            &mut SeededRoll::new(seed),
        );
        if let Some(rider) = out.rider {
            if !rider.roll.crit {
                assert_eq!(rider.roll.chance_pct, expected);
                seen = true;
            }
        }
    }
    assert!(seen);
    assert!(expected < CFG.support.rider_chance_pct);
}

#[test]
fn an_effect_on_a_state_immune_target_rolls_nothing() {
    assert!(effect_immune(
        EffectAction::Freeze,
        state_bit(StateKind::Freeze)
    ));
    assert!(!effect_immune(
        EffectAction::Stun,
        state_bit(StateKind::Freeze)
    ));
    assert!(!effect_immune(EffectAction::Mod(ModKind::Slow), u8::MAX));

    let r = Registry::shipped().unwrap();
    let mut p = r.template(WOLF).unwrap().profile(1, &CFG.monsters);
    p.immune_states = state_bit(StateKind::Freeze);
    let h = hero_with(6, Aspect::Piercing);
    let frost = skill(
        Category::Active,
        Purpose::Debuff,
        &[Aspect::Cold],
        Effect::Freeze,
    );
    let mut m = Monster {
        hp: 20,
        hp_max: 20,
        alive: true,
        ..Monster::default()
    };
    let mut mods = TimedMods::default();
    let out = debuff_monster(
        &CFG,
        0,
        &h,
        &frost,
        EffectAction::Freeze,
        &p,
        &mut m,
        &mut mods,
        &mut SeededRoll::new(1),
    );
    assert_eq!(out, RiderOutcome::IMMUNE);
    assert_eq!(out.roll.outcome, Outcome::Immune);
    assert_eq!(out.roll.rolls_used, 0);
    assert!(m.states.iter().all(|s| s.turns == 0));
}

/// The shipped bestiary with `edit` applied to the wolf.
fn bestiary_with(edit: impl Fn(&mut serde_json::Value)) -> Result<Vec<u8>, String> {
    let text = include_str!("../../../assets/registry/bestiary.json");
    let mut json: serde_json::Value = serde_json::from_str(text).unwrap();
    let list = json["records"].as_array_mut().unwrap();
    edit(&mut list[WOLF as usize]);
    encode_bestiary(&serde_json::to_string(&json).unwrap())
}

#[test]
fn the_bestiary_sets_defenses_by_tag() {
    let body = bestiary_with(|w| {
        w["tag_resistance"] = serde_json::json!(["Magical"]);
        w["tag_immunity"] = serde_json::json!(["Projectile"]);
    })
    .unwrap();
    let size = deadroll_rules::monster::record::SIZE;
    let t = MonsterTemplate::from_record(&body[..size]).unwrap();
    assert_eq!(t.defense.resist_tags, source_bit(Source::Magical));
    assert_eq!(t.defense.immune_tags, form_bit(Form::Projectile));
    let p = t.profile(1, &CFG.monsters);
    assert_eq!(
        p.tag_resist_bp[Source::Magical as usize],
        CFG.monsters.resist_bp
    );
    assert_eq!(p.tag_resist_bp[Source::Combat as usize], 0);
    let d = monster_defender(&p, &no_states(), &TimedMods::default());
    assert_eq!(d.immune_tags, form_bit(Form::Projectile));
    assert_eq!(d.immune_mask(Source::Combat, Form::Projectile), u8::MAX);
    assert_eq!(d.immune_mask(Source::Combat, Form::Strike), 0);

    let bad = bestiary_with(|w| w["tag_immunity"] = serde_json::json!(["Fire"]));
    assert!(bad.unwrap_err().contains("unknown tag"));
}
