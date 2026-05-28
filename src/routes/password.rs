use actix_web::{web, HttpResponse, ResponseError, http::StatusCode};
use sqlx::{self, PgPool, Transaction, Postgres};
use chrono::{Duration, Utc};
use secrecy::{SecretString, ExposeSecret};
use crate::utils::Token;
use crate::utils::email::EmailClient;
use crate::utils::spawn_blocking_with_tracing;

use crate::domain::{SubscriberPassword};

#[derive(serde::Deserialize)]
pub struct PasswordResetFormData {
    email: String
}

#[derive(thiserror::Error, Debug)]
pub enum PasswordResetError {
    #[error("Failed to acquire a Postgres connection from the pool")]
    PoolConnectionFailed(#[source] sqlx::Error),
    #[error("Email not found")]
    EmailNotFound,
    #[error("Failed to send password reset email")]
    EmailSendFailed(#[source] anyhow::Error),
    #[error("User account not activated cannot reset password")]
    UserNotActivated,
    #[error("Failed to create email client")]
    EmailClientCreationFailed(#[source] anyhow::Error),
    #[error("Invalid or expired token")]
    InvalidToken,
    #[error("failed to insert token in database")]
    StoreTokenFailed(#[source] anyhow::Error),
}

impl ResponseError for PasswordResetError {
    fn status_code(&self) -> StatusCode {
        match self {
            PasswordResetError::EmailNotFound => StatusCode::NOT_FOUND,
            PasswordResetError::PoolConnectionFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            PasswordResetError::EmailSendFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            PasswordResetError::UserNotActivated => StatusCode::FORBIDDEN,
            PasswordResetError::EmailClientCreationFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            PasswordResetError::InvalidToken => StatusCode::BAD_REQUEST,
            PasswordResetError::StoreTokenFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

pub async fn reset_password(
    form: web::Form<PasswordResetFormData>,
    pool: web::Data<PgPool>,
)-> Result<HttpResponse, PasswordResetError> {
    let email = form.0.email;
    // Check if email exist in the database
    let user = user_exists(&email, &pool).await?;

    let user_id = user.id;

    // Check if user is activated, if not return an error
    user_activated(user_id, &pool).await?;
    // Generate a password reset token
    let token = Token::new(user_id, "password-reset", Utc::now() + Duration::minutes(30));
    // Store the token in database
    let mut tx = pool.begin().await.map_err(|e| PasswordResetError::PoolConnectionFailed(e))?;
    token.insert_token(&mut tx).await.map_err(|_e|
        PasswordResetError::StoreTokenFailed(anyhow::anyhow!("Failed to store token in database"))
    )?;
    tx.commit()
        .await
        .map_err(|e| PasswordResetError::PoolConnectionFailed(e))?;
    // Send password reset email to user with the token
    let email_client = EmailClient::build(
        &user.first_name,
        &user.email,
        "Password Reset Request",
        &token.plaintext,
    ).map_err(|_e| PasswordResetError::EmailClientCreationFailed(anyhow::anyhow!("Failed to create email client")))?;

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

    Ok(HttpResponse::Accepted().finish())


}

#[derive(serde::Deserialize)]
pub struct PasswordResetData {
    token: String,
}

pub struct NewPasswordData {
    password: SecretString,
}

pub async fn handle_password_reset(
    query: web::Query<PasswordResetData>,
    form: web::Form<NewPasswordData>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, PasswordResetError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| PasswordResetError::PoolConnectionFailed(e))?;

    let token = query.0.token;

    let user_id = validate_token(&token, "password-reset", &mut tx).await?;
    // Token is valid and not expired, allow user to reset password
    let new_password = form.0.password.expose_secret().to_string();
    let hashed_password = SubscriberPassword::new(new_password).await;

    update_password(user_id, &hashed_password, &mut tx)
        .await
        .map_err(|_| PasswordResetError::StoreTokenFailed(anyhow::anyhow!("Failed to update password")))?;
    
    // Delete the reset token from database
    sqlx::query!(
        r#"
        DELETE FROM tokens WHERE user_id = $1 AND scope = $2
        "#,
        user_id,
        "password-reset"
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| PasswordResetError::PoolConnectionFailed(e))?;

    tx.commit()
        .await
        .map_err(|e| PasswordResetError::PoolConnectionFailed(e))?;

    Ok(HttpResponse::Ok().finish())

}

async fn validate_token(
    token: &str,
    scope: &str,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<uuid::Uuid, PasswordResetError> {
    let token_hash = Token::hash_token(token);
    let result = sqlx::query!(
        r#"
        SELECT user_id, expiry, hash FROM tokens WHERE hash = $1 AND scope = $2
        "#,
        token_hash,
        scope,
    )
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| PasswordResetError::PoolConnectionFailed(e))?;

    match result {
        Some(record) => {
            // Token is valid and not expired
            tracing::info!("Token is valid for user");
            Ok(record.user_id)
        }
        None => {
            // Token not found or expired
            return Err(PasswordResetError::InvalidToken);
        }
    }
}

async fn update_password(
    user_id:uuid::Uuid,
    hash_password: &SubscriberPassword,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), PasswordResetError>{
    sqlx::query!(
        r#"
        UPDATE users SET password_hash = $1 WHERE id = $2
        "#,
        hash_password.hashed_password.expose_secret(),
        user_id,
    )
    .execute(&mut **tx)
    .await
    .map_err(|e| PasswordResetError::PoolConnectionFailed(e))?;

    Ok(())

}
#[derive(Debug, serde::Serialize)]
pub struct UserRecord {
    pub id: uuid::Uuid,
    pub email: String,
    pub first_name: String,
}

async fn user_exists(email: &str, pool: &PgPool) -> Result<UserRecord, PasswordResetError>{
    let result = sqlx::query_as!(
        UserRecord,
        r#"
        SELECT id, email, first_name FROM users WHERE email = $1
        "#,
        email,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e|  PasswordResetError::PoolConnectionFailed(e))?;

    match result {
        Some(record) => Ok(record),
        None => Err(PasswordResetError::EmailNotFound)
    }
}

async fn user_activated(
    user_id:uuid::Uuid,
    pool: &PgPool,
) -> Result<(), PasswordResetError> {
    let result = sqlx::query!(
        r#"
        SELECT is_activated FROM users WHERE id = $1
        "#,
        user_id,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| PasswordResetError::PoolConnectionFailed(e))?;

    match result.is_activated {
        Some(true) => Ok(()),
        Some(false) => Err(PasswordResetError::UserNotActivated),
        None => Err(PasswordResetError::EmailNotFound),
    }
    
}