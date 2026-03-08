//! Build enrichment query candidates from local work metadata.

use std::collections::HashSet;

use chrono::Datelike;
use unicode_normalization::UnicodeNormalization;

use crate::domain::work::Work;

#[derive(Debug, Clone)]
pub struct EnrichmentQueryInput {
    pub primary_title: String,
    pub search_terms: Vec<String>,
    pub known_brand: Option<String>,
    pub expected_year: Option<u32>,
}

pub fn build_query_input(work: &Work) -> EnrichmentQueryInput {
    let mut terms = Vec::new();
    let mut seen = HashSet::new();

    push_candidate(&mut terms, &mut seen, Some(&work.title));
    push_candidate(&mut terms, &mut seen, work.title_original.as_deref());

    for alias in &work.title_aliases {
        push_candidate(&mut terms, &mut seen, Some(alias));
    }

    if terms.is_empty() {
        terms.push(work.title.trim().to_string());
    }

    EnrichmentQueryInput {
        primary_title: terms.first().cloned().unwrap_or_else(|| work.title.clone()),
        search_terms: terms,
        known_brand: work
            .developer
            .as_ref()
            .map(|value| sanitize_query(value))
            .filter(|value| !value.is_empty()),
        expected_year: work.release_date.map(|date| date.year() as u32),
    }
}

pub fn build_query_input_from_title(title: &str) -> EnrichmentQueryInput {
    let mut terms = Vec::new();
    let mut seen = HashSet::new();
    push_candidate(&mut terms, &mut seen, Some(title));

    if terms.is_empty() {
        terms.push(title.trim().to_string());
    }

    EnrichmentQueryInput {
        primary_title: terms.first().cloned().unwrap_or_default(),
        search_terms: terms,
        known_brand: None,
        expected_year: None,
    }
}

pub fn extend_query_input<I, S>(input: &mut EnrichmentQueryInput, values: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen: HashSet<String> = input
        .search_terms
        .iter()
        .map(|value| value.to_lowercase())
        .collect();

    for value in values {
        push_candidate(&mut input.search_terms, &mut seen, Some(value.as_ref()));
    }

    if input.primary_title.trim().is_empty() {
        input.primary_title = input.search_terms.first().cloned().unwrap_or_default();
    }
}

pub fn canonicalize_query(raw: &str) -> String {
    sanitize_query(raw)
        .nfkc()
        .collect::<String>()
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '・' && *c != '～' && *c != '~')
        .collect()
}

fn push_candidate(terms: &mut Vec<String>, seen: &mut HashSet<String>, raw: Option<&str>) {
    let Some(raw) = raw else {
        return;
    };
    let sanitized = sanitize_query(raw);
    if sanitized.is_empty() {
        return;
    }

    insert_unique(terms, seen, sanitized.clone());

    for variant in derive_variants(&sanitized) {
        insert_unique(terms, seen, variant);
    }

    if let Some(symbol_free) = symbol_normalized_variant(&sanitized) {
        insert_unique(terms, seen, symbol_free);
    }
}

fn insert_unique(terms: &mut Vec<String>, seen: &mut HashSet<String>, value: String) {
    let key = value.to_lowercase();
    if seen.insert(key) {
        terms.push(value);
    }
}

fn derive_variants(value: &str) -> Vec<String> {
    let mut variants = Vec::new();

    for separator in [" + ", " ＋ ", " ~", " ～", " -", ": ", "：", "／", "/"] {
        if let Some((head, _)) = value.split_once(separator) {
            let trimmed = head.trim();
            if trimmed.chars().count() >= 3 {
                variants.push(trimmed.to_string());
            }
        }
    }

    for delimiter in [
        "予約特典",
        "特典",
        "初回特典",
        "豪華限定版",
        "豪華版",
        "限定版",
        "通常版",
        "Update",
        "UPDATE",
    ] {
        if let Some((head, _)) = value.split_once(delimiter) {
            let trimmed = head.trim().trim_end_matches('+').trim();
            if trimmed.chars().count() >= 3 {
                variants.push(trimmed.to_string());
            }
        }
    }

    for suffix in [
        "リメイク",
        " remake",
        "remake",
        " remaster",
        "remaster",
        "完全版",
        "豪華版",
        "豪華限定版",
        "限定版",
        "通常版",
        "DL版",
        "初回版",
        "特別版",
        "復刻版",
        "重制版",
        "重製版",
        " オリジナルヴォーカルCD",
        " Theme Song",
        " Voice Drama",
        " Vocal CD",
        " Update",
    ] {
        if let Some(stripped) = value.strip_suffix(suffix) {
            let trimmed = stripped.trim();
            if trimmed.chars().count() >= 3 {
                variants.push(trimmed.to_string());
            }
        }
    }

    let chars: Vec<char> = value.chars().collect();
    if chars.len() >= 8 {
        for prefix_len in [8usize, 12, 16] {
            if chars.len() >= prefix_len {
                let prefix: String = chars[..prefix_len].iter().collect();
                if prefix.chars().count() >= 4 {
                    variants.push(prefix.trim().to_string());
                }
            }
        }

        for suffix_len in [8usize, 12] {
            if chars.len() >= suffix_len {
                let suffix: String = chars[chars.len() - suffix_len..].iter().collect();
                if suffix.chars().count() >= 4 {
                    variants.push(suffix.trim().to_string());
                }
            }
        }
    }

    variants
}

fn symbol_normalized_variant(value: &str) -> Option<String> {
    let normalized = value
        .chars()
        .map(|ch| match ch {
            '≡' | '＝' | '=' | '!' | '！' | '・' | '･' | '☆' | '★' | '♪' | '〜' | '～' | '~' => {
                ' '
            }
            '×' => 'x',
            _ => ch,
        })
        .collect::<String>();
    let compact = sanitize_query(&normalized);
    if compact.is_empty() || compact == value {
        None
    } else {
        Some(compact)
    }
}

fn sanitize_query(raw: &str) -> String {
    raw.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c| matches!(c, '"' | '\'' | '[' | ']' | '(' | ')'))
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::work::Work;

    #[test]
    fn build_query_input_dedupes_and_adds_variants() {
        let mut work = Work::from_discovery("C:/tmp".into(), "DeepOne -領界侵犯-".to_string(), 0.0);
        work.title_original = Some("DeepOne -領界侵犯-".to_string());
        work.title_aliases = vec!["DeepOne -領界侵犯-".to_string(), "DeepOne".to_string()];
        let input = build_query_input(&work);
        assert_eq!(input.search_terms[0], "DeepOne -領界侵犯-");
        assert!(input.search_terms.iter().any(|term| term == "DeepOne"));
    }

    #[test]
    fn canonicalize_query_normalizes_variants() {
        assert_eq!(canonicalize_query("ＡＢＣ ゲーム"), "abcゲーム");
    }

    #[test]
    fn derive_variants_strips_common_suffixes() {
        assert!(derive_variants("下級生リメイク")
            .iter()
            .any(|term| term == "下級生"));
    }

    #[test]
    fn build_query_input_adds_symbol_free_variant() {
        let input = build_query_input_from_title("CRACK≡TRICK!");
        assert!(input.search_terms.iter().any(|term| term == "CRACK TRICK"));
    }
}
