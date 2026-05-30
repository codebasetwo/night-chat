use actix_web::{HttpResponse, HttpRequest, ResponseError, 
    http::{header, StatusCode}, 
    web,
};
use sqlx::{PgPool};
use crate::utils::{Token, User};

#[derive(thiserror::Error, Debug)]
pub enum LogoutError{
    #[error("Not authenticated")]
    UserNotFound,
    #[error("Missing or invalid token")]
    InvalidToken,
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}

impl ResponseError for LogoutError{
    fn status_code(&self) -> StatusCode{
        match self {
            LogoutError::UserNotFound     => StatusCode::UNAUTHORIZED,
            LogoutError::InvalidToken     => StatusCode::UNAUTHORIZED,
            LogoutError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

pub async fn logout(
    user: Option<web::ReqData<User>>,
    pool: web::Data<PgPool>,
    req: HttpRequest,
) -> Result<HttpResponse, LogoutError> {
    let user = user.ok_or(LogoutError::UserNotFound)?;

    let token = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(LogoutError::InvalidToken)?;

    // Check against the database for the currrent token
    // single session authentication.
    let hash = Token::hash_token(token);

    sqlx::query!(
        r#"
            DELETE FROM tokens WHERE user_id = $1 AND hash = $2 AND scope = 'authentication'
        "#,
        user.id,
        hash,

    )
    .execute(pool.get_ref())   
    .await?; 
    Ok(
        HttpResponse::SeeOther()
        .insert_header((header::LOCATION, "/v1/api"))
        .finish()
    )
}