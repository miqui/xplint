//! Multi-document YAML loader that keeps document indexes and line offsets.

use crate::model::Resource;
use crate::yamlpath::{LineIndex, YamlPath};
use serde_yaml_ng::Value;
use std::path::{Path, PathBuf};

/// One YAML document from a file.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    /// 1-based document number within the file (empty documents are not counted).
    pub index: usize,
    /// 1-based line in the file where this document's text begins.
    pub start_line: usize,
    pub resource: Resource,
    /// Path-to-line index of this document's text.
    pub lines: LineIndex,
}

impl Document {
    /// 1-based line in the file for a YAML path (nearest ancestor if absent).
    pub fn line_of(&self, path: &YamlPath) -> usize {
        self.start_line + self.lines.resolve(path)
    }
}

/// A document together with the file it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceDoc {
    pub file: PathBuf,
    pub document: Document,
}

/// Raw text chunk of a multi-document stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk<'a> {
    pub index: usize,
    pub start_line: usize,
    pub text: &'a str,
}

/// Errors from loading YAML.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("{}: {source}", file.display())]
    Io {
        file: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{}:{line}: document {document}: {message}", file.display())]
    Parse {
        file: PathBuf,
        document: usize,
        /// 1-based line in the file.
        line: usize,
        message: String,
    },
}

fn is_separator(line: &str) -> bool {
    let line = line.trim_end();
    line == "---" || line.starts_with("--- #")
}

fn has_content(text: &str) -> bool {
    text.lines().any(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with('#')
    })
}

fn push_chunk<'a>(
    text: &'a str,
    start: usize,
    end: usize,
    line: usize,
    chunks: &mut Vec<Chunk<'a>>,
) {
    let t = &text[start..end];
    if has_content(t) {
        chunks.push(Chunk {
            index: chunks.len() + 1,
            start_line: line,
            text: t,
        });
    }
}

/// Split a stream on `---` lines. Blank/comment-only documents are dropped.
pub fn split_documents(text: &str) -> Vec<Chunk<'_>> {
    let mut chunks = Vec::new();
    let mut start = 0usize; // byte offset of current chunk
    let mut start_line = 1usize;
    let mut offset = 0usize;
    for (i, line) in text.split_inclusive('\n').enumerate() {
        if is_separator(line.trim_end_matches(['\n', '\r'])) {
            push_chunk(text, start, offset, start_line, &mut chunks);
            start = offset + line.len();
            start_line = i + 2;
        }
        offset += line.len();
    }
    push_chunk(text, start, text.len(), start_line, &mut chunks);
    chunks
}

/// Load all documents from `text`; `file` is used for error messages only.
pub fn load_str(file: &Path, text: &str) -> Result<Vec<Document>, LoadError> {
    split_documents(text)
        .into_iter()
        .map(|c| {
            let err = |line: usize, message: String| LoadError::Parse {
                file: file.to_path_buf(),
                document: c.index,
                line,
                message,
            };
            let value: Value = serde_yaml_ng::from_str(c.text).map_err(|e| {
                let line = e
                    .location()
                    .map_or(c.start_line, |l| c.start_line + l.line() - 1);
                err(line, e.to_string())
            })?;
            let resource =
                Resource::from_value(value).map_err(|e| err(c.start_line, e.to_string()))?;
            Ok(Document {
                index: c.index,
                start_line: c.start_line,
                resource,
                lines: LineIndex::build(c.text),
            })
        })
        .collect()
}

/// Read and load a file.
pub fn load_file(path: &Path) -> Result<Vec<Document>, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| LoadError::Io {
        file: path.to_path_buf(),
        source,
    })?;
    load_str(path, &text)
}

/// Load one file, or every `.yaml`/`.yml` file under a directory (recursive,
/// sorted by path so output is deterministic).
pub fn load_tree(path: &Path) -> Result<Vec<SourceDoc>, LoadError> {
    let mut files = Vec::new();
    if path.is_dir() {
        collect_files(path, &mut files)?;
    } else {
        files.push(path.to_path_buf());
    }
    files.sort();
    let mut out = Vec::new();
    for file in files {
        for document in load_file(&file)? {
            out.push(SourceDoc {
                file: file.clone(),
                document,
            });
        }
    }
    Ok(out)
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), LoadError> {
    let io = |source| LoadError::Io {
        file: dir.to_path_buf(),
        source,
    };
    for entry in std::fs::read_dir(dir).map_err(io)? {
        let p = entry.map_err(io)?.path();
        if p.is_dir() {
            collect_files(&p, out)?;
        } else if matches!(p.extension().and_then(|e| e.to_str()), Some("yaml" | "yml")) {
            out.push(p);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_with_line_offsets() {
        let t = "# c\n---\na: 1\n---\n\n---\nb: 2\n";
        let c = split_documents(t);
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].index, c[0].start_line), (1, 3));
        assert_eq!((c[1].index, c[1].start_line), (2, 7));
        assert_eq!(c[1].text, "b: 2\n");
    }

    #[test]
    fn no_separator_is_one_document_at_line_1() {
        let c = split_documents("a: 1\nb: 2");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].start_line, 1);
    }

    #[test]
    fn empty_input_has_no_documents() {
        assert!(split_documents("").is_empty());
        assert!(split_documents("---\n---\n").is_empty());
    }

    #[test]
    fn crlf_separators_work() {
        let c = split_documents("a: 1\r\n---\r\nb: 2\r\n");
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].start_line, 3);
    }

    #[test]
    fn parse_error_reports_file_document_and_absolute_line() {
        let err = load_str(Path::new("x.yaml"), "a: 1\n---\nb: [1\nc: 2\n").unwrap_err();
        let msg = err.to_string();
        assert!(msg.starts_with("x.yaml:"), "{msg}");
        assert!(msg.contains("document 2"), "{msg}");
        match err {
            LoadError::Parse { line, .. } => assert!(line >= 3),
            _ => panic!("expected parse error"),
        }
    }

    #[test]
    fn missing_file_is_io_error() {
        let err = load_file(Path::new("does/not/exist.yaml")).unwrap_err();
        assert!(matches!(err, LoadError::Io { .. }));
    }
}
