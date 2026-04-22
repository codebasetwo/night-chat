use std::net::TcpListener;
use actix_web;
use sqlx::postgres::PgPoolOptions;
use chat_app::server::Application;
use chat_app::configuration::get_configuration;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let settings = get_configuration().expect("Failed to read configurations");
    let app = Application::build(settings.clone()).await?;
    
}
