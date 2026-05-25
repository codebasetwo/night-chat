use actix_web::{
    body::MessageBody,
    dev::{ ServiceRequest, ServiceResponse },
    middleware::{Next},
};
use crate::utils::{Token};
use sqlx::PgPool;
use crate::utils::user_utils::get_user_from_token;
use actix_web::HttpMessage;

pub async fn socket_auth_middleware<B>(
    req: ServiceRequest, 
    next: Next<B>,
) -> Result<ServiceResponse<B>, actix_web::Error>
where B: MessageBody,
{
    // Expect token as query parameter: ?token=...
    let query = req.query_string();
    let mut token = String::from("");
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            if key == "token" && !value.is_empty() {
                token = value.to_string()
            }
        }
    }

    let hash_token = Token::hash_token(&token);
    let hash_token = String::from_utf8(hash_token)
        .map_err(|_| actix_web::error::ErrorInternalServerError("Token encoding error"))?;

    let pool = req.app_data::<actix_web::web::Data<PgPool>>()
        .ok_or_else(
            || actix_web::error::ErrorInternalServerError("database pool not found.")
        )?;
    let user = get_user_from_token(pool.get_ref(), &hash_token, "authentication")
        .await
        .map_err(|_| actix_web::error::ErrorUnauthorized("Invalid token"))?
        .ok_or_else(|| actix_web::error::ErrorUnauthorized("Invalid token"))?;

    req.extensions_mut().insert(user);
    next.call(req).await
}