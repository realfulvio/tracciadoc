use crate::{
    diag,
    engine::{self, Shared},
    model::{Finding, Previous, Review, Settings},
    parser, report,
};
use eframe::egui::{self, Color32, RichText};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
const ACCENT: Color32 = Color32::from_rgb(62, 207, 178);
const PRIMARY: Color32 = Color32::from_rgb(18, 128, 111);
const BG: Color32 = Color32::from_rgb(26, 29, 33);
const SURFACE: Color32 = Color32::from_rgb(35, 39, 45);
const OK: Color32 = Color32::from_rgb(126, 217, 134);
const WARN: Color32 = Color32::from_rgb(242, 184, 75);
const DANGER: Color32 = Color32::from_rgb(143, 59, 59);
/// Grigio quasi invisibile: elementi non attivi.
const DIM: Color32 = Color32::from_rgb(75, 83, 91);
fn semibold(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("Titillium SemiBold".into()))
}
fn texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> egui::TextureHandle {
    let image = image::load_from_memory(bytes)
        .expect("PNG incorporato")
        .to_rgba8();
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        ),
        egui::TextureOptions::LINEAR,
    )
}
type Preview = Option<(egui::TextureHandle, egui::TextureHandle)>;
/// Miniatura locale in memoria (nitida e sfocata). Solo immagini raster, mai su disco.
fn make_preview(ctx: &egui::Context, path: &Path, format: &str) -> Preview {
    if !["png", "jpeg", "bmp", "webp", "gif", "tiff"].contains(&format.to_ascii_lowercase().as_str())
        || std::fs::metadata(path).ok()?.len() > 32 * 1024 * 1024
    {
        return None;
    }
    let mut reader = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(parser::MAX_SIDE);
    limits.max_image_height = Some(parser::MAX_SIDE);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let small = reader.decode().ok()?.thumbnail(176, 120).to_rgba8();
    let blurred = image::imageops::blur(&small, 6.0);
    let tex = |name: &str, img: &image::RgbaImage| {
        ctx.load_texture(
            name,
            egui::ColorImage::from_rgba_unmultiplied(
                [img.width() as usize, img.height() as usize],
                img.as_raw(),
            ),
            egui::TextureOptions::LINEAR,
        )
    };
    Some((tex("anteprima-sfocata", &blurred), tex("anteprima", &small)))
}
pub struct App {
    roots: Vec<PathBuf>,
    path: String,
    settings: Settings,
    operator: String,
    reference: String,
    shared: Shared,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
    volumes: Vec<(PathBuf, String)>,
    notice: String,
    include_paths: bool,
    demo: bool,
    started: Option<Instant>,
    ocr_present: bool,
    mark: egui::TextureHandle,
    signature: egui::TextureHandle,
    zoom: f32,
    confirm: bool,
    previews: HashMap<PathBuf, Preview>,
    /// Intervallo di ridisegno durante la scansione: piu lungo sul rendering software (WARP).
    repaint: Duration,
    /// Adattatore grafico della GUI, riportato nella diagnostica.
    gpu: String,
    console_open: bool,
    /// Esito dell'ultima copia della diagnostica, mostrato per qualche secondo.
    copied: Option<(String, Instant)>,
    /// Campioni (istante, file esaminati) degli ultimi secondi: velocita e tempo residuo.
    samples: Vec<(Instant, u64)>,
}
/// Numero con separatore delle migliaia all'italiana (12.480).
fn it_num(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}
/// "1 area" / "2 aree": singolare e plurale.
fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}
fn it_eta(secs: f64) -> String {
    let s = secs.ceil() as u64;
    if s < 60 {
        format!("~ {s} s")
    } else if s < 3600 {
        format!("~ {} min", s.div_ceil(60))
    } else {
        format!("~ {} h {} min", s / 3600, (s % 3600).div_ceil(60))
    }
}
/// Cerchio che si riempie con la percentuale al centro.
fn ring(ui: &mut egui::Ui, size: f32, frac: f32, text: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let (c, r, w) = (rect.center(), size / 2. - 7., 10.);
    let painter = ui.painter();
    painter.circle_stroke(c, r, egui::Stroke::new(w, SURFACE));
    let frac = frac.clamp(0., 1.);
    if frac > 0. {
        let start = -std::f32::consts::FRAC_PI_2;
        let steps = ((frac * 96.).ceil() as usize).max(2);
        let at = |i: usize| {
            let a = start + frac * std::f32::consts::TAU * i as f32 / steps as f32;
            c + egui::vec2(a.cos(), a.sin()) * r
        };
        let points: Vec<egui::Pos2> = (0..=steps).map(at).collect();
        painter.add(egui::Shape::line(points, egui::Stroke::new(w, PRIMARY)));
        painter.circle_filled(at(0), w / 2., PRIMARY);
        painter.circle_filled(at(steps), w / 2., PRIMARY);
    }
    painter.text(c, egui::Align2::CENTER_CENTER, text, semibold(size / 4.), Color32::from_rgb(236, 239, 242));
}
fn stat(ui: &mut egui::Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.y = 2.;
        ui.label(RichText::new(label).size(14.).weak());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).font(semibold(14.5)));
        });
    });
}
/// Passo della barra laterale: cerchio numerato (spunta se concluso), evidenziato se corrente.
fn step_row(ui: &mut egui::Ui, n: u32, label: &str, active: u32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.), egui::Sense::hover());
    let painter = ui.painter();
    if n == active {
        painter.rect_filled(rect, 8., Color32::from_rgb(24, 70, 63));
    }
    let c = egui::pos2(rect.left() + 22., rect.center().y);
    let (fill, text_color) = if n < active { (OK, Color32::from_rgb(236, 239, 242)) } else if n == active { (ACCENT, Color32::WHITE) } else { (Color32::TRANSPARENT, Color32::from_rgb(154, 164, 173)) };
    painter.circle_filled(c, 11., fill);
    if n > active {
        painter.circle_stroke(c, 11., egui::Stroke::new(1.5, Color32::from_rgb(54, 60, 68)));
    }
    if n < active {
        let ink = egui::Stroke::new(2.2, Color32::from_rgb(12, 42, 16));
        painter.line_segment([c + egui::vec2(-4.5, 0.5), c + egui::vec2(-1.5, 3.5)], ink);
        painter.line_segment([c + egui::vec2(-1.5, 3.5), c + egui::vec2(4.5, -3.5)], ink);
    } else {
        painter.text(c, egui::Align2::CENTER_CENTER, n.to_string(), semibold(13.), if n == active { Color32::from_rgb(6, 49, 42) } else { text_color });
    }
    painter.text(egui::pos2(rect.left() + 44., rect.center().y), egui::Align2::LEFT_CENTER, label, semibold(17.), text_color);
}
/// Etichetta arrotondata: stato, tipo di documento, accelerazione.
fn chip(ui: &mut egui::Ui, text: &str, color: Color32, strong: bool) {
    egui::Frame::new()
        .fill(if strong { color.gamma_multiply(0.16) } else { Color32::TRANSPARENT })
        .stroke(egui::Stroke::new(1., if strong { color.gamma_multiply(0.6) } else { Color32::from_rgb(35, 39, 45) }))
        .corner_radius(99)
        .inner_margin(egui::Margin::symmetric(9, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).font(semibold(13.5)).color(color));
        });
}
fn heading(ui: &mut egui::Ui, title: &str, sub: &str) {
    ui.label(RichText::new(title).font(semibold(32.)));
    ui.label(RichText::new(sub).size(17.).weak());
}
/// Riquadro numerico della scansione: etichetta piccola, valore grande.
fn tile(ui: &mut egui::Ui, width: f32, label: &str, value: String, color: Option<Color32>) {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1., Color32::from_rgb(54, 60, 68)))
        .corner_radius(10)
        .inner_margin(14)
        .show(ui, |ui| {
            ui.set_width((width - 28.).max(60.));
            ui.label(RichText::new(label).size(12.).color(DIM));
            ui.label(RichText::new(value).font(semibold(28.)).color(color.unwrap_or(Color32::from_rgb(236, 239, 242))));
        });
}
fn primary(text: &str, height: f32) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).font(semibold(19.)).color(Color32::WHITE)).fill(PRIMARY).min_size(egui::vec2(220., height))
}
/// Tipo di documento dedotto dai motivi della segnalazione.
fn kind_of(f: &Finding) -> (&'static str, Color32) {
    for (name, prefixes, color) in [
        ("Carta d'identita", &["Carta d'identita:", "MRZ TD1:"][..], ACCENT),
        ("Passaporto", &["Passaporto:", "MRZ TD3:"][..], ACCENT),
        ("Patente", &["Patente:"][..], WARN),
        ("Tessera sanitaria", &["Tessera sanitaria:"][..], ACCENT),
    ] {
        if f.reasons.iter().any(|r| prefixes.iter().any(|p| r.starts_with(p))) {
            return (name, color);
        }
    }
    ("Da verificare", WARN)
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Tema scuro fisso: con il tema chiaro di Windows i colori del testo restavano quelli di default su sfondo scuro.
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "Titillium Regular".into(),
            Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/ui/fonts/TitilliumWeb-Regular.ttf"
            ))),
        );
        fonts.font_data.insert(
            "Titillium SemiBold".into(),
            Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/ui/fonts/TitilliumWeb-SemiBold.ttf"
            ))),
        );
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "Titillium Regular".into());
        // Titillium e Ubuntu-Light non hanno ◉ ● ○ dei passi: fallback su Hack.
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .push("Hack".into());
        let mut bold = vec!["Titillium SemiBold".into()];
        bold.extend(fonts.families[&egui::FontFamily::Proportional].clone());
        fonts
            .families
            .insert(egui::FontFamily::Name("Titillium SemiBold".into()), bold);
        cc.egui_ctx.set_fonts(fonts);
        cc.egui_ctx.style_mut_of(egui::Theme::Dark, |style| {
            style.spacing.item_spacing = egui::vec2(12., 10.);
            style.spacing.button_padding = egui::vec2(16., 10.);
            style.visuals.panel_fill = BG;
            style.visuals.window_fill = SURFACE;
            style.visuals.extreme_bg_color = Color32::from_rgb(20, 22, 26);
            style.visuals.override_text_color = Some(Color32::from_rgb(236, 239, 242));
            style.visuals.weak_text_color = Some(Color32::from_rgb(154, 164, 173));
            style.visuals.selection.bg_fill = Color32::from_rgb(24, 70, 63);
            style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(18, 150, 128);
            style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1., ACCENT);
            style.visuals.widgets.active.bg_stroke = egui::Stroke::new(2., ACCENT);
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(16.));
            style
                .text_styles
                .insert(egui::TextStyle::Button, semibold(16.));
            style
                .text_styles
                .insert(egui::TextStyle::Heading, semibold(21.));
            style
                .text_styles
                .insert(egui::TextStyle::Small, egui::FontId::proportional(13.));
        });
        let gpu = cc.wgpu_render_state.as_ref().map_or_else(
            || "OpenGL (glow)".to_string(),
            |rs| {
                let i = rs.adapter.get_info();
                format!("{:?} · {} · {:?}", i.backend, i.name, i.device_type)
            },
        );
        diag::info(format!(
            "TracciaDoc {} avviato · grafica {gpu}",
            env!("CARGO_PKG_VERSION")
        ));
        Self {
            roots: vec![],
            path: String::new(),
            settings: Settings::default(),
            operator: String::new(),
            reference: String::new(),
            shared: Arc::new(Mutex::new(Default::default())),
            stop: Arc::new(AtomicBool::new(false)),
            handle: None,
            volumes: engine::volumes(),
            notice: String::new(),
            include_paths: true,
            demo: false,
            started: None,
            mark: texture(
                &cc.egui_ctx,
                "tracciadoc-mark",
                include_bytes!("../assets/ui/mark-on-dark-128.png"),
            ),
            signature: texture(
                &cc.egui_ctx,
                "realhcok-original",
                include_bytes!("../assets/ui/realhcok-kit-white-720.png"),
            ),
            zoom: 1.,
            confirm: false,
            previews: HashMap::new(),
            gpu,
            console_open: true,
            copied: None,
            samples: vec![],
            // Su WARP ogni frame costa CPU sottratta all'OCR: a 150 ms la scansione sintetica passava da ~12 s a ~90 s.
            repaint: if cc.wgpu_render_state.as_ref().is_some_and(|rs| {
                rs.adapter.get_info().device_type == eframe::wgpu::DeviceType::Cpu
            }) {
                Duration::from_millis(1000)
            } else {
                Duration::from_millis(150)
            },
            ocr_present: parser::runtime().join("ocr/libtesseract-5.5.dll").exists()
                && parser::runtime()
                    .join("ocr/tessdata/ita.traineddata")
                    .exists()
                && parser::runtime()
                    .join("ocr/tessdata/eng.traineddata")
                    .exists(),
        }
    }
    fn add(&mut self, path: PathBuf) {
        if engine::is_network(&path) {
            self.notice = "Sono ammessi soltanto percorsi locali. Unita di rete escluse.".into();
            return;
        }
        if !path.is_absolute() || !path.is_dir() {
            self.notice = "Seleziona una cartella locale esistente con percorso assoluto.".into();
            return;
        }
        if !self.roots.contains(&path) {
            self.roots.push(path);
        }
    }
    fn start(&mut self, previous: Option<Previous>) {
        self.settings.deep = true;
        self.stop.store(false, Ordering::Relaxed);
        self.shared.lock().unwrap().run = Some(engine::new_run(
            self.roots.clone(),
            self.settings.clone(),
            self.operator.clone(),
            self.reference.clone(),
        ));
        self.shared.lock().unwrap().run.as_mut().unwrap().previous = previous;
        self.confirm = false;
        self.previews.clear();
        self.samples.clear();
        self.shared.lock().unwrap().counts = Default::default();
        let shared = self.shared.clone();
        let stop = self.stop.clone();
        self.started = Some(Instant::now());
        self.handle = Some(thread::spawn(move || engine::run(shared, stop)));
        self.notice.clear();
    }
}
fn card<R>(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui) -> R) {
    let width = ui.available_width();
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1., Color32::from_rgb(54, 60, 68)))
        .corner_radius(10)
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_min_width((width - 32.).max(0.));
            body(ui)
        });
}
impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        if self.handle.as_ref().is_some_and(|h| h.is_finished())
            && self.handle.take().unwrap().join().is_err()
        {
            let mut p = self.shared.lock().unwrap();
            if let Some(r) = &mut p.run {
                r.issue("Errore interno del coordinatore");
                diag::err("Errore interno del coordinatore (thread terminato in panico)");
                r.finished = true;
                r.interrupted = true;
                r.end = engine::now();
            }
        }
        let running = self.handle.is_some();
        if running {
            ctx.request_repaint_after(self.repaint);
        }
        let screen = ctx.content_rect();
        let short = screen.height() < 700.;
        let phase_state = self.shared.lock().unwrap().run.as_ref().map(|r| (r.finished, r.previous.is_some()));
        let active = match (running, phase_state) {
            (true, _) => 2,
            (false, Some((true, true))) => 4,
            (false, Some((true, false))) => 3,
            _ => 1,
        };
        if self.copied.as_ref().is_some_and(|(_, t)| t.elapsed() > Duration::from_secs(8)) {
            self.copied = None;
        } else if self.copied.is_some() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        let (done, total, total_final, outcome) = {
            let p = self.shared.lock().unwrap();
            (
                p.counts.done.load(Ordering::Relaxed),
                p.counts.total.load(Ordering::Relaxed),
                p.counts.total_final.load(Ordering::Relaxed),
                p.run.as_ref().map(|r| (r.finished, r.interrupted)),
            )
        };
        if running {
            self.samples.push((Instant::now(), done));
            self.samples.retain(|(t, _)| t.elapsed() < Duration::from_secs(20));
        }
        let rate = match (self.samples.first(), self.samples.last()) {
            (Some((t0, d0)), Some((t1, d1))) if t1.duration_since(*t0) >= Duration::from_secs(3) => {
                d1.saturating_sub(*d0) as f64 / t1.duration_since(*t0).as_secs_f64()
            }
            _ => 0.,
        };
        let elapsed = self.started.map_or(0., |t| t.elapsed().as_secs_f64());
        let shown = running || outcome.is_some_and(|(finished, _)| finished);
        let finished_ok = !running && outcome.is_some_and(|(f, i)| f && !i);
        let frac = if finished_ok { 1. } else if total > 0 { (done as f32 / total as f32).min(1.) } else { 0. };
        let ring_text = if finished_ok {
            "100%".to_string()
        } else {
            // Mai 100% finche la scansione non e davvero finita; "~" se il totale e ancora in conteggio.
            let pct = ((frac * 100.).floor() as u32).min(99);
            if total_final || total == 0 { format!("{pct}%") } else { format!("~{pct}%") }
        };
        let ring_label = if running { if total_final { "Scansione in corso" } else { "Stima in corso..." } } else if finished_ok { "Scansione completata" } else { "Scansione interrotta" };
        let files_text = format!("{} / {}{}", it_num(done), it_num(total), if total_final || !running { "" } else { "+" });
        let rate_text = if rate > 0. { format!("{} file/s", it_num(rate.round() as u64)) } else { "-".into() };
        let eta_text = if total_final && rate > 0.5 && done >= 30 && elapsed >= 10. {
            it_eta(total.saturating_sub(done) as f64 / rate)
        } else {
            "-".into()
        };
        let (mut run, current, phase) = {
            let p = self.shared.lock().unwrap();
            (p.run.clone(), p.current.clone(), p.phase.clone())
        };
        let mut rescan: Option<Previous> = None;

        // Console: barra sottile con stato, zoom e diagnostica; da aperta mostra circa sei righe.
        egui::Panel::bottom("console")
            .frame(egui::Frame::new().fill(Color32::from_rgb(14, 16, 19)).inner_margin(egui::Margin::symmetric(16, 8)))
            .show(root, |ui| {
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 30.), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let arrow = if self.console_open { "▾" } else { "▸" };
                    if ui.add(egui::Button::new(RichText::new(format!("{arrow} Console")).font(semibold(15.))).frame(false)).clicked() {
                        self.console_open = !self.console_open;
                    }
                    let doing = if running { format!("{phase}  {}", diag::mask(&current)) } else { "In attesa".to_string() };
                    ui.add(egui::Label::new(RichText::new(doing).small().weak()).truncate());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new("Copia diagnostica").small().color(ACCENT)).frame(false)).on_hover_text("Copia negli appunti un report per gli sviluppatori: nessun contenuto dei documenti, nome utente mascherato").clicked(){
                            let text=diag::report(&self.gpu,&phase,&current,run.as_ref());
                            let kb=text.len().div_ceil(1024);
                            // Scrittura diretta negli appunti di Windows; se fallisce ripiega su egui.
                            let direct=diag::copy_to_clipboard(&text);
                            if !direct {ctx.copy_text(text);}
                            diag::info(format!("Diagnostica copiata negli appunti ({kb} KB{})",if direct{""}else{", via egui"}));
                            self.copied=Some((format!("Diagnostica copiata negli appunti ({kb} KB): incollala nella chat"),Instant::now()));
                        }
                        egui::ComboBox::from_id_salt("ui-scale")
                            .selected_text(format!("UI {:.0}%", self.zoom * 100.))
                            .show_ui(ui, |ui| {
                                for v in [1., 1.25, 1.5, 2.] {
                                    if ui.selectable_value(&mut self.zoom, v, format!("{:.0}%", v * 100.)).changed() {
                                        ctx.set_zoom_factor(v);
                                    }
                                }
                            });
                        if let Some((msg, _)) = &self.copied {
                            ui.add(egui::Label::new(RichText::new(msg).small().color(OK)).truncate());
                        }
                    });
                });
                if self.console_open {
                    egui::ScrollArea::vertical().id_salt("console-scroll").min_scrolled_height(if short { 60. } else { 132. }).max_height(if short { 60. } else { 132. }).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.;
                        for e in diag::recent(300) {
                            let color = match e.level {
                                diag::Level::Info => Color32::from_rgb(154, 164, 173),
                                diag::Level::Ok => OK,
                                diag::Level::Warn => WARN,
                                diag::Level::Err => Color32::from_rgb(255, 148, 148),
                            };
                            ui.add(egui::Label::new(RichText::new(e.line()).monospace().size(13.).color(color)).wrap());
                        }
                    });
                }
            });

        // Barra laterale: marchio, passi, indicatore, accelerazioni, firma.
        egui::Panel::left("steps")
            .exact_size(220.)
            .frame(egui::Frame::new().fill(Color32::from_rgb(20, 22, 26)).inner_margin(16))
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Image::from_texture(&self.mark).fit_to_exact_size(egui::vec2(34., 34.)));
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.;
                        ui.label(RichText::new("TracciaDoc").font(semibold(22.)));
                        ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).small().weak());
                    });
                });
                ui.add_space(14.);
                ui.spacing_mut().item_spacing.y = 4.;
                for (n, label) in [(1, "Aree"), (2, "Scansione"), (3, "Revisione"), (4, "Verifica finale")] {
                    step_row(ui, n, label, active);
                }
                ui.add_space(14.);
                if shown {
                    ui.vertical_centered(|ui| {
                        ring(ui, if ui.available_height() < 400. { 76. } else { 104. }, frac, &ring_text);
                        ui.label(RichText::new(ring_label).size(14.).weak());
                    });
                    ui.add_space(2.);
                    stat(ui, "File", files_text.clone());
                    if running {
                        stat(ui, "Velocita", rate_text.clone());
                        stat(ui, "Tempo residuo", eta_text.clone());
                    }
                }
                // Accelerazioni subito sotto le statistiche: grigie se inattive, verdi se usate nella scansione.
                ui.add_space(14.);
                ui.label(RichText::new("ACCELERAZIONI").size(11.5).color(DIM));
                let method = engine::cpu_method(self.settings.cpu_only);
                let items = [
                    ("SSE4.2", running && (method == "avx2" || method == "sse")),
                    ("AVX2", running && method == "avx2"),
                    ("AVX-512", false),
                    ("GPU", false),
                    ("iGPU", false),
                    ("NPU", false),
                ];
                egui::Grid::new("accel").spacing([6., 6.]).show(ui, |ui| {
                    for (i, (name, on)) in items.into_iter().enumerate() {
                        chip(ui, name, if on { ACCENT } else { DIM }, on);
                        if i % 3 == 2 { ui.end_row(); }
                    }
                });
                // Firma in basso.
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    if ui.available_height() >= 28. {
                        let ratio = self.signature.size()[1] as f32 / self.signature.size()[0] as f32;
                        ui.add(egui::Image::from_texture(&self.signature).fit_to_exact_size(egui::vec2(112., 112. * ratio))).on_hover_text("powered by realhcok");
                        ui.label(RichText::new("powered by").size(12.).weak());
                    }
                });
            });

        // Barra delle azioni: una sola azione principale per fase, in basso a destra.
        egui::Panel::bottom("actions")
            .frame(egui::Frame::new().fill(Color32::from_rgb(23, 26, 30)).inner_margin(egui::Margin::symmetric(28, if short { 8 } else { 14 })))
            .show(root, |ui| {
                let h = if short { 40. } else { 48. };
                ui.horizontal(|ui| {
                    ui.set_min_height(h);
                    match active {
                        1 => {
                            let ready = !self.roots.is_empty() && !self.operator.trim().is_empty();
                            let hint = if self.roots.is_empty() { "Seleziona almeno un disco o una cartella".to_string() } else if self.operator.trim().is_empty() { "Inserisci il nome dell'operatore".to_string() } else { plural(self.roots.len(), "area selezionata", "aree selezionate") };
                            let button_w = if ui.available_width() < 560. { 170. } else { 220. };
                            let left_w = (ui.available_width() - button_w - 16.).max(60.);
                            ui.allocate_ui_with_layout(egui::vec2(left_w, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.spacing_mut().item_spacing.y = 2.;
                                if self.ocr_present {
                                    chip(ui, "OCR offline pronto", ACCENT, true);
                                } else {
                                    chip(ui, "OCR NON DISPONIBILE", WARN, true);
                                }
                                ui.add(egui::Label::new(RichText::new(hint).weak()).truncate());
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add_enabled(ready, primary("Avvia verifica", h).min_size(egui::vec2(button_w, h))).clicked() {
                                    self.start(None);
                                }
                            });
                        }
                        2 => {
                            ui.add(egui::Label::new(RichText::new(phase.as_str()).weak()).truncate());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(RichText::new("Interrompi").font(semibold(18.))).min_size(egui::vec2(160., h))).clicked() {
                                    self.stop.store(true, Ordering::Relaxed);
                                    self.notice = "Interruzione richiesta: chiusura parser e conservazione degli esiti raccolti.".into();
                                }
                            });
                        }
                        _ => {
                            let Some(run) = run.as_mut() else { return };
                            let selected = run.findings.iter().filter(|f| f.selected && !f.deleted()).count();
                            let open = run.findings.iter().filter(|f| !f.deleted()).count();
                            if self.confirm && selected > 0 {
                                ui.add(egui::Label::new(RichText::new(format!("Eliminare in modo definitivo {selected} file? Non vanno nel cestino e TracciaDoc non puo recuperarli.")).color(WARN)).truncate());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(RichText::new("Conferma eliminazione").font(semibold(18.)).color(Color32::WHITE)).fill(DANGER).min_size(egui::vec2(220., h))).clicked() {
                                        for f in run.findings.iter_mut().filter(|f| f.selected && !f.deleted()) {
                                            match crate::delete::remove(f) {Ok(())=>{f.deleted_at=engine::now();f.delete_error.clear();f.selected=false;diag::ok(format!("ELIMINATO {}",diag::shown(&f.path)));},Err(e)=>{diag::err(format!("Eliminazione fallita {} · {e}",diag::shown(&f.path)));f.delete_error=e}}
                                        }
                                        let _=engine::audit(run,"delete");diag::info(format!("Eliminazione definitiva: eliminati {}, residui {}",run.deleted_count(),run.remaining()));
                                        self.notice=format!("{} eliminati; {} residui. Eseguire la ri-scansione di verifica.",plural(run.deleted_count(),"file","file"),run.remaining());
                                        self.confirm=false;
                                    }
                                    if ui.add(egui::Button::new("Annulla").min_size(egui::vec2(0., h))).clicked() { self.confirm = false; }
                                });
                            } else {
                                let mut all = open > 0 && selected == open;
                                if ui.add_enabled(open > 0, egui::Checkbox::new(&mut all, "Seleziona tutti")).changed() {
                                    for f in run.findings.iter_mut().filter(|f| !f.deleted()) { f.selected = all; }
                                }
                                ui.add(egui::Label::new(RichText::new(plural(selected, "selezionato", "selezionati")).weak()).truncate());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let resolved = open == 0 || run.deleted_count() > 0 && selected == 0;
                                    if run.deleted_count() > 0 && selected == 0 && ui.add(primary("Ri-scansiona per la verifica finale", h)).clicked() {
                                        rescan = Some(run.summary());
                                    }
                                    if selected > 0 && ui.add(egui::Button::new(RichText::new(format!("Elimina definitivamente ({selected})")).font(semibold(18.)).color(Color32::WHITE)).fill(DANGER).min_size(egui::vec2(0., h))).clicked() {
                                        self.confirm = true;
                                    }
                                    if run.finished {
                                        let btn = if resolved && run.deleted_count() == 0 || active == 4 { primary("Genera verbale PDF...", h) } else { egui::Button::new("Genera verbale PDF...").min_size(egui::vec2(0., h)) };
                                        if ui.add(btn).clicked()
                                            && let Some(path)=rfd::FileDialog::new().set_parent(&*frame).add_filter("Verbale PDF",&["pdf"]).set_file_name(format!("Verbale-TracciaDoc-{}.pdf",run.id)).save_file() {
                                                match report::write(run,&path,self.include_paths,self.demo) {Ok(())=>{self.notice=format!("Verbale salvato: {}",diag::shown(&path));diag::ok(format!("Verbale PDF salvato: {}",diag::shown(&path)));let _=engine::audit(run,"pdf-export");},Err(e)=>{diag::err(format!("Verbale PDF non scritto: {e}"));self.notice=e}}
                                            }
                                    }
                                });
                            }
                        }
                    }
                });
            });

        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(36, 28))).show(root, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // set_max_width(1100) da solo allargava l'ui oltre il pannello nelle finestre strette.
                ui.set_max_width(ui.available_width().min(1100.));
                if !self.notice.is_empty() {
                    ui.label(RichText::new(&self.notice).color(WARN));
                }
                match active {
                    1 => {
                        heading(ui, "Dove cerchiamo?", "Scegli i dischi o le cartelle da controllare. Tutto resta su questo computer, in sola lettura.");
                        ui.add_space(12.);
                        ui.label(RichText::new("AREE").size(12.).color(DIM));
                        // Griglia a colonne fisse: le schede vanno a capo invece di uscire dalla finestra. Rete esclusa: add() la rifiuterebbe.
                        let mut areas: Vec<(PathBuf, String)> = vec![];
                        for (path, label) in &self.volumes {
                            if label.contains("rete") || areas.iter().any(|(p, _)| p == path) { continue; }
                            let text = path.to_string_lossy();
                            let kind = label.rsplit("  -  ").next().unwrap_or("");
                            let name = if text.starts_with("\\\\?\\Volume") { format!("Volume senza lettera  ·  {kind}") } else { format!("{}  ·  {kind}", text.trim_end_matches('\\')) };
                            areas.push((path.clone(), name));
                        }
                        let usable = ui.available_width().max(240.);
                        let columns = ((usable + 12.) / 332.).floor().max(1.) as usize;
                        let card_w = ((usable - 12. * (columns as f32 - 1.)) / columns as f32).max(200.);
                        egui::Grid::new("areas").spacing([12., 12.]).show(ui, |ui| {
                            for (n, (path, label)) in areas.into_iter().enumerate() {
                                let on = self.roots.contains(&path);
                                let mut v = on;
                                egui::Frame::new()
                                    .fill(if on { Color32::from_rgb(28, 42, 41) } else { SURFACE })
                                    .stroke(egui::Stroke::new(1., if on { ACCENT } else { Color32::from_rgb(54, 60, 68) }))
                                    .corner_radius(10)
                                    .inner_margin(egui::Margin::symmetric(16, 12))
                                    .show(ui, |ui| {
                                        ui.set_width(card_w - 34.);
                                        ui.set_max_width(card_w - 34.);
                                        ui.checkbox(&mut v, RichText::new(label).font(semibold(if card_w < 340. { 15. } else { 18. })));
                                    })
                                    .response
                                    .on_hover_text(diag::shown(&path));
                                if v != on {
                                    if v { self.add(path); } else { self.roots.retain(|r| r != &path); }
                                }
                                if (n + 1) % columns == 0 { ui.end_row(); }
                            }
                        });
                        let mut remove = None;
                        for (i, p) in self.roots.iter().enumerate() {
                            if self.volumes.iter().any(|(v, _)| v == p) { continue; }
                            ui.horizontal(|ui| {
                                ui.label(diag::shown(p));
                                if ui.small_button("Rimuovi").clicked() { remove = Some(i); }
                            });
                        }
                        if let Some(i) = remove { self.roots.remove(i); }
                        if ui.button("＋ Aggiungi cartella...").clicked() && let Some(p) = rfd::FileDialog::new().set_parent(&*frame).pick_folder() { self.add(p); }
                        ui.add_space(14.);
                        card(ui, |ui| {
                            ui.label(RichText::new("DATI DEL VERBALE").size(12.).color(DIM));
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Operatore");
                                ui.add(egui::TextEdit::singleline(&mut self.operator).hint_text("nome e cognome").desired_width(220.));
                                ui.label("Riferimento");
                                ui.add(egui::TextEdit::singleline(&mut self.reference).hint_text("facoltativo").desired_width(260.));
                            });
                        });
                        ui.add_space(6.);
                        ui.collapsing("Altre opzioni di selezione", |ui| {
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut self.path).hint_text("Percorso locale, anche volume montato").desired_width((ui.available_width() - 110.).max(130.)));
                                if ui.button("Aggiungi").clicked() { self.add(PathBuf::from(self.path.trim())); }
                            });
                            if ui.button("Carica prove sintetiche").clicked() {
                                let p=std::env::current_exe().unwrap().parent().unwrap().join("prove-sintetiche");
                                if p.is_dir() {self.roots=vec![p];self.demo=true;self.operator="Operatore dimostrativo".into();self.reference="COLLAUDO SINTETICO".into();}
                                else {self.notice="Cartella prove-sintetiche assente accanto all'EXE.".into();}
                            }
                        });
                        ui.collapsing("Copertura e opzioni avanzate", |ui| {
                            ui.label("Cerca immagini, PDF anche misti, RTF e Office con immagini incorporate, con OCR locale entro i limiti dichiarati. I file di solo testo non vengono letti.");
                            ui.label("Incluse tutte le sottocartelle accessibili: profili, AppData, temporanei e cestino. Nessuna estensione esclusa silenziosamente.");
                            ui.label("Esclusi percorsi esterni, rete, collegamenti e junction. I volumi montati discendenti vanno selezionati separatamente. File cloud offline, EFS, negati, formati non supportati e limiti rendono la verifica incompleta.");
                            ui.horizontal_wrapped(|ui| {ui.selectable_value(&mut self.settings.cpu_only,false,"Automatico");ui.selectable_value(&mut self.settings.cpu_only,true,"Solo CPU");});
                            ui.checkbox(&mut self.settings.prefilter,"Prefiltro: salta l'OCR delle immagini senza tratti tipo testo (dichiarate nel verbale come scartate)");
                            ui.label(format!("Disponibile: CPU. Automatico: {} rilevato a runtime; Solo CPU: prodotto scalare generico per confronto. GPU/NPU non implementate. Nessun requisito AVX globale; AVX-512 non selezionata.",engine::cpu_method(false)));
                            ui.horizontal(|ui|{ui.label("MiB/file");ui.add(egui::DragValue::new(&mut self.settings.max_mb).range(1..=256));ui.label("Pagine/immagini");ui.add(egui::DragValue::new(&mut self.settings.max_pages).range(1..=500));});
                            ui.horizontal(|ui|{ui.label("Secondi/file");ui.add(egui::DragValue::new(&mut self.settings.timeout_secs).range(1..=1800));ui.label("MiB worker");ui.add(egui::DragValue::new(&mut self.settings.memory_mb).range(256..=2048));});
                            ui.horizontal(|ui|{ui.label("Limite file");ui.add(egui::DragValue::new(&mut self.settings.max_entries).range(1..=10_000_000));});
                            ui.horizontal(|ui|{ui.label("Worker OCR in parallelo");ui.add(egui::DragValue::new(&mut self.settings.workers).range(1..=engine::physical_cores().max(1)));ui.label(format!("(core fisici: {}; ogni worker usa fino a {} MiB)",engine::physical_cores(),self.settings.memory_mb));});
                            ui.label("Concorrenza: ogni worker e un processo separato con un thread OCR e il proprio limite di memoria. Su HDD lenti ridurre a 1. Nessuna cache; nessun temporaneo OCR su disco. Limiti di decodifica e Office descritti nel verbale.");
                            ui.label(format!("Storico essenziale: {}. Solo conteggi e limiti, senza contenuti o percorsi. ACL ereditate dal profilo; nessuna protezione contro amministratori.",engine::state_dir().display()));
                        });
                    }
                    2 => {
                        heading(ui, "Scansione in corso", "Puoi lasciare il computer cosi: alla fine ti mostro cosa ho trovato.");
                        ui.add_space(14.);
                        let findings = run.as_ref().map_or(0, |r| r.findings.len());
                        ui.horizontal(|ui| {
                            ring(ui, 220., frac, &ring_text);
                            let w = ((ui.available_width() - 12.) / 2.).max(130.);
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    tile(ui, w, "FILE ANALIZZATI", files_text.clone(), None);
                                    tile(ui, w, "SOSPETTI TROVATI", findings.to_string(), if findings > 0 { Some(WARN) } else { None });
                                });
                                ui.horizontal(|ui| {
                                    tile(ui, w, "VELOCITA", rate_text.clone(), None);
                                    tile(ui, w, "TEMPO RESIDUO", eta_text.clone(), None);
                                });
                            });
                        });
                        ui.add_space(10.);
                        card(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Niente ui.spinner(): chiede un ridisegno a ogni frame; l'indicatore avanza col ridisegno periodico.
                                let tick = self.started.map(|s| s.elapsed().as_millis() / self.repaint.as_millis()).unwrap_or(0) as usize;
                                ui.label(RichText::new(["◐","◓","◑","◒"][tick % 4]).color(ACCENT));
                                ui.label(RichText::new("ORA IN ESAME").size(12.).color(DIM));
                                ui.add(egui::Label::new(RichText::new(diag::mask(&current)).monospace().size(13.).weak()).truncate());
                            });
                        });
                        if let Some(run) = &run {
                            ui.collapsing("Dettagli", |ui| {
                                ui.label(format!("Incontrati {}    Analizzati {}    Parziali {}    Saltati {}    Ignorati per tipo {}    Tempo {:.1}s",run.encountered,run.analyzed,run.partial_files,run.skipped,run.ignored_type,elapsed));
                                if run.timing.prefilter_skipped>0 {ui.label(format!("Immagini scartate dal prefiltro (senza OCR): {}",run.timing.prefilter_skipped));}
                            });
                        }
                    }
                    _ => {
                        if let Some(run) = run.as_mut() {
                            let remaining = run.remaining();
                            let title = if active == 4 { "Verifica finale".to_string() } else if remaining == 0 && run.deleted_count() == 0 { "Nessun file da rivedere".to_string() } else if remaining == 0 { "Tutto eliminato".to_string() } else if remaining == 1 { "1 file da rivedere".to_string() } else { format!("{remaining} file da rivedere") };
                            ui.label(RichText::new(title).font(semibold(32.)));
                            ui.label(RichText::new(run.outcome()).font(semibold(17.)).color(if run.incomplete() { WARN } else { ACCENT }));
                            if remaining > 0 { ui.label(RichText::new("Controlla le anteprime e spunta i file da eliminare.").weak()); }
                            ui.add_space(10.);
                            // Al massimo 2 decodifiche per frame: la GUI resta reattiva anche con molti risultati.
                            let mut budget = 2;
                            let running_now = running;
                            for (i, f) in run.findings.iter_mut().enumerate().take(5000) {
                                // I file eliminati escono dalla lista: restano nel conteggio, nella console e nel verbale.
                                if f.deleted() { continue; }
                                // Finestra stretta: l'anteprima resta solo se resta spazio per il testo.
                                let side = if ui.available_width() >= 560. { 104. } else { 0. };
                                if side > 0. && budget > 0 && !self.previews.contains_key(&f.path) {
                                    budget -= 1;
                                    let pv = make_preview(&ctx, &f.path, &f.format);
                                    self.previews.insert(f.path.clone(), pv);
                                    ctx.request_repaint();
                                }
                                let (kind, color) = kind_of(f);
                                egui::Frame::new()
                                    .fill(if f.selected { Color32::from_rgb(28, 42, 41) } else { SURFACE })
                                    .stroke(egui::Stroke::new(1., if f.selected { ACCENT } else { Color32::from_rgb(54, 60, 68) }))
                                    .corner_radius(10)
                                    .inner_margin(14)
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.add_enabled(!running_now, egui::Checkbox::without_text(&mut f.selected)).on_hover_text("Da eliminare");
                                            if side > 0. {
                                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(side, 72.), egui::Sense::hover());
                                                ui.painter().rect_filled(rect, 8., BG);
                                                match self.previews.get(&f.path) {
                                                    Some(Some((blur, sharp))) => {
                                                        let tex = if resp.hovered() { sharp } else { blur };
                                                        egui::Image::from_texture(tex).fit_to_exact_size(rect.size()).paint_at(ui, rect);
                                                    }
                                                    _ => { ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, f.format.to_uppercase(), semibold(14.), Color32::from_rgb(154, 164, 173)); }
                                                }
                                                if !resp.hovered() && matches!(self.previews.get(&f.path), Some(Some(_))) {
                                                    ui.painter().text(rect.center_bottom() - egui::vec2(0., 4.), egui::Align2::CENTER_BOTTOM, "passa sopra", egui::FontId::proportional(11.), Color32::WHITE);
                                                }
                                            }
                                            let w = (ui.available_width() - 200.).max(100.);
                                            ui.allocate_ui(egui::vec2(w, 0.), |ui| {
                                                ui.set_width(w);
                                                ui.vertical(|ui| {
                                                    ui.spacing_mut().item_spacing.y = 3.;
                                                    ui.horizontal(|ui| {
                                                        let name = f.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| f.path.display().to_string());
                                                        ui.add(egui::Label::new(RichText::new(name).font(semibold(19.))).truncate());
                                                        chip(ui, kind, color, true);
                                                    });
                                                    ui.add(egui::Label::new(RichText::new(diag::shown(&f.path)).monospace().size(12.5).weak()).truncate());
                                                    if let Some(reason) = f.reasons.first() {
                                                        ui.add(egui::Label::new(RichText::new(reason).small()).truncate()).on_hover_text(f.reasons.join("\n"));
                                                    }
                                                    if !f.delete_error.is_empty() { ui.label(RichText::new(&f.delete_error).color(WARN)); }
                                                });
                                            });
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                ui.add_enabled_ui(!running_now, |ui| {
                                                    ui.vertical(|ui| {
                                                        ui.horizontal(|ui| {
                                                            if ui.small_button("Cartella").clicked() && let Err(e) = crate::open::folder(&f.path) { self.notice = e; }
                                                            if ui.small_button("Apri").clicked() && let Err(e) = crate::open::document(f) { self.notice = e; }
                                                        });
                                                        egui::ComboBox::from_id_salt(("review", i)).selected_text(f.review.label()).width(150.).show_ui(ui, |ui| {
                                                            for value in [Review::Pending, Review::Confirmed, Review::Discarded] { ui.selectable_value(&mut f.review, value, value.label()); }
                                                        });
                                                    });
                                                });
                                            });
                                        });
                                    });
                                ui.add_space(4.);
                            }
                            ui.add_space(8.);
                            if run.finished && remaining > 0 || run.deleted_count() > 0 {
                                ui.collapsing("Riepilogo", |ui| { for line in run.finding_summary() { ui.label(line); } });
                            }
                            if !run.issues.is_empty() {
                                ui.collapsing(format!("Errori e limiti ({})", plural(run.issues.len(), "categoria", "categorie")), |ui| { for (s, n) in &run.issues { ui.label(format!("{n} - {s}")); } });
                            }
                            ui.collapsing("Opzioni del verbale", |ui| {
                                ui.checkbox(&mut self.include_paths, "Includi percorsi dei file segnalati nel PDF (possono contenere dati personali)");
                                ui.checkbox(&mut self.demo, "Contrassegna il verbale come DIMOSTRATIVO");
                            });
                        }
                    }
                }
                ui.add_space(12.);
                ui.label(RichText::new("Il controllo non certifica l'assenza assoluta di documenti. Le segnalazioni richiedono verifica manuale. Nessuna approvazione Lepida.").small().weak());
            });
        });
        if !running && let Some(run) = &run {
            self.shared.lock().unwrap().run = Some(run.clone());
        }
        if let Some(p) = rescan {
            if let Some(run) = &run { self.roots = run.roots.clone(); }
            self.start(Some(p));
        }
    }
}
impl Drop for App {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
