use actix_web;
use chat_app::server::Application;
use chat_app::configuration::get_configuration;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let settings = get_configuration().expect("Failed to read configurations");
    let _app = Application::build(settings.clone()).await?;


    Ok(())
    
}
