use crate::authentication::{validate_credentials, Credentials};
use crate::routes::helpers::ApiError;
use crate::session_state::TypedSession;
use crate::utils::see_other;

use actix_web::{web, HttpResponse, error::InternalError};
use actix_web_flash_messages::FlashMessage;
use secrecy::Secret;

#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

#[tracing::instrument(
    skip(form, pool, session),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty),
)]
pub async fn login(
    form: web::Form<FormData>,
    pool: web::Data<sqlx::PgPool>,
    session: TypedSession,
) -> Result<HttpResponse, InternalError<ApiError>> {
    let creds = Credentials {
        username: form.0.username,
        password: form.0.password,
    };
    
    tracing::Span::current().record(
        "username",
        &tracing::field::display(&creds.username),
    );

    match validate_credentials(creds, &pool).await { 
        Ok(user_id) => {
            tracing::Span::current().record(
                "user_id",
                &tracing::field::display(&user_id),
            );

            session.renew();
            session
                .insert_user_id(user_id)
                .map_err(|e| login_redirect(ApiError::UnexpectedError(e.into())))?;


            Ok(see_other("/admin/dashboard"))
        },
        Err(e) => {
            let e = match e {
                ApiError::AuthBasicError => ApiError::AuthorizationError,
                _ => ApiError::UnexpectedError(anyhow::anyhow!("Oops! Something went wrong.")),
            };

            Err(login_redirect(e))
        }
    }
}

fn login_redirect(e: ApiError) -> InternalError<ApiError> { 
    FlashMessage::error(e.to_string()).send();
    let response = see_other("/login");

    InternalError::from_response(e, response)
}
