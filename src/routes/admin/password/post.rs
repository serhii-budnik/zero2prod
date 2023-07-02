use crate::authentication::middleware::CurrentUserId;
use crate::authentication::{Credentials, validate_credentials};
use crate::routes::admin::dashboard::get_username;
use crate::routes::helpers::ApiError;
use crate::utils::see_other;

use actix_web::{HttpResponse, web};
use actix_web_flash_messages::FlashMessage;
use secrecy::{Secret, ExposeSecret};
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

pub async fn change_password(
    form: web::Form<FormData>,
    current_user_id: web::ReqData<CurrentUserId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        FlashMessage::error("You entered two different new passwords - the field values must match.").send();

        return Ok(see_other("/admin/password"));
    }

    let username = get_username(current_user_id.0, &pool).await?;

    let creds = Credentials {
        username,
        password: form.0.current_password,
    };

    if let Err(e) = validate_credentials(creds, &pool).await {
        return match e {
            ApiError::AuthBasicError => {
                FlashMessage::error("The current password is incorrect.").send();
                Ok(see_other("/admin/password"))
            },
            _ => Err(ApiError::UnexpectedError(anyhow::anyhow!("Oops! Something went wrong."))),
        }
    }

    todo!()
}
