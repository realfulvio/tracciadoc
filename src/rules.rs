use regex::Regex;
use std::sync::LazyLock;

static CF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Z]{6}[0-9LMNPQRSTUV]{2}[ABCDEHLMPRST][0-9LMNPQRSTUV]{2}[A-Z][0-9LMNPQRSTUV]{3}[A-Z]\b").unwrap()
});
static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:[A-Z]{2}[0-9]{5}[A-Z]{2}|[A-Z]{2}[0-9]{7}|[A-Z][0-9]{7}|[A-Z0-9]{10})\b")
        .unwrap()
});
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
            &["TESSERA SANITARIA", "HEALTH INSURANCE CARD"][..],
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
                let fields = labels.iter().filter(|s| context.contains(**s)).count();
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
    fn checked_mrz() {
        assert!(!detect("L898902C36UTO7408122F1204159ZE184226B<<<<<10").is_empty());
    }
}
