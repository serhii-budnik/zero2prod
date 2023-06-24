use crate::authentication::{validate_credentials, Credentials};
use crate::routes::helpers::ApiError;

use actix_web_flash_messages::FlashMessage;
use actix_web::http::header::LOCATION;
use actix_web::{web, HttpResponse};
use secrecy::Secret;

#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

#[tracing::instrument(
    skip(form, pool),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty),
)]
pub async fn login(
    form: web::Form<FormData>,
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, ApiError> {
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

            Ok(
                HttpResponse::SeeOther()
                    .insert_header((LOCATION, "/admin/dashboard"))
                    .finish()
            )
        },
        Err(e) => {
            match e {
                ApiError::AuthBasicError => {
                    FlashMessage::error("Authentication failed").send();

                    Ok(
                        HttpResponse::SeeOther()
                            .insert_header((LOCATION, "/login"))
                            .finish()
                    )
                },
                _ => Err(ApiError::UnexpectedError(anyhow::anyhow!("Oops! Something went wrong."))),
            }
        }
    }
}

