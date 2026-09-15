//! Structured SoftGPU error taxonomy.
//!
//! Errors map to stable categories for CLI, future ABI status mapping, and
//! diagnostics. Phase 0 does not yet map these onto HSA status codes.

use std::fmt;

/// High-level error categories used across SoftGPU surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// User-supplied configuration or CLI arguments are invalid.
    Config,
    /// Device profile schema or provenance validation failed.
    Profile,
    /// Requested behavior is explicitly unsupported.
    Unsupported,
    /// Input failed structural validation (malformed / out of bounds).
    Validation,
    /// Internal invariant violated; should not occur in correct SoftGPU use.
    Internal,
    /// I/O failure while reading profiles or related artifacts.
    Io,
}

impl ErrorCategory {
    /// Stable machine-readable category id.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Profile => "profile",
            Self::Unsupported => "unsupported",
            Self::Validation => "validation",
            Self::Internal => "internal",
            Self::Io => "io",
        }
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// SoftGPU error with category and human-readable context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    category: ErrorCategory,
    message: String,
    remediation: Option<String>,
}

impl Error {
    pub fn new(category: ErrorCategory, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
            remediation: None,
        }
    }

    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }

    pub fn category(&self) -> ErrorCategory {
        self.category
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn remediation(&self) -> Option<&str> {
        self.remediation.as_deref()
    }

    /// Emit a single-line diagnostic suitable for CLI stderr.
    pub fn format_diagnostic(&self) -> String {
        match &self.remediation {
            Some(hint) => format!(
                "error[{}]: {}; remediation: {}",
                self.category.as_str(),
                self.message,
                hint
            ),
            None => format!("error[{}]: {}", self.category.as_str(), self.message),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format_diagnostic())
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::new(ErrorCategory::Io, value.to_string())
            .with_remediation("check that the path exists and is readable")
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error::new(
            ErrorCategory::Profile,
            format!("profile JSON parse/serialize failure: {value}"),
        )
        .with_remediation("see docs/architecture.md profile schema and docs/support-matrix.md")
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_includes_category_and_remediation() {
        let err = Error::new(
            ErrorCategory::Unsupported,
            "queues not implemented in phase 0",
        )
        .with_remediation("wait for phase 3; see docs/status.md");
        let text = err.format_diagnostic();
        assert!(text.contains("error[unsupported]"));
        assert!(text.contains("queues not implemented"));
        assert!(text.contains("phase 3"));
        assert_eq!(err.category(), ErrorCategory::Unsupported);
        assert!(err.message().contains("queues"));
        assert!(err.remediation().unwrap().contains("phase 3"));
        assert_eq!(err.to_string(), text);
    }

    #[test]
    fn categories_and_from_io_json_are_stable() {
        for (cat, id) in [
            (ErrorCategory::Config, "config"),
            (ErrorCategory::Profile, "profile"),
            (ErrorCategory::Unsupported, "unsupported"),
            (ErrorCategory::Validation, "validation"),
            (ErrorCategory::Internal, "internal"),
            (ErrorCategory::Io, "io"),
        ] {
            assert_eq!(cat.as_str(), id);
            assert_eq!(cat.to_string(), id);
        }

        let io: Error = std::io::Error::new(std::io::ErrorKind::NotFound, "missing").into();
        assert_eq!(io.category(), ErrorCategory::Io);
        assert!(io.remediation().is_some());

        let json: Error = serde_json::from_str::<serde_json::Value>("{")
            .unwrap_err()
            .into();
        assert_eq!(json.category(), ErrorCategory::Profile);
    }
}
