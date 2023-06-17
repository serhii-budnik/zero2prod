use actix_web::{web, HttpResponse};
use anyhow::Context;
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sqlx::{Transaction, Postgres, PgPool};
use uuid::Uuid;

use crate::domain::NewSubscriber;
use crate::domain::SubscriberEmail;
use crate::domain::SubscriberName;
use crate::email_client::EmailClient;
use crate::routes::helpers::{ApiError, error_chain_fmt};
use crate::startup::ApplicationBaseUrl;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;

        Ok(Self { email, name })
    }
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database.",
    skip(form, pool, email_client, base_url),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, ApiError> { 
    let new_subscriber = form.0.try_into().map_err(ApiError::ValidationError)?;
    let mut transaction = pool.begin().await.context("Failed to acquire a Postgres connection from the pool")?;

    let subscriber = find_subscriber_id(&mut transaction, &new_subscriber)
        .await
        .context("Failed to find subscriber by id")?;

    let subscriber_id = match subscriber {
        Some(subscriber) => {
            if subscriber.is_confirmed() {
                return Ok(HttpResponse::UnprocessableEntity().body("Email is already confirmed."));
            };

            subscriber.id
        },
        None => { 
            insert_subscriber(&mut transaction, &new_subscriber)
            .await
            .context("Failed to insert new subscriber")?
        },
    };

    let subscription_token = generate_subscription_token();

    store_token(&mut transaction, subscriber_id, &subscription_token).await.context("Failed to store token")?;
    transaction.commit().await.context("Failed to commit transaction")?;

    send_confirmation_email(
        &email_client,
        new_subscriber,
        &base_url.0,
        &subscription_token,
    )
    .await
    .context("Failed to send confirmation email")?;

    Ok(HttpResponse::Ok().finish())
}

// move this later to the domain
struct Subscriber {
    id: Uuid,
    status: String,
}

impl Subscriber {
    fn is_confirmed(&self) -> bool {
        self.status == "confirmed"
    }
}

#[tracing::instrument(
    skip(new_subscriber, transaction)
)]
async fn find_subscriber_id(
    transaction: &mut Transaction<'_, Postgres>,
    new_subscriber: &NewSubscriber,
)
-> Result<Option<Subscriber>, sqlx::Error> {
    let outcome = sqlx::query_as!(
        Subscriber,
        r#"SELECT id, status FROM subscriptions WHERE email = $1"#,
        new_subscriber.email.as_ref(),
    )
    .fetch_optional(transaction)
    .await?;

    Ok(outcome)
}

#[tracing::instrument(
    skip(new_subscriber, transaction)
)]
async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now()
    )
    .execute(transaction)
    .await?;

    Ok(subscriber_id)
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, new_subscriber)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
    subscriber_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url,
        subscriber_token
    );

    email_client
        .send_email(
            &new_subscriber.email,
            "Welcome!",
            &format!(
                "Welcome to our newsletter! <br />\
                Click <a href=\"{}\">here</a> to confirm your subscription.",
                confirmation_link
            ),
            &format!(
                "Welcome to our newsletter!\n Visit {} to confirm your subscription.",
                confirmation_link,
            ),
        )
        .await?;

    Ok(())
}

pub fn generate_subscription_token() -> String {
    let mut rng = thread_rng();

    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        r#"
        INSERT INTO subscription_tokens (subscription_token, subscriber_id)
            VALUES ($1, $2)
        "#,
        subscription_token,
        subscriber_id,
    )
    .execute(transaction)
    .await
    .map_err(StoreTokenError)?;

    Ok(())
}

pub struct StoreTokenError(sqlx::Error);

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store a subscription token."
        )
    }
}

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
