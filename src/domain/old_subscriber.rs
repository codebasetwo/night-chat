use crate::domain::subscriber_email::SubscriberEmail;
use crate::domain::subscriber_password::SubscriberPassword;

pub struct LoginCredentials {
    pub email:SubscriberEmail,
    pub password: SubscriberPassword,
}


