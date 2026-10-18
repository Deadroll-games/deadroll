//! Starter skill sets per class (docs/SKILL_BUILDER.md, section 10.1): 4 active, 1 reactive,
//! 1 passive. The app offers them at character creation; the program checks them like any
//! other starting skill. Names and descriptions are the player's text, these are defaults.

use deadroll_types::{
    Aspect::{self, *},
    Class, Form, RangeTag, Source,
};

use crate::{Category, Effect, Purpose, SkillBlocks, SkillExt, Target, Trigger, STARTING_MAX};

/// A named starting skill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    pub name: &'static str,
    pub blocks: SkillBlocks,
    pub ext: SkillExt,
    /// Default description, English, at most 200 characters (section 12).
    pub description: &'static str,
}

struct B(SkillBlocks, SkillExt);

impl B {
    const fn new(category: Category, purpose: Purpose, source: Source, form: Form) -> Self {
        B(
            SkillBlocks {
                category,
                purpose,
                source,
                form,
                target: Target::Single,
                aspects: [Fire; 3],
                aspect_count: 0,
                effect: Effect::None,
                trigger: Trigger::None,
                aura_radius: 0,
            },
            SkillExt::MELEE,
        )
    }
    const fn one(mut self, a: Aspect) -> Self {
        self.0.aspects[0] = a;
        self.0.aspect_count = 1;
        self
    }
    const fn two(mut self, a: Aspect, b: Aspect) -> Self {
        self.0.aspects = [a, b, Fire];
        self.0.aspect_count = 2;
        self
    }
    const fn effect(mut self, e: Effect) -> Self {
        self.0.effect = e;
        self
    }
    const fn line(mut self) -> Self {
        self.0.target = Target::Line;
        self
    }
    const fn ranged(mut self) -> Self {
        self.1.range = RangeTag::Ranged;
        self
    }
    const fn trigger(mut self, t: Trigger) -> Self {
        self.0.trigger = t;
        self
    }
    const fn below(mut self, pct: u8) -> Self {
        self.1.hp_below_pct = pct;
        self
    }
    const fn aura(mut self, radius: u8) -> Self {
        self.0.aura_radius = radius;
        self
    }
    const fn named(self, name: &'static str, description: &'static str) -> Preset {
        Preset {
            name,
            blocks: self.0,
            ext: self.1,
            description,
        }
    }
}

use Category::{Active, Passive, Reactive};
use Form::{Projectile, Strike};
use Purpose::{Buff, Damage, Debuff};
use Source::{Combat, Magical};

/// Martial only.
pub const WARRIOR: [Preset; STARTING_MAX] = [
    B::new(Active, Damage, Combat, Strike).one(Piercing).named(
        "Piercing Thrust",
        "A straight thrust that finds the gap in the armor.",
    ),
    B::new(Active, Damage, Combat, Strike)
        .one(Blunt)
        .effect(Effect::Stun)
        .named(
            "Shield Bash",
            "The shield rim to the head. Sometimes they stay down.",
        ),
    B::new(Active, Damage, Combat, Projectile)
        .one(Piercing)
        .ranged()
        .named("Javelin", "Thrown hard and low, before they close in."),
    B::new(Active, Damage, Combat, Strike)
        .two(Blood, Piercing)
        .line()
        .ranged()
        .named(
            "Bleeding Cleave",
            "A wide arc of steel that opens everyone in a row.",
        ),
    B::new(Reactive, Damage, Combat, Strike)
        .one(Piercing)
        .trigger(Trigger::Hit)
        .named("Riposte", "Hit me once and the blade answers."),
    B::new(Passive, Buff, Combat, Strike)
        .one(Blunt)
        .effect(Effect::Protection)
        .named("Iron Skin", "Years of blows have left a hide like iron."),
];

/// Magical only.
pub const MAGE: [Preset; STARTING_MAX] = [
    B::new(Active, Damage, Magical, Projectile)
        .one(Cold)
        .ranged()
        .named("Frost Bolt", "A shard of ice, thrown from the palm."),
    B::new(Active, Damage, Magical, Projectile)
        .two(Cold, Fire)
        .ranged()
        .named("Frost Burn", "Cold so deep it burns."),
    B::new(Active, Damage, Magical, Projectile)
        .one(Cold)
        .effect(Effect::Freeze)
        .ranged()
        .named("Ice Prison", "The air around the target turns to ice."),
    B::new(Active, Damage, Magical, Strike)
        .one(Lightning)
        .line()
        .ranged()
        .named(
            "Arc",
            "Lightning runs down the line and spares no one on it.",
        ),
    B::new(Reactive, Debuff, Magical, Strike)
        .one(Cold)
        .effect(Effect::Slow)
        .trigger(Trigger::EnemyAdjacent)
        .named("Cold Snap", "Step too close and the frost takes your legs."),
    B::new(Passive, Buff, Magical, Projectile)
        .one(Cold)
        .effect(Effect::Empowerment)
        .named("Cold Mind", "A calm, cold focus behind every spell."),
];

/// Buffs and debuffs, any power source.
pub const SUPPORT: [Preset; STARTING_MAX] = [
    B::new(Active, Buff, Magical, Projectile)
        .one(Lightning)
        .effect(Effect::Haste)
        .ranged()
        .named("Quicken", "A spark in the blood: move, now."),
    B::new(Active, Debuff, Magical, Projectile)
        .one(Blood)
        .effect(Effect::Weakening)
        .ranged()
        .named("Sap", "Drains the strength out of a foe's arms."),
    B::new(Active, Debuff, Magical, Projectile)
        .one(Fire)
        .effect(Effect::Vulnerability)
        .ranged()
        .named("Expose", "Marks a foe with fire. Fire finds it easier now."),
    B::new(Active, Buff, Combat, Strike)
        .one(Blunt)
        .effect(Effect::Protection)
        .named("Shelter", "A hand on the shoulder and a shield in front."),
    B::new(Reactive, Buff, Combat, Strike)
        .one(Blunt)
        .effect(Effect::Protection)
        .trigger(Trigger::HpBelow)
        .below(40)
        .aura(2)
        .named(
            "Last Stand",
            "When the blood runs low, everyone near stands behind the shield.",
        ),
    B::new(Passive, Buff, Magical, Strike)
        .one(Cold)
        .effect(Effect::Protection)
        .aura(2)
        .named(
            "Cold Ward",
            "A ring of still, cold air guards those nearby.",
        ),
];

/// The starter set of a class.
pub const fn for_class(class: Class) -> &'static [Preset; STARTING_MAX] {
    match class {
        Class::Warrior => &WARRIOR,
        Class::Mage => &MAGE,
        Class::Support => &SUPPORT,
    }
}
