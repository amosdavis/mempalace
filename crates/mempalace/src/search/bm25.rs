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
