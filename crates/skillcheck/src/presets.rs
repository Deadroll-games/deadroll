//! Starter skill sets per class (docs/SKILL_BUILDER.md, section 10.1): 4 active, 1 reactive,
//! 1 passive. The app offers them at character creation; the program checks them like any
//! other starting skill. Names are the player's text, these are defaults.

use deadroll_types::{
    Aspect::{self, *},
    Class, Form, Source,
};

use crate::{Category, Effect, Purpose, SkillBlocks, Target, Trigger, STARTING_MAX};

/// A named starting skill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    pub name: &'static str,
    pub blocks: SkillBlocks,
}

struct B(SkillBlocks);

impl B {
    const fn new(category: Category, purpose: Purpose, source: Source, form: Form) -> Self {
        B(SkillBlocks {
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
        })
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
    const fn trigger(mut self, t: Trigger) -> Self {
        self.0.trigger = t;
        self
    }
    const fn aura(mut self, radius: u8) -> Self {
        self.0.aura_radius = radius;
        self
    }
    const fn named(self, name: &'static str) -> Preset {
        Preset {
            name,
            blocks: self.0,
        }
    }
}

use Category::{Active, Passive, Reactive};
use Form::{Projectile, Strike};
use Purpose::{Buff, Damage, Debuff};
use Source::{Combat, Magical};

/// Martial only.
pub const WARRIOR: [Preset; STARTING_MAX] = [
    B::new(Active, Damage, Combat, Strike)
        .one(Piercing)
        .named("Piercing Thrust"),
    B::new(Active, Damage, Combat, Strike)
        .one(Blunt)
        .effect(Effect::Stun)
        .named("Shield Bash"),
    B::new(Active, Damage, Combat, Projectile)
        .one(Piercing)
        .named("Javelin"),
    B::new(Active, Damage, Combat, Strike)
        .two(Blood, Piercing)
        .line()
        .named("Bleeding Cleave"),
    B::new(Reactive, Damage, Combat, Strike)
        .one(Piercing)
        .trigger(Trigger::Hit)
        .named("Riposte"),
    B::new(Passive, Buff, Combat, Strike)
        .one(Blunt)
        .effect(Effect::Protection)
        .named("Iron Skin"),
];

/// Magical only.
pub const MAGE: [Preset; STARTING_MAX] = [
    B::new(Active, Damage, Magical, Projectile)
        .one(Cold)
        .named("Frost Bolt"),
    B::new(Active, Damage, Magical, Projectile)
        .two(Cold, Fire)
        .named("Frost Burn"),
    B::new(Active, Damage, Magical, Projectile)
        .one(Cold)
        .effect(Effect::Freeze)
        .named("Ice Prison"),
    B::new(Active, Damage, Magical, Strike)
        .one(Lightning)
        .line()
        .named("Arc"),
    B::new(Reactive, Debuff, Magical, Strike)
        .one(Cold)
        .effect(Effect::Slow)
        .trigger(Trigger::EnemyAdjacent)
        .named("Cold Snap"),
    B::new(Passive, Buff, Magical, Projectile)
        .one(Cold)
        .effect(Effect::Empowerment)
        .named("Cold Mind"),
];

/// Buffs and debuffs, any power source.
pub const SUPPORT: [Preset; STARTING_MAX] = [
    B::new(Active, Buff, Magical, Projectile)
        .one(Lightning)
        .effect(Effect::Haste)
        .named("Quicken"),
    B::new(Active, Debuff, Magical, Projectile)
        .one(Blood)
        .effect(Effect::Weakening)
        .named("Sap"),
    B::new(Active, Debuff, Magical, Projectile)
        .one(Fire)
        .effect(Effect::Vulnerability)
        .named("Expose"),
    B::new(Active, Buff, Combat, Strike)
        .one(Blunt)
        .effect(Effect::Protection)
        .named("Shelter"),
    B::new(Reactive, Buff, Combat, Strike)
        .one(Blunt)
        .effect(Effect::Protection)
        .trigger(Trigger::HpBelow)
        .aura(2)
        .named("Last Stand"),
    B::new(Passive, Buff, Magical, Strike)
        .one(Cold)
        .effect(Effect::Protection)
        .aura(2)
        .named("Cold Ward"),
];

/// The starter set of a class.
pub const fn for_class(class: Class) -> &'static [Preset; STARTING_MAX] {
    match class {
        Class::Warrior => &WARRIOR,
        Class::Mage => &MAGE,
        Class::Support => &SUPPORT,
    }
}
