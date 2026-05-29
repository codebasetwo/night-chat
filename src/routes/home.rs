use actix_web::{ web, Responder, HttpResponse };
use actix_web::http::header::ContentType;

pub async fn greet(name: web::Path<String>) -> impl Responder {
    format!("Welcome {} to your favorite Chat App", name)   
}

pub async fn home() -> HttpResponse {
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(include_str!("home.html"))
}