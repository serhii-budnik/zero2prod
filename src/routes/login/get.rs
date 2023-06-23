use actix_web::http::header::ContentType;
use actix_web::{HttpRequest, HttpResponse};
use actix_web_flash_messages::{IncomingFlashMessages, Level};
use std::fmt::Write;

pub async fn login_form(
    _request: HttpRequest,
    flash_messages: IncomingFlashMessages
) -> HttpResponse {
    let mut err_html = String::new();

    for message in flash_messages.iter().filter(|m| m.level() == Level::Error) {
        writeln!(err_html, "<p>{}</p>", message.content()).unwrap();
    }

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(include_str!("login.html"), error_html = err_html))
}
