use crate::{
    diag,
    engine::{self, Shared},
    install, sched,
    model::{Finding, Previous, Review, Settings},
    parser,
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
/// Tema chiaro attivo (impostato dal pulsante in console). Tavolozza scelta a ogni disegno.
static LIGHT: AtomicBool = AtomicBool::new(false);
fn light() -> bool {
    LIGHT.load(Ordering::Relaxed)
}
fn pick(dark: (u8, u8, u8), bright: (u8, u8, u8)) -> Color32 {
    let (r, g, b) = if light() { bright } else { dark };
    Color32::from_rgb(r, g, b)
}
/// I loghi sono bianchi per lo sfondo scuro: sul tema chiaro si tingono di blu notte.
fn logo_tint() -> Color32 { if light() { Color32::from_rgb(20, 32, 46) } else { Color32::WHITE } }
fn c_accent() -> Color32 { pick((47, 209, 176), (11, 91, 140)) }
fn c_primary() -> Color32 { pick((22, 154, 130), (11, 91, 140)) }
fn c_bg() -> Color32 { pick((18, 20, 24), (238, 242, 246)) }
fn c_surface() -> Color32 { pick((27, 30, 36), (255, 255, 255)) }
fn c_side() -> Color32 { pick((14, 16, 19), (223, 231, 239)) }
fn c_bar() -> Color32 { pick((22, 25, 29), (230, 236, 243)) }
fn c_console() -> Color32 { pick((11, 13, 16), (214, 223, 233)) }
fn c_line() -> Color32 { pick((43, 48, 57), (211, 220, 230)) }
fn c_text() -> Color32 { pick((233, 236, 239), (20, 32, 46)) }
fn c_weak() -> Color32 { pick((141, 148, 159), (93, 107, 124)) }
fn c_dim() -> Color32 { pick((75, 83, 91), (150, 162, 176)) }
fn c_ok() -> Color32 { pick((126, 217, 134), (30, 140, 70)) }
fn c_warn() -> Color32 { pick((244, 183, 63), (184, 106, 0)) }
fn c_danger() -> Color32 { pick((168, 50, 58), (179, 40, 47)) }
fn c_err() -> Color32 { pick((255, 148, 148), (179, 40, 47)) }
fn c_sel() -> Color32 { pick((18, 51, 47), (220, 235, 245)) }
fn c_sel_soft() -> Color32 { pick((22, 44, 43), (232, 242, 249)) }
/// Colore del segno dentro i cerchi pieni (spunta, numero).
fn c_ink() -> Color32 { pick((7, 32, 27), (255, 255, 255)) }
/// Stile egui dalla tavolozza corrente; da richiamare a ogni cambio di tema.
fn apply_style(ctx: &egui::Context) {
    let theme = if light() { egui::Theme::Light } else { egui::Theme::Dark };
    ctx.set_theme(theme);
    ctx.style_mut_of(theme, |style| {
        style.spacing.item_spacing = egui::vec2(12., 10.);
        style.spacing.button_padding = egui::vec2(16., 10.);
        style.visuals.panel_fill = c_bg();
        style.visuals.window_fill = c_surface();
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1., pick((96, 106, 116), (160, 172, 186)));
        style.visuals.widgets.inactive.weak_bg_fill = pick((40, 44, 52), (244, 247, 250));
        style.visuals.extreme_bg_color = c_side();
        style.visuals.override_text_color = Some(c_text());
        style.visuals.weak_text_color = Some(c_weak());
        style.visuals.selection.bg_fill = c_sel();
        style.visuals.widgets.hovered.bg_fill = pick((18, 150, 128), (200, 222, 238));
        style.visuals.widgets.hovered.weak_bg_fill = pick((48, 54, 64), (226, 236, 246));
        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1., c_accent());
        style.visuals.widgets.active.bg_stroke = egui::Stroke::new(2., c_accent());
        style.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(16.));
        style.text_styles.insert(egui::TextStyle::Button, semibold(16.));
        style.text_styles.insert(egui::TextStyle::Heading, semibold(21.));
        style.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(13.));
    });
}
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
    let small = reader.decode().ok()?.thumbnail(240, 160).to_rgba8();
    let blurred = image::imageops::blur(&small, 8.0);
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
    last_pdf: Option<std::path::PathBuf>,
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
    /// Zoom iniziale scelto in base alla larghezza dello schermo (una sola volta).
    zoom_set: bool,
    /// Capacita e spazio libero (byte) dei volumi, per le schede delle aree.
    space: HashMap<PathBuf, (u64, u64)>,
    /// File mostrato nel pannello di dettaglio della revisione.
    focus: Option<PathBuf>,
    /// Filtro per tipo di documento nella revisione ("" = tutti).
    filter: String,
    /// File al secondo, un campione ogni ~1 s, per il grafico della scansione.
    rate_hist: Vec<f32>,
    rate_at: Instant,
    /// Pagina "Pianificazione" aperta al posto del flusso di verifica.
    plan_page: bool,
    /// Configurazione salvata in ProgramData e bozza in modifica.
    cfg: sched::Config,
    draft: sched::Plan,
    plan_on: bool,
    update_mode: sched::UpdateMode,
    plan_msg: String,
    /// Comando eseguito come amministratore (UAC) in un thread: esito "OK ..."/"ERR ...".
    job: Option<Arc<Mutex<Option<String>>>>,
    /// Arresta il PC al termine se la scansione non trova documenti (verbale salvato prima).
    shutdown_after: bool,
    shutdown_at: Option<Instant>,
    /// Tenuto durante la scansione: l'aggiornamento automatico aspetta.
    scan_lock: Option<sched::ScanLock>,
    /// Esito di una scansione pianificata con documenti non ancora rivisti.
    pending_review: Option<crate::model::Run>,
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
    painter.circle_stroke(c, r, egui::Stroke::new(w, c_surface()));
    let frac = frac.clamp(0., 1.);
    if frac > 0. {
        let start = -std::f32::consts::FRAC_PI_2;
        let steps = ((frac * 96.).ceil() as usize).max(2);
        let at = |i: usize| {
            let a = start + frac * std::f32::consts::TAU * i as f32 / steps as f32;
            c + egui::vec2(a.cos(), a.sin()) * r
        };
        let points: Vec<egui::Pos2> = (0..=steps).map(at).collect();
        painter.add(egui::Shape::line(points, egui::Stroke::new(w, c_accent())));
        painter.circle_filled(at(0), w / 2., c_accent());
        painter.circle_filled(at(steps), w / 2., c_accent());
    }
    painter.text(c, egui::Align2::CENTER_CENTER, text, semibold(size / 4.), c_text());
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
        painter.rect_filled(rect, 8., c_sel());
    }
    let c = egui::pos2(rect.left() + 22., rect.center().y);
    let (fill, text_color) = if n <= active { (c_accent(), c_text()) } else { (Color32::TRANSPARENT, c_weak()) };
    painter.circle_filled(c, 11., fill);
    if n > active {
        painter.circle_stroke(c, 11., egui::Stroke::new(1.5, c_line()));
    }
    if n < active {
        let ink = egui::Stroke::new(2.2, c_ink());
        painter.line_segment([c + egui::vec2(-4.5, 0.5), c + egui::vec2(-1.5, 3.5)], ink);
        painter.line_segment([c + egui::vec2(-1.5, 3.5), c + egui::vec2(4.5, -3.5)], ink);
    } else {
        painter.text(c, egui::Align2::CENTER_CENTER, n.to_string(), semibold(13.), if n == active { c_ink() } else { text_color });
    }
    painter.text(egui::pos2(rect.left() + 44., rect.center().y), egui::Align2::LEFT_CENTER, label, semibold(17.), text_color);
}
/// Etichetta arrotondata: stato, tipo di documento, accelerazione.
fn chip(ui: &mut egui::Ui, text: &str, color: Color32, strong: bool) {
    egui::Frame::new()
        .fill(if strong { color.gamma_multiply(0.16) } else { Color32::TRANSPARENT })
        .stroke(egui::Stroke::new(1., if strong { color.gamma_multiply(0.6) } else { c_line() }))
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
        .fill(c_surface())
        .stroke(egui::Stroke::new(1., c_line()))
        .corner_radius(10)
        .inner_margin(14)
        .show(ui, |ui| {
            ui.set_width((width - 28.).max(60.));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.;
                ui.label(RichText::new(label).size(12.5).color(c_weak()));
                ui.label(RichText::new(value).font(semibold(30.)).color(color.unwrap_or(c_text())));
            });
        });
}
fn primary(text: &str, height: f32) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).font(semibold(19.)).color(Color32::WHITE)).fill(c_primary()).min_size(egui::vec2(220., height))
}
/// Dimensione di un file in KB/MB.
fn size_text(bytes: u64) -> String {
    if bytes >= 1_048_576 { format!("{:.1} MB", bytes as f64 / 1_048_576.).replace('.', ",") } else { format!("{} KB", bytes.div_ceil(1024)) }
}
/// Dimensione in GB/TB all'italiana.
fn it_size(bytes: u64) -> String {
    let gb = bytes as f64 / 1_073_741_824.;
    if gb >= 1000. { format!("{:.1} TB", gb / 1024.).replace('.', ",") } else { format!("{} GB", gb.round() as u64) }
}
/// Barra orizzontale piena per la frazione data.
fn bar(ui: &mut egui::Ui, frac: f32, height: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
    ui.painter().rect_filled(rect, height / 2., c_dim().gamma_multiply(0.35));
    let frac = frac.clamp(0., 1.);
    if frac > 0. {
        let fill = egui::Rect::from_min_size(rect.min, egui::vec2((rect.width() * frac).max(height), height));
        ui.painter().rect_filled(fill, height / 2., color);
    }
}
/// Grafico a linea dei campioni (file al secondo).
fn spark(ui: &mut egui::Ui, data: &[f32], height: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], egui::Stroke::new(1., c_line()));
    if data.len() < 2 {
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "il grafico compare dopo qualche secondo di scansione", egui::FontId::proportional(13.), c_weak());
        return;
    }
    let max = data.iter().cloned().fold(1., f32::max);
    let n = data.len() as f32 - 1.;
    let pts: Vec<egui::Pos2> = data.iter().enumerate().map(|(i, v)| egui::pos2(rect.left() + rect.width() * i as f32 / n, rect.bottom() - 4. - (rect.height() - 8.) * (v / max))).collect();
    ui.painter().add(egui::Shape::line(pts, egui::Stroke::new(2.5, c_accent())));
}
/// Giudizio sugli indizi di un file: tipo di riscontri trovati, non una probabilita statistica.
fn level_of(f: &Finding) -> (&'static str, Color32, f32) {
    let strong = f.reasons.iter().any(|r| r.starts_with("MRZ") || r.contains("numerazione formalmente compatibile") || r.contains("codice fiscale superato"));
    // Fototessera "probabile": volto frontale unico su sfondo uniforme, un buon indizio ma senza testo.
    let photo = f.reasons.iter().any(|r| r.starts_with("Fototessera:") && r.contains("(probabile)"));
    match (strong, f.reasons.len()) {
        (false, 1) if photo => ("Indizi buoni", c_accent(), 0.7),
        (true, n) if n >= 2 => ("Indizi forti", c_accent(), 0.95),
        (true, _) => ("Indizi buoni", c_accent(), 0.75),
        (false, n) if n >= 2 => ("Indizi buoni", c_accent(), 0.7),
        _ => ("Indizi parziali", c_warn(), 0.45),
    }
}
/// Pulsante-filtro a pillola.
fn pill(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(14.).color(if on { c_accent() } else { c_weak() }))
            .fill(if on { c_sel() } else { Color32::TRANSPARENT })
            .stroke(egui::Stroke::new(1., if on { c_accent() } else { c_line() }))
            .corner_radius(99)
            .min_size(egui::vec2(0., 30.)),
    )
}
/// Anteprima del verbale: pagina disegnata con i numeri reali, non il PDF vero.
fn paper(ui: &mut egui::Ui, width: f32, before: u64, after: u64, date: &str) {
    let size = egui::vec2(width.min(380.), width.min(380.) * 1.36);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    let ink = Color32::from_rgb(26, 34, 48);
    let mute = Color32::from_rgb(102, 110, 124);
    let teal = Color32::from_rgb(22, 154, 130);
    p.rect_filled(rect.translate(egui::vec2(0., 5.)), 4., Color32::from_black_alpha(60));
    p.rect_filled(rect, 4., Color32::WHITE);
    let k = size.x / 380.;
    let m = 30. * k;
    let x0 = rect.left() + m;
    let x1 = rect.right() - m;
    let mut y = rect.top() + m;
    p.text(egui::pos2(x0, y), egui::Align2::LEFT_TOP, "TracciaDoc", semibold(16. * k), ink);
    p.text(egui::pos2(x1, y + 3. * k), egui::Align2::RIGHT_TOP, "Verbale di verifica", egui::FontId::proportional(10. * k), mute);
    y += 28. * k;
    p.line_segment([egui::pos2(x0, y), egui::pos2(x1, y)], egui::Stroke::new(2., teal));
    y += 14. * k;
    p.text(egui::pos2(x0, y), egui::Align2::LEFT_TOP, "Verbale di verifica\ndocumenti d'identita", semibold(17. * k), ink);
    y += 50. * k;
    p.text(egui::pos2(x0, y), egui::Align2::LEFT_TOP, format!("Data {date}  ·  Operatore: ________"), egui::FontId::proportional(10. * k), mute);
    y += 26. * k;
    let bw = (x1 - x0 - 12. * k) / 2.;
    for (i, (label, value, color, fill)) in [("Prima", before, Color32::from_rgb(184, 106, 0), Color32::WHITE), ("Dopo", after, teal, Color32::from_rgb(234, 248, 245))].into_iter().enumerate() {
        let b = egui::Rect::from_min_size(egui::pos2(x0 + i as f32 * (bw + 12. * k), y), egui::vec2(bw, 62. * k));
        p.rect_filled(b, 4., fill);
        p.rect_stroke(b, 4., egui::Stroke::new(1., if i == 1 { teal } else { Color32::from_rgb(213, 219, 226) }), egui::StrokeKind::Inside);
        p.text(b.min + egui::vec2(8. * k, 6. * k), egui::Align2::LEFT_TOP, label, egui::FontId::proportional(10. * k), mute);
        p.text(b.min + egui::vec2(8. * k, 22. * k), egui::Align2::LEFT_TOP, value.to_string(), semibold(26. * k), color);
    }
    y += 82. * k;
    p.text(egui::pos2(x0, y), egui::Align2::LEFT_TOP, "Elenco dei file eliminati", semibold(11. * k), ink);
    y += 20. * k;
    for _ in 0..5 {
        p.rect_filled(egui::Rect::from_min_size(egui::pos2(x0, y), egui::vec2(x1 - x0, 5. * k)), 3., Color32::from_rgb(230, 235, 240));
        y += 15. * k;
    }
    y += 6. * k;
    let note = egui::Rect::from_min_size(egui::pos2(x0, y), egui::vec2(x1 - x0, 44. * k));
    p.rect_filled(note, 3., Color32::from_rgb(234, 248, 245));
    p.rect_filled(egui::Rect::from_min_size(note.min, egui::vec2(3., note.height())), 0., teal);
    p.text(note.min + egui::vec2(12. * k, 8. * k), egui::Align2::LEFT_TOP, if after == 0 { "Nessun documento rilevato\nnell'ambito esaminato." } else { "Restano segnalazioni\nda rivedere." }, egui::FontId::proportional(10. * k), Color32::from_rgb(14, 90, 76));
}
/// Tipo di documento dedotto dai motivi della segnalazione.
fn kind_of(f: &Finding) -> (&'static str, Color32) {
    for (name, prefixes, color) in [
        ("Carta d'identita", &["Carta d'identita:", "MRZ TD1:"][..], c_accent()),
        ("Passaporto", &["Passaporto:", "MRZ TD3:"][..], c_accent()),
        ("Patente", &["Patente:"][..], c_warn()),
        ("Tessera sanitaria", &["Tessera sanitaria:"][..], c_accent()),
        ("Fototessera", &["Fototessera:"][..], c_warn()),
    ] {
        if f.reasons.iter().any(|r| prefixes.iter().any(|p| r.starts_with(p))) {
            return (name, color);
        }
    }
    ("Da verificare", c_warn())
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Riprova in background i verbali rimasti solo in locale.
        std::thread::spawn(|| {
            let (ok, left) = crate::shared::riprova_sospesi(&crate::shared::local_dir(), crate::shared::report_dir(None).as_deref());
            if ok > 0 {
                diag::ok(format!("Verbali in sospeso consegnati alla cartella condivisa: {ok}"));
            }
            if left > 0 {
                diag::warn(format!("Verbali ancora in sospeso (cartella condivisa non raggiungibile): {left}"));
            }
        });
        // Tema scelto dall'app (scuro all'avvio): il tema di Windows non conta, i colori del testo sono sempre espliciti.
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
        apply_style(&cc.egui_ctx);
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
        let volumes = engine::volumes();
        let space: HashMap<PathBuf, (u64, u64)> = volumes.iter().filter_map(|(p, _)| engine::disk_space(p).map(|s| (p.clone(), s))).collect();
        // Di default tutti i dischi fissi con lettera: la scansione parte solo dal pulsante.
        let roots: Vec<PathBuf> = volumes.iter().filter(|(p, l)| l.ends_with("locale fisso") && !p.to_string_lossy().starts_with("\\\\?\\Volume")).map(|(p, _)| p.clone()).collect();
        let cfg = sched::load_config();
        Self {
            roots,
            path: String::new(),
            settings: Settings::default(),
            operator: String::new(),
            reference: String::new(),
            shared: Arc::new(Mutex::new(Default::default())),
            stop: Arc::new(AtomicBool::new(false)),
            handle: None,
            volumes,
            notice: String::new(),
            last_pdf: None,
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
            zoom_set: false,
            confirm: false,
            previews: HashMap::new(),
            gpu,
            console_open: true,
            copied: None,
            samples: vec![],
            space,
            focus: None,
            filter: String::new(),
            rate_hist: vec![],
            rate_at: Instant::now(),
            plan_page: std::env::args().any(|a| a == "--pianificazione"),
            draft: {
                let mut d = cfg.plan.clone().unwrap_or_default();
                if let Some(day) = sched::parse_date(&d.start) { d.start = day.format("%d/%m/%Y").to_string(); }
                if let Some(day) = sched::parse_date(&d.end) { d.end = day.format("%d/%m/%Y").to_string(); }
                d
            },
            plan_on: cfg.plan.is_some(),
            update_mode: cfg.update,
            cfg,
            plan_msg: String::new(),
            job: None,
            shutdown_after: false,
            shutdown_at: None,
            scan_lock: None,
            pending_review: std::fs::read(sched::last_run_path())
                .ok()
                .and_then(|b| serde_json::from_slice::<crate::model::Run>(&b).ok())
                .filter(|r| r.remaining() > 0 && !sched::seen(&r.id)),
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
        self.rate_hist.clear();
        self.shared.lock().unwrap().counts = Default::default();
        let shared = self.shared.clone();
        let stop = self.stop.clone();
        self.started = Some(Instant::now());
        self.scan_lock = sched::scan_lock();
        self.handle = Some(thread::spawn(move || engine::run(shared, stop)));
        self.notice.clear();
    }
}
/// Voce "Pianificazione" della barra laterale: orologio disegnato, cliccabile se non c'è una scansione.
fn plan_row(ui: &mut egui::Ui, on: bool, disabled: bool) -> egui::Response {
    let sense = if disabled { egui::Sense::hover() } else { egui::Sense::click() };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.), sense);
    let painter = ui.painter();
    if on {
        painter.rect_filled(rect, 8., c_sel());
    } else if resp.hovered() {
        painter.rect_filled(rect, 8., c_sel_soft());
    }
    let c = egui::pos2(rect.left() + 22., rect.center().y);
    let color = if disabled { c_dim() } else if on { c_accent() } else { c_weak() };
    painter.circle_stroke(c, 10., egui::Stroke::new(1.8, color));
    painter.line_segment([c, c + egui::vec2(0., -6.)], egui::Stroke::new(1.8, color));
    painter.line_segment([c, c + egui::vec2(4.5, 2.)], egui::Stroke::new(1.8, color));
    painter.text(egui::pos2(rect.left() + 44., rect.center().y), egui::Align2::LEFT_CENTER, "Pianificazione", semibold(17.), if on { c_text() } else { color });
    if disabled { resp.on_hover_text("Disponibile al termine della scansione") } else { resp.on_hover_cursor(egui::CursorIcon::PointingHand) }
}
fn label_dim(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(12.).color(c_dim()));
}
impl App {
    /// Fine scansione con "Arresta il PC": verbale nella cartella dei verbali, poi conto alla rovescia.
    fn after_scan(&mut self) {
        if !self.shutdown_after {
            return;
        }
        let Some(run) = self.shared.lock().unwrap().run.clone() else { return };
        if run.interrupted || !run.findings.is_empty() {
            self.notice = "Spegnimento non eseguito: la scansione ha trovato documenti o è stata interrotta. Rivedi i risultati.".into();
            diag::warn("Spegnimento non eseguito: documenti trovati o scansione interrotta");
            return;
        }
        match sched::save_report(&run) {
            Ok(path) => {
                diag::ok(format!("Verbale PDF salvato: {}", diag::shown(&path)));
                let _ = engine::audit(&run, "pdf-export");
                self.notice = format!("Verbale salvato: {}", diag::shown(&path));
                self.shutdown_at = Some(Instant::now() + Duration::from_secs(sched::SHUTDOWN_DELAY));
            }
            Err(e) => {
                diag::err(format!("Verbale PDF non scritto, spegnimento annullato: {e}"));
                self.notice = format!("Verbale non salvato ({e}): il PC resta acceso.");
            }
        }
    }
    /// Lancia questo EXE come amministratore (UAC) in un thread; l'esito arriva da un file.
    fn elevate(&mut self, exe: PathBuf, args: String) {
        let slot = Arc::new(Mutex::new(None));
        self.job = Some(slot.clone());
        self.plan_msg = "In attesa dell'autorizzazione di amministratore...".into();
        thread::spawn(move || {
            let _ = std::fs::create_dir_all(engine::state_dir());
            let out = engine::state_dir().join(format!("esito-{}.txt", std::process::id()));
            let _ = std::fs::remove_file(&out);
            let text = match install::run_elevated(&exe, &format!("{args} --result \"{}\"", out.display())) {
                Ok(code) => std::fs::read_to_string(&out).unwrap_or_else(|_| if code == 0 { "OK Fatto".into() } else { format!("ERR Codice di uscita {code}") }),
                Err(e) => format!("ERR {e}"),
            };
            let _ = std::fs::remove_file(&out);
            *slot.lock().unwrap() = Some(text);
        });
    }
    fn poll_job(&mut self, ctx: &egui::Context) {
        let Some(slot) = &self.job else { return };
        let done = slot.lock().unwrap().take();
        match done {
            None => ctx.request_repaint_after(Duration::from_millis(300)),
            Some(text) => {
                self.job = None;
                self.plan_msg = text.strip_prefix("OK ").map_or_else(|| text.strip_prefix("ERR ").unwrap_or(&text).to_string(), |m| format!("✓ {m}"));
                if text.starts_with("OK") { diag::ok(&self.plan_msg) } else { diag::warn(&self.plan_msg) }
                self.cfg = sched::load_config();
                // Dopo l'installazione l'icona parte subito, senza attendere il prossimo accesso
                // (non elevata: è questa GUI a lanciarla; un'istanza già attiva esce da sola).
                if text.starts_with("OK") && install::is_installed() {
                    let _ = std::process::Command::new(install::installed_exe()).arg("--tray").spawn();
                }
            }
        }
    }
    /// Configurazione dalla bozza (date normalizzate AAAA-MM-GG).
    fn draft_config(&self) -> Result<sched::Config, String> {
        let plan = if self.plan_on {
            let mut p = self.draft.clone();
            p.validate()?;
            p.start = sched::parse_date(&p.start).map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or(p.start);
            p.end = if p.end.trim().is_empty() || p.repeat == sched::Repeat::Once { String::new() } else { sched::parse_date(&p.end).map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or(p.end) };
            Some(p)
        } else {
            None
        };
        Ok(sched::Config { plan, update: self.update_mode })
    }
    fn plan_actions(&mut self, ui: &mut egui::Ui, h: f32) {
        let installed = install::is_installed();
        let draft = self.draft_config();
        let changed = draft.as_ref().is_ok_and(|c| *c != self.cfg);
        let hint = match (&draft, installed) {
            (_, false) => "Installa TracciaDoc per attivare la pianificazione".to_string(),
            (Err(e), _) => e.clone(),
            (Ok(c), _) => match &c.plan { Some(p) => format!("Scansione {}", p.summary()), None => "Nessuna scansione pianificata".into() },
        };
        ui.add(egui::Label::new(RichText::new(hint).weak()).truncate());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let ready = installed && changed && self.job.is_none();
            if ui.add_enabled(ready, primary("Salva pianificazione", h)).clicked()
                && let Ok(cfg) = draft
            {
                let path = engine::state_dir().join("pianificazione-da-applicare.json");
                let _ = std::fs::create_dir_all(engine::state_dir());
                match std::fs::write(&path, serde_json::to_vec_pretty(&cfg).unwrap_or_default()) {
                    Ok(()) => {
                        let exe = std::env::current_exe().unwrap_or_else(|_| install::installed_exe());
                        self.elevate(exe, format!("--apply-config \"{}\"", path.display()));
                    }
                    Err(e) => self.plan_msg = format!("Configurazione non scritta: {e}"),
                }
            }
            if ui.add(egui::Button::new("Torna alla verifica").min_size(egui::vec2(0., h))).clicked() {
                self.plan_page = false;
            }
        });
    }
    fn plan_ui(&mut self, ui: &mut egui::Ui) {
        heading(ui, "Quando scansioniamo?", "La scansione parte da sola all'orario scelto, anche senza utente collegato, e salva il verbale. Non cancella mai nulla.");
        ui.add_space(12.);
        if !self.plan_msg.is_empty() {
            let ok = self.plan_msg.starts_with('✓');
            ui.label(RichText::new(&self.plan_msg).color(if ok { c_ok() } else { c_warn() }));
            ui.add_space(6.);
        }
        let busy = self.job.is_some();
        card(ui, |ui| {
            label_dim(ui, "INSTALLAZIONE");
            if install::is_installed() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("✓ Installato su questa postazione").font(semibold(17.)).color(c_ok()));
                    ui.label(RichText::new(diag::shown(&install::install_dir())).small().weak());
                    if ui.add_enabled(!busy, egui::Button::new("Disinstalla...")).on_hover_text("Toglie programma, icona e attività pianificate. Verbali e storico restano.").clicked() {
                        self.elevate(install::installed_exe(), "--uninstall".into());
                    }
                });
                if !install::running_installed() {
                    ui.label(RichText::new("Stai usando una copia portatile: le scansioni pianificate usano la copia installata. \"Reinstalla\" la aggiorna con questa versione.").small().weak());
                    if ui.add_enabled(!busy, egui::Button::new("Reinstalla con questa versione")).clicked() {
                        self.elevate(std::env::current_exe().unwrap_or_default(), "--install".into());
                    }
                }
            } else {
                ui.label("Per pianificare le scansioni TracciaDoc va installato su questa postazione (Program Files, serve l'amministratore). Aggiunge l'icona vicino all'orologio, avviata a ogni accesso.");
                if ui.add_enabled(!busy, primary("Installa su questa postazione", 42.)).clicked() {
                    self.elevate(std::env::current_exe().unwrap_or_default(), "--install".into());
                }
            }
        });
        ui.add_space(8.);
        let wide_layout = ui.available_width() >= 900.;
        let left_w = if wide_layout { ui.available_width() * 0.6 } else { ui.available_width() };
        let body = |ui: &mut egui::Ui, this: &mut Self| {
            card(ui, |ui| {
                ui.checkbox(&mut this.plan_on, RichText::new("Scansioni pianificate attive").font(semibold(17.)));
                if !this.plan_on {
                    ui.label(RichText::new("Nessuna scansione automatica: TracciaDoc parte solo a mano.").weak());
                    return;
                }
                let d = &mut this.draft;
                ui.add_space(4.);
                label_dim(ui, "RIPETIZIONE");
                ui.horizontal_wrapped(|ui| {
                    for r in [sched::Repeat::Once, sched::Repeat::Daily, sched::Repeat::Weekly, sched::Repeat::Monthly] {
                        if pill(ui, r.label(), d.repeat == r).clicked() {
                            d.repeat = r;
                        }
                    }
                });
                if d.repeat == sched::Repeat::Weekly {
                    label_dim(ui, "GIORNI");
                    ui.horizontal_wrapped(|ui| {
                        for (i, name) in sched::DAYS.iter().enumerate() {
                            if pill(ui, name, d.days[i]).clicked() {
                                d.days[i] = !d.days[i];
                            }
                        }
                    });
                }
                label_dim(ui, "ORARIO E DATE");
                ui.horizontal_wrapped(|ui| {
                    if d.repeat == sched::Repeat::Monthly {
                        ui.label("giorno");
                        ui.add(egui::DragValue::new(&mut d.day_of_month).range(1..=31));
                    }
                    ui.label("alle");
                    ui.add(egui::DragValue::new(&mut d.hour).range(0..=23).custom_formatter(|v, _| format!("{v:02}")));
                    ui.label(":");
                    ui.add(egui::DragValue::new(&mut d.minute).range(0..=59).speed(0.3).custom_formatter(|v, _| format!("{v:02}")));
                    ui.add_space(12.);
                    ui.label(if d.repeat == sched::Repeat::Once { "il" } else { "dal" });
                    ui.add(egui::TextEdit::singleline(&mut d.start).hint_text("GG/MM/AAAA").desired_width(110.));
                    if d.repeat != sched::Repeat::Once {
                        ui.label("al");
                        ui.add(egui::TextEdit::singleline(&mut d.end).hint_text("senza scadenza").desired_width(130.));
                    }
                });
                ui.add_space(4.);
                ui.checkbox(&mut d.shutdown, RichText::new("Arresta il PC a fine scansione").font(semibold(16.)));
                ui.label(RichText::new("Solo dopo aver salvato il verbale (anche se la verifica risulta incompleta), mai se la scansione viene interrotta. Avviso di 60 secondi, annullabile dall'icona; i programmi aperti non vengono chiusi a forza.").small().weak());
                ui.label(RichText::new("Se il PC è spento all'orario, la scansione parte appena viene acceso. Parte anche senza utente collegato (account di sistema), a priorità bassa, su tutti i dischi fissi.").small().weak());
                if let Err(e) = d.validate() {
                    ui.label(RichText::new(e).color(c_warn()));
                }
            });
            ui.add_space(8.);
            card(ui, |ui| {
                label_dim(ui, "AGGIORNAMENTI");
                ui.horizontal_wrapped(|ui| {
                    for m in [sched::UpdateMode::Auto, sched::UpdateMode::Notify, sched::UpdateMode::Off] {
                        if pill(ui, m.label(), this.update_mode == m).clicked() {
                            this.update_mode = m;
                        }
                    }
                });
                ui.label(RichText::new(match this.update_mode {
                    sched::UpdateMode::Auto => "Controlla le release pubbliche ogni giorno, verifica lo SHA-256 e installa la nuova versione quando non c'è una scansione in corso.",
                    sched::UpdateMode::Notify => "Mostra solo l'avviso (pallino blu sull'icona): l'aggiornamento si avvia dal menu dell'icona.",
                    sched::UpdateMode::Off => "Nessun controllo degli aggiornamenti.",
                }).small().weak());
                let note = sched::load_status().update_note;
                if !note.is_empty() {
                    ui.label(RichText::new(format!("Ultimo controllo: {note}")).small().weak());
                }
            });
        };
        let side = |ui: &mut egui::Ui, this: &Self| {
            card(ui, |ui| {
                label_dim(ui, "PROSSIME ESECUZIONI");
                let next = if this.plan_on { this.draft.next_runs(chrono::Local::now().naive_local(), 3) } else { vec![] };
                if next.is_empty() {
                    ui.label(RichText::new("nessuna").weak());
                }
                for t in next {
                    ui.label(sched::when_text(t));
                }
                if this.draft_config().ok() != Some(this.cfg.clone()) {
                    ui.label(RichText::new("Modifiche non ancora salvate").small().color(c_warn()));
                }
            });
            ui.add_space(8.);
            card(ui, |ui| {
                label_dim(ui, "ULTIME ESECUZIONI");
                let history = sched::load_status().history;
                if history.is_empty() {
                    ui.label(RichText::new("ancora nessuna").weak());
                }
                for e in history.iter().take(6) {
                    let (text, color) = match e.outcome.as_str() {
                        "clean" => ("✓ nessun documento".to_string(), c_ok()),
                        "found" => (format!("⚠ {} documenti trovati", e.findings), c_warn()),
                        "incomplete" => ("⚠ incompleta".to_string(), c_warn()),
                        _ => (format!("✗ non riuscita {}", e.note), c_err()),
                    };
                    ui.horizontal(|ui| {
                        ui.label(&e.when);
                        ui.label(RichText::new(text).color(color));
                    });
                }
                if ui.link("Apri cartella verbali").clicked() {
                    let dir = sched::verbali_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::process::Command::new(PathBuf::from(std::env::var_os("WINDIR").unwrap_or_default()).join("explorer.exe")).arg(dir).spawn();
                }
            });
        };
        if wide_layout {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(left_w - 12.);
                    body(ui, self);
                });
                ui.vertical(|ui| side(ui, self));
            });
        } else {
            body(ui, self);
            ui.add_space(8.);
            side(ui, self);
        }
    }
}
fn card<R>(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui) -> R) {
    let width = ui.available_width();
    egui::Frame::new()
        .fill(c_surface())
        .stroke(egui::Stroke::new(1., c_line()))
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
        let just_finished = self.handle.as_ref().is_some_and(|h| h.is_finished());
        if just_finished && self.handle.take().unwrap().join().is_err() {
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
        if just_finished {
            self.scan_lock = None;
            self.after_scan();
        }
        self.poll_job(&ctx);
        if let Some(at) = self.shutdown_at {
            if Instant::now() >= at {
                self.shutdown_at = None;
                match sched::run_tool("shutdown.exe", &["/s", "/t", "0", "/d", "p:0:0"]) {
                    Ok(_) => diag::info("Spegnimento richiesto a fine scansione"),
                    Err(e) => diag::err(format!("Spegnimento non riuscito: {e}")),
                }
            } else {
                ctx.request_repaint_after(Duration::from_millis(500));
            }
        }
        if !self.zoom_set {
            self.zoom_set = true;
            // Schermi grandi a risoluzione nativa: UI troppo piccola, parte ingrandita (resta regolabile dal menu).
            let w = ctx.content_rect().width();
            let z = if w >= 2400. { 1.5 } else if w >= 1900. { 1.25 } else { 1. };
            if z > 1. {
                self.zoom = z;
                ctx.set_zoom_factor(z);
            }
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
        if running && rate > 0. && self.rate_at.elapsed() >= Duration::from_secs(1) {
            self.rate_at = Instant::now();
            self.rate_hist.push(rate as f32);
            if self.rate_hist.len() > 90 {
                self.rate_hist.remove(0);
            }
        }
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
            .frame(egui::Frame::new().fill(c_console()).inner_margin(egui::Margin::symmetric(16, 8)))
            .show(root, |ui| {
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 30.), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let arrow = if self.console_open { "▾" } else { "▸" };
                    if ui.add(egui::Button::new(RichText::new(format!("{arrow} Console")).font(semibold(15.))).frame(false)).clicked() {
                        self.console_open = !self.console_open;
                    }
                    let doing = if running { format!("{phase}  {}", diag::mask(&current)) } else { "In attesa".to_string() };
                    ui.add(egui::Label::new(RichText::new(doing).small().weak()).truncate());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new("Copia diagnostica").small().color(c_accent())).frame(false)).on_hover_text("Copia negli appunti un report per gli sviluppatori: nessun contenuto dei documenti, nome utente mascherato").clicked(){
                            let text=diag::report(&self.gpu,&phase,&current,run.as_ref());
                            let kb=text.len().div_ceil(1024);
                            // Scrittura diretta negli appunti di Windows; se fallisce ripiega su egui.
                            let direct=diag::copy_to_clipboard(&text);
                            if !direct {ctx.copy_text(text);}
                            diag::info(format!("Diagnostica copiata negli appunti ({kb} KB{})",if direct{""}else{", via egui"}));
                            self.copied=Some((format!("Diagnostica copiata negli appunti ({kb} KB): incollala nella chat"),Instant::now()));
                        }
                        if ui.add(egui::Button::new(RichText::new(if light() { "Tema scuro" } else { "Tema chiaro" }).small().color(c_accent())).frame(false)).clicked() {
                            LIGHT.store(!light(), Ordering::Relaxed);
                            apply_style(&ctx);
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
                            ui.add(egui::Label::new(RichText::new(msg).small().color(c_ok())).truncate());
                        }
                    });
                });
                if self.console_open {
                    egui::ScrollArea::vertical().id_salt("console-scroll").min_scrolled_height(if short { 60. } else { 132. }).max_height(if short { 60. } else { 132. }).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.;
                        for e in diag::recent(300) {
                            let color = match e.level {
                                diag::Level::Info => c_weak(),
                                diag::Level::Ok => c_ok(),
                                diag::Level::Warn => c_warn(),
                                diag::Level::Err => c_err(),
                            };
                            ui.add(egui::Label::new(RichText::new(e.line()).monospace().size(13.).color(color)).wrap());
                        }
                    });
                }
            });

        // Barra laterale: marchio, passi, indicatore, accelerazioni, firma.
        egui::Panel::left("steps")
            .exact_size(220.)
            .frame(egui::Frame::new().fill(c_side()).inner_margin(16))
            .show(root, |ui| {
                let mut content = |ui: &mut egui::Ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Image::from_texture(&self.mark).fit_to_exact_size(egui::vec2(34., 34.)).tint(logo_tint()));
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.;
                        ui.label(RichText::new("TracciaDoc").font(semibold(22.)));
                        ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).small().weak());
                    });
                });
                ui.add_space(14.);
                ui.spacing_mut().item_spacing.y = 4.;
                for (n, label) in [(1, "Aree"), (2, "Scansione"), (3, "Revisione"), (4, "Verifica finale")] {
                    step_row(ui, n, label, if self.plan_page && !running { 0 } else { active });
                }
                ui.add_space(10.);
                ui.label(RichText::new("AUTOMAZIONE").size(11.5).color(c_dim()));
                if plan_row(ui, self.plan_page && !running, running).clicked() && !running {
                    self.plan_page = !self.plan_page;
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
                ui.label(RichText::new("ACCELERAZIONI").size(11.5).color(c_dim()));
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
                        chip(ui, name, if on { c_accent() } else { c_dim() }, on);
                        if i % 3 == 2 { ui.end_row(); }
                    }
                });
                };
                // Schermo basso o UI ingrandita: la barra scorre invece di tagliare le accelerazioni; la firma resta solo se c'e posto.
                if short {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| content(ui));
                } else {
                    content(ui);
                // Firma in basso.
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    if !short && ui.available_height() >= 28. {
                        let ratio = self.signature.size()[1] as f32 / self.signature.size()[0] as f32;
                        ui.add(egui::Image::from_texture(&self.signature).fit_to_exact_size(egui::vec2(112., 112. * ratio)).tint(logo_tint())).on_hover_text("powered by realhcok");
                        ui.label(RichText::new("powered by").size(12.).weak());
                    }
                });
                }
            });

        // Barra delle azioni: una sola azione principale per fase, in basso a destra.
        egui::Panel::bottom("actions")
            .frame(egui::Frame::new().fill(c_bar()).inner_margin(egui::Margin::symmetric(28, if short { 8 } else { 14 })))
            .show(root, |ui| {
                let h = if short { 40. } else { 48. };
                ui.horizontal(|ui| {
                    ui.set_min_height(h);
                    if self.plan_page && !running {
                        self.plan_actions(ui, h);
                        return;
                    }
                    match active {
                        1 => {
                            let ready = !self.roots.is_empty() && !self.operator.trim().is_empty();
                            let hint = if self.roots.is_empty() { "Seleziona almeno un disco o una cartella".to_string() } else if self.operator.trim().is_empty() { "Inserisci il nome dell'operatore".to_string() } else {
                                let used: u64 = self.roots.iter().filter_map(|p| self.space.get(p)).map(|(t, f)| t.saturating_sub(*f)).sum();
                                if used > 0 { format!("{}  ·  {} occupati", plural(self.roots.len(), "area selezionata", "aree selezionate"), it_size(used)) } else { plural(self.roots.len(), "area selezionata", "aree selezionate") }
                            };
                            let button_w = if ui.available_width() < 560. { 170. } else { 220. };
                            let left_w = (ui.available_width() - button_w - 16.).max(60.);
                            ui.allocate_ui_with_layout(egui::vec2(left_w, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.spacing_mut().item_spacing.y = 2.;
                                if self.ocr_present {
                                    chip(ui, "OCR offline pronto", c_accent(), true);
                                } else {
                                    chip(ui, "OCR NON DISPONIBILE", c_warn(), true);
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
                                ui.add(egui::Label::new(RichText::new(format!("Eliminare in modo definitivo {selected} file? Non vanno nel cestino e TracciaDoc non puo recuperarli.")).color(c_warn())).truncate());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(RichText::new("Conferma eliminazione").font(semibold(18.)).color(Color32::WHITE)).fill(c_danger()).min_size(egui::vec2(220., h))).clicked() {
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
                                if open > 0 {
                                    let mut all = selected == open;
                                    if ui.add(egui::Checkbox::new(&mut all, "Seleziona tutti")).changed() {
                                        for f in run.findings.iter_mut().filter(|f| !f.deleted()) { f.selected = all; }
                                    }
                                    ui.add(egui::Label::new(RichText::new(plural(selected, "selezionato", "selezionati")).weak()).truncate());
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let resolved = open == 0 || run.deleted_count() > 0 && selected == 0;
                                    if run.deleted_count() > 0 && selected == 0 && ui.add(primary("Ri-scansiona per la verifica finale", h)).clicked() {
                                        rescan = Some(run.summary());
                                    }
                                    if selected > 0 && ui.add(egui::Button::new(RichText::new(format!("Elimina definitivamente ({selected})")).font(semibold(18.)).color(Color32::WHITE)).fill(c_danger()).min_size(egui::vec2(0., h))).clicked() {
                                        self.confirm = true;
                                    }
                                    if run.finished {
                                        let btn = if resolved && run.deleted_count() == 0 || active == 4 { primary("Genera verbale PDF...", h) } else { egui::Button::new("Genera verbale PDF...").min_size(egui::vec2(0., h)) };
                                        if ui.add(btn).clicked() {
                                            match crate::shared::genera_e_salva(run,self.include_paths,self.demo,None) {
                                                Ok((pdf,esito))=>{let m=crate::shared::messaggio(&pdf,&esito);if matches!(esito,crate::shared::Esito::SoloLocale(_)){diag::warn(m.clone())}else{diag::ok(m.clone())};self.notice=m;self.last_pdf=Some(pdf);let _=engine::audit(run,"pdf-export");},
                                                Err(e)=>{diag::err(format!("Verbale PDF non scritto: {e}"));self.notice=e}
                                            }
                                        }
                                        if let Some(src)=self.last_pdf.clone() && ui.add(egui::Button::new("Salva una copia con nome...").min_size(egui::vec2(0., h))).clicked()
                                            && let Some(path)=rfd::FileDialog::new().set_parent(&*frame).add_filter("Verbale PDF",&["pdf"]).set_file_name(src.file_name().map(|n|n.to_string_lossy().into_owned()).unwrap_or_default()).save_file() {
                                                self.notice=match std::fs::copy(&src,&path) {Ok(_)=>format!("Copia salvata: {}",diag::shown(&path)),Err(e)=>format!("Copia non salvata: {e}")};
                                            }
                                    }
                                });
                            }
                        }
                    }
                });
            });

        egui::CentralPanel::default().frame(egui::Frame::new().fill(c_bg()).inner_margin(egui::Margin::symmetric(36, 28))).show(root, |ui| {
            // Barra di scorrimento fissa: quella a comparsa copriva i bordi destri di schede e pulsanti.
            ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // set_max_width(1100) da solo allargava l'ui oltre il pannello nelle finestre strette.
                ui.set_max_width(ui.available_width().min(1700.));
                if let Some(at) = self.shutdown_at {
                    card(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            let left = at.saturating_duration_since(Instant::now()).as_secs();
                            ui.label(RichText::new(format!("Il PC si spegne tra {left} secondi: verbale salvato, nessun documento trovato.")).font(semibold(17.)).color(c_warn()));
                            if ui.button("Annulla spegnimento").clicked() {
                                self.shutdown_at = None;
                                diag::info("Spegnimento annullato dall'operatore");
                            }
                        });
                    });
                    ui.add_space(8.);
                }
                if !self.notice.is_empty() {
                    ui.label(RichText::new(&self.notice).color(c_warn()));
                }
                if self.plan_page && !running {
                    self.plan_ui(ui);
                    return;
                }
                match active {
                    1 => {
                        if let Some(run) = self.pending_review.clone() {
                            card(ui, |ui| {
                                ui.label(RichText::new(format!("La scansione pianificata del {} ha trovato {} possibili documenti.", run.start.get(..16).unwrap_or(&run.start).replace('T', " "), run.remaining())).font(semibold(18.)).color(c_warn()));
                                ui.label(RichText::new("Non è stato cancellato nulla. Rivedi i file, elimina quelli da togliere e ri-scansiona per il verbale finale.").weak());
                                ui.horizontal(|ui| {
                                    if ui.add(primary("Apri la revisione", 40.)).clicked() {
                                        sched::mark_seen(&run.id);
                                        self.shared.lock().unwrap().run = Some(run.clone());
                                        self.pending_review = None;
                                    }
                                    if ui.button("Più tardi").clicked() {
                                        self.pending_review = None;
                                    }
                                });
                            });
                            ui.add_space(10.);
                        }
                        heading(ui, "Dove cerchiamo?", "Scegli i dischi o le cartelle da controllare. Tutto resta su questo computer, in sola lettura.");
                        ui.add_space(12.);
                        ui.label(RichText::new("AREE").size(12.).color(c_dim()));
                        // Griglia a colonne fisse: le schede vanno a capo invece di uscire dalla finestra. Rete esclusa: add() la rifiuterebbe.
                        let mut areas: Vec<(PathBuf, String, String)> = vec![];
                        for (path, label) in &self.volumes {
                            if label.contains("rete") || areas.iter().any(|(p, _, _)| p == path) { continue; }
                            let text = path.to_string_lossy();
                            let kind = label.rsplit("  -  ").next().unwrap_or("");
                            let name = if text.starts_with("\\\\?\\Volume") { "Volume".to_string() } else { text.trim_end_matches('\\').to_string() };
                            let kind = match kind { "locale fisso" => "Disco fisso".to_string(), "rimovibile" => "Rimovibile".to_string(), other => other.to_string() };
                            areas.push((path.clone(), name, kind));
                        }
                        let usable = ui.available_width().max(240.);
                        let columns = ((usable + 12.) / 332.).floor().max(1.) as usize;
                        let card_w = ((usable - 12. * (columns as f32 - 1.)) / columns as f32).max(200.);
                        egui::Grid::new("areas").spacing([12., 12.]).show(ui, |ui| {
                            for (n, (path, label, kind)) in areas.into_iter().enumerate() {
                                let on = self.roots.contains(&path);
                                let mut v = on;
                                egui::Frame::new()
                                    .fill(if on { c_sel_soft() } else { c_surface() })
                                    .stroke(egui::Stroke::new(1., if on { c_accent() } else { c_line() }))
                                    .corner_radius(10)
                                    .inner_margin(egui::Margin::symmetric(16, 12))
                                    .show(ui, |ui| ui.vertical(|ui| {
                                        ui.set_width(card_w - 34.);
                                        ui.set_max_width(card_w - 34.);
                                        ui.checkbox(&mut v, RichText::new(label).font(semibold(if card_w < 340. { 26. } else { 34. })));
                                        ui.label(RichText::new(kind).size(15.).weak());
                                        if let Some((total, free)) = self.space.get(&path) {
                                            let used = total.saturating_sub(*free);
                                            let frac = used as f32 / *total as f32;
                                            ui.label(RichText::new(format!("{}  ·  {} occupati", it_size(*total), it_size(used))).size(14.).weak());
                                            bar(ui, frac, 8., if on { c_accent() } else { c_weak() });
                                            ui.label(RichText::new(format!("occupato {:.0} %", frac * 100.)).monospace().size(12.).weak());
                                        }
                                    }))
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
                            ui.label(RichText::new("COSA CERCA").size(12.).color(c_dim()));
                            ui.horizontal_wrapped(|ui| {
                                for t in ["Carta d'identita", "Patente di guida", "Tessera sanitaria", "Passaporto", "Fototessera"] {
                                    ui.label(RichText::new("●").size(10.).color(c_accent()));
                                    ui.label(RichText::new(t).size(16.));
                                    ui.add_space(14.);
                                }
                            });
                            ui.label(RichText::new("Le fototessere sono segnalate come indizio da verificare (volto frontale unico su sfondo uniforme). Nomi o codici fiscali isolati non vengono cercati. Cerca in immagini, PDF, RTF e Office con immagini incorporate.").small().weak());
                        });
                        ui.add_space(6.);
                        card(ui, |ui| {
                            ui.label(RichText::new("DATI DEL VERBALE").size(12.).color(c_dim()));
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Operatore");
                                ui.add(egui::TextEdit::singleline(&mut self.operator).hint_text("nome e cognome").desired_width(220.));
                                ui.label("Riferimento");
                                ui.add(egui::TextEdit::singleline(&mut self.reference).hint_text("facoltativo").desired_width(260.));
                            });
                            ui.checkbox(&mut self.shutdown_after, "Arresta il PC al termine se non trova documenti (salva prima il verbale, avviso di 60 secondi)");
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
                            ui.checkbox(&mut self.settings.photo_id,"Fototessere: cerca anche le foto formato tessera (rilevatore di volti offline, indizio da verificare)");
                            ui.checkbox(&mut self.settings.prefilter,"Prefiltro: salta l'OCR delle immagini senza tratti tipo testo (dichiarate nel verbale come scartate)");
                            ui.label(format!("Disponibile: CPU. Automatico: {} rilevato a runtime; Solo CPU: prodotto scalare generico per confronto. GPU/NPU non implementate. Nessun requisito AVX globale; AVX-512 non selezionata.",engine::cpu_method(false)));
                            ui.horizontal(|ui|{ui.label("MiB/file");ui.add(egui::DragValue::new(&mut self.settings.max_mb).range(1..=256));ui.label("Pagine/immagini");ui.add(egui::DragValue::new(&mut self.settings.max_pages).range(1..=500));});
                            ui.horizontal(|ui|{ui.label("Secondi/file");ui.add(egui::DragValue::new(&mut self.settings.timeout_secs).range(1..=1800));ui.label("MiB worker");ui.add(egui::DragValue::new(&mut self.settings.memory_mb).range(256..=2048));});
                            ui.horizontal(|ui|{ui.label("Limite file");ui.add(egui::DragValue::new(&mut self.settings.max_entries).range(1..=10_000_000));});
                            ui.horizontal(|ui|{ui.label("Worker OCR in parallelo");ui.add(egui::DragValue::new(&mut self.settings.workers).range(1..=engine::physical_cores().max(1)));ui.label(format!("(core fisici: {}; ogni worker usa fino a {} MiB)",engine::physical_cores(),self.settings.memory_mb));});
                            ui.label("Concorrenza: ogni worker e un processo separato con un thread OCR e il proprio limite di memoria. Su HDD lenti ridurre a 1. I file con contenuto identico a uno gia analizzato in questa sessione riusano lo stesso esito (SHA-256); nessuna cache e nessun temporaneo OCR su disco. Limiti di decodifica e Office descritti nel verbale.");
                            ui.label(format!("Storico essenziale: {}. Solo conteggi e limiti, senza contenuti o percorsi. ACL ereditate dal profilo; nessuna protezione contro amministratori.",engine::state_dir().display()));
                        });
                    }
                    2 => {
                        heading(ui, "Scansione in corso", "Puoi lasciare il computer cosi: alla fine ti mostro cosa ho trovato.");
                        ui.add_space(14.);
                        let findings = run.as_ref().map_or(0, |r| r.findings.len());
                        let secs = elapsed as u64;
                        let elapsed_text = format!("{:02}:{:02}", secs / 60, secs % 60);
                        let wide = ui.available_width() >= 900.;
                        let ring_size = if wide { 260. } else { 200. };
                        ui.horizontal_top(|ui| {
                            egui::Frame::new().fill(c_surface()).stroke(egui::Stroke::new(1., c_line())).corner_radius(10).inner_margin(16).show(ui, |ui| {
                                ui.set_width(ring_size - 32.);
                                ui.vertical_centered(|ui| {
                                    ring(ui, ring_size - 32., frac, &ring_text);
                                    ui.label(RichText::new(files_text.clone()).monospace().size(13.).weak());
                                    if total_final && rate > 0.5 && done >= 30 && elapsed >= 10. {
                                        ui.label(RichText::new(format!("{} rimanenti", eta_text)).size(14.).color(c_accent()));
                                    }
                                });
                            });
                            ui.vertical(|ui| {
                                let w = ((ui.available_width() - 36.) / 4.).max(110.);
                                ui.horizontal(|ui| {
                                    tile(ui, w, "ANALIZZATI", it_num(done), None);
                                    tile(ui, w, "SOSPETTI", findings.to_string(), if findings > 0 { Some(c_warn()) } else { None });
                                    tile(ui, w, "VELOCITA", rate_text.clone(), None);
                                    tile(ui, w, "TRASCORSO", elapsed_text, None);
                                });
                                card(ui, |ui| {
                                    ui.label(RichText::new("FASI").size(12.).color(c_dim()));
                                    for (name, f, note) in [
                                        ("Elenco file", if total_final { 1. } else { 0.4 }, format!("{}{}", it_num(total), if total_final { "" } else { "+" })),
                                        ("Analisi testo e OCR", frac, format!("{} / {}", it_num(done), it_num(total))),
                                    ] {
                                        ui.horizontal(|ui| {
                                            ui.add_sized([170., 18.], egui::Label::new(RichText::new(name).size(15.)));
                                            ui.allocate_ui(egui::vec2((ui.available_width() - 150.).max(80.), 18.), |ui| { ui.centered_and_justified(|ui| bar(ui, f, 10., c_accent())); });
                                            ui.label(RichText::new(note).monospace().size(12.5).weak());
                                        });
                                    }
                                    if let Some(run) = &run {
                                        if run.timing.prefilter_skipped > 0 { ui.label(RichText::new(format!("Immagini scartate dal prefiltro (senza OCR): {}", it_num(run.timing.prefilter_skipped))).small().weak()); }
                                        if run.pages_ocr > 0 { ui.label(RichText::new(format!("Pagine e immagini passate all'OCR: {}", it_num(run.pages_ocr))).small().weak()); }
                                    }
                                });
                                card(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("FILE AL SECONDO").size(12.).color(c_dim()));
                                        if let Some(peak) = self.rate_hist.iter().cloned().reduce(f32::max) {
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(RichText::new(format!("picco {}", it_num(peak as u64))).monospace().size(12.).weak()); });
                                        }
                                    });
                                    spark(ui, &self.rate_hist, 70.);
                                });
                            });
                        });
                        ui.add_space(10.);
                        card(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Niente ui.spinner(): chiede un ridisegno a ogni frame; l'indicatore avanza col ridisegno periodico.
                                let tick = self.started.map(|s| s.elapsed().as_millis() / self.repaint.as_millis()).unwrap_or(0) as usize;
                                ui.label(RichText::new(["◐","◓","◑","◒"][tick % 4]).color(c_accent()));
                                ui.label(RichText::new("ORA IN ESAME").size(12.).color(c_dim()));
                                ui.add(egui::Label::new(RichText::new(diag::mask(&current)).monospace().size(13.).weak()).truncate());
                            });
                        });
                        if let Some(run) = &run {
                            ui.collapsing("Dettagli", |ui| {
                                ui.label(format!("Incontrati {}    Analizzati {}    Parziali {}    Saltati {}    Ignorati per tipo {}    Tempo {:.1}s",run.encountered,run.analyzed,run.partial_files,run.skipped,run.ignored_type,elapsed));
                                if run.timing.prefilter_skipped>0 {ui.label(format!("Immagini scartate dal prefiltro (senza OCR): {}",run.timing.prefilter_skipped));}
                                if run.timing.reused>0 {ui.label(format!("File con esito riusato (contenuto identico gia analizzato): {}",run.timing.reused));}
                            });
                        }
                    }
                    _ => {
                        if let Some(run) = run.as_mut() {
                            let remaining = run.remaining();
                            let title = if active == 4 { "Verifica finale".to_string() } else if remaining == 0 && run.deleted_count() == 0 { "Nessun file da rivedere".to_string() } else if remaining == 0 { "Tutto eliminato".to_string() } else if remaining == 1 { "1 file da rivedere".to_string() } else { format!("{remaining} file da rivedere") };
                            ui.label(RichText::new(title).font(semibold(32.)));
                            ui.label(RichText::new(run.outcome()).font(semibold(17.)).color(if run.incomplete() { c_warn() } else { c_accent() }));
                            if active == 4 && remaining == 0 {
                                // Verifica finale: confronto prima/dopo e anteprima del verbale.
                                let before = run.previous.as_ref().map_or(0, |p| p.findings);
                                let after = run.findings.len() as u64;
                                let date = run.start.get(..10).map(|d| d.split('-').rev().collect::<Vec<_>>().join("/")).unwrap_or_default();
                                let wide = ui.available_width() >= 980.;
                                ui.add_space(12.);
                                ui.horizontal_top(|ui| {
                                    let left = if wide { ui.available_width() - 404. } else { ui.available_width() };
                                    ui.vertical(|ui| {
                                        ui.set_width(left);
                                        let w = ((left - 70.) / 2.).max(120.);
                                        ui.horizontal(|ui| {
                                            for (label, value, color, hot) in [("PRIMA", before, c_warn(), false), ("DOPO", after, c_accent(), true)] {
                                                egui::Frame::new().fill(if hot { c_sel_soft() } else { c_surface() }).stroke(egui::Stroke::new(1., if hot { c_accent() } else { c_line() })).corner_radius(12).inner_margin(18).show(ui, |ui| {
                                                    ui.set_width(w - 36.);
                                                    ui.label(RichText::new(label).size(12.).color(c_dim()));
                                                    ui.label(RichText::new(value.to_string()).font(semibold(64.)).color(color));
                                                    ui.label(RichText::new("file segnalati").weak());
                                                });
                                                if !hot { ui.label(RichText::new("→").size(30.).weak()); }
                                            }
                                        });
                                        ui.add_space(6.);
                                        card(ui, |ui| {
                                            ui.label(RichText::new("RIEPILOGO").size(12.).color(c_dim()));
                                            let mut rows = vec![("Eliminati definitivamente", run.previous.as_ref().map_or(0, |p| p.deleted).to_string())];
                                            if let Some(p) = &run.previous && p.failed > 0 { rows.push(("Eliminazioni non riuscite", p.failed.to_string())); }
                                            rows.push(("File analizzati", it_num(run.analyzed)));
                                            rows.push(("Durata della scansione", format!("{} min {:02} s", run.elapsed_secs as u64 / 60, run.elapsed_secs as u64 % 60)));
                                            rows.push(("Aree", plural(run.roots.len(), "area", "aree")));
                                            for (label, value) in rows {
                                                ui.separator();
                                                stat(ui, label, value);
                                            }
                                        });
                                    });
                                    if wide {
                                        ui.add_space(12.);
                                        paper(ui, 380., before, after, &date);
                                    }
                                });
                            } else {
                            if remaining > 0 { ui.label(RichText::new("Controlla le anteprime e spunta i file da eliminare.").weak()); }
                            ui.add_space(10.);
                            // Filtri per tipo di documento.
                            let kinds = ["Carta d'identita", "Passaporto", "Patente", "Tessera sanitaria", "Fototessera", "Da verificare"];
                            let count_of = |k: &str| run.findings.iter().filter(|f| !f.deleted() && kind_of(f).0 == k).count();
                            if !self.filter.is_empty() && count_of(&self.filter) == 0 { self.filter.clear(); }
                            ui.horizontal_wrapped(|ui| {
                                if pill(ui, &format!("Tutti {remaining}"), self.filter.is_empty()).clicked() { self.filter.clear(); }
                                for k in kinds {
                                    let n = count_of(k);
                                    if n > 0 && pill(ui, &format!("{k} {n}"), self.filter == k).clicked() { self.filter = k.to_string(); }
                                }
                            });
                            ui.add_space(6.);
                            let focus_path = self.focus.clone().filter(|p| run.findings.iter().any(|f| &f.path == p && !f.deleted() && (self.filter.is_empty() || kind_of(f).0 == self.filter))).or_else(|| run.findings.iter().find(|f| !f.deleted() && (self.filter.is_empty() || kind_of(f).0 == self.filter)).map(|f| f.path.clone()));
                            self.focus = focus_path.clone();
                            // Al massimo 2 decodifiche per frame, e solo per le righe vicine alla zona visibile.
                            let mut budget = 2;
                            let running_now = running;
                            let wide = ui.available_width() >= 1150.;
                            let panel_w = 400.;
                            ui.horizontal_top(|ui| {
                                let left = if wide { ui.available_width() - panel_w - 16. } else { ui.available_width() };
                                ui.vertical(|ui| {
                                    ui.set_width(left);
                                    for (i, f) in run.findings.iter_mut().enumerate().take(5000) {
                                        // I file eliminati escono dalla lista: restano nel conteggio, nella console e nel verbale.
                                        if f.deleted() { continue; }
                                        let (kind, color) = kind_of(f);
                                        if !self.filter.is_empty() && kind != self.filter { continue; }
                                        // Finestra stretta: l'anteprima resta solo se resta spazio per il testo.
                                        let side = if ui.available_width() >= 560. { 96. } else { 0. };
                                        if side > 0. && budget > 0 && !self.previews.contains_key(&f.path) && ui.clip_rect().expand(300.).contains(ui.cursor().min) {
                                            budget -= 1;
                                            let pv = make_preview(&ctx, &f.path, &f.format);
                                            self.previews.insert(f.path.clone(), pv);
                                            ctx.request_repaint();
                                        }
                                        let is_focus = focus_path.as_ref() == Some(&f.path);
                                        let (level, level_color, _) = level_of(f);
                                        egui::Frame::new()
                                            .fill(if f.selected || is_focus { c_sel_soft() } else { c_surface() })
                                            .stroke(egui::Stroke::new(1., if is_focus { c_accent() } else if f.selected { c_accent().gamma_multiply(0.6) } else { c_line() }))
                                            .corner_radius(10)
                                            .inner_margin(12)
                                            .show(ui, |ui| {
                                                ui.set_width(ui.available_width());
                                                ui.horizontal(|ui| {
                                                    ui.add_enabled(!running_now, egui::Checkbox::without_text(&mut f.selected)).on_hover_text("Da eliminare");
                                                    if side > 0. {
                                                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(side, 60.), egui::Sense::click());
                                                        if resp.clicked() { self.focus = Some(f.path.clone()); }
                                                        ui.painter().rect_filled(rect, 8., c_bg());
                                        ui.painter().rect_stroke(rect, 8., egui::Stroke::new(1., c_line()), egui::StrokeKind::Inside);
                                                        match self.previews.get(&f.path) {
                                                            Some(Some((blur, sharp))) => {
                                                                let tex = if resp.hovered() { sharp } else { blur };
                                                                egui::Image::from_texture(tex).fit_to_exact_size(rect.size()).paint_at(ui, rect);
                                                            }
                                                            _ => { ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, f.format.to_uppercase().split(' ').next().unwrap_or_default(), semibold(14.), c_weak()); }
                                                        }
                                                    }
                                                    let right_w = if wide { 130. } else { 200. };
                                                    let w = (ui.available_width() - right_w).max(100.);
                                                    ui.allocate_ui(egui::vec2(w, 0.), |ui| {
                                                        ui.set_width(w);
                                                        ui.vertical(|ui| {
                                                            ui.spacing_mut().item_spacing.y = 3.;
                                                            ui.horizontal(|ui| {
                                                                let name = f.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| f.path.display().to_string());
                                                                if ui.add(egui::Label::new(RichText::new(name).font(semibold(18.))).truncate().sense(egui::Sense::click())).clicked() { self.focus = Some(f.path.clone()); }
                                                                chip(ui, kind, color, true);
                                                            });
                                                            ui.add(egui::Label::new(RichText::new(diag::shown(&f.path)).monospace().size(12.).weak()).truncate());
                                                            if !wide && let Some(reason) = f.reasons.first() {
                                                                ui.add(egui::Label::new(RichText::new(reason).small()).truncate()).on_hover_text(f.reasons.join("\n"));
                                                            }
                                                            if !f.delete_error.is_empty() { ui.label(RichText::new(&f.delete_error).color(c_warn())); }
                                                        });
                                                    });
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        if wide {
                                                            ui.vertical(|ui| {
                                                                ui.label(RichText::new(level).font(semibold(15.)).color(level_color));
                                                                ui.label(RichText::new(plural(f.reasons.len(), "indizio", "indizi")).small().weak());
                                                            });
                                                        } else {
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
                                                        }
                                                    });
                                                });
                                            });
                                        ui.add_space(4.);
                                    }
                                });
                                if wide && let Some(fp) = &focus_path && let Some((i, f)) = run.findings.iter_mut().enumerate().find(|(_, f)| &f.path == fp) {
                                    ui.add_space(16.);
                                    egui::Frame::new().fill(c_surface()).stroke(egui::Stroke::new(1., c_line())).corner_radius(12).inner_margin(16).show(ui, |ui| ui.vertical(|ui| {
                                        ui.set_width(panel_w - 32.);
                                        ui.label(RichText::new("ANTEPRIMA PROTETTA").size(12.).color(c_dim()));
                                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(panel_w - 32., 200.), egui::Sense::hover());
                                        ui.painter().rect_filled(rect, 8., c_bg());
                                        ui.painter().rect_stroke(rect, 8., egui::Stroke::new(1., c_line()), egui::StrokeKind::Inside);
                                        match self.previews.get(&f.path) {
                                            Some(Some((blur, sharp))) => {
                                                let tex = if resp.hovered() { sharp } else { blur };
                                                let ts = tex.size_vec2();
                                                let scale = (rect.width() / ts.x).min(rect.height() / ts.y);
                                                egui::Image::from_texture(tex).paint_at(ui, egui::Rect::from_center_size(rect.center(), ts * scale));
                                                ui.label(RichText::new("passa sopra l'anteprima per vederla nitida").size(12.).color(c_dim()));
                                            }
                                            _ => { ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, format!("{}  ·  nessuna anteprima", f.format.to_uppercase()), semibold(14.), c_weak()); }
                                        }
                                        ui.label(RichText::new(f.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).font(semibold(19.)));
                                        ui.add(egui::Label::new(RichText::new(diag::shown(&f.path)).monospace().size(12.).weak()).wrap());
                                        let (level, level_color, frac) = level_of(f);
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(level).font(semibold(15.)).color(level_color));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(RichText::new(size_text(f.size)).small().weak()); });
                                        });
                                        bar(ui, frac, 7., level_color);
                                        ui.label(RichText::new("PERCHE E SEGNALATO").size(12.).color(c_dim()));
                                        for reason in &f.reasons {
                                            ui.horizontal_top(|ui| {
                                                ui.label(RichText::new("●").size(9.).color(c_accent()));
                                                ui.add(egui::Label::new(RichText::new(reason).size(14.5)).wrap());
                                            });
                                        }
                                        ui.add_space(4.);
                                        ui.add_enabled_ui(!running_now, |ui| {
                                            ui.horizontal(|ui| {
                                                if ui.button("Apri").clicked() && let Err(e) = crate::open::document(f) { self.notice = e; }
                                                if ui.button("Cartella").clicked() && let Err(e) = crate::open::folder(&f.path) { self.notice = e; }
                                                egui::ComboBox::from_id_salt(("review-focus", i)).selected_text(f.review.label()).width(150.).show_ui(ui, |ui| {
                                                    for value in [Review::Pending, Review::Confirmed, Review::Discarded] { ui.selectable_value(&mut f.review, value, value.label()); }
                                                });
                                            });
                                        });
                                    }));
                                }
                            });
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
