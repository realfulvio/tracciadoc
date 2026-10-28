# TracciaDoc: istruzioni di progetto

1. Leggi README.md e docs/PROMPT.md prima di modificare il progetto. Il prompt è la specifica di prodotto, non una prova di funzionalità già implementate.
2. Rispondi in italiano. Mantieni il progetto e il repository privati.
3. Realizza l'app in Rust per Windows, con fallback CPU portatile e accelerazione hardware solo dopo verifica e benchmark. Mai compilare l'EXE distribuito soltanto per le istruzioni della macchina di sviluppo.
4. Non avviare scansioni su dati reali senza una richiesta esplicita. Usa soltanto fixture sintetiche nei test e nei commit.
5. Non pubblicare dati personali, percorsi privati, credenziali, report operativi, database, chiavi o dump. Non copiare il precedente archivio di sviluppo nel repository senza revisione.
6. Una verifica incompleta non è un esito pulito. Nessuna dicitura di certificazione assoluta o di approvazione Lepida.
7. Non modificare impostazioni di sistema, installare driver, disabilitare protezioni o usare servizi cloud per far funzionare l'app.
8. Aggiorna lo stato nel README in base alle prove effettive. Tieni distinti test unitari, avvio GUI, collaudo interattivo, benchmark ed eseguibile distribuito.
9. Per lo sviluppo usa branch con prefisso codex/. Non effettuare force push.
