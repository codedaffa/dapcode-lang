use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub file_id: usize,
}

impl Span {
    pub const DUMMY: Span = Span {
        start: 0,
        end: 0,
        file_id: 0,
    };

    pub fn new(start: usize, end: usize, file_id: usize) -> Self {
        Span { start, end, file_id }
    }

    pub fn merge(self, other: Span) -> Span {
        if self.file_id != other.file_id {
            return self;
        }
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            file_id: self.file_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: usize,
    pub path: PathBuf,
    pub src: Arc<str>,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(id: usize, path: PathBuf, src: String) -> Self {
        let mut line_starts = vec![0];
        for (idx, b) in src.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(idx + 1);
            }
        }
        SourceFile {
            id,
            path,
            src: Arc::from(src),
            line_starts,
        }
    }

    pub fn line_col(&self, byte_offset: usize) -> (usize, usize) {
        let offset = byte_offset.min(self.src.len());
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        let line_start = self.line_starts[line_idx];
        let col = offset - line_start + 1;
        (line_idx + 1, col)
    }

    pub fn get_line(&self, line_num: usize) -> Option<&str> {
        if line_num == 0 || line_num > self.line_starts.len() {
            return None;
        }
        let line_idx = line_num - 1;
        let start = self.line_starts[line_idx];
        let end = if line_idx + 1 < self.line_starts.len() {
            let next_start = self.line_starts[line_idx + 1];
            if next_start > 0 && self.src.as_bytes()[next_start - 1] == b'\n' {
                if next_start > 1 && self.src.as_bytes()[next_start - 2] == b'\r' {
                    next_start - 2
                } else {
                    next_start - 1
                }
            } else {
                next_start
            }
        } else {
            self.src.len()
        };
        Some(&self.src[start..end])
    }
}

#[derive(Debug, Default, Clone)]
pub struct SourceMap {
    files: Vec<Arc<SourceFile>>,
}

impl SourceMap {
    pub fn new() -> Self {
        SourceMap { files: Vec::new() }
    }

    pub fn add_file(&mut self, path: impl AsRef<Path>, src: String) -> (usize, Arc<SourceFile>) {
        let id = self.files.len();
        let file = Arc::new(SourceFile::new(id, path.as_ref().to_path_buf(), src));
        self.files.push(Arc::clone(&file));
        (id, file)
    }

    pub fn get_file(&self, id: usize) -> Option<Arc<SourceFile>> {
        self.files.get(id).cloned()
    }
}
