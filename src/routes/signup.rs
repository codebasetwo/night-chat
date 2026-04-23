use actix_web::{
    http::StatusCode,
    web,
    HttpResponse,
    ResponseError,
};
use::sqlx::{
    Executor,
};
use sqlx::{
    PgPool,
    Postgres,
    Transaction,
};
use anyhow::Context;
use secrecy::ExposeSecret;
use uuid::Uuid;
use crate::domain::NewSubscriber;
use crate::domain::SubscriberEmail;
use crate::domain::SubscriberName;
use crate::domain::SubscriberPassword;

#[derive(serde::Deserialize)]
pub struct FormSignUpData {
    email: String,
    first_name: String,
    last_name: String,
    password: String,
}

#[derive(thiserror::Error, Debug)]
pub enum SignUpError {
    // add error annotations to variant for display trait
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    // add anyhow to get any other kind of error
    UnexpectedError(#[from] anyhow::Error)
}

/// Gives actix_web the ability to turn error into web response
impl ResponseError for SignUpError {
    fn status_code(&self) -> StatusCode {
        match self {
            SignUpError::ValidationError(_) => StatusCode::BAD_REQUEST,
            SignUpError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl TryFrom<FormSignUpData> for NewSubscriber {
    type Error = String;
    fn try_from(value: FormSignUpData) -> Result<Self, Self::Error> {
        let email = SubscriberEmail::parse(value.email)?;
        let first_name = SubscriberName::parse(value.first_name)?;
        let last_name = SubscriberName::parse(value.last_name)?;
        let password = SubscriberPassword::parse(value.password)?;

        Ok(
            Self{ 
                email, 
                first_name, 
                last_name, 
                password 
        })
    }
}

pub async fn signup(
    form: web::Form<FormSignUpData>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, SignUpError> {
    // Get the form data and convert to a new subscriber
    let new_subscriber = form.0.try_into().map_err(SignUpError::ValidationError)?;
    // Insert the subscriber into the database
    // Use transaction to ensure that the subscriber is only inserted if all operations succeed
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let _user_id = insert_users(&new_subscriber, &mut transaction)
        .await
        .context("Failed to insert new subscriber in the database.")?;
    // commit the the transaction to save subcriber in the 
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new subscriber.")?;
    // Return a response to client
    Ok(HttpResponse::Ok().finish())
}


pub async fn insert_users(
    new_subscriber: &NewSubscriber, 
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Uuid, sqlx::Error> {
    let user_id = Uuid::new_v4();
    let query = sqlx::query!(
        r#"
        INSERT INTO users (id, email, first_name, last_name, password_hash)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        user_id,
        new_subscriber.email.as_ref(),
        new_subscriber.first_name.as_ref(),
        new_subscriber.last_name.as_ref(),
        new_subscriber.password.hashed_password.expose_secret(),
    );
    transaction.execute(query).await?;
    Ok(user_id)

}

