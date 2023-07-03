use secrecy::{ExposeSecret, Secret};
use unicode_segmentation::UnicodeSegmentation;

pub struct NewPassword(Secret<String>);

impl NewPassword { 
    pub fn parse(new_password: Secret<String>, new_password_check: Secret<String>) -> Result<Self, String> {
        if new_password.expose_secret() != new_password_check.expose_secret() {
            return Err("You entered two different new passwords - the field values must match.".into());
        }

        let pass_length = new_password.expose_secret().graphemes(true).count();
        let is_too_long = pass_length > 128;
        let is_too_short = pass_length < 12;

        if is_too_long || is_too_short {
            return Err("The password is not valid. The length of password should be at least 12 and at most 128".into());
        }

        Ok(Self(new_password))
    }
}
