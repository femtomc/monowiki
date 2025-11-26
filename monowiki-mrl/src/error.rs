use std::fmt;

// Re-export Span from shared types
pub use monowiki_types::Span;

/// Errors that can occur during MRL processing
#[derive(Debug, thiserror::Error)]
pub enum MrlError {
    #[error("Lexer error at {span:?}: {message}")]
    LexerError { span: Span, message: String },

    #[error("Parser error at {span:?}: {message}")]
    ParserError { span: Span, message: String },

    #[error("Type error at {span:?}: {message}")]
    TypeError { span: Span, message: String },

    #[error("Hygiene error at {span:?}: {message}")]
    HygieneError { span: Span, message: String },

    #[error("Expansion error at {span:?}: {message}")]
    ExpansionError { span: Span, message: String },

    #[error("Evaluation error at {span:?}: {message}")]
    EvaluationError { span: Span, message: String },

    #[error("Unbound identifier at {span:?}: {name}")]
    UnboundIdentifier { span: Span, name: String },

    #[error("Arity mismatch at {span:?}: expected {expected}, got {got}")]
    ArityMismatch {
        span: Span,
        expected: usize,
        got: usize,
    },

    #[error("Kind mismatch at {span:?}: expected {expected}, got {got}")]
    KindMismatch {
        span: Span,
        expected: String,
        got: String,
    },

    #[error("Invalid content nesting at {span:?}: Inline cannot contain Block")]
    InvalidContentNesting { span: Span },

    #[error("Stage level error at {span:?}: {message}")]
    StageLevelError { span: Span, message: String },

    #[error("Capability error at {span:?}: {message}")]
    CapabilityError { span: Span, message: String },
}

pub type Result<T> = std::result::Result<T, MrlError>;

/// Display context for better error messages
pub struct ErrorContext<'a> {
    pub source: &'a str,
    pub error: &'a MrlError,
}

impl<'a> ErrorContext<'a> {
    pub fn new(source: &'a str, error: &'a MrlError) -> Self {
        Self { source, error }
    }

    /// Get the source line containing the error
    pub fn source_line(&self) -> Option<&'a str> {
        let span = self.span();
        let start = span.start;

        // Find the start of the line
        let line_start = self.source[..start]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0);

        // Find the end of the line
        let line_end = self.source[start..]
            .find('\n')
            .map(|pos| start + pos)
            .unwrap_or(self.source.len());

        Some(&self.source[line_start..line_end])
    }

    /// Get line and column numbers (1-indexed)
    pub fn line_col(&self) -> (usize, usize) {
        let span = self.span();
        let start = span.start;

        let line = self.source[..start].matches('\n').count() + 1;
        let line_start = self.source[..start]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0);
        let col = start - line_start + 1;

        (line, col)
    }

    fn span(&self) -> Span {
        match self.error {
            MrlError::LexerError { span, .. } => *span,
            MrlError::ParserError { span, .. } => *span,
            MrlError::TypeError { span, .. } => *span,
            MrlError::HygieneError { span, .. } => *span,
            MrlError::ExpansionError { span, .. } => *span,
            MrlError::EvaluationError { span, .. } => *span,
            MrlError::UnboundIdentifier { span, .. } => *span,
            MrlError::ArityMismatch { span, .. } => *span,
            MrlError::KindMismatch { span, .. } => *span,
            MrlError::InvalidContentNesting { span } => *span,
            MrlError::StageLevelError { span, .. } => *span,
            MrlError::CapabilityError { span, .. } => *span,
        }
    }
}

impl<'a> fmt::Display for ErrorContext<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (line, col) = self.line_col();
        writeln!(f, "Error at line {}, column {}:", line, col)?;
        writeln!(f, "  {}", self.error)?;

        if let Some(source_line) = self.source_line() {
            writeln!(f, "")?;
            writeln!(f, "  {}", source_line)?;
            let span = self.span();
            let col_start = col - 1;
            let col_end = col_start + span.len().min(source_line.len() - col_start);
            let indicator = " ".repeat(col_start) + &"^".repeat((col_end - col_start).max(1));
            writeln!(f, "  {}", indicator)?;
        }

        Ok(())
    }
}
