use actix_web::{
    web,
    http,
    HttpResponse,
    Responder,
    HttpRequest,
    ResponseError,
    HttpMessage,
};
use sqlx:: {
    PgPool,
};
use crate::utils::{ UserSummary, User, get_all_users};

#[derive(Debug)]
pub struct DbError(anyhow::Error);

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "error occured while processing request."
        )
    }
}

impl From<anyhow::Error> for DbError {
    fn from(error: anyhow::Error) -> Self {
        Self(error)
    }
}

impl ResponseError for DbError {
    fn status_code(&self) -> http::StatusCode {
        http::StatusCode::INTERNAL_SERVER_ERROR
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .body(self.0.to_string())
    }
}

// get all contacts // find all user that is not equal to the logged in user without password
pub async fn get_all_contacts(req: HttpRequest) -> Result<HttpResponse, DbError> {
    // get logged in user data from request extensions
    let logged_in_user_id: uuid::Uuid = req.extensions_mut()
        .get::<User>()
        .map(|user| user.id)
        .unwrap();
    // get the pool from request data
    let pool = req.app_data::<web::Data<PgPool>>()
        .ok_or_else(|| anyhow::anyhow!("database connection pool not found"))?;
    // get all users from db where id id not equal to loged in user id without the password field
    let filtered_users: Vec<UserSummary> = get_all_users(&pool, logged_in_user_id).await?;

    Ok(HttpResponse::Ok().json(filtered_users))
}

// get messages by user id

pub async fn get_messages_by_user_id() -> impl Responder {
    HttpResponse::Ok().finish()
}

// send message to user id use socket io

pub async fn send_message() -> impl Responder {
    HttpResponse::Ok().finish()
}

// get chat patners where logged in user is either receiver or sender

pub async fn get_chat_partners() -> impl Responder {
    HttpResponse::Ok().finish()
}
