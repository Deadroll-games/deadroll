//! Skills: the in-game builder (`SkillBlocks` from the enums' `ALL`), checked by `skillcheck`
//! like the character program does, with the derived numbers of the current config.
use deadroll_sim::{named::Named, presets::SkillPreset};
use deadroll_skillcheck::{
    derive_with, fusion_id, Category, Effect, Purpose, Target, Trigger, AURA_RADIUS_MAX,
    SKILL_ASPECTS_MAX,
};
use deadroll_types::{Aspect, Class, Form, RangeTag, Source, TimeTag};
use eframe::egui::{self, Ui};

use crate::{app::BalanceApp, widgets};

fn fresh() -> SkillPreset {
    SkillPreset {
        name: "New skill".into(),
        category: Category::Active,
        purpose: Purpose::Damage,
        source: Source::Combat,
        form: Form::Strike,
        target: Target::Single,
        aspects: vec![Aspect::Piercing],
        effect: Effect::None,
        trigger: Trigger::None,
        aura_radius: 0,
        range: RangeTag::Melee,
        timing: TimeTag::Instant,
        hp_below_pct: 0,
        description: String::new(),
    }
}

pub fn show(app: &mut BalanceApp, ui: &mut Ui) {
    egui::Panel::left("skill_list")
        .resizable(true)
        .default_size(300.0)
        .show(ui, |ui| {
            let action =
                widgets::preset_list(ui, &app.presets.skills, |s| &s.name, &mut app.selected[1]);
            widgets::apply_list_action(
                action,
                &mut app.presets.skills,
                &mut app.selected[1],
                fresh,
                |s| &mut s.name,
            );
        });
    let Some(id) = app.selected[1]
        .clone()
        .filter(|id| app.presets.skills.contains_key(id))
    else {
        ui.label("Pick a skill on the left or press New.");
        return;
    };
    let used_by: Vec<String> = app
        .presets
        .heroes
        .iter()
        .filter(|(_, h)| h.skills.contains(&id))
        .map(|(hid, _)| hid.clone())
        .collect();
    let tuning = app.cfg.skills;
    egui::ScrollArea::vertical()
        .id_salt("skill_edit")
        .show(ui, |ui| {
            ui.heading(format!("Skill `{id}`"));
            let Some(s) = app.presets.skills.get_mut(&id) else {
                return;
            };
            egui::Grid::new("skill_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut s.name);
                    ui.end_row();
                    ui.label("Category");
                    widgets::pick(ui, "category", &mut s.category);
                    ui.end_row();
                    ui.label("Purpose");
                    widgets::pick(ui, "purpose", &mut s.purpose);
                    ui.end_row();
                    ui.label("Power source");
                    widgets::pick(ui, "source", &mut s.source);
                    ui.end_row();
                    ui.label("Form");
                    widgets::pick(ui, "form", &mut s.form);
                    ui.end_row();
                    ui.label("Target");
                    widgets::pick(ui, "target", &mut s.target);
                    ui.end_row();
                    ui.label("Aspects (main first)");
                    ui.horizontal(|ui| {
                        let mut remove = None;
                        for (i, a) in s.aspects.iter_mut().enumerate() {
                            widgets::pick(ui, ("aspect", i), a);
                            if ui.small_button("✖").clicked() {
                                remove = Some(i);
                            }
                        }
                        if let Some(i) = remove {
                            s.aspects.remove(i);
                        }
                        // One past the limit is allowed so the builder can show why it fails.
                        if s.aspects.len() <= SKILL_ASPECTS_MAX && ui.small_button("+").clicked() {
                            let next = Aspect::all()
                                .iter()
                                .copied()
                                .find(|a| !s.aspects.contains(a))
                                .unwrap_or_default();
                            s.aspects.push(next);
                        }
                    });
                    ui.end_row();
                    ui.label("Effect");
                    widgets::pick(ui, "effect", &mut s.effect);
                    ui.end_row();
                    ui.label("Trigger (reactive)");
                    widgets::pick(ui, "trigger", &mut s.trigger);
                    ui.end_row();
                    ui.label("Aura radius (0 = none)");
                    widgets::int(ui, &mut s.aura_radius, 0..=AURA_RADIUS_MAX + 1);
                    ui.end_row();
                    ui.label("Range");
                    widgets::pick(ui, "range", &mut s.range);
                    ui.end_row();
                    ui.label("Timing (a tag only)");
                    widgets::pick(ui, "timing", &mut s.timing);
                    ui.end_row();
                    ui.label("HP threshold, percent (HpBelow, AllyHpBelow)");
                    widgets::int(ui, &mut s.hp_below_pct, 0..=100);
                    ui.end_row();
                    ui.label("Description");
                    ui.text_edit_multiline(&mut s.description);
                    ui.end_row();
                });
            if s.description.len() > 200 {
                ui.colored_label(
                    widgets::WARNING,
                    "⚠ a description has at most 200 characters",
                );
            }

            ui.add_space(8.0);
            ui.strong("Builder check (skillcheck)");
            match s.check() {
                Ok((blocks, _)) => {
                    ui.colored_label(widgets::OK, "valid skill");
                    let d = derive_with(&blocks, &tuning);
                    let shares: Vec<String> = s
                        .aspects
                        .iter()
                        .zip(d.shares_bp)
                        .map(|(a, bp)| format!("{} {:.0}%", a.name(), bp as f64 / 100.0))
                        .collect();
                    ui.label(format!(
                        "power {:.1}%, cooldown {} turns, {} AP, shares {}",
                        d.power_bp as f64 / 100.0,
                        d.cooldown,
                        d.ap_cost,
                        shares.join(" / ")
                    ));
                    if s.aspects.len() == 2 {
                        ui.label(format!(
                            "fusion id {}",
                            fusion_id(s.aspects[0], s.aspects[1])
                        ));
                    }
                    match blocks.category {
                        Category::Active => {}
                        Category::Reactive => {
                            ui.label(
                                "reactive: triggers by itself in a fight (no AP, own cooldown)",
                            );
                        }
                        Category::Passive => {
                            ui.label(
                            "passive: works all the time in a fight (an aura if it has a radius)",
                        );
                        }
                    }
                }
                Err(e) => {
                    ui.colored_label(widgets::ERROR, format!("✖ {} ({e:?})", e.message()));
                }
            }
            ui.add_space(4.0);
            ui.strong("Starting skill per class");
            for &class in Class::all() {
                match s.check_for(class) {
                    Ok(_) => ui.colored_label(widgets::OK, format!("✔ {}", class.name())),
                    Err(e) => ui.colored_label(
                        widgets::ERROR,
                        format!("✖ {}: {} ({e:?})", class.name(), e.message()),
                    ),
                };
            }
            ui.add_space(4.0);
            ui.label(format!("Summary: {}", s.summary()));
            if used_by.is_empty() {
                ui.label("Used by no hero.");
            } else {
                ui.label(format!("Used by: {}", used_by.join(", ")));
            }
        });
}
