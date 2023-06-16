use crate::domain::SubscriberEmail;
use reqwest::Client;
use secrecy::{Secret, ExposeSecret};

pub struct EmailClient {
    base_url: String,
    http_client: Client,
    sender: SubscriberEmail,
    authorization_token: Secret<String>,
    inbox_id: Secret<String>,
}

#[derive(serde::Serialize)]
pub struct SendEmailRequest<'a> {
    from: Address<'a>,
    to: Vec<Address<'a>>,
    subject: &'a str,
    html: &'a str,
    text: &'a str,
}

#[derive(serde::Serialize)]
pub struct Address<'a> {
    pub email: &'a str,
}

impl EmailClient {
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        authorization_token: Secret<String>,
        inbox_id: Secret<String>,
        timeout: std::time::Duration,
    ) -> Self {
        let http_client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap();

        Self {
            base_url,
            http_client,
            sender,
            authorization_token,
            inbox_id,
        }
    }

    pub async fn send_email(
        &self,
        recipient: &SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/api/send/{}", self.base_url, self.inbox_id.expose_secret());
        let request_body = SendEmailRequest {
            from: Address { email: self.sender.as_ref() },
            to: vec![Address { email: recipient.as_ref() }],
            subject,
            html: html_content,
            text: text_content,
        };

        self
            .http_client
            .post(&url)
            .header(
                "Api-Token",
                self.authorization_token.expose_secret()
            )
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use claim::{assert_err, assert_ok};
    use crate::domain::SubscriberEmail;
    use crate::email_client::EmailClient;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Sentence, Paragraph};
    use fake::{Fake, Faker};
    use secrecy::Secret;
    use wiremock::matchers::{any, header_exists, header, path, method};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    struct SendEmailBodyMatcher;

    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            let result: Result<serde_json::Value, _> = 
                serde_json::from_slice(&request.body);

            match result {
                Ok(body) => { 
                    body.get("from").unwrap().get("email").is_some()
                        && body.get("to").unwrap()[0].get("email").is_some()
                        && body.get("subject").is_some()
                        && body.get("html").is_some()
                        && body.get("text").is_some()
                },
                Err(_) => false,
            }
        }
    }

    fn subject() -> String {
        Sentence(1..2).fake()
    }

    fn content() -> String {
        Paragraph(1..10).fake()
    }

    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    fn email_client(base_url: String, inbox_id: Secret<String>) -> EmailClient {
        EmailClient::new(
            base_url,
            email(),
            Secret::new(Faker.fake()),
            inbox_id,
            std::time::Duration::from_millis(200),
        )
    }

    #[tokio::test]
    async fn send_email_fires_a_request_to_base_url() {
        let mock_server = MockServer::start().await;
        let inbox_id: String = Faker.fake();

        let email_client = email_client(mock_server.uri(), Secret::new(inbox_id.clone()));

        Mock::given(header_exists("Api-Token"))
            .and(header("Content-Type", "application/json"))
            .and(path(format!("/api/send/{}", &inbox_id)))
            .and(method("POST"))
            .and(SendEmailBodyMatcher)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(&email(), &subject(), &content(), &content())
            .await;

        assert_ok!(outcome);
    }

    #[tokio::test]
    async fn send_email_fails_if_the_server_returns_500() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri(), Secret::new(Faker.fake()));

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(&email(), &subject(), &content(), &content())
            .await;

        assert_err!(outcome);
    }

    #[tokio::test]
    async fn send_email_times_out_if_the_server_takes_too_long() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri(), Secret::new(Faker.fake()));

        let response = ResponseTemplate::new(200)
            .set_delay(std::time::Duration::from_secs(180));
        Mock::given(any())
            .respond_with(response)
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(&email(), &subject(), &content(), &content())
            .await;

        assert_err!(outcome);
    }
}
