//! Rilevamento delle fototessere: un volto frontale, grande, su sfondo uniforme.
//!
//! Il rilevatore e YuNet (OpenCV Zoo, MIT) in formato ONNX, incorporato nell'eseguibile ed
//! eseguito con tract (puro Rust, solo CPU, nessuna DLL aggiuntiva). Il modello trova i volti;
//! la decisione "fototessera" la prendono i criteri di `judge` su dimensione, proporzioni,
//! frontalita e sfondo. L'esito e sempre un indizio da rivedere, mai una prova.
use image::{DynamicImage, GenericImageView};
use std::sync::OnceLock;
use tract_onnx::prelude::*;

const MODEL: &[u8] = include_bytes!("../assets/face/yunet_320.onnx");
const SIDE: usize = 320;
/// Punteggio minimo di un volto per entrare nell'elenco.
const MIN_SCORE: f32 = 0.6;
const NMS_IOU: f32 = 0.3;

/// Prefisso comune dei motivi: la GUI e il verbale riconoscono la classe da qui.
pub const CLASS: &str = "Fototessera:";

type Plan = TypedRunnableModel<TypedModel>;

#[derive(Clone, Copy, Debug)]
pub struct Face {
    pub score: f32,
    /// Riquadro nell'immagine originale, in pixel.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Occhio destro e sinistro (del soggetto, quindi a sinistra e a destra nell'immagine) e naso.
    pub eyes: [(f32, f32); 2],
    pub nose: (f32, f32),
}

fn plan() -> Result<&'static Plan, String> {
    static PLAN: OnceLock<Result<Plan, String>> = OnceLock::new();
    PLAN.get_or_init(|| {
        (|| -> TractResult<Plan> {
            tract_onnx::onnx()
                .model_for_read(&mut &MODEL[..])?
                .with_input_fact(0, f32::fact([1, 3, SIDE, SIDE]).into())?
                .into_optimized()?
                .into_runnable()
        })()
        .map_err(|e| format!("Modello volti non caricato: {e}"))
    })
    .as_ref()
    .map_err(|e| e.clone())
}

/// Volti trovati nell'immagine, dal piu sicuro, senza sovrapposizioni.
pub fn faces(img: &DynamicImage) -> Result<Vec<Face>, String> {
    let (iw, ih) = img.dimensions();
    if iw == 0 || ih == 0 {
        return Ok(vec![]);
    }
    let plan = plan()?;
    let small = img.thumbnail(SIDE as u32, SIDE as u32).to_rgb8();
    let scale = small.width() as f32 / iw as f32;
    // Ingresso: BGR, 0-255, CHW; il riquadro non coperto dall'immagine resta nero.
    let mut input = tract_ndarray::Array4::<f32>::zeros((1, 3, SIDE, SIDE));
    for (x, y, p) in small.enumerate_pixels() {
        for (c, v) in [(0, p[2]), (1, p[1]), (2, p[0])] {
            input[[0, c, y as usize, x as usize]] = v as f32;
        }
    }
    let out = plan
        .run(tvec!(Tensor::from(input).into()))
        .map_err(|e| format!("Rilevamento volti non riuscito: {e}"))?;
    let get = |i: usize| -> Result<tract_ndarray::ArrayViewD<'_, f32>, String> {
        out[i].to_array_view::<f32>().map_err(|e| e.to_string())
    };
    let mut found: Vec<Face> = vec![];
    for (level, stride) in [8usize, 16, 32].into_iter().enumerate() {
        let (cls, obj, bbox, kps) = (get(level)?, get(3 + level)?, get(6 + level)?, get(9 + level)?);
        let n = SIDE / stride;
        let at = |a: &tract_ndarray::ArrayViewD<'_, f32>, k: usize, j: usize| a[[0, k, j]];
        for k in 0..n * n {
            let score = (at(&cls, k, 0).clamp(0., 1.) * at(&obj, k, 0).clamp(0., 1.)).sqrt();
            if score < MIN_SCORE {
                continue;
            }
            let (row, col) = ((k / n) as f32, (k % n) as f32);
            let s = stride as f32;
            let (cx, cy) = ((col + at(&bbox, k, 0)) * s, (row + at(&bbox, k, 1)) * s);
            let (w, h) = (at(&bbox, k, 2).exp() * s, at(&bbox, k, 3).exp() * s);
            let pt = |i: usize| ((col + at(&kps, k, 2 * i)) * s / scale, (row + at(&kps, k, 2 * i + 1)) * s / scale);
            found.push(Face {
                score,
                x: (cx - w / 2.) / scale,
                y: (cy - h / 2.) / scale,
                w: w / scale,
                h: h / scale,
                eyes: [pt(0), pt(1)],
                nose: pt(2),
            });
        }
    }
    found.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Face> = vec![];
    for f in found {
        if kept.iter().all(|k| iou(k, &f) < NMS_IOU) {
            kept.push(f);
        }
    }
    Ok(kept)
}

fn iou(a: &Face, b: &Face) -> f32 {
    let w = ((a.x + a.w).min(b.x + b.w) - a.x.max(b.x)).max(0.);
    let h = ((a.y + a.h).min(b.y + b.h) - a.y.max(b.y)).max(0.);
    let i = w * h;
    i / (a.w * a.h + b.w * b.h - i).max(1e-6)
}

/// Il volto guarda l'obiettivo: occhi quasi in orizzontale e naso tra gli occhi.
fn frontal(f: &Face) -> bool {
    let [(rx, ry), (lx, ly)] = f.eyes;
    let dist = ((lx - rx).powi(2) + (ly - ry).powi(2)).sqrt();
    if dist < 1. {
        return false;
    }
    let tilt = ((ly - ry).atan2(lx - rx)).to_degrees().abs();
    let off = (f.nose.0 - (rx + lx) / 2.).abs() / dist;
    tilt <= 20. && off <= 0.3 && f.nose.1 > ry.min(ly)
}

/// Sfondo uniforme: i due margini laterali nella parte alta dell'immagine hanno poca variazione
/// di luminosita e sono simili fra loro. Una fototessera ha un fondo di un solo colore; un selfie
/// o un ritratto d'ambiente no.
pub fn uniform_background(img: &DynamicImage) -> bool {
    let g = img.thumbnail(96, 96).to_luma8();
    let (w, h) = g.dimensions();
    if w < 16 || h < 16 {
        return false;
    }
    let band = (w as f32 * 0.16).ceil() as u32;
    let rows = (h as f32 * 0.65) as u32;
    let stats = |x0: u32, x1: u32| {
        let vals: Vec<f32> = (0..rows)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .map(|(x, y)| g.get_pixel(x, y)[0] as f32)
            .collect();
        let mean = vals.iter().sum::<f32>() / vals.len() as f32;
        let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / vals.len() as f32;
        (mean, var.sqrt())
    };
    let (lm, ls) = stats(0, band);
    let (rm, rs) = stats(w - band, w);
    ls <= 16. && rs <= 16. && (lm - rm).abs() <= 40.
}

/// Decide se l'immagine e una fototessera, dati i volti trovati. Restituisce il motivo da
/// riportare, oppure None.
pub fn judge(img: &DynamicImage, faces: &[Face]) -> Option<String> {
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let main = faces.first()?;
    // Altri volti di dimensione paragonabile: foto di gruppo, non una fototessera.
    if faces.iter().skip(1).any(|f| f.h >= main.h * 0.5) {
        return None;
    }
    let aspect = iw / ih;
    if !(0.55..=1.15).contains(&aspect) {
        return None;
    }
    let size = main.h / ih;
    if size < 0.30 || !frontal(main) {
        return None;
    }
    // Il volto sta al centro, non tagliato dal bordo.
    let cx = (main.x + main.w / 2.) / iw;
    if !(0.3..=0.7).contains(&cx) || main.x < 0. || main.x + main.w > iw {
        return None;
    }
    if uniform_background(img) {
        Some(format!("{CLASS} ritratto frontale unico su sfondo uniforme (probabile)"))
    } else if size >= 0.45 {
        Some(format!("{CLASS} ritratto frontale molto ravvicinato, sfondo non uniforme (possibile)"))
    } else {
        None
    }
}

/// Fototessera nell'immagine, se c'e. Gli errori del rilevatore non fermano l'analisi: tornano
/// come nota da riportare fra i limiti.
pub fn photo_id(img: &DynamicImage) -> Result<Option<String>, String> {
    let (w, h) = (img.width(), img.height());
    // Icone, miniature e strisce non possono essere una fototessera leggibile.
    if w.min(h) < 100 || !(0.55..=1.15).contains(&(w as f32 / h as f32)) {
        return Ok(None);
    }
    Ok(judge(img, &faces(img)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn fill(img: &mut RgbImage, x0: u32, y0: u32, x1: u32, y1: u32, c: [u8; 3]) {
        for y in y0..y1.min(img.height()) {
            for x in x0..x1.min(img.width()) {
                img.put_pixel(x, y, Rgb(c));
            }
        }
    }
    fn ellipse(img: &mut RgbImage, cx: f32, cy: f32, rx: f32, ry: f32, c: [u8; 3]) {
        for y in 0..img.height() {
            for x in 0..img.width() {
                let (dx, dy) = ((x as f32 - cx) / rx, (y as f32 - cy) / ry);
                if dx * dx + dy * dy <= 1. {
                    img.put_pixel(x, y, Rgb(c));
                }
            }
        }
    }
    /// Ritratto disegnato (nessuna persona reale): fondo, spalle, collo, testa, capelli, occhi,
    /// naso e bocca. Il rilevatore lo riconosce come volto.
    fn portrait(bg: [u8; 3], noisy: bool) -> DynamicImage {
        let (w, h) = (350u32, 450u32);
        let mut im = RgbImage::from_pixel(w, h, Rgb(bg));
        if noisy {
            // Fondo d'ambiente: strisce di colori diversi.
            for i in 0..14u32 {
                let c = [(40 + i * 15) as u8, (200 - i * 9) as u8, (90 + (i % 3) * 50) as u8];
                fill(&mut im, 0, i * 32, 70, i * 32 + 32, c);
                fill(&mut im, w - 70, i * 32, w, i * 32 + 32, [c[2], c[0], c[1]]);
            }
        }
        let (skin, hair) = ([224, 172, 140], [60, 40, 30]);
        let cx = w as f32 / 2.;
        ellipse(&mut im, cx, h as f32 * 1.05, 150., 150., [40, 60, 110]);
        fill(&mut im, (cx - 30.) as u32, (h as f32 * 0.55) as u32, (cx + 30.) as u32, (h as f32 * 0.75) as u32, skin);
        ellipse(&mut im, cx, h as f32 * 0.39, 62., h as f32 * 0.21, skin);
        ellipse(&mut im, cx, h as f32 * 0.25, 64., h as f32 * 0.09, hair);
        for dx in [-28., 28.] {
            ellipse(&mut im, cx + dx, h as f32 * 0.36, 14., 8., [255, 255, 255]);
            ellipse(&mut im, cx + dx, h as f32 * 0.36, 6., 6., [50, 30, 20]);
            fill(&mut im, (cx + dx - 18.) as u32, (h as f32 * 0.31) as u32, (cx + dx + 18.) as u32, (h as f32 * 0.31) as u32 + 5, hair);
        }
        fill(&mut im, cx as u32 - 3, (h as f32 * 0.38) as u32, cx as u32 + 3, (h as f32 * 0.48) as u32, [190, 140, 110]);
        fill(&mut im, cx as u32 - 22, (h as f32 * 0.52) as u32, cx as u32 + 22, (h as f32 * 0.52) as u32 + 5, [160, 60, 60]);
        // Come una foto: bordi morbidi.
        DynamicImage::ImageRgba8(image::imageops::blur(&DynamicImage::ImageRgb8(im).to_rgba8(), 1.5))
    }

    #[test]
    fn model_loads_and_empty_images_have_no_faces() {
        let plain = DynamicImage::ImageRgb8(RgbImage::from_pixel(400, 500, Rgb([230, 230, 230])));
        assert!(faces(&plain).unwrap().is_empty());
        assert_eq!(photo_id(&plain).unwrap(), None);
    }

    #[test]
    fn drawn_portrait_on_plain_background_is_a_photo_id() {
        let img = portrait([225, 228, 232], false);
        let f = faces(&img).unwrap();
        assert_eq!(f.len(), 1, "{f:?}");
        let reason = judge(&img, &f).expect("fototessera");
        assert!(reason.starts_with(CLASS) && reason.contains("probabile"), "{reason}");
    }

    #[test]
    fn busy_background_is_not_uniform() {
        assert!(uniform_background(&portrait([225, 228, 232], false)));
        assert!(!uniform_background(&portrait([225, 228, 232], true)));
    }

    #[test]
    fn small_face_in_a_large_page_is_not_judged() {
        // Stesso ritratto dentro un foglio molto piu grande: il volto e il 10% dell'altezza.
        let p = portrait([225, 228, 232], false).to_rgb8();
        let mut page = RgbImage::from_pixel(1000, 1400, Rgb([250, 250, 250]));
        image::imageops::overlay(&mut page, &p, 300, 400);
        let page = DynamicImage::ImageRgb8(page);
        assert_eq!(photo_id(&page).unwrap(), None);
    }

    #[test]
    fn landscape_images_are_skipped() {
        let wide = DynamicImage::ImageRgb8(RgbImage::from_pixel(800, 400, Rgb([200, 200, 200])));
        assert_eq!(photo_id(&wide).unwrap(), None);
    }

    #[test]
    fn two_similar_faces_are_a_group() {
        let f = |x: f32| Face {
            score: 0.9,
            x,
            y: 100.,
            w: 100.,
            h: 150.,
            eyes: [(x + 30., 140.), (x + 70., 140.)],
            nose: (x + 50., 170.),
        };
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(400, 450, Rgb([225, 228, 232])));
        assert_eq!(judge(&img, &[f(80.), f(220.)]), None);
        assert!(judge(&img, &[f(150.)]).is_some());
    }

    #[test]
    fn turned_face_is_not_frontal() {
        let f = Face {
            score: 0.9,
            x: 150.,
            y: 100.,
            w: 100.,
            h: 140.,
            eyes: [(170., 140.), (230., 140.)],
            nose: (228., 180.),
        };
        assert!(!frontal(&f));
    }

    /// Prova su un'immagine reale fornita a mano (mai nel repository):
    /// TRACCIADOC_FACE_SAMPLE=percorso cargo test face_sample -- --nocapture
    #[test]
    fn face_sample() {
        let Ok(path) = std::env::var("TRACCIADOC_FACE_SAMPLE") else { return };
        let img = image::open(path).unwrap();
        let t = std::time::Instant::now();
        let f = faces(&img).unwrap();
        println!("volti {} in {:?}; giudizio: {:?}", f.len(), t.elapsed(), judge(&img, &f));
    }
}
