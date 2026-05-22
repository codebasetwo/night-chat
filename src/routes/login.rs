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
use crate::utils::{ Token, get_user_data };

#[derive(thiserror::Error, Debug)]
pub enum LoginError{
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("User not found")]
    UserNotFound,
    #[error("something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

// Pass error to actix_web
impl ResponseError for LoginError {
    fn status_code(&self) -> StatusCode {
        match self {
            LoginError::AuthError(_) => StatusCode::UNAUTHORIZED,
            LoginError::UserNotFound => StatusCode::UNAUTHORIZED,
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
    let credentials = LoginCredentials {
        email: login_form.0.email,
        password: login_form.0.password,
    };
    let user_password = SubscriberPassword::new(credentials.password.expose_secret());
    let (user_id, expected_password_hash): (uuid::Uuid, SecretString) = 
            get_user_data(&pool, &credentials.email)
            .await
            .map_err(|_| LoginError::UserNotFound)?;
    let _ = SubscriberPassword::verify_password(
        user_password.plaintext_password.expose_secret().as_bytes(),
        expected_password_hash.expose_secret(),
    )?;

    // Get database connection
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    
    // Generate token
    let token = Token::new(
        user_id,
        "authentication",
        Utc::now() + Duration::hours(24)
    );
    // Store token
    token.insert_token(&mut transaction)
        .await
        .context("Failed to store the authentication token in the databse")?;

    transaction
        .commit()
        .await
        .context("failed to commit SQL transaction to store the authentication token")?;
    Ok(HttpResponse::Ok().finish())

}