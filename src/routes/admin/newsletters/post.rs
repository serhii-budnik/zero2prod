use crate::authentication::middleware::CurrentUserId;
use crate::email_client::EmailClient;
use crate::idempotency::{IdempotencyKey, save_response, try_processing, NextAction};
use crate::routes::helpers::ApiError;
use crate::{domain::SubscriberEmail, utils::see_other};

use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    idempotency_key: String,
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(
    name = "Publish a newsletter issue"
    skip(form, pool, email_client),
    fields(username=tracing::field::Empty, user_is=tracing::field::Empty)
)]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    user_id: web::ReqData<CurrentUserId>,
) -> Result<HttpResponse, ApiError> {
    let FormData { title, text_content, html_content, idempotency_key } = form.0;

    let idempotency_key: Result<IdempotencyKey, anyhow::Error> = idempotency_key.try_into();
    let idempotency_key = idempotency_key.map_err(|e| ApiError::ValidationError(e.to_string()))?;

    let transaction = match try_processing(&pool, &idempotency_key, user_id.0).await.map_err(|e| ApiError::ValidationError(e.to_string()))? {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved_response) => {
            success_message().send();
            println!("before returning saved response");
            return Ok(saved_response);
        }
    };

    let subscribers = get_confirmed_subscribers(&pool).await?;

    // TODO: don't like this approach. fix it later
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(&subscriber.email, &title, &html_content, &text_content)
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

    success_message().send();

    let response = see_other("/admin/newsletters");
    let response = save_response(transaction, &idempotency_key, user_id.0, response).await?;
    Ok(response)
}

fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issue has been accepted - emails will go out shortly.")
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
