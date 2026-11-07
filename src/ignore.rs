//! Inline ignores: `# xplint-ignore: rule-a, rule-b`.
//!
//! * Trailing comment on a line: ignores findings reported on that line.
//! * Comment-only line: ignores findings reported on the next line.
//! * Comment-only line in a document's header (before its first content line,
//!   e.g. right after `---`): ignores those rules for the whole document.

use crate::diagnostic::Diagnostic;
use crate::loader::split_documents;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

const MARKER: &str = "xplint-ignore:";

/// Rule ids named by a `# xplint-ignore: ...` comment on `line`, and whether
/// the line holds only that comment.
fn parse_line(line: &str) -> Option<(Vec<String>, bool)> {
    let comment_only = line.trim_start().starts_with('#');
    for (i, _) in line.match_indices('#') {
        let rest = line[i + 1..].trim_start();
        if let Some(ids) = rest.strip_prefix(MARKER) {
            let ids = ids
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            return Some((ids, comment_only));
        }
    }
    None
}

/// Ignore table for one file's text.
#[derive(Debug, Default)]
pub struct Ignores {
    /// (first line, end line exclusive, rules) per document header.
    documents: Vec<(usize, usize, BTreeSet<String>)>,
    /// line -> rules ignored on that line.
    lines: HashMap<usize, BTreeSet<String>>,
}

impl Ignores {
    pub fn parse(text: &str) -> Ignores {
        let mut ig = Ignores::default();
        let all: Vec<&str> = text.lines().collect();
        let chunks = split_documents(text);
        let mut header_comment_lines = BTreeSet::new();
        for (n, chunk) in chunks.iter().enumerate() {
            // A document spans from its first line up to the next document's
            // `---` separator (which sits on the line before its start_line).
            let end = chunks
                .get(n + 1)
                .map_or(all.len() + 1, |next| next.start_line);
            let mut rules = BTreeSet::new();
            for (offset, l) in chunk.text.lines().enumerate() {
                let t = l.trim();
                if t.is_empty() {
                    continue;
                }
                if !t.starts_with('#') {
                    break; // first content line ends the header
                }
                if let Some((ids, _)) = parse_line(l) {
                    rules.extend(ids);
                    header_comment_lines.insert(chunk.start_line + offset);
                }
            }
            if !rules.is_empty() {
                ig.documents.push((chunk.start_line, end, rules));
            }
        }
        for (i, l) in all.iter().enumerate() {
            let lineno = i + 1;
            if header_comment_lines.contains(&lineno) {
                continue;
            }
            if let Some((ids, comment_only)) = parse_line(l) {
                let target = if comment_only { lineno + 1 } else { lineno };
                ig.lines.entry(target).or_default().extend(ids);
            }
        }
        ig
    }

    pub fn is_ignored(&self, rule: &str, line: usize) -> bool {
        self.lines.get(&line).is_some_and(|s| s.contains(rule))
            || self
                .documents
                .iter()
                .any(|(a, b, s)| (*a..*b).contains(&line) && s.contains(rule))
    }
}

/// Remove diagnostics suppressed by inline comments in their source files.
/// Files that cannot be read are left unfiltered.
pub fn filter_ignored(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut cache: HashMap<PathBuf, Ignores> = HashMap::new();
    diagnostics
        .into_iter()
        .filter(|d| {
            let ig = cache.entry(d.file.clone()).or_insert_with(|| {
                std::fs::read_to_string(&d.file)
                    .map(|t| Ignores::parse(&t))
                    .unwrap_or_default()
            });
            !ig.is_ignored(&d.rule, d.line)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_comment_ignores_its_line_only() {
        let ig = Ignores::parse("a: 1\nb: 2 # xplint-ignore: r1\nc: 3\n");
        assert!(ig.is_ignored("r1", 2));
        assert!(!ig.is_ignored("r1", 1));
        assert!(!ig.is_ignored("r1", 3));
        assert!(!ig.is_ignored("r2", 2));
    }

    #[test]
    fn comment_only_line_ignores_next_line() {
        let ig = Ignores::parse("a: 1\n# xplint-ignore: r1, r2\nb: 2\nc: 3\n");
        assert!(ig.is_ignored("r1", 3) && ig.is_ignored("r2", 3));
        assert!(!ig.is_ignored("r1", 4));
    }

    #[test]
    fn header_comment_ignores_whole_document_only() {
        let t = "# xplint-ignore: r1\na: 1\nb: 2\n---\n# xplint-ignore: r2\nc: 1\n---\nd: 1\n";
        let ig = Ignores::parse(t);
        assert!(ig.is_ignored("r1", 1) && ig.is_ignored("r1", 3));
        assert!(!ig.is_ignored("r1", 6));
        assert!(ig.is_ignored("r2", 6));
        assert!(!ig.is_ignored("r2", 3));
        assert!(!ig.is_ignored("r1", 8) && !ig.is_ignored("r2", 8));
    }

    #[test]
    fn unrelated_comments_are_ignored() {
        let ig = Ignores::parse("# just a note\na: 1 # xplint-ignorex\n");
        assert!(!ig.is_ignored("x", 1) && !ig.is_ignored("x", 2));
    }
}
