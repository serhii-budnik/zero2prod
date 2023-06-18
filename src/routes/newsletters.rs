use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::routes::helpers::ApiError;
use crate::telemetry::spawn_blocking_with_tracing;

use actix_web::http::header::HeaderMap;
use actix_web::{web, HttpResponse, HttpRequest};
use anyhow::Context;
use argon2::{PasswordHash, PasswordVerifier, Argon2};
use secrecy::{Secret, ExposeSecret};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    text: String,
    html: String,
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(
    name = "Publish a newsletter issue"
    skip(body, pool, email_client, request),
    fields(username=tracing::field::Empty, user_is=tracing::field::Empty)
)]
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let credentials = basic_authentication(request.headers())
        .or(Err(ApiError::AuthBasicError))?;

    tracing::Span::current().record(
        "username",
        &tracing::field::display(&credentials.username),
    );

    let user_id = validate_credentials(credentials, &pool).await?;

    tracing::Span::current().record(
        "user_id",
        &tracing::field::display(&user_id),
    );

    let subscribers = get_confirmed_subscribers(&pool).await?;

    // TODO: don't like this approach. fix it later
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &body.title,
                        &body.content.html,
                        &body.content.text,
                    )
                    .await
                    .with_context(|| format!("Failed to send newsletter issue to {:?}", subscriber.email))?;
            }
            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    "Skipping a confirmed subscriber. \
                    Their stored contact details are invalid",
                );
            }
        }
    };

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT email FROM subscriptions WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await?;

    let confirmed_subscribers = rows
        .into_iter()
        .map(|r| match SubscriberEmail::parse(r.email) {
            Ok(email) => Ok(ConfirmedSubscriber { email }),
            Err(error) => Err(anyhow::anyhow!(error)),
        })
        .collect();


    Ok(confirmed_subscribers)
}

struct Credentials {
    username: String,
    password: Secret<String>,
}

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header was missing")?
        .to_str()
        .context("The 'Authorization' header was not a valid UTF8 string.")?;

    let base64encoded_segment = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic'.")?;

    let decoded_bytes = base64::decode_config(base64encoded_segment, base64::STANDARD)
        .context("Failed to base64-decode 'Basic' credentials.")?;

    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The authorization value was not valid base64.")?;

    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A username must be provided in 'Basic' auth."))?
        .to_string();

    let password = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A password must be provided in 'Basic' auth."))?
        .to_string();

    Ok(Credentials {
        username,
        password: Secret::new(password),
    })
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

async fn validate_credentials(
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
