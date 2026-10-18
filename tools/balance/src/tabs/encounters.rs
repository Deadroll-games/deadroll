//! Encounters: which registry monsters at which level (elite or not, with which pool skills), on
//! what arena.
use deadroll_sim::{
    presets::{Arena, EncounterSlot},
    resolve_fight, EncounterPreset,
};
use eframe::egui::{self, Ui};

use crate::{app::BalanceApp, widgets};

pub fn show(app: &mut BalanceApp, ui: &mut Ui) {
    egui::Panel::left("encounter_list")
        .resizable(true)
        .default_size(300.0)
        .show(ui, |ui| {
            let action = widgets::preset_list(
                ui,
                &app.presets.encounters,
                |e| &e.name,
                &mut app.selected[3],
            );
            let first = app
                .presets
                .registry
                .monster_names
                .first()
                .cloned()
                .unwrap_or_default();
            widgets::apply_list_action(
                action,
                &mut app.presets.encounters,
                &mut app.selected[3],
                || EncounterPreset {
                    name: "New encounter".into(),
                    monsters: vec![EncounterSlot {
                        monster: first,
                        level: 1,
                        count: 1,
                        elite: false,
                        skills: None,
                    }],
                    arena: Arena::default(),
                },
                |e| &mut e.name,
            );
        });
    let Some(id) = app.selected[3]
        .clone()
        .filter(|id| app.presets.encounters.contains_key(id))
    else {
        ui.label("Pick an encounter on the left or press New.");
        return;
    };
    let monsters: Vec<(String, String)> = app
        .presets
        .registry
        .monster_names
        .iter()
        .map(|n| (n.clone(), n.clone()))
        .collect();
    let registry = app.presets.registry.clone();
    let cfg = app.cfg;
    egui::ScrollArea::vertical()
        .id_salt("encounter_edit")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(format!("Encounter `{id}`"));
                if ui.button("Use in the run").clicked() {
                    app.encounter = id.clone();
                }
            });
            let Some(e) = app.presets.encounters.get_mut(&id) else {
                return;
            };
            egui::Grid::new("encounter_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut e.name);
                    ui.end_row();
                    ui.label("Arena width / height");
                    ui.horizontal(|ui| {
                        widgets::int(ui, &mut e.arena.width, 3..=32);
                        widgets::int(ui, &mut e.arena.height, 2..=32);
                    });
                    ui.end_row();
                    ui.label("Gap (columns between the party and the monsters)");
                    widgets::int(ui, &mut e.arena.gap, 1..=30);
                    ui.end_row();
                });
            ui.add_space(6.0);
            ui.strong("Monsters");
            let mut remove = None;
            for (i, slot) in e.monsters.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    widgets::pick_id(ui, ("enc_monster", i), &mut slot.monster, &monsters);
                    ui.label("level");
                    widgets::int(ui, &mut slot.level, 0..=50);
                    ui.label("count");
                    widgets::int(ui, &mut slot.count, 0..=24);
                    ui.checkbox(&mut slot.elite, "elite");
                    if ui.small_button("✖").clicked() {
                        remove = Some(i);
                    }
                });
                // The pool: checked = taken (`Spawn.skills`); none checked = the default choice.
                if let Some(t) = registry
                    .monster_id(&slot.monster)
                    .and_then(|id| registry.template(id))
                {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("    skills:");
                        let mut mask = slot.skills.unwrap_or_else(|| {
                            let cats: Vec<_> = registry
                                .pool(&t)
                                .iter()
                                .flatten()
                                .map(|k| k.blocks.category)
                                .collect();
                            let flags = if slot.elite {
                                deadroll_types::SPAWN_ELITE
                            } else {
                                0
                            };
                            deadroll_sim::setup::default_skills(
                                &cats,
                                t.slots(flags, &cfg.monsters),
                            )
                        });
                        let before = mask;
                        for b in 0..t.pool_len.min(16) {
                            let at = t.pool_start as usize + b as usize;
                            let name = registry.skill_names.get(at).cloned().unwrap_or_default();
                            let mut on = mask & (1 << b) != 0;
                            if ui.checkbox(&mut on, name).changed() {
                                mask ^= 1 << b;
                            }
                        }
                        if mask != before {
                            slot.skills = Some(mask);
                        }
                        if slot.skills.is_some() && ui.small_button("default").clicked() {
                            slot.skills = None;
                        }
                    });
                }
            }
            if let Some(i) = remove {
                e.monsters.remove(i);
            }
            if !monsters.is_empty() && ui.button("+ monster").clicked() {
                e.monsters.push(EncounterSlot {
                    monster: monsters[0].0.clone(),
                    level: 1,
                    count: 1,
                    elite: false,
                    skills: None,
                });
            }
            ui.add_space(6.0);
            let (fight, issues) = resolve_fight(&app.presets, &id, &app.cfg);
            if let Some(f) = &fight {
                let bosses = f.spawns.iter().filter(|s| s.template.is_boss()).count();
                ui.label(format!(
                    "{} monsters; ends when {}",
                    f.spawns.len(),
                    if bosses > 0 {
                        "a boss dies"
                    } else {
                        "every monster dies"
                    }
                ));
            }
            widgets::issues(ui, &issues.errors, &issues.warnings);
        });
}
