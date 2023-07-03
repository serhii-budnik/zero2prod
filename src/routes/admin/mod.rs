mod dashboard;
mod logout;
mod newsletters;
mod password;

pub use dashboard::{admin_dashboard, get_username};
pub use logout::log_out;
pub use newsletters::*;
pub use password::*;
