use std::{env, path::Path};
use lettre::{
    message::{Mailbox, SinglePart, MultiPart, header::ContentType}
};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::response::Response;
use lettre::{ Message, SmtpTransport, Transport, Address};
use secrecy::{ ExposeSecret, SecretString };
use dotenvy::dotenv;


pub struct EmailClient{
    app_name: SecretString,
    html: String,
    smtp_username: SecretString,
    password: SecretString,
    recipient: String,
    recipient_name: String,
    smtp_server: SecretString,
    subject: String,
    text: String,
}

impl EmailClient {
    pub fn build(
        recipient_name: &str, 
        recipient: &str, 
        subject: &str,
        token: &SecretString,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Load .env file into the system environment
        dotenv()?;
        let app_name = SecretString::new(env::var("APP_NAME").expect("App name not present"));
        let smtp_username = SecretString::new(env::var("SMTP_SENDER").expect("No sender Exists"));
        let password = SecretString::new(env::var("SMTP_PASSWORD").expect("Please Provide password. No credentials in environment"));
        let smtp_server = SecretString::new(env::var("SMTP_SERVER").expect("expected an smtp server in environment"));
        let html = Self::read_html(token);
        let text = Self::read_txt(token);

        Ok(Self {
            app_name,
            html,
            smtp_username,
            password,
            recipient: recipient.to_string(),
            recipient_name: recipient_name.to_string(),
            smtp_server,
            subject: subject.to_string(),
            text,
        })
    }

    pub fn send_email(&self) -> Result<Response, Box<dyn std::error::Error>> {
        let email = Message::builder()
            .from(Mailbox::new(Some(self.app_name.expose_secret().to_owned()), 
            self.smtp_username.expose_secret().parse::<Address>().unwrap()))
            .to(Mailbox::new(Some(self.recipient_name.to_owned()), self.recipient.parse::<Address>().unwrap()))
            .subject(self.subject.to_owned())
            .multipart(
                MultiPart::alternative() // This is composed of two parts.
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(self.text.to_owned()), // Every message should have a plain text fallback.
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(self.html.to_owned()),
                    ),
            )
            .unwrap();
    
        let creds = Credentials::new(self.smtp_username.expose_secret().to_owned(), self.password.expose_secret().to_owned());
    
        let mailer = 
            // Open a remote connection to gmail
            SmtpTransport::relay(self.smtp_server.expose_secret())
                .unwrap()
                .credentials(creds)
                .timeout(Some(std::time::Duration::from_secs(10)))
                .build();
    
        let response = mailer.send(&email)?;
        Ok(response)
    }
    
    fn read_html(token: &SecretString) -> String {
        // Read the HTML template from a file
        let path = Path::new("email_templates/welcome_email.html");
        let html = std::fs::read_to_string(path).expect("Failed to read HTML file. check if file exists");
        html.replace("{{token}}", token.expose_secret().as_str())

    }

    fn read_txt(token: &SecretString) -> String {
        // Read the HTML template from a file
        let path = Path::new("email_templates/welcome_email.txt");
        let text = std::fs::read_to_string(path).expect("Failed to read TXT file. check if file exists");
        text.replace("{{token}}", token.expose_secret().as_str())
    }



}    

