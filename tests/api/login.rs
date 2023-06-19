use crate::helpers::{assert_is_redirect_to, spawn_app};

#[tokio::test]
async fn an_error_flash_message_is_set_on_failure() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": "unknown_user",
        "password": "password"
    });

    let response = app.post_login(&login_body).await;

    assert_is_redirect_to(&response, "/login")
}
