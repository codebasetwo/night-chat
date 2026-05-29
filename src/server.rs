use std::net::TcpListener;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use std::net::IpAddr;
use std::str::FromStr;
use actix_web::{ dev, middleware::from_fn, web, App, HttpServer };
use actix_governor::{Governor, GovernorConfigBuilder, };
use sqlx::postgres::{ PgPoolOptions, PgPool };
use secrecy::SecretString;
use tracing_actix_web::TracingLogger;
use crate::configuration::DatabaseSettings;
use crate::configuration::Settings;
use crate::handlers::message::{
    get_all_contacts, 
    get_messages_by_user_id, 
    send_message, 
    get_chat_partners,
};
use crate::handlers::ws_handler::{WsSessions, ws_handler};
use crate::middlewares::{auth_middleware, socket_auth_middleware, RealIpKeyExtractor};
use crate::oauth::oauth;
use crate::routes::{home, login, logout, password, signup};

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
        let host = configuration.application.host;
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run_server(
            listener,
            connection_pool,
            host,
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
    host: String,
) -> Result<dev::Server, anyhow::Error>{
    dotenvy::dotenv()?;

    let google_app_config = oauth::AppConfig{
        google_client_id: std::env::var("GOOGLE_CLIENT_ID").expect("Please provide your google client ID"),
        google_client_secret: SecretString::new(std::env::var("GOOGLE_CLIENT_SECRET").expect("Please provide google client secret")),//.expect("Please provide yout google client secret"),
        redirect_uri: std::env::var("REDIRECT_URI").expect("Please provide google redirect uri"),
    };

    let trusted_reverse_proxy_ip = IpAddr::from_str(&host).unwrap();
    let trusted_reverse_proxy_ip = web::Data::new(trusted_reverse_proxy_ip);
    let db_pool =web::Data::new(db_pool);
    // Initialize the real-time session tracker
    let ws_sessions: WsSessions = Arc::new(RwLock::new(HashMap::new()));
    let ws_sessions_data = web::Data::new(ws_sessions);

    // Allow bursts with up to five requests per IP address
    // and replenishes one element every two seconds
    let governor_conf = GovernorConfigBuilder::default()
        .seconds_per_request(2)
        .burst_size(5)
        .key_extractor(RealIpKeyExtractor)
        .finish()
        .unwrap();

    let server = HttpServer::new(move || {
        App::new()
            .app_data(google_app_config.clone())
            .app_data(trusted_reverse_proxy_ip.clone())
            .app_data(db_pool.clone())
            .app_data(ws_sessions_data.clone())
            .wrap(TracingLogger::default())
            .wrap(Governor::new(&governor_conf))
            .service(
                web::scope("/v1/api")
                .route("/", web::get().to(home::home))
                .route("/signup", web::post().to(signup::signup))
                .route("/login", web::post().to(login::login))
                .route("/google_login", web::post().to(oauth::oauth))
                .route("/oauth/callback", web::post().to(oauth::redirect_uri))
                .route("/{name}", web::get().to(home::greet))
            )
            
            // create routes for message prefix /messages without duplicating the prefix
            .service(
                web::scope("/v1/api/messages")
                    .wrap(from_fn(auth_middleware))
                    .route("/contacts", web::get().to(get_all_contacts))
                    .route("/chats", web::get().to(get_chat_partners))
                    .route("/{user_id}", web::get().to(get_messages_by_user_id))
                    .route("/send/{id}", web::post().to(send_message))
            )
            .service(
                web::scope("/v1/api/account")
                    .wrap(from_fn(auth_middleware))
                    .route("/password/request_reset", web::post().to(password::handle_password_reset))
                    .route("/password/set_password", web::post().to(password::set_password_account))
                    .route("/logout", web::post().to(logout::logout))
            )
            // WebSocket endpoint — also auth-protected
            .service(
                web::scope("/ws")
                    .route("", web::get().to(ws_handler))
                    .wrap(from_fn(socket_auth_middleware))
            )
            
    })
    .listen(tcp_listener)?
    .run();
    Ok(server)
}