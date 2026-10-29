use crate::{
    engine::{self, Shared},
    model::{Previous, Review, Settings},
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
const ACCENT: Color32 = Color32::from_rgb(121, 205, 242);
const PRIMARY: Color32 = Color32::from_rgb(0, 110, 166);
const BG: Color32 = Color32::from_rgb(17, 27, 42);
const SURFACE: Color32 = Color32::from_rgb(27, 43, 64);
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
    limits.max_image_width = Some(7000);
    limits.max_image_height = Some(7000);
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
    stemma: egui::TextureHandle,
    zoom: f32,
    confirm: bool,
    previews: HashMap<PathBuf, Preview>,
    /// Intervallo di ridisegno durante la scansione: piu lungo sul rendering software (WARP).
    repaint: Duration,
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
            style.visuals.override_text_color = Some(Color32::from_rgb(243, 246, 250));
            style.visuals.weak_text_color = Some(Color32::from_rgb(171, 193, 210));
            style.visuals.selection.bg_fill = Color32::from_rgb(18, 62, 89);
            style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(0, 125, 184);
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
            stemma: texture(
                &cc.egui_ctx,
                "stemma-comune-bondeno",
                include_bytes!("../assets/ui/stemma-comune-bondeno.png"),
            ),
            zoom: 1.,
            confirm: false,
            previews: HashMap::new(),
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
        .stroke(egui::Stroke::new(1., Color32::from_rgb(49, 74, 96)))
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
                r.finished = true;
                r.interrupted = true;
                r.end = engine::now();
            }
        }
        let running = self.handle.is_some();
        if running {
            ctx.request_repaint_after(self.repaint);
        }
        egui::Panel::bottom("brand_footer")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(19, 31, 47))
                    .inner_margin(16),
            )
            .show(root, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let ratio = self.signature.size()[1] as f32 / self.signature.size()[0] as f32;
                    ui.add(
                        egui::Image::from_texture(&self.signature)
                            .fit_to_exact_size(egui::vec2(168., 168. * ratio)),
                    )
                    .on_hover_text("powered by realhcok");
                    ui.separator();
                    ui.label(
                        RichText::new("Locale  /  Solo lettura  /  Nessuna telemetria")
                            .small()
                            .weak(),
                    );
                    egui::ComboBox::from_id_salt("ui-scale")
                        .selected_text(format!("UI {:.0}%", self.zoom * 100.))
                        .show_ui(ui, |ui| {
                            for v in [1., 1.25, 1.5, 2.] {
                                if ui
                                    .selectable_value(
                                        &mut self.zoom,
                                        v,
                                        format!("{:.0}%", v * 100.),
                                    )
                                    .changed()
                                {
                                    ctx.set_zoom_factor(v);
                                }
                            }
                        });
                    if ui.add_enabled(!running && !self.roots.is_empty() && !self.operator.trim().is_empty(),egui::Button::new(RichText::new("Avvia verifica").font(semibold(19.)).color(Color32::WHITE)).fill(PRIMARY).min_size(egui::vec2(220.,48.))).clicked(){self.start(None);}
                    if running && ui.button("Interrompi").clicked(){self.stop.store(true,Ordering::Relaxed);self.notice="Interruzione richiesta: chiusura parser e conservazione degli esiti raccolti.".into();}
                });
            });
        let phase_state = self.shared.lock().unwrap().run.as_ref().map(|r| (r.finished, r.previous.is_some()));
        let active = match (running, phase_state) {
            (true, _) => 2,
            (false, Some((true, true))) => 4,
            (false, Some((true, false))) => 3,
            _ => 1,
        };
        egui::Panel::left("steps")
            .exact_size(220.)
            .frame(egui::Frame::new().fill(Color32::from_rgb(19, 31, 47)).inner_margin(16))
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Image::from_texture(&self.mark).fit_to_exact_size(egui::vec2(36., 36.)));
                    ui.label(RichText::new("TracciaDoc").font(semibold(22.)));
                });
                ui.add_space(14.);
                for (n, label) in [(1, "Aree da controllare"), (2, "Scansione"), (3, "Revisione e pulizia"), (4, "Verifica finale")] {
                    let (mark, color) = if n < active { ("◉", Color32::from_rgb(103, 214, 191)) } else if n == active { ("●", ACCENT) } else { ("○", Color32::from_rgb(171, 193, 210)) };
                    ui.label(RichText::new(format!("{mark}  {label}")).font(semibold(16.)).color(color));
                }
                ui.add_space(24.);
                // Stemma con scritta scura: su disco bianco per restare leggibile sul fondo blu.
                ui.vertical_centered(|ui| {
                    let (rect, response) = ui.allocate_exact_size(egui::vec2(140., 140.), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 70., Color32::WHITE);
                    egui::Image::from_texture(&self.stemma).paint_at(ui, rect.shrink(4.));
                    response.on_hover_text("Comune di Bondeno");
                });
            });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(24)).show(root,|ui| {
            egui::ScrollArea::vertical().show(ui,|ui| {
                ui.add_space(12.);
                ui.horizontal(|ui| {
                    ui.add(egui::Image::from_texture(&self.mark).fit_to_exact_size(egui::vec2(48.,48.)));
                    ui.label(RichText::new("TracciaDoc").font(semibold(30.)));
                });
                ui.label(RichText::new("Verifica questo computer").font(semibold(24.)));
                ui.label(RichText::new("Cerca possibili copie di documenti nei dischi locali. Il verbale documenta la verifica.").weak());
                ui.add_space(8.);
                ui.add_enabled_ui(!running,|ui| {
                    card(ui,|ui| {
                        ui.label(RichText::new("1   Aree da controllare").font(semibold(21.)));
                        ui.label("Nessun disco selezionato automaticamente. Il controllo parte solo con Avvia verifica.");
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Aggiungi cartella...").clicked()&& let Some(p)=rfd::FileDialog::new().set_parent(&*frame).pick_folder(){self.add(p);}
                            egui::ComboBox::from_id_salt("volumes").selected_text("Dischi e volumi locali").show_ui(ui,|ui| {
                                for (path,label) in self.volumes.clone() {if ui.selectable_label(false,label).clicked(){self.add(path);}}
                            });
                            if ui.button("Carica prove sintetiche").clicked() {
                                let p=std::env::current_exe().unwrap().parent().unwrap().join("prove-sintetiche");
                                if p.is_dir() {self.roots=vec![p];self.demo=true;self.operator="Operatore dimostrativo".into();self.reference="COLLAUDO SINTETICO".into();}
                                else {self.notice="Cartella prove-sintetiche assente accanto all'EXE.".into();}
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut self.path).hint_text("Percorso locale, anche volume montato").desired_width((ui.available_width()-110.).max(130.)));
                            if ui.button("Aggiungi").clicked() {self.add(PathBuf::from(self.path.trim()));}
                        });
                        let mut remove=None;
                        for (i,p) in self.roots.iter().enumerate() {ui.horizontal_wrapped(|ui|{ui.label(p.to_string_lossy());if ui.small_button("Rimuovi").clicked(){remove=Some(i);}});}
                        if let Some(i)=remove {self.roots.remove(i);}
                        if self.roots.is_empty(){ui.label(RichText::new("Seleziona almeno una cartella o un disco locale.").color(Color32::from_rgb(247,199,110)));}
                        ui.collapsing("Copertura ed esclusioni",|ui| {
                            ui.label("Incluse tutte le sottocartelle accessibili: profili, AppData, temporanei e cestino. Nessuna estensione esclusa silenziosamente.");
                            ui.label("Esclusi percorsi esterni, rete, collegamenti e junction. I volumi montati discendenti vanno selezionati separatamente. File cloud offline, EFS, negati, formati non supportati e limiti rendono la verifica incompleta.");
                        });
                    });
                    ui.add_space(8.);
                    card(ui,|ui| {
                        ui.label(RichText::new("2   Ricerca completa e verbale").font(semibold(21.)));
                        ui.label("Testo, immagini, PDF anche misti e Office con immagini incorporate. OCR locale con tentativi di rotazione, entro i limiti dichiarati.");
                        ui.label(RichText::new(if self.ocr_present {"OCR: componenti ita + eng presenti; caricamento e funzionamento verificati all'uso."}else{"OCR NON DISPONIBILE: mancano motore o modelli; copertura incompleta."}).color(if self.ocr_present {ACCENT}else{Color32::YELLOW}));
                        ui.horizontal(|ui| {
                            ui.label("Operatore");ui.add(egui::TextEdit::singleline(&mut self.operator).desired_width(220.));
                        });
                        ui.horizontal(|ui|{ui.label("Riferimento");ui.add(egui::TextEdit::singleline(&mut self.reference).desired_width(300.));});
                        ui.collapsing("Opzioni avanzate e limiti",|ui| {
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
                    });
                });
                ui.add_space(10.);
                if !running && self.operator.trim().is_empty(){ui.label("Compila il nome dell'operatore per avviare la verifica.");}
                if !self.notice.is_empty(){ui.label(RichText::new(&self.notice).color(Color32::from_rgb(247,199,110)));}
                let mut rescan: Option<Previous> = None;
                let (run,current,phase)={let p=self.shared.lock().unwrap();(p.run.clone(),p.current.clone(),p.phase.clone())};
                if let Some(mut run)=run {
                    ui.add_space(8.);
                    card(ui,|ui| {
                        ui.label(RichText::new("3   Avanzamento e risultati").font(semibold(21.)));
                        // Niente ui.spinner(): chiede un ridisegno a ogni frame; l'indicatore avanza col ridisegno periodico.
                        if running {let tick=self.started.map(|s|s.elapsed().as_millis()/self.repaint.as_millis()).unwrap_or(0) as usize;ui.horizontal(|ui|{ui.label(RichText::new(["◐","◓","◑","◒"][tick%4]).color(ACCENT));ui.label(&phase);});ui.label(current);}
                        let elapsed=if running {self.started.map(|s|s.elapsed().as_secs_f64()).unwrap_or(0.)}else{run.elapsed_secs};
                        ui.label(format!("Incontrati {}    Analizzati {}    Parziali {}    Saltati {}    Segnalati {}    Tempo {:.1}s",run.encountered,run.analyzed,run.partial_files,run.skipped,run.findings.len(),elapsed));
                        if run.timing.prefilter_skipped>0 {ui.label(format!("Immagini scartate dal prefiltro (senza OCR): {}",run.timing.prefilter_skipped));}
                        if !running {
                            ui.label(RichText::new(run.outcome()).font(semibold(21.)).color(if run.incomplete(){Color32::from_rgb(247,199,110)}else{ACCENT}));
                            for line in run.finding_summary(){ui.label(line);}
                        }
                        if !running && run.finished {
                            ui.checkbox(&mut self.include_paths,"Includi percorsi dei file segnalati nel PDF (possono contenere dati personali)");
                            ui.checkbox(&mut self.demo,"Contrassegna il verbale come DIMOSTRATIVO");
                            if ui.button("Genera verbale PDF...").clicked()
                                && let Some(path)=rfd::FileDialog::new().set_parent(&*frame).add_filter("Verbale PDF",&["pdf"]).set_file_name(format!("Verbale-TracciaDoc-{}.pdf",run.id)).save_file() {
                                    match report::write(&run,&path,self.include_paths,self.demo) {Ok(())=>{self.notice=format!("Verbale salvato: {}",path.display());let _=engine::audit(&run,"pdf-export");},Err(e)=>self.notice=e}
                                }
                        }
                        if !running && run.finished && !run.findings.is_empty() {
                            let selected=run.findings.iter().filter(|f|f.selected&&!f.deleted()).count();
                            let open=run.findings.iter().filter(|f|!f.deleted()).count();
                            ui.horizontal_wrapped(|ui|{
                                let mut all=open>0&&selected==open;
                                if ui.add_enabled(open>0,egui::Checkbox::new(&mut all,"Seleziona tutti")).changed(){for f in run.findings.iter_mut().filter(|f|!f.deleted()){f.selected=all;}}
                                if ui.add_enabled(selected>0,egui::Button::new(format!("Elimina definitivamente i {selected} selezionati"))).clicked(){self.confirm=true;}
                                if run.deleted_count()>0 && ui.add(egui::Button::new(RichText::new("Ri-scansiona per la verifica finale").color(Color32::WHITE)).fill(PRIMARY)).clicked(){rescan=Some(run.summary());}
                            });
                        }
                        if !running && self.confirm {
                            let n=run.findings.iter().filter(|f|f.selected&&!f.deleted()).count();
                            ui.label(RichText::new(format!("ATTENZIONE: {n} file saranno eliminati in modo definitivo (non vanno nel cestino) e non potranno essere recuperati da TracciaDoc.")).color(Color32::from_rgb(247,199,110)));
                            ui.horizontal(|ui|{
                                if ui.button("Conferma eliminazione definitiva").clicked() {
                                    for f in run.findings.iter_mut().filter(|f|f.selected&&!f.deleted()) {
                                        match crate::delete::remove(f) {Ok(())=>{f.deleted_at=engine::now();f.delete_error.clear();f.selected=false;},Err(e)=>f.delete_error=e}
                                    }
                                    let _=engine::audit(&run,"delete");
                                    self.notice=format!("Eliminati {} file; residui {}. Eseguire la ri-scansione di verifica.",run.deleted_count(),run.remaining());
                                    self.confirm=false;
                                }
                                if ui.button("Annulla").clicked(){self.confirm=false;}
                            });
                        }
                        egui::CollapsingHeader::new(format!("File segnalati ({}) - percorso e verifica manuale",run.findings.len())).id_salt("finding-files").default_open(true).show(ui,|ui|{
                            // Al massimo 2 decodifiche per frame: la GUI resta reattiva anche con molti risultati.
                            let mut budget=2;
                            for (i,f) in run.findings.iter_mut().enumerate().take(5000) {
                                let side=164.;
                                if budget>0&&!self.previews.contains_key(&f.path)&&!f.deleted(){budget-=1;let pv=make_preview(&ctx,&f.path,&f.format);self.previews.insert(f.path.clone(),pv);ctx.request_repaint();}
                                ui.horizontal_top(|ui| {
                                    ui.allocate_ui(egui::vec2((ui.available_width()-side-12.).max(260.),0.),|ui| { ui.vertical(|ui| {
                                ui.label(RichText::new(format!("{}. {}",i+1,f.path.display())).font(semibold(16.)));
                                    ui.label(format!("Formato rilevato: {}",f.format));
                                    for reason in &f.reasons{ui.label(reason);}
                                    if f.deleted(){ui.label(RichText::new(format!("ELIMINATO definitivamente: {}",f.deleted_at)).color(ACCENT));}
                                    else if !f.delete_error.is_empty(){ui.label(RichText::new(&f.delete_error).color(Color32::from_rgb(247,199,110)));}
                                    ui.add_enabled_ui(!running&&!f.deleted(),|ui| {ui.horizontal_wrapped(|ui| {
                                        if ui.button("Apri file").clicked()&& let Err(e)=crate::open::document(f){self.notice=e;}
                                        if ui.button("Mostra nella cartella").clicked()&& let Err(e)=crate::open::folder(&f.path){self.notice=e;}
                                        ui.checkbox(&mut f.selected,"Da eliminare");
                                        egui::ComboBox::from_id_salt(("review",i)).selected_text(f.review.label()).show_ui(ui,|ui| {
                                            for value in [Review::Pending,Review::Confirmed,Review::Discarded] {ui.selectable_value(&mut f.review,value,value.label());}
                                        });
                                    });});

                                    });});
                                    let (rect,resp)=ui.allocate_exact_size(egui::vec2(side,112.),egui::Sense::hover());
                                    ui.painter().rect_filled(rect,8.,Color32::from_rgb(27,43,64));
                                    match self.previews.get(&f.path) {
                                        Some(Some((blur,sharp)))=>{let tex=if resp.hovered(){sharp}else{blur};egui::Image::from_texture(tex).fit_to_exact_size(rect.size()).paint_at(ui,rect);ui.painter().text(rect.center_bottom()-egui::vec2(0.,8.),egui::Align2::CENTER_BOTTOM,if resp.hovered(){""}else{"passa sopra"},egui::FontId::proportional(12.),Color32::WHITE);}
                                        _=>{ui.painter().text(rect.center(),egui::Align2::CENTER_CENTER,f.format.to_uppercase(),semibold(15.),Color32::from_rgb(171,193,210));}
                                    }
                                });
                                ui.separator();
                            }
                        });
                        ui.collapsing(format!("Errori e limiti ({} categorie)",run.issues.len()),|ui|{for (s,n) in &run.issues {ui.label(format!("{n} - {s}"));}});
                    });
                    if self.handle.is_none(){self.shared.lock().unwrap().run=Some(run.clone());}
                    if let Some(p)=rescan {self.roots=run.roots.clone();self.start(Some(p));}
                }
                ui.add_space(10.);
                ui.label(RichText::new("Il controllo non certifica l'assenza assoluta di documenti. Le segnalazioni richiedono verifica manuale. Nessuna approvazione Lepida.").small().weak());
            });
        });
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
