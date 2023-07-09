use crate::{
    configuration::Settings,
    domain::SubscriberEmail,
    email_client::EmailClient,
    startup::get_connection_pool,
};

use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use std::time::Duration;
use uuid::Uuid;

pub enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}

type PgTransaction = Transaction<'static, Postgres>;

pub async fn run_worker_until_stopped(
    configuration: Settings
) -> Result<(), anyhow::Error> {
    let connection_pool = get_connection_pool(&configuration.database);
    let email_client = configuration.email_client.client();

    worker_loop(connection_pool, email_client).await
}

async fn worker_loop(
    pool: PgPool,
    email_client: EmailClient,
) -> Result<(), anyhow::Error> {
    loop {
        match try_execute_task(&pool, &email_client).await {
            Ok(ExecutionOutcome::EmptyQueue) => tokio::time::sleep(Duration::from_secs(30)).await,
            Ok(ExecutionOutcome::TaskCompleted) => {},
            Err(_) => tokio::time::sleep(Duration::from_secs(1)).await,
        }
    }
}

#[tracing::instrument(
    skip_all,
    fields(
        newsletter_issue_id=tracing::field::Empty,
        subscriber_email=tracing::field::Empty,
    ),
    err
)]
pub async fn try_execute_task(
    pool: &PgPool,
    email_client: &EmailClient,
) -> Result<ExecutionOutcome, anyhow::Error> {
    let (transaction, newsletter_issue_id, email) = match dequeue_task(pool).await? {
        Some(res) => res,
        None => return Ok(ExecutionOutcome::EmptyQueue),
    };

    let NewsletterIssue { title, text_content, html_content } = get_newsletter_issue(&pool, newsletter_issue_id)
        .await?; 
    let email = SubscriberEmail::parse(email).map_err(|e| anyhow::anyhow!(e))?;

    email_client
        .send_email(&email, &title, &html_content, &text_content)
        .await
        .with_context(|| format!("Failed to send newsletter to a confitmed subscriber {:?}. Skipping.", email))?;

    delete_task(transaction, newsletter_issue_id, email.as_ref()).await?;

    Ok(ExecutionOutcome::TaskCompleted)
}

async fn dequeue_task(
    pool: &PgPool,
) -> Result<Option<(PgTransaction, Uuid, String)>, anyhow::Error> {
    let mut transaction = pool.begin().await?;

    let record = sqlx::query!(
        r#"
        SELECT newsletter_issue_id, subscriber_email
        FROM issue_delivery_queue
        FOR UPDATE
        SKIP LOCKED
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut transaction)
    .await?;

    match record {
        Some(rec) => Ok(Some((transaction, rec.newsletter_issue_id, rec.subscriber_email))),
        None => Ok(None),
    }
}

#[tracing::instrument(skip_all)]
async fn delete_task(
    mut transaction: PgTransaction,
    issue_id: Uuid,
    email: &str,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
        DELETE FROM issue_delivery_queue
        WHERE newsletter_issue_id = $1 
            AND subscriber_email = $2
        "#,
        issue_id,
        email
    )
    .execute(&mut transaction)
    .await?;

    transaction.commit().await?;
    Ok(())
}

struct NewsletterIssue {
    title: String,
    text_content: String,
    html_content: String,
}

async fn get_newsletter_issue(pool: &PgPool, newsletter_issue_id: Uuid) -> Result<NewsletterIssue, anyhow::Error> {
    let record = sqlx::query_as!(
        NewsletterIssue,
        r#"
        SELECT title,
            text_content,
            html_content
        FROM newsletter_issues
        WHERE id = $1
        "#,
        newsletter_issue_id
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

