# Changelog

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
