//! Monsters: the registry bestiary (`assets/registry/bestiary.json`), the data the program copies
//! at `open_session`. Numbers are edited here and saved to that file; the skill pools are listed
//! from `monster_skills.json` (edited in the file). What a template becomes at a few levels uses
//! the current scaling.
use deadroll_rules::registry::{read_bestiary_json, write_bestiary_json, Registry, KINS};
use deadroll_sim::presets::{registry_dir, SkillPreset};
use eframe::egui::{self, Ui};

use crate::{app::BalanceApp, widgets};

pub fn show(app: &mut BalanceApp, ui: &mut Ui) {
    let dir = registry_dir(&app.root);
    if app.bestiary.is_none() {
        match read_bestiary_json(&dir) {
            Ok(b) => app.bestiary = Some(b),
            Err(e) => {
                ui.colored_label(egui::Color32::RED, e);
                return;
            }
        }
    }
    let Some(bestiary) = app.bestiary.as_mut() else {
        return;
    };
    egui::Panel::left("monster_list")
        .resizable(true)
        .default_size(260.0)
        .show(ui, |ui| {
            ui.strong("bestiary.json");
            for r in &bestiary.records {
                let picked = app.selected[2].as_deref() == Some(r.name.as_str());
                if ui.selectable_label(picked, &r.name).clicked() {
                    app.selected[2] = Some(r.name.clone());
                }
            }
        });
    let Some(name) = app.selected[2].clone() else {
        ui.label("Pick a monster on the left.");
        return;
    };
    let Some(index) = bestiary.records.iter().position(|r| r.name == name) else {
        ui.label("Pick a monster on the left.");
        return;
    };
    let scaling = app.cfg.monsters;
    let skills_cfg = app.cfg.skills;
    let mut save = false;
    let mut reload = false;
    egui::ScrollArea::vertical()
        .id_salt("monster_edit")
        .show(ui, |ui| {
            let m = &mut bestiary.records[index];
            ui.heading(format!("Monster `{}` (bestiary id {})", m.name, m.id));
            ui.label(format!(
                "{} · {} · size {} · min tier {} · kin {}",
                m.role,
                m.biome,
                m.size,
                m.min_tier,
                m.kin.as_deref().unwrap_or("none")
            ));
            egui::Grid::new("monster_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Natural attack");
                    ui.label(format!(
                        "{} {} {}",
                        m.attack_source, m.attack_form, m.attack_aspect
                    ));
                    ui.end_row();
                    ui.label("Damage min / max (level 0)");
                    ui.horizontal(|ui| {
                        widgets::int(ui, &mut m.damage_min, 0..=10_000);
                        widgets::int(ui, &mut m.damage_max, 0..=10_000);
                    });
                    ui.end_row();
                    ui.label("Accuracy (d20 bonus, level 0) / evasion (hit threshold)");
                    ui.horizontal(|ui| {
                        widgets::int(ui, &mut m.accuracy, 0..=100);
                        widgets::int(ui, &mut m.evasion, 0..=100);
                    });
                    ui.end_row();
                    ui.label("Armor");
                    widgets::int(ui, &mut m.armor, 0..=10_000);
                    ui.end_row();
                    ui.label("HP (level 0) / per level");
                    ui.horizontal(|ui| {
                        widgets::int(ui, &mut m.hp, 0..=1_000_000);
                        widgets::int(ui, &mut m.hp_per_level, 0..=100_000);
                    });
                    ui.end_row();
                    ui.label("Speed (action points) / initiative");
                    ui.horizontal(|ui| {
                        widgets::int(ui, &mut m.speed, 1..=255);
                        widgets::int(ui, &mut m.initiative, 0..=1_000);
                    });
                    ui.end_row();
                    ui.label("XP per level");
                    widgets::int(ui, &mut m.xp, 0..=100_000);
                    ui.end_row();
                    ui.label("Resists / immune / state immune");
                    ui.label(format!(
                        "{:?} / {:?} / {:?}",
                        m.resistance, m.immunity, m.state_immunity
                    ));
                    ui.end_row();
                    ui.label("Tags: resists / immune");
                    ui.label(format!("{:?} / {:?}", m.tag_resistance, m.tag_immunity));
                    ui.end_row();
                    ui.label("Skill pool (monster_skills.json)");
                    ui.horizontal(|ui| {
                        ui.label("start");
                        widgets::int(ui, &mut m.pool_start, 0..=127);
                        ui.label("length");
                        widgets::int(ui, &mut m.pool_len, 0..=16);
                    });
                    ui.end_row();
                    ui.label("Kin");
                    egui::ComboBox::from_id_salt("monster_kin")
                        .selected_text(m.kin.clone().unwrap_or_else(|| "none".into()))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut m.kin, None, "none");
                            for k in KINS {
                                ui.selectable_value(&mut m.kin, Some(k.to_string()), k);
                            }
                        });
                    ui.end_row();
                });
            ui.horizontal(|ui| {
                save = ui.button("Save bestiary.json").clicked();
                reload = ui.button("Reload from disk").clicked();
            });
            ui.add_space(8.0);
            // The saved registry: the pool and the numbers by level as the fights see them.
            let reg: &Registry = &app.presets.registry;
            let id = m.id;
            if let Some(t) = reg.template(id) {
                ui.strong("Skill pool (as saved)");
                for (i, k) in reg.pool(&t).iter().enumerate() {
                    let at = t.pool_start as usize + i;
                    let label = match k {
                        Some(k) => {
                            let p = SkillPreset::from_blocks("", &k.blocks, &k.ext);
                            let d = deadroll_skillcheck::derive_with(&k.blocks, &skills_cfg);
                            format!(
                                "{}: {}{} · power {:.0}% · AP {} · cooldown {}",
                                reg.skill_names.get(at).map_or("?", |n| n.as_str()),
                                p.summary(),
                                if k.kin_only() { " · kin only" } else { "" },
                                d.power_bp as f64 / 100.0,
                                d.ap_cost,
                                d.cooldown
                            )
                        }
                        None => format!("{at}: missing"),
                    };
                    ui.label(format!("  • {label}"));
                }
                for (rank, flags) in [("regular", 0), ("elite", deadroll_types::SPAWN_ELITE)] {
                    if t.is_boss() && flags != 0 {
                        continue;
                    }
                    let [a, r, p] = t.slots(flags, &scaling);
                    ui.label(format!(
                        "Slots ({rank}): {a} active, {r} reactive, {p} passive"
                    ));
                }
                ui.add_space(8.0);
                ui.strong("By level (current scaling)");
                egui::Grid::new("monster_levels")
                    .striped(true)
                    .show(ui, |ui| {
                        for h in ["level", "HP", "elite HP", "damage", "accuracy", "XP"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for level in [1u8, 2, 3, 5] {
                            let p = t.profile(level, &scaling);
                            ui.label(level.to_string());
                            ui.label(t.hp_at(level, 0, &scaling).to_string());
                            ui.label(
                                t.hp_at(level, deadroll_types::SPAWN_ELITE, &scaling)
                                    .to_string(),
                            );
                            ui.label(format!("{}..{}", p.weapon.min, p.weapon.max));
                            ui.label(p.attack_bonus.to_string());
                            ui.label(p.xp.to_string());
                            ui.end_row();
                        }
                    });
            }
        });
    if save {
        let result = app
            .bestiary
            .as_ref()
            .map(|b| write_bestiary_json(&dir, b))
            .unwrap_or(Ok(()));
        app.status = match result.and_then(|()| Registry::load(&dir)) {
            Ok(reg) => {
                app.presets.registry = reg;
                format!("saved {}", dir.join("bestiary.json").display())
            }
            Err(e) => format!("not saved: {e}"),
        };
    }
    if reload {
        app.bestiary = None;
        if let Ok(reg) = Registry::load(&dir) {
            app.presets.registry = reg;
        }
    }
}
