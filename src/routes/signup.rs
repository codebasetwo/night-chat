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
use crate::utils::StoreTokenError;
use crate::utils::email::EmailClient;
use crate::utils::{Token, spawn_blocking_with_tracing};
use chrono::{Duration, Utc};

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
    UserInsertionFailed(#[from] StoreTokenError),

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
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .json(serde_json::json!(
                {
                    "error": self.to_string(),
                }
            ))

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
    let subscriber_password: SubscriberPassword = SubscriberPassword::new(new_subscriber.password.expose_secret().to_string())
        .await;
            
    // Insert the subscriber into the database
    // Use transaction to ensure that the subscriber is only inserted if all operations succeed
    let mut tx = pool
        .begin()
        .await
        .map_err(SignUpError::PoolConnectionFailed)?;

    let user_id = insert_users(&new_subscriber, subscriber_password.hashed_password, &mut tx)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                SignUpError::EmailConflict
            }
            other_sql_error => SignUpError::UserInsertionFailed(StoreTokenError(other_sql_error)),
    })?;

    let token = Token::new(
        user_id,
        "activation",
        Utc::now() + Duration::hours(24)
    );

    // Store token
    token.insert_token(&mut tx)
        .await
        .map_err(|e| SignUpError::UserInsertionFailed(e))?;

    // commit the the transaction to save subcriber in the 
    tx
        .commit()
        .await
        .map_err(SignUpError::TransactionCommitFailed)?;

    let recipient = new_subscriber.email.as_ref();
    let first_name = new_subscriber.first_name.as_ref();
    let subject= "Welcome Email";

    let email_client = EmailClient::build(
        first_name, recipient, subject, &token.plaintext
    ).map_err(|e| SignUpError::ValidationError(format!("Failed to create email client: {}", e)))?;

    // fire and forget email sending task, we don't want to make the user wait for the email to be sent before we respond to them
    spawn_blocking_with_tracing(move ||
        match email_client.send_email() {
            Ok(response) => {
                tracing::info!("email sent sucessfully: {:?}", response);
            }
            Err(e) => {
                tracing::error!("Message not sent.{:?}", e);
            },
        }
    );

    // Return a response to client
    Ok(HttpResponse::Created().json(serde_json::json!(
        {
            "message": "User created Successfully please check your email to activate your account",
        }
    )))
}


#[tracing::instrument(
    name = "Inserting new user in database.",
    skip(new_subscriber, hashed_password, transaction),
)]
pub async fn insert_users(
    new_subscriber: &NewSubscriber,
    hashed_password: SecretString, 
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Uuid, sqlx::Error> {
    let user_id = Uuid::new_v4();
    let query = sqlx::query!(
        r#"
        INSERT INTO users (id, email, first_name, last_name, password_hash, is_activated)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        user_id,
        new_subscriber.email.as_ref(),
        new_subscriber.first_name.as_ref(),
        new_subscriber.last_name.as_ref(),
        hashed_password.expose_secret(),
        false,
    );
    transaction.execute(query).await?;
    Ok(user_id)

}


pub async fn send_welcome_email(recipient: &str, recipient_name: &str, token: &SecretString) -> Result<(), Box<dyn std::error::Error>> {
    let subject = "Welcome to our service!";
    let email_client = EmailClient::build(recipient_name, recipient, subject, token)?;
    email_client.send_email()?;
    Ok(())
}

#[derive(serde::Deserialize)]
pub struct ActivateUserData {
    token: String,
}

#[derive(thiserror::Error, Debug)]
pub enum ActivateUserError {
    #[error("Failed to acquire a Postgres connection from the pool")]
    PoolConnectionFailed(#[source] sqlx::Error),

    #[error("Failed to execute SQL query to activate user account.")]
    UserActivationFailed(#[source] sqlx::Error),

    #[error("Invalid or expired activation token.")]
    InvalidToken,
}

pub async fn activate_user_handler(
    query: web::Query<ActivateUserData>,
    pool:web::Data<PgPool>,
) -> Result<HttpResponse, ActivateUserError> {
    // Validate the token and activate the user account
    // This would involve checking the token against the database, ensuring it's validity, and then updating the user's account to be activated if the token is valid.
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ActivateUserError::PoolConnectionFailed(e))?;

    let token = query.0.token;
    validate_token(&token, "activation", &mut tx)
        .await?;

    tx
        .commit()
        .await
        .map_err(|e| ActivateUserError::PoolConnectionFailed(e))?;

    Ok(
        HttpResponse::NoContent().finish()
    )
}


async fn validate_token(token: &str, activation: &str, tx: &mut Transaction<'_, Postgres>) -> Result<(), ActivateUserError> {
    let token_hash = Token::hash_token(token);
    // check the hash against the token table in the databse to ensure the token is valid and not expired
    let token_record = sqlx::query!(
            r#"
                SELECT hash, user_id, expiry
                FROM tokens
                WHERE hash = $1
                AND expiry > NOW()
            "#,
            token_hash,
    )
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| ActivateUserError::PoolConnectionFailed(e))?;
    
    match token_record {
        Some(record) => {
            // Token is valid and not expired
            tracing::info!("Token is valid for user");
            update_user_activation_status(record.user_id, tx).await?;
            delete_token(record.user_id, activation, tx).await?;
            Ok(())
        }
        None => {
            // Token not found or expired
            return Err(ActivateUserError::InvalidToken);
        }
    }
}


async fn update_user_activation_status(
    user_id: uuid::Uuid,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), ActivateUserError> {
    sqlx::query!(
        r#"
            UPDATE users
            SET is_activated = true
            WHERE id = $1
        "#,
        user_id,
    )
    .execute(&mut **tx)
    .await
    .map_err(|e| ActivateUserError::UserActivationFailed(e))?;
    Ok(())
}

pub async fn delete_token(
    user_id: uuid::Uuid, 
    activation: &str, 
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), ActivateUserError> {
    sqlx::query!(
        r#"
        DELETE FROM tokens
        WHERE scope = $1 AND user_id = $2
        "#,
        activation,
        user_id,
    )
    .execute(&mut **tx)
    .await
    .map_err(|e| ActivateUserError::PoolConnectionFailed(e))?;
    Ok(())
}