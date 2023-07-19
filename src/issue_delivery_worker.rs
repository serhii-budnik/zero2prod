use crate::{
    configuration::Settings,
    domain::SubscriberEmail,
    email_client::EmailClient,
    startup::get_connection_pool,
};

use sqlx::{postgres::PgArguments, PgPool, Postgres, Transaction};
use std::time::Duration;
use uuid::Uuid;

pub enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}

#[derive(Debug)]
pub enum ErrorType {
    SoftError(JobErrorWithRetryConf),
    HardError(JobError),
    UnexpectedError(anyhow::Error),
}

impl ErrorType { 
    fn create_soft_error(
        error: anyhow::Error,
        newsletter_issue_id: Uuid,
        subscriber_email: String,
        n_retries: i16,
        execute_after_in_secs: Option<i32>,
    ) -> Self {
        ErrorType::SoftError(
            JobErrorWithRetryConf {
                job_error: JobError::new(error, newsletter_issue_id, subscriber_email),
                retry_conf: RetryConf { n_retries, execute_after_in_secs }
            }
        )
    }

    fn create_hard_error(error: anyhow::Error, newsletter_issue_id: Uuid, subscriber_email: String) -> Self {
        ErrorType::HardError(JobError::new(error, newsletter_issue_id, subscriber_email))
    }
}

impl JobError { 
    fn new(error: anyhow::Error, newsletter_issue_id: Uuid, subscriber_email: String) -> Self {
        Self { 
            error,
            issue_job_info: IssueJobInfo { newsletter_issue_id, subscriber_email },
        }
    }
}

impl std::fmt::Display for ErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self { 
            ErrorType::SoftError(job_error_with_retry) => job_error_with_retry.job_error.error.to_string(),
            ErrorType::HardError(job_error) => job_error.error.to_string(),
            ErrorType::UnexpectedError(e) => e.to_string(),
        };

        write!(f, "{}", &message)
    }
}

#[derive(Debug)]
pub struct JobError {
    error: anyhow::Error,
    issue_job_info: IssueJobInfo,
}

#[derive(Debug)]
pub struct JobErrorWithRetryConf {
    job_error: JobError,
    retry_conf: RetryConf,
}

#[derive(Debug)]
struct IssueJobInfo {
    newsletter_issue_id: Uuid,
    subscriber_email: String,
}

#[derive(Debug)]
struct RetryConf {
    n_retries: i16,
    execute_after_in_secs: Option<i32>,
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
            Ok(ExecutionOutcome::EmptyQueue) => tokio::time::sleep(Duration::from_secs(10)).await,
            Ok(ExecutionOutcome::TaskCompleted) => {},
            Err(error_type) => handle_worker_error(&pool, error_type).await?,
        }
    }
}

#[tracing::instrument(skip(pool))]
pub async fn handle_worker_error(pool: &PgPool, error_type: ErrorType) -> Result<(), anyhow::Error> {
    match error_type {
        ErrorType::SoftError(job_error_with_retry) => {
            let IssueJobInfo { newsletter_issue_id, subscriber_email } = job_error_with_retry.job_error.issue_job_info;
            let n_retries = job_error_with_retry.retry_conf.n_retries;

            if n_retries <= 1 {
                delete_task_query(newsletter_issue_id, &subscriber_email)
                    .execute(pool)
                    .await?;
            } else {
                let execute_after_in_secs = job_error_with_retry.retry_conf.execute_after_in_secs;
                let execute_after_in_secs: i64 = execute_after_in_secs.unwrap_or(30) as i64;

                sqlx::query!(
                    r#"
                    UPDATE issue_delivery_queue
                    SET n_retries = $1,
                        execute_after = $2
                    WHERE newsletter_issue_id = $3 AND subscriber_email = $4;
                    "#,
                    n_retries - 1,
                    chrono::Utc::now() + chrono::Duration::seconds(execute_after_in_secs),
                    newsletter_issue_id,
                    subscriber_email,
                )
                .execute(pool)
                .await?;

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        },
        ErrorType::HardError(job_error) => {
            let IssueJobInfo { newsletter_issue_id, subscriber_email } = job_error.issue_job_info;

            delete_task_query(newsletter_issue_id, &subscriber_email)
                .execute(pool)
                .await?;
        },
        ErrorType::UnexpectedError(_) => tokio::time::sleep(Duration::from_secs(1)).await,
    };

    Ok(())
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
) -> Result<ExecutionOutcome, ErrorType> {
    let task = dequeue_task(pool).await.map_err(|e| ErrorType::UnexpectedError(e))?;
    let (transaction, newsletter_issue_id, email, n_retries, execute_after_in_secs) = match task {
        Some(res) => res,
        None => return Ok(ExecutionOutcome::EmptyQueue),
    };

    let NewsletterIssue { title, text_content, html_content } = get_newsletter_issue(&pool, newsletter_issue_id)
        .await.map_err(|e| ErrorType::UnexpectedError(e))?; 
    let email = SubscriberEmail::parse(email).map_err(|e| { 
        ErrorType::create_hard_error(anyhow::anyhow!(e.0), newsletter_issue_id, e.1)
    })?;

    email_client
        .send_email(&email, &title, &html_content, &text_content)
        .await
        .map_err(|_| { 
            ErrorType::create_soft_error(
                anyhow::anyhow!(format!("Failed to send newsletter to a confitmed subscriber {:?}. Skipping.", &email)),
                newsletter_issue_id,
                email.as_ref().into(),
                n_retries,
                execute_after_in_secs,
            )
        })?;

    delete_task(transaction, newsletter_issue_id, email.as_ref()).await.map_err(|e| ErrorType::UnexpectedError(e))?;

    Ok(ExecutionOutcome::TaskCompleted)
}

async fn dequeue_task(
    pool: &PgPool,
) -> Result<Option<(PgTransaction, Uuid, String, i16, Option<i32>)>, anyhow::Error> {
    let mut transaction = pool.begin().await?;

    let record = sqlx::query!(
        r#"
        SELECT newsletter_issue_id, subscriber_email, n_retries, execute_after_in_secs
        FROM issue_delivery_queue
        WHERE execute_after < now() OR execute_after IS NULL
        FOR UPDATE
        SKIP LOCKED
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut transaction)
    .await?;

    match record {
        Some(rec) => { 
            Ok(Some((transaction,
                     rec.newsletter_issue_id,
                     rec.subscriber_email,
                     rec.n_retries,
                     rec.execute_after_in_secs)))
        },
        None => Ok(None),
    }
}

fn delete_task_query(
    issue_id: Uuid,
    email: &str,
) -> sqlx::query::Query<'_, Postgres, PgArguments> {
    sqlx::query!(
        r#"
        DELETE FROM issue_delivery_queue
        WHERE newsletter_issue_id = $1 
            AND subscriber_email = $2
        "#,
        issue_id,
        email
    )
}

#[tracing::instrument(skip_all)]
async fn delete_task(
    mut transaction: PgTransaction,
    issue_id: Uuid,
    email: &str,
) -> Result<(), anyhow::Error> {
    delete_task_query(issue_id, email)
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

