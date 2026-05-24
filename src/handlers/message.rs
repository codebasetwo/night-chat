use actix_web::{
    web,
    http,
    HttpResponse,
    HttpRequest,
    ResponseError,
    HttpMessage,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    PgPool,
};
use crate::utils::{UserSummary, User, get_all_users};

// Message struct representing a message in the database
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Message {
    pub id: uuid::Uuid,
    pub sender_id: uuid::Uuid,
    pub receiver_id: uuid::Uuid,
    pub text: Option<String>,
    pub image: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// Request payload for sending a message
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub text: Option<String>,
    pub image: Option<String>,
}

#[derive(thiserror::Error, Debug)]
#[error("error occured while processing request.")]
pub struct DbError(anyhow::Error);


impl From<anyhow::Error> for DbError {
    fn from(error: anyhow::Error) -> Self {
        Self(error)
    }
}

impl From<sqlx::Error> for DbError {
    fn from(error: sqlx::Error) -> Self {
        Self(anyhow::anyhow!(error))
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
#[tracing::instrument(
    name = "Get all contacts",
    skip(req),
)]
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

// Get messages between two users (logged-in user and another user)
#[tracing::instrument(
    name = "Getting messages between users",
    skip(req, user_id),
    fields(
        user_id = %user_id
    )
)]
pub async fn get_messages_by_user_id(
    req: HttpRequest,
    user_id: web::Path<uuid::Uuid>,
) -> Result<HttpResponse, DbError> {
    let logged_in_user_id: uuid::Uuid = req.extensions_mut()
        .get::<User>()
        .map(|user| user.id)
        .unwrap();
    
    let user_to_chat_id = user_id.into_inner();

    let pool = req.app_data::<web::Data<PgPool>>()
        .ok_or_else(|| anyhow::anyhow!("database connection pool not found"))?;

    // Query messages where logged-in user is either sender or receiver
    let messages = sqlx::query_as::<_, Message>(
        r#"
            SELECT id, sender_id, receiver_id, text, image, created_at
            FROM messages
            WHERE (sender_id = $1 AND receiver_id = $2)
               OR (sender_id = $2 AND receiver_id = $1)
            ORDER BY created_at ASC
        "#,
    )
    .bind(logged_in_user_id)
    .bind(user_to_chat_id)
    .fetch_all(pool.get_ref())
    .await?;

    Ok(HttpResponse::Ok().json(messages))
}

#[tracing::instrument(
    name = "Get all contacts",
    skip(req, receiver_id, payload),
)]
// Send a message to another user
pub async fn send_message(
    req: HttpRequest,
    receiver_id: web::Path<uuid::Uuid>,
    payload: web::Json<SendMessageRequest>,
) -> Result<HttpResponse, DbError> {
    let sender_id: uuid::Uuid = req.extensions_mut()
        .get::<User>()
        .map(|user| user.id)
        .unwrap();

    let receiver_id = receiver_id.into_inner();

    // Validate that text or image is provided
    if payload.text.is_none() && payload.image.is_none() {
        return Err(DbError(anyhow::anyhow!("Text or image is required")));
    }

    // Validate that user is not sending message to themselves
    if sender_id == receiver_id {
        return Err(DbError(anyhow::anyhow!("Cannot send messages to yourself")));
    }

    let pool = req.app_data::<web::Data<PgPool>>()
        .ok_or_else(|| anyhow::anyhow!("database connection pool not found"))?;

    // Check if receiver exists
    let receiver_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(receiver_id)
        .fetch_one(pool.get_ref())
        .await?;

    if !receiver_exists {
        return Err(DbError(anyhow::anyhow!("Receiver not found")));
    }

    // Insert message into database
    let message_id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now();

    let new_message = sqlx::query_as::<_, Message>(
        r#"
            INSERT INTO messages (id, sender_id, receiver_id, text, image, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, sender_id, receiver_id, text, image, created_at
        "#,
    )
    .bind(message_id)
    .bind(sender_id)
    .bind(receiver_id)
    .bind(&payload.text)
    .bind(&payload.image)
    .bind(now)
    .fetch_one(pool.get_ref())
    .await?;

    // TODO: Send message in real-time via WebSocket

    Ok(HttpResponse::Created().json(new_message))
}


// Get all chat partners (users who have exchanged messages with logged-in user)
#[tracing::instrument(
    name = "Get all chat partners for current user.",
    skip(req),
)]
pub async fn get_chat_partners(req: HttpRequest) -> Result<HttpResponse, DbError> {
    let logged_in_user_id: uuid::Uuid = req.extensions_mut()
        .get::<User>()
        .map(|user| user.id)
        .unwrap();
    
    let pool = req.app_data::<web::Data<PgPool>>()
        .ok_or_else(|| anyhow::anyhow!("database connection pool not found"))?;

    // Get all messages where logged-in user is sender or receiver
    let messages = sqlx::query!(
        r#"
            SELECT sender_id, receiver_id FROM messages
            WHERE sender_id = $1 OR receiver_id = $1
        "#,
        logged_in_user_id,
    )
    .fetch_all(pool.get_ref())
    .await?;

    // Extract unique chat partner IDs
    let mut chat_partner_ids: Vec<uuid::Uuid> = messages
        .iter()
        .map(|msg| {
            if msg.sender_id == logged_in_user_id {
                msg.receiver_id
            } else {
                msg.sender_id
            }
        })
        .collect();

    // Remove duplicates
    chat_partner_ids.sort();
    chat_partner_ids.dedup();

    // Get user details for all chat partners
    let chat_partners: Vec<UserSummary> = if !chat_partner_ids.is_empty() {
        sqlx::query_as::<_, UserSummary>(
            r#"
                SELECT id, first_name, email FROM users
                WHERE id = ANY($1)
            "#,
        )
        .bind(chat_partner_ids)
        .fetch_all(pool.get_ref())
        .await?
    } else {
        vec![]
    };

    Ok(HttpResponse::Ok().json(chat_partners))
}
