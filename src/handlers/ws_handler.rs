use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use actix_ws::{Session, AggregatedMessage};
use actix_web::{web, HttpRequest, HttpMessage, HttpResponse};
use crate::utils::User;
use futures_util::StreamExt as _;

pub type WsSessions = Arc<RwLock<HashMap<uuid::Uuid, Session>>>;

#[tracing::instrument(name = "WebSocket Connection Handler", skip(req, body, sessions))]
pub async fn ws_handler(
    req: HttpRequest,
    body: web::Payload,
    sessions: web::Data<WsSessions>,
) -> Result<HttpResponse, actix_web::Error> {

    let user_id: uuid::Uuid = req.extensions_mut()
        .get::<User>()
        .map(|user| user.id)
        .ok_or_else(|| actix_web::error::ErrorUnauthorized("Unauthorized access"))?;

    // Upgrade the HTTP request to a WebSocket connection
    // Handshake
    let (response, mut session, stream) = actix_ws::handle(&req, body)?;

    let mut stream = stream
        .aggregate_continuations()
        .max_continuation_size(1_usize << 20);

    if let Ok(mut map) = sessions.get_ref().write() {
        map.insert(user_id, session.clone());
    }
    let sessions_clone = Arc::clone(&sessions);

    // Spawn a detached background task to keep the connection alive
    actix_web::rt::spawn(async move {
        // Process incoming control messages (like Ping/Close)
        // Text/Bin and binary are from http end point.
        while let Some(msg) = stream.next().await {
            match msg {
                Ok(AggregatedMessage::Ping(msg)) => {
                    session.pong(&msg).await.unwrap();
                }
                Ok(AggregatedMessage::Close(reason)) => {
                    session.close(reason).await.ok();
                    break;
                }
                // (Optional) Handle incoming text/binary if you want bidirectional WebSockets later
                _ => {}
            }
        }
        // Remove the user from the active sessions when they disconnect
        if let Ok(mut map) = sessions_clone.write() {
            map.remove(&user_id);
        }
    });

    Ok(response)
}

