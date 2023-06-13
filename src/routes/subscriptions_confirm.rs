use actix_web::{HttpResponse, web};
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

use crate::routes::helpers::ApiError;

#[derive(serde::Deserialize)]
pub struct Parameters {
    pub subscription_token: String,
}

#[tracing::instrument(
    name = "Confirm a pending subscriber",
    skip(parameters),
)]
pub async fn confirm(
    parameters: web::Query<Parameters>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let subcriber_id_option = get_subscriber_id_from_token(&pool, &parameters.subscription_token)
        .await
        .context("Failed to retrieve a subscriber")?;

    let subscriber_id = subcriber_id_option.ok_or(ApiError::AuthorizationError)?;

    confirm_subscriber(&pool, &subscriber_id)
        .await
        .context("Failed to confirm subscriber")?;
        
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
    name = "Mark subscriber as confirmed",
    skip(pool, subscriber_id),
)]
pub async fn confirm_subscriber(
    pool: &PgPool,
    subscriber_id: &Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
        subscriber_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

#[tracing::instrument(
    name = "Get subscriber_id from token",
    skip(subscription_token, pool),
)]
pub async fn get_subscriber_id_from_token(
    pool: &PgPool,
    subscription_token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT subscriber_id FROM subscription_tokens
            INNER JOIN subscriptions
                ON subscription_tokens.subscriber_id = subscriptions.id
            WHERE subscriptions.status = 'pending_confirmation'
                AND subscription_token = $1
        "#,
        subscription_token,
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|r| r.subscriber_id))
}
