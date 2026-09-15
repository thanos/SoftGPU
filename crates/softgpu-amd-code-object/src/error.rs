//! Fail-closed errors for code-object parsing.

use std::fmt;

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, CodeObjectError>;

/// Structured parse / validation failures (never panics across the API).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeObjectError {
    Io(String),
    TooLarge { size: usize, max: usize },
    Truncated { detail: String },
    NotElf,
    UnsupportedClass { class: u8 },
    UnsupportedEndian { data: u8 },
    UnsupportedElfType { etype: u16 },
    BadHeader { detail: String },
    SectionFault { detail: String },
    NoteNotFound,
    UnsupportedNote { detail: String },
    MsgPack { detail: String },
    Metadata { detail: String },
    UnsupportedMetadataVersion { major: u32, minor: u32 },
    UnsupportedTarget { target: String },
    LimitExceeded { detail: String },
}

impl fmt::Display for CodeObjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(s) => write!(f, "io: {s}"),
            Self::TooLarge { size, max } => write!(f, "file too large ({size} > {max})"),
            Self::Truncated { detail } => write!(f, "truncated: {detail}"),
            Self::NotElf => write!(f, "not an ELF object"),
            Self::UnsupportedClass { class } => write!(f, "unsupported ELF class {class}"),
            Self::UnsupportedEndian { data } => write!(f, "unsupported ELF endian {data}"),
            Self::UnsupportedElfType { etype } => write!(f, "unsupported ELF type {etype}"),
            Self::BadHeader { detail } => write!(f, "bad ELF header: {detail}"),
            Self::SectionFault { detail } => write!(f, "section fault: {detail}"),
            Self::NoteNotFound => write!(f, "AMDGPU metadata note not found"),
            Self::UnsupportedNote { detail } => write!(f, "unsupported note: {detail}"),
            Self::MsgPack { detail } => write!(f, "msgpack: {detail}"),
            Self::Metadata { detail } => write!(f, "metadata: {detail}"),
            Self::UnsupportedMetadataVersion { major, minor } => {
                write!(f, "unsupported amdhsa.version [{major}, {minor}]")
            }
            Self::UnsupportedTarget { target } => write!(f, "unsupported target '{target}'"),
            Self::LimitExceeded { detail } => write!(f, "limit exceeded: {detail}"),
        }
    }
}

impl std::error::Error for CodeObjectError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_covers_variants() {
        let cases = [
            CodeObjectError::Io("x".into()),
            CodeObjectError::TooLarge { size: 9, max: 1 },
            CodeObjectError::Truncated { detail: "t".into() },
            CodeObjectError::NotElf,
            CodeObjectError::UnsupportedClass { class: 1 },
            CodeObjectError::UnsupportedEndian { data: 2 },
            CodeObjectError::UnsupportedElfType { etype: 3 },
            CodeObjectError::BadHeader { detail: "h".into() },
            CodeObjectError::SectionFault { detail: "s".into() },
            CodeObjectError::NoteNotFound,
            CodeObjectError::UnsupportedNote { detail: "n".into() },
            CodeObjectError::MsgPack { detail: "m".into() },
            CodeObjectError::Metadata {
                detail: "md".into(),
            },
            CodeObjectError::UnsupportedMetadataVersion { major: 1, minor: 2 },
            CodeObjectError::UnsupportedTarget {
                target: "gfx000".into(),
            },
            CodeObjectError::LimitExceeded { detail: "l".into() },
        ];
        for err in cases {
            let s = err.to_string();
            assert!(!s.is_empty(), "{err:?}");
        }
    }
}
