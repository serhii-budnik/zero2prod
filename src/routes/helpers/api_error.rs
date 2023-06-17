use actix_web::http::header::HeaderValue;
use actix_web::http::StatusCode;
use actix_web::{ResponseError, HttpResponse};
use reqwest::header;

use crate::routes::helpers::error_chain_fmt;

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::ValidationError(_) => StatusCode::BAD_REQUEST,
            ApiError::AuthorizationError | 
            ApiError::AuthBasicError => StatusCode::UNAUTHORIZED,
            ApiError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        match self {
            Self::AuthBasicError => {
                let mut response = HttpResponse::new(self.status_code());
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#)
                    .unwrap();

                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_value);

                response
            },
            _ => {
                HttpResponse::new(self.status_code())
            }
        }
    }
}

#[derive(thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    ValidationError(String),
    #[error("Unauthorized")]
    AuthorizationError, // in book we add here #[source] anyhow::Error check later if we really need it
    #[error("Unauthorized")]
    AuthBasicError,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
