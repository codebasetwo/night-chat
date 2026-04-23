use actix_web::{
    web,
    HttpResponse,
    ResponseError,
    http::StatusCode,
};
use anyhow:: { Context };
use chrono::{ DateTime, Duration, Utc};
use secrecy::{ ExposeSecret, SecretString, Secret };
use sqlx::{ Executor, PgPool, Postgres, Transaction };
use rand::distr::Alphanumeric;
use rand::{rng, RngExt};
use uuid;
use crate::domain::{ SubscriberPassword };

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
            .await?
            .ok_or(LoginError::UserNotFound)?;
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
    let token = generate_token();
     let hashed_token = hash_token(&token); // Hash the token before storing
    let expiry = Utc::now() + Duration::days(7);
    // Store token
    store_token(&mut transaction, user_id, &hashed_token,  expiry, "authentication",)
        .await
        .context("Failed to store the authentication token")?;

    transaction
        .commit()
        .await
        .context("failed to commit SQL transaction to store the authentication token")?;
    Ok(HttpResponse::Ok().finish())

}

pub struct StoreTokenError(sqlx::Error);

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database failure was encountered while trying to store a subscription token."
        )
    }
}

pub fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

// store token
async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: uuid::Uuid,
    token: &[u8],
    expiry: DateTime<Utc>,
    scope: &str,
) -> Result<(), StoreTokenError> {
    let query =sqlx::query!(
        r#"
        INSERT INTO tokens (user_id, hash, expiry, scope)
        VALUES ($1, $2, $3, $4)
        "#,
        user_id,
        token,
        expiry,
        scope,
    );

    transaction
        .execute(query)
        .await
        .map_err(StoreTokenError)?;
    Ok(())
} 

// Get user data
async fn get_user_data(
    pool: &PgPool, 
    email: &str,
) -> Result<Option<(uuid::Uuid, SecretString)>, anyhow::Error> {
    // 1. Fetch the user record by email
    let row = sqlx::query!(
        r#"SELECT id, password_hash FROM users WHERE email = $1"#,
        email,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform query to retrieve credentials")?;
    Ok(row.map(|r| (r.id, Secret::new(r.password_hash))))

}

// Get token
fn generate_token() -> String {
    let rng = rng();
    rng.sample_iter(Alphanumeric)
        .take(25)
        .map(char::from)
        .collect()
}

// Hash token for storage (you should never store raw tokens)
fn hash_token(token: &str) -> Vec<u8> {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hasher.finalize().to_vec()
}