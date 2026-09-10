//! Scoring for golden tests. Model output drifts between versions, so tests assert on
//! thresholds (WER, chrF, speaker structure) rather than exact strings.

/// Word error rate via Levenshtein on lowercase, punctuation-stripped tokens.
pub fn wer(reference: &str, hypothesis: &str) -> f32 {
    let r = norm(reference); let h = norm(hypothesis);
    if r.is_empty() { return if h.is_empty() { 0.0 } else { 1.0 }; }
    levenshtein(&r, &h) as f32 / r.len() as f32
}

/// chrF (character n-gram F-score, β=2, n≤6) — the standard cheap MT metric, language-agnostic.
pub fn chrf(reference: &str, hypothesis: &str) -> f32 {
    let r: Vec<char> = reference.chars().filter(|c| !c.is_whitespace()).collect();
    let h: Vec<char> = hypothesis.chars().filter(|c| !c.is_whitespace()).collect();
    let (mut p_sum, mut r_sum, mut n_used) = (0.0, 0.0, 0);
    for n in 1..=6 {
        if r.len() < n || h.len() < n { continue; }
        let rg = grams(&r, n); let hg = grams(&h, n);
        let overlap: usize = hg.iter().map(|(g, c)| c.min(rg.get(g).copied().unwrap_or(0))).sum();
        let hc: usize = hg.values().sum(); let rc: usize = rg.values().sum();
        p_sum += overlap as f32 / hc.max(1) as f32;
        r_sum += overlap as f32 / rc.max(1) as f32;
        n_used += 1;
    }
    if n_used == 0 { return 0.0; }
    let (p, r) = (p_sum / n_used as f32, r_sum / n_used as f32);
    if p + r == 0.0 { 0.0 } else { 5.0 * p * r / (4.0 * p + r) }
}

fn grams(s: &[char], n: usize) -> std::collections::HashMap<String, usize> {
    let mut m = std::collections::HashMap::new();
    for w in s.windows(n) { *m.entry(w.iter().collect::<String>()).or_insert(0) += 1; }
    m
}

fn norm(s: &str) -> Vec<String> {
    s.to_lowercase().split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
        .filter(|w| !w.is_empty()).collect()
}

fn levenshtein(a: &[String], b: &[String]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, y) in b.iter().enumerate() {
            let cost = if x == y { 0 } else { 1 };
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn wer_basic() {
        assert_eq!(wer("я читаю книгу", "я читаю книгу"), 0.0);
        assert!((wer("я читаю книгу", "я читаю газету") - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(wer("Hello, world!", "hello world"), 0.0, "punctuation/case ignored");
    }
    #[test] fn chrf_orders() {
        let r = "I am reading a book at home.";
        assert!(chrf(r, r) > 0.99);
        assert!(chrf(r, "I read a book at home.") > chrf(r, "The weather is nice today."));
    }
}
