use crate::domain::{NewPassword, CurrentPassword};

pub struct ResetPassword {
    pub current_password: CurrentPassword,
    pub new_password: NewPassword,
}
