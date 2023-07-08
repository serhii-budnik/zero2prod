use actix_web::{http::header::ContentType, HttpResponse};
use actix_web_flash_messages::IncomingFlashMessages;
use std::fmt::Write;
use uuid::Uuid;

use crate::routes::helpers::ApiError;

pub async fn submit_newsletter_form(
    flash_messages: IncomingFlashMessages
) -> Result<HttpResponse, ApiError> {
    let mut flash_msg = String::new();
    for message in flash_messages.iter() {
        writeln!(flash_msg, "<p><i>{}</i></p>", message.content()).unwrap()
    }

    let idempotency_key = Uuid::new_v4();

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

                <form action="/admin/newsletters" method="post">
                    <input hidden type="text" name="idempotency_key" value={idempotency_key}>
                    <lable>Title</lable>
                    <input type="text" name="title">
                    <lable>Text</lable>
                    <input type="text" name="text_content">
                    <lable>HTML</lable>
                    <input type="text" name="html_content">
                    <input type="submit" name="send">
                </form>

                <a href="/admin/dashboard">&lt;- Back</a>
            </body>
            </html>
            "#
        );

    Ok(
        HttpResponse::Ok()
            .content_type(ContentType::html())
            .body(body)
    )
}
