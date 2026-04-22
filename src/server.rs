use std::net::TcpListener;
use actix_web::{ dev, web, App, HttpServer };
use sqlx::postgres::{ PgPoolOptions, PgPool };
use crate::routes::{ home, greet };
use crate::configuration::DatabaseSettings;
use crate::configuration::Settings;


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
) -> Result<dev::Server, std::io::Error>{
    let server = HttpServer::new(|| {
    let db_pool =web::Data::new(db_pool);
        App::new()
            .route("/", web::get().to(home))
            .route("/{name}", web::get().to(greet))
            .app_data(db_pool.clone())
    })
    .listen(tcp_listener)?
    .run();
    Ok(server)
}