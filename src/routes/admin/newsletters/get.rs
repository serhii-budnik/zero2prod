use actix_web::{http::header::ContentType, HttpResponse};

use crate::routes::helpers::ApiError;

pub async fn submit_newsletter_form() -> Result<HttpResponse, ApiError> {
    let body = 
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta http-equiv="content-type" content="text/html; charset=utf-8">
            <title>Change Password</title>
        </head>
        <body>
            <p>submit newsletter form: under constructions</p>
            <a href="/admin/dashboard">&lt;- Back</a>
        </body>
        </html>
        "#;

    Ok(
        HttpResponse::Ok()
            .content_type(ContentType::html())
            .body(body)
    )
}
