# TracciaDoc - prompt completo di realizzazione

Realizza **TracciaDoc**, un'applicazione Windows gratuita e open source in **Rust**, per verificare la presenza di possibili copie di documenti d'identità nei dischi locali del computer. Il repository di progetto è **realfulvio/tracciadoc** su GitHub e deve restare **privato**. Usa il checkout esistente: non creare altri repository e non cambiare la visibilità.

L'utilizzatore è un amministratore IT di un Comune. Serve un programma semplice da usare, esteticamente curato, **molto veloce, leggero ed efficace**, che produca un verbale PDF del controllo, conservabile ed eventualmente presentabile a Lepida su richiesta.

Procedi fino a un MVP funzionante, compilato e verificato. Non fermarti a progettazione, mockup, codice non compilato o dichiarazioni di funzionalità non provate. Gestisci autonomamente le normali decisioni tecniche, rispettando i confini indicati qui e nelle istruzioni del workspace.

## 1. Ambito operativo

- Il programma viene avviato direttamente sul PC da controllare, eventualmente con privilegi amministrativi per accedere ai profili degli altri utenti.
- Permetti la selezione di dischi locali, singole cartelle e volumi montati senza lettera. Mostra cosa sarà controllato e cosa resterà escluso.
- Distingui dischi fissi, rimovibili, unità di rete e volumi non disponibili. Non includere automaticamente unità di rete o supporti rimovibili.
- Non servono Active Directory, scansione remota, SMB, WinRM, agenti o servizi residenti.
- Tutte le operazioni sui documenti devono essere in sola lettura: nessuna cancellazione, modifica, quarantena, spostamento o invio di file.
- Analisi interamente locale, funzionamento offline durante le verifiche, nessuna telemetria e nessuna API a pagamento.
- Non avviare scansioni sui dati reali di questo PC o dell'ente senza una mia richiesta esplicita. Per sviluppo e collaudo usa fixture sintetiche.

Se trovi un precedente MVP Python, utilizzalo come riferimento per requisiti e casi di test, conservandolo. Realizza la versione Rust separatamente: non spostare o sovrascrivere il vecchio progetto e non copiarne indiscriminatamente dati, database o report nel nuovo repository.

## 2. Priorità: velocità, leggerezza ed efficacia

Obiettivi centrali: avvio rapido, interfaccia sempre reattiva, consumo contenuto di RAM e CPU, distribuzione compatta e nessun processo lasciato in background dopo la chiusura.

Non ottenere velocità ignorando silenziosamente documenti o riducendo la qualità del riconoscimento. Ogni compromesso deve essere misurato e visibile.

Progetta una pipeline progressiva:

1. Enumerazione efficiente del filesystem e raccolta dei soli metadati necessari.
2. Identificazione del formato, verificando quando possibile anche il contenuto oltre all'estensione. Nella modalità approfondita non ignorare automaticamente i file senza estensione o con estensione errata: prevedi riconoscimento leggero della firma del formato o registra la lacuna.
3. Estrazione testuale dai formati supportati.
4. Analisi degli indizi e della struttura documentale.
5. OCR e ulteriori elaborazioni dove necessari.

Usa streaming, code con capacità limitata, buffer riutilizzabili e parallelismo controllato. Evita letture duplicate, copie complete non necessarie e la creazione di un nuovo processo pesante per ogni pagina se puoi mantenere un pool isolato e recuperabile.

Adatta il parallelismo alle risorse e al tipo di disco: saturare un HDD con accessi casuali concorrenti può rallentare il controllo. Evita anche di moltiplicare i thread del programma per quelli interni del motore OCR. Lascia risorse sufficienti alla normale attività dell'utente.

Gestisci i duplicati preservando tutti i percorsi coinvolti. Non considerare due file identici sulla sola base di nome, dimensione e data. Un'eventuale cache persistente deve essere facoltativa, protetta, invalidata quando cambiano contenuto, motore, regole o impostazioni e chiaramente rappresentata nel verbale. Nella verifica approfondita finale consenti di rianalizzare tutto senza cache.

Offri due modalità chiaramente distinte:

- **Rapida:** controllo preliminare con copertura dichiarata.
- **Approfondita:** ricerca più completa tra le capacità effettivamente implementate, con OCR e analisi delle parti pertinenti.

Entrambe possono produrre un verbale, ma quello della modalità rapida deve riportare in evidenza che è preliminare. Nessuna modalità garantisce di trovare qualsiasi documento in qualsiasi condizione.

## 3. Accelerazione CPU, iGPU, GPU ed eventuale NPU

Valuta concretamente l'accelerazione hardware: ad esempio **SSE/AVX2/AVX-512 e altre istruzioni CPU pertinenti**, oltre a GPU integrate, GPU dedicate ed eventualmente NPU, quando un backend adatto le supporta.

Non basta mostrare l'hardware disponibile o aggiungere opzioni nella GUI: considera una funzione accelerata implementata solo quando è realmente usata e verificata.

### CPU e SIMD

- Distribuisci un eseguibile Windows x64 con un percorso CPU di base compatibile. Non imporre AVX2 o AVX-512 all'intero programma.
- Rileva a runtime le istruzioni effettivamente utilizzabili dal processo, inclusi i requisiti del sistema operativo. Per AVX-512 controlla le estensioni specifiche richieste dal codice, non una generica etichetta "AVX-512".
- Seleziona dinamicamente i percorsi ottimizzati solo sulle macchine compatibili. In caso di incompatibilità o errore torna al percorso CPU di base, senza crash per istruzione non valida.
- Evita una release compilata soltanto con `target-cpu=native` o feature globali della macchina di sviluppo.
- Prima profila il programma: valuta SIMD e librerie già ottimizzate nei passaggi realmente costosi, come elaborazione immagini, ricerca testuale, hashing o inferenza OCR. Non implementare routine SIMD artigianali senza un vantaggio dimostrato.
- AVX-512 non deve essere scelta automaticamente solo perché disponibile: confronta durata complessiva, consumo di risorse e impatto sulla frequenza. Abilitala dove i test indicano un beneficio.
- Per accelerazioni che devono essere semanticamente equivalenti, verifica gli stessi risultati del percorso di base. Se cambiano motore o precisione numerica, misura anche l'effetto sui documenti mancati e sui falsi positivi.

### GPU e altri acceleratori

- Valuta backend locali mantenuti per OCR o analisi immagini. Non presumere che Tesseract usi automaticamente CUDA, GPU o AVX-512: verifica il motore e il build effettivamente distribuiti.
- Studia le opzioni attualmente documentate per Windows e Rust, inclusi quando appropriati WinML/ONNX Runtime e provider compatibili. Considera le alternative solo se migliorano concretamente il prodotto.
- Mantieni il programma utilizzabile senza GPU dedicata, SDK di sviluppo, connessione Internet o driver aggiuntivi installati dall'applicazione.
- Misura anche inizializzazione, trasferimenti CPU/GPU, memoria occupata, tempi a freddo e a caldo: il solo tempo del kernel non rappresenta il vantaggio per l'utente.
- Non inizializzare un runtime pesante all'avvio se non serve. I modelli o backend voluminosi possono essere pacchetti opzionali, espliciti e documentati; il download non deve avvenire durante una scansione né essere obbligatorio per il funzionamento di base.
- Nessun download automatico di provider, installazione di driver o configurazione di servizi senza una richiesta esplicita. Verifica licenze, integrità dei modelli e modalità di distribuzione.
- Gestisci assenza di supporto, memoria insufficiente e guasti del backend con fallback CPU e registrazione del cambio nel riepilogo.

Nelle opzioni avanzate prevedi **Automatico** e **Solo CPU**. Mostra una scelta esplicita dell'acceleratore solo se quel backend è implementato. Separa "disponibile", "selezionato" e "effettivamente utilizzato" e mostra il motivo di un eventuale fallback.

L'accelerazione hardware non deve ritardare inutilmente un MVP già utile: prima consegna una base CPU efficiente e corretta; aggiungi nel medesimo percorso di lavoro le ottimizzazioni con beneficio dimostrabile. Non dichiarare collaudate feature che non hai potuto eseguire sull'hardware disponibile e non simulare benchmark come se fossero reali.

## 4. Interfaccia

GUI in italiano, moderna, dark, curata e semplice da capire, con testo leggibile, spaziature coerenti e stati distinguibili anche senza affidarsi soltanto ai colori.

Scegli una libreria adatta a Rust e Windows dopo averne verificato documentazione, manutenzione, licenza e impatto sul pacchetto. Preferisci una GUI desktop senza WebView se soddisfa bene i requisiti. Non scegliere una tecnologia soltanto perché produce una demo più facilmente.

La schermata principale deve rendere immediati:

1. Scelta dei dischi o delle cartelle.
2. Modalità rapida o approfondita.
3. Riepilogo della copertura prevista e dello stato OCR.
4. Pulsante **Avvia verifica**.
5. Avanzamento e risultati.
6. Generazione del **verbale PDF**.

Dettagli tecnici e opzioni vanno in pannelli espandibili o sezioni dedicate. L'app deve essere semplice al primo utilizzo e ricca di informazioni quando richieste. Deve restare utilizzabile su schermi piccoli e con ridimensionamento DPI.

Durante la scansione mostra area esaminata, fase corrente, file incontrati e analizzati, file saltati, possibili documenti individuati e tempo trascorso. Aggiorna la GUI senza rallentare il motore. Non inventare percentuali o tempi residui quando non sono stimabili.

Consenti l'interruzione mantenendo gli esiti già raccolti e marcando la verifica come non conclusa. Presenta errori comprensibili, con dettagli tecnici consultabili separatamente e senza contenuti personali nei log.

## 5. Riconoscimento e OCR

Cerca possibili copie di:

- Carte d'identità cartacee ed elettroniche.
- Passaporti.
- Patenti.
- Tessere sanitarie.

Analizza PDF, immagini e documenti Office nei formati effettivamente supportati. Dichiara l'elenco esatto dei formati e i relativi limiti.

Combina testo estratto, OCR, parole chiave contestuali, struttura del documento, numerazioni con controlli formali pertinenti, eventuali zone MRZ e nome del file come indizio aggiuntivo. Non affidarti soltanto al nome del file.

Un'email o un codice fiscale isolato non devono essere automaticamente classificati come copia di documento. Distingui indizi deboli e riscontri più consistenti e spiega ogni segnalazione senza conservare il valore personale riconosciuto. Non presentare punteggi euristici come probabilità statistiche calibrate.

Prevedi OCR locale, per esempio Tesseract o un'alternativa verificata. Nella modalità approfondita considera anche PDF misti, pagine con testo digitale e immagini pertinenti, documenti ruotati e immagini incorporate nei formati supportati. Non escludere automaticamente l'OCR perché il PDF contiene già del testo.

Se una parte non è supportata o non può essere esaminata, registrala come limitazione. Gestisci la qualità delle scansioni con preprocessamenti proporzionati al beneficio; evita di applicare trasformazioni costose indiscriminatamente.

Il funzionamento OCR deve essere verificabile dal pacchetto consegnato. Se il motore o i modelli mancano, segnalalo prima del controllo e nel PDF; non limitarti a consegnare un checkbox OCR che non funziona.

## 6. Copertura, esclusioni e file problematici

Considera, secondo selezione e permessi disponibili, profili di tutti gli utenti, Desktop, Documenti, Download, cartelle temporanee, cestino e altre cartelle dei dischi locali. Non escludere silenziosamente AppData o altre aree importanti per una verifica di questo tipo.

Gestisci e rendi visibili accessi negati, file in uso, file cifrati/corrotti, formati non supportati, archivi non aperti, placeholder cloud non disponibili localmente, volumi non accessibili e limiti raggiunti. Non recuperare credenziali né eludere autorizzazioni.

Evita cicli nei collegamenti e attraversamenti verso percorsi di rete o aree esterne all'ambito autorizzato. Tratta esplicitamente volumi montati come cartelle per non perderli silenziosamente. Impedisci estrazioni di archivi fuori dalla destinazione prevista e limita decompressione e risorse.

Non promettere un'immagine coerente nel tempo dell'intero disco: segnala le situazioni rilevabili in cui un file cambia durante il controllo. Nessun elemento saltato o fallito deve essere contato come analizzato con successo.

## 7. Verbale PDF e risultati

Il programma **non certifica l'assenza assoluta** di documenti sul computer e non deve apparire approvato da Lepida. Genera un **Verbale di verifica dei documenti presenti sulla postazione**, leggibile e stampabile.

Il verbale deve riportare:

- Identificazione del computer, operatore e riferimento della verifica.
- Versioni del programma, delle regole e dei motori impiegati.
- Inizio e fine, con data e fuso orario espliciti.
- Dischi e percorsi selezionati e aree non controllate.
- Modalità rapida o approfondita, stato OCR e impostazioni rilevanti.
- Backend e accelerazioni effettivamente usati, eventuali fallback.
- Conteggi di file analizzati, saltati e segnalati, con significato chiaro.
- Eventuale riuso di risultati precedenti.
- Errori, esclusioni e limiti incontrati.
- Conclusione proporzionata alla copertura.
- Spazio per firma o riferimento alla sottoscrizione del responsabile.

Distingui **possibili documenti rilevati, da verificare**, **nessun documento rilevato nell'ambito esaminato** e **verifica incompleta o interrotta**. Rilevamenti e incompletezza possono coesistere e devono essere rappresentati entrambi.

Una scansione incompleta con zero corrispondenze non deve diventare un esito "tutto regolare". Una firma o un hash del PDF non trasformano il controllo in una garanzia di assenza. L'accettazione da parte di Lepida resta da confermare rispetto ai requisiti effettivamente richiesti.

Non inserire nel PDF immagini dei documenti, estratti OCR, codici fiscali o numeri dei documenti. Permetti di includere separatamente i percorsi dei file segnalati, con avviso che possono contenere dati personali. Con pochi risultati mantieni il verbale compatto, senza impaginazioni ridondanti.

## 8. Sicurezza, audit e dati locali

Imposta limiti configurabili per concorrenza, dimensioni, pagine, tempi e memoria dove applicabile. Gestisci crash e timeout di parser/OCR senza bloccare tutta l'applicazione. Il semplice uso di processi separati non equivale a una sandbox: dichiara i limiti dell'isolamento e riduci i privilegi dove praticabile.

Mantieni uno storico locale e un audit essenziale, con permessi adeguati, senza salvare i contenuti riconosciuti. Documenta posizione dei dati, eventuale cache, conservazione e rimozione. Se usi SQLite o un'alternativa, motivala senza introdurre servizi esterni.

Gestisci e documenta i temporanei dell'OCR, anche in caso di interruzione. Nessuna modifica automatica a impostazioni di sicurezza, antivirus, firewall, policy, driver o servizi.

## 9. Test, qualità e prestazioni misurate

Prepara un corpus sintetico etichettato, senza dati reali, con positivi e negativi: PDF testuali e misti, immagini, documenti Office, nomi generici, scansioni ruotate o poco contrastate, accesso negato, file corrotti, cifrati, limitazioni e interruzione.

Misura separatamente:

- Documenti individuati, documenti mancati e falsi positivi, per classe e formato.
- Durata, file al secondo e pagine OCR al secondo.
- Picco di RAM e memoria GPU quando usata.
- Tempo di avvio e dimensione del pacchetto.
- Costo di inizializzazione di backend/modelli e impatto del parallelismo.

Confronta gli stessi input tra percorso CPU di base e percorsi accelerati realmente disponibili. Misura tempi complessivi a freddo e a caldo, distinguendo cache applicativa e filesystem quando possibile. Riporta CPU, istruzioni disponibili, RAM, dischi, GPU, driver e impostazioni del test.

Valuta sia prestazioni sia qualità: un backend più veloce che perde documenti non è automaticamente migliore. Non dichiarare percentuali di accuratezza, accelerazioni o compatibilità non provate. L'hardware non disponibile va segnato come non collaudato.

Verifica che i PDF rappresentino correttamente tutti gli esiti, che la GUI rimanga reattiva e che i processi di lavoro si chiudano dopo annullamento o uscita.

## 10. Build, repository e consegna sul Desktop

Usa Rust stabile e dipendenze mantenute; verifica licenze del codice, dei motori, dei modelli e dei runtime ridistribuiti. Motiva brevemente le scelte che incidono su peso, velocità o compatibilità. Non includere dati personali, report di PC reali, credenziali, percorsi privati, dump o documenti dell'ente.

Compila nativamente per Windows e verifica il pacchetto distribuito, non solo il programma avviato dall'ambiente di sviluppo. Prova l'interfaccia reale su Windows e renderizza i PDF per controllare impaginazione e leggibilità. I test automatici/offscreen e l'avvio via SSH non sostituiscono una prova interattiva delle funzioni GUI.

Alla fine individua il **Desktop reale dell'utente di questo PC tramite le API Windows**, senza presumere il percorso standard. Crea una cartella **TracciaDoc**, senza modificare altri elementi del Desktop e senza sovrascrivere file estranei se la cartella esiste già.

Dentro consegna:

- **TracciaDoc.exe**, pronto da avviare.
- Tutte le dipendenze necessarie e modelli OCR previsti dalla distribuzione scelta.
- Istruzioni brevi in italiano.
- Licenze e informazioni sulle versioni dei componenti.
- Una cartella di documenti sintetici di prova.
- Un esempio di verbale PDF chiaramente contrassegnato come dimostrativo.

Il PC destinatario non deve richiedere Rust, compilatori, SDK di sviluppo o Internet per la scansione. Preferisci un pacchetto portatile. Se componenti opzionali GPU/modelli sono voluminosi, distinguili dalla base e documenta come prepararli offline; l'app deve restare utile con la sola CPU.

Mantieni i sorgenti nella cartella del progetto prevista dal workspace e usa branch con prefisso `codex/` per lo sviluppo. Non effettuare force push e non rendere pubblico il repository.

Concludi indicando percorso esatto dell'EXE, cosa è implementato, prove effettive, prestazioni misurate e limiti ancora presenti. Non confondere un piano di sviluppo o il rilevamento hardware con una funzionalità funzionante.

## Riferimenti tecnici da ricontrollare durante l'implementazione

Punti di partenza ufficiali consultati il 28 ottobre 2026; verifica versioni e compatibilità prima di scegliere le dipendenze:

- Rust, rilevamento CPU a runtime: https://doc.rust-lang.org/std/macro.is_x86_feature_detected.html
- ONNX Runtime, percorso Windows/WinML: https://onnxruntime.ai/docs/get-started/with-windows.html
- ONNX Runtime, DirectML e relativo stato di manutenzione: https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html
- Microsoft, provider Windows ML e relativi requisiti: https://learn.microsoft.com/en-us/windows/ai/new-windows-ml/supported-execution-providers
- Tesseract, documentazione e FAQ: https://tesseract-ocr.github.io/tessdoc/FAQ.html

Questi riferimenti non obbligano a usare un particolare motore e non dimostrano che il programma li abbia già integrati. Le scelte devono derivare da requisiti, compatibilità offline, licenze e benchmark.
