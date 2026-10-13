//! Small editors shared by the tabs. Enum pickers list `ALL`, so new variants show up by themselves.
use std::{collections::BTreeMap, ops::RangeInclusive};

use deadroll_sim::{named::Named, presets::slug};
use eframe::egui::{self, emath::Numeric, Color32, DragValue, Ui};

pub const CHANGED: Color32 = Color32::from_rgb(230, 180, 60);
pub const ERROR: Color32 = Color32::from_rgb(235, 90, 80);
pub const WARNING: Color32 = Color32::from_rgb(220, 180, 70);
pub const OK: Color32 = Color32::from_rgb(110, 200, 120);

/// A row label, marked when the value differs from the default.
pub fn label(ui: &mut Ui, text: &str, changed: bool) {
    if changed {
        ui.colored_label(CHANGED, format!("{text} *"))
            .on_hover_text("differs from the default");
    } else {
        ui.label(text);
    }
}

/// Basis points shown as a percentage (10 000 bp = 100%).
pub fn bp(ui: &mut Ui, v: &mut u32) -> egui::Response {
    ui.add(
        DragValue::new(v)
            .speed(10.0)
            .range(0..=10_000_000u32)
            .custom_formatter(|n, _| format!("{:.2}%", n / 100.0))
            .custom_parser(|s| {
                s.trim()
                    .trim_end_matches('%')
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .map(|p| (p * 100.0).round())
            }),
    )
}

/// An integer.
pub fn int<N: Numeric>(ui: &mut Ui, v: &mut N, range: RangeInclusive<N>) -> egui::Response {
    ui.add(DragValue::new(v).range(range))
}

/// A picker over every variant of `E`.
pub fn pick<E: Named>(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, v: &mut E) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(v.name())
        .show_ui(ui, |ui| {
            for &e in E::all() {
                ui.selectable_value(v, e, e.name());
            }
        });
}

/// A picker over preset ids, showing `id: name`.
pub fn pick_id(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    value: &mut String,
    options: &[(String, String)],
) {
    let shown = options.iter().find(|(i, _)| i == value).map_or_else(
        || format!("{value} (missing)"),
        |(i, n)| format!("{i}: {n}"),
    );
    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .width(260.0)
        .show_ui(ui, |ui| {
            for (i, n) in options {
                ui.selectable_value(value, i.clone(), format!("{i}: {n}"));
            }
        });
}

/// Problems in red and yellow, or a green "ok".
pub fn issues(ui: &mut Ui, errors: &[String], warnings: &[String]) {
    if errors.is_empty() && warnings.is_empty() {
        ui.colored_label(OK, "valid");
    }
    for e in errors {
        ui.colored_label(ERROR, format!("✖ {e}"));
    }
    for w in warnings {
        ui.colored_label(WARNING, format!("⚠ {w}"));
    }
}

/// What the preset list asked for this frame.
pub enum ListAction {
    None,
    New,
    Copy(String),
    Delete(String),
}

/// The list of presets on the left of an editor tab: select, new, copy, delete.
pub fn preset_list<T>(
    ui: &mut Ui,
    items: &BTreeMap<String, T>,
    name: impl Fn(&T) -> &str,
    selected: &mut Option<String>,
) -> ListAction {
    let mut action = ListAction::None;
    ui.horizontal(|ui| {
        if ui.button("New").clicked() {
            action = ListAction::New;
        }
        let picked = selected.clone().filter(|s| items.contains_key(s));
        if ui
            .add_enabled(picked.is_some(), egui::Button::new("Copy"))
            .clicked()
        {
            action = ListAction::Copy(picked.clone().unwrap_or_default());
        }
        if ui
            .add_enabled(picked.is_some(), egui::Button::new("Delete"))
            .clicked()
        {
            action = ListAction::Delete(picked.unwrap_or_default());
        }
    });
    ui.separator();
    egui::ScrollArea::vertical()
        .id_salt("preset_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (id, item) in items {
                let on = selected.as_deref() == Some(id.as_str());
                if ui
                    .selectable_label(on, format!("{}  ({id})", name(item)))
                    .clicked()
                {
                    *selected = Some(id.clone());
                }
            }
        });
    action
}

/// A free id for `name` among `items`.
pub fn free_id<T>(items: &BTreeMap<String, T>, name: &str) -> String {
    let base = slug(name);
    if !items.contains_key(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}_{n}"))
        .find(|id| !items.contains_key(id))
        .unwrap_or(base)
}

/// Applies a list action: new items from `fresh`, copies named "<name> copy".
pub fn apply_list_action<T: Clone>(
    action: ListAction,
    items: &mut BTreeMap<String, T>,
    selected: &mut Option<String>,
    fresh: impl FnOnce() -> T,
    name: impl Fn(&mut T) -> &mut String,
) {
    match action {
        ListAction::None => {}
        ListAction::New => {
            let mut item = fresh();
            let id = free_id(items, name(&mut item));
            items.insert(id.clone(), item);
            *selected = Some(id);
        }
        ListAction::Copy(from) => {
            if let Some(mut item) = items.get(&from).cloned() {
                name(&mut item).push_str(" copy");
                let id = free_id(items, name(&mut item));
                items.insert(id.clone(), item);
                *selected = Some(id);
            }
        }
        ListAction::Delete(id) => {
            items.remove(&id);
            *selected = None;
        }
    }
}
