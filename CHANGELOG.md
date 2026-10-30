# Changelog

## 0.2.7 - 2026-10-30

- Lettura dei vecchi documenti Word `.doc` (binario OLE, Word 97-2003 e successivi, anche Word 6/95), scritta in Rust senza dipendenze nuove: il testo viene estratto dalla tabella dei pezzi (corpo, intestazioni, note, caselle di testo) e analizzato con le regole esistenti; le immagini incorporate (JPEG, PNG, TIFF, DIB, inline e mobili) vanno all'OCR come quelle dei `.docx`. Prima i `.doc` finivano in "formato non supportato".
- Restano segnalati come verifica incompleta: `.doc` cifrati con password, immagini vettoriali EMF/WMF/PICT, oggetti OLE incorporati e macro, file corrotti. Un `.doc` che in realtà è un RTF viene letto come RTF.
- Corpus sintetico: aggiunti `documento.doc` e `immagine.doc` (veri Word 97 generati con LibreOffice); `legacy.doc` resta il caso "DOC corrotto".

## 0.2.6 - 2026-10-30

Correzioni dal collaudo sulla VM Windows 11 (corpus sintetico di stress con 3.424 file: scansioni ad alta risoluzione, file corrotti o con estensione ingannevole, Office protetto da password, HEIC, PDF lunghi, nomi Unicode, percorsi oltre 260 caratteri, migliaia di file).

- File con estensione di un formato cercato ma contenuto non riconosciuto (foto `.heic`, `.docx` protetto da password o in formato OLE, `.jpg`/`.pdf` corrotti o rinominati) non vengono più contati in silenzio come "ignorati per tipo": sono saltati con un motivo esplicito e la verifica risulta incompleta. Restano ignorati i file senza estensione significativa (.dat, .tmp, ...) che non contengono un formato di interesse.
- Le scansioni A4 a 600 dpi (4960x7016) erano scartate come "immagine corrotta" per il limite di 7000 px: il limite passa a 10000 px per lato (sempre entro 128 MiB decodificati) e le immagini oltre 3600 px vengono ridotte prima dell'OCR (circa 300 dpi).
- PDF con byte spuri prima dell'intestazione `%PDF-` riconosciuti come PDF.
- Regole 2026.10.30-1: le etichette dei campi contano solo a inizio parola. Prima "COGNOME" contava anche come "NOME" e "SURNAME" come "NAME", e un modulo che chiede di allegare la carta d'identità con un codice pratica veniva segnalato come documento.
- Avvisi di enumerazione (accesso negato, collegamenti, file cloud/EFS) limitati a 50 per tipo nel log, con il percorso del file; prima ogni voce scriveva una riga.
- Conteggio preliminare dell'avanzamento allineato al limite file (contava un file in meno).
- GUI a finestra piccola (minimo 640x540) e su schermi 1024x768: footer su una riga ("Interrompi" prende il posto di "Avvia verifica"), console ridotta, indicatore a cerchio più piccolo, anteprime nascoste quando non c'è spazio; la finestra ripristinata misura 980x660 e non esce più dallo schermo.
- "Apri file" accetta anche le immagini `.jfif`.
- Generare il verbale su un file esistente dà un messaggio chiaro invece dell'errore di sistema.
- Clippy `-D warnings` torna pulito con Rust 1.99 (`needless_borrow` in engine.rs).
- `edge_tests.py`: il caso `memory` usa un raster 6400x6400 e un limite di 96 MiB; a 64 MiB anche l'OCR di una piccola immagine (motori ita+eng e OSD, ~90 MiB) non entrava più, e il test falliva sul file che doveva riuscire.

## 0.2.5 - 2026-10-29

- Indicatore di avanzamento nella barra laterale: cerchio che si riempie con la percentuale, file esaminati/totale, file al secondo e tempo residuo. Il totale viene da un conteggio preliminare in parallelo (solo nomi e dimensioni, niente lettura dei file): finché non è finito la percentuale è indicata con "~", il totale con "+" e il tempo residuo resta "-". Non arriva al 100% finché la scansione non è conclusa.
- La finestra si apre massimizzata (con barra del titolo, ridimensionabile).
- I file di solo testo (txt, csv, tsv, log, md) non vengono più letti: non possono contenere copie di documenti e producevano solo avvisi e falsi positivi. Restano analizzati rtf, PDF, immagini e documenti Office/OpenDocument. Il corpus sintetico non include più file di testo.

## 0.2.4 - 2026-10-29

- Prima di aprire un file si guarda nome e dimensione: si aprono solo immagini (jpg, png, bmp, tif, gif, webp, heic), pdf, documenti Office/OpenDocument, rtf e testo. exe, dll, svg e il resto non vengono aperti e finiscono nel contatore "ignorati per tipo". Solo file senza estensione o con estensione tipo .dat/.tmp/.bak (20 KB fino al limite) vengono riconosciuti dalla firma.
- Log e console puliti: "Formato non supportato", immagini scartate dal prefiltro, TIFF/GIF, RTF, archivi generici e testi non UTF-8 sono solo contati (restano nei problemi del report). Restano negli eventi i problemi veri (timeout, immagini corrotte, accesso negato, file bloccati, limiti), al massimo 50 per tipo. L'arresto manuale non produce più un errore per file.
- Il log su file a 2 MiB non si azzera più: passa a `diagnostica.old.log`.
- Versione mostrata sotto il logo realhcok, che è più piccolo.

## 0.2.3 - 2026-10-29

- Dopo l'eliminazione i file eliminati escono dalla lista dei segnalati e l'intestazione mostra il conteggio attuale ("File segnalati (28)"); restano nel conteggio delle eliminazioni, nella console e nel verbale.

## 0.2.2 - 2026-10-29

- Dopo l'eliminazione, il riepilogo conta solo le segnalazioni ancora presenti: "File con possibili copie", tipi, formati e verifica manuale coincidono con la lista e con "segnalazioni residue". Prima restavano i totali iniziali (es. residue 28 ma 29 ancora da verificare).

## 0.2.1 - 2026-10-29

- Nuova palette della GUI: grafite neutro con accento verde acqua e avvisi ambra, al posto del blu notte.
- Console sempre visibile in basso (ripiegabile) con gli eventi di scansione, eliminazione e verbale, e pulsante "Copia diagnostica" che mette negli appunti un report per gli sviluppatori: versione, sistema, CPU, grafica, fase, contatori, errori con il file che li ha causati, ultimi 200 eventi. Mai il contenuto dei documenti; il nome utente nei percorsi diventa `<utente>`.
- Log su file `%LOCALAPPDATA%\TracciaDoc\diagnostica.log` (massimo 2 MiB), separato dallo storico `audit.jsonl` che resta senza percorsi.

## 0.2.0 - 2026-10-29

- Selezione dei file segnalati (casella singola e "Seleziona tutti") ed eliminazione definitiva con conferma esplicita. Solo file regolari locali; nessun cestino.
- Ri-scansione di verifica sugli stessi percorsi e verbale finale con riepilogo della verifica iniziale e delle eliminazioni.
- L'esito "nessun documento rilevato dopo l'eliminazione" appare solo con copertura completa. Dopo l'eliminazione, la verifica originale richiede la ri-scansione.
- GUI su DirectX 12 tramite wgpu, con WARP sui PC senza driver video e riavvio automatico su OpenGL come ripiego.
- GUI: tema scuro fisso (con il tema chiaro di Windows il testo era illeggibile), simboli dei passi visibili, ridisegno ogni 1000 ms durante la scansione quando il rendering e su WARP.
- Collaudo su Windows 11 Enterprise LTSC 2024 x64 (VM senza driver video, DX12 su WARP, senza Mesa): `cargo test`, Clippy `-D warnings`, `validate.py` sul corpus sintetico; GUI a schermo con prove sintetiche, eliminazione con conferma, file modificato dopo l'analisi non eliminato, ri-scansione e verbale PDF finale con riepilogo e eliminazioni non riuscite.
- `edge_tests.py`: il caso `memory` usa un raster 5600x5600 (~94 MB decodificati) invece dell'OCR di una piccola immagine, il cui consumo dipendeva dalla macchina e restava sotto 64 MiB. Tutti i casi passano sulla VM Windows.
- Collaudo su PC fisico (Ryzen 7 PRO 5750G, Quadro RTX 4000): test, validazione ed edge test passati; 296 file sintetici in 21,1 s con 7 worker; GUI non massimizzata su DirectX 12 con la GPU NVIDIA, non WARP. Dettagli in `docs/VALIDAZIONE.md`.
- Non verificati: iGPU reali, ripiego OpenGL con finestra aperta.
- OCR in parallelo: N worker parser/OCR, ognuno processo separato con il proprio Windows Job e limite di memoria; default core fisici - 1 (minimo 1), `--workers` e Opzioni avanzate della GUI. Segnalazioni ordinate per percorso; il verbale dichiara il numero di worker.
- Prefiltro: niente OCR sulle immagini e pagine senza zone con tratti a contrasto tipo testo; dichiarate nel verbale come "scartate dal prefiltro", file contati come parziali. `--no-prefilter` e casella nella GUI per l'OCR su tutto. L'OCR gira solo sulla zona con tratti tipo testo piu un margine.
- Orientamento con OSD di Tesseract (`osd.traineddata` con hash verificato): una sola passata con confidenza >= 2, altrimenti le 4 rotazioni, prima dritta e poi quella suggerita.
- Stemma del Comune di Bondeno nella barra laterale e in testa al verbale; fonte in `assets/ui/FONTI-E-LICENZE.md`.
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
