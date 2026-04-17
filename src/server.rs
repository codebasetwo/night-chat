use std::net::TcpListener;
use actix_web::{ dev, web, App, HttpServer };

use crate::routes::{ home, greet };

pub fn run_server(
    tcp_listener: TcpListener,
) -> Result<dev::Server, std::io::Error>{
    let server = HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(home))
            .route("/{name}", web::get().to(greet))
    })
    .listen(tcp_listener)?
    .run();
    Ok(server)
}