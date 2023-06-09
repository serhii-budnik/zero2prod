use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

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
) -> HttpResponse {
    let subcriber_id_option = match get_subscriber_id_from_token(&pool, &parameters.subscription_token).await {
        Ok(option) => option,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let subscriber_id = match subcriber_id_option {
        Some(subscriber_id) => subscriber_id,
        None => return HttpResponse::Unauthorized().finish(),
    };

    if confirm_subscriber(&pool, &subscriber_id).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    };
        
    HttpResponse::Ok().finish()
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
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

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
        r#"SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1"#,
        subscription_token,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

    Ok(result.map(|r| r.subscriber_id))
}
