use crate::engine::*;
use crate::theme::{self, Palette};
use crate::util::*;
use egui::{
    Align, Align2, Button, Color32, CornerRadius, FontId, Key, Label, Layout, Modifiers, Panel,
    ProgressBar, RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, vec2,
};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

type EngineOp = fn(&mut dyn DownloadEngine, DownloadId) -> EngineResult<()>;
const ROW_H: f32 = 78.0;
const LIMITS: [Option<u64>; 4] = [None, Some(1 << 20), Some(5 << 20), Some(10 << 20)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Filter {
    All,
    Downloading,
    Queued,
    Paused,
    Failed,
    Completed,
    Kind(FileKind),
}

impl Filter {
    const STATES: [Filter; 6] = [
        Self::All,
        Self::Downloading,
        Self::Queued,
        Self::Paused,
        Self::Failed,
        Self::Completed,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::All => "All downloads",
            Self::Downloading => "Downloading",
            Self::Queued => "Queued",
            Self::Paused => "Paused",
            Self::Failed => "Failed",
            Self::Completed => "Completed",
            Self::Kind(k) => k.label(),
        }
    }

    pub fn matches(self, d: &Download) -> bool {
        use DownloadState as S;
        match self {
            Self::All => true,
            Self::Downloading => d.state == S::Downloading,
            Self::Queued => d.state == S::Queued,
            Self::Paused => d.state == S::Paused,
            Self::Failed => matches!(d.state, S::Failed(_)),
            Self::Completed => d.state == S::Completed,
            Self::Kind(k) => FileKind::of(&d.file_name) == k,
        }
    }
}

/// Everything the UI can ask the engine to do; public so tests can drive the app without a window.
#[derive(Clone, Debug)]
pub enum Command {
    Pause(Vec<DownloadId>),
    Resume(Vec<DownloadId>),
    Remove(Vec<DownloadId>),
}

struct AddDialog {
    text: String,
    dir: String,
}

pub struct DownloadApp {
    engine: Box<dyn DownloadEngine>,
    pub snapshot: Vec<Download>,
    pub selected: BTreeSet<DownloadId>,
    pub filter: Filter,
    pub search: String,
    dark: bool,
    add: Option<AddDialog>,
    confirm_remove: Option<Vec<DownloadId>>,
    notice: Option<(String, bool, f64)>,
    time: f64,
    default_dir: PathBuf,
}

#[cfg(any(target_os = "linux", test))]
fn existing_save_dir(download_dir: Option<PathBuf>, home_dir: Option<PathBuf>) -> PathBuf {
    [download_dir, home_dir]
        .into_iter()
        .flatten()
        .find(|dir| dir.is_dir())
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod directory_tests {
    use super::existing_save_dir;
    use std::path::PathBuf;

    #[test]
    fn uses_existing_download_dir_then_home_then_current_dir() {
        let home = std::env::temp_dir();
        let missing = home.join(format!("missing-download-dir-{}", std::process::id()));
        assert_eq!(existing_save_dir(Some(home.clone()), None), home);
        assert_eq!(existing_save_dir(Some(missing), Some(home.clone())), home);
        assert_eq!(existing_save_dir(None, None), PathBuf::from("."));
    }
}

impl DownloadApp {
    pub fn default_save_dir() -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            existing_save_dir(dirs::download_dir(), dirs::home_dir())
        }
        #[cfg(not(target_os = "linux"))]
        {
            dirs::download_dir().or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from("."))
        }
    }

    pub fn new(ctx: &egui::Context, engine: Box<dyn DownloadEngine>, default_dir: PathBuf) -> Self {
        theme::apply(ctx, true);
        let snapshot = engine.downloads();
        Self {
            engine,
            snapshot,
            selected: BTreeSet::new(),
            filter: Filter::All,
            search: String::new(),
            dark: true,
            add: None,
            confirm_remove: None,
            notice: None,
            time: 0.0,
            default_dir,
        }
    }

    pub fn tick(&mut self, dt: Duration) {
        self.engine.tick(dt);
        self.refresh();
    }

    fn refresh(&mut self) {
        self.snapshot = self.engine.downloads();
        let ids: BTreeSet<_> = self.snapshot.iter().map(|d| d.id).collect();
        self.selected.retain(|i| ids.contains(i));
    }

    fn notify(&mut self, msg: impl Into<String>, is_err: bool) {
        self.notice = Some((msg.into(), is_err, self.time + 4.0));
    }

    /// Adds one download per non-empty line. All-or-nothing: nothing is added if any line is invalid.
    pub fn add_links(&mut self, text: &str, dir: &str) -> Result<usize, String> {
        let urls: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if urls.is_empty() {
            return Err("Paste at least one link".into());
        }
        if let Some(bad) = urls.iter().find(|u| validate_url(u).is_err()) {
            return Err(format!("Not a valid link: {bad}"));
        }
        let save_dir = if dir.trim().is_empty() || dir == self.default_dir.to_string_lossy() {
            self.default_dir.clone()
        } else {
            PathBuf::from(dir.trim())
        };
        for u in &urls {
            if let Err(e) = self.engine.add(AddRequest {
                url: (*u).into(),
                file_name: None,
                save_dir: save_dir.clone(),
            }) {
                self.refresh();
                return Err(e.to_string());
            }
        }
        self.refresh();
        Ok(urls.len())
    }

    pub fn apply(&mut self, cmd: Command) {
        let (verb, ids, f): (_, _, EngineOp) = match &cmd {
            Command::Pause(i) => ("Paused", i, |e, id| e.pause(id)),
            Command::Resume(i) => ("Resumed", i, |e, id| e.resume(id)),
            Command::Remove(i) => ("Removed", i, |e, id| e.remove(id)),
        };
        let mut errors = vec![];
        for &id in ids {
            if let Err(e) = f(self.engine.as_mut(), id) {
                errors.push(e.to_string());
            }
        }
        let ok = ids.len() - errors.len();
        match errors.first() {
            Some(e) => self.notify(e.clone(), true),
            None => self.notify(
                format!("{verb} {ok} download{}", if ok == 1 { "" } else { "s" }),
                false,
            ),
        }
        self.refresh();
    }

    fn ids_where(
        &self,
        scope: Option<&BTreeSet<DownloadId>>,
        pred: impl Fn(&DownloadState) -> bool,
    ) -> Vec<DownloadId> {
        self.snapshot
            .iter()
            .filter(|d| scope.is_none_or(|s| s.contains(&d.id)) && pred(&d.state))
            .map(|d| d.id)
            .collect()
    }

    fn visible(&self) -> Vec<&Download> {
        let q = self.search.trim().to_lowercase();
        let mut v: Vec<_> = self
            .snapshot
            .iter()
            .filter(|d| self.filter.matches(d))
            .filter(|d| {
                q.is_empty()
                    || d.file_name.to_lowercase().contains(&q)
                    || d.url.to_lowercase().contains(&q)
            })
            .collect();
        v.reverse(); // newest first
        v
    }

    fn count(&self, f: Filter) -> usize {
        self.snapshot.iter().filter(|d| f.matches(d)).count()
    }

    pub fn show(&mut self, ui: &mut Ui) {
        self.time = ui.input(|i| i.time);
        if self.notice.as_ref().is_some_and(|n| n.2 < self.time) {
            self.notice = None;
        }
        if self.snapshot.iter().any(|d| d.state.is_active()) || self.notice.is_some() {
            ui.request_repaint_after(Duration::from_millis(100));
        }
        let p = Palette::of(self.dark);
        self.shortcuts(ui);
        self.toolbar(ui, &p);
        self.banner(ui, &p);
        self.status_bar(ui, &p);
        self.sidebar(ui, &p);
        self.details(ui, &p);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.bg).inner_margin(16.0))
            .show(ui, |ui| {
                self.list(ui, &p);
            });
        self.add_dialog(ui.ctx(), &p);
        self.confirm_dialog(ui.ctx());
    }

    fn shortcuts(&mut self, ui: &Ui) {
        let ctx = ui.ctx();
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::N)) && self.add.is_none() {
            self.open_add();
        }
        if ctx.egui_wants_keyboard_input() || self.add.is_some() || self.confirm_remove.is_some() {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::A)) {
            self.selected = self.visible().iter().map(|d| d.id).collect();
        }
        if ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace))
            && !self.selected.is_empty()
        {
            self.confirm_remove = Some(self.selected.iter().copied().collect());
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.selected.clear();
        }
    }

    fn open_add(&mut self) {
        self.add = Some(AddDialog {
            text: String::new(),
            dir: self.default_dir.display().to_string(),
        });
    }

    fn toolbar(&mut self, ui: &mut Ui, p: &Palette) {
        let can_pause = self.engine.supports_pause()
            && !self
                .ids_where(Some(&self.selected), DownloadState::is_active)
                .is_empty();
        let can_resume = !self
            .ids_where(Some(&self.selected), |s| {
                matches!(s, DownloadState::Paused | DownloadState::Failed(_))
            })
            .is_empty();
        let has_sel = !self.selected.is_empty();
        let mut cmd = None;
        Panel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(p.sidebar)
                    .inner_margin(egui::Margin::symmetric(16, 10)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⬇ Download Manager").strong().size(17.0));
                    ui.add_space(16.0);
                    if ui
                        .add(
                            Button::new(
                                RichText::new("➕  Add URL").strong().color(Color32::WHITE),
                            )
                            .fill(p.accent),
                        )
                        .on_hover_text("Add links (Ctrl/Cmd+N)")
                        .clicked()
                    {
                        self.open_add();
                    }
                    let sel_ids =
                        |s: &Self, f: fn(&DownloadState) -> bool| s.ids_where(Some(&s.selected), f);
                    if ui
                        .add_enabled(can_resume, Button::new("▶ Resume"))
                        .clicked()
                    {
                        cmd = Some(Command::Resume(sel_ids(self, |s| {
                            matches!(s, DownloadState::Paused | DownloadState::Failed(_))
                        })));
                    }
                    if ui.add_enabled(can_pause, Button::new("⏸ Pause")).clicked() {
                        cmd = Some(Command::Pause(sel_ids(self, DownloadState::is_active)));
                    }
                    if ui
                        .add_enabled(has_sel, Button::new("🗑 Remove"))
                        .on_hover_text("Remove from list (Delete)")
                        .clicked()
                    {
                        self.confirm_remove = Some(self.selected.iter().copied().collect());
                    }
                    ui.separator();
                    if ui
                        .add_enabled(self.engine.supports_pause(), Button::new("Pause all"))
                        .clicked()
                    {
                        cmd = Some(Command::Pause(
                            self.ids_where(None, DownloadState::is_active),
                        ));
                    }
                    if ui.button("Resume all").clicked() {
                        cmd = Some(Command::Resume(self.ids_where(None, |s| {
                            matches!(s, DownloadState::Paused | DownloadState::Failed(_))
                        })));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .button(if self.dark { "☀" } else { "🌙" })
                            .on_hover_text("Toggle theme")
                            .clicked()
                        {
                            self.dark = !self.dark;
                            theme::apply(ui.ctx(), self.dark);
                        }
                        ui.add(
                            TextEdit::singleline(&mut self.search)
                                .hint_text("🔍 Search downloads")
                                .desired_width(220.0),
                        );
                    });
                });
            });
        if let Some(c) = cmd {
            self.apply(c);
        }
    }

    fn banner(&self, ui: &mut Ui, p: &Palette) {
        let info = self.engine.info();
        if info.is_real {
            return;
        }
        Panel::top("banner").frame(egui::Frame::new().fill(p.warn_bg).inner_margin(egui::Margin::symmetric(16, 6))).show(ui, |ui| {
            ui.label(RichText::new(format!(
                "⚠ {} — demo data only. No network requests are made and no files are written. Connect a real engine via the DownloadEngine trait.",
                info.name
            )).color(p.warn_fg).small());
        });
    }

    fn status_bar(&mut self, ui: &mut Ui, p: &Palette) {
        let total_speed: u64 = self.snapshot.iter().map(|d| d.speed_bps).sum();
        let summary = format!(
            "{} downloading · {} queued · {} completed",
            self.count(Filter::Downloading),
            self.count(Filter::Queued),
            self.count(Filter::Completed)
        );
        let limit = self.engine.speed_limit();
        let mut new_limit = None;
        Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(p.sidebar)
                    .inner_margin(egui::Margin::symmetric(16, 6)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(summary).color(p.muted).small());
                    if let Some((msg, is_err, _)) = &self.notice {
                        ui.add_space(12.0);
                        let c = if *is_err {
                            p.state_color(&DownloadState::Failed(String::new()))
                        } else {
                            p.state_color(&DownloadState::Completed)
                        };
                        ui.label(RichText::new(msg).color(c).small().strong());
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(self.engine.info().name)
                                .color(p.muted)
                                .small(),
                        );
                        ui.separator();
                        ui.add_enabled_ui(self.engine.supports_speed_limit(), |ui| {
                            egui::ComboBox::from_id_salt("limit")
                                .selected_text(limit_label(limit))
                                .show_ui(ui, |ui| {
                                    for l in LIMITS {
                                        if ui.selectable_label(limit == l, limit_label(l)).clicked()
                                        {
                                            new_limit = Some(l);
                                        }
                                    }
                                });
                            ui.label(RichText::new("Limit").color(p.muted).small());
                        });
                        ui.separator();
                        ui.label(
                            RichText::new(format!("↓ {}", format_speed(total_speed)))
                                .strong()
                                .small(),
                        );
                    });
                });
            });
        if let Some(l) = new_limit {
            self.engine.set_speed_limit(l);
        }
    }

    fn sidebar(&mut self, ui: &mut Ui, p: &Palette) {
        Panel::left("sidebar")
            .exact_size(210.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(p.sidebar).inner_margin(12.0))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new("STATUS").small().color(p.muted).strong());
                for f in Filter::STATES {
                    let dot = match f {
                        Filter::Downloading => Some(DownloadState::Downloading),
                        Filter::Queued => Some(DownloadState::Queued),
                        Filter::Paused => Some(DownloadState::Paused),
                        Filter::Failed => Some(DownloadState::Failed(String::new())),
                        Filter::Completed => Some(DownloadState::Completed),
                        _ => None,
                    };
                    if nav_item(
                        ui,
                        p,
                        f.label(),
                        self.count(f),
                        self.filter == f,
                        dot.map(|s| p.state_color(&s)),
                    )
                    .clicked()
                    {
                        self.filter = f;
                    }
                }
                ui.add_space(14.0);
                ui.label(RichText::new("FILE TYPES").small().color(p.muted).strong());
                for k in FileKind::ALL {
                    let f = Filter::Kind(k);
                    if nav_item(
                        ui,
                        p,
                        k.label(),
                        self.count(f),
                        self.filter == f,
                        Some(theme::kind_color(k)),
                    )
                    .clicked()
                    {
                        self.filter = f;
                    }
                }
            });
    }

    fn details(&mut self, ui: &mut Ui, p: &Palette) {
        let [id] = self.selected.iter().copied().collect::<Vec<_>>()[..] else {
            return;
        };
        let Some(d) = self.snapshot.iter().find(|d| d.id == id).cloned() else {
            return;
        };
        Panel::right("details")
            .default_size(290.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(p.sidebar).inner_margin(16.0))
            .show(ui, |ui| {
                ui.label(RichText::new(&d.file_name).strong().size(16.0));
                ui.label(
                    RichText::new(d.state.label())
                        .color(p.state_color(&d.state))
                        .strong(),
                );
                if let DownloadState::Failed(why) = &d.state {
                    ui.label(RichText::new(why).color(p.muted).small());
                }
                ui.add_space(8.0);
                egui::Grid::new("details_grid")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        let mut row = |k: &str, v: String| {
                            ui.label(RichText::new(k).color(p.muted));
                            ui.add(Label::new(v).wrap());
                            ui.end_row();
                        };
                        row("Source", host_of(&d.url).into());
                        row("Save to", d.save_dir.display().to_string());
                        row("Size", d.total_bytes.map_or("Unknown".into(), format_bytes));
                        row(
                            "Done",
                            format!(
                                "{}{}",
                                format_bytes(d.downloaded_bytes),
                                d.progress()
                                    .map_or(String::new(), |p| format!(" ({:.0}%)", p * 100.0))
                            ),
                        );
                        row("Speed", format_speed(d.speed_bps));
                        row("Time left", d.eta().map_or("—".into(), format_eta));
                    });
                ui.add_space(8.0);
                ui.label(RichText::new("URL").color(p.muted));
                ui.add(Label::new(RichText::new(&d.url).small()).wrap());
                if ui.button("Copy URL").clicked() {
                    ui.ctx().copy_text(d.url.clone());
                    self.notify("URL copied", false);
                }
            });
    }

    fn list(&mut self, ui: &mut Ui, p: &Palette) {
        let rows: Vec<Download> = self.visible().into_iter().cloned().collect();
        if rows.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                let empty = self.snapshot.is_empty();
                ui.label(
                    RichText::new(if empty {
                        "No downloads yet"
                    } else {
                        "Nothing matches this view"
                    })
                    .size(20.0)
                    .strong(),
                );
                ui.label(
                    RichText::new(if empty {
                        "Paste a link to get started."
                    } else {
                        "Try another category or clear the search."
                    })
                    .color(p.muted),
                );
                if empty && ui.button("➕ Add URL").clicked() {
                    self.open_add();
                }
            });
            return;
        }
        let mut cmd = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for d in &rows {
                    let selected = self.selected.contains(&d.id);
                    let (resp, c) = download_row(ui, p, d, selected, self.engine.supports_pause());
                    if c.is_some() {
                        cmd = c;
                    }
                    if resp.clicked() || resp.secondary_clicked() {
                        let multi = ui.input(|i| i.modifiers.command || i.modifiers.shift);
                        if multi && resp.clicked() {
                            if !self.selected.remove(&d.id) {
                                self.selected.insert(d.id);
                            }
                        } else if !selected {
                            self.selected = BTreeSet::from([d.id]);
                        }
                    }
                    resp.context_menu(|ui| {
                        let paused =
                            matches!(d.state, DownloadState::Paused | DownloadState::Failed(_));
                        if ui.add_enabled(paused, Button::new("▶ Resume")).clicked() {
                            cmd = Some(Command::Resume(vec![d.id]));
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                self.engine.supports_pause() && d.state.is_active(),
                                Button::new("⏸ Pause"),
                            )
                            .clicked()
                        {
                            cmd = Some(Command::Pause(vec![d.id]));
                            ui.close();
                        }
                        if ui.button("Copy URL").clicked() {
                            ui.ctx().copy_text(d.url.clone());
                            ui.close();
                        }
                        if ui.button("🗑 Remove from list").clicked() {
                            self.confirm_remove = Some(vec![d.id]);
                            ui.close();
                        }
                    });
                }
            });
        match cmd {
            Some(Command::Remove(ids)) => self.confirm_remove = Some(ids),
            Some(c) => self.apply(c),
            None => {}
        }
    }

    fn add_dialog(&mut self, ctx: &egui::Context, p: &Palette) {
        let Some(dlg) = &mut self.add else { return };
        let (mut submit, mut cancel) = (false, false);
        let lines: Vec<String> = dlg
            .text
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        let all_valid = !lines.is_empty() && lines.iter().all(|l| validate_url(l).is_ok());
        egui::Window::new("Add downloads")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([520.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Links (one per line)");
                let r = ui.add(
                    TextEdit::multiline(&mut dlg.text)
                        .desired_rows(5)
                        .desired_width(f32::INFINITY)
                        .hint_text("https://example.com/file.zip"),
                );
                if !r.has_focus() && dlg.text.is_empty() {
                    r.request_focus();
                }
                egui::ScrollArea::vertical()
                    .max_height(110.0)
                    .show(ui, |ui| {
                        for l in &lines {
                            match validate_url(l) {
                                Ok(()) => ui.label(
                                    RichText::new(format!(
                                        "✔ {}  ·  {}",
                                        file_name_from_url(l),
                                        host_of(l)
                                    ))
                                    .small()
                                    .color(p.state_color(&DownloadState::Completed)),
                                ),
                                Err(e) => ui.label(
                                    RichText::new(format!("✖ {e}: {}", truncate(l, 40)))
                                        .small()
                                        .color(
                                            p.state_color(&DownloadState::Failed(String::new())),
                                        ),
                                ),
                            };
                        }
                    });
                ui.label("Save to");
                ui.add(TextEdit::singleline(&mut dlg.dir).desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    let label = format!(
                        "Add {} download{}",
                        lines.len(),
                        if lines.len() == 1 { "" } else { "s" }
                    );
                    submit = ui
                        .add_enabled(
                            all_valid,
                            Button::new(RichText::new(label).strong().color(Color32::WHITE))
                                .fill(p.accent),
                        )
                        .clicked()
                        || (all_valid
                            && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter)));
                    cancel = ui.button("Cancel").clicked()
                        || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
                    ui.label(
                        RichText::new("Ctrl/Cmd+Enter to add")
                            .small()
                            .color(p.muted),
                    );
                });
            });
        if submit {
            let (text, dir) = self.add.take().map(|d| (d.text, d.dir)).unwrap();
            match self.add_links(&text, &dir) {
                Ok(n) => self.notify(
                    format!("Added {n} download{}", if n == 1 { "" } else { "s" }),
                    false,
                ),
                Err(e) => self.notify(e, true),
            }
        } else if cancel {
            self.add = None;
        }
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(ids) = &self.confirm_remove else {
            return;
        };
        let n = ids.len();
        let (mut yes, mut no) = (false, false);
        egui::Window::new("Remove downloads?")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!(
                    "Remove {n} download{} from the list? Files already on disk are not deleted.",
                    if n == 1 { "" } else { "s" }
                ));
                ui.horizontal(|ui| {
                    yes = ui.button("Remove").clicked();
                    no = ui.button("Cancel").clicked()
                        || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
                });
            });
        if yes {
            let ids = self.confirm_remove.take().unwrap();
            self.apply(Command::Remove(ids));
        } else if no {
            self.confirm_remove = None;
        }
    }
}

impl eframe::App for DownloadApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let dt = ui.input(|i| i.stable_dt).min(0.25);
        self.tick(Duration::from_secs_f32(dt));
        self.show(ui);
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.into()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

fn limit_label(l: Option<u64>) -> String {
    l.map_or("Unlimited".into(), format_speed)
}

fn nav_item(
    ui: &mut Ui,
    p: &Palette,
    label: &str,
    count: usize,
    selected: bool,
    dot: Option<Color32>,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, label));
    let bg = if selected {
        p.accent.gamma_multiply(0.20)
    } else if resp.hovered() {
        p.card_hover
    } else {
        Color32::TRANSPARENT
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(8), bg);
    let mut x = rect.left() + 12.0;
    if let Some(c) = dot {
        painter.circle_filled(egui::pos2(x + 4.0, rect.center().y), 4.0, c);
        x += 16.0;
    }
    let text_c = if selected {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().text_color()
    };
    painter.text(
        egui::pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        text_c,
    );
    painter.text(
        egui::pos2(rect.right() - 12.0, rect.center().y),
        Align2::RIGHT_CENTER,
        count,
        FontId::proportional(12.0),
        p.muted,
    );
    resp
}

fn icon_btn(ui: &mut Ui, glyph: &str, tip: &str) -> bool {
    ui.add(
        Button::new(RichText::new(glyph).size(16.0))
            .min_size(vec2(34.0, 34.0))
            .frame(false),
    )
    .on_hover_text(tip)
    .clicked()
}

/// One card. Returns the row response (for selection/context menu) and any button command.
fn download_row(
    ui: &mut Ui,
    p: &Palette,
    d: &Download,
    selected: bool,
    can_pause: bool,
) -> (egui::Response, Option<Command>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    // Registered before the buttons so they win hit-testing.
    let resp = ui.interact(rect, ui.scope_id().with(d.id), Sense::click());
    let fill = if selected {
        p.accent.gamma_multiply(0.14)
    } else if resp.hovered() {
        p.card_hover
    } else {
        p.card
    };
    let stroke = Stroke::new(1.0, if selected { p.accent } else { p.border });
    ui.painter().rect(
        rect,
        CornerRadius::same(10),
        fill,
        stroke,
        StrokeKind::Inside,
    );

    let kind = FileKind::of(&d.file_name);
    let state_c = p.state_color(&d.state);
    let mut cmd = None;
    let mut inner = ui.new_child(
        UiBuilder::new()
            .max_rect(rect.shrink2(vec2(14.0, 10.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    let ui = &mut inner;
    let (badge, _) = ui.allocate_exact_size(vec2(42.0, 42.0), Sense::hover());
    let kc = theme::kind_color(kind);
    ui.painter()
        .rect_filled(badge, CornerRadius::same(10), kc.gamma_multiply(0.18));
    ui.painter().text(
        badge.center(),
        Align2::CENTER_CENTER,
        theme::kind_badge(kind),
        FontId::proportional(11.0),
        kc,
    );
    ui.add_space(6.0);
    let mid = (rect.width() - 28.0 - 42.0 - 14.0 - 84.0).max(80.0);
    ui.vertical(|ui| {
        ui.set_width(mid);
        ui.spacing_mut().item_spacing.y = 5.0;
        ui.add(Label::new(RichText::new(&d.file_name).strong().size(15.0)).truncate());
        let bar = ProgressBar::new(d.progress().unwrap_or(0.0))
            .desired_width(mid)
            .desired_height(6.0)
            .fill(state_c)
            .corner_radius(3);
        ui.add(
            if d.progress().is_none() && d.state == DownloadState::Downloading {
                bar.animate(true)
            } else {
                bar
            },
        );
        let size = match d.total_bytes {
            Some(t) if d.state != DownloadState::Completed => format!(
                "{} of {}",
                format_bytes(d.downloaded_bytes),
                format_bytes(t)
            ),
            Some(t) => format!("{} · {}", format_bytes(t), host_of(&d.url)),
            None => format_bytes(d.downloaded_bytes),
        };
        let extra = match &d.state {
            DownloadState::Downloading => format!(
                " · {}{}",
                format_speed(d.speed_bps),
                d.eta()
                    .map_or(String::new(), |e| format!(" · {} left", format_eta(e)))
            ),
            DownloadState::Failed(why) => format!(" · {why}"),
            _ => String::new(),
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(
                RichText::new(d.state.label().to_uppercase())
                    .small()
                    .strong()
                    .color(state_c),
            );
            ui.add(
                Label::new(
                    RichText::new(format!("{size}{extra}"))
                        .small()
                        .color(p.muted),
                )
                .truncate(),
            );
        });
    });
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if icon_btn(ui, "🗑", "Remove from list") {
            cmd = Some(Command::Remove(vec![d.id])); // list() routes this through the confirm dialog
        }
        match d.state {
            DownloadState::Downloading | DownloadState::Queued => {
                if can_pause && icon_btn(ui, "⏸", "Pause") {
                    cmd = Some(Command::Pause(vec![d.id]));
                }
            }
            DownloadState::Paused => {
                if icon_btn(ui, "▶", "Resume") {
                    cmd = Some(Command::Resume(vec![d.id]));
                }
            }
            DownloadState::Failed(_) => {
                if icon_btn(ui, "⟳", "Retry") {
                    cmd = Some(Command::Resume(vec![d.id]));
                }
            }
            DownloadState::Completed => {}
        }
    });
    (resp, cmd)
}
