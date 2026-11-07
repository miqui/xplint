//! YAML paths and a line index so diagnostics can point at source lines.
//!
//! The indexer is a small indentation-based scanner for block-style YAML. For
//! flow style (`{a: b}`, `[a, b]`) a path resolves to the line of its nearest
//! indexed ancestor.

use std::collections::HashMap;
use std::fmt;

/// One path segment.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Seg {
    Key(String),
    Idx(usize),
}

/// A path from the document root to a node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct YamlPath(pub Vec<Seg>);

impl YamlPath {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a copy extended with a mapping key.
    pub fn key(&self, k: &str) -> Self {
        let mut v = self.0.clone();
        v.push(Seg::Key(k.to_string()));
        YamlPath(v)
    }

    /// Return a copy extended with a sequence index.
    pub fn idx(&self, i: usize) -> Self {
        let mut v = self.0.clone();
        v.push(Seg::Idx(i));
        YamlPath(v)
    }
}

impl fmt::Display for YamlPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, s) in self.0.iter().enumerate() {
            match s {
                Seg::Key(k) => {
                    if i > 0 {
                        f.write_str(".")?;
                    }
                    f.write_str(k)?;
                }
                Seg::Idx(n) => write!(f, "[{n}]")?,
            }
        }
        Ok(())
    }
}

/// Map of every indexed path to its 0-based line within the text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineIndex {
    lines: HashMap<Vec<Seg>, usize>,
}

impl LineIndex {
    /// Index block-style YAML text.
    pub fn build(text: &str) -> Self {
        Indexer::default().run(text)
    }

    /// 0-based line of `path`, or of its nearest indexed ancestor (root = 0).
    pub fn resolve(&self, path: &YamlPath) -> usize {
        for n in (0..=path.0.len()).rev() {
            if let Some(l) = self.lines.get(&path.0[..n]) {
                return *l;
            }
        }
        0
    }
}

struct Frame {
    indent: usize,
    path: Vec<Seg>,
    is_seq: bool,
    next: usize,
}

#[derive(Default)]
struct Indexer {
    lines: HashMap<Vec<Seg>, usize>,
    stack: Vec<Frame>,
    pending: Option<Vec<Seg>>,
    skip_above: Option<usize>,
}

/// Split `key: value` / `key:`; returns (key, value text).
fn split_key(s: &str) -> Option<(String, &str)> {
    if let Some(q) = s.chars().next().filter(|c| *c == '"' || *c == '\'') {
        let end = s[1..].find(q)? + 1;
        let rest = s[end + 1..].trim_start();
        let value = rest.strip_prefix(':')?;
        if !(value.is_empty() || value.starts_with(' ')) {
            return None;
        }
        return Some((s[1..end].to_string(), value.trim()));
    }
    if s.starts_with(['[', '{', '#', '|', '>']) {
        return None;
    }
    let pos = if let Some(stripped) = s.strip_suffix(':') {
        stripped.len()
    } else {
        s.find(": ")?
    };
    let key = s[..pos].trim_end();
    (!key.is_empty()).then(|| (key.to_string(), s[pos + 1..].trim()))
}

impl Indexer {
    fn run(mut self, text: &str) -> LineIndex {
        for (ln, raw) in text.lines().enumerate() {
            let content = raw.trim_start();
            if content.is_empty() || content.starts_with('#') {
                continue;
            }
            let indent = raw.len() - content.len();
            if let Some(s) = self.skip_above {
                if indent > s {
                    continue;
                }
                self.skip_above = None;
            }
            self.line(ln, indent, content);
        }
        LineIndex { lines: self.lines }
    }

    fn line(&mut self, ln: usize, indent: usize, content: &str) {
        let is_dash = content == "-" || content.starts_with("- ");
        while let Some(top) = self.stack.last() {
            if top.indent > indent || (top.is_seq && top.indent == indent && !is_dash) {
                self.stack.pop();
            } else {
                break;
            }
        }
        if is_dash {
            self.dash(ln, indent, content);
        } else if split_key(content).is_some() {
            self.key_line(ln, indent, content, indent);
        }
        // anything else: scalar/flow continuation, ignored
    }

    fn dash(&mut self, ln: usize, indent: usize, content: &str) {
        let top_is_seq = matches!(self.stack.last(), Some(f) if f.is_seq && f.indent == indent);
        if !top_is_seq {
            let path = self
                .pending
                .take()
                .or_else(|| self.stack.last().map(|f| f.path.clone()))
                .unwrap_or_default();
            self.stack.push(Frame {
                indent,
                path,
                is_seq: true,
                next: 0,
            });
        }
        let frame = self.stack.last_mut().expect("seq frame");
        let mut item = frame.path.clone();
        item.push(Seg::Idx(frame.next));
        frame.next += 1;
        self.lines.entry(item.clone()).or_insert(ln);
        let rest = &content[1..];
        let rest_trim = rest.trim_start();
        let offset = indent + 1 + (rest.len() - rest_trim.len());
        if rest_trim.is_empty() || rest_trim.starts_with('#') {
            self.pending = Some(item);
        } else if split_key(rest_trim).is_some() {
            self.stack.push(Frame {
                indent: offset,
                path: item,
                is_seq: false,
                next: 0,
            });
            self.key_line(ln, offset, rest_trim, offset);
        } else {
            self.pending = None;
        }
    }

    fn key_line(&mut self, ln: usize, indent: usize, content: &str, key_col: usize) {
        let ok = matches!(self.stack.last(), Some(f) if !f.is_seq && f.indent == indent);
        if !ok {
            match (self.stack.last(), self.pending.take()) {
                (_, Some(path)) => self.stack.push(Frame {
                    indent,
                    path,
                    is_seq: false,
                    next: 0,
                }),
                (None, None) => self.stack.push(Frame {
                    indent,
                    path: Vec::new(),
                    is_seq: false,
                    next: 0,
                }),
                (Some(_), None) => return, // continuation line
            }
        }
        let (key, value) = split_key(content).expect("checked by caller");
        let mut path = self.stack.last().expect("map frame").path.clone();
        path.push(Seg::Key(key));
        self.lines.entry(path.clone()).or_insert(ln);
        if value.is_empty() || value.starts_with('#') {
            self.pending = Some(path);
        } else {
            self.pending = None;
            if value.starts_with(['|', '>']) {
                self.skip_above = Some(key_col);
            }
        }
    }
}

/// Parse a Crossplane field path like `spec.items[0].name` or `a["b.c"]`.
/// Returns `None` for syntax this parser does not understand (e.g. `[*]`).
pub fn parse_field_path(s: &str) -> Option<Vec<Seg>> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    let flush = |cur: &mut String, out: &mut Vec<Seg>| {
        if !cur.is_empty() {
            out.push(Seg::Key(std::mem::take(cur)));
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '.' => {
                if cur.is_empty() && !matches!(out.last(), Some(Seg::Idx(_))) {
                    return None;
                }
                flush(&mut cur, &mut out);
            }
            '[' => {
                flush(&mut cur, &mut out);
                let mut inner = String::new();
                loop {
                    match chars.next()? {
                        ']' => break,
                        ch => inner.push(ch),
                    }
                }
                if let Ok(n) = inner.parse::<usize>() {
                    out.push(Seg::Idx(n));
                } else if inner.len() >= 2
                    && ((inner.starts_with('"') && inner.ends_with('"'))
                        || (inner.starts_with('\'') && inner.ends_with('\'')))
                {
                    out.push(Seg::Key(inner[1..inner.len() - 1].to_string()));
                } else if !inner.is_empty() && inner != "*" {
                    out.push(Seg::Key(inner));
                } else {
                    return None;
                }
            }
            _ => cur.push(c),
        }
    }
    flush(&mut cur, &mut out);
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(parts: &[&str]) -> YamlPath {
        YamlPath(
            parts
                .iter()
                .map(|s| match s.parse::<usize>() {
                    Ok(n) => Seg::Idx(n),
                    Err(_) => Seg::Key((*s).to_string()),
                })
                .collect(),
        )
    }

    const DOC: &str = "\
apiVersion: v1
spec:
  pipeline:
    - step: a
      functionRef:
        name: fa
    - step: b
      input:
        resources:
        - name: r
          base:
            spec: {x: 1}
  list:
  - one
  - two
  text: |
    spec:
      fake: 1
  after: yes
";

    #[test]
    fn indexes_maps_sequences_and_nested_items() {
        let ix = LineIndex::build(DOC);
        assert_eq!(ix.resolve(&p(&["apiVersion"])), 0);
        assert_eq!(ix.resolve(&p(&["spec", "pipeline", "0"])), 3);
        assert_eq!(
            ix.resolve(&p(&["spec", "pipeline", "0", "functionRef", "name"])),
            5
        );
        assert_eq!(ix.resolve(&p(&["spec", "pipeline", "1", "step"])), 6);
        assert_eq!(
            ix.resolve(&p(&[
                "spec",
                "pipeline",
                "1",
                "input",
                "resources",
                "0",
                "base",
                "spec"
            ])),
            11
        );
        assert_eq!(ix.resolve(&p(&["spec", "list", "1"])), 14);
    }

    #[test]
    fn block_scalar_contents_are_not_indexed() {
        let ix = LineIndex::build(DOC);
        assert_eq!(ix.resolve(&p(&["spec", "text"])), 15);
        assert_eq!(ix.resolve(&p(&["spec", "after"])), 18);
        // `spec.text.spec` was inside the scalar: falls back to the `text` line.
        assert_eq!(ix.resolve(&p(&["spec", "text", "spec"])), 15);
    }

    #[test]
    fn missing_path_falls_back_to_nearest_ancestor() {
        let ix = LineIndex::build(DOC);
        assert_eq!(ix.resolve(&p(&["spec", "nope", "deeper"])), 1);
        assert_eq!(ix.resolve(&p(&["zzz"])), 0);
    }

    #[test]
    fn display_formats_keys_and_indexes() {
        assert_eq!(
            p(&["spec", "pipeline", "0", "step"]).to_string(),
            "spec.pipeline[0].step"
        );
    }

    #[test]
    fn parses_field_paths() {
        use Seg::*;
        assert_eq!(
            parse_field_path("spec.items[2].name"),
            Some(vec![
                Key("spec".into()),
                Key("items".into()),
                Idx(2),
                Key("name".into())
            ])
        );
        assert_eq!(
            parse_field_path("metadata.labels[\"a.b\"]"),
            Some(vec![
                Key("metadata".into()),
                Key("labels".into()),
                Key("a.b".into())
            ])
        );
        assert_eq!(parse_field_path("spec.items[*]"), None);
        assert_eq!(parse_field_path(""), None);
        assert_eq!(parse_field_path("a..b"), None);
    }
}
