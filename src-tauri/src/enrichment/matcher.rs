//! Fuzzy title matching + scoring.
//!
//! Compares local candidate titles against API results.
//! Score thresholds: ≥85 auto-match, ≥75 pending review, <75 reject.

use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub api_id: String,
    pub api_title: String,
    pub score: f64,
    pub verdict: MatchVerdict,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchVerdict {
    AutoMatch,
    PendingReview,
    Rejected,
}

#[derive(Debug, Clone, Default)]
pub struct MatchBonuses {
    pub known_brand: Option<String>,
    pub expected_year: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct MatchInput {
    pub titles: Vec<String>,
    pub bonuses: MatchBonuses,
}

fn normalize(title: &str) -> String {
    title
        .nfkc()
        .collect::<String>()
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '・' && *c != '～' && *c != '~')
        .collect()
}

pub fn similarity(local: &str, api: &str) -> f64 {
    let a = normalize(local);
    let b = normalize(api);

    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    if a == b {
        return 100.0;
    }

    let shorter_len = a.chars().count().min(b.chars().count());
    let shorter_in_longer = if a.chars().count() <= b.chars().count() {
        b.contains(&a)
    } else {
        a.contains(&b)
    };
    if shorter_in_longer && shorter_len >= 6 {
        return 90.0;
    }

    let lcs_len = lcs_length(&a, &b) as f64;
    let combined_len = (a.chars().count() + b.chars().count()) as f64;

    ((2.0 * lcs_len) / combined_len) * 100.0
}

fn lcs_length(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    let mut prev = vec![0usize; n + 1];
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        for j in 1..=n {
            if a_chars[i - 1] == b_chars[j - 1] {
                curr[j] = prev[j - 1] + 1;
            } else {
                curr[j] = prev[j].max(curr[j - 1]);
            }
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.fill(0);
    }

    prev.into_iter().max().unwrap_or(0)
}

pub fn score_candidate(input: &MatchInput, api_title: &str, api_id: &str) -> MatchResult {
    score_candidate_with_titles(input, &[api_title.to_string()], api_id)
}

pub fn score_candidate_with_titles(
    input: &MatchInput,
    api_titles: &[String],
    api_id: &str,
) -> MatchResult {
    let primary_title = api_titles.first().cloned().unwrap_or_default();
    let mut score = api_titles
        .iter()
        .flat_map(|api_title| {
            input
                .titles
                .iter()
                .map(move |title| similarity(title, api_title))
        })
        .fold(0.0, f64::max);

    if let Some(ref brand) = input.bonuses.known_brand {
        if api_titles
            .iter()
            .any(|title| title.to_lowercase().contains(&brand.to_lowercase()))
        {
            score += 5.0;
        }
    }

    if let Some(year) = input.bonuses.expected_year {
        if api_id.contains(&year.to_string())
            || api_titles
                .iter()
                .any(|title| title.contains(&year.to_string()))
        {
            score += 3.0;
        }
    }

    score = score.min(100.0);

    let verdict = if score >= 85.0 {
        MatchVerdict::AutoMatch
    } else if score >= 75.0 {
        MatchVerdict::PendingReview
    } else {
        MatchVerdict::Rejected
    };

    MatchResult {
        api_id: api_id.to_string(),
        api_title: primary_title,
        score,
        verdict,
    }
}

pub fn best_match(input: &MatchInput, candidates: &[(String, String)]) -> Option<MatchResult> {
    if candidates.is_empty() {
        return None;
    }

    let mut results: Vec<MatchResult> = candidates
        .iter()
        .map(|(id, title)| score_candidate(input, title, id))
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let score = similarity("Summer Pockets", "Summer Pockets");
        assert!((score - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_normalized_match() {
        let score = similarity("ＡＢＣゲーム", "ABCゲーム");
        assert!(
            score > 90.0,
            "Score should be high for NFKC-normalized match, got {}",
            score
        );
    }

    #[test]
    fn test_partial_match() {
        let score = similarity("Summer Pockets REFLECTION BLUE", "Summer Pockets");
        assert!(score > 50.0, "Should have partial match, got {}", score);
        assert!(score < 100.0, "Should not be perfect match");
    }

    #[test]
    fn test_substring_match_boost() {
        let score = similarity(
            "ママ×カノEX",
            "ママ×カノEX ～領主貴族に嫁ぎたくない娘の為に、お母さんがエッチな手ほどきいたします～",
        );
        assert!(score >= 90.0, "Expected substring boost, got {}", score);
    }

    #[test]
    fn test_best_match_uses_aliases() {
        let input = MatchInput {
            titles: vec![
                "アンラベル・トリガー 豪華版".to_string(),
                "アンラベル・トリガー".to_string(),
            ],
            bonuses: MatchBonuses::default(),
        };
        let candidates = vec![
            ("v1".to_string(), "別の作品".to_string()),
            ("v2".to_string(), "アンラベル・トリガー".to_string()),
        ];

        let result = best_match(&input, &candidates).expect("best match");
        assert_eq!(result.api_id, "v2");
        assert_eq!(result.verdict, MatchVerdict::AutoMatch);
    }
}
