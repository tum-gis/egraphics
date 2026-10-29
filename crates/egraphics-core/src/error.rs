use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("No triangle")]
    NoElements,

    #[error("the data for key `{0}` is not available")]
    InvalidLodName(String),
}
