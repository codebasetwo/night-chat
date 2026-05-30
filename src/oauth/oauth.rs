use actix_web::{web, HttpResponse, ResponseError, http::StatusCode};
use secrecy::{SecretString, ExposeSecret};
use sqlx::PgPool;
use chrono::{Duration, Utc};
use crate::utils::Token;
use crate::utils::PkceChallenge;

#[derive(thiserror::Error, Debug)]
pub enum OauthError {
    #[error("Auth error occured: {0}")]
    BadRequest(String),
    #[error("Token error: {0}")]
    TokenError(String),
    #[error("Unexpected error: {0}")]
    Unexpected(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}

impl ResponseError for OauthError {
    fn status_code(&self) -> StatusCode {
        match self {
            OauthError::BadRequest(_) => StatusCode::BAD_REQUEST,
            OauthError::TokenError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            OauthError::Unexpected(_)  => StatusCode::INTERNAL_SERVER_ERROR,
            OauthError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Clone)]
pub struct AppConfig {
    pub google_client_id: String,
    pub google_client_secret: SecretString,
    pub redirect_uri: String,
}

pub async fn oauth(
    config: web::Data<AppConfig>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, OauthError> {
    // create a random state parameter for CSRF protection
    let state = Token::generate_token();

    let pkce = PkceChallenge::new();
    // persist the state so the callback can verify it (CSRF protection)
    store_oauth_state(&pool, &state, &pkce.code_verifier)
        .await
        .map_err(|e| OauthError::Unexpected(format!("Failed to store state: {}", e)))?;

    let auth_base_url = "https://accounts.google.com/o/oauth2/v2/auth";
    let google_client_id = &config.google_client_id;
    let redirect_uri = &config.redirect_uri;
    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&state={}&scope=openid%20email%20profile&code_challenge={}",
        auth_base_url, google_client_id, redirect_uri, state, pkce.code_challenge,
    );

    Ok(
        HttpResponse::Found()
        .append_header(("Location", auth_url))
        .finish()
    )
}


#[derive(Debug, serde::Deserialize)]
pub struct CallbackParams {
    code: String,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    id_token: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct GoogleUserInfo {
    sub: String,     // Google's unique user ID
    email: Option<String>,
    name: Option<String>,
    picture: Option<String>,
    email_verified: Option<bool>,
}

pub async fn redirect_uri(
    config: web::Data<AppConfig>,
    query: web::Query<CallbackParams>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, OauthError> {
    if let Some(error) = &query.error {
        return Err(OauthError::BadRequest(format!("Auth rejected {}", error)));
    }
    let state = match &query.state {
        Some(s) => s,
        None => return Err(OauthError::BadRequest("Invalid state parameter".into())),
    };

    // Verify the state parameter to prevent CSRF attacks and get the code_verifier
    let code_verifier = verify_and_delete_oauth_state(&pool, state)
        .await
        .map_err(|e| OauthError::Unexpected(e.to_string()))?
        .ok_or_else(|| OauthError::BadRequest("Invalid or expired state".into()))?;

    // Exchange the code for tokens
    let http_client = reqwest::Client::new();
    
    let token_response: Result<reqwest::Response, _> = http_client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", query.code.as_str()),
            ("client_id", config.google_client_id.as_str()),
            ("client_secret", config.google_client_secret.expose_secret()),
            ("redirect_uri", config.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier.as_str())
        ])
        .send()
        .await;

    let token_data: TokenResponse = match token_response {
        Ok(resp) => match resp.json().await {
            Ok(data) => data,
            Err(e) => {
                return Err(OauthError::TokenError(format!("Failed to parse token response {}", e)));
            }
        },
        Err(e) => {
            return Err(OauthError::TokenError(format!("Token Exchange failed {}", e)));
        }
    };

    // Get user info from Google
    let user_info = reqwest::Client::new()
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(&token_data.access_token)
        .send()
        .await
        .map_err(|e| OauthError::Unexpected(format!("Failed to get user info: {}", e)))?
        .json::<GoogleUserInfo>()
        .await
        .map_err(|e| OauthError::Unexpected(format!("Failed to parse user info response {e}")))?;

    let email = user_info.email
        .ok_or_else(|| OauthError::BadRequest("Google did not return an email".into()))?;

    // Find-or-create user  ← THE KEY INTEGRATION POINT
    let user_id = find_or_create_oauth_user(
        &pool,
        &user_info.sub,
        &email,
        user_info.name.as_deref(),
    )
    .await
    .map_err(|e| OauthError::Unexpected(format!("DB error: {}", e)))?;

    // Start DB transaction
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| OauthError::Unexpected(format!("Failed to start DB transaction: {}", e)))?;

    // Get token expiry time
    let expiry = Utc::now() + Duration::hours(24);
    //  Issue token
    let session_token = Token::new(user_id, "authentication", expiry);
    session_token.insert_token(&mut tx)
        .await
        .map_err(|e| OauthError::Unexpected(format!("Failed to store token: {}", e)))?;

    tx.commit().await?;
    Ok(
        HttpResponse::Ok().json(
            serde_json::json!({ "token": session_token.plaintext })
        )
    )

}

/// Persist the OAuth state value so the callback can verify it.
async fn store_oauth_state(pool: &PgPool, state: &str, code_verifier: &str ) -> Result<(), sqlx::Error> {
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(10);
    sqlx::query!(
        "INSERT INTO oauth_states (state, expires_at, code_verifier) VALUES ($1, $2, $3)",
        state, expires_at, code_verifier
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Verify the state value and delete it (one-time use).
async fn verify_and_delete_oauth_state(
    pool:  &PgPool,
    state: &str,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<_> = sqlx::query!(
        r#"DELETE FROM oauth_states WHERE state = $1 AND expires_at > NOW() - INTERVAL '10 minutes'
         RETURNING code_verifier"#,
        state, 
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.code_verifier))
}



/// Core find-or-create: returns the user_id to issue a token for.
async fn find_or_create_oauth_user(
    pool:       &PgPool,
    provider_user_id: &str,   // Google's `sub`
    email:      &str,
    first_name:       Option<&str>,
) -> Result<uuid::Uuid, sqlx::Error> {
    
    // ── Case 1: OAuth account already linked → just return the user_id
    if let Some(row) = sqlx::query!(
        "SELECT user_id FROM oauth_accounts
         WHERE provider = 'google' AND provider_user_id = $1",
        provider_user_id
    )
    .fetch_optional(pool)
    .await?
    {
        return Ok(row.user_id);
    }

    // ── Case 2: Email exists (user has a password account) → link OAuth to it
    if let Some(row) = sqlx::query!("SELECT id FROM users WHERE email = $1", email)
        .fetch_optional(pool)
        .await?
    {
        sqlx::query!(
            "INSERT INTO oauth_accounts (user_id, provider, provider_user_id, provider_email)
             VALUES ($1, 'google', $2, $3)",
            row.id, provider_user_id, email
        )
        .execute(pool)
        .await?;

        return Ok(row.id);
    }
    // Brand new user — create user row + oauth_account in one transaction
    let mut tx = pool.begin().await?;

    let user_id = sqlx::query_scalar!(
        "INSERT INTO users (email, first_name) VALUES ($1, $2) RETURNING id",
        email, first_name
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO oauth_accounts (user_id, provider, provider_user_id, provider_email)
         VALUES ($1, 'google', $2, $3)",
        user_id, provider_user_id, email
    )
    .execute(&mut *tx)
    .await?;

    Ok(user_id)
}