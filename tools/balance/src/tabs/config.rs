//! Every number of `BalanceConfig`. Basis points are edited as percentages, values that differ from
//! the program's defaults are marked, tables indexed by an enum list its `ALL`.
use deadroll_combat::ModKind;
use deadroll_sim::{config_io, named::Named, BalanceConfig};
use deadroll_skillcheck::SKILL_ASPECTS_MAX;
use deadroll_types::{Class, Race, StateKind};
use eframe::egui::{self, emath::Numeric, Ui};

use crate::{app::BalanceApp, widgets};

fn bp_row(ui: &mut Ui, label: &str, v: &mut u32, default: u32) {
    widgets::label(ui, label, *v != default);
    widgets::bp(ui, v);
    ui.end_row();
}

fn int_row<N: Numeric + PartialEq>(
    ui: &mut Ui,
    label: &str,
    v: &mut N,
    default: N,
    range: std::ops::RangeInclusive<N>,
) {
    widgets::label(ui, label, *v != default);
    widgets::int(ui, v, range);
    ui.end_row();
}

/// One row per variant of `E`, a bp or integer cell each.
fn by_variant<E: Named, T: Copy + PartialEq>(
    ui: &mut Ui,
    id: &str,
    label: &str,
    values: &mut [T],
    defaults: &[T],
    mut edit: impl FnMut(&mut Ui, &mut T),
) {
    ui.label(label);
    egui::Grid::new(id).striped(true).show(ui, |ui| {
        for (i, e) in E::all().iter().enumerate() {
            if let (Some(v), Some(d)) = (values.get_mut(i), defaults.get(i)) {
                widgets::label(ui, &e.name(), v != d);
                edit(ui, v);
                ui.end_row();
            }
        }
    });
}

pub fn show(app: &mut BalanceApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if ui.button("Reset to defaults").clicked() {
            app.cfg = BalanceConfig::DEFAULT;
        }
        if ui.button("Save JSON…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .set_directory(&app.root)
                .set_file_name("balance.json")
                .save_file()
            {
                app.status = match std::fs::write(&path, config_io::to_json(&app.cfg)) {
                    Ok(()) => format!("config saved to {}", path.display()),
                    Err(e) => format!("config save failed: {e}"),
                };
            }
        }
        if ui.button("Load JSON…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .set_directory(&app.root)
                .pick_file()
            {
                app.status = match std::fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|t| config_io::from_json(&t))
                {
                    Ok(cfg) => {
                        app.cfg = cfg;
                        format!("config loaded from {}", path.display())
                    }
                    Err(e) => format!("config load failed: {e}"),
                };
            }
        }
        let changed = app.cfg != BalanceConfig::DEFAULT;
        ui.label(if changed {
            "* marks values that differ from the program's defaults"
        } else {
            "all values are the program's defaults"
        });
    });
    ui.separator();
    let d = BalanceConfig::DEFAULT;
    let c = &mut app.cfg;
    egui::ScrollArea::vertical()
        .id_salt("config")
        .show(ui, |ui| {
            egui::CollapsingHeader::new("Hit math (crates/combat Balance)")
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("combat")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            bp_row(
                                ui,
                                "Crit damage bonus (more)",
                                &mut c.combat.crit_more_bp,
                                d.combat.crit_more_bp,
                            );
                            int_row(
                                ui,
                                "Armor constant K",
                                &mut c.combat.armor_k,
                                d.combat.armor_k,
                                1..=100_000,
                            );
                            bp_row(
                                ui,
                                "Resistance cap (counterweight)",
                                &mut c.combat.resist_cap_bp,
                                d.combat.resist_cap_bp,
                            );
                            bp_row(
                                ui,
                                "Chance floor",
                                &mut c.combat.chance_min_bp,
                                d.combat.chance_min_bp,
                            );
                            bp_row(
                                ui,
                                "Chance ceiling",
                                &mut c.combat.chance_max_bp,
                                d.combat.chance_max_bp,
                            );
                            int_row(
                                ui,
                                "Minimum damage",
                                &mut c.combat.min_damage,
                                d.combat.min_damage,
                                0..=1_000,
                            );
                            int_row(
                                ui,
                                "State duration cap",
                                &mut c.combat.state_turns_max,
                                d.combat.state_turns_max,
                                0..=50,
                            );
                            bp_row(
                                ui,
                                "Damage over time share",
                                &mut c.combat.dot_share_bp,
                                d.combat.dot_share_bp,
                            );
                            bp_row(
                                ui,
                                "Chill buildup per hit",
                                &mut c.combat.chill_buildup_bp,
                                d.combat.chill_buildup_bp,
                            );
                            bp_row(
                                ui,
                                "Freeze threshold",
                                &mut c.combat.freeze_threshold_bp,
                                d.combat.freeze_threshold_bp,
                            );
                            bp_row(
                                ui,
                                "Chill slow share",
                                &mut c.combat.chill_slow_share_bp,
                                d.combat.chill_slow_share_bp,
                            );
                        });
                    by_variant::<StateKind, u8>(
                        ui,
                        "state_turns",
                        "State duration, target turns",
                        &mut c.combat.state_turns,
                        &d.combat.state_turns,
                        |ui, v| {
                            widgets::int(ui, v, 0..=50);
                        },
                    );
                    ui.label("Tier tables (tier 1 best)");
                    egui::Grid::new("tiers").striped(true).show(ui, |ui| {
                        ui.label("");
                        for t in 1..=14 {
                            ui.label(format!("t{t}"));
                        }
                        ui.end_row();
                        let rows: [(&str, &mut [u32; 14], &[u32; 14]); 3] = [
                            (
                                "Tint chance",
                                &mut c.combat.tint_chance_bp,
                                &d.combat.tint_chance_bp,
                            ),
                            (
                                "Resonance more",
                                &mut c.combat.resonance_more_bp,
                                &d.combat.resonance_more_bp,
                            ),
                            (
                                "Pure pair",
                                &mut c.combat.pure_pair_bp,
                                &d.combat.pure_pair_bp,
                            ),
                        ];
                        for (name, values, defaults) in rows {
                            widgets::label(ui, name, values != defaults);
                            for v in values.iter_mut() {
                                widgets::bp(ui, v);
                            }
                            ui.end_row();
                        }
                    });
                });

            egui::CollapsingHeader::new("Buffs, debuffs, skill effects (SupportBalance)")
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("support")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            bp_row(
                                ui,
                                "Skill effect chance on a hit",
                                &mut c.support.rider_chance_bp,
                                d.support.rider_chance_bp,
                            );
                        });
                    egui::Grid::new("mods").striped(true).show(ui, |ui| {
                        for h in ["", "strength min", "strength max", "own turns"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for &kind in ModKind::all() {
                            let i = kind as usize;
                            let changed = c.support.mod_min_bp[i] != d.support.mod_min_bp[i]
                                || c.support.mod_max_bp[i] != d.support.mod_max_bp[i]
                                || c.support.mod_duration[i] != d.support.mod_duration[i];
                            widgets::label(ui, &kind.name(), changed);
                            widgets::bp(ui, &mut c.support.mod_min_bp[i]);
                            widgets::bp(ui, &mut c.support.mod_max_bp[i]);
                            widgets::int(ui, &mut c.support.mod_duration[i], 0..=20);
                            ui.end_row();
                        }
                    });
                });

            egui::CollapsingHeader::new("Skills (skillcheck derive)")
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("skills")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            bp_row(
                                ui,
                                "Line target counterweight",
                                &mut c.skills.line_counter_bp,
                                d.skills.line_counter_bp,
                            );
                            bp_row(
                                ui,
                                "Effect on damage counterweight",
                                &mut c.skills.effect_counter_bp,
                                d.skills.effect_counter_bp,
                            );
                            bp_row(
                                ui,
                                "Third aspect counterweight",
                                &mut c.skills.third_aspect_counter_bp,
                                d.skills.third_aspect_counter_bp,
                            );
                            int_row(
                                ui,
                                "Active cooldown (base)",
                                &mut c.skills.active_cooldown,
                                d.skills.active_cooldown,
                                0..=20,
                            );
                            int_row(
                                ui,
                                "Reactive cooldown",
                                &mut c.skills.reactive_cooldown,
                                d.skills.reactive_cooldown,
                                0..=20,
                            );
                        });
                    ui.label("Aspect shares by aspect count (each row should sum to 100%)");
                    egui::Grid::new("shares").striped(true).show(ui, |ui| {
                        for n in 0..SKILL_ASPECTS_MAX {
                            let row = &mut c.skills.shares_bp[n];
                            widgets::label(
                                ui,
                                &format!("{} aspect(s)", n + 1),
                                row != &d.skills.shares_bp[n],
                            );
                            for v in row.iter_mut().take(n + 1) {
                                widgets::bp(ui, v);
                            }
                            let sum: u32 = row.iter().take(n + 1).sum();
                            if sum != 10_000 {
                                ui.colored_label(
                                    widgets::WARNING,
                                    format!("sum {:.2}%", sum as f64 / 100.0),
                                );
                            }
                            ui.end_row();
                        }
                    });
                });

            egui::CollapsingHeader::new("Heroes (PlayerTuning)")
                .default_open(true)
                .show(ui, |ui| {
                    let (p, dp) = (&mut c.players, &d.players);
                    ui.label("Base action points");
                    egui::Grid::new("ap_base").striped(true).show(ui, |ui| {
                        ui.label("");
                        for class in Class::all() {
                            ui.strong(class.name());
                        }
                        ui.end_row();
                        for &race in Race::all() {
                            let (r, dr) =
                                (&mut p.ap_base[race as usize], &dp.ap_base[race as usize]);
                            widgets::label(ui, &race.name(), r != dr);
                            for v in r.iter_mut() {
                                widgets::int(ui, v, 0..=30);
                            }
                            ui.end_row();
                        }
                    });
                    egui::Grid::new("players")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            int_row(
                                ui,
                                "Dexterity per extra AP",
                                &mut p.ap_dex_div,
                                dp.ap_dex_div,
                                1..=1_000,
                            );
                            int_row(ui, "AP min", &mut p.ap_min, dp.ap_min, 0..=30);
                            int_row(ui, "AP max", &mut p.ap_max, dp.ap_max, 1..=60);
                            int_row(ui, "AP per action", &mut p.act_ap, dp.act_ap, 0..=30);
                            int_row(
                                ui,
                                "Initiative = Intelligence /",
                                &mut p.initiative_div,
                                dp.initiative_div,
                                1..=100,
                            );
                            int_row(
                                ui,
                                "Strike reach",
                                &mut p.strike_range,
                                dp.strike_range,
                                1..=10,
                            );
                            int_row(
                                ui,
                                "Projectile reach",
                                &mut p.projectile_range,
                                dp.projectile_range,
                                1..=30,
                            );
                            bp_row(
                                ui,
                                "Skill state chance",
                                &mut p.skill_state_chance_bp,
                                dp.skill_state_chance_bp,
                            );
                            bp_row(
                                ui,
                                "Weapon state chance",
                                &mut p.basic_state_chance_bp,
                                dp.basic_state_chance_bp,
                            );
                            bp_row(
                                ui,
                                "Heal per weapon point",
                                &mut p.heal_weapon_bp,
                                dp.heal_weapon_bp,
                            );
                            int_row(
                                ui,
                                "Predisposition damage min",
                                &mut p.predisposition_added[0],
                                dp.predisposition_added[0],
                                0..=1_000,
                            );
                            int_row(
                                ui,
                                "Predisposition damage max",
                                &mut p.predisposition_added[1],
                                dp.predisposition_added[1],
                                0..=1_000,
                            );
                            int_row(
                                ui,
                                "Attack bonus = level + stat /",
                                &mut p.attack_stat_div,
                                dp.attack_stat_div,
                                1..=100,
                            );
                            bp_row(
                                ui,
                                "Crit per Dexterity point",
                                &mut p.crit_per_dex_bp,
                                dp.crit_per_dex_bp,
                            );
                            int_row(
                                ui,
                                "Defense base",
                                &mut p.defense_base,
                                dp.defense_base,
                                0..=100,
                            );
                            int_row(
                                ui,
                                "Defense = base + Dexterity /",
                                &mut p.defense_dex_div,
                                dp.defense_dex_div,
                                1..=100,
                            );
                            int_row(
                                ui,
                                "HP per Endurance",
                                &mut p.hp_per_endurance,
                                dp.hp_per_endurance,
                                0..=100,
                            );
                            int_row(
                                ui,
                                "Creation: stat base",
                                &mut p.stat_base,
                                dp.stat_base,
                                0..=100,
                            );
                            int_row(
                                ui,
                                "Creation: points",
                                &mut p.stat_points_start,
                                dp.stat_points_start,
                                0..=200,
                            );
                            int_row(
                                ui,
                                "Creation: stat cap",
                                &mut p.stat_max_start,
                                dp.stat_max_start,
                                0..=200,
                            );
                            int_row(
                                ui,
                                "Creation: tier",
                                &mut p.start_tier,
                                dp.start_tier,
                                1..=14,
                            );
                        });
                    ui.label("By class");
                    egui::Grid::new("by_class").striped(true).show(ui, |ui| {
                        for h in [
                            "",
                            "class increase",
                            "support increase",
                            "armor",
                            "HP base",
                            "HP / level",
                        ] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for &class in Class::all() {
                            let i = class as usize;
                            let changed = p.class_increase_bp[i] != dp.class_increase_bp[i]
                                || p.support_increase_bp[i] != dp.support_increase_bp[i]
                                || p.armor[i] != dp.armor[i]
                                || p.hp_base[i] != dp.hp_base[i]
                                || p.hp_per_level[i] != dp.hp_per_level[i];
                            widgets::label(ui, &class.name(), changed);
                            widgets::bp(ui, &mut p.class_increase_bp[i]);
                            widgets::bp(ui, &mut p.support_increase_bp[i]);
                            widgets::int(ui, &mut p.armor[i], 0..=10_000);
                            widgets::int(ui, &mut p.hp_base[i], 0..=100_000);
                            widgets::int(ui, &mut p.hp_per_level[i], 0..=10_000);
                            ui.end_row();
                        }
                    });
                });

            egui::CollapsingHeader::new("Monster scaling (MonsterScaling)")
                .default_open(true)
                .show(ui, |ui| {
                    let (m, dm) = (&mut c.monsters, &d.monsters);
                    egui::Grid::new("monsters")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            bp_row(
                                ui,
                                "Weapon min per level",
                                &mut m.weapon_min_per_level_bp,
                                dm.weapon_min_per_level_bp,
                            );
                            bp_row(
                                ui,
                                "Weapon max per level",
                                &mut m.weapon_max_per_level_bp,
                                dm.weapon_max_per_level_bp,
                            );
                            bp_row(
                                ui,
                                "Attack bonus per level",
                                &mut m.attack_per_level_bp,
                                dm.attack_per_level_bp,
                            );
                            int_row(ui, "AP min", &mut m.ap_min, dm.ap_min, 0..=30);
                            int_row(ui, "AP max", &mut m.ap_max, dm.ap_max, 1..=60);
                        });
                });
        });
}
