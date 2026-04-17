use config::{ self, Config };
use serde;

#[derive(serde::Deserialize)]
pub struct Settings {
    pub application: AppSettings,
}

#[derive(serde::Deserialize)]
pub struct AppSettings {
    pub port: u16,
    pub host: String,
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine base path");
    let directory = base_path.join("configurations");

    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");

    let settings = Config::builder()
        .add_source(config::File::from(directory.join("base")).required(true))
        .add_source(config::File::from(directory.join(environment.as_str())).required(true))
        .build()?;

    settings.try_deserialize()
}


pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production"
        }
    } 
}

impl TryFrom<String> for Environment {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}

