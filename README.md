# TracciaDoc

Applicazione Windows locale, gratuita e con sorgenti MIT, per cercare possibili copie di documenti d'identità e generare un verbale PDF della verifica.

**Stato:** specifica iniziale. L'applicazione Rust, l'EXE e l'accelerazione hardware non sono ancora implementati. Il repository nasce privato e deve rimanere tale salvo richiesta esplicita del proprietario.

## Specifica da eseguire

Il documento completo è [docs/PROMPT.md](docs/PROMPT.md). Contiene requisiti funzionali, GUI, prestazioni, accelerazione CPU/GPU, test e consegna sul Desktop.

## Principi

- Scansione locale in sola lettura; nessuna elaborazione cloud.
- Interfaccia semplice, dettagli e opzioni avanzate disponibili quando servono.
- Prestazioni misurate senza nascondere le lacune di copertura.
- CPU compatibile come base; AVX2/AVX-512 e GPU facoltative, selezionate solo quando supportate e vantaggiose.
- Il verbale descrive il controllo: non certifica l'assenza assoluta di documenti né un'approvazione di Lepida.
- Nel repository solo codice, specifiche e fixture sintetiche. Mai documenti reali, report di postazioni, credenziali o dati dell'ente.

Nessuna scansione di dati reali è autorizzata dalla sola attività di sviluppo.
