use crate::authentication::middleware::CurrentUserId;
use crate::idempotency::{IdempotencyKey, save_response, try_processing, NextAction};
use crate::routes::helpers::ApiError;
use crate::utils::see_other;

use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::PgPool;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(serde::Deserialize, Debug)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    idempotency_key: String,
    n_retries: Option<String>,
    execute_after_in_secs: Option<String>,
}

#[tracing::instrument(
    name = "Publish a newsletter issue"
    skip(form, pool),
    fields(username=tracing::field::Empty, user_is=tracing::field::Empty)
)]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    user_id: web::ReqData<CurrentUserId>,
) -> Result<HttpResponse, ApiError> {
    // TODO: we can create validation function for incoming data
    let FormData { title, text_content, html_content, idempotency_key, n_retries, execute_after_in_secs } = form.0;

    let n_retries = n_retries.and_then(|s| s.parse::<u8>().ok());
    let execute_after_in_secs = execute_after_in_secs.and_then(|s| s.parse::<u32>().ok());

    let idempotency_key: Result<IdempotencyKey, anyhow::Error> = idempotency_key.try_into();
    let idempotency_key = idempotency_key.map_err(|e| ApiError::ValidationError(e.to_string()))?;

    let mut transaction = match try_processing(&pool, &idempotency_key, user_id.0)
        .await
        .map_err(|e| ApiError::ValidationError(e.to_string()))? {

        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved_response) => {
            success_message().send();
            println!("before returning saved response");
            return Ok(saved_response);
        }
    };

    let issue_id = insert_newsletter_issue(&mut transaction, &title, &text_content, &html_content)
        .await
        .context("Failed to store newsletter issue details")
        .map_err(ApiError::UnexpectedError)?;

    enqueue_delivery_tasks(&mut transaction, issue_id, n_retries, execute_after_in_secs)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(ApiError::UnexpectedError)?;
        
    success_message().send();

    let response = see_other("/admin/newsletters");
    let response = save_response(transaction, &idempotency_key, user_id.0, response).await?;
    Ok(response)
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO newsletter_issues(
            id,
            title,
            text_content,
            html_content,
            published_at
        )
        VALUES ($1, $2, $3, $4, now())
        "#,
        id,
        title,
        text_content,
        html_content
    )
    .execute(transaction)
    .await?;

    Ok(id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, Postgres>,
    newsletter_issue_id: Uuid,
    n_retries: Option<u8>,
    execute_after_in_secs: Option<u32>
) -> Result<(), sqlx::Error> {
    let n_retries: i16 = n_retries.and_then(|num| Some(num as i16)).unwrap_or(20);
    let execute_after_in_secs: Option<i32> = execute_after_in_secs.and_then(|num| Some(num as i32));

    sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (
            newsletter_issue_id,
            subscriber_email,
            n_retries,
            execute_after_in_secs
        )
        SELECT $1, email, $2, $3
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_id,
        n_retries,
        execute_after_in_secs,
    )
    .execute(transaction)
    .await?;

    Ok(())
}

fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issue has been accepted - emails will go out shortly.")
}
