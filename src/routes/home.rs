use actix_web::{ web, Responder, };

pub async fn greet(name: web::Path<String>) -> impl Responder {
    format!("Welcome {} to your favorite Chat App", name)   
}

pub async fn home() -> impl Responder {
    format!("Welcome to Chat App")
}

