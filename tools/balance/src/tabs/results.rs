//! Results: win rate, rounds and hits to kill (p10 / p50 / p90), damage per turn by hero, crit and
//! effect shares, control uptime, histograms, problems found in the selection, random builds.
use deadroll_sim::{
    named::Named,
    presets::{HeroPreset, SkillPreset},
    BuildScore, Histogram, Percentiles,
};
use deadroll_types::{StateKind, PARTY_MAX};
use eframe::egui::{self, Ui};
use egui_plot::{Bar, BarChart, Plot};

use crate::{app::BalanceApp, widgets};

fn pct(v: f64) -> String {
    format!("{:.1}%", v * 100.0)
}

fn p3(p: &Percentiles) -> String {
    if p.samples == 0 {
        "-".into()
    } else {
        format!("{:.1} / {:.1} / {:.1}", p.p10, p.p50, p.p90)
    }
}

fn card(ui: &mut Ui, title: &str, value: String, hint: &str) {
    egui::Frame::group(ui.style())
        .show(ui, |ui| {
            ui.set_min_width(170.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(title).weak());
                ui.label(egui::RichText::new(value).heading().strong());
            });
        })
        .response
        .on_hover_text(hint);
}

fn histogram(ui: &mut Ui, id: &str, title: &str, h: &Histogram, x: &str) {
    ui.vertical(|ui| {
        ui.strong(title);
        let bars: Vec<Bar> = h
            .bins
            .iter()
            .map(|&(start, count)| {
                Bar::new(start + h.width / 2.0, count as f64).width(h.width * 0.9)
            })
            .collect();
        Plot::new(id)
            .height(190.0)
            .width(420.0)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .x_axis_label(x)
            .y_axis_label("fights")
            .show(ui, |plot| plot.bar_chart(BarChart::new(title, bars)));
    });
}

pub fn show(app: &mut BalanceApp, ui: &mut Ui) {
    egui::ScrollArea::vertical()
        .id_salt("results")
        .show(ui, |ui| {
            batch(app, ui);
            ui.add_space(12.0);
            ui.separator();
            builds(app, ui);
        });
}

fn batch(app: &mut BalanceApp, ui: &mut Ui) {
    let Some(b) = &app.batch else {
        ui.label("No results yet: pick a party and an encounter above and press Run.");
        return;
    };
    let r = &b.report;
    ui.heading(format!(
        "{}: {} fights in {:.1}s",
        b.label,
        r.fights,
        b.took.as_secs_f64()
    ));
    if !b.issues.is_empty() {
        egui::CollapsingHeader::new(format!(
            "Problems in the selection: {} errors, {} warnings",
            b.issues.errors.len(),
            b.issues.warnings.len()
        ))
        .default_open(!b.issues.errors.is_empty())
        .show(ui, |ui| {
            widgets::issues(ui, &b.issues.errors, &b.issues.warnings)
        });
    }
    ui.horizontal_wrapped(|ui| {
        card(ui, "Win rate", pct(r.win_rate), "victories / fights");
        card(
            ui,
            "Victory / defeat / draw",
            format!("{} / {} / {}", r.victories, r.defeats, r.draws),
            "draw = still going after the round cap",
        );
        card(
            ui,
            "Rounds to win p10/50/90",
            p3(&r.rounds_to_win),
            "rounds of the fights won",
        );
        card(
            ui,
            "Hits to kill p10/50/90",
            p3(&r.hits_to_kill),
            "attacks a monster took before it died",
        );
        card(
            ui,
            "Party damage / turn p10/50/90",
            p3(&r.party_dpt),
            "party damage per hero turn, per fight",
        );
        card(
            ui,
            "Damage from crits",
            pct(r.crit_share),
            "share of all damage that crits added",
        );
        card(
            ui,
            "Damage from effects",
            pct(r.effect_share),
            "share of all damage dealt over time (Burning, Bleeding)",
        );
        card(
            ui,
            "Control uptime",
            pct(r.control_uptime),
            "monster turns lost to Freeze / Stun",
        );
    });
    ui.add_space(8.0);
    ui.strong("States landed per fight");
    ui.horizontal_wrapped(|ui| {
        for kind in StateKind::ALL {
            card(
                ui,
                &kind.name(),
                format!("{:.2}", r.states_per_fight[*kind as usize]),
                "applications that landed, per fight",
            );
        }
    });
    ui.add_space(8.0);
    ui.strong("By hero");
    egui::Grid::new("slots").striped(true).show(ui, |ui| {
        for h in [
            "hero",
            "dmg/turn p10/50/90",
            "mean",
            "crit share",
            "effect share",
            "hit rate",
            "crit rate",
            "healed/turn",
            "effects/fight",
            "deaths",
        ] {
            ui.strong(h);
        }
        ui.end_row();
        for s in &r.slots {
            ui.label(&s.name);
            ui.label(p3(&s.dpt));
            ui.label(format!("{:.2}", s.dpt.mean));
            ui.label(pct(s.crit_share));
            ui.label(pct(s.dot_share));
            ui.label(pct(s.hit_rate));
            ui.label(pct(s.crit_rate));
            ui.label(format!("{:.2}", s.healed_per_turn));
            ui.label(format!("{:.2}", s.effects_per_fight));
            ui.label(pct(s.death_rate));
            ui.end_row();
        }
    });
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        histogram(ui, "h_rounds", "Rounds to win", &r.rounds_hist, "rounds");
        histogram(ui, "h_hits", "Hits to kill", &r.hits_hist, "attacks");
        histogram(
            ui,
            "h_party",
            "Party damage per turn",
            &r.party_dpt_hist,
            "damage / turn",
        );
        for (i, s) in r.slots.iter().enumerate() {
            histogram(
                ui,
                &format!("h_slot{i}"),
                &format!("{}: damage per turn", s.name),
                &s.dpt_hist,
                "damage / turn",
            );
        }
    });
}

fn builds(app: &mut BalanceApp, ui: &mut Ui) {
    ui.heading("Random builds");
    ui.label(
        "Random level-1 heroes, skills drawn from the builder and kept only when skillcheck accepts \
         them, fought in the party above; top 10 by damage per turn.",
    );
    ui.horizontal_wrapped(|ui| {
        ui.label("Builds:");
        widgets::int(ui, &mut app.random.count, 10..=1_000_000);
        ui.label("Fights each:");
        widgets::int(ui, &mut app.random.fights_each, 1..=10_000);
        ui.label("Class:");
        app.random_class_pick(ui);
        ui.label("Party slot:");
        let mut slot = app.random.slot + 1;
        widgets::int(ui, &mut slot, 1..=PARTY_MAX);
        app.random.slot = slot - 1;
        ui.label("Seed:");
        widgets::int(ui, &mut app.random.seed, 0..=u64::MAX);
        let ctx = ui.ctx().clone();
        if ui
            .add_enabled(!app.running(), egui::Button::new("▶ Run random builds"))
            .clicked()
        {
            app.start_builds(&ctx);
        }
    });
    let Some(b) = &app.builds else {
        return;
    };
    ui.label(format!("{} ({:.1}s)", b.label, b.took.as_secs_f64()));
    if !b.issues.errors.is_empty() {
        widgets::issues(ui, &b.issues.errors, &[]);
    }
    let mut save: Option<BuildScore> = None;
    egui::Grid::new("builds").striped(true).show(ui, |ui| {
        for h in ["#", "dmg/turn", "win rate", "hero", "skills", ""] {
            ui.strong(h);
        }
        ui.end_row();
        for (i, s) in b.top.iter().enumerate() {
            let h = &s.hero;
            ui.label((i + 1).to_string());
            ui.label(format!("{:.2}", s.dpt));
            ui.label(pct(s.win_rate));
            let stats: Vec<String> = h.stats.iter().map(|v| v.to_string()).collect();
            ui.label(format!(
                "{} {} ({}), stats {}",
                h.race.name(),
                h.class.name(),
                h.predisposition.name(),
                stats.join("/")
            ));
            ui.vertical(|ui| {
                for sk in &h.inline_skills {
                    ui.label(sk.summary());
                }
            });
            if ui.button("Save as preset").clicked() {
                save = Some(s.clone());
            }
            ui.end_row();
        }
    });
    if let Some(s) = save {
        let id = save_build(app, &s.hero);
        app.status = format!("random build saved as hero `{id}` (press Save presets to write it)");
    }
}

/// Adds a random build as a hero preset, its skills as skill presets.
fn save_build(app: &mut BalanceApp, hero: &HeroPreset) -> String {
    let mut skill_ids = Vec::new();
    for (i, s) in hero.inline_skills.iter().enumerate() {
        let name = format!("{} skill {}", hero.name, i + 1);
        let id = widgets::free_id(&app.presets.skills, &name);
        let preset = SkillPreset { name, ..s.clone() };
        app.presets.skills.insert(id.clone(), preset);
        skill_ids.push(id);
    }
    let preset = HeroPreset {
        skills: skill_ids,
        inline_skills: vec![],
        equipment: None,
        ..hero.clone()
    };
    let id = widgets::free_id(&app.presets.heroes, &preset.name);
    app.presets.heroes.insert(id.clone(), preset);
    app.selected[0] = Some(id.clone());
    id
}
