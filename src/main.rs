use actix_web;
use chat_app::server::Application;
use chat_app::configuration::get_configuration;
use chat_app::utils::{get_subscriber, init_subscriber};

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    // logging initializations
    let subscriber = get_subscriber("chat_app".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    // app setting configs
    let settings = get_configuration().expect("Failed to read configurations");
    let _app = Application::build(settings.clone()).await?;

    Ok(())
}
