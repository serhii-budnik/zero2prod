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

                    <lable>
                        Title
                        <input type="text" name="title">
                    </lable>

                    <lable>
                        Text content
                        <input type="text" name="text_content">
                    </lable>

                    <lable>
                        HTML content
                        <input type="text" name="html_content">
                    </lable>

                    <p>Retry settings (Optional)</p>
                    <label>
                        N-retries
                        <input type="number" name="n_retries" placeholder="20">
                    </label>

                    <label>
                        Retry after in seconds
                        <input type="number" name="execute_after_in_secs">
                    </label>

                    <button type="submit">Send</button>
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
