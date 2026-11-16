# Verifiche 0.5.2 - 16 novembre 2026

Release correttiva di copertura e rendicontazione. Le prove seguenti sono distinte dal collaudo umano del riconoscimento: non costituiscono una certificazione di assenza di documenti. Le sezioni successive conservano le misure storiche delle versioni precedenti.

## Build e prove automatiche

Windows 11 x64, build MSVC nativa, Rust 1.99.1. CRT statica e target x86_64 standard, senza `target-cpu=native` o AVX imposta globalmente. Dipendenze Rust e runtime OCR/PDF invariati rispetto alla 0.5.1; file del runtime confrontati tramite SHA-256.

- `cargo test --release --locked`: **48 test passati**.
- `cargo clippy --release --all-targets --locked -- -D warnings` e build release: passati.
- Pacchetto portatile: `scripts/validate.py`, `scripts/edge_tests.py` e le 11 scansioni di `scripts/collaudo_regressions.py` passati. Sulle 36 fixture tracciate del pacchetto: 29 positivi rilevati, 3 negativi, 4 casi limitati; CPU generica e AVX2 producono le stesse motivazioni. Questo insieme non misura l'accuratezza su documenti reali.
- Regressioni verificate: ZIP/posta dichiarati incompleti, file vuoto, immagine rinominata sotto 20 KiB, estensioni TXT/download, ADS su file e cartella, percorsi Unicode e loro omissione, summary leggibile da PowerShell 5.1. Tre pagine PDF di solo testo evitano l'OCR; un PDF misto con immagine positiva viene ancora segnalato tramite OCR.
- Timeout ripetuto una sola volta e conteggi senza duplicazioni, memoria insufficiente e recupero del worker, accesso negato, file bloccato, limiti, junction, radice assente, audit e annullamento: verificati dagli script. Le ACL della sola fixture di prova vengono ripristinate.
- **42 mutazioni deterministiche** di PDF, DOCX, ODT, PPTX, XLSX, ZIP ed EML: processo concluso senza crash/hang, 42 file contabilizzati, verifica incompleta con motivazioni. Non equivalgono a fuzzing continuo dei parser Windows.
- `cargo audit` 0.22.2, database RustSec del 15 novembre 2026: zero vulnerabilità segnalate; restano tre avvisi di manutenzione per `zbus_names`, `zvariant` e `zvariant_derive`.

## Canary, eliminazione e interfaccia

Documento sintetico riconosciuto nel temporaneo dell'utente e in esecuzione SYSTEM. Nella sessione interattiva di un utente standard, il file è finito realmente nel Cestino (hash confrontato) ed è stato riconosciuto. Un file EFS realmente cifrato è stato escluso con motivo e verifica incompleta. Nessun contenuto ADS è stato letto: la presenza viene dichiarata come lacuna.

Un banco di prova esterno include gli effettivi `delete.rs` e `model.rs`: scansione con una segnalazione, eliminazione, ri-scansione con zero segnalazioni e senza incompletezza. Questa prova non verifica la selezione e conferma nella GUI né la recuperabilità dal disco. Il verbale chiarisce che l'eliminazione non sovrascrive lo spazio liberato.

La GUI 0.5.2 è stata avviata e osservata nella sessione Windows interattiva, con rendering DirectX 12 WARP. Il ciclo completo scansione/revisione/eliminazione dalla GUI **non è stato ricollaudato per questa versione**; i risultati CLI e del banco esterno non lo sostituiscono.

## Fototessere: corpus esterno sintetico

250 immagini CGI: 90 foto singole positive, 60 fogli da quattro fototessere e 100 ritratti negativi secondo il manifest originario. Controllati visivamente 24 campioni: alcune etichette positive comprendono pose oblique, sfondi scuri e forti degradi; positivi e negativi possono essere simili. Il corpus non è una verifica di conformità fotografica e non contiene documenti reali. Manifest ed etichette sono rimasti invariati.

Le cinque scansioni sequenziali (0.5.1 e 0.5.2 automatiche, entrambe in CPU generica, ripetizione CPU della 0.5.2) producono **esattamente gli stessi 39 percorsi segnalati**. Nessuna modifica alle soglie del riconoscimento è stata tarata su questi dati.

| Gruppo | Segnalati / totale | Tasso | Wilson 95% |
|---|---:|---:|---:|
| Foto singole positive | 13/90 | 14,4% | 8,6–23,2% |
| Fogli con quattro foto, esclusi dal prodotto | 0/60 | 0% | 0–6,0% |
| Ritratti negativi (falsi positivi secondo le etichette) | 26/100 | 26,0% | 18,4–35,4% |

Fra le foto singole: 12/30 pulite, 1/30 nel degrado intermedio e 0/30 nel più forte. Bootstrap a cluster sulle 30 identità delle foto singole, 2.000 ricampionamenti: 7,8–22,2%; sui negativi: 17–35%. Il valore aggregato originario 13/150 (8,7%) include i fogli multipli fuori ambito e non deve sostituire la misura sulle singole. I cinque passaggi non sono campioni indipendenti.

Tempi del motore con nove worker, senza altri build o benchmark concorrenti: automatico 43,48 s (0.5.1) e 43,49 s (0.5.2); CPU 46,85 s e 46,95 s, ripetizione 46,88 s. Nessuna cache applicativa persistente; cache filesystem non svuotata. Non emerge un miglioramento di riconoscimento o velocità su questo insieme. La quota di falsi positivi supera il riferimento del 10% riportato nel passaggio di consegne del collaudo; il riconoscimento delle fototessere resta un limite aperto.

Il binario definitivo distribuito è stato nuovamente provato: 43,14 s in automatico, gli stessi 39 percorsi segnalati. L'ultima correzione riguarda la conservazione dei dettagli di copertura nelle scansioni pianificate e il testo del verbale, senza cambiare il riconoscimento.

## Corpus esterno di 4.096 file e riservatezza degli output

Sul binario definitivo, con nove worker, limite 120 s per tentativo e 768 MiB per worker:

| Percorso | Secondi del motore | Segnalazioni | Timeout | Esito tecnico |
|---|---:|---:|---:|---|
| CPU generica | 365,56 | 191 | 0 | Incompleto per le lacune dichiarate |
| Automatico AVX2 | 342,88 | 191 | 0 | Incompleto per le lacune dichiarate |

Gli insiemi dei percorsi segnalati sono identici, anche rispetto alla prima build correttiva (330,28 s). Tutti i 4.096 hash sono invariati; il manifest JSON è contato separatamente come un ulteriore file ignorato. Conteggi finali: 3.675 analizzati, 386 parziali, 35 saltati; 2.517 pagine PDF di solo testo senza OCR. Picco aggregato campionato del working set circa 1,72 GiB, massimo dieci processi. Nessun altro build o benchmark concorrente durante le misure; cache filesystem non svuotata.

Secondo le etichette originarie: **180/546 positivi rilevabili trovati (32,97%)**, Wilson 95% 29,15–37,02%, bootstrap a cluster 23,44–42,30%. Falsi positivi: 0/3.100 negativi normali e 11/440 difficili. Nessuna segnalazione delle quattro corse precedenti 0.5.1 viene persa; rispetto alla migliore si aggiungono quattro PDF multipagina positivi. Sono facsimili disegnati con pochi cluster per classe: questi numeri non stimano l'accuratezza su documenti reali. Fra i 366 miss, 63 hanno una lacuna per file nel verbale; per gli altri la causa non è determinata da questa prova.

Confronto controllato di un PDF ordinario di quattro pagine, un worker, ordine vecchio/nuovo/vecchio/nuovo: 0.5.1 18,46 e 18,49 s; 0.5.2 0,043 e 0,041 s, stesso esito e zero pagine OCR nel nuovo. I tempi sono del motore. Le corse complete storiche avevano carichi diversi e non sono usate per attribuire un fattore di accelerazione controllato.

Cercati 65 token sintetici, compresi cinque canary dedicati, e il pattern del codice fiscale in **272 output**: 76 PDF, 118 JSON, 22 log, 53 TXT e 3 audit JSONL. Zero occorrenze e zero errori di lettura. Controllati testo/metadati PDF e testo UTF-8/UTF-16; esclusi gli input sintetici e i risultati del valutatore indipendente che contengono le etichette. Nessun dump TracciaDoc disponibile nelle quattro posizioni di profilo controllate. Non è una verifica delle cache di sistema, delle immagini incorporate o dello spazio non allocato.

## Copertura del codice e fuzzing portabile

Sul codice dei moduli portabili, invariato rispetto alla 0.5.1: 13 test unitari su Linux; righe di produzione eseguite `rules.rs` 163/170 (95,88%), `doc.rs` 265/318 (83,33%), escludendo i test. Non è copertura della scansione Windows, della GUI o dell'intero parser. Branch coverage e MC/DC non misurati.

Fuzzing aggiuntivo di `doc::read` con due DOC sintetici validi come semi: 491.330 esecuzioni in 121 s, nessun crash/hang/OOM, picco RSS 528 MiB; input massimo 1 MiB, timeout 20 s, limite RSS 2 GiB. Questi risultati non attestano l'assenza di vulnerabilità.

## Verifiche ancora mancanti

Set cieco umano con secondo valutatore, collaudo Windows 10, OneDrive con veri placeholder, ciclo GUI completo della 0.5.2, recupero da spazio non allocato e copie VSS non verificati. I documenti completi delle soglie del collaudo esterno non erano accessibili: nessun PASS complessivo attribuito ai relativi gate. L'aggiornamento corregge lacune dimostrate senza dichiarare risolti i limiti di riconoscimento.

---

# Validazione MVP 0.1.0 - 28 ottobre 2026

Verifiche effettuate nativamente su Windows 11 Pro build 26300, Rust 1.99.1 x64 MSVC. CPU AMD Ryzen 7 PRO, 8 core / 16 thread; 32 GB RAM; corpus su SSD NVMe. GPU rilevate: una NVIDIA dedicata e un adattatore virtuale. Nessun backend OCR GPU/NPU utilizzato e nessuna misura di memoria GPU dichiarata.

Rilevamento effettivo via macro Rust: SSE4.1, AVX, AVX2 e FMA disponibili; AVX512F/BW/DQ/VL/VNNI non disponibili. Il percorso numerico OCR `avx2` e stato confermato dalla variabile del motore distribuito, non dedotto dal solo inventario hardware. AVX-512 non viene selezionata. Il build Rust non impone AVX globalmente e incorpora la CRT MSVC statica.

## Test automatici e pacchetto

- `cargo test --release --locked`: 6 test passati (regole, CF, MRZ, esiti e minimizzazione PDF, blocco apertura di estensioni eseguibili mascherate, entita XML Office e risorse esterne).
- `cargo clippy --all-targets --locked -- -D warnings`: passato.
- Build release nativa passata; icone ICON/GROUP_ICON e metadati presenti nelle risorse PE.
- Pacchetto distribuito provato con tutti i runtime risolti relativamente all'EXE: nessun Rust/Python/Tesseract di sistema necessario al controllo.
- SHA256 di tutti i file sintetici invariati prima/dopo le scansioni. OCR interamente in memoria; nessuna estrazione di archivi su disco.
- Esclusivo accesso Windows (file bloccato), accesso negato tramite ACL su un solo file sintetico, dimensione massima, numero massimo di file, timeout, memoria insufficiente, radice assente e junction esterna: tutti producono incompletezza. ACL ripristinata nel blocco finally del test. Dopo timeout/memoria il file testuale successivo viene ancora analizzato e segnalato.
- Annullamento CLI a 250 ms: terminazione in circa 0,27 secondi di motore e 0,39 secondi complessivi, esito interrotto. Il dato e del corpus sintetico, non una garanzia universale di latenza.
- Vecchia opzione CLI `--quick` rifiutata; GUI senza scelta Rapida/Approfondita. Ogni nuova verifica usa `deep=true`; i record storici non vengono riscritti.

## Qualita sul corpus sintetico

37 file etichettati: 27 positivi analizzabili, 6 negativi e 4 casi con limiti. Fra i 4 casi limitati c'e un ulteriore positivo cifrato che non viene analizzato: **il totale dei positivi noti e 28 e il totale individuato e 27**. Gli altri limiti sono PDF corrotto, DOC legacy non supportato e ZIP generico non aperto.

| Classe | Positivi analizzabili | Rilevati | Mancati fra gli analizzabili | Non analizzato cifrato |
|---|---:|---:|---:|---:|
| Carta d'identita | 8 | 8 | 0 | 1 |
| Passaporto | 7 | 7 | 0 | 0 |
| Patente | 6 | 6 | 0 | 0 |
| Tessera sanitaria | 6 | 6 | 0 | 0 |

I 6 negativi (testo ordinario, richiesta di allegare un documento, email e codice fiscale isolati, PNG e PDF ordinari) non generano segnalazioni. Il corpus include PDF testuali/raster/misti, PNG/JPEG/BMP, rotazioni 90/180/270 gradi, basso contrasto, DOCX/XLSX/PPTX, ODT/ODS/ODP e DOCX con immagine incorporata. PDF rinominato BIN e PNG senza estensione vengono riconosciuti dalla firma. I documenti Office sono generati con librerie di scrittura dei rispettivi formati, non solo ZIP rinominati.

I percorsi `generic` e `avx2` producono le stesse motivazioni per tutti i file. Questi sono test funzionali su facsimili con testo pulito: non misurano accuratezza statistica su documenti reali, non coprono tutte le impaginazioni o qualita delle scansioni e non dimostrano assenza di falsi negativi nel mondo reale.

## Prestazioni misurate

Un parser persistente, un thread OCR, limite 64 MiB/file, 40 pagine/immagini, timeout 120 secondi/file e 768 MiB di memoria privata worker. Nessuna cache applicativa. La cache filesystem non e stata svuotata: freddo/caldo indica il processo, non un disco freddo. Carichi concorrenti della postazione non sono stati sospesi.

| Percorso | Motore completo | Con PDF e avvio processo | File/s | Pagine/immagini OCR | Picco working set worker |
|---|---:|---:|---:|---:|---:|
| Automatico AVX2, processo nuovo | 11,849 s | 11,972 s | 3,12 | 19 | 158,3 MiB |
| Automatico AVX2, ripetizione | 11,950 s | 12,074 s | 3,10 | 19 | 158,4 MiB |
| CPU generic | 12,784 s | 12,907 s | 2,89 | 19 | 157,8 MiB |

L'OCR assorbe circa 11,44 secondi dei 11,85 secondi AVX2; inizializzazione motore/modelli circa 171-176 ms. Produttivita OCR includendo i tentativi di rotazione: circa 1,66 immagini/pagine al secondo durante il tratto OCR (19 / 11,44); i singoli passaggi di rotazione non vengono contati come nuove pagine. Vantaggio AVX2 complessivo osservato circa 7%, con le stesse segnalazioni; campione piccolo, nessuna promessa di accelerazione su altre macchine. Nella 0.1.0 il parallelismo era fissato a uno; dalla 0.2.0 i worker sono configurabili (sezione in fondo).

Avvio del pacchetto: 372 ms fino a Windows InputIdle in una misura; non equivale alla misura strumentale del primo frame visibile. EXE circa 11,7 MiB; pacchetto completo circa 63,5 MiB prima dell'aggiunta del piccolo PDF dimostrativo. GUI: circa 100-153 MiB di working set durante i controlli interattivi; picco campionato vicino a 153 MiB. Picco aggregato GUI+parser non campionato in modo sincronizzato; i dati del worker provengono da GetProcessMemoryInfo. Il pacchetto e verificato anche sul Desktop dopo la copia, con confronto degli hash.

## Collaudo interattivo Windows

GUI reale con font incorporati, colori, icona e firma originale; selezione del corpus sintetico, avvio, progressione, interruzione reale, conservazione dei risultati e stato VERIFICA INTERROTTA controllati. Chiusura durante scansione: parser terminato e nessun processo TracciaDoc residuo dopo la chiusura.

Percorsi completi e comandi provati: Apri file ha avviato Foxit e mostrato la carta sintetica; Mostra nella cartella ha aperto la cartella corretta con carta.pdf selezionato. Foxit ha mostrato anche un avviso relativo alla provenienza da applicazione non firmata: nessuna protezione e stata modificata o superata automaticamente. Conferma manuale del primo risultato effettuata nella GUI; riepilogo aggiornato a un confermato e PDF riesportato con lo stesso stato, controllato in lettura.

Generazione PDF dal dialogo nativo provata, non solo da CLI. PDF negativo, positivo, con errori e interrotto controllati; rendering e ispezione visiva effettuati con PDFium tramite pypdfium2, poiche Poppler non era disponibile. Con una segnalazione il verbale dimostrativo finale occupa una pagina. Quello interrotto con 11 segnalazioni e percorsi occupa due pagine. Nessun estratto OCR, numero documento o valore CF riconosciuto nei verbali.

Scale UI 100%, 125%, 150% e 200% controllate visivamente in finestre Windows reali. Finestra ridotta a 640x540 punti client: scroll e comandi nel footer accessibili. Si tratta del fattore di scala dell'app: non sono state cambiate le impostazioni DPI globali di Windows e non si dichiara un collaudo su quattro monitor/DPI nativi distinti. Le prove automatizzate non sostituiscono questi controlli.

## Limiti ancora presenti

Il corpus e piccolo e sintetico; nessuna scansione di documenti reali eseguita. Nessun confronto hardware su HDD, altro PC o CPU priva di AVX2; SSE, AVX-512, GPU e NPU non collaudati come percorsi accelerati alternativi. GIF: primo fotogramma; XLS/PPT legacy, archivi generici, OLE/macro, cifratura e risorse esterne non analizzati. Rotazioni ortogonali provate, deskew e prospettiva non implementati. Documenti e parti illeggibili o oltre limite rendono la verifica incompleta.

Il parser mantiene i privilegi dell'operatore: Windows Job e processo separato non sono una sandbox di sicurezza. Nessuno snapshot o protezione contro modifiche concorrenti intenzionali del filesystem. Nessuna firma Authenticode o sottoscrizione digitale automatica del verbale. Firma del responsabile e accettazione Lepida restano decisioni esterne al programma.

## 0.2.0: OCR in parallelo, prefiltro e orientamento - 29 ottobre 2026

Misure sulla VM di test: Windows 11 Enterprise LTSC 2024 x64 su KVM, 4 vCPU (4 core fisici rilevati), 7,9 GB RAM, scheda video virtuale QXL. **Nessuna GPU reale**: la GUI usa DirectX 12 su WARP. Rust 1.99.1 MSVC, CLI release, corpus solo sintetico, file in cache (nessuna misura a disco freddo). Default: 3 worker.

### Corpus di validazione (41 file)

`validate.py`: generic e AVX2 con le stesse motivazioni, **TP 29, TN 6, limitati 6**, nessun FN o FP. Ai 37 file della 0.1.0 si aggiungono due foto senza testo (rumore sfumato 1600x1200 e 2400x1800, scartate dal prefiltro e quindi "limitate"), `carta-ruotata-90.png` e `documento-fotografato.jpg` (patente ruotata di 270 gradi incollata in una foto 2400x1800). Scansione CLI completa: 9,8-10,0 s automatico, 10,7 s generic. `edge_tests.py`: tutti i casi passano; il caso timeout usa un mosaico 3x3 di `appunti.png`, perche con OSD l'immagine singola richiede una sola passata e scende sotto 1 s.

### Benchmark per fasi, prima e dopo

Corpus: `prove-sintetiche` 0.1.0 ripetuto 8 volte (296 file) e 8 foto senza testo. Prima = main 005dcc5 con la sola strumentazione per fasi; dopo = questa versione. Tempi di motore dal JSON della CLI.

| Corpus | Configurazione | Tempo | Core usati | Passate OCR | Rilevamenti |
|---|---|---:|---:|---:|---:|
| 296 file | prima, AVX2 | 96,2 s | 0,97 | 248 | 216 |
| 296 file | prima, generic | 101,6 s | 0,98 | 248 | 216 |
| 296 file | dopo, AVX2, 3 worker | **44,8 s** | 2,92 | 208 | 216 |
| 296 file | dopo, generic, 3 worker | 48,0 s | 2,89 | 208 | 216 |
| 296 file | dopo, 1 worker | 96,4 s | 0,98 | 208 | 216 |
| 296 file | dopo, senza prefiltro | 49,0 s | 2,91 | 208 | 216 |
| 8 foto senza testo | prima, AVX2 | 203,0 s | 0,99 | 32 | 0 |
| 8 foto senza testo | prima, generic | 217,5 s | 0,98 | 32 | 0 |
| 8 foto senza testo | dopo, prefiltro attivo | **0,5 s** | - | 0 (8 scartate) | 0 |
| 8 foto senza testo | dopo, senza prefiltro | 80,3 s | 2,65 | 29 | 0 |
| documento in foto (1 file) | prima della zona di ritaglio | 64,8 s | 1 | 4 | 1 |
| documento in foto (1 file) | dopo | 9,4 s | 1 | 3 | 1 |

Ripartizione prima: OCR 96,8-99,9% del tempo, lettura <= 1,4%, decodifica immagini <= 0,3%, rasterizzazione PDF 0,6%, parsing e regole <= 0,5%, IPC ed enumerazione <= 0,3%. Dopo: OCR (OSD incluso) 95% del lavoro dei worker; prefiltro 3,8 s di CPU complessiva su 152 immagini/pagine; OSD 25-37 s di CPU complessiva (circa 170 ms per immagine).

Lettura dei numeri: sul corpus testuale il guadagno viene dal parallelismo (2,15x con 3 worker su 4 vCPU); l'OSD toglie il 16% delle passate ma ne restituisce il costo, perche e affidabile (confidenza >= 2) solo su 16 immagini su 152. Sui file in cui il testo occupa una parte della foto il ritaglio alla zona con testo e il guadagno principale. Le foto senza testo, il caso peggiore della 0.1.0 (circa 28 s ciascuna su un core), non vanno piu all'OCR. AVX2 rispetto a generic: 5-7% come nella 0.1.0.

### Prefiltro: taratura e limiti

Immagine ridotta a 1024 px, griglia 8x8, quota di pixel con salto di luminosita > 32 verso destra o in basso. Sul corpus: zone con testo da 0,10 (BMP a basso contrasto) a 0,26; foto lisce 0,00. Soglia 0,02 per zona: basta una zona tipo testo per eseguire l'OCR, e solo su quella zona piu un riquadro di margine. Tarata solo su facsimili sintetici e foto sintetiche lisce: foto reali con dettagli nitidi superano la soglia e vanno comunque all'OCR (nessun risparmio, nessuna perdita), mentre testo molto sbiadito (salti <= 32 su 255) o piu piccolo di quanto resta leggibile a 1024 px puo essere scartato. Per questo lo scarto e dichiarato nel verbale, il file conta come parziale e `--no-prefilter` resta disponibile.

### OSD

Sul corpus OSD restituisce angoli sbagliati con confidenza bassa (es. `carta.png` dritta data a 180 gradi con 1,57; `generico-ruotato-180.jpg` data a 0 gradi con 0,71). Per questo una sola passata solo con confidenza >= 2; sotto soglia restano le 4 rotazioni, prima dritta e poi quella suggerita. Nessun falso negativo introdotto sul corpus.

### GUI

GUI collaudata a schermo sulla VM (DirectX 12 su WARP, UI Automation e clic reali): scansione del corpus da 41 file in 16-18 s con 2 foto scartate, eliminazione con conferma (file modificato dopo l'analisi non eliminato), ri-scansione finale e verbale PDF dal dialogo di salvataggio, con stemma in testa, "3 worker" e "scartate dal prefiltro: 2".

### PC fisico con GPU reale

Collaudo su un PC fisico: Ryzen 7 PRO (8 core fisici, 16 logici), 32 GB RAM, GPU NVIDIA dedicata, SSD NVMe, Windows 11 Pro. Pacchetto ricompilato da zero dal commit cd0214f, stessi passi del README. `cargo test --release` (7 test), Clippy `-D warnings`, `validate.py` (TP 29, TN 6, limitati 6 in generic e AVX2; scansione completa 7,9-8,0 s automatico, 9,7 s generic) e `edge_tests.py` passati. Corpus solo sintetico, file in cache.

| Corpus | Configurazione | Tempo | Core usati | Rilevamenti |
|---|---|---:|---:|---:|
| 296 file | default (7 worker), AVX2 | **21,1 s** | 6,62 | 216 |
| 296 file | 7 worker, generic | 22,6 s | 6,62 | 216 |
| 296 file | 3 worker, AVX2 | 43,4 s | 2,97 | 216 |
| 296 file | 3 worker, generic | 46,0 s | 2,97 | 216 |
| 296 file | 1 worker | 112,9 s | 0,99 | 216 |
| 296 file | senza prefiltro | 22,5 s | 6,64 | 216 |
| 8 foto senza testo | prefiltro attivo | 0,6 s | - | 0 (8 scartate) |
| 8 foto senza testo | senza prefiltro | 85,4 s | 4,19 | 0 |
| 41 file | default (7 worker) | 8,5 s | 3,63 | 29 |
| 41 file | 3 worker | 8,7 s | 2,94 | 29 |

Da 1 a 7 worker 5,35x. Sul corpus a 41 file il tempo e dominato da `documento-fotografato.jpg`, per cui piu worker non aiutano. Con 7 worker l'inizializzazione dei motori OCR somma circa 3,3 s di CPU (circa 1 s con 3). Un worker su questo PC e piu lento di una vCPU della VM (112,9 s contro 96,4 s). Le 8 foto senza testo sono le due del corpus copiate 4 volte.

GUI in finestra non massimizzata (1196x859): wgpu carica sia il driver NVIDIA sia WARP durante l'enumerazione, ma il processo alloca memoria dedicata (194-224 MiB) solo sulla GPU NVIDIA e nulla sugli altri adattatori: rendering sulla GPU reale. Scansione delle prove sintetiche dalla GUI: 41 file, 29 rilevamenti, 7,5 s.

### Non verificato

iGPU reali (sul PC fisico l'iGPU non e esposta); HDD o disco freddo; foto reali (il prefiltro e tarato su immagini sintetiche); ripiego OpenGL con finestra aperta.
