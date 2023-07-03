use crate::authentication::middleware::CurrentUserId;
use crate::authentication::{Credentials, validate_credentials};
use crate::domain::{NewPassword, ResetPassword, CurrentPassword};
use crate::routes::admin::dashboard::get_username;
use crate::routes::helpers::ApiError;
use crate::utils::see_other;

use actix_web::{HttpResponse, web};
use actix_web_flash_messages::FlashMessage;
use secrecy::Secret;
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

impl TryFrom<FormData> for ResetPassword {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let new_password = NewPassword::parse(value.new_password, value.new_password_check)?;
        let current_password = CurrentPassword::parse(value.current_password);

        Ok(Self { new_password, current_password })
    }
}

pub async fn change_password(
    form: web::Form<FormData>,
    current_user_id: web::ReqData<CurrentUserId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let reset_password: ResetPassword = match form.0.try_into() {
        Ok(res) => res,
        Err(e) => {
            FlashMessage::error(e).send();

            return Ok(see_other("/admin/password"));
        },
    };

    let username = get_username(current_user_id.0, &pool).await?;

    let creds = Credentials {
        username,
        password: reset_password.current_password.0,
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
