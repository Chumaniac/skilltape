#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub code: String,
    pub level: DiagnosticLevel,
    pub file: String,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LintReport {
    pub errors: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
    pub files_checked: usize,
}

impl LintReport {
    pub(crate) fn push(
        &mut self,
        code: impl Into<String>,
        level: DiagnosticLevel,
        file: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        let diagnostic = Diagnostic {
            code: code.into(),
            level,
            file: file.into(),
            path: path.into(),
            message: message.into(),
        };

        match diagnostic.level {
            DiagnosticLevel::Error => self.errors.push(diagnostic),
            DiagnosticLevel::Warning => self.warnings.push(diagnostic),
        }
    }
}
