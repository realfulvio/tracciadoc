# Changelog

## 0.2.0 - non rilasciata

- Selezione dei file segnalati (casella singola e "Seleziona tutti") ed eliminazione definitiva con conferma esplicita. Solo file regolari locali; nessun cestino.
- Ri-scansione di verifica sugli stessi percorsi e verbale finale con riepilogo della verifica iniziale e delle eliminazioni.
- L'esito "nessun documento rilevato dopo l'eliminazione" appare solo con copertura completa. Dopo l'eliminazione, la verifica originale richiede la ri-scansione.
- GUI su DirectX 12 tramite wgpu, con WARP sui PC senza driver video e riavvio automatico su OpenGL come ripiego.
- GUI: tema scuro fisso (con il tema chiaro di Windows il testo era illeggibile), simboli dei passi visibili, ridisegno ogni 1000 ms durante la scansione quando il rendering e su WARP.
- Collaudo su Windows 11 Enterprise LTSC 2024 x64 (VM senza driver video, DX12 su WARP, senza Mesa): `cargo test`, Clippy `-D warnings`, `validate.py` sul corpus sintetico; GUI a schermo con prove sintetiche, eliminazione con conferma, file modificato dopo l'analisi non eliminato, ri-scansione e verbale PDF finale con riepilogo e eliminazioni non riuscite.
- Non verificati: GPU/iGPU reali, ripiego OpenGL con finestra aperta, finestra non massimizzata. `edge_tests.py` si ferma sul caso `memory` (identico sulla 0.1.0).

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
