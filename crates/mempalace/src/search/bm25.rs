use std::collections::HashMap;

pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .filter(|s| !is_stop_word(s))
        .collect()
}

fn is_stop_word(word: &str) -> bool {
    matches!(
        word,
        "a" | "an" | "the" | "is" | "it" | "in" | "on" | "at" | "to" | "of"
        | "and" | "or" | "but" | "not" | "be" | "was" | "are" | "were" | "been"
        | "has" | "have" | "had" | "do" | "does" | "did" | "will" | "would"
        | "could" | "should" | "may" | "might" | "can" | "with" | "this" | "that"
        | "for" | "from" | "by" | "as" | "if" | "so" | "we" | "you" | "he" | "she"
        | "they" | "i" | "my" | "your" | "his" | "her" | "its" | "our" | "their"
        | "what" | "which" | "who" | "how" | "when" | "where" | "why"
    )
}

pub fn bm25_scores(query: &str, documents: &[&str]) -> Vec<f64> {
    const K1: f64 = 1.5;
    const B: f64 = 0.75;

    let query_terms = tokenize(query);
    if query_terms.is_empty() || documents.is_empty() {
        return vec![0.0; documents.len()];
    }

    let n = documents.len() as f64;
    let tokenized: Vec<Vec<String>> = documents.iter().map(|d| tokenize(d)).collect();
    let avg_dl = tokenized.iter().map(|d| d.len() as f64).sum::<f64>() / n;

    let mut df: HashMap<String, f64> = HashMap::new();
    for doc in &tokenized {
        let unique: std::collections::HashSet<&String> = doc.iter().collect();
        for term in unique {
            *df.entry(term.clone()).or_insert(0.0) += 1.0;
        }
    }

    let mut scores = vec![0.0f64; documents.len()];
    for (i, doc_tokens) in tokenized.iter().enumerate() {
        let dl = doc_tokens.len() as f64;
        let mut tf_map: HashMap<String, f64> = HashMap::new();
        for t in doc_tokens {
            *tf_map.entry(t.clone()).or_insert(0.0) += 1.0;
        }
        for term in &query_terms {
            let tf = *tf_map.get(term).unwrap_or(&0.0);
            if tf == 0.0 { continue; }
            let df_val = *df.get(term).unwrap_or(&0.0);
            let idf = ((n - df_val + 0.5) / (df_val + 0.5) + 1.0).ln();
            let tf_score = (tf * (K1 + 1.0)) / (tf + K1 * (1.0 - B + B * dl / avg_dl));
            scores[i] += idf * tf_score;
        }
    }
    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_filters_stop_words() {
        let tokens = tokenize("the quick brown fox is on a log");
        assert!(!tokens.contains(&"the".to_string()));
        assert!(!tokens.contains(&"is".to_string()));
        assert!(!tokens.contains(&"on".to_string()));
        assert!(!tokens.contains(&"a".to_string()));
        assert!(tokens.contains(&"quick".to_string()));
        assert!(tokens.contains(&"brown".to_string()));
        assert!(tokens.contains(&"fox".to_string()));
        assert!(tokens.contains(&"log".to_string()));
    }

    #[test]
    fn tokenize_splits_punctuation() {
        let tokens = tokenize("hello, world! foo-bar");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"foo".to_string()));
        assert!(tokens.contains(&"bar".to_string()));
    }

    #[test]
    fn tokenize_lowercases() {
        let tokens = tokenize("Hello WORLD FoO");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"foo".to_string()));
    }

    #[test]
    fn tokenize_empty_input() {
        assert!(tokenize("").is_empty());
    }

    #[test]
    fn bm25_empty_query() {
        let scores = bm25_scores("", &["hello world"]);
        assert_eq!(scores, vec![0.0]);
    }

    #[test]
    fn bm25_empty_documents() {
        let scores = bm25_scores("hello", &[]);
        assert!(scores.is_empty());
    }

    #[test]
    fn bm25_exact_match_scores_higher() {
        let docs = &["rust programming language", "python programming language", "rust rust rust"];
        let scores = bm25_scores("rust", docs);
        assert!(scores[0] > scores[1], "exact match should beat no match");
        assert!(scores[2] > scores[0], "repeated term should score highest");
    }

    #[test]
    fn bm25_no_match_scores_zero() {
        let docs = &["hello world", "foo bar"];
        let scores = bm25_scores("xyz", docs);
        assert_eq!(scores[0], 0.0);
        assert_eq!(scores[1], 0.0);
    }

    #[test]
    fn bm25_single_document() {
        let docs = &["the quick brown fox"];
        let scores = bm25_scores("quick fox", docs);
        assert!(scores[0] > 0.0, "matching terms should produce positive score");
    }
}
