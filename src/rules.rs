use regex::Regex;
use std::sync::LazyLock;

static CF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Z]{6}[0-9LMNPQRSTUV]{2}[ABCDEHLMPRST][0-9LMNPQRSTUV]{2}[A-Z][0-9LMNPQRSTUV]{3}[A-Z]\b").unwrap()
});
static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    // Le carte d'identita cartacee stampano il numero con uno spazio: "AV 1234567".
    Regex::new(r"\b(?:[A-Z]{2}[0-9]{5}[A-Z]{2}|[A-Z]{2} ?[0-9]{7}|[A-Z][0-9]{7}|[A-Z0-9]{10})\b")
        .unwrap()
});
/// Numero della carta cartacea dopo il titolo: due lettere e sette cifre, con o senza spazio.
static PAPER_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Z]{2} ?[0-9]{7}\b").unwrap());
pub fn valid_cf(s: &str) -> bool {
    if s.len() != 16
        || !s
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return false;
    }
    let odd = [
        1, 0, 5, 7, 9, 13, 15, 17, 19, 21, 2, 4, 18, 20, 11, 3, 6, 8, 12, 14, 16, 10, 22, 25, 24,
        23,
    ];
    let sum: usize = s
        .bytes()
        .take(15)
        .enumerate()
        .map(|(i, b)| {
            let v = if b.is_ascii_digit() {
                (b - b'0') as usize
            } else {
                (b - b'A') as usize
            };
            if i % 2 == 0 { odd[v] } else { v }
        })
        .sum();
    s.as_bytes()[15] == b'A' + (sum % 26) as u8
}
fn mrz_digit(s: &[u8]) -> u8 {
    (s.iter()
        .enumerate()
        .map(|(i, &b)| {
            let n = match b {
                b'0'..=b'9' => b - b'0',
                b'A'..=b'Z' => b - b'A' + 10,
                _ => 0,
            };
            n as u32 * [7, 3, 1][i % 3]
        })
        .sum::<u32>()
        % 10) as u8
        + b'0'
}
/// Etichetta all'inizio di una parola: NOME non conta dentro COGNOME, NAME non dentro SURNAME.
fn has_label(context: &str, label: &str) -> bool {
    context
        .match_indices(label)
        .any(|(i, _)| !context[..i].chars().next_back().is_some_and(char::is_alphanumeric))
}
pub fn detect(text: &str) -> Vec<String> {
    let upper = text.to_uppercase();
    let normalized = upper.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = vec![];
    // A contextual cluster is evaluated in a bounded neighbourhood, not across an entire workbook.
    for (class, terms) in [
        (
            "Carta d'identita",
            &["CARTA DI IDENTIT", "CARTA D'IDENTIT", "IDENTITY CARD"][..],
        ),
        ("Passaporto", &["PASSAPORTO", "PASSPORT"][..]),
        (
            "Patente",
            &["PATENTE DI GUIDA", "DRIVING LICEN", "PATENTE"][..],
        ),
        (
            "Tessera sanitaria",
            &[
                "TESSERA SANITARIA",
                "HEALTH INSURANCE CARD",
                "TESSERA EUROPEA DI ASSICURAZIONE MALATTIA",
                "CARTA REGIONALE DEI SERVIZI",
                "CARTA NAZIONALE DEI SERVIZI",
            ][..],
        ),
    ] {
        for term in terms {
            for (pos, _) in normalized.match_indices(term) {
                let context: String = normalized[..pos]
                    .chars()
                    .rev()
                    .take(200)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .chain(normalized[pos..].chars().take(650))
                    .collect();
                let labels = [
                    "COGNOME",
                    "SURNAME",
                    "NOME",
                    "NAME",
                    "NASCITA",
                    "BIRTH",
                    "SCADENZA",
                    "EXPIR",
                    "RILASCI",
                    "ISSU",
                    "CITTADINANZA",
                    "NAZIONALIT",
                    "SESSO",
                    "SEX",
                    "4A.",
                    "4B.",
                ];
                let fields = labels.iter().filter(|s| has_label(&context, s)).count();
                let number = NUMBER
                    .find_iter(&context)
                    .any(|m| m.as_str().bytes().any(|b| b.is_ascii_digit()));
                let cf = CF.find_iter(&context).any(|m| valid_cf(m.as_str()));
                if fields >= 3 || (fields >= 2 && (number || cf)) {
                    out.push(format!(
                        "{class}: intestazione e campi documentali coerenti{}",
                        if number {
                            "; numerazione formalmente compatibile"
                        } else if cf {
                            "; controllo formale codice fiscale superato"
                        } else {
                            ""
                        }
                    ));
                }
            }
        }
    }
    // Interno della carta d'identita cartacea: non riporta l'intestazione, ma i connotati
    // (statura, capelli, occhi, segni particolari) insieme ai dati anagrafici sono tipici solo
    // di quel documento. Servono almeno due connotati e altri due campi, nello stesso tratto.
    const TRAITS: [&str; 5] = ["CONNOTATI", "STATURA", "CAPELLI", "OCCHI", "SEGNI PARTICOLARI"];
    const CIVIL: [&str; 6] = ["COGNOME", "NOME", "CITTADINANZA", "RESIDENZA", "STATO CIVILE", "PROFESSIONE"];
    for (pos, _) in normalized.match_indices("STATURA").chain(normalized.match_indices("CONNOTATI")) {
        let from = normalized[..pos].char_indices().rev().nth(600).map_or(0, |(i, _)| i);
        let to = normalized[pos..].char_indices().nth(400).map_or(normalized.len(), |(i, _)| pos + i);
        let context = &normalized[from..to];
        let traits = TRAITS.iter().filter(|s| has_label(context, s)).count();
        let civil = CIVIL.iter().filter(|s| has_label(context, s)).count();
        if traits >= 2 && civil >= 2 {
            out.push("Carta d'identita cartacea: connotati e dati anagrafici coerenti".into());
            break;
        }
    }
    // Copertina della carta cartacea: stemma, "REPUBBLICA ITALIANA", "COMUNE DI", titolo e
    // numero, senza campi etichettati. I caratteri romani sono spesso letti con V al posto di U.
    let cover = normalized.replace("REPVBBLICA", "REPUBBLICA").replace("COMVNE", "COMUNE");
    for (pos, _) in cover
        .match_indices("CARTA D'IDENTIT")
        .chain(cover.match_indices("CARTA DI IDENTIT"))
    {
        let from = cover[..pos].char_indices().rev().nth(250).map_or(0, |(i, _)| i);
        let to = cover[pos..].char_indices().nth(120).map_or(cover.len(), |(i, _)| pos + i);
        let context = &cover[from..to];
        let after = &cover[pos..to];
        if context.contains("REPUBBLICA ITALIANA")
            && context.contains("COMUNE DI")
            && PAPER_NUMBER.is_match(after)
        {
            out.push("Carta d'identita cartacea: copertina con intestazione e numero".into());
            break;
        }
    }
    let lines: Vec<String> = upper
        .lines()
        .map(|s| s.chars().filter(|c| !c.is_whitespace()).collect())
        .filter(|s: &String| {
            [30, 36, 44].contains(&s.len())
                && s.bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'<')
        })
        .collect();
    for line in &lines {
        let b = line.as_bytes();
        if b.len() == 44
            && mrz_digit(&b[0..9]) == b[9]
            && mrz_digit(&b[13..19]) == b[19]
            && mrz_digit(&b[21..27]) == b[27]
        {
            out.push("MRZ TD3: tre cifre di controllo valide; possibile passaporto".into());
        }
        if b.len() == 30
            && (b.starts_with(b"ID") || b.starts_with(b"I<"))
            && mrz_digit(&b[5..14]) == b[14]
        {
            out.push("MRZ TD1: intestazione e controllo numero documento validi".into());
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_values_are_not_documents() {
        assert!(valid_cf("RSSMRA80A01H501U"));
        assert!(!valid_cf("RSSMRA80A01H501A"));
        for text in [
            "RSSMRA80A01H501U",
            "test@example.invalid",
            "carta di identita da allegare alla richiesta",
            "AA00000AA",
            // Un modulo che chiede il documento: COGNOME contiene NOME, non sono due campi.
            "Allegare copia della carta d'identita. Cognome del richiedente. Codice pratica AB12345678",
        ] {
            assert!(detect(text).is_empty());
        }
    }
    #[test]
    fn all_classes_and_no_personal_values() {
        for title in [
            "CARTA DI IDENTITA",
            "PASSAPORTO",
            "PATENTE DI GUIDA",
            "TESSERA SANITARIA",
        ] {
            let found = detect(&format!(
                "{title}\nCognome ESEMPIO Nome PROVA Data nascita 01/01/2000 Scadenza 01/01/2030 AA00000AA"
            ));
            assert!(!found.is_empty(), "{title}");
            assert!(!found.join(" ").contains("AA00000AA"));
        }
    }
    #[test]
    fn paper_identity_card_inside() {
        let inside = "Cognome ESEMPIO Nome PROVA nato il 01/01/2000 Cittadinanza ITALIANA \
            Residenza ESEMPIO Stato civile Professione CONNOTATI E CONTRASSEGNI SALIENTI \
            Statura 1,70 Capelli CASTANI Occhi CASTANI Segni particolari NESSUNO";
        assert!(!detect(inside).is_empty());
        // Una scheda medica con statura e occhi ma senza dati anagrafici non basta.
        assert!(detect("Visita oculistica: statura 1,70, occhi arrossati").is_empty());
        // Copertina: intestazione e numero con spazio, piu nomi in campi senza etichetta no;
        // con due campi si.
        assert!(!detect("CARTA D'IDENTITA N. AV 0000000 Cognome ESEMPIO Nome PROVA").is_empty());
    }
    #[test]
    fn paper_identity_card_cover() {
        let cover = "REPVBBLICA ITALIANA COMVNE DI ESEMPIO CARTA D'IDENTITA' N. AV 0000000 DI";
        assert!(!detect(cover).is_empty());
        // Una lettera che cita il documento senza intestazione della Repubblica non basta.
        assert!(detect("Comune di Esempio. Si allega carta d'identita n. AV 0000000.").is_empty());
    }
    #[test]
    fn european_health_card() {
        assert!(!detect("TESSERA EUROPEA DI ASSICURAZIONE MALATTIA Cognome ESEMPIO Nome PROVA Data di nascita 01/01/2000").is_empty());
    }
    #[test]
    fn checked_mrz() {
        assert!(!detect("L898902C36UTO7408122F1204159ZE184226B<<<<<10").is_empty());
    }
}
