use actix_web::{
    web,
    HttpResponse,
    ResponseError,
    http::StatusCode,
};
use anyhow:: { Context };
use chrono::{Duration, Utc};
use secrecy::{ ExposeSecret, SecretString};
use sqlx::{ PgPool, };
use uuid;
use crate::domain::{ SubscriberPassword };
use crate::utils::{ Token, get_user_data, spawn_blocking_with_tracing, StoreTokenError };

#[derive(thiserror::Error, Debug)]
pub enum LoginError{
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),

    /// Database lookups failed or timed out
    #[error("Database error occurred")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Invalid email or password")]
    InvalidCredentials,

    #[error(transparent)]
    TokenError(#[from] StoreTokenError),

    #[error("something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

// Pass error to actix_web
impl ResponseError for LoginError {
    fn status_code(&self) -> StatusCode {
        match self {
            LoginError::AuthError(_) => StatusCode::UNAUTHORIZED,
            LoginError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            LoginError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            LoginError::TokenError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            LoginError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(serde::Deserialize)]
pub struct LoginCredentials {
    pub email: String,
    pub password: SecretString,
}

impl From<argon2::password_hash::Error> for LoginError {
    fn from(err: argon2::password_hash::Error) -> Self {
        LoginError::AuthError(anyhow::anyhow!(err))
    }
}

#[tracing::instrument(
    name = "Login in user",
    skip(login_form, pool),
    fields(
        email = %login_form.email,
    )
)]
pub async fn login(
    login_form: web::Form<LoginCredentials>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, LoginError> {
    let email = login_form.0.email;
    let password = login_form.0.password;

    let (user_id, expected_password_hash): (uuid::Uuid, SecretString) = get_user_data(&pool, &email)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => LoginError::InvalidCredentials,
            other_db_error => LoginError::DatabaseError(other_db_error),
        })?;

    let plaintext_password = password.expose_secret().as_bytes().to_vec();
    let hash_string = expected_password_hash.expose_secret().to_string();

    // Verify password off-thread
    let verification_result = spawn_blocking_with_tracing( move || {
        SubscriberPassword::verify_password(
            &plaintext_password,
            &hash_string,
        )
    })
    .await
    .map_err(|e| 
        LoginError::UnexpectedError(anyhow::anyhow!(e))
    )
    .context("failed to spawn blocking task")?;
    
    // If password verification library failed or password was wrong
    verification_result.map_err(|_auth_failed| LoginError::InvalidCredentials)?;
        
    // Get database connection
    let mut transaction = pool
        .begin()
        .await?;
    
    // Generate token
    let token = Token::new(
        user_id,
        "authentication",
        Utc::now() + Duration::hours(24)
    );
    // Store token
    token.insert_token(&mut transaction)
        .await?;

    transaction
        .commit()
        .await?;
    Ok(HttpResponse::Ok().finish())

}