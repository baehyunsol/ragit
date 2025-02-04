use crate::schema::SchemaParseError;
use ragit_fs::FileError;
use std::string::FromUtf8Error;

#[derive(Debug)]
pub enum Error {
    TeraError(tera::Error),
    RoleMissing,
    InvalidPdl(String),
    InvalidTurnSeparator(String),
    InvalidInlineBlock,
    InvalidImageType(String),
    InvalidRole(String),
    FileError(FileError),
    Utf8Error(FromUtf8Error),
    SchemaParseError(SchemaParseError),

    /// see <https://docs.rs/base64/latest/base64/enum.DecodeError.html>
    Base64DecodeError(base64::DecodeError),
}

impl From<SchemaParseError> for Error {
    fn from(e: SchemaParseError) -> Error {
        match e {
            SchemaParseError::Utf8Error(e) => Error::Utf8Error(e),
            e => Error::SchemaParseError(e),
        }
    }
}

impl From<FromUtf8Error> for Error {
    fn from(e: FromUtf8Error) -> Error {
        Error::Utf8Error(e)
    }
}

impl From<FileError> for Error {
    fn from(e: FileError) -> Error {
        Error::FileError(e)
    }
}

impl From<base64::DecodeError> for Error {
    fn from(e: base64::DecodeError) -> Error {
        Error::Base64DecodeError(e)
    }
}
