#[derive(Debug)]
pub enum SkkError {
    Io(std::io::Error),
    Parse(std::num::ParseIntError),
    Reqwest(reqwest::Error),
    String(std::string::FromUtf8Error),
    Json(json::Error),
    Bincode(bincode::Error),
    JisyoRead,
    BrokenCache,
    CacheOpen,
    BrokenDictionary,
    CommandLine,
    Encoding,
    Request,
}

impl From<std::io::Error> for SkkError {
    fn from(err: std::io::Error) -> SkkError {
        SkkError::Io(err)
    }
}

impl From<std::num::ParseIntError> for SkkError {
    fn from(err: std::num::ParseIntError) -> SkkError {
        SkkError::Parse(err)
    }
}

impl From<reqwest::Error> for SkkError {
    fn from(err: reqwest::Error) -> SkkError {
        SkkError::Reqwest(err)
    }
}

impl From<std::string::FromUtf8Error> for SkkError {
    fn from(err: std::string::FromUtf8Error) -> SkkError {
        SkkError::String(err)
    }
}

impl From<json::Error> for SkkError {
    fn from(err: json::Error) -> SkkError {
        SkkError::Json(err)
    }
}

impl From<bincode::Error> for SkkError {
    fn from(err: bincode::Error) -> SkkError {
        SkkError::Bincode(err)
    }
}

impl std::fmt::Display for SkkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            SkkError::Io(ref err) => write!(f, "Io error: {}", err),
            SkkError::Parse(ref err) => write!(f, "Parse error: {}", err),
            SkkError::Reqwest(ref err) => write!(f, "Reqwest error: {}", err),
            SkkError::String(ref err) => write!(f, "String error: {}", err),
            SkkError::Json(ref err) => write!(f, "Json error: {}", err),
            SkkError::Bincode(ref err) => write!(f, "Bincode error: {}", err),
            SkkError::JisyoRead => write!(f, "JisyoRead error"),
            SkkError::BrokenCache => write!(f, "BrokenCache error"),
            SkkError::CacheOpen => write!(f, "CacheOpen error"),
            SkkError::BrokenDictionary => write!(f, "BrokenDictionary error"),
            SkkError::CommandLine => write!(f, "CommandLine error"),
            SkkError::Encoding => write!(f, "Encoding error"),
            SkkError::Request => write!(f, "Request error"),
        }
    }
}

impl std::error::Error for SkkError {
    fn description(&self) -> &str {
        match *self {
            SkkError::Io(ref err) => err.description(),
            SkkError::Parse(ref err) => err.description(),
            SkkError::Reqwest(ref err) => err.description(),
            SkkError::String(ref err) => err.description(),
            SkkError::Json(ref err) => err.description(),
            SkkError::Bincode(ref err) => err.description(),
            SkkError::JisyoRead => "JisyoRead error",
            SkkError::BrokenCache => "BrokenCache error",
            SkkError::CacheOpen => "CacheOpen error",
            SkkError::BrokenDictionary => "BrokenDictionary error",
            SkkError::CommandLine => "CommandLine error",
            SkkError::Encoding => "Encoding error",
            SkkError::Request => "Request error",
        }
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        match *self {
            SkkError::Io(ref err) => Some(err),
            SkkError::Parse(ref err) => Some(err),
            SkkError::Reqwest(ref err) => Some(err),
            SkkError::String(ref err) => Some(err),
            SkkError::Json(ref err) => Some(err),
            SkkError::Bincode(ref err) => Some(err),
            SkkError::JisyoRead => Some(&SkkError::JisyoRead),
            SkkError::BrokenCache => Some(&SkkError::BrokenCache),
            SkkError::CacheOpen => Some(&SkkError::CacheOpen),
            SkkError::BrokenDictionary => Some(&SkkError::BrokenDictionary),
            SkkError::CommandLine => Some(&SkkError::CommandLine),
            SkkError::Encoding => Some(&SkkError::Encoding),
            SkkError::Request => Some(&SkkError::Request),
        }
    }
}
