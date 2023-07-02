use crate::routes::helpers::ApiError;
use crate::authentication::middleware::CurrentUserId;

use actix_web::http::header::ContentType;
use actix_web::{web, HttpResponse};
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn admin_dashboard(
    pool: web::Data<PgPool>,
    current_user_id: web::ReqData<CurrentUserId>,
) -> Result<HttpResponse, ApiError> {
    let username = get_username(current_user_id.0, &pool).await?;

    Ok(
        HttpResponse::Ok()
            .content_type(ContentType::html())
            .body(
                format!(
                    r#"
                    <!DOCTYPE html>
                    <html lang="en">
                    <head>
                        <meta http-equiv="content-type" content="text/html; charset=utf-8">
                        <title>Admin dashboard</title>
                    </head>
                    <body>
                        <p>Welcome {username}!</p>
                    </body>
                    </html>
                    "#, 
                    username = username
                )
            )
    )
}

#[tracing::instrument(name = "Get username", skip(pool))]
async fn get_username(
    user_id: Uuid,
    pool: &PgPool,
) -> Result<String, anyhow::Error> {
    let row = sqlx::query!(
        r#"SELECT username FROM users WHERE id = $1"#,
        user_id,
    )
    .fetch_one(pool)
    .await
    .context("Failed to perform a query to retrieve a usernmae")?;

    Ok(row.username)
}