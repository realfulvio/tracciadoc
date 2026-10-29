#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod delete;
mod engine;
mod gui;
mod model;
mod open;
mod parser;
mod report;
mod rules;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|s| s == "--worker") {
        parser::worker();
        return;
    }
    // CLI intentionally requires --scan; never starts a scan from merely opening the EXE.
    if args.get(1).is_some_and(|s| s == "--scan") {
        let result = (|| -> Result<(), String> {
            if args.contains(&"--quick".into()) {
                return Err(
                    "Modalita rapida rimossa: le nuove verifiche usano la ricerca completa".into(),
                );
            }
            let root = PathBuf::from(args.get(2).ok_or("Manca percorso locale")?);
            if !root.is_absolute() {
                return Err("Percorso assoluto richiesto".into());
            }
            let output = PathBuf::from(args.get(3).ok_or("Manca percorso verbale PDF")?);
            if output.exists() || output.with_extension("json").exists() {
                return Err("Output gia esistente: non sovrascritto".into());
            }
            let mut settings = model::Settings {
                deep: true,
                cpu_only: args.contains(&"--cpu".into()),
                prefilter: !args.contains(&"--no-prefilter".into()),
                ..Default::default()
            };
            for (flag, min, max, field) in [
                ("--max-mb", 1, 256, &mut settings.max_mb),
                ("--timeout-secs", 1, 1800, &mut settings.timeout_secs),
                ("--max-files", 1, 10_000_000, &mut settings.max_entries),
                ("--memory-mb", 64, 2048, &mut settings.memory_mb),
            ] {
                if let Some(i) = args.iter().position(|s| s == flag) {
                    let n = args
                        .get(i + 1)
                        .and_then(|s| s.parse::<u64>().ok())
                        .ok_or("Limite numerico non valido")?;
                    if !(min..=max).contains(&n) {
                        return Err("Limite numerico fuori intervallo".into());
                    }
                    *field = n;
                }
            }
            if let Some(i) = args.iter().position(|s| s == "--workers") {
                let n = args
                    .get(i + 1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .ok_or("Numero worker non valido")?;
                if !(1..=64).contains(&n) {
                    return Err("Numero worker fuori intervallo".into());
                }
                settings.workers = n;
            }
            if let Some(i) = args.iter().position(|s| s == "--max-pages") {
                let n = args
                    .get(i + 1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .ok_or("Limite pagine non valido")?;
                if !(1..=500).contains(&n) {
                    return Err("Limite pagine fuori intervallo".into());
                }
                settings.max_pages = n;
            }
            let run = engine::new_run(
                vec![root],
                settings,
                "Operatore CLI".into(),
                "Verifica CLI esplicita".into(),
            );
            let shared = Arc::new(Mutex::new(engine::Progress {
                run: Some(run),
                ..Default::default()
            }));
            let stop = Arc::new(AtomicBool::new(false));
            let timer = if let Some(pos) = args.iter().position(|s| s == "--cancel-after-ms") {
                args.get(pos + 1).and_then(|s| s.parse::<u64>().ok())
            } else {
                None
            };
            if let Some(ms) = timer {
                let stop = stop.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                    stop.store(true, std::sync::atomic::Ordering::Relaxed);
                });
            }
            engine::run(shared.clone(), stop);
            let mut run = shared.lock().unwrap().run.clone().unwrap();
            let demo = args.contains(&"--demo".into());
            if demo {
                run.computer = "POSTAZIONE-DIMOSTRATIVA".into();
                run.operator = "Operatore dimostrativo".into();
                run.reference = "SOLO FIXTURE SINTETICHE".into();
            }
            if demo {
                for f in &mut run.findings {
                    f.path = PathBuf::from("CORPUS-SINTETICO")
                        .join(f.path.file_name().unwrap_or_default());
                }
                run.roots = vec![PathBuf::from(
                    "CORPUS-SINTETICO (solo fixture dimostrative)",
                )];
            }
            report::write(&run, &output, !args.contains(&"--omit-paths".into()), demo)?;
            // Structured CLI summary deliberately omits selected roots and finding paths.
            let summary = serde_json::json!({"seconds":run.elapsed_secs,"encountered":run.encountered,"analyzed":run.analyzed,"partial":run.partial_files,"skipped":run.skipped,"findings":run.findings.iter().map(|f|serde_json::json!({"format":f.format,"reasons":f.reasons})).collect::<Vec<_>>(),"issues":run.issues,"incomplete":run.incomplete(),"interrupted":run.interrupted,"pages_ocr":run.pages_ocr,"ocr_seconds":run.ocr_seconds,"ocr_init_ms":run.ocr_init_ms,"peak_worker_bytes":run.peak_worker_bytes,"backend":run.backend,"timing":run.timing,"timing_by_format":run.timing_by_format});
            std::fs::write(
                output.with_extension("json"),
                serde_json::to_vec_pretty(&summary).unwrap(),
            )
            .map_err(|_| "Riepilogo non scrivibile")?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    // DirectX 12 via wgpu: funziona anche senza driver del produttore grazie a WARP
    // (adattatore software di Windows). OpenGL resta solo come ripiego.
    // winit non permette un secondo event loop nello stesso processo: il ripiego riavvia l'EXE.
    if args.get(1).is_some_and(|s| s == "--gui-opengl") {
        if let Err(e) = gui_run(eframe::Renderer::Glow) {
            eprintln!("GUI OpenGL: {e}");
        }
    } else if let Err(e) = gui_run(eframe::Renderer::Wgpu) {
        eprintln!("GUI DirectX 12: {e}; ripiego OpenGL");
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).arg("--gui-opengl").spawn();
        }
    }
}
fn gui_run(renderer: eframe::Renderer) -> eframe::Result {
    use eframe::{egui_wgpu, wgpu};
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = wgpu::Backends::DX12;
    // Preferisce una GPU reale compatibile con la finestra, altrimenti WARP (Cpu).
    setup.native_adapter_selector = Some(Arc::new(|adapters, surface| {
        let usable = |a: &&wgpu::Adapter| surface.is_none_or(|s| a.is_surface_supported(s));
        let rank = |a: &wgpu::Adapter| match a.get_info().device_type {
            wgpu::DeviceType::DiscreteGpu => 0,
            wgpu::DeviceType::IntegratedGpu => 1,
            wgpu::DeviceType::VirtualGpu => 2,
            wgpu::DeviceType::Other => 3,
            wgpu::DeviceType::Cpu => 4,
        };
        adapters
            .iter()
            .filter(usable)
            .min_by_key(|a| rank(a))
            .cloned()
            .ok_or_else(|| "Nessun adattatore DirectX 12 disponibile".to_string())
    }));
    let native = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1180., 820.])
            .with_min_inner_size([640., 540.])
            .with_icon({
                let img =
                    image::load_from_memory(include_bytes!("../assets/ui/app-icon-dark-256.png"))
                        .expect("Icona incorporata")
                        .to_rgba8();
                eframe::egui::IconData {
                    width: img.width(),
                    height: img.height(),
                    rgba: img.into_raw(),
                }
            }),
        renderer,
        wgpu_options: egui_wgpu::WgpuConfiguration {
            wgpu_setup: egui_wgpu::WgpuSetup::CreateNew(setup),
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        "TracciaDoc - verifica documenti",
        native,
        Box::new(|cc| Ok(Box::new(gui::App::new(cc)))),
    )
}
