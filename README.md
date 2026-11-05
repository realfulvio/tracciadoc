<p align="center">
  <img src="docs/img/hero.png" alt="TracciaDoc: trova, rivedi, elimina, verifica" width="100%">
</p>

<p align="center">
  <a href="https://github.com/realfulvio/tracciadoc/releases/latest"><b>Scarica per Windows x64</b></a> ·
  <a href="CHANGELOG.md">Changelog</a> ·
  <a href="docs/ISTRUZIONI.txt">Istruzioni</a> ·
  <a href="docs/VALIDAZIONE.md">Validazione</a>
</p>

## Cos'è

**TracciaDoc** è un'app per Windows 10/11 che cerca sui dischi di un PC le **copie di documenti d'identità** rimaste lì per sbaglio: scansioni, foto e PDF di carta d'identità, passaporto, patente e tessera sanitaria, e le **fototessere**. Quando un ente rilascia documenti o gestisce pratiche, quelle copie finiscono facilmente in Download, Documenti o sul Desktop e ci restano.

Con TracciaDoc l'operatore le trova, le guarda, le elimina e produce un **verbale PDF** che documenta cosa è stato controllato e cosa è stato rimosso.

Tutto avviene **in locale**: niente cloud, niente telemetria, nessun file lascia il computer.

<p align="center">
  <img src="docs/img/flusso.png" alt="Scansiona, rivedi, elimina, verifica" width="100%">
</p>

## A cosa serve

- **Verifiche periodiche** delle postazioni che trattano documenti d'identità, ad esempio quelle per il rilascio della CIE.
- **Pulizia guidata**: l'elenco delle segnalazioni, con anteprima sfocata, si controlla a occhio prima di cancellare qualsiasi cosa.
- **Prova documentata**: il verbale riporta ambito, esiti, limiti, eliminazioni e la ri-scansione finale.

**Fototessere.** Un rilevatore di volti offline (YuNet, solo CPU, incorporato nel programma) segnala le immagini con un solo volto frontale, grande, su sfondo uniforme: la foto formato tessera. È sempre un *indizio da verificare*: dice "probabile" se lo sfondo è uniforme e "possibile" per un ritratto molto ravvicinato su sfondo non uniforme. Restano fuori un volto piccolo dentro una pagina, i volti di profilo, le foto di gruppo e i fogli con più fototessere. Costa circa 70-130 ms per immagine su CPU e si disattiva dalle opzioni avanzate. I semplici dati anagrafici sparsi nei file (un nome, un codice fiscale) non sono l'obiettivo e non vengono segnalati da soli.

<p align="center">
  <img src="docs/img/caratteristiche.png" alt="Offline, leggero, sotto controllo, verbale PDF" width="100%">
</p>

## Come funziona

1. **Scansiona.** Scegli dischi e cartelle: la ricerca è in sola lettura. PDF (anche scansionati), immagini e documenti Office/OpenDocument, compresi i vecchi Word `.doc`, vengono letti per firma, anche con estensione errata, ed esaminati con testo e **OCR offline italiano e inglese**.
2. **Rivedi.** Ogni segnalazione mostra tipo di documento, formato, percorso e motivo, con miniatura sfocata che si schiarisce al passaggio del mouse. Si apre il file o la cartella e si decide.
3. **Elimina.** Si spuntano i file da togliere e si conferma. L'eliminazione è definitiva (niente cestino) e riguarda solo file regolari locali; se un file cambia dopo l'analisi non viene toccato.
4. **Verifica.** TracciaDoc ripete la scansione sugli stessi percorsi e genera il verbale PDF con la situazione prima e dopo.

L'esito **«nessun documento rilevato»** compare solo a copertura completa: errori, accessi negati e limiti raggiunti sono dichiarati e rendono la verifica incompleta.

## Com'è fatto

<p align="center">
  <img src="docs/img/gui-aree-030.png" alt="TracciaDoc v0.3.0: nuova interfaccia con schede dei dischi, capacità e spazio occupato" width="100%">
</p>
<p align="center"><sub>Schermata reale della versione 0.3.0 per Windows: selezione delle aree, prima dell'avvio della scansione. Tema scuro. Nessun documento o dato personale visualizzato.</sub></p>

La versione 0.3.0 introduce tema scuro e chiaro, schede dei dischi con spazio occupato, avanzamento per fasi, filtri e pannello di dettaglio nella revisione e confronto prima/dopo nella verifica finale. La schermata qui sopra mostra la selezione delle aree; le vecchie immagini della 0.2.7 sono state rimosse dalla presentazione corrente.

<p align="center">
  <img src="docs/img/verbale-esempio.png" alt="Prima pagina del verbale PDF di esempio" width="40%">
</p>
<p align="center"><sub>Prima pagina del verbale PDF (esempio dimostrativo).</sub></p>

## Installazione e scansioni pianificate

Facoltative: TracciaDoc continua a funzionare anche solo come cartella portatile.

- **Installa** dalla sezione *Pianificazione* (o con `TracciaDoc.exe --install` da amministratore): il programma va in `Program Files\TracciaDoc`, l'icona compare vicino all'orologio a ogni accesso e TracciaDoc risulta in App installate. Si disinstalla da lì o con `--uninstall`.
- **Pianifica** quando eseguire la scansione: una volta, ogni giorno, ogni settimana o ogni mese, con orario e date. Parte anche senza utente collegato, a priorità bassa, e se il PC era spento appena viene acceso. Salva il verbale e **non cancella mai nulla da sola**: se trova documenti lo segnala e la revisione si fa aprendo TracciaDoc.
- **Arresta il PC a fine scansione** (attivo di default): solo dopo aver salvato il verbale (anche con verifica incompleta, che il verbale dichiara), mai se la scansione è interrotta, dopo 60 secondi di avviso annullabili dall'icona.
- **Si aggiorna da solo** dalle release di questo repository, con verifica SHA-256 e mai durante una scansione. Si può passare a "solo avviso" o disattivare.

| Comando | Cosa fa |
|---|---|
| `--install` / `--uninstall` | installa o rimuove (amministratore) |
| `--scheduled` | scansione pianificata senza finestra (la lancia l'Utilità di pianificazione) |
| `--tray` | icona nell'area di notifica (avviata all'accesso) |
| `--update` | cerca e installa l'ultima versione (amministratore) |
| `--update-from <zip>` | aggiorna da uno zip già scaricato, con `SHA256SUMS.txt` accanto (PC senza Internet) |
| `--version` | stampa la versione |

Configurazione, stato e verbali delle scansioni pianificate sono in `%ProgramData%\TracciaDoc`.

## Sotto il cofano

- **Rust**, eseguibile portatile con GUI nativa (egui) su DirectX 12; con PC senza driver video usa l'adattatore software WARP, con OpenGL come ripiego. Portatile oppure installabile; nessun Python. Internet serve solo per l'aggiornamento automatico, disattivabile.
- **OCR Tesseract 5 offline** e PDFium incorporati. Più worker in parallelo (default: core fisici − 1), ognuno in un processo separato con Windows Job, limite di memoria e timeout.
- **Prefiltro** che scarta le immagini senza zone di testo e **orientamento automatico** delle scansioni ruotate: l'OCR lavora solo dove serve.
- **AVX2/SSE** scelti a runtime con ripiego su CPU generica. L'uso di GPU/iGPU/NPU è valutato solo se un benchmark dimostra un vantaggio reale.
- Nessuna cache persistente. Lo storico `audit.jsonl` contiene solo eventi e conteggi, mai percorsi o contenuti.

I dettagli sulle prestazioni misurate, il corpus di prova e i metodi di misura sono in [docs/VALIDAZIONE.md](docs/VALIDAZIONE.md); componenti e licenze in [docs/COMPONENTI.md](docs/COMPONENTI.md).

## Provarlo in due minuti

1. Scarica lo ZIP dall'ultima [release](https://github.com/realfulvio/tracciadoc/releases/latest) ed **estrai tutta la cartella** (l'EXE deve restare accanto a `runtime` e agli altri file).
2. Avvia `TracciaDoc.exe` e premi **Carica prove sintetiche**: il pacchetto include facsimili inventati, anche volutamente problematici, quindi su quel corpus un esito incompleto è normale.
3. Per una verifica vera scegli i dischi o le cartelle; per leggere i profili di altri utenti possono servire privilegi amministrativi.

La verifica del download è nel file `SHA256SUMS.txt` della release. L'EXE non è firmato.

## Cosa non fa

TracciaDoc è uno strumento di aiuto, non una garanzia.

- Il riconoscimento è euristico: possibili falsi positivi e falsi negativi. Le segnalazioni vanno viste da una persona.
- Il verbale documenta l'ambito effettivamente controllato e **non certifica l'assenza assoluta** di documenti, né implica l'approvazione di terzi.
- Non sono letti: HEIC/HEIF, XLS e PPT legacy, archivi generici, file cifrati, oggetti OLE e macro (quelli con estensione moderna rendono la verifica incompleta). GIF solo il primo fotogramma (i TIFF multipagina sono letti tutti).
- Il parser gira con i privilegi dell'operatore: processo separato e Windows Job non sono una sandbox di sicurezza. Non c'è uno snapshot coerente dell'intero disco.
- Il PDF può contenere percorsi e giudizi manuali: va conservato in una cartella riservata, oppure si omettono i percorsi in esportazione.

## Compilare dai sorgenti

Servono solo allo sviluppatore: Rust stabile ≥ 1.95 (x64 MSVC), Visual Studio Build Tools con SDK Windows, PowerShell 7 e Python 3.11+.

```powershell
python -m venv scratch/tooling
& .\scratch\tooling\Scripts\python.exe -m pip install -r scripts/requirements-build.txt
pwsh -NoProfile -File scripts/prepare_runtime.ps1
& .\scratch\tooling\Scripts\python.exe scripts/make_fixtures.py
cargo test --release --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
& .\scratch\tooling\Scripts\python.exe scripts/package.py
& .\scratch\tooling\Scripts\python.exe scripts/validate.py
& .\scratch\tooling\Scripts\python.exe scripts/edge_tests.py
```

L'uscita è in `dist/TracciaDoc/`. Il manifesto del runtime MSYS2 è bloccato in [docs/runtime-lock.json](docs/runtime-lock.json); PDFium e modelli OCR sono verificati con SHA256.

Scansione da riga di comando, solo con percorso assoluto esplicito:

```powershell
.\dist\TracciaDoc\TracciaDoc.exe --scan '<CARTELLA_LOCALE>' '<VERBALE_NUOVO.pdf>'
```

Opzioni: `--cpu`, `--omit-paths`, `--max-mb`, `--max-pages`, `--max-files`, `--timeout-secs`, `--memory-mb`, `--workers`, `--no-prefilter`. `--demo` marca gli output sintetici; un output già esistente non viene sovrascritto.

Test e benchmark usano **solo il corpus sintetico** in `fixtures/`: facsimili inventati, nessun documento reale.

## Licenze

Sorgenti propri sotto [licenza MIT](LICENSE). Dipendenze, font e modelli OCR mantengono le rispettive licenze, incluse nel pacchetto. Il marchio **realhcok** è dell'autore del progetto; il repository non contiene stemmi o loghi di enti; un logo proprio si aggiunge come `logo-ente.png` accanto all'EXE (vedi [assets/ui/FONTI-E-LICENZE.md](assets/ui/FONTI-E-LICENZE.md)).

---

<p align="center"><sub>TracciaDoc · powered by <b>real&gt;hcok_</b></sub></p>
