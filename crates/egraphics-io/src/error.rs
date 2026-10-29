use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("path has no file name")]
    NoFileName(),
    #[error("file extension `{0}` is invalid")]
    InvalidFileExtension(String),
}
