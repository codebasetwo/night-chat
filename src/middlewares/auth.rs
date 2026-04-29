use actix_web::{
    HttpResponse, ResponseError,
    body::MessageBody,
    dev::{ ServiceRequest, ServiceResponse },
    middleware::{ Next},
};
use crate::utils::{ Token};
use sqlx::PgPool;
use crate::utils::user_utils::get_user_from_token;
use actix_web::HttpMessage;

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("invalid or missing authentication token")]
    Unauthorized,
    #[error("unexpected error: {0}")]
    Unexpected(String),
}

impl From<sqlx::Error> for AuthError {
    fn from(err: sqlx::Error) -> Self {
        AuthError::Unexpected(err.to_string())
    }
}

impl ResponseError for AuthError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AuthError::Unauthorized => {
                let mut response = HttpResponse::Unauthorized();
                response.insert_header(("WWW-Authenticate", "Bearer"));
                response.finish()
            }
            AuthError::Unexpected(_) => HttpResponse::InternalServerError().finish(),
        }
    }
}

pub async fn auth_middleware<B>(
    req: ServiceRequest, 
    next: Next<B>,
) -> Result<ServiceResponse<B>, actix_web::Error>
where B: MessageBody,
{
    // check authorization header
    // if authorization header value is an empty string, return unauthorized
    // if authorization header value is not an empty string, check if it contains
    // Bearer token Bearer <token>, if it does split it with space and get the token, if it does not.
    if let Some(authorization_header) = req.headers().get("Authorization") {
        if let Ok(authorization_str) = authorization_header.to_str() {
            if authorization_str.starts_with("Bearer ") {
                let token = authorization_str.trim_start_matches("Bearer ")
                    .trim();
                if token.is_empty() {
                    // return unuthorized if token is empty
                    return Err(AuthError::Unauthorized.into());
                }
                let pool = req.app_data::<actix_web::web::Data<PgPool>>()
                    .ok_or_else(
                        || AuthError::Unexpected("database connection pool not found".into())
                    )?;
                // validate the token here, if valid continue to next middleware, if not return unauthorized
                let hash_token = Token::hash_token(token);
                let hash_token = match String::from_utf8(hash_token) {
                    Ok(safe_string) => safe_string,
                    Err(e) => return Err(AuthError::Unexpected(e.to_string()).into()),
                };
                let user_option = get_user_from_token(&pool, &hash_token, "authentication")
                    .await
                    .map_err(|e| AuthError::Unexpected(e.to_string()))?;
                    
                // 3. Handle the Option
                if let Some(user) = user_option {
                    req.extensions_mut().insert(user);
                    next.call(req).await//.map_err(|e| AuthError::Unexpected(e.to_string()))
                } else {
                    Err(AuthError::Unauthorized.into())
                };
            }
        }
        return Err(AuthError::Unauthorized.into());
    }
    Err(AuthError::Unauthorized.into())
}