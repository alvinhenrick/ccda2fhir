//! The conversion report: what was lossy, unexpected or unsupported.

/// How serious an [`Issue`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Worth knowing, no data was lost.
    Info,
    /// Data was dropped, defaulted or approximated.
    Warning,
}

/// One thing worth telling the caller about. Never contains document values (no PHI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// How serious the issue is.
    pub severity: Severity,
    /// A stable, machine-readable code such as `patient.gender.unrecognized`.
    pub code: &'static str,
    /// A human-readable description with no document content.
    pub message: String,
}

/// Collects [`Issue`]s during a conversion.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Issues in the order they were found.
    pub issues: Vec<Issue>,
}

impl Report {
    /// Record a warning.
    pub fn warn(&mut self, code: &'static str, message: impl Into<String>) {
        self.issues.push(Issue {
            severity: Severity::Warning,
            code,
            message: message.into(),
        });
    }

    /// Record an informational note.
    pub fn info(&mut self, code: &'static str, message: impl Into<String>) {
        self.issues.push(Issue {
            severity: Severity::Info,
            code,
            message: message.into(),
        });
    }

    /// True if any warning was recorded.
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Warning)
    }
}
