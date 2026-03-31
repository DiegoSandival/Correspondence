use thiserror::Error;

#[derive(Debug, Error)]
pub enum CellError {
    #[error("bad request")]
    BadRequest,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("internal error")]
    Internal,
}

impl CellError {
    pub fn code(&self) -> u16 {
        match self {
            Self::BadRequest => 400,
            Self::Unauthorized => 401,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::Internal => 500,
        }
    }
}

pub type CellResult<T> = Result<T, CellError>;
