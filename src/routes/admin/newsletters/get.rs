use actix_web::{http::header::ContentType, HttpResponse};
use actix_web_flash_messages::IncomingFlashMessages;
use std::fmt::Write;

use crate::routes::helpers::ApiError;

pub async fn submit_newsletter_form(
    flash_messages: IncomingFlashMessages
) -> Result<HttpResponse, ApiError> {
    let mut msg = String::new();
    for message in flash_messages.iter() {
        writeln!(msg, "<p><i>{}</i></p>", message.content()).unwrap()
    }

    let body = 
        format!(
            r#"
            <!DOCTYPE html>
            <html lang="en">
            <head>
                <meta http-equiv="content-type" content="text/html; charset=utf-8">
                <title>Change Password</title>
            </head>
            <body>
                {flash_msg}
                <p>submit newsletter form: under constructions</p>
                <a href="/admin/dashboard">&lt;- Back</a>
            </body>
            </html>
            "#, 
            flash_msg = msg
        );

    Ok(
        HttpResponse::Ok()
            .content_type(ContentType::html())
            .body(body)
    )
}
