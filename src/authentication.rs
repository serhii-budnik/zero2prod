use crate::routes::helpers::ApiError;
use crate::telemetry::spawn_blocking_with_tracing;

use anyhow::Context;
use argon2::{PasswordHash, PasswordVerifier, Argon2};
use secrecy::{Secret, ExposeSecret};
use sqlx::PgPool;
use uuid::Uuid;

pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}

pub async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<Uuid, ApiError> {
    let (user_id, expected_password_hash) = match get_stored_credentials(&credentials.username, pool).await? {
        Some(row) => row,
        None => (None, Secret::new(
            "$argon2id$v=19$m=15000,t=2,p=1$\
            HdFWisuoULgZIDF0OKW7EA$IfkXo59yJ7KLk5BqakAs4ecioYMfY14xAznmBPanMns".to_string()
        )),
    };

    spawn_blocking_with_tracing(move || {
        verify_password_hash(expected_password_hash, credentials.password)
    })
    .await
    .context("Failed to spawn blocking task.")
    .map_err(ApiError::UnexpectedError)??;

    // This is only set to Some if we found credentials in the store
    // So, even if the default password ends up matching (somehow)
    // the provided password, we never authenticate a non-existing user.
    // It is needed to be `side-channel attack` resistant.
    user_id.ok_or(ApiError::AuthBasicError)
}


#[tracing::instrument(name = "Get stored credentials", skip(username, pool))]
async fn get_stored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<(Option<Uuid>, Secret<String>)>, anyhow::Error> {
    let row = sqlx::query!(
        r#"SELECT id, password_hash FROM users WHERE username = $1"#,
        username,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to validate auth credentials.")?
    .map(|row| (Some(row.id), Secret::new(row.password_hash)));

    Ok(row)
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash, password_candidate),
)]
fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: Secret<String>,
) -> Result<(), ApiError> {
    let expected_password_hash = PasswordHash::new(
        expected_password_hash.expose_secret()
    )
    .context("Failed to parse hash in PHC string format.")
    .map_err(ApiError::UnexpectedError)?;

    let res = Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        );
    
    res.or(Err(ApiError::AuthBasicError))
}
