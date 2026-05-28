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
// use uuid;
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

    // Initialize variables to prevent timing attacks
    let mut user_id = None;
    let mut expected_password_hash = SecretString::new(
        "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
        .to_string()
    );

    let email = login_form.0.email;
    let password = login_form.0.password;

    if let Some((uid, stored_password)) = get_user_data(&pool, &email)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => LoginError::InvalidCredentials,
            other_db_error => LoginError::DatabaseError(other_db_error),
        })?
        {
            // set some here if user exists
            user_id = Some(uid);
            // set the real password since user exists.
            expected_password_hash = stored_password;
        }

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
    .context("failed to spawn blocking task")
    .map_err(|e| 
        LoginError::UnexpectedError(anyhow::anyhow!(e))
    )?;
    
    // If password verification library failed or password was wrong
    verification_result.map_err(|_auth_failed| LoginError::InvalidCredentials)?;
    
    // Extract user_id or return error
    // only set to Some if email exists, otherwise remains none, preventing timing attacks
    // by returning early.
    let user_id = user_id.ok_or(LoginError::InvalidCredentials)?;
        
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
    Ok(HttpResponse::Ok().finish()) // return token

}