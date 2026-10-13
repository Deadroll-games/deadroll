//! Items: every item base (`ItemTuning.bases`), the class starter kits and bare hands. Rows and
//! pickers list `ItemBase`, `EquipSlot`, `Aspect`, `Stat` and `Class` from their `ALL`. Item
//! presets are `ItemTuning` JSON files in `presets/items/`.
use deadroll_rules::{items::KIT_MAX, ItemBaseStats, ItemTuning};
use deadroll_sim::{config_io, named::Named};
use deadroll_types::{Class, ItemBase, Stat};
use eframe::egui::{self, Ui};

use crate::{app::BalanceApp, widgets};

/// Folder of item presets under the presets root.
pub const ITEMS_DIR: &str = "items";

/// A kit entry: one base or nothing.
fn pick_kit_item(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    v: &mut Option<ItemBase>,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(v.map_or("—".into(), |b| b.name()))
        .show_ui(ui, |ui| {
            ui.selectable_value(v, None, "—");
            for &b in ItemBase::all() {
                ui.selectable_value(v, Some(b), b.name());
            }
        });
}

fn base_row(ui: &mut Ui, base: ItemBase, b: &mut ItemBaseStats, d: &ItemBaseStats) {
    widgets::label(ui, &base.name(), b != d);
    widgets::pick(ui, ("item_slot", base as u8), &mut b.slot);
    widgets::int(ui, &mut b.level, 1..=99);
    widgets::pick(ui, ("item_aspect", base as u8), &mut b.aspect);
    widgets::int(ui, &mut b.damage[0], 0..=10_000);
    widgets::int(ui, &mut b.damage[1], 0..=10_000);
    widgets::int(ui, &mut b.armor, 0..=10_000);
    widgets::int(ui, &mut b.defense, 0..=1_000);
    widgets::int(ui, &mut b.attack, 0..=1_000);
    for &stat in Stat::all() {
        widgets::int(ui, &mut b.stats[stat as usize], 0..=999);
    }
    widgets::int(ui, &mut b.hp, 0..=100_000);
    ui.end_row();
}

fn save(app: &mut BalanceApp) {
    let dir = app.root.join(ITEMS_DIR);
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .set_directory(&dir)
        .set_file_name("items.json")
        .save_file()
    {
        app.status = match std::fs::write(&path, config_io::items_to_json(&app.cfg.items)) {
            Ok(()) => format!("items saved to {}", path.display()),
            Err(e) => format!("items save failed: {e}"),
        };
    }
}

fn load(app: &mut BalanceApp) {
    let dir = app.root.join(ITEMS_DIR);
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .set_directory(&dir)
        .pick_file()
    {
        app.status = match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| config_io::items_from_json(&t))
        {
            Ok(items) => {
                app.cfg.items = items;
                format!("items loaded from {}", path.display())
            }
            Err(e) => format!("items load failed: {e}"),
        };
    }
}

pub fn show(app: &mut BalanceApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if ui.button("Reset items").clicked() {
            app.cfg.items = ItemTuning::DEFAULT;
        }
        if ui.button("Save items JSON…").clicked() {
            save(app);
        }
        if ui.button("Load items JSON…").clicked() {
            load(app);
        }
        ui.label(if app.cfg.items != ItemTuning::DEFAULT {
            "* marks values that differ from the program's defaults"
        } else {
            "all items are the program's defaults"
        });
    });
    ui.separator();
    let d = ItemTuning::DEFAULT;
    let t = &mut app.cfg.items;
    egui::ScrollArea::both().id_salt("items").show(ui, |ui| {
        egui::CollapsingHeader::new("Item bases")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("A base with damage max > 0 is a weapon: in the main hand its damage replaces bare hands as the base of every attack and skill. Everything else adds flat.");
                egui::Grid::new("item_bases").striped(true).show(ui, |ui| {
                    for h in [
                        "", "slot", "level", "aspect", "dmg min", "dmg max", "armor", "defense",
                        "attack",
                    ] {
                        ui.strong(h);
                    }
                    for &stat in Stat::all() {
                        ui.strong(format!("+{}", stat.name()));
                    }
                    ui.strong("HP");
                    ui.end_row();
                    for &base in ItemBase::all() {
                        let i = base as usize;
                        base_row(ui, base, &mut t.bases[i], &d.bases[i]);
                    }
                });
            });
        egui::CollapsingHeader::new("Starter kits (soulbound, worn at creation)")
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("starter_kits").striped(true).show(ui, |ui| {
                    for &class in Class::all() {
                        let i = class as usize;
                        widgets::label(ui, &class.name(), t.starter_kits[i] != d.starter_kits[i]);
                        for k in 0..KIT_MAX {
                            pick_kit_item(ui, ("kit", i, k), &mut t.starter_kits[i][k]);
                        }
                        ui.end_row();
                    }
                });
            });
        egui::CollapsingHeader::new("Bare hands and level")
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("items_misc").num_columns(3).striped(true).show(ui, |ui| {
                    widgets::label(ui, "Unarmed damage (Blunt)", t.unarmed != d.unarmed);
                    widgets::int(ui, &mut t.unarmed[0], 0..=10_000);
                    widgets::int(ui, &mut t.unarmed[1], 0..=10_000);
                    ui.end_row();
                    widgets::label(ui, "Level gap (wear items up to level + gap)", t.level_gap != d.level_gap);
                    widgets::int(ui, &mut t.level_gap, 0..=99);
                    ui.end_row();
                });
            });
    });
}
