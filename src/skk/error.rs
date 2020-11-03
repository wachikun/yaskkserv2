use thiserror::Error;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Error)]
pub enum SkkError {
    #[error("{}", .0)]
    Io(#[from] std::io::Error),
    #[error("{}", .0)]
    Parse(#[from] std::num::ParseIntError),
    #[error("{}", .0)]
    Reqwest(#[from] reqwest::Error),
    #[error("{}", .0)]
    String(#[from] std::string::FromUtf8Error),
    #[error("{}", .0)]
    Json(#[from] json::Error),
    #[error("{}", .0)]
    Bincode(#[from] bincode::Error),
    #[error("JisyoRead error")]
    JisyoRead,
    #[error("BrokenCache error")]
    BrokenCache,
    #[error("CacheOpen error")]
    CacheOpen,
    #[error("BrokenDictionary error")]
    BrokenDictionary,
    #[error("CommandLine error")]
    CommandLine,
    #[error("Encoding error")]
    Encoding,
    #[error("Request error")]
    Request,
}
