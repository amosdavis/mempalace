use regex::Regex;
use std::path::{Path, PathBuf};

pub struct GitignoreFilter {
    patterns: Vec<GitignorePattern>,
    root: PathBuf,
}

struct GitignorePattern {
    regex: Regex,
    negated: bool,
}

impl GitignoreFilter {
    pub fn new(root: &Path) -> Self {
        let mut patterns = Vec::new();
        let gi = root.join(".gitignore");
        if let Ok(content) = std::fs::read_to_string(gi) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Some(p) = GitignorePattern::from_line(line) {
                    patterns.push(p);
                }
            }
        }
        Self { patterns, root: root.to_path_buf() }
    }

    pub fn is_ignored(&self, path: &Path) -> bool {
        let rel = match path.strip_prefix(&self.root) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => return false,
        };
        let mut ignored = false;
        for p in &self.patterns {
            if p.regex.is_match(&rel) {
                ignored = !p.negated;
            }
        }
        ignored
    }
}

impl GitignorePattern {
    fn from_line(line: &str) -> Option<Self> {
        let (negated, pat) = if line.starts_with('!') {
            (true, &line[1..])
        } else {
            (false, line)
        };
        let pat = pat.trim_start_matches('/');
        let re_str = glob_to_regex(pat);
        Regex::new(&re_str).ok().map(|regex| GitignorePattern { regex, negated })
    }
}

fn glob_to_regex(pattern: &str) -> String {
    let mut r = String::from("(?:^|/)");
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                r.push_str(".*");
                i += 2;
                if i < chars.len() && chars[i] == '/' { i += 1; }
            }
            '*' => { r.push_str("[^/]*"); i += 1; }
            '?' => { r.push_str("[^/]");  i += 1; }
            c @ ('.' | '+' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\') => {
                r.push('\\'); r.push(c); i += 1;
            }
            c => { r.push(c); i += 1; }
        }
    }
    r.push_str("(?:/.*)?$");
    r
}
