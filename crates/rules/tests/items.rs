//! Equipment rules: starter kits, where items fit, what worn items add to a hero.
use deadroll_combat::{AccuracyBuff, ModKind, TimedMod, TimedMods, MODS_MAX};
use deadroll_rules::{
    input::{effect_range, hero, player_attacker, player_defender, EffectAction, HeroStats},
    items::{
        check_equip, item_bonus, slot_fits, starter_kit, starter_loadout, weapon_range, EquipError,
        Equipment,
    },
    BalanceConfig,
};
use deadroll_types::{
    ActiveState, Aspect, Class, EquipSlot, ItemBase, Race, Stat, EMPTY_ITEM, EQUIP_SLOTS,
    STATES_MAX,
};

const NONE: TimedMods = [TimedMod {
    kind: ModKind::Haste,
    aspect: Aspect::Fire,
    turns: 0,
    power_bp: 0,
    source: 0,
}; MODS_MAX];

fn hero_of(class: Class, equipment: Equipment) -> HeroStats {
    HeroStats {
        race: Race::Human,
        class,
        level: 1,
        stats: [10, 7, 5, 8],
        predisposition: Aspect::Piercing,
        predisposition_tier: 14,
        class_tier: 14,
        equipment,
    }
}

fn kit(class: Class) -> Equipment {
    starter_loadout(class, &BalanceConfig::DEFAULT.items).equipment()
}

#[test]
fn every_class_gets_a_weapon_and_body_armor() {
    let t = &BalanceConfig::DEFAULT.items;
    let expected = [
        (Class::Warrior, ItemBase::Sword, ItemBase::LeatherArmor),
        (Class::Mage, ItemBase::Staff, ItemBase::Robe),
        (Class::Support, ItemBase::Wand, ItemBase::LightLeatherArmor),
    ];
    for (class, weapon, armor) in expected {
        assert_eq!(starter_kit(class, t).collect::<Vec<_>>(), [weapon, armor]);
        let l = starter_loadout(class, t);
        assert_eq!(l.count, 2);
        assert_eq!(l.worn[EquipSlot::MainHand as usize], Some(0));
        assert_eq!(l.worn[EquipSlot::Body as usize], Some(1));
        let eq = l.equipment();
        assert_eq!(eq.get(EquipSlot::MainHand), Some(weapon));
        assert_eq!(eq.get(EquipSlot::Body), Some(armor));
        assert_eq!(eq.worn().count(), 2);
    }
}

#[test]
fn a_second_kit_item_for_a_taken_slot_stays_unworn() {
    let mut t = BalanceConfig::DEFAULT.items;
    t.starter_kits[Class::Warrior as usize] = [
        Some(ItemBase::Sword),
        Some(ItemBase::Staff),
        Some(ItemBase::Robe),
        None,
    ];
    let l = starter_loadout(Class::Warrior, &t);
    assert_eq!(l.count, 3);
    assert_eq!(l.worn[EquipSlot::MainHand as usize], Some(0));
    assert_eq!(l.worn[EquipSlot::Body as usize], Some(2));
    assert_eq!(l.worn.iter().flatten().count(), 2);
}

#[test]
fn equip_checks_slot_and_level() {
    let mut t = BalanceConfig::DEFAULT.items;
    assert_eq!(
        check_equip(1, ItemBase::Sword, 1, EquipSlot::MainHand, &t),
        Ok(())
    );
    assert_eq!(
        check_equip(1, ItemBase::Sword, 1, EquipSlot::Body, &t),
        Err(EquipError::WrongSlot)
    );
    // Level 5 items fit a level 1 character (gap 4), level 6 do not.
    assert_eq!(
        check_equip(1, ItemBase::Robe, 5, EquipSlot::Body, &t),
        Ok(())
    );
    assert_eq!(
        check_equip(1, ItemBase::Robe, 6, EquipSlot::Body, &t),
        Err(EquipError::LevelTooLow)
    );
    t.level_gap = 0;
    assert_eq!(
        check_equip(1, ItemBase::Robe, 2, EquipSlot::Body, &t),
        Err(EquipError::LevelTooLow)
    );
    assert!(slot_fits(EquipSlot::Ring1, EquipSlot::Ring2));
    assert!(slot_fits(EquipSlot::Ring2, EquipSlot::Ring1));
    assert!(!slot_fits(EquipSlot::Ring1, EquipSlot::Necklace));
}

#[test]
fn equipment_bytes_round_trip() {
    let mut eq = Equipment::EMPTY;
    assert!(eq.is_empty());
    assert_eq!(eq.0, [EMPTY_ITEM; EQUIP_SLOTS]);
    eq.set(EquipSlot::Cloak, Some(ItemBase::Robe));
    assert_eq!(eq.get(EquipSlot::Cloak), Some(ItemBase::Robe));
    eq.set(EquipSlot::Cloak, None);
    assert!(eq.is_empty());
    // An unknown byte reads as nothing.
    eq.0[0] = 200;
    assert_eq!(eq.get(EquipSlot::MainHand), None);
}

#[test]
fn the_starter_kit_keeps_the_old_class_numbers() {
    // Before items: class weapon 4-8 / 3-6 / 2-5 and armor 30 / 5 / 5. Since patch 4 the
    // weapons have their own aspects: a Piercing sword, a Blunt staff, a Lightning wand.
    let cfg = BalanceConfig::DEFAULT;
    let old = [
        (Class::Warrior, Aspect::Piercing, [4, 8], 30),
        (Class::Mage, Aspect::Blunt, [3, 6], 5),
        (Class::Support, Aspect::Lightning, [2, 5], 5),
    ];
    let states = [ActiveState::default(); STATES_MAX];
    for (class, aspect, [min, max], armor) in old {
        let h = hero_of(class, kit(class));
        let a = player_attacker(&cfg, 0, &h, None, &NONE, &AccuracyBuff::default());
        assert_eq!(
            (a.weapon.aspect, a.weapon.min, a.weapon.max),
            (aspect, min, max)
        );
        let d = player_defender(&cfg, &h, &states, &NONE);
        assert_eq!(d.armor, armor, "{class:?}");
        assert_eq!(
            hero::hp_max(&cfg, &h),
            hero::hp_max(&cfg, &hero_of(class, Equipment::EMPTY))
        );
    }
}

#[test]
fn the_weapon_tints_skills_but_not_its_own_basic_attack() {
    use deadroll_combat::{Carrier, Tint};
    use deadroll_rules::input::{SkillStats, ITEM_TINT, RACE_TINT};
    let cfg = BalanceConfig::DEFAULT;
    let h = hero_of(Class::Mage, kit(Class::Mage));
    let basic = player_attacker(&cfg, 0, &h, None, &NONE, &AccuracyBuff::default());
    assert_eq!(basic.tints[ITEM_TINT], Tint::default());
    assert_eq!(basic.tints[RACE_TINT].carrier, Carrier::Race);
    let skill = SkillStats::derive(
        deadroll_skillcheck::presets::MAGE[0].blocks,
        deadroll_skillcheck::presets::MAGE[0].ext,
        &cfg.skills,
    );
    let cast = player_attacker(&cfg, 0, &h, Some(&skill), &NONE, &AccuracyBuff::default());
    assert_eq!(
        cast.tints[ITEM_TINT],
        Tint::new(Aspect::Blunt, 14, Carrier::Item)
    );
    // The class slot is empty in the first version (CHARACTER 8.1: no class aspect).
    assert_eq!(cast.tints[1], Tint::default());
    // Armor carries no aspect; bare hands neither.
    let bare = hero_of(Class::Mage, Equipment::EMPTY);
    let cast = player_attacker(
        &cfg,
        0,
        &bare,
        Some(&skill),
        &NONE,
        &AccuracyBuff::default(),
    );
    assert_eq!(cast.tints[ITEM_TINT], Tint::default());
}

#[test]
fn an_accuracy_buff_helps_only_attacks_of_its_aspect() {
    use deadroll_rules::input::SkillStats;
    let cfg = BalanceConfig::DEFAULT;
    let h = hero_of(Class::Mage, kit(Class::Mage));
    let skill = SkillStats::derive(
        deadroll_skillcheck::presets::MAGE[0].blocks,
        deadroll_skillcheck::presets::MAGE[0].ext,
        &cfg.skills,
    );
    let plain = player_attacker(&cfg, 0, &h, Some(&skill), &NONE, &AccuracyBuff::default());
    let buff = |aspect| AccuracyBuff {
        aspect,
        turns: 2,
        bonus: 3,
    };
    let base = skill.blocks.aspects[0];
    let on = player_attacker(&cfg, 0, &h, Some(&skill), &NONE, &buff(base));
    assert_eq!(on.attack_bonus, plain.attack_bonus + 3);
    let other = Aspect::ALL
        .iter()
        .copied()
        .find(|a| !skill.blocks.aspects[..skill.blocks.aspect_count as usize].contains(a))
        .unwrap();
    let off = player_attacker(&cfg, 0, &h, Some(&skill), &NONE, &buff(other));
    assert_eq!(off.attack_bonus, plain.attack_bonus);
}

#[test]
fn without_a_weapon_the_hero_fights_bare_handed() {
    let cfg = BalanceConfig::DEFAULT;
    let h = hero_of(Class::Warrior, Equipment::EMPTY);
    let w = player_attacker(&cfg, 0, &h, None, &NONE, &AccuracyBuff::default()).weapon;
    assert_eq!((w.min, w.max), (cfg.items.unarmed[0], cfg.items.unarmed[1]));
    assert_eq!(item_bonus(&h.equipment, &cfg.items).weapon, None);
    // Armor in the main hand is not a weapon.
    let mut eq = Equipment::EMPTY;
    eq.set(EquipSlot::MainHand, Some(ItemBase::Robe));
    assert_eq!(weapon_range(&eq, &cfg.items), w);
}

#[test]
fn every_item_number_reaches_the_hero() {
    let mut cfg = BalanceConfig::DEFAULT;
    let robe = &mut cfg.items.bases[ItemBase::Robe as usize];
    robe.armor = 7;
    robe.defense = 3;
    robe.attack = 2;
    robe.hp = 11;
    robe.stats[Stat::Dexterity as usize] = 10;
    robe.stats[Stat::Endurance as usize] = 4;
    robe.stats[Stat::Intelligence as usize] = 6;
    robe.crit_pct = 7;
    robe.speed = 5;
    robe.speed_counter_bp = 10_000;
    robe.initiative = 4;
    let sword = &mut cfg.items.bases[ItemBase::Sword as usize];
    sword.aspect = Aspect::Piercing;
    sword.damage = [5, 9];

    let mut eq = Equipment::EMPTY;
    eq.set(EquipSlot::MainHand, Some(ItemBase::Sword));
    eq.set(EquipSlot::Body, Some(ItemBase::Robe));
    let bare = hero_of(Class::Mage, Equipment::EMPTY);
    let dressed = hero_of(Class::Mage, eq);

    let a = player_attacker(&cfg, 0, &dressed, None, &NONE, &AccuracyBuff::default());
    let a0 = player_attacker(&cfg, 0, &bare, None, &NONE, &AccuracyBuff::default());
    assert_eq!(
        (a.weapon.aspect, a.weapon.min, a.weapon.max),
        (Aspect::Piercing, 5, 9)
    );
    // Accuracy: +2 flat, and Dexterity 7 + 10 = 17: 17 / 5 = 3 against 7 / 5 = 1.
    assert_eq!(a.attack_bonus, a0.attack_bonus + 2 + 2);
    // Crit comes from items only.
    assert_eq!((a0.crit_chance_pct, a.crit_chance_pct), (0, 7));

    let states = [ActiveState::default(); STATES_MAX];
    let d = player_defender(&cfg, &dressed, &states, &NONE);
    let d0 = player_defender(&cfg, &bare, &states, &NONE);
    assert_eq!(d.armor, d0.armor + 7);
    // Dexterity 7 + 10 = 17: 17 / 5 = 3 against 7 / 5 = 1.
    assert_eq!(d.defense, d0.defense + 3 + 2);

    assert_eq!(
        hero::hp_max(&cfg, &dressed),
        hero::hp_max(&cfg, &bare) + 11 + 4 * cfg.players.hp_per_endurance
    );
    // Speed: human mage 3, +1 from Dexterity 17, +5 from the item, then its counterweight halves.
    assert_eq!(hero::ap(&cfg, &bare), 3);
    assert_eq!(hero::ap(&cfg, &dressed), (3 + 1 + 5) / 2);
    // Intelligence 11 / 2 against 5 / 2, plus the item.
    assert_eq!(
        hero::initiative(&cfg, &dressed),
        hero::initiative(&cfg, &bare) + 3 + 4
    );
    // Heals scale from the equipped weapon.
    let heal = effect_range(&cfg, &dressed, 10_000, EffectAction::Heal);
    let heal0 = effect_range(&cfg, &bare, 10_000, EffectAction::Heal);
    assert!(heal.1 > heal0.1);
}

#[cfg(feature = "serde")]
#[test]
fn item_tuning_is_json_by_name() {
    let json = serde_json::to_value(BalanceConfig::DEFAULT.items).unwrap();
    assert_eq!(json["bases"]["Sword"]["slot"], "MainHand");
    assert_eq!(json["bases"]["Robe"]["aspect"], "Blunt");
    assert_eq!(json["bases"]["LeatherArmor"]["armor"], 10);
    assert_eq!(json["bases"]["Sword"]["stats"]["Strength"], 0);
    assert_eq!(
        json["starter_kits"]["Mage"],
        serde_json::json!(["Staff", "Robe", null, null])
    );
    let back: deadroll_rules::ItemTuning = serde_json::from_value(json).unwrap();
    assert_eq!(back, BalanceConfig::DEFAULT.items);
    let bad = serde_json::json!({"Warrior": ["Axe", null, null, null]});
    let mut j = serde_json::to_value(BalanceConfig::DEFAULT.items).unwrap();
    j["starter_kits"] = bad;
    let err = serde_json::from_value::<deadroll_rules::ItemTuning>(j).unwrap_err();
    assert!(err.to_string().contains("unknown `Axe`"), "{err}");
}
