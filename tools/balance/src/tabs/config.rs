//! Every number of `BalanceConfig`. Basis points are edited as percentages, values that differ from
//! the program's defaults are marked, tables indexed by an enum list its `ALL`.
use deadroll_combat::ModKind;
use deadroll_rules::TIERS;
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
                            int_row(
                                ui,
                                "Chance floor, percent",
                                &mut c.combat.chance_min_pct,
                                d.combat.chance_min_pct,
                                0..=100,
                            );
                            int_row(
                                ui,
                                "Chance ceiling, percent",
                                &mut c.combat.chance_max_pct,
                                d.combat.chance_max_pct,
                                0..=100,
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
                            bp_row(
                                ui,
                                "Shock vulnerability",
                                &mut c.combat.shock_vuln_bp,
                                d.combat.shock_vuln_bp,
                            );
                            bp_row(
                                ui,
                                "Penetration strength",
                                &mut c.combat.penetration_power_bp,
                                d.combat.penetration_power_bp,
                            );
                            int_row(
                                ui,
                                "Resonance state chance, percent",
                                &mut c.combat.extra_state_chance_pct,
                                d.combat.extra_state_chance_pct,
                                0..=100,
                            );
                            int_row(
                                ui,
                                "Resonance modifier, own turns",
                                &mut c.combat.res_mod_turns,
                                d.combat.res_mod_turns,
                                0..=20,
                            );
                            int_row(
                                ui,
                                "Accuracy buff, own turns",
                                &mut c.combat.res_accuracy_turns,
                                d.combat.res_accuracy_turns,
                                0..=20,
                            );
                            int_row(
                                ui,
                                "Burning cells, rounds",
                                &mut c.combat.burning_cell_rounds,
                                d.combat.burning_cell_rounds,
                                0..=20,
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
                        widgets::label(
                            ui,
                            "Tint chance, percent",
                            c.combat.tint_chance_pct != d.combat.tint_chance_pct,
                        );
                        for v in c.combat.tint_chance_pct.iter_mut() {
                            widgets::int(ui, v, 0..=100);
                        }
                        ui.end_row();
                        let rows: [(&str, &mut [u32; 14], &[u32; 14]); 5] = [
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
                            (
                                "Keenness penetration",
                                &mut c.combat.keenness_pen_bp,
                                &d.combat.keenness_pen_bp,
                            ),
                            (
                                "Resonance state power",
                                &mut c.combat.res_power_bp,
                                &d.combat.res_power_bp,
                            ),
                            (
                                "Resonance modifier",
                                &mut c.combat.res_mod_bp,
                                &d.combat.res_mod_bp,
                            ),
                        ];
                        for (name, values, defaults) in rows {
                            widgets::label(ui, name, values != defaults);
                            for v in values.iter_mut() {
                                widgets::bp(ui, v);
                            }
                            ui.end_row();
                        }
                        widgets::label(
                            ui,
                            "Resonance extra turns",
                            c.combat.res_turns != d.combat.res_turns,
                        );
                        for v in c.combat.res_turns.iter_mut() {
                            widgets::int(ui, v, 0..=10);
                        }
                        ui.end_row();
                        widgets::label(
                            ui,
                            "Accuracy buff, d20",
                            c.combat.res_accuracy != d.combat.res_accuracy,
                        );
                        for v in c.combat.res_accuracy.iter_mut() {
                            widgets::int(ui, v, 0..=20);
                        }
                        ui.end_row();
                        widgets::label(
                            ui,
                            "Tint added damage min / max",
                            c.combat.tint_added != d.combat.tint_added,
                        );
                        for [lo, hi] in c.combat.tint_added.iter_mut() {
                            ui.vertical(|ui| {
                                widgets::int(ui, lo, 0..=1_000);
                                widgets::int(ui, hi, 0..=1_000);
                            });
                        }
                        ui.end_row();
                    });
                });

            egui::CollapsingHeader::new("Buffs, debuffs, skill effects (SupportBalance)")
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("support")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            int_row(
                                ui,
                                "Skill effect chance on a hit, percent",
                                &mut c.support.rider_chance_pct,
                                d.support.rider_chance_pct,
                                0..=100,
                            );
                            bp_row(
                                ui,
                                "Passive skill strength (point of the range)",
                                &mut c.support.passive_point_bp,
                                d.support.passive_point_bp,
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
                            int_row(
                                ui,
                                "AP of an active skill (base)",
                                &mut c.skills.ap_base,
                                d.skills.ap_base,
                                0..=30,
                            );
                            int_row(
                                ui,
                                "AP + line",
                                &mut c.skills.ap_line,
                                d.skills.ap_line,
                                0..=30,
                            );
                            int_row(
                                ui,
                                "AP + third aspect",
                                &mut c.skills.ap_third_aspect,
                                d.skills.ap_third_aspect,
                                0..=30,
                            );
                            int_row(
                                ui,
                                "AP + effect on damage",
                                &mut c.skills.ap_effect,
                                d.skills.ap_effect,
                                0..=30,
                            );
                            int_row(
                                ui,
                                "AP + hard control",
                                &mut c.skills.ap_hard_control,
                                d.skills.ap_hard_control,
                                0..=30,
                            );
                            int_row(
                                ui,
                                "HP threshold min, percent",
                                &mut c.skills.hp_below_min_pct,
                                d.skills.hp_below_min_pct,
                                1..=99,
                            );
                            int_row(
                                ui,
                                "HP threshold max, percent",
                                &mut c.skills.hp_below_max_pct,
                                d.skills.hp_below_max_pct,
                                1..=99,
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
                            int_row(
                                ui,
                                "AP of the basic attack",
                                &mut p.basic_attack_ap,
                                dp.basic_attack_ap,
                                0..=30,
                            );
                            int_row(
                                ui,
                                "Initiative = Intelligence /",
                                &mut p.initiative_div,
                                dp.initiative_div,
                                1..=100,
                            );
                            int_row(
                                ui,
                                "Melee reach",
                                &mut p.melee_range,
                                dp.melee_range,
                                1..=10,
                            );
                            int_row(
                                ui,
                                "Ranged reach",
                                &mut p.ranged_range,
                                dp.ranged_range,
                                1..=30,
                            );
                            int_row(
                                ui,
                                "Nearby ally (AllyHpBelow), cells",
                                &mut p.nearby_ally_cells,
                                dp.nearby_ally_cells,
                                0..=30,
                            );
                            int_row(
                                ui,
                                "Skill state chance, percent",
                                &mut p.skill_state_chance_pct,
                                dp.skill_state_chance_pct,
                                0..=100,
                            );
                            int_row(
                                ui,
                                "Weapon state chance, percent",
                                &mut p.basic_state_chance_pct,
                                dp.basic_state_chance_pct,
                                0..=100,
                            );
                            bp_row(
                                ui,
                                "Heal per weapon point",
                                &mut p.heal_weapon_bp,
                                dp.heal_weapon_bp,
                            );
                            int_row(
                                ui,
                                "Accuracy base",
                                &mut p.accuracy_base,
                                dp.accuracy_base,
                                0..=100,
                            );
                            int_row(
                                ui,
                                "Accuracy = base + Dexterity /",
                                &mut p.accuracy_dex_div,
                                dp.accuracy_dex_div,
                                1..=100,
                            );
                            int_row(
                                ui,
                                "Evasion base",
                                &mut p.defense_base,
                                dp.defense_base,
                                0..=100,
                            );
                            int_row(
                                ui,
                                "Evasion = base + Dexterity /",
                                &mut p.defense_dex_div,
                                dp.defense_dex_div,
                                1..=100,
                            );
                            bp_row(
                                ui,
                                "Strength: martial strike damage / point",
                                &mut p.str_damage_bp,
                                dp.str_damage_bp,
                            );
                            bp_row(
                                ui,
                                "Dexterity: martial projectile damage / point",
                                &mut p.dex_projectile_bp,
                                dp.dex_projectile_bp,
                            );
                            bp_row(
                                ui,
                                "Intelligence: magical damage / point",
                                &mut p.int_magic_bp,
                                dp.int_magic_bp,
                            );
                            bp_row(
                                ui,
                                "Strength: martial state chance / point",
                                &mut p.str_state_chance_bp,
                                dp.str_state_chance_bp,
                            );
                            bp_row(
                                ui,
                                "Intelligence: magical state chance / point",
                                &mut p.int_state_chance_bp,
                                dp.int_state_chance_bp,
                            );
                            bp_row(
                                ui,
                                "Dexterity: tempo effect chance / point",
                                &mut p.dex_state_chance_bp,
                                dp.dex_state_chance_bp,
                            );
                            int_row(
                                ui,
                                "Own buff on self: +1 turn per Endurance",
                                &mut p.self_buff_end_div,
                                dp.self_buff_end_div,
                                1..=1_000,
                            );
                            int_row(
                                ui,
                                "Own buff on self: extra turns max",
                                &mut p.self_buff_turns_max,
                                dp.self_buff_turns_max,
                                0..=10,
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
                                "Creation: race trait tier",
                                &mut p.start_tier,
                                dp.start_tier,
                                1..=14,
                            );
                            int_row(
                                ui,
                                "Creation: class trait tier",
                                &mut p.class_start_tier,
                                dp.class_start_tier,
                                1..=14,
                            );
                            int_row(
                                ui,
                                "Stat points per level",
                                &mut p.stat_points_per_level,
                                dp.stat_points_per_level,
                                0..=50,
                            );
                        });
                    ui.label("Total XP for each level");
                    egui::Grid::new("level_xp").striped(true).show(ui, |ui| {
                        for (i, (v, d)) in p.level_xp.iter_mut().zip(&dp.level_xp).enumerate() {
                            widgets::label(ui, &format!("level {}", i + 1), v != d);
                            widgets::int(ui, v, 0..=10_000_000);
                            ui.end_row();
                        }
                    });
                    ui.label("By class");
                    egui::Grid::new("by_class").striped(true).show(ui, |ui| {
                        for h in ["", "armor", "HP base", "HP / level"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for &class in Class::all() {
                            let i = class as usize;
                            let changed = p.armor[i] != dp.armor[i]
                                || p.hp_base[i] != dp.hp_base[i]
                                || p.hp_per_level[i] != dp.hp_per_level[i];
                            widgets::label(ui, &class.name(), changed);
                            widgets::int(ui, &mut p.armor[i], 0..=10_000);
                            widgets::int(ui, &mut p.hp_base[i], 0..=100_000);
                            widgets::int(ui, &mut p.hp_per_level[i], 0..=10_000);
                            ui.end_row();
                        }
                    });
                    ui.label(
                        "Trait tiers (1 best): race trait = predisposition damage, class trait = \
                         class increase and support increase",
                    );
                    egui::Grid::new("by_tier").striped(true).show(ui, |ui| {
                        ui.strong("tier");
                        ui.strong("race min");
                        ui.strong("race max");
                        for class in Class::all() {
                            ui.strong(format!("{} increase", class.name()));
                            ui.strong(format!("{} support", class.name()));
                        }
                        ui.end_row();
                        for t in 0..TIERS {
                            let changed = p.predisposition_added_by_tier[t]
                                != dp.predisposition_added_by_tier[t]
                                || Class::all().iter().any(|&c| {
                                    let c = c as usize;
                                    p.class_increase_by_tier_bp[c][t]
                                        != dp.class_increase_by_tier_bp[c][t]
                                        || p.support_increase_by_tier_bp[c][t]
                                            != dp.support_increase_by_tier_bp[c][t]
                                });
                            widgets::label(ui, &(t + 1).to_string(), changed);
                            let r = &mut p.predisposition_added_by_tier[t];
                            widgets::int(ui, &mut r[0], 0..=1_000);
                            widgets::int(ui, &mut r[1], 0..=1_000);
                            for &class in Class::all() {
                                let c = class as usize;
                                widgets::bp(ui, &mut p.class_increase_by_tier_bp[c][t]);
                                widgets::bp(ui, &mut p.support_increase_by_tier_bp[c][t]);
                            }
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
                            bp_row(
                                ui,
                                "Resistance level (bestiary resists)",
                                &mut m.resist_bp,
                                dm.resist_bp,
                            );
                            bp_row(ui, "Elite HP increase", &mut m.elite_hp_bp, dm.elite_hp_bp);
                            for (rank, name) in ["regular", "elite", "boss"].iter().enumerate() {
                                for (k, kind) in
                                    ["active", "reactive", "passive"].iter().enumerate()
                                {
                                    int_row(
                                        ui,
                                        &format!("Skill slots, {name}, {kind}"),
                                        &mut m.slots[rank][k],
                                        dm.slots[rank][k],
                                        0..=5,
                                    );
                                }
                            }
                            int_row(
                                ui,
                                "Monster AI: heal an ally below, %",
                                &mut m.ai_heal_below_pct,
                                dm.ai_heal_below_pct,
                                0..=100,
                            );
                        });
                });
        });
}
