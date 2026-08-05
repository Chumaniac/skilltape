use serde::Serialize;
use skilltape_core::{Diagnostic, DiagnosticLevel, LintReport};

#[derive(Serialize)]
struct JsonReport<'a> {
    files_checked: usize,
    errors: Vec<JsonDiagnostic<'a>>,
    warnings: Vec<JsonDiagnostic<'a>>,
}

#[derive(Serialize)]
struct JsonDiagnostic<'a> {
    code: &'a str,
    level: &'static str,
    file: &'a str,
    path: &'a str,
    message: &'a str,
}

pub(crate) fn human_report(report: &LintReport) -> String {
    let mut output = String::new();

    for diagnostic in report.errors.iter().chain(report.warnings.iter()) {
        let level = match diagnostic.level {
            DiagnosticLevel::Error => "error",
            DiagnosticLevel::Warning => "warning",
        };
        output.push_str(&format_diagnostic(diagnostic, level));
    }

    output.push_str(&format!(
        "Checked {} files: {} errors, {} warnings\n",
        report.files_checked,
        report.errors.len(),
        report.warnings.len()
    ));

    output
}

pub(crate) fn json_report(report: &LintReport) -> String {
    let report = JsonReport {
        files_checked: report.files_checked,
        errors: report
            .errors
            .iter()
            .map(|diagnostic| JsonDiagnostic::from_diagnostic(diagnostic))
            .collect(),
        warnings: report
            .warnings
            .iter()
            .map(|diagnostic| JsonDiagnostic::from_diagnostic(diagnostic))
            .collect(),
    };

    serde_json::to_string(&report).expect("lint report JSON serialization cannot fail")
}

impl<'a> JsonDiagnostic<'a> {
    fn from_diagnostic(diagnostic: &'a Diagnostic) -> Self {
        Self {
            code: &diagnostic.code,
            level: match diagnostic.level {
                DiagnosticLevel::Error => "error",
                DiagnosticLevel::Warning => "warning",
            },
            file: &diagnostic.file,
            path: &diagnostic.path,
            message: &diagnostic.message,
        }
    }
}

fn format_diagnostic(diagnostic: &Diagnostic, level: &str) -> String {
    format!(
        "{level}[{}] {}:{}\n  {}\n",
        diagnostic.code, diagnostic.file, diagnostic.path, diagnostic.message
    )
}
