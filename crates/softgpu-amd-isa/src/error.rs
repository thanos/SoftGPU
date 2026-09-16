//! SoftGPU ISA errors and traps (fail closed; no silent corruption).

use std::fmt;

/// Categories for SoftGPU ISA diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsaErrorKind {
    Config,
    Validation,
    Unsupported,
    Trap,
}

/// Fail-closed ISA error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsaError {
    kind: IsaErrorKind,
    message: String,
    remediation: Option<String>,
}

impl IsaError {
    pub fn new(kind: IsaErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            remediation: None,
        }
    }

    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }

    pub fn kind(&self) -> IsaErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn format_diagnostic(&self) -> String {
        let mut s = format!("[{:?}] {}", self.kind, self.message);
        if let Some(r) = &self.remediation {
            s.push_str("\n  remediation: ");
            s.push_str(r);
        }
        s
    }
}

impl fmt::Display for IsaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_diagnostic())
    }
}

impl std::error::Error for IsaError {}

pub type Result<T> = std::result::Result<T, IsaError>;

/// Why SoftGPU stopped before applying an instruction effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrapKind {
    UnsupportedEncoding {
        word: u32,
        pc: u32,
    },
    UnsupportedOpcode {
        format: &'static str,
        op: u32,
        word: u32,
        pc: u32,
    },
    UnsupportedOperand {
        encoding: u8,
        role: &'static str,
        pc: u32,
    },
    MisalignedPc {
        pc: u32,
    },
    FetchOutOfBounds {
        pc: u32,
        len: usize,
    },
    Halted,
}

impl TrapKind {
    pub fn to_error(&self) -> IsaError {
        match self {
            TrapKind::UnsupportedEncoding { word, pc } => IsaError::new(
                IsaErrorKind::Trap,
                format!("unsupported encoding word=0x{word:08x} at pc=0x{pc:x}"),
            )
            .with_remediation(
                "SoftGPU Phase 10 subset is softgpu-gfx1201-salu-v1; see docs/articles/11-decoding-amdgpu-isa.md",
            ),
            TrapKind::UnsupportedOpcode {
                format,
                op,
                word,
                pc,
            } => IsaError::new(
                IsaErrorKind::Trap,
                format!(
                    "unsupported {format} opcode={op} word=0x{word:08x} at pc=0x{pc:x}"
                ),
            )
            .with_remediation("extend SoftGPU tables only with sourced goldens; do not invent opcodes"),
            TrapKind::UnsupportedOperand {
                encoding,
                role,
                pc,
            } => IsaError::new(
                IsaErrorKind::Trap,
                format!("unsupported {role} operand encoding=0x{encoding:02x} at pc=0x{pc:x}"),
            )
            .with_remediation(
                "Phase 10 supports SGPR 0..=105 and inline constants 0..=64 / -1 only",
            ),
            TrapKind::MisalignedPc { pc } => IsaError::new(
                IsaErrorKind::Trap,
                format!("misaligned PC 0x{pc:x} (must be 4-byte aligned for SALU words)"),
            ),
            TrapKind::FetchOutOfBounds { pc, len } => IsaError::new(
                IsaErrorKind::Trap,
                format!("fetch out of bounds pc=0x{pc:x} code_len={len}"),
            ),
            TrapKind::Halted => IsaError::new(
                IsaErrorKind::Validation,
                "machine already halted (s_endpgm)",
            ),
        }
    }
}
