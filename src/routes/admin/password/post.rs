use crate::routes::helpers::ApiError;
use crate::utils::see_other;

use actix_web::{HttpResponse, web};
use actix_web_flash_messages::FlashMessage;
use secrecy::{Secret, ExposeSecret};

#[derive(serde::Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

pub async fn change_password(
    form: web::Form<FormData>,
) -> Result<HttpResponse, ApiError> {
    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        FlashMessage::error("You entered two different new passwords - the field values must match.").send();

        return Ok(see_other("/admin/password"));
    }

    todo!()
}
