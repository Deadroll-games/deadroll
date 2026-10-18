//! App state, the run bar and the background jobs. The window never waits on a fight: batches run
//! on their own thread (rayon inside) and report through `Progress` and a channel.
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, mpsc, Arc},
    time::{Duration, Instant},
};

use deadroll_sim::{
    random_builds, resolve_fight, resolve_party, run_batch, BalanceConfig, BuildScore, Issues,
    Presets, Progress, RandomBuilds, Report, Settings,
};
use deadroll_types::{Class, PARTY_MAX};
use eframe::egui::{self, Ui};

use crate::{tabs, widgets};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Heroes,
    Skills,
    Monsters,
    Encounters,
    Items,
    Config,
    Results,
}

impl Tab {
    const ALL: [Tab; 7] = [
        Tab::Heroes,
        Tab::Skills,
        Tab::Monsters,
        Tab::Encounters,
        Tab::Items,
        Tab::Config,
        Tab::Results,
    ];

    fn title(self) -> &'static str {
        match self {
            Tab::Heroes => "Heroes",
            Tab::Skills => "Skills",
            Tab::Monsters => "Monsters",
            Tab::Encounters => "Encounters",
            Tab::Items => "Items",
            Tab::Config => "Config",
            Tab::Results => "Results",
        }
    }
}

enum JobResult {
    Batch(Option<Box<Report>>),
    Builds(Option<Vec<BuildScore>>),
}

struct Job {
    what: &'static str,
    progress: Arc<Progress>,
    rx: mpsc::Receiver<JobResult>,
    started: Instant,
}

/// The last batch and what went into it.
pub struct BatchResult {
    pub report: Report,
    pub issues: Issues,
    pub label: String,
    pub took: Duration,
}

/// The last random-builds run.
pub struct BuildsResult {
    pub top: Vec<BuildScore>,
    pub issues: Issues,
    pub label: String,
    pub took: Duration,
}

pub struct BalanceApp {
    pub root: PathBuf,
    pub presets: Presets,
    /// As on disk, to show unsaved changes.
    saved: Presets,
    pub load_errors: Vec<String>,
    pub cfg: BalanceConfig,
    pub tab: Tab,
    /// Hero ids, one per party slot.
    pub party: Vec<String>,
    pub encounter: String,
    pub fights: u32,
    pub seed: u64,
    pub settings: Settings,
    pub random: RandomBuilds,
    pub batch: Option<BatchResult>,
    pub builds: Option<BuildsResult>,
    pub selected: [Option<String>; 4],
    pub status: String,
    /// `assets/registry/bestiary.json` as the Monsters tab edits it (read on first use).
    pub bestiary: Option<deadroll_rules::registry::BestiaryJson>,
    job: Option<Job>,
    /// Issues and label of the running job, moved into its result.
    batch_meta: Option<(Issues, String)>,
}

impl BalanceApp {
    pub fn new(root: PathBuf) -> Self {
        let (presets, load_errors) = Presets::load(&root);
        let heroes: Vec<String> = presets.heroes.keys().take(2).cloned().collect();
        let encounter = presets
            .encounters
            .keys()
            .next()
            .cloned()
            .unwrap_or_default();
        let status = format!(
            "{} heroes, {} skills, {} monsters, {} encounters from {}",
            presets.heroes.len(),
            presets.skills.len(),
            presets.registry.templates.iter().flatten().count(),
            presets.encounters.len(),
            root.display()
        );
        BalanceApp {
            root,
            saved: presets.clone(),
            presets,
            load_errors,
            cfg: BalanceConfig::DEFAULT,
            tab: Tab::Heroes,
            party: heroes,
            encounter,
            fights: 10_000,
            seed: 1,
            settings: Settings::default(),
            random: RandomBuilds {
                count: 2_000,
                fights_each: 20,
                class: None,
                slot: 0,
                seed: 1,
            },
            batch: None,
            builds: None,
            selected: Default::default(),
            bestiary: None,
            status,
            job: None,
            batch_meta: None,
        }
    }

    pub fn running(&self) -> bool {
        self.job.is_some()
    }

    fn save_presets(&mut self) {
        match self.presets.save(&self.root) {
            Ok(()) => {
                self.saved = self.presets.clone();
                self.status = format!("presets saved to {}", self.root.display());
            }
            Err(e) => self.status = format!("save failed: {e}"),
        }
    }

    fn reload_presets(&mut self) {
        let (p, errors) = Presets::load(&self.root);
        self.saved = p.clone();
        self.presets = p;
        self.load_errors = errors;
        self.status = "presets reloaded from disk".into();
    }

    fn party_label(&self) -> String {
        format!("{} vs {}", self.party.join(" + "), self.encounter)
    }

    /// Resolves the selection; `None` when nothing can fight.
    fn prepare(&self) -> Option<(Vec<deadroll_sim::Fighter>, deadroll_sim::Fight, Issues)> {
        let (team, mut issues) = resolve_party(&self.presets, &self.party, &self.cfg);
        let (fight, fight_issues) = resolve_fight(&self.presets, &self.encounter, &self.cfg);
        issues.extend(fight_issues);
        match (team.is_empty(), fight) {
            (false, Some(f)) => Some((team, f, issues)),
            _ => None,
        }
    }

    pub fn start_batch(&mut self, ctx: &egui::Context) {
        let Some((team, fight, issues)) = self.prepare() else {
            self.status = "nothing to run: pick at least one hero and a valid encounter".into();
            return;
        };
        let (cfg, settings, fights, seed) = (self.cfg, self.settings, self.fights, self.seed);
        let label = self.party_label();
        self.spawn("fights", ctx, move |progress| {
            JobResult::Batch(
                run_batch(&cfg, &team, &fight, settings, fights, seed, progress).map(Box::new),
            )
        });
        self.batch_meta = Some((issues, label));
    }

    pub fn start_builds(&mut self, ctx: &egui::Context) {
        let Some((team, fight, issues)) = self.prepare() else {
            self.status = "nothing to run: pick at least one hero and a valid encounter".into();
            return;
        };
        let (cfg, settings, rb) = (self.cfg, self.settings, self.random);
        let presets = self.presets.clone();
        let label = format!(
            "{} random {} builds in slot {} of {}",
            rb.count,
            rb.class.map_or("any class".into(), |c| format!("{c:?}")),
            rb.slot + 1,
            self.party_label()
        );
        self.spawn("random builds", ctx, move |progress| {
            JobResult::Builds(random_builds(
                &cfg, &presets, &team, &fight, settings, rb, progress,
            ))
        });
        self.batch_meta = Some((issues, label));
    }

    fn spawn(
        &mut self,
        what: &'static str,
        ctx: &egui::Context,
        work: impl FnOnce(&Progress) -> JobResult + Send + 'static,
    ) {
        let progress = Arc::new(Progress::default());
        let (tx, rx) = mpsc::channel();
        let (p, ctx) = (progress.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(work(&p));
            ctx.request_repaint();
        });
        self.job = Some(Job {
            what,
            progress,
            rx,
            started: Instant::now(),
        });
        self.status = format!("running {what}...");
    }

    fn cancel(&mut self) {
        if let Some(job) = &self.job {
            job.progress.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.job else {
            return;
        };
        let Ok(result) = job.rx.try_recv() else {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        };
        let took = job.started.elapsed();
        let what = job.what;
        self.job = None;
        let (issues, label) = self.batch_meta.take().unwrap_or_default();
        let problems = issues.errors.len() + issues.warnings.len();
        match result {
            JobResult::Batch(Some(report)) => {
                self.status = format!(
                    "{} fights in {:.1}s, win rate {:.1}%{}",
                    report.fights,
                    took.as_secs_f64(),
                    report.win_rate * 100.0,
                    if problems > 0 {
                        format!(", {problems} problems (see Results)")
                    } else {
                        String::new()
                    }
                );
                self.batch = Some(BatchResult {
                    report: *report,
                    issues,
                    label,
                    took,
                });
                self.tab = Tab::Results;
            }
            JobResult::Builds(Some(top)) => {
                self.status = format!("random builds done in {:.1}s", took.as_secs_f64());
                self.builds = Some(BuildsResult {
                    top,
                    issues,
                    label,
                    took,
                });
                self.tab = Tab::Results;
            }
            JobResult::Batch(None) | JobResult::Builds(None) => {
                self.status = format!("{what} cancelled");
            }
        }
    }

    fn run_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Party:");
            let heroes: Vec<(String, String)> = self
                .presets
                .heroes
                .iter()
                .map(|(id, h)| (id.clone(), h.name.clone()))
                .collect();
            let mut remove = None;
            for (slot, id) in self.party.iter_mut().enumerate() {
                widgets::pick_id(ui, ("party", slot), id, &heroes);
                if ui
                    .small_button("✖")
                    .on_hover_text("remove from the party")
                    .clicked()
                {
                    remove = Some(slot);
                }
            }
            if let Some(slot) = remove {
                self.party.remove(slot);
            }
            if self.party.len() < PARTY_MAX && !heroes.is_empty() && ui.button("+ hero").clicked() {
                self.party.push(heroes[0].0.clone());
            }
            ui.separator();
            ui.label("Encounter:");
            let encounters: Vec<(String, String)> = self
                .presets
                .encounters
                .iter()
                .map(|(id, e)| (id.clone(), e.name.clone()))
                .collect();
            widgets::pick_id(ui, "encounter", &mut self.encounter, &encounters);
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Fights:");
            widgets::int(ui, &mut self.fights, 1..=2_000_000);
            ui.label("Seed:");
            widgets::int(ui, &mut self.seed, 0..=u64::MAX);
            ui.label("Max rounds:");
            widgets::int(ui, &mut self.settings.max_rounds, 1..=1_000);
            ui.label("Heal below %:");
            widgets::int(ui, &mut self.settings.heal_below_pct, 0..=100);
            ui.separator();
            let ctx = ui.ctx().clone();
            if self.running() {
                if ui.button("Cancel").clicked() {
                    self.cancel();
                }
                if let Some(job) = &self.job {
                    let f = job.progress.fraction();
                    let done = job.progress.done.load(Ordering::Relaxed);
                    let total = job.progress.total.load(Ordering::Relaxed);
                    ui.add(
                        egui::ProgressBar::new(f)
                            .desired_width(260.0)
                            .text(format!("{} {done}/{total}", job.what)),
                    );
                }
            } else if ui
                .button(egui::RichText::new("▶ Run").strong())
                .on_hover_text("run the fights in the background")
                .clicked()
            {
                self.start_batch(&ctx);
            }
            ui.separator();
            let dirty = self.presets != self.saved;
            let save = if dirty {
                "Save presets *"
            } else {
                "Save presets"
            };
            if ui
                .add_enabled(dirty, egui::Button::new(save))
                .on_hover_text(format!("write every preset to {}", self.root.display()))
                .clicked()
            {
                self.save_presets();
            }
            if ui
                .button("Reload")
                .on_hover_text("drop unsaved preset changes")
                .clicked()
            {
                self.reload_presets();
            }
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&self.status).weak());
        });
        for e in &self.load_errors {
            ui.colored_label(widgets::ERROR, format!("✖ {e}"));
        }
    }

    pub fn random_class_pick(&mut self, ui: &mut Ui) {
        egui::ComboBox::from_id_salt("random_class")
            .selected_text(
                self.random
                    .class
                    .map_or("any class".into(), |c| format!("{c:?}")),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.random.class, None, "any class");
                for &c in Class::ALL {
                    ui.selectable_value(&mut self.random.class, Some(c), format!("{c:?}"));
                }
            });
    }
}

impl eframe::App for BalanceApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        egui::Panel::top("run_bar").show(ui, |ui| {
            ui.add_space(4.0);
            self.run_bar(ui);
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                for tab in Tab::ALL {
                    ui.selectable_value(
                        &mut self.tab,
                        tab,
                        egui::RichText::new(tab.title()).heading(),
                    );
                }
            });
            ui.add_space(2.0);
        });
        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            Tab::Heroes => tabs::heroes::show(self, ui),
            Tab::Skills => tabs::skills::show(self, ui),
            Tab::Monsters => tabs::monsters::show(self, ui),
            Tab::Encounters => tabs::encounters::show(self, ui),
            Tab::Items => tabs::items::show(self, ui),
            Tab::Config => tabs::config::show(self, ui),
            Tab::Results => tabs::results::show(self, ui),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{kittest::Queryable, Harness};

    fn harness() -> Harness<'static, BalanceApp> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("presets");
        Harness::builder()
            .with_size(egui::vec2(1500.0, 1000.0))
            .build_eframe(move |_| BalanceApp::new(root))
    }

    /// Steps until the background job is done (the window keeps drawing meanwhile).
    fn wait(h: &mut Harness<'static, BalanceApp>) {
        for _ in 0..3_000 {
            h.step();
            if !h.state().running() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("job did not finish");
    }

    #[test]
    fn every_tab_draws_and_edits_its_presets() {
        let mut h = harness();
        assert!(
            h.state().load_errors.is_empty(),
            "{:?}",
            h.state().load_errors
        );
        h.run();
        for (tab, item) in [
            ("Heroes", "Human warrior  (human_warrior)"),
            ("Skills", "Ice Prison  (ice_prison)"),
            ("Monsters", "goblin chieftain"),
            ("Encounters", "Wolves (demo pack 1)  (wolves)"),
        ] {
            h.get_all_by_label(tab).next().unwrap().click();
            h.run();
            h.get_by_label(item).click();
            h.run();
        }
        // The builder shows the skillcheck verdicts of the selected skill.
        h.get_all_by_label("Skills").next().unwrap().click();
        h.run();
        h.get_by_label("valid skill");
        h.get_by_label("✖ Warrior: the class does not allow this power source (ClassSource)");
        // New + Copy add presets, Delete removes the selection; Save is enabled once dirty.
        let before = h.state().presets.skills.len();
        h.get_by_label("New").click();
        h.run();
        h.get_by_label("Copy").click();
        h.run();
        assert_eq!(h.state().presets.skills.len(), before + 2);
        assert!(h.state().presets.skills.contains_key("new_skill_copy"));
        h.get_by_label("Delete").click();
        h.run();
        assert_eq!(h.state().presets.skills.len(), before + 1);
        h.get_by_label("Save presets *");
        h.get_by_label("Config").click();
        h.run();
        h.get_by_label("Armor constant K");
        // Items: every base and class kit from the enums, edits reach the config.
        h.get_by_label("Items").click();
        h.run();
        for base in [
            "Sword",
            "Staff",
            "Wand",
            "LeatherArmor",
            "Robe",
            "LightLeatherArmor",
        ] {
            h.get_by_label(base);
        }
        h.get_by_label("all items are the program's defaults");
        h.state_mut().cfg.items.bases[0].armor = 3;
        h.run();
        h.get_by_label("Sword *");
        // Heroes wear their class kit by default; the panel shows what the items give.
        h.get_all_by_label("Heroes").next().unwrap().click();
        h.run();
        h.get_by_label("Class starter kit");
        h.get_by_label("Sword Piercing 4-8");
        h.get_by_label("+13 armor");
        h.get_by_label("Results").click();
        h.run();
    }

    #[test]
    fn run_and_random_builds_fill_the_results() {
        let mut h = harness();
        h.state_mut().fights = 300;
        h.run();
        h.get_by_label("▶ Run").click();
        h.step();
        assert!(h.state().running());
        wait(&mut h);
        h.run();
        let batch = h.state().batch.as_ref().expect("a report");
        assert_eq!(batch.report.fights, 300);
        assert_eq!(h.state().tab, Tab::Results);
        h.get_by_label("Win rate");
        h.get_by_label("Hits to kill");

        h.state_mut().random.count = 30;
        h.state_mut().random.fights_each = 3;
        h.run();
        h.get_by_label("▶ Run random builds").scroll_to_me();
        h.run();
        h.get_by_label("▶ Run random builds").click();
        h.step();
        wait(&mut h);
        h.run();
        let top = &h.state().builds.as_ref().expect("random builds").top;
        assert_eq!(top.len(), 10);
        // Saving a build adds a hero and its skills as presets.
        let heroes = h.state().presets.heroes.len();
        h.get_all_by_label("Save as preset")
            .next()
            .unwrap()
            .scroll_to_me();
        h.run();
        h.get_all_by_label("Save as preset").next().unwrap().click();
        h.run();
        assert_eq!(h.state().presets.heroes.len(), heroes + 1);
    }

    #[test]
    fn cancel_stops_a_run() {
        let mut h = harness();
        h.state_mut().fights = 500_000;
        h.run();
        h.get_by_label("▶ Run").click();
        h.step();
        h.step();
        h.get_by_label("Cancel").click();
        wait(&mut h);
        assert!(h.state().batch.is_none());
        assert!(
            h.state().status.contains("cancelled"),
            "{}",
            h.state().status
        );
    }
}
