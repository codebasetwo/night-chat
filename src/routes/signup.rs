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
use secrecy::{ ExposeSecret, SecretString };
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
    password: SecretString,
}

#[derive(thiserror::Error, Debug)]
pub enum SignUpError {
    #[error("An account with this email already exists.")]
    EmailConflict,

    #[error("Failed to acquire a Postgres connection from the pool")]
    PoolConnectionFailed(#[source] sqlx::Error),

    #[error("Failed to commit SQL transaction to store a new subscriber.")]
    TransactionCommitFailed(#[source] sqlx::Error),

    #[error("Failed to execute subscriber insertion query")]
    UserInsertionFailed(#[source] sqlx::Error),

    #[error("validating user {0}")]
    ValidationError(String),

}

/// Gives actix_web the ability to turn error into web response
impl ResponseError for SignUpError {
    fn status_code(&self) -> StatusCode {
        match self {
            SignUpError::EmailConflict => StatusCode::CONFLICT, // 409 Conflict
            SignUpError::PoolConnectionFailed(_) |
            SignUpError::TransactionCommitFailed(_) | 
            SignUpError::UserInsertionFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            SignUpError::ValidationError(_) => StatusCode::BAD_REQUEST,
        }
    }
}

impl TryFrom<FormSignUpData> for NewSubscriber {
    type Error = String;
    fn try_from(value: FormSignUpData) -> Result<Self, Self::Error> {
        let email = SubscriberEmail::parse(value.email)?;
        let first_name = SubscriberName::parse(value.first_name)?;
        let last_name = SubscriberName::parse(value.last_name)?;
        let password = SubscriberPassword::parse(value.password.expose_secret().to_string())?;

        Ok(
            Self{ 
                email, 
                first_name, 
                last_name, 
                password,
        })
    }
}

#[tracing::instrument(
    name = "Signup user",
    skip(form, pool),
    fields(
        first_name = %form.first_name,
        email = %form.email,
    )
)]
pub async fn signup(
    form: web::Form<FormSignUpData>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, SignUpError> {
    // Get the form data and convert to a new subscriber
    let new_subscriber: NewSubscriber = form.0.try_into().map_err(SignUpError::ValidationError)?;
    let subscriber_password = SubscriberPassword::new(new_subscriber.password.expose_secret().to_string()).await;
    
    // Insert the subscriber into the database
    // Use transaction to ensure that the subscriber is only inserted if all operations succeed
    let mut transaction = pool
        .begin()
        .await
        .map_err(SignUpError::PoolConnectionFailed)?;

    let _user_id = insert_users(&new_subscriber, subscriber_password.hashed_password, &mut transaction)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                SignUpError::EmailConflict
            }
            other_sql_error => SignUpError::UserInsertionFailed(other_sql_error),
        })?;
    // commit the the transaction to save subcriber in the 
    transaction
        .commit()
        .await
        .map_err(SignUpError::TransactionCommitFailed)?;
    // Return a response to client
    Ok(HttpResponse::Created().finish())
}


#[tracing::instrument(
    name = "Inserting new user in database.",
    skip(new_subscriber, transaction),
)]
pub async fn insert_users(
    new_subscriber: &NewSubscriber,
    hashed_password: SecretString, 
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
        hashed_password.expose_secret(),
    );
    transaction.execute(query).await?;
    Ok(user_id)

}

