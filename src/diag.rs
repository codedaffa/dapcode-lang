use std::fmt;
use colored::*;
use crate::span::{SourceMap, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Note,
    Help,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub span: Option<Span>,
    pub label: Option<String>,
    pub notes: Vec<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Diagnostic {
            level: DiagnosticLevel::Error,
            message: message.into(),
            span: None,
            label: None,
            notes: Vec::new(),
            help: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Diagnostic {
            level: DiagnosticLevel::Warning,
            message: message.into(),
            span: None,
            label: None,
            notes: Vec::new(),
            help: None,
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn render(&self, source_map: &SourceMap) -> String {
        let mut out = String::new();

        let (prefix, color_fn): (&str, fn(&str) -> ColoredString) = match self.level {
            DiagnosticLevel::Error => ("kesalahan (error)", |s| s.red().bold()),
            DiagnosticLevel::Warning => ("peringatan (warning)", |s| s.yellow().bold()),
            DiagnosticLevel::Note => ("catatan (note)", |s| s.cyan().bold()),
            DiagnosticLevel::Help => ("bantuan (help)", |s| s.green().bold()),
        };

        if let Some(span) = self.span {
            if let Some(file) = source_map.get_file(span.file_id) {
                let (line, col) = file.line_col(span.start);
                out.push_str(&format!(
                    "{}: {}\n",
                    color_fn(&format!("{}: {}:{}:{}", prefix, file.path.display(), line, col)),
                    self.message.bold()
                ));

                if let Some(line_content) = file.get_line(line) {
                    let line_num_str = line.to_string();
                    let pad = " ".repeat(line_num_str.len());

                    out.push_str(&format!(" {} {}\n", pad.blue().bold(), "|".blue().bold()));
                    out.push_str(&format!(
                        " {} {} {}\n",
                        line_num_str.blue().bold(),
                        "|".blue().bold(),
                        line_content
                    ));

                    let span_len = if span.end > span.start {
                        span.end - span.start
                    } else {
                        1
                    };
                    let indent = " ".repeat(col.saturating_sub(1));
                    let underline_char = match self.level {
                        DiagnosticLevel::Error => "^",
                        _ => "~",
                    };
                    let underline = underline_char.repeat(span_len.max(1));
                    let colored_underline = match self.level {
                        DiagnosticLevel::Error => underline.red().bold(),
                        DiagnosticLevel::Warning => underline.yellow().bold(),
                        _ => underline.cyan().bold(),
                    };

                    let label_str = self
                        .label
                        .as_deref()
                        .map(|l| format!(" {}", l))
                        .unwrap_or_default();

                    out.push_str(&format!(
                        " {} {} {}{}{}\n",
                        pad.blue().bold(),
                        "|".blue().bold(),
                        indent,
                        colored_underline,
                        label_str.cyan()
                    ));
                    out.push_str(&format!(" {} {}\n", pad.blue().bold(), "|".blue().bold()));
                }
            } else {
                out.push_str(&format!("{}: {}\n", color_fn(prefix), self.message.bold()));
            }
        } else {
            out.push_str(&format!("{}: {}\n", color_fn(prefix), self.message.bold()));
        }

        for note in &self.notes {
            out.push_str(&format!("  {}: {}\n", "catatan".cyan().bold(), note));
        }

        if let Some(help) = &self.help {
            out.push_str(&format!("  {}: {}\n", "bantuan".green().bold(), help));
        }

        out
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.level, self.message)
    }
}

pub type Result<T> = std::result::Result<T, Diagnostic>;
