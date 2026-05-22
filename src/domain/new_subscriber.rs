use crate::domain::subscriber_name::SubscriberName;
use crate::domain::subscriber_email::SubscriberEmail;
use secrecy::SecretString;

pub struct NewSubscriber {
    pub email: SubscriberEmail,
    pub first_name: SubscriberName,
    pub last_name: SubscriberName,
    pub password: SecretString,
}
