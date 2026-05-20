use std::net::TcpListener;
use actix_web::{ dev, middleware::from_fn, web, App, HttpServer };
use sqlx::postgres::{ PgPoolOptions, PgPool };
use crate::routes::{ home, greet, signup, login };
use crate::configuration::DatabaseSettings;
use crate::configuration::Settings;
use crate::handlers::message::{
    get_all_contacts, 
    get_messages_by_user_id, 
    send_message, 
    get_chat_partners
};
use crate::middlewares::auth_middleware;

pub struct Application {
    port: u16,
    server: dev::Server,
}

impl Application {
 pub async fn build(configuration: Settings) -> Result<Self, anyhow::Error> {
        let connection_pool = get_connection_pool(&configuration.database);

        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run_server(
            listener,
            connection_pool,
        )
        .await?;

        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub fn get_connection_pool(database_config: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new().connect_lazy_with(database_config.connect_options())
}

async fn run_server(
    tcp_listener: TcpListener,
    db_pool: PgPool,
) -> Result<dev::Server, anyhow::Error>{
    let db_pool =web::Data::new(db_pool);
    let server = HttpServer::new(move || {
        App::new()
            .route("/", web::get().to(home))
            .route("/signup", web::post().to(signup))
            .route("/login", web::post().to(login))
            // create routes for message prefix /messages without duplicating the prefix
            .service(
                web::scope("/messages")
                    // .wrap(from_fn(rate_limit_middleware))
                    // .wrap(from_fn(auth_middleware))
                    .route("/contacts", web::get().to(get_all_contacts))
                    .route("/chats", web::get().to(get_chat_partners))
                    .route("/{user_id}", web::get().to(get_messages_by_user_id))
                    .route("/send/{id}", web::post().to(send_message))
                    .wrap(from_fn(auth_middleware))
            )
            // test route
            .route("/{name}", web::get().to(greet))
            .app_data(db_pool.clone())
    })
    .listen(tcp_listener)?
    .run();
    Ok(server)
}