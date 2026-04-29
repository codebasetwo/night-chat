use actix_web::{
    HttpResponse,
    Responder, 
};

// get all contacts // find all user that is not equal to the logged in user without password
pub async fn get_all_contacts() -> impl Responder {
    HttpResponse::Ok().finish()
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
