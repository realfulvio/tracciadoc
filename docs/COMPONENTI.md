# Componenti della distribuzione

- Applicazione Rust MIT, compilazione x64 MSVC con CRT statica, senza `target-cpu=native`.
- egui/eframe 0.37.0, MIT/Apache-2.0: GUI nativa con renderer Glow, senza WebView. Le risorse sono incorporate e caricate una volta; repaint su eventi e durante il controllo.
- PDFium 158.0.8125.0, build `chromium/8125` di bblanchon/pdfium-binaries, licenze BSD e dipendenze in `licenze/pdfium`. `pdfium-render` 0.8.37 e caricato dinamicamente nel processo parser. Usato per estrazione, rasterizzazione e produzione PDF.
- Tesseract 5.5.3 e Leptonica 1.87.0 da pacchetti MSYS2 mingw64, Apache-2.0/BSD e dipendenze. Manifesto pacchetti, hash e licenze in `runtime/ocr/`. La distribuzione include soltanto la chiusura delle DLL importate dal motore; non installa MSYS2.
- Modelli `tessdata_fast` ita+eng, Apache-2.0, commit `87416418657359cb625c412a48b6e1d6d41c29bd`. SHA256 inglese `7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2`; italiano `b8f89e1e785118dac4d51ae042c029a64edb5c3ee42ef73027a6d412748d8827`.
- Titillium Web Regular/SemiBold incorporati, SIL OFL 1.1. Simbolo e icona TracciaDoc dal kit fornito, MIT. Firma originale realhcok fornita dal proprietario per questo uso, senza autorizzazione generale a terzi sul marchio. Licenze e provenienza in `licenze/ui-kit`.
- Elenco versioni/licenze Rust completo in `licenze/dipendenze-rust.txt`; file di licenza, inclusi font di fallback, nella stessa cartella.

Il file `SHA256SUMS.txt` verifica l'integrita dei componenti, non e una firma digitale del programma o del verbale. L'EXE non e firmato con un certificato Authenticode.

L'OCR e inizializzato soltanto al primo contenuto raster. Un'istanza viene riutilizzata nel parser persistente; un solo thread OCR evita sovrapposizione del parallelismo. Il parser e confinato da un Windows Job con limite di memoria e chiusura dei processi, timeout e recupero del worker; mantiene il token dell'operatore e non costituisce una sandbox di sicurezza.

`Automatico` seleziona il prodotto scalare AVX2/SSE con rilevamento Rust a runtime (comprensivo del supporto OS); la scelta viene confermata dalla variabile effettiva Tesseract `dotproduct`. `Solo CPU` forza `generic`. Nessuna funzione OCR GPU/NPU e integrata; AVX-512 e deliberatamente esclusa. Un arresto del parser automatico attiva un nuovo tentativo generico registrato; un timeout non viene mascherato come successo.

Valutazione acceleratori: la documentazione ONNX Runtime raccomanda WinML per i nuovi progetti Windows; DirectML e in sustained engineering. WinML richiede comunque un modello OCR e provider compatibili, alcuni scaricati su richiesta. Non sono stati introdotti provider, modelli o SDK GPU non collaudati nell'MVP offline. Nessun confronto prestazionale GPU/NPU e dichiarato.

Fonti tecniche consultate il 28 ottobre 2026:

- https://github.com/emilk/egui/releases
- https://github.com/tesseract-ocr/tesseract/releases/tag/5.5.3
- https://github.com/tesseract-ocr/tesseract/blob/5.5.3/src/arch/simddetect.cpp
- https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium/8125
- https://doc.rust-lang.org/std/macro.is_x86_feature_detected.html
- https://onnxruntime.ai/docs/get-started/with-windows.html
- https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html
- https://learn.microsoft.com/en-us/windows/ai/new-windows-ml/supported-execution-providers
