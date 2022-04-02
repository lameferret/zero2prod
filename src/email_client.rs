use reqwest::Client;
use secrecy::{ExposeSecret, Secret};

use crate::domain::SubscriberEmail;
#[derive(Debug)]
pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
    auth_token: Secret<String>,
}
// #[derive(serde::Serialize)]
// #[serde(rename_all = "PascalCase")]
// struct SendEmailRequest<'a> {
//     to: &'a str,
//     subject: &'a str,
//     from: &'a str,
//     text_body: &'a str,
// }

#[derive(serde::Serialize)]
struct EmailAndName<'a> {
    email: &'a str,
    name: &'a str,
}
#[derive(serde::Serialize)]
struct ToAndSubject<'a> {
    // !!!!! Hard coding 1 as I will send email to 1 user only !!!!!
    to: [EmailAndName<'a>; 1],
    subject: &'a str,
}
#[derive(serde::Serialize)]
struct ContentAndValue<'a> {
    r#type: TextOrHtml,
    value: &'a str,
}
#[derive(serde::Serialize)]
enum TextOrHtml {
    Text,
    Html,
}

// impl TextOrHtml {
//     pub fn as_str(&self) -> &'static str {
//         match self {
//             TextOrHtml::Html => "text/html",
//             TextOrHtml::Text => "text/plain",
//         }
//     }
// }

// impl TryFrom<String> for TextOrHtml {
//     type Error = String;
//     fn try_from(s: String) -> Result<Self, Self::Error> {
//         match s.to_lowercase().as_str() {
//             "text/plain" => Ok(Self::Text),
//             "text/html" => Ok(Self::Html),
//             other => Err(format!(
//                 "{} is not supported content type. Use either `text/plain` or `text/html`.",
//                 other
//             )),
//         }
//     }
// }

#[derive(serde::Serialize)]
struct SendGridStruct<'a> {
    personalizations: [ToAndSubject<'a>; 1],
    content: [ContentAndValue<'a>; 2],
    from: EmailAndName<'a>,
    reply_to: EmailAndName<'a>,
}

// curl --request POST \
// --url https://api.sendgrid.com/v3/mail/send \
// --header 'Authorization: Bearer <<YOUR_API_KEY>>' \
// --header 'Content-Type: application/json' \
// --data '{
//     "personalizations":
//         [{"to":[{"email":"john.doe@example.com","name":"John Doe"}],"subject":"Hello, World!"}],
//     "content":
//         [{"type":"text/plain","value":"Heya!"}],
//     "from":
//         {"email":"sam.smith@example.com","name":"Sam Smith"},
//      "reply_to":
//         {"email":"sam.smith@example.com","name":"Sam Smith"}
//         }'

impl EmailClient {
    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        text_content: &str,
        html_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/send", self.base_url);
        // let request_body = SendEmailRequest {
        //     to: recipient.as_ref().to_owned(),
        //     subject: subject.to_owned(),
        //     from: self.sender.as_ref().to_owned(),
        //     text_body: text_content.to_owned(),
        // };

        // let request_body_temp = SendGridStruct {
        //     personalizations: [ToAndSubject {
        //         to: [EmailAndName {
        //             email: recipient.as_ref(),
        //             // Would get name later on as user input
        //             name: recipient.as_ref().split_once("a").unwrap().0,
        //         }],
        //         subject,
        //     }],
        //     content: [
        //         ContentAndValue {
        //             r#type: TextOrHtml::Html,
        //             value: html_content,
        //         },
        //         ContentAndValue {
        //             r#type: TextOrHtml::Text,
        //             value: text_content,
        //         },
        //     ],
        //     from: EmailAndName {
        //         email: self.sender.as_ref(),
        //         name: self.sender.as_ref().split_once("@").unwrap().0,
        //     },
        //     reply_to: EmailAndName {
        //         email: self.sender.as_ref(),
        //         name: self.sender.as_ref().split_once("@").unwrap().0,
        //     },
        // };
        let request_body_temp = serde_json::json!(
        {
            "personalizations":
                [{
                    "to":[{
                        "email":recipient.as_ref(),
                        "name":"John Doe"}],
                    "subject":subject.to_owned()
                }],
            "content":
                [{
                    "type":"text/plain","value": text_content,
                    "type": "text/html", "value": html_content
                }],
            "from":{
                "email":self.sender.as_ref(),
                "name":"Sam Smith"
            },
            "reply_to":{
                "email":self.sender.as_ref(),
                "name":"Sam Smith"
            }
        });
        let _builder = self
            .http_client
            .post(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.auth_token.expose_secret()),
            )
            .json(&request_body_temp)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        auth_token: Secret<String>,
        timeout: std::time::Duration,
    ) -> Self {
        let http_client = Client::builder().timeout(timeout).build().unwrap();
        Self {
            http_client,
            base_url,
            sender,
            auth_token,
        }
    }
}

#[cfg(test)]
mod tests {
    use claim::{assert_err, assert_ok};
    use fake::{
        faker::{
            internet::en::SafeEmail,
            lorem::en::{Paragraph, Sentence},
        },
        Fake, Faker,
    };
    use secrecy::Secret;
    use wiremock::{
        matchers::{any, header, header_exists, method, path},
        Mock, MockServer, Request, ResponseTemplate,
    };

    use crate::domain::SubscriberEmail;

    use crate::email_client::EmailClient;

    struct SendEmailBodyMatcher;

    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);

            if let Ok(body) = result {
                dbg!(&body);
                body.get("personalizations").unwrap()[0].get("to").is_some()
                    && body.get("content").is_some()
                    && body.get("from").is_some()
            } else {
                false
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

    fn email_client(base_url: String) -> EmailClient {
        EmailClient::new(
            base_url,
            email(),
            Secret::new(Faker.fake()),
            std::time::Duration::from_millis(200),
        )
    }

    #[tokio::test]
    async fn send_email_fires_a_request_to_base_url() {
        let mock_server = MockServer::start().await;

        Mock::given(header_exists("Authorization"))
            .and(header("Content-Type", "application/json"))
            .and(path("send"))
            .and(method("POST"))
            .and(SendEmailBodyMatcher)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let _ = email_client(mock_server.uri())
            .send_email(email(), &subject(), &content(), &content())
            .await;
    }

    #[tokio::test]
    async fn send_email_succeeds_if_the_server_returns_200() {
        let mock_server = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client(mock_server.uri())
            .send_email(email(), &subject(), &content(), &content())
            .await;

        assert_ok!(outcome);
    }

    #[tokio::test]
    async fn send_email_fails_if_the_server_returns_500() {
        let mock_server = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;
        let outcome = email_client(mock_server.uri())
            .send_email(email(), &subject(), &content(), &content())
            .await;
        assert_err!(outcome);
    }

    #[tokio::test]
    async fn send_email_times_out_if_the_server_takes_too_long() {
        let mock_server = MockServer::start().await;
        let response = ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(180));

        Mock::given(any())
            .respond_with(response)
            .expect(1)
            .mount(&mock_server)
            .await;
        let outcome = email_client(mock_server.uri())
            .send_email(email(), &subject(), &content(), &content())
            .await;
        assert_err!(outcome);
    }
}
