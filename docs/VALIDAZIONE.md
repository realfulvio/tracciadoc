# Validazione MVP 0.1.0 - 28 ottobre 2026

Verifiche effettuate nativamente su Windows 11 Pro build 26300, Rust 1.99.1 x64 MSVC. CPU AMD Ryzen 7 PRO 5750G, 8 core / 16 thread; 32 GB RAM; corpus su SSD NVMe iDsonix. GPU rilevate: NVIDIA Quadro RTX 4000, driver 32.0.16.1692, e VDGPUProxy Device 11.29.55.197. Nessun backend OCR GPU/NPU utilizzato e nessuna misura di memoria GPU dichiarata.

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

L'OCR assorbe circa 11,44 secondi dei 11,85 secondi AVX2; inizializzazione motore/modelli circa 171-176 ms. Produttivita OCR includendo i tentativi di rotazione: circa 1,66 immagini/pagine al secondo durante il tratto OCR (19 / 11,44); i singoli passaggi di rotazione non vengono contati come nuove pagine. Vantaggio AVX2 complessivo osservato circa 7%, con le stesse segnalazioni; campione piccolo, nessuna promessa di accelerazione su altre macchine. Il parallelismo e fissato a uno per limitare risorse e accessi casuali: nessun confronto di scalabilita con piu worker e dichiarato.

Avvio del pacchetto: 372 ms fino a Windows InputIdle in una misura; non equivale alla misura strumentale del primo frame visibile. EXE circa 11,7 MiB; pacchetto completo circa 63,5 MiB prima dell'aggiunta del piccolo PDF dimostrativo. GUI: circa 100-153 MiB di working set durante i controlli interattivi; picco campionato vicino a 153 MiB. Picco aggregato GUI+parser non campionato in modo sincronizzato; i dati del worker provengono da GetProcessMemoryInfo. Il pacchetto e verificato anche sul Desktop dopo la copia, con confronto degli hash.

## Collaudo interattivo Windows

GUI reale con font incorporati, colori, icona e firma originale; selezione del corpus sintetico, avvio, progressione, interruzione reale, conservazione dei risultati e stato VERIFICA INTERROTTA controllati. Chiusura durante scansione: parser terminato e nessun processo TracciaDoc residuo dopo la chiusura.

Percorsi completi e comandi provati: Apri file ha avviato Foxit e mostrato la carta sintetica; Mostra nella cartella ha aperto la cartella corretta con carta.pdf selezionato. Foxit ha mostrato anche un avviso relativo alla provenienza da applicazione non firmata: nessuna protezione e stata modificata o superata automaticamente. Conferma manuale del primo risultato effettuata nella GUI; riepilogo aggiornato a un confermato e PDF riesportato con lo stesso stato, controllato in lettura.

Generazione PDF dal dialogo nativo provata, non solo da CLI. PDF negativo, positivo, con errori e interrotto controllati; rendering e ispezione visiva effettuati con PDFium tramite pypdfium2, poiche Poppler non era disponibile. Con una segnalazione il verbale dimostrativo finale occupa una pagina. Quello interrotto con 11 segnalazioni e percorsi occupa due pagine. Nessun estratto OCR, numero documento o valore CF riconosciuto nei verbali.

Scale UI 100%, 125%, 150% e 200% controllate visivamente in finestre Windows reali. Finestra ridotta a 640x540 punti client: scroll e comandi nel footer accessibili. Si tratta del fattore di scala dell'app: non sono state cambiate le impostazioni DPI globali di Windows e non si dichiara un collaudo su quattro monitor/DPI nativi distinti. Le prove automatizzate non sostituiscono questi controlli.

## Limiti ancora presenti

Il corpus e piccolo e sintetico; nessuna scansione di documenti reali eseguita. Nessun confronto hardware su HDD, altro PC o CPU priva di AVX2; SSE, AVX-512, GPU e NPU non collaudati come percorsi accelerati alternativi. GIF/TIFF: prima immagine; DOC/XLS/PPT legacy, archivi generici, OLE/macro, cifratura e risorse esterne non analizzati. Rotazioni ortogonali provate, deskew e prospettiva non implementati. Documenti e parti illeggibili o oltre limite rendono la verifica incompleta.

Il parser mantiene i privilegi dell'operatore: Windows Job e processo separato non sono una sandbox di sicurezza. Nessuno snapshot o protezione contro modifiche concorrenti intenzionali del filesystem. Nessuna firma Authenticode o sottoscrizione digitale automatica del verbale. Firma del responsabile e accettazione Lepida restano decisioni esterne al programma.
