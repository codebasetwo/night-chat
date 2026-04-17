use std::net::TcpListener;
use actix_web;
use chat_app::server::run_server;
use chat_app::configuration::get_configuration;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let settings = get_configuration().expect("Failed to read configurations");
    let port = settings.application.port;
    let host = settings.application.host;

    let address = format!{
        "{}:{}",
        host,
        port,
    };
    let tcp_listener = TcpListener::bind(address)?;
    run_server(tcp_listener)?.await
}
