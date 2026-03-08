use std::path::Path;

use crate::config::PairConfig;

// Built-in patterns always ignored — regardless of user config
const BUILTIN: &[&str] = &[".hard-sync-trash", ".hard_sync_cli", ".hardsyncignore"];

pub struct IgnoreList {
    patterns: Vec<String>,
}

impl IgnoreList {
    /// Build the full ignore list for a pair:
    /// built-ins + per-pair config list + .hardsyncignore file in source dir
    pub fn from_pair(pair: &PairConfig, source_path: &Path) -> Self {
        let mut patterns: Vec<String> = BUILTIN.iter().map(|s| s.to_string()).collect();

        for p in &pair.ignore {
            patterns.push(p.clone());
        }

        // Load .hardsyncignore from source dir
        let ignore_file = source_path.join(".hardsyncignore");
        if let Ok(content) = std::fs::read_to_string(&ignore_file) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                patterns.push(line.to_string());
            }
        }

        Self { patterns }
    }

    /// Returns true if the relative path (forward-slash separated) should be ignored.
    ///
    /// Matching rules (v1, simple but covers all common cases):
    /// - If a pattern matches any single path component exactly → ignored
    ///   (e.g. "node_modules" ignores "node_modules/foo/bar.js")
    /// - If the relative path starts with the pattern → ignored
    ///   (e.g. ".hard-sync-trash" ignores ".hard-sync-trash/foo.txt")
    /// - If the pattern contains a '/' it is matched against the full path prefix
    ///   (e.g. "src/generated" ignores "src/generated/foo.rs")
    pub fn is_ignored(&self, rel: &str) -> bool {
        let components: Vec<&str> = rel.split('/').collect();
        for pattern in &self.patterns {
            if pattern.contains('/') {
                // Full path prefix match
                if rel.starts_with(pattern.as_str()) {
                    return true;
                }
            } else {
                // Component match or leading segment match
                if components.iter().any(|c| *c == pattern.as_str()) {
                    return true;
                }
                if rel.starts_with(pattern.as_str()) {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(patterns: &[&str]) -> IgnoreList {
        IgnoreList {
            patterns: patterns.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn ignores_component_match() {
        let l = list(&["node_modules", ".git"]);
        assert!(l.is_ignored("node_modules/lodash/index.js"));
        assert!(l.is_ignored("packages/app/node_modules/foo.js"));
        assert!(l.is_ignored(".git/config"));
        assert!(!l.is_ignored("src/main.rs"));
    }

    #[test]
    fn ignores_prefix_match() {
        let l = list(&[".hard-sync-trash"]);
        assert!(l.is_ignored(".hard-sync-trash/2026-03-07_foo.txt"));
        assert!(!l.is_ignored("src/hard-sync-trash.rs"));
    }

    #[test]
    fn ignores_path_prefix() {
        let l = list(&["src/generated"]);
        assert!(l.is_ignored("src/generated/foo.rs"));
        assert!(!l.is_ignored("src/main.rs"));
    }

    #[test]
    fn builtin_always_ignored() {
        // Built-ins are part of patterns when constructed via from_pair,
        // test them directly here
        let l = list(BUILTIN);
        assert!(l.is_ignored(".hard-sync-trash/foo.txt"));
        assert!(l.is_ignored(".hard_sync_cli/tracker.json"));
        assert!(l.is_ignored(".hardsyncignore"));
    }
}
