use secrecy::Secret;

#[derive(Debug)]
pub struct CurrentPassword(pub Secret<String>);

impl CurrentPassword { 
    pub fn parse(current_password: Secret<String>) -> Self {
        Self(current_password)
    }
}
