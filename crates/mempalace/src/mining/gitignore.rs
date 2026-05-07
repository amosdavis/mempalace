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
        let (negated, pat) = if let Some(stripped) = line.strip_prefix('!') {
            (true, stripped)
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
    if r.ends_with('/') {
        // Directory pattern (e.g. "target/"): match the dir and everything inside it
        r.push_str(".*$");
    } else {
        r.push_str("(?:/.*)?$");
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_star_matches_filename() {
        let re = Regex::new(&glob_to_regex("*.rs")).expect("regex");
        assert!(re.is_match("foo.rs"));
        assert!(re.is_match("bar/foo.rs"));
        assert!(!re.is_match("foo.txt"));
    }

    #[test]
    fn glob_double_star_matches_deeply_nested() {
        let re = Regex::new(&glob_to_regex("**/*.rs")).expect("regex");
        assert!(re.is_match("src/main.rs"));
        assert!(re.is_match("a/b/c/d.rs"));
    }

    #[test]
    fn glob_directory_pattern() {
        let re = Regex::new(&glob_to_regex("node_modules/")).expect("regex");
        assert!(re.is_match("node_modules/"));
        assert!(re.is_match("node_modules/foo.js"));
    }

    #[test]
    fn glob_question_mark() {
        let re = Regex::new(&glob_to_regex("?.txt")).expect("regex");
        assert!(re.is_match("a.txt"));
        assert!(!re.is_match("ab.txt"));
    }

    #[test]
    fn glob_escapes_special_chars() {
        let re = Regex::new(&glob_to_regex("file.txt")).expect("regex");
        assert!(re.is_match("file.txt"));
        assert!(!re.is_match("filextxt"));
    }

    #[test]
    fn filter_empty_gitignore() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(".gitignore"), "# just a comment\n\n").expect("write");
        let filter = GitignoreFilter::new(tmp.path());
        assert!(!filter.is_ignored(&tmp.path().join("foo.rs")));
    }

    #[test]
    fn filter_basic_pattern() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(".gitignore"), "*.log\ntarget/\n").expect("write");
        let filter = GitignoreFilter::new(tmp.path());
        assert!(filter.is_ignored(&tmp.path().join("debug.log")));
        assert!(filter.is_ignored(&tmp.path().join("target/release/bin")));
        assert!(!filter.is_ignored(&tmp.path().join("src/main.rs")));
    }

    #[test]
    fn filter_negation() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(".gitignore"), "*.log\n!important.log\n").expect("write");
        let filter = GitignoreFilter::new(tmp.path());
        assert!(filter.is_ignored(&tmp.path().join("debug.log")));
        assert!(!filter.is_ignored(&tmp.path().join("important.log")));
    }

    #[test]
    fn filter_no_gitignore_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let filter = GitignoreFilter::new(tmp.path());
        assert!(!filter.is_ignored(&tmp.path().join("anything.rs")));
    }
}
