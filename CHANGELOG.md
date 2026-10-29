# Changelog

## 0.2.0 - non rilasciata

- Selezione dei file segnalati (casella singola e "Seleziona tutti") ed eliminazione definitiva con conferma esplicita. Solo file regolari locali; nessun cestino.
- Ri-scansione di verifica sugli stessi percorsi e verbale finale con riepilogo della verifica iniziale e delle eliminazioni.
- L'esito "nessun documento rilevato dopo l'eliminazione" appare solo con copertura completa. Dopo l'eliminazione, la verifica originale richiede la ri-scansione.
- GUI su DirectX 12 tramite wgpu, con WARP sui PC senza driver video e riavvio automatico su OpenGL come ripiego.
- GUI: tema scuro fisso (con il tema chiaro di Windows il testo era illeggibile), simboli dei passi visibili, ridisegno ogni 1000 ms durante la scansione quando il rendering e su WARP.
- Collaudo su Windows 11 Enterprise LTSC 2024 x64 (VM senza driver video, DX12 su WARP, senza Mesa): `cargo test`, Clippy `-D warnings`, `validate.py` sul corpus sintetico; GUI a schermo con prove sintetiche, eliminazione con conferma, file modificato dopo l'analisi non eliminato, ri-scansione e verbale PDF finale con riepilogo e eliminazioni non riuscite.
- `edge_tests.py`: il caso `memory` usa un raster 5600x5600 (~94 MB decodificati) invece dell'OCR di una piccola immagine, il cui consumo dipendeva dalla macchina e restava sotto 64 MiB. Tutti i casi passano sulla VM Windows.
- Non verificati: GPU/iGPU reali, ripiego OpenGL con finestra aperta, finestra non massimizzata.
- OCR in parallelo: N worker parser/OCR, ognuno processo separato con il proprio Windows Job e limite di memoria; default core fisici - 1 (minimo 1), `--workers` e Opzioni avanzate della GUI. Segnalazioni ordinate per percorso; il verbale dichiara il numero di worker.
- Prefiltro: niente OCR sulle immagini e pagine senza zone con tratti a contrasto tipo testo; dichiarate nel verbale come "scartate dal prefiltro", file contati come parziali. `--no-prefilter` e casella nella GUI per l'OCR su tutto. L'OCR gira solo sulla zona con tratti tipo testo piu un margine.
- Orientamento con OSD di Tesseract (`osd.traineddata` con hash verificato): una sola passata con confidenza >= 2, altrimenti le 4 rotazioni, prima dritta e poi quella suggerita.
- Stemma del Comune di Bondeno nella barra laterale e in testa al verbale; fonte in `assets/ui/FONTI-E-LICENZE.md`.
- `scripts/sign.ps1` per la firma Authenticode con certificato da variabile d'ambiente; provato solo con certificato self-signed temporaneo.
- Corpus di validazione a 41 file: due foto senza testo, un PNG ruotato e un documento ruotato dentro una foto. `edge_tests.py`: timeout su un mosaico di testo.
- Misure e collaudo sulla VM Windows in `docs/VALIDAZIONE.md`, sezione 0.2.0.

## 0.1.0 - 2026-10-28

Primo MVP Windows x64 portatile di TracciaDoc.

- Ricerca locale in sola lettura di possibili carte d'identità, passaporti, patenti e tessere sanitarie.
- Unica ricerca completa supportata, estrazione testuale e OCR offline ita+eng su PDF, immagini e formati Office supportati.
- GUI dark con brand TracciaDoc, font incorporato e firma realhcok.
- Esito esplicito, percorso completo, apertura del documento e della cartella, conferma/scarto manuale.
- Verbale PDF con ambito, limitazioni, tipi/formati e revisione manuale, senza estratti OCR o valori personali riconosciuti.
- Parser persistente separato, timeout, limiti di memoria, annullamento e recupero; CPU generica e AVX2/SSE selezionati a runtime.
- Distribuzione con EXE, runtime, licenze, corpus sintetico e verbale dimostrativo.

Non implementati AVX-512, GPU/NPU, lettura legacy DOC/XLS/PPT, archivi generici e cifrati. GIF/TIFF limitati alla prima immagine. Parser non equivalente a sandbox, EXE non firmato Authenticode. Il verbale non certifica assenza assoluta o accettazione Lepida.

Dettagli del collaudo e benchmark: [docs/VALIDAZIONE.md](docs/VALIDAZIONE.md). Le prestazioni e i risultati sono riferiti al corpus sintetico, non a documenti reali.
