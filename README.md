# TracciaDoc

MVP Windows x64 in Rust per trovare possibili copie di documenti d'identita nei file locali e produrre un verbale PDF. Repository **privato**, sorgenti MIT. Nessuna scansione automatica, telemetria o elaborazione cloud.

## Uso

Aprire il pacchetto portatile, selezionare dischi/cartelle, compilare operatore e riferimento, premere **Avvia verifica**. Viene eseguita una sola ricerca completa fra le capacita supportate, con OCR locale. Il footer mantiene accessibili avvio/interruzione e la scala UI.

L'esito mostra se sono state trovate possibili copie, quanti file, quali tipi (carta d'identita, passaporto, patente, tessera sanitaria) e quali formati. Ogni segnalazione mostra percorso completo, motivo, **Apri file**, **Mostra nella cartella** e stato della verifica manuale. Il PDF distingue l'analisi automatica da conferme/scarti dell'operatore. I percorsi sono inclusi per impostazione iniziale e si possono omettere.

**Nessun documento rilevato nell'ambito esaminato** e **verifica incompleta/interrotta** sono esiti distinti. Rilevamenti e incompletezza possono coesistere. Il verbale non certifica l'assenza assoluta e non implica approvazione Lepida.

## Implementazione

- egui/eframe 0.37, font Titillium Web e asset del kit incorporati, palette blu notte/petrolio, firma originale realhcok, icona PE e finestra.
- Enumerazione streaming, nessuna esclusione automatica di AppData/cestino/profili. Scelta di dischi, cartelle e volumi senza lettera; rete esclusa e rimovibili solo su selezione esplicita. Junction e collegamenti discendenti non attraversati, volumi montati da selezionare separatamente.
- Lettura per firma, anche con estensione errata/assente; PDFium per PDF, XML/ZIP con decompressione limitata per Office, decoder immagini limitati e Tesseract 5.5.3 ita+eng. Nei PDF viene eseguito OCR anche sulle pagine che contengono testo digitale.
- Parser persistente separato, un thread OCR, Windows Job con memoria limitata, timeout/annullamento e recupero. Non e una sandbox di sicurezza e mantiene i privilegi dell'operatore.
- CPU x64 di base, CRT statica. Automatico: AVX2/SSE solo dopo rilevamento runtime Rust e conferma della scelta Tesseract. Solo CPU: prodotto scalare generic per confronto, stesse regole. AVX-512/GPU/NPU non implementate. Nessun target-cpu=native globale.
- Nessuna cache persistente. I duplicati vengono rianalizzati conservando i percorsi. Audit JSONL limitato a eventi/conteggi, senza documenti, testo OCR o percorsi.

Formati e limiti esatti: [istruzioni](docs/ISTRUZIONI.txt). Componenti, licenze e valutazione acceleratori: [componenti](docs/COMPONENTI.md). Prove e misure: [validazione](docs/VALIDAZIONE.md).

## Specifiche e continuita

[docs/PROMPT.md](docs/PROMPT.md) conserva la specifica iniziale. Le richieste successive del 28 ottobre 2026 la aggiornano: una sola ricerca completa, integrazione del kit UI, percorsi e apertura dei file, verifica manuale e risultato esplicito in GUI/PDF. Il campo interno deep resta per rappresentare correttamente eventuali vecchi record preliminari; ogni nuova scansione lo imposta a true e la vecchia opzione CLI --quick viene rifiutata.

Il precedente MVP Python e stato consultato come riferimento per requisiti/test ed e rimasto separato e intatto. Nessun documento, database o report precedente e stato copiato qui.

## Build nativa Windows

Richiesti solo allo sviluppatore: Rust stabile >=1.95 per x64 MSVC, Visual Studio Build Tools con SDK Windows, PowerShell 7, Python 3.11+. La macchina destinataria usa esclusivamente il pacchetto portatile, senza strumenti di sviluppo o Internet. Preparazione delle dipendenze solo prima della distribuzione:

```powershell
pwsh -NoProfile -File scripts/prepare_runtime.ps1
& .\scratch\tooling\Scripts\python.exe scripts/make_fixtures.py
cargo test --release --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
& .\scratch\tooling\Scripts\python.exe scripts/package.py
& .\scratch\tooling\Scripts\python.exe scripts/validate.py
& .\scratch\tooling\Scripts\python.exe scripts/edge_tests.py
```

Output dist/TracciaDoc/TracciaDoc.exe, runtime, licenze e prove sintetiche. Il manifesto MSYS2 e bloccato in [runtime-lock.json](docs/runtime-lock.json); PDFium e modelli sono verificati con SHA256 nello script. Gli hash attestano integrita, non sono firme digitali. Il programma non e firmato Authenticode.

CLI facoltativa, scansione solo con percorso assoluto esplicito:

```powershell
.\dist\TracciaDoc\TracciaDoc.exe --scan '<CARTELLA_LOCALE>' '<VERBALE_NUOVO.pdf>'
```

Opzioni: --cpu, --omit-paths, --max-mb, --max-pages, --max-files, --timeout-secs, --memory-mb. --demo marca output sintetici. L'output preesistente non viene sovrascritto. --worker e il protocollo interno, non un'altra modalita utente.

I test e i benchmark usano soltanto il corpus sintetico; nessuna scansione dei dati reali e autorizzata dallo sviluppo.
