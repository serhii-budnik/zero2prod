use crate::helpers::{assert_is_redirect_to, spawn_app};

#[tokio::test]
async fn an_error_flash_message_is_set_on_failure() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": "unknown_user",
        "password": "password"
    });

    let response = app.post_login(&login_body).await;
    
    assert_is_redirect_to(&response, "/login");

    let html_page = app.get_login_html().await;

    assert!(html_page.contains("Authentication failed"));

    let html_page = app.get_login_html().await;

    assert!(!html_page.contains("Authentication failed"));
}

#[tokio::test]
async fn redirect_to_admin_dashboard_after_login_success() {
    let app = spawn_app().await;
    let (username, password) = app.add_test_user().await;

    let login_body = serde_json::json!({
        "username": &username,
        "password": password,
    });

    let response = app.post_login(&login_body).await;

    assert_is_redirect_to(&response, "/admin/dashboard");

    let html_page = app.get_admin_dashboard_html().await;
    assert!(html_page.contains(&format!("Welcome {}", username)));
}

// #[tokio::test]
// async fn non_existing_user_with_correct_password() {
//     let app = spawn_app().await;
//
//     let username = Uuid::new_v4().to_string();
//     let password = "944d4326-5508-4837-8f01-9115c5224c77".to_string();
//
//     let response = reqwest::Client::new()
//         .post(&format!("{}/newsletters", &app.address))
//         .basic_auth(username, Some(password))
//         .json(&serde_json::json!({
//             "title": "Newsletter title",
//             "content": { "text": "Newsletter body as plain text", "html": "<p>Newsletter body as HTML</p>" },
//         }))
//         .send()
//         .await
//         .expect("Failed to make request.");
//
//     assert_eq!(401, response.status().as_u16());
//     assert_eq!(
//         r#"Basic realm="publish""#,
//         response.headers()["WWW-Authenticate"]
//     );
// }
//
//
// async fn invalid_password_is_rejected() {
//     let app = spawn_app().await;
//
//     let (user_username, user_password) = app.add_test_user().await;
//     let rand_password = Uuid::new_v4().to_string();
//
//     assert_ne!(user_password, rand_password);
//
//     let response = reqwest::Client::new()
//         .post(format!("{}/newsletters", &app.address))
//         .basic_auth(user_username, Some(rand_password))
//         .json(&serde_json::json!({
//             "title": "Newsletter title",
//             "content": { "text": "plain text", "html": "<p>HTML</p>" },
//         }))
//         .send()
//         .await
//         .expect("Failed to execute request.");
//
//     assert_eq!(401, response.status().as_u16());
//     assert_eq!(
//         r#"Basic realm="publish""#,
//         response.headers()["WWW-Authenticate"]
//     );
// }
