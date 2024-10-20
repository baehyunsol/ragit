use crate::chunk::Uid;
pub use ragit_api::{Error as ApiError, JsonType, get_type};
use ragit_fs::FileError;

pub type Path = String;

#[derive(Debug)]
pub enum Error {
    JsonTypeError {
        expected: JsonType,
        got: JsonType,
    },
    IndexAlreadyExists(Path),
    InvalidChunkPrefix(u8),
    InvalidConfigKey(String),
    PromptMissing(String),
    IndexNotFound,
    NoSuchChunk { uid: Uid },
    NoSuchFile { file: String },

    // TODO: more enum variants for this type?
    BrokenIndex(String),

    /// see <https://docs.rs/json/latest/json/enum.Error.html>
    JsonError(json::Error),

    /// see <https://docs.rs/serde_json/latest/serde_json/struct.Error.html>
    JsonSerdeError(serde_json::Error),

    FileError(FileError),
    StdIoError(std::io::Error),

    // I'm too lazy to add all the variants of ragit_api::Error
    ApiError(ApiError),
}

impl From<json::Error> for Error {
    fn from(e: json::Error) -> Error {
        Error::JsonError(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::JsonSerdeError(e)
    }
}

impl From<FileError> for Error {
    fn from(e: FileError) -> Error {
        Error::FileError(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::StdIoError(e)
    }
}

impl From<ApiError> for Error {
    fn from(e: ApiError) -> Self {
        match e {
            ApiError::JsonTypeError { expected, got } => Error::JsonTypeError { expected, got },
            ApiError::JsonError(e) => Error::JsonError(e),
            ApiError::JsonSerdeError(e) => Error::JsonSerdeError(e),
            ApiError::FileError(e) => Error::FileError(e),
            e => Error::ApiError(e),
        }
    }
}
