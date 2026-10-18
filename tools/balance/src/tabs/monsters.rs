//! Monsters: a template at level 0 and what it becomes at a few levels with the current scaling.
use deadroll_sim::{
    named::Named,
    presets::{AttackPreset, MonsterPreset},
};
use deadroll_types::{Aspect, Form, Source, StateKind};
use eframe::egui::{self, Ui};

use crate::{app::BalanceApp, widgets};

fn fresh() -> MonsterPreset {
    MonsterPreset {
        name: "New monster".into(),
        bestiary_id: None,
        boss: false,
        attack: AttackPreset {
            aspect: Aspect::Piercing,
            source: Source::Combat,
            form: Form::Strike,
            min: 1,
            max: 3,
        },
        attack_bonus: 1,
        defense: 10,
        armor: 0,
        resistance: Vec::new(),
        immunity: Vec::new(),
        state_immunity: Vec::new(),
        xp_per_level: 5,
        hp: 10,
        hp_per_level: 2,
        speed: 4,
        initiative: 5,
    }
}

pub fn show(app: &mut BalanceApp, ui: &mut Ui) {
    egui::Panel::left("monster_list")
        .resizable(true)
        .default_size(280.0)
        .show(ui, |ui| {
            let action =
                widgets::preset_list(ui, &app.presets.monsters, |m| &m.name, &mut app.selected[2]);
            widgets::apply_list_action(
                action,
                &mut app.presets.monsters,
                &mut app.selected[2],
                fresh,
                |m| &mut m.name,
            );
        });
    let Some(id) = app.selected[2]
        .clone()
        .filter(|id| app.presets.monsters.contains_key(id))
    else {
        ui.label("Pick a monster on the left or press New.");
        return;
    };
    let scaling = app.cfg.monsters;
    let registry = app.presets.registry.clone();
    egui::ScrollArea::vertical()
        .id_salt("monster_edit")
        .show(ui, |ui| {
            ui.heading(format!("Monster `{id}`"));
            let Some(m) = app.presets.monsters.get_mut(&id) else {
                return;
            };
            egui::Grid::new("monster_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut m.name);
                    ui.end_row();
                    ui.label("Registry bestiary id");
                    ui.horizontal(|ui| {
                        let mut has = m.bestiary_id.is_some();
                        ui.checkbox(&mut has, "");
                        match (has, &mut m.bestiary_id) {
                            (true, Some(v)) => {
                                widgets::int(ui, v, 0..=255);
                            }
                            (true, None) => m.bestiary_id = Some(0),
                            (false, _) => m.bestiary_id = None,
                        }
                    });
                    ui.end_row();
                    ui.label("Boss (its death wins)");
                    ui.checkbox(&mut m.boss, "");
                    ui.end_row();
                    ui.label("Attack aspect / source / form");
                    ui.horizontal(|ui| {
                        widgets::pick(ui, "attack_aspect", &mut m.attack.aspect);
                        widgets::pick(ui, "attack_source", &mut m.attack.source);
                        widgets::pick(ui, "attack_form", &mut m.attack.form);
                    });
                    ui.end_row();
                    ui.label("Attack damage min / max (level 0)");
                    ui.horizontal(|ui| {
                        widgets::int(ui, &mut m.attack.min, 0..=10_000);
                        widgets::int(ui, &mut m.attack.max, 0..=10_000);
                    });
                    ui.end_row();
                    ui.label("Attack bonus (level 0)");
                    widgets::int(ui, &mut m.attack_bonus, 0..=100);
                    ui.end_row();
                    ui.label("Defense");
                    widgets::int(ui, &mut m.defense, 0..=100);
                    ui.end_row();
                    ui.label("Armor");
                    widgets::int(ui, &mut m.armor, 0..=10_000);
                    ui.end_row();
                    if m.bestiary_id.is_some() {
                        // The program reads these from the registry record: shown, not edited.
                        let d = m.defense(&registry);
                        let names = |mask: u8, all: &[String]| {
                            let list: Vec<&str> = all
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| mask & (1 << i) != 0)
                                .map(|(_, n)| n.as_str())
                                .collect();
                            if list.is_empty() {
                                "none".to_string()
                            } else {
                                list.join(", ")
                            }
                        };
                        let aspects: Vec<String> = Aspect::all().iter().map(|a| a.name()).collect();
                        let states: Vec<String> =
                            StateKind::all().iter().map(|k| k.name()).collect();
                        ui.label("Resists (registry bestiary.json)");
                        ui.label(names(d.resist, &aspects));
                        ui.end_row();
                        ui.label("Immune to aspects / states");
                        ui.label(format!(
                            "{} / {}",
                            names(d.immune_aspects, &aspects),
                            names(d.immune_states, &states)
                        ));
                        ui.end_row();
                    } else {
                        let toggle = |ui: &mut Ui, list: &mut Vec<Aspect>, a: Aspect| {
                            let mut on = list.contains(&a);
                            if ui.checkbox(&mut on, a.name()).changed() {
                                list.retain(|x| *x != a);
                                if on {
                                    list.push(a);
                                }
                            }
                        };
                        ui.label("Resists (at the scaling's resistance)");
                        ui.horizontal_wrapped(|ui| {
                            for &a in Aspect::all() {
                                toggle(ui, &mut m.resistance, a);
                            }
                        });
                        ui.end_row();
                        ui.label("Immune to aspects");
                        ui.horizontal_wrapped(|ui| {
                            for &a in Aspect::all() {
                                toggle(ui, &mut m.immunity, a);
                            }
                        });
                        ui.end_row();
                        ui.label("Immune to states");
                        ui.horizontal_wrapped(|ui| {
                            for &k in StateKind::all() {
                                let mut on = m.state_immunity.contains(&k);
                                if ui.checkbox(&mut on, k.name()).changed() {
                                    m.state_immunity.retain(|x| *x != k);
                                    if on {
                                        m.state_immunity.push(k);
                                    }
                                }
                            }
                        });
                        ui.end_row();
                    }
                    ui.label("HP (level 0) / per level");
                    ui.horizontal(|ui| {
                        widgets::int(ui, &mut m.hp, 0..=1_000_000);
                        widgets::int(ui, &mut m.hp_per_level, 0..=100_000);
                    });
                    ui.end_row();
                    ui.label("Speed (action points)");
                    widgets::int(ui, &mut m.speed, 0..=255);
                    ui.end_row();
                    ui.label("Initiative");
                    widgets::int(ui, &mut m.initiative, 0..=1_000);
                    ui.end_row();
                    ui.label("XP per level");
                    widgets::int(ui, &mut m.xp_per_level, 0..=100_000);
                    ui.end_row();
                });
            ui.add_space(8.0);
            ui.strong("By level (current scaling)");
            let base = m.base();
            let defense = m.defense(&registry);
            egui::Grid::new("monster_levels")
                .striped(true)
                .show(ui, |ui| {
                    for h in ["level", "HP", "damage", "attack bonus", "XP"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for level in [1u8, 2, 3, 5] {
                        let p = base.profile(level, &scaling, &defense);
                        ui.label(level.to_string());
                        ui.label(base.hp_at(level).to_string());
                        ui.label(format!("{}..{}", p.weapon.min, p.weapon.max));
                        ui.label(p.attack_bonus.to_string());
                        ui.label(p.xp.to_string());
                        ui.end_row();
                    }
                });
            let warnings = m.warnings();
            widgets::issues(ui, &[], &warnings);
        });
}
