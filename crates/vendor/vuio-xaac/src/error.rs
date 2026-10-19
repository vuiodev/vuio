use core::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidConfig(&'static str),
    InvalidInput {
        expected: usize,
        actual: usize,
        context: &'static str,
    },
    NeedMoreInput,
    EndOfStream,
    InputTooLarge {
        capacity: usize,
        actual: usize,
    },
    AllocationFailed {
        size: usize,
        alignment: usize,
    },
    EncoderError(i32),
    DecoderError(i32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(message) => write!(f, "invalid configuration: {message}"),
            Self::InvalidInput {
                expected,
                actual,
                context,
            } => write!(
                f,
                "invalid input for {context}: expected {expected} bytes, got {actual}"
            ),
            Self::NeedMoreInput => write!(f, "decoder needs more input"),
            Self::EndOfStream => write!(f, "decoder reached end of stream"),
            Self::InputTooLarge { capacity, actual } => {
                write!(
                    f,
                    "input too large: capacity is {capacity} bytes, got {actual}"
                )
            }
            Self::AllocationFailed { size, alignment } => {
                write!(
                    f,
                    "failed to allocate {size} bytes with alignment {alignment}"
                )
            }
            Self::EncoderError(status) => {
                write!(f, "libxaac encoder failed with status {status:#x}")
            }
            Self::DecoderError(status) => {
                write!(f, "libxaac decoder failed with status {status:#x}")
            }
        }
    }
}

impl std::error::Error for Error {}
