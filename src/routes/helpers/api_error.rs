use actix_web::http::StatusCode;
use actix_web::ResponseError;

use crate::routes::helpers::error_chain_fmt;

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::ValidationError(_) => StatusCode::BAD_REQUEST,
            ApiError::AuthorizationError => StatusCode::UNAUTHORIZED,
            ApiError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    ValidationError(String),
    #[error("Unauthorized")]
    AuthorizationError,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
