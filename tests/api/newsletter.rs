use crate::helpers::{spawn_app, TestApp, ConfirmationLinks, assert_is_redirect_to};

use fake::Fake;
use fake::faker::{internet::en::SafeEmail, name::en::Name};
use std::time::Duration;
use uuid::Uuid;
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    let app = spawn_app().await;
    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": Uuid::new_v4().to_string(),
    });

    let (username, password) = app.add_test_user().await;
    let body = serde_json::json!({
        "username": &username,
        "password": &password,
    });

    app.post_login(&body).await;

    let response = app.post_newsletters(&newsletter_request_body).await;

    assert_is_redirect_to(&response, "/admin/newsletters");
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;

    Mock::given(path(format!("/api/send/{}", app.inbox_id)))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": Uuid::new_v4().to_string(),
    });

    let (username, password) = app.add_test_user().await;
    let body = serde_json::json!({
        "username": &username,
        "password": &password,
    });

    app.post_login(&body).await;
    let response = app.post_newsletters(&newsletter_request_body).await;

    app.dispatch_all_pending_emails().await;

    assert_is_redirect_to(&response, "/admin/newsletters");
}

#[tokio::test]
async fn newsletters_returns_400_for_invalid_data() {
    let app = spawn_app().await;
    let test_cases = vec![
        (
            serde_json::json!({
                "text_content": "Newsletter body as plain text",
                "html_content": "<p>Newsletter body as HTML</p>",
                "idempotency_key": Uuid::new_v4().to_string(),
            }),
            "missing title",
        ),
        (
            serde_json::json!({
                "title": "Newsletter!",
                "idempotency_key": Uuid::new_v4().to_string(),
            }),
            "missing content",
        ),
        (
            serde_json::json!({
                "title": "Newsletter!",
                "text_content": "Newsletter body as plain text",
                "html_content": "<p>Newsletter body as HTML</p>",
            }),
            "The idempotency key cannot be empty",
        ),
    ];

    let (username, password) = app.add_test_user().await;
    let body = serde_json::json!({
        "username": &username,
        "password": &password,
    });

    app.post_login(&body).await;
    for (invalid_body, error_message) in test_cases {
        let response = app.post_newsletters(&invalid_body).await;
        
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    };
}

#[tokio::test]
async fn unauthorized_user_is_rejected() {
    let app = spawn_app().await;

    let body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "plain text", 
        "html_content": "<p>HTML</p>",
        "idempotency_key": Uuid::new_v4().to_string(),
    });

    let response = app.post_newsletters(&body).await;

    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn newsletter_creation_is_idempotent() {
    let app = spawn_app().await;

    create_confirmed_subscriber(&app).await;
    app.user_login().await;

    Mock::given(path(format!("/api/send/{}", app.inbox_id)))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Body</p>",
        "idempotency_key": Uuid::new_v4().to_string(),
    });

    let response = app.post_publish_newsletter(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");


    let html_page = app.get_publish_newsletter_html().await;

    assert!(
        html_page.contains("<p><i>The newsletter issue has been accepted - emails will go out shortly.</i></p>")
    );

    let response = app.post_publish_newsletter(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_publish_newsletter_html().await;

    assert!(
        html_page.contains("<p><i>The newsletter issue has been accepted - emails will go out shortly.</i></p>")
    );

    app.dispatch_all_pending_emails().await;
}

#[tokio::test]
async fn concurrent_form_submission_is_handled_gracefully() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    app.user_login().await;

    Mock::given(path(format!("/api/send/{}", app.inbox_id)))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Body</p>",
        "idempotency_key": Uuid::new_v4().to_string(),
    });

    let response1 = app.post_publish_newsletter(&newsletter_request_body);
    let response2 = app.post_publish_newsletter(&newsletter_request_body);

    let (response1, response2) = tokio::join!(response1, response2);

    assert_eq!(response1.status(), response2.status());
    assert_eq!(response1.text().await.unwrap(), response2.text().await.unwrap());

    app.dispatch_all_pending_emails().await;
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let name: String = Name().fake();
    let email: String = SafeEmail().fake();

    let body = serde_urlencoded::to_string(&serde_json::json!({
        "name": name,
        "email": email,
    }))
    .unwrap();

    let _mock_guard = Mock::given(path(format!("/api/send/{}", app.inbox_id)))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    app.get_confirmation_links(&email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_link = create_unconfirmed_subscriber(app).await;

    reqwest::get(confirmation_link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}
