mod api;

use api::ApiClient;
use eframe::egui;
use media_shared::*;
use std::sync::{Arc, Mutex};

// ── Color palette ───────────────────────────────────────────────────────────

const BG_DARK: egui::Color32 = egui::Color32::from_rgb(18, 18, 24);
const BG_PANEL: egui::Color32 = egui::Color32::from_rgb(26, 27, 38);
const BG_CARD: egui::Color32 = egui::Color32::from_rgb(34, 35, 50);
const BG_ELEVATED: egui::Color32 = egui::Color32::from_rgb(42, 43, 62);
const BORDER_SUBTLE: egui::Color32 = egui::Color32::from_rgb(55, 56, 78);
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(228, 228, 240);
const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(148, 150, 175);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(100, 102, 130);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(99, 102, 241);
const ACCENT_HOVER: egui::Color32 = egui::Color32::from_rgb(129, 132, 255);
const SUCCESS: egui::Color32 = egui::Color32::from_rgb(52, 211, 153);
const WARNING: egui::Color32 = egui::Color32::from_rgb(251, 191, 36);
const ERROR: egui::Color32 = egui::Color32::from_rgb(248, 113, 113);
const INFO: egui::Color32 = egui::Color32::from_rgb(96, 165, 250);

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Media Manager",
        options,
        Box::new(|cc| Ok(Box::new(MediaApp::new(cc)))),
    )
}

// ── Shared state updated from background threads ────────────────────────────

#[derive(Default)]
struct AsyncState {
    files: Vec<MediaFile>,
    jobs: Vec<EncodeJob>,
    status_msg: String,
    loading: bool,
}

// ── Application ─────────────────────────────────────────────────────────────

struct MediaApp {
    api: Arc<ApiClient>,
    state: Arc<Mutex<AsyncState>>,
    runtime: tokio::runtime::Runtime,

    // UI state
    server_url: String,
    scan_path: String,
    selected_file: Option<String>,
    selected_tab: Tab,

    // Encode form
    enc_video_codec: usize,
    enc_audio_codec: usize,
    enc_resolution: usize,
    enc_crf: u8,

    // Folder browser
    show_browser: bool,
    browser_path: String,
    browser_entries: Vec<DirEntry>,
    browser_loading: bool,
    browser_error: String,
    browser_result: Option<Arc<Mutex<Option<Result<Vec<DirEntry>, String>>>>>,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Tab {
    Library,
    Encode,
    Jobs,
}

impl MediaApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::setup_custom_style(&cc.egui_ctx);

        let server_url = "http://10.42.1.1:9090".to_string();
        let api = Arc::new(ApiClient::new(&server_url));
        Self {
            api,
            state: Arc::new(Mutex::new(AsyncState::default())),
            runtime: tokio::runtime::Runtime::new().expect("tokio runtime"),
            server_url,
            scan_path: String::new(),
            selected_file: None,
            selected_tab: Tab::Library,
            enc_video_codec: 0,
            enc_audio_codec: 0,
            enc_resolution: 4, // Original
            enc_crf: 23,

            show_browser: false,
            browser_path: "/".to_string(),
            browser_entries: Vec::new(),
            browser_loading: false,
            browser_error: String::new(),
            browser_result: None,
        }
    }

    fn setup_custom_style(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();

        // Spacing
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(16.0, 6.0);
        style.spacing.window_margin = egui::Margin::same(16.0);
        style.spacing.combo_width = 220.0;

        // Rounding
        let rounding = egui::Rounding::same(8.0);
        style.visuals.window_rounding = rounding;
        style.visuals.menu_rounding = rounding;

        // Dark base
        style.visuals.dark_mode = true;
        style.visuals.panel_fill = BG_DARK;
        style.visuals.window_fill = BG_PANEL;
        style.visuals.extreme_bg_color = BG_CARD;
        style.visuals.faint_bg_color = BG_ELEVATED;

        // Widget visuals
        let widget_rounding = egui::Rounding::same(6.0);

        style.visuals.widgets.noninteractive.bg_fill = BG_CARD;
        style.visuals.widgets.noninteractive.fg_stroke =
            egui::Stroke::new(1.0, TEXT_SECONDARY);
        style.visuals.widgets.noninteractive.rounding = widget_rounding;
        style.visuals.widgets.noninteractive.bg_stroke =
            egui::Stroke::new(1.0, BORDER_SUBTLE);

        style.visuals.widgets.inactive.bg_fill = BG_ELEVATED;
        style.visuals.widgets.inactive.fg_stroke =
            egui::Stroke::new(1.0, TEXT_PRIMARY);
        style.visuals.widgets.inactive.rounding = widget_rounding;
        style.visuals.widgets.inactive.bg_stroke =
            egui::Stroke::new(1.0, BORDER_SUBTLE);

        style.visuals.widgets.hovered.bg_fill = ACCENT;
        style.visuals.widgets.hovered.fg_stroke =
            egui::Stroke::new(1.5, egui::Color32::WHITE);
        style.visuals.widgets.hovered.rounding = widget_rounding;
        style.visuals.widgets.hovered.bg_stroke =
            egui::Stroke::new(1.0, ACCENT_HOVER);

        style.visuals.widgets.active.bg_fill = ACCENT_HOVER;
        style.visuals.widgets.active.fg_stroke =
            egui::Stroke::new(2.0, egui::Color32::WHITE);
        style.visuals.widgets.active.rounding = widget_rounding;

        style.visuals.widgets.open.bg_fill = BG_ELEVATED;
        style.visuals.widgets.open.fg_stroke =
            egui::Stroke::new(1.0, TEXT_PRIMARY);
        style.visuals.widgets.open.rounding = widget_rounding;

        // Selection
        style.visuals.selection.bg_fill = ACCENT.linear_multiply(0.3);
        style.visuals.selection.stroke = egui::Stroke::new(1.5, ACCENT);

        // Striped table rows
        style.visuals.striped = true;

        // Text cursor
        style.visuals.text_cursor.stroke = egui::Stroke::new(2.0, ACCENT);

        // Separator
        style.visuals.widgets.noninteractive.bg_stroke =
            egui::Stroke::new(1.0, BORDER_SUBTLE);

        ctx.set_style(style);
    }

    fn refresh_files(&self, ctx: &egui::Context) {
        let api = Arc::clone(&self.api);
        let state = Arc::clone(&self.state);
        let ctx = ctx.clone();
        self.runtime.spawn(async move {
            state.lock().unwrap().loading = true;
            match api.list_files().await {
                Ok(files) => {
                    let mut s = state.lock().unwrap();
                    s.files = files;
                    s.status_msg = format!("Loaded {} files", s.files.len());
                    s.loading = false;
                }
                Err(e) => {
                    let mut s = state.lock().unwrap();
                    s.status_msg = format!("Error: {e}");
                    s.loading = false;
                }
            }
            ctx.request_repaint();
        });
    }

    fn refresh_jobs(&self, ctx: &egui::Context) {
        let api = Arc::clone(&self.api);
        let state = Arc::clone(&self.state);
        let ctx = ctx.clone();
        self.runtime.spawn(async move {
            match api.list_jobs().await {
                Ok(jobs) => {
                    let mut s = state.lock().unwrap();
                    s.jobs = jobs;
                }
                Err(e) => {
                    state.lock().unwrap().status_msg = format!("Error: {e}");
                }
            }
            ctx.request_repaint();
        });
    }

    fn browse_path(&mut self, ctx: &egui::Context) {
        let api = Arc::clone(&self.api);
        let path = self.browser_path.clone();
        let ctx = ctx.clone();
        self.browser_loading = true;
        self.browser_error.clear();

        // We store results directly via a shared Arc<Mutex<..>> for the browser
        let entries_out: Arc<Mutex<Option<Result<Vec<DirEntry>, String>>>> =
            Arc::new(Mutex::new(None));
        let entries_out2 = Arc::clone(&entries_out);

        self.runtime.spawn(async move {
            let result = api.browse_directory(&path).await;
            *entries_out2.lock().unwrap() = Some(result);
            ctx.request_repaint();
        });

        // We'll poll results in the UI via a stored handle
        self.browser_result = Some(entries_out);
    }

    fn poll_browser_result(&mut self) {
        if let Some(ref result_handle) = self.browser_result {
            let mut guard = result_handle.lock().unwrap();
            if let Some(result) = guard.take() {
                self.browser_loading = false;
                match result {
                    Ok(entries) => {
                        self.browser_entries = entries;
                        self.browser_error.clear();
                    }
                    Err(e) => {
                        self.browser_entries.clear();
                        self.browser_error = e;
                    }
                }
                drop(guard);
                self.browser_result = None;
                return;
            }
        }
    }

    fn scan_directory(&self, ctx: &egui::Context) {
        let api = Arc::clone(&self.api);
        let state = Arc::clone(&self.state);
        let dir = self.scan_path.clone();
        let ctx = ctx.clone();
        self.runtime.spawn(async move {
            state.lock().unwrap().loading = true;
            state.lock().unwrap().status_msg = format!("Scanning {dir}...");
            ctx.request_repaint();
            match api.scan_directory(&dir).await {
                Ok(files) => {
                    let mut s = state.lock().unwrap();
                    s.files = files;
                    s.status_msg = format!("Scan complete: {} files found", s.files.len());
                    s.loading = false;
                }
                Err(e) => {
                    let mut s = state.lock().unwrap();
                    s.status_msg = format!("Scan error: {e}");
                    s.loading = false;
                }
            }
            ctx.request_repaint();
        });
    }

    fn start_encode(&self, ctx: &egui::Context) {
        let file_id = match &self.selected_file {
            Some(id) => id.clone(),
            None => return,
        };

        let req = EncodeRequest {
            file_id,
            video_codec: VideoCodec::ALL[self.enc_video_codec],
            audio_codec: AudioCodec::ALL[self.enc_audio_codec],
            resolution: ResolutionProfile::ALL[self.enc_resolution],
            quality_crf: Some(self.enc_crf),
        };

        let api = Arc::clone(&self.api);
        let state = Arc::clone(&self.state);
        let ctx = ctx.clone();
        self.runtime.spawn(async move {
            match api.start_encode(&req).await {
                Ok(job) => {
                    state.lock().unwrap().status_msg =
                        format!("Encode job started: {}", job.id);
                }
                Err(e) => {
                    state.lock().unwrap().status_msg = format!("Encode error: {e}");
                }
            }
            ctx.request_repaint();
        });
    }
}

impl eframe::App for MediaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Auto-refresh jobs every 2 seconds when on Jobs tab
        if self.selected_tab == Tab::Jobs {
            ctx.request_repaint_after(std::time::Duration::from_secs(2));
            self.refresh_jobs(ctx);
        }

        // ── Top header bar ──────────────────────────────────────────────
        egui::TopBottomPanel::top("top_panel")
            .frame(
                egui::Frame::default()
                    .fill(BG_PANEL)
                    .inner_margin(egui::Margin::symmetric(20.0, 12.0))
                    .stroke(egui::Stroke::new(1.0, BORDER_SUBTLE)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // App title
                    ui.label(
                        egui::RichText::new("▶ Media Manager")
                            .size(18.0)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );

                    ui.add_space(24.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Server connection
                    ui.label(
                        egui::RichText::new("Server")
                            .size(12.0)
                            .color(TEXT_DIM),
                    );
                    let server_edit = egui::TextEdit::singleline(&mut self.server_url)
                        .desired_width(220.0)
                        .margin(egui::Margin::symmetric(8.0, 4.0));
                    let changed = ui.add(server_edit).changed();
                    if changed {
                        self.api = Arc::new(ApiClient::new(&self.server_url));
                    }

                    if styled_button(ui, "Connect", ACCENT, false).clicked() {
                        self.api = Arc::new(ApiClient::new(&self.server_url));
                        self.refresh_files(ctx);
                    }
                });
            });

        // ── Tab navigation bar ──────────────────────────────────────────
        egui::TopBottomPanel::top("tabs")
            .frame(
                egui::Frame::default()
                    .fill(BG_PANEL)
                    .inner_margin(egui::Margin::symmetric(20.0, 8.0))
                    .stroke(egui::Stroke::new(1.0, BORDER_SUBTLE)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    tab_button(ui, &mut self.selected_tab, Tab::Library, "📁  Library");
                    tab_button(ui, &mut self.selected_tab, Tab::Encode, "🎬  Encode");
                    tab_button(ui, &mut self.selected_tab, Tab::Jobs, "⏳  Jobs");
                });
            });

        // ── Bottom status bar ───────────────────────────────────────────
        egui::TopBottomPanel::bottom("status_bar")
            .frame(
                egui::Frame::default()
                    .fill(BG_PANEL)
                    .inner_margin(egui::Margin::symmetric(20.0, 8.0))
                    .stroke(egui::Stroke::new(1.0, BORDER_SUBTLE)),
            )
            .show(ctx, |ui| {
                let state = self.state.lock().unwrap();
                ui.horizontal(|ui| {
                    if state.loading {
                        ui.spinner();
                        ui.add_space(4.0);
                    }
                    ui.label(
                        egui::RichText::new(&state.status_msg)
                            .size(12.0)
                            .color(TEXT_SECONDARY),
                    );
                });
            });

        // ── Main content ────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(BG_DARK)
                    .inner_margin(egui::Margin::same(24.0)),
            )
            .show(ctx, |ui| match self.selected_tab {
                Tab::Library => self.show_library(ui, ctx),
                Tab::Encode => self.show_encode(ui, ctx),
                Tab::Jobs => self.show_jobs(ui),
            });

        // ── Folder browser window (rendered on top) ─────────────────────
        self.poll_browser_result();
        self.show_browser_window(ctx);
    }
}

// ── Reusable UI components ──────────────────────────────────────────────────

fn tab_button(ui: &mut egui::Ui, current: &mut Tab, target: Tab, label: &str) {
    let is_active = *current == target;
    let (bg, text_color) = if is_active {
        (ACCENT, egui::Color32::WHITE)
    } else {
        (egui::Color32::TRANSPARENT, TEXT_SECONDARY)
    };

    let btn = egui::Button::new(
        egui::RichText::new(label).color(text_color).size(13.0),
    )
    .fill(bg)
    .rounding(egui::Rounding::same(6.0))
    .stroke(if is_active {
        egui::Stroke::NONE
    } else {
        egui::Stroke::new(1.0, BORDER_SUBTLE)
    });

    if ui.add(btn).clicked() {
        *current = target;
    }
}

fn styled_button(
    ui: &mut egui::Ui,
    label: &str,
    color: egui::Color32,
    large: bool,
) -> egui::Response {
    let size = if large { 14.0 } else { 13.0 };
    let btn = egui::Button::new(
        egui::RichText::new(label)
            .color(egui::Color32::WHITE)
            .size(size),
    )
    .fill(color)
    .rounding(egui::Rounding::same(6.0));
    ui.add(btn)
}

fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(22.0)
            .strong()
            .color(TEXT_PRIMARY),
    );
    ui.add_space(4.0);
}

fn card_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(BG_CARD)
        .rounding(egui::Rounding::same(10.0))
        .stroke(egui::Stroke::new(1.0, BORDER_SUBTLE))
        .inner_margin(egui::Margin::same(16.0))
}

fn form_label(ui: &mut egui::Ui, text: &str) {
    ui.allocate_ui_with_layout(
        egui::vec2(110.0, 20.0),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(13.0)
                    .color(TEXT_SECONDARY),
            );
        },
    );
}

// ── Tab renderers ───────────────────────────────────────────────────────────

impl MediaApp {
    fn show_library(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        section_heading(ui, "Media Library");

        // Scan controls card
        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                form_label(ui, "Directory");
                let scan_edit = egui::TextEdit::singleline(&mut self.scan_path)
                    .desired_width(350.0)
                    .hint_text("Enter path to scan…")
                    .margin(egui::Margin::symmetric(8.0, 4.0));
                ui.add(scan_edit);

                if styled_button(ui, "🔍  Scan", ACCENT, false).clicked()
                    && !self.scan_path.is_empty()
                {
                    self.scan_directory(ctx);
                }
                if styled_button(ui, "�  Browse", BG_ELEVATED, false).clicked() {
                    self.show_browser = true;
                    if self.scan_path.is_empty() {
                        self.browser_path = "/".to_string();
                    } else {
                        self.browser_path = self.scan_path.clone();
                    }
                    self.browse_path(ctx);
                }
                if styled_button(ui, "�🔄  Refresh", BG_ELEVATED, false).clicked() {
                    self.refresh_files(ctx);
                }
            });
        });

        ui.add_space(12.0);

        let state = self.state.lock().unwrap();
        let files = state.files.clone();
        drop(state);

        if files.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("No files loaded")
                        .size(16.0)
                        .color(TEXT_DIM),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Scan a directory or click Refresh to get started")
                        .size(13.0)
                        .color(TEXT_DIM),
                );
            });
            return;
        }

        // File count badge
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{} files", files.len()))
                    .size(13.0)
                    .color(TEXT_SECONDARY),
            );
        });
        ui.add_space(4.0);

        // File table in a card
        card_frame().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("file_grid")
                    .striped(true)
                    .min_col_width(60.0)
                    .spacing(egui::vec2(16.0, 6.0))
                    .show(ui, |ui| {
                        // Header
                        for h in ["", "Filename", "Size", "Duration", "Video", "Audio", "Resolution"] {
                            ui.label(
                                egui::RichText::new(h)
                                    .size(11.0)
                                    .strong()
                                    .color(TEXT_DIM),
                            );
                        }
                        ui.end_row();

                        for file in &files {
                            let is_selected =
                                self.selected_file.as_deref() == Some(&file.id);
                            let dot_color = if is_selected { ACCENT } else { TEXT_DIM };
                            if ui
                                .add(egui::Label::new(
                                    egui::RichText::new("●").color(dot_color).size(14.0),
                                ).sense(egui::Sense::click()))
                                .clicked()
                            {
                                self.selected_file = Some(file.id.clone());
                                self.selected_tab = Tab::Encode;
                            }

                            ui.label(
                                egui::RichText::new(&file.filename)
                                    .color(TEXT_PRIMARY)
                                    .size(13.0),
                            );
                            ui.label(
                                egui::RichText::new(format_size(file.size_bytes))
                                    .color(TEXT_SECONDARY)
                                    .size(13.0),
                            );
                            ui.label(
                                egui::RichText::new(
                                    file.duration_secs
                                        .map(format_duration)
                                        .unwrap_or_else(|| "—".into()),
                                )
                                .color(TEXT_SECONDARY)
                                .size(13.0),
                            );
                            ui.label(
                                egui::RichText::new(
                                    file.video_codec.as_deref().unwrap_or("—"),
                                )
                                .color(TEXT_SECONDARY)
                                .size(13.0),
                            );
                            ui.label(
                                egui::RichText::new(
                                    file.audio_codec.as_deref().unwrap_or("—"),
                                )
                                .color(TEXT_SECONDARY)
                                .size(13.0),
                            );
                            ui.label(
                                egui::RichText::new(match (file.width, file.height) {
                                    (Some(w), Some(h)) => format!("{w}×{h}"),
                                    _ => "—".into(),
                                })
                                .color(TEXT_SECONDARY)
                                .size(13.0),
                            );
                            ui.end_row();
                        }
                    });
            });
        });
    }

    fn show_encode(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        section_heading(ui, "Encode Settings");

        let state = self.state.lock().unwrap();
        let files = state.files.clone();
        drop(state);

        // Source file card
        card_frame().show(ui, |ui| {
            ui.label(
                egui::RichText::new("SOURCE FILE")
                    .size(11.0)
                    .strong()
                    .color(TEXT_DIM),
            );
            ui.add_space(8.0);

            let selected_name = self
                .selected_file
                .as_ref()
                .and_then(|id| files.iter().find(|f| &f.id == id))
                .map(|f| f.filename.clone())
                .unwrap_or_else(|| "None — select from Library".into());

            ui.horizontal(|ui| {
                form_label(ui, "File");
                egui::ComboBox::from_id_source("file_select")
                    .selected_text(&selected_name)
                    .width(350.0)
                    .show_ui(ui, |ui: &mut egui::Ui| {
                        for file in &files {
                            let label = format!(
                                "{} ({})",
                                file.filename,
                                format_size(file.size_bytes)
                            );
                            ui.selectable_value(
                                &mut self.selected_file,
                                Some(file.id.clone()),
                                label,
                            );
                        }
                    });
            });

            // File info badges
            if let Some(ref id) = self.selected_file {
                if let Some(file) = files.iter().find(|f| &f.id == id) {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(114.0); // align with form fields
                        badge(ui, file.video_codec.as_deref().unwrap_or("?"), INFO);
                        badge(ui, file.audio_codec.as_deref().unwrap_or("?"), SUCCESS);
                        badge(
                            ui,
                            &match (file.width, file.height) {
                                (Some(w), Some(h)) => format!("{w}×{h}"),
                                _ => "unknown".into(),
                            },
                            WARNING,
                        );
                    });
                }
            }
        });

        ui.add_space(12.0);

        // Encoding options card
        card_frame().show(ui, |ui| {
            ui.label(
                egui::RichText::new("ENCODING OPTIONS")
                    .size(11.0)
                    .strong()
                    .color(TEXT_DIM),
            );
            ui.add_space(8.0);

            // Video codec
            ui.horizontal(|ui| {
                form_label(ui, "Video Codec");
                egui::ComboBox::from_id_source("vcodec")
                    .selected_text(VideoCodec::ALL[self.enc_video_codec].label())
                    .width(250.0)
                    .show_ui(ui, |ui: &mut egui::Ui| {
                        for (i, codec) in VideoCodec::ALL.iter().enumerate() {
                            ui.selectable_value(&mut self.enc_video_codec, i, codec.label());
                        }
                    });
            });

            // Audio codec
            ui.horizontal(|ui| {
                form_label(ui, "Audio Codec");
                egui::ComboBox::from_id_source("acodec")
                    .selected_text(AudioCodec::ALL[self.enc_audio_codec].label())
                    .width(250.0)
                    .show_ui(ui, |ui: &mut egui::Ui| {
                        for (i, codec) in AudioCodec::ALL.iter().enumerate() {
                            ui.selectable_value(&mut self.enc_audio_codec, i, codec.label());
                        }
                    });
            });

            // Resolution
            ui.horizontal(|ui| {
                form_label(ui, "Resolution");
                egui::ComboBox::from_id_source("resolution")
                    .selected_text(ResolutionProfile::ALL[self.enc_resolution].label())
                    .width(250.0)
                    .show_ui(ui, |ui: &mut egui::Ui| {
                        for (i, res) in ResolutionProfile::ALL.iter().enumerate() {
                            ui.selectable_value(&mut self.enc_resolution, i, res.label());
                        }
                    });
            });

            // CRF slider
            ui.horizontal(|ui| {
                form_label(ui, "Quality (CRF)");
                ui.add(
                    egui::Slider::new(&mut self.enc_crf, 0..=51)
                        .text("lower = better"),
                );
            });
        });

        ui.add_space(16.0);

        // Submit button
        let can_encode = self.selected_file.is_some();
        ui.add_enabled_ui(can_encode, |ui| {
            if styled_button(ui, "🚀  Start Encoding", ACCENT, true)
                .on_hover_text("Submit encoding job to server")
                .clicked()
            {
                self.start_encode(ctx);
                self.selected_tab = Tab::Jobs;
            }
        });
    }

    fn show_jobs(&mut self, ui: &mut egui::Ui) {
        section_heading(ui, "Encoding Jobs");

        let state = self.state.lock().unwrap();
        let jobs = state.jobs.clone();
        drop(state);

        if jobs.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("No encoding jobs yet")
                        .size(16.0)
                        .color(TEXT_DIM),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Submit one from the Encode tab")
                        .size(13.0)
                        .color(TEXT_DIM),
                );
            });
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for job in &jobs {
                card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Status indicator
                        let (status_text, color) = match job.status {
                            JobStatus::Queued => ("⏳ Queued", WARNING),
                            JobStatus::Running => ("▶ Running", INFO),
                            JobStatus::Completed => ("✅ Done", SUCCESS),
                            JobStatus::Failed => ("❌ Failed", ERROR),
                        };
                        badge(ui, status_text, color);

                        ui.add_space(8.0);

                        // Codec/resolution badges
                        badge(ui, job.video_codec.label(), BG_ELEVATED);
                        badge(ui, job.audio_codec.label(), BG_ELEVATED);
                        badge(ui, job.resolution.label(), BG_ELEVATED);
                        if let Some(crf) = job.quality_crf {
                            badge(ui, &format!("CRF {crf}"), BG_ELEVATED);
                        }

                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                let out = job
                                    .output_path
                                    .as_deref()
                                    .and_then(|p| p.rsplit('/').next())
                                    .unwrap_or("");
                                if !out.is_empty() {
                                    ui.label(
                                        egui::RichText::new(out)
                                            .size(12.0)
                                            .color(TEXT_DIM),
                                    );
                                }
                            },
                        );
                    });

                    ui.add_space(6.0);

                    // Progress bar
                    let progress = job.progress_percent / 100.0;
                    let bar = egui::ProgressBar::new(progress)
                        .text(format!("{:.1}%", job.progress_percent))
                        .fill(match job.status {
                            JobStatus::Completed => SUCCESS,
                            JobStatus::Failed => ERROR,
                            JobStatus::Running => ACCENT,
                            JobStatus::Queued => BG_ELEVATED,
                        })
                        .rounding(egui::Rounding::same(4.0));
                    ui.add(bar);

                    // Error message
                    if let Some(ref err) = job.error {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(err)
                                .size(12.0)
                                .color(ERROR),
                        );
                    }
                });
                ui.add_space(6.0);
            }
        });
    }

    fn show_browser_window(&mut self, ctx: &egui::Context) {
        if !self.show_browser {
            return;
        }

        let mut open = self.show_browser;
        let mut selected_path: Option<String> = None;
        let mut navigate_to: Option<String> = None;

        egui::Window::new("📂  Browse Server Directories")
            .open(&mut open)
            .default_size([500.0, 450.0])
            .resizable(true)
            .collapsible(false)
            .frame(
                egui::Frame::default()
                    .fill(BG_PANEL)
                    .rounding(egui::Rounding::same(10.0))
                    .stroke(egui::Stroke::new(1.0, BORDER_SUBTLE))
                    .inner_margin(egui::Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                // Current path display with navigation
                ui.horizontal(|ui| {
                    // Up button
                    let can_go_up = self.browser_path != "/";
                    ui.add_enabled_ui(can_go_up, |ui| {
                        if styled_button(ui, "⬆  Up", BG_ELEVATED, false).clicked() {
                            let parent = std::path::Path::new(&self.browser_path)
                                .parent()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|| "/".to_string());
                            navigate_to = Some(parent);
                        }
                    });

                    // Path breadcrumb
                    ui.label(
                        egui::RichText::new("Path:")
                            .size(12.0)
                            .color(TEXT_DIM),
                    );
                    ui.label(
                        egui::RichText::new(&self.browser_path)
                            .size(13.0)
                            .color(TEXT_PRIMARY)
                            .strong(),
                    );
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                if self.browser_loading {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new("Loading…")
                                .size(13.0)
                                .color(TEXT_SECONDARY),
                        );
                    });
                } else if !self.browser_error.is_empty() {
                    ui.label(
                        egui::RichText::new(&self.browser_error)
                            .size(13.0)
                            .color(ERROR),
                    );
                } else if self.browser_entries.is_empty() {
                    ui.label(
                        egui::RichText::new("Empty directory")
                            .size(13.0)
                            .color(TEXT_DIM),
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(ui, |ui| {
                            let entries = self.browser_entries.clone();
                            for entry in &entries {
                                let icon = if entry.is_dir { "📁" } else { "📄" };
                                let text_color = if entry.is_dir {
                                    TEXT_PRIMARY
                                } else {
                                    TEXT_DIM
                                };

                                let row = ui.horizontal(|ui| {
                                    let label = egui::Label::new(
                                        egui::RichText::new(format!("{icon}  {}", entry.name))
                                            .size(13.0)
                                            .color(text_color),
                                    )
                                    .sense(egui::Sense::click());

                                    let resp = ui.add(label);
                                    if resp.double_clicked() && entry.is_dir {
                                        navigate_to = Some(entry.path.clone());
                                    }
                                    resp
                                });
                                let _ = row;
                            }
                        });
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Action buttons
                ui.horizontal(|ui| {
                    if styled_button(ui, "✅  Select This Directory", ACCENT, false).clicked() {
                        selected_path = Some(self.browser_path.clone());
                    }
                    if styled_button(ui, "Cancel", BG_ELEVATED, false).clicked() {
                        self.show_browser = false;
                    }
                });
            });

        self.show_browser = open;

        // Apply navigation after the window is done rendering
        if let Some(path) = navigate_to {
            self.browser_path = path;
            self.browse_path(ctx);
        }
        if let Some(path) = selected_path {
            self.scan_path = path;
            self.show_browser = false;
        }
    }
}

fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    let frame = egui::Frame::default()
        .fill(color.linear_multiply(0.2))
        .rounding(egui::Rounding::same(4.0))
        .inner_margin(egui::Margin::symmetric(8.0, 2.0));
    frame.show(ui, |ui| {
        ui.label(
            egui::RichText::new(text)
                .size(11.0)
                .color(color.linear_multiply(3.0)),
        );
    });
}

// ── Formatting helpers ──────────────────────────────────────────────────────

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn format_duration(secs: f64) -> String {
    let total = secs as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}
