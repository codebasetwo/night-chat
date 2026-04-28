use actix_web::
    http::header::HeaderValue,
    body:;MessageBody,
    {dev::{ ServiceRequest, ServiceResponse },
    middleware::{Middleware, Next},
};

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("invalid or missing authentication token")]
    Unauthorized,
    #[error("unexpected error: {0}")]
    Unexpected(String),
}

impl actix::web::ResponseError for AuthError {
    fn error_response(&self) -> actix::web::HttpReponse {
        match self {
            AuthError::Unauthorized => {
                let mut response = actix::web::HttpResponse::Unauthorized();
                response.insert_header(("WWW-Authenticate", "Bearer"));
                response.finish()
            }
            AuthError::Unexpected(_) => actix::web:HttpResponse::InternalServerError().finish(),
        }
    }
}

async fn auth_middleware(
    req: ServiceRequest, 
    next: Next<impl MessageBody>,
) -> Result<ServcieResponse<impl MessageBody>, AuthError> {
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
                    return Err(AuthError::Unauthorized);
                }
                // validate the token here, if valid continue to next middleware, if not return unauthorized

            }
        } e
    }

    Ok()

}


// validate token function
fn validate_token(token: &str) -> bool {
    
}