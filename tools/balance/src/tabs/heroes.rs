//! Heroes: race, class, level, stats, predisposition and skills (ids of skill presets), with the
//! numbers the rules derive and the same validation a Run applies.
use deadroll_rules::player::{
    player_ap, player_attack_bonus, player_crit_bp, player_defense, player_initiative,
};
use deadroll_sim::{
    named::Named,
    presets::HeroPreset,
    setup::{fighter, Issues},
    BalanceConfig,
};
use deadroll_types::{Aspect, Class, Race, Source, Stat, PARTY_MAX, STATS_COUNT};
use eframe::egui::{self, Ui};

use crate::{app::BalanceApp, widgets};

fn fresh(cfg: &BalanceConfig, skill: Option<String>) -> HeroPreset {
    let t = &cfg.players;
    // Creation points spread round-robin under the cap.
    let mut stats = [t.stat_base; STATS_COUNT];
    let mut left = t.stat_points_start;
    let mut i = 0;
    while left > 0 && stats.iter().any(|&s| s < t.stat_max_start) {
        if stats[i] < t.stat_max_start {
            stats[i] += 1;
            left -= 1;
        }
        i = (i + 1) % STATS_COUNT;
    }
    HeroPreset {
        name: "New hero".into(),
        race: Race::default(),
        class: Class::default(),
        level: 1,
        stats,
        predisposition: Aspect::Piercing,
        tier: t.start_tier,
        skills: skill.into_iter().collect(),
        inline_skills: vec![],
    }
}

pub fn show(app: &mut BalanceApp, ui: &mut Ui) {
    egui::Panel::left("hero_list")
        .resizable(true)
        .default_size(280.0)
        .show(ui, |ui| {
            let action =
                widgets::preset_list(ui, &app.presets.heroes, |h| &h.name, &mut app.selected[0]);
            let first_skill = app.presets.skills.keys().next().cloned();
            let cfg = app.cfg;
            widgets::apply_list_action(
                action,
                &mut app.presets.heroes,
                &mut app.selected[0],
                || fresh(&cfg, first_skill),
                |h| &mut h.name,
            );
        });
    let Some(id) = app.selected[0]
        .clone()
        .filter(|id| app.presets.heroes.contains_key(id))
    else {
        ui.label("Pick a hero on the left or press New.");
        return;
    };
    let skills: Vec<(String, String)> = app
        .presets
        .skills
        .iter()
        .map(|(i, s)| (i.clone(), s.name.clone()))
        .collect();
    let cfg = app.cfg;
    // Each skill checked for this hero's class, like the character program does at creation.
    let class = app.presets.heroes[&id].class;
    let verdicts: std::collections::BTreeMap<String, Result<(), String>> = app
        .presets
        .skills
        .iter()
        .map(|(sid, s)| {
            let v = s
                .check_for(class)
                .map(|_| ())
                .map_err(|e| format!("{} ({e:?})", e.message()));
            (sid.clone(), v)
        })
        .collect();
    egui::ScrollArea::vertical()
        .id_salt("hero_edit")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(format!("Hero `{id}`"));
                let in_party = app.party.contains(&id);
                if ui
                    .add_enabled(
                        !in_party && app.party.len() < PARTY_MAX,
                        egui::Button::new("Add to party"),
                    )
                    .clicked()
                {
                    app.party.push(id.clone());
                }
            });
            let Some(hero) = app.presets.heroes.get_mut(&id) else {
                return;
            };
            egui::Grid::new("hero_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut hero.name);
                    ui.end_row();
                    ui.label("Race");
                    widgets::pick(ui, "race", &mut hero.race);
                    ui.end_row();
                    ui.label("Class");
                    widgets::pick(ui, "class", &mut hero.class);
                    ui.end_row();
                    ui.label("Level");
                    widgets::int(ui, &mut hero.level, 1..=99);
                    ui.end_row();
                    for &stat in Stat::all() {
                        ui.label(stat.name());
                        widgets::int(ui, &mut hero.stats[stat as usize], 0..=999);
                        ui.end_row();
                    }
                    ui.label("Predisposition");
                    widgets::pick(ui, "predisposition", &mut hero.predisposition);
                    ui.end_row();
                    ui.label("Tier (1 best)");
                    widgets::int(ui, &mut hero.tier, 1..=14);
                    ui.end_row();
                });
            ui.add_space(6.0);
            ui.strong("Skills");
            let mut remove = None;
            let mut swap = None;
            let count = hero.skills.len();
            for (i, sid) in hero.skills.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{}.", i + 1));
                    widgets::pick_id(ui, ("hero_skill", i), sid, &skills);
                    match verdicts.get(sid.as_str()) {
                        Some(Ok(())) => {
                            ui.colored_label(widgets::OK, "✔");
                        }
                        Some(Err(why)) => {
                            ui.colored_label(widgets::ERROR, "✖")
                                .on_hover_text(why.as_str());
                        }
                        None => {
                            ui.colored_label(widgets::ERROR, "✖")
                                .on_hover_text("no such skill preset");
                        }
                    }
                    if ui
                        .add_enabled(i > 0, egui::Button::new("▲").small())
                        .clicked()
                    {
                        swap = Some(i - 1);
                    }
                    if ui
                        .add_enabled(i + 1 < count, egui::Button::new("▼").small())
                        .clicked()
                    {
                        swap = Some(i);
                    }
                    if ui.small_button("✖").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = swap {
                hero.skills.swap(i, i + 1);
            }
            if let Some(i) = remove {
                hero.skills.remove(i);
            }
            if !skills.is_empty() && ui.button("+ skill").clicked() {
                hero.skills.push(skills[0].0.clone());
            }

            // What a Run would do with this hero.
            let hero = hero.clone();
            let mut issues = Issues::default();
            let f = fighter(&app.presets, &id, &hero, &cfg, &mut issues);
            ui.add_space(8.0);
            ui.strong("In a fight");
            let t = &cfg.players;
            let h = &f.hero;
            let (defense, armor) = player_defense(h.class, &h.stats, t);
            egui::Grid::new("hero_numbers")
                .num_columns(2)
                .show(ui, |ui| {
                    let mut row = |k: &str, v: String| {
                        ui.label(k);
                        ui.label(v);
                        ui.end_row();
                    };
                    row("HP", f.hp_max.to_string());
                    row(
                        "Action points",
                        player_ap(h.race, h.class, h.stats[Stat::Dexterity as usize], t)
                            .to_string(),
                    );
                    row("Initiative", player_initiative(&h.stats, t).to_string());
                    row("Defense / armor", format!("{defense} / {armor}"));
                    row(
                        "Attack bonus (martial / magical)",
                        format!(
                            "{} / {}",
                            player_attack_bonus(h.level, &h.stats, Source::Combat, t),
                            player_attack_bonus(h.level, &h.stats, Source::Magical, t)
                        ),
                    );
                    row(
                        "Crit chance",
                        format!("{:.2}%", player_crit_bp(&h.stats, t) as f64 / 100.0),
                    );
                });
            ui.add_space(4.0);
            ui.label(format!("Actions: weapon + {} skills", f.skills.len()));
            for s in &f.skills {
                ui.label(format!(
                    "  • {}: power {:.0}%, cooldown {}",
                    s.name,
                    s.stats.power_bp as f64 / 100.0,
                    s.stats.cooldown
                ));
            }
            ui.add_space(4.0);
            widgets::issues(ui, &issues.errors, &issues.warnings);
        });
}
