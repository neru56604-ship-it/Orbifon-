use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub port: u16,
    pub upload_dir: String,
    pub max_image_bytes: usize,
}

impl Config {
    /// Loads config from environment (via `.env` in dev, real env vars in prod).
    /// Panics on startup if anything required is missing — fail fast, not at 2am in prod.
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let database_url = env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set (see .env.example)");
        let jwt_secret = env::var("JWT_SECRET")
            .expect("JWT_SECRET must be set (see .env.example)");

        if jwt_secret.len() < 32 {
            panic!("JWT_SECRET must be at least 32 characters for adequate signing security");
        }

        let port: u16 = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .expect("PORT must be a valid u16");

        let upload_dir = env::var("UPLOAD_DIR").unwrap_or_else(|_| "./uploads".to_string());

        let max_image_bytes: usize = env::var("MAX_IMAGE_BYTES")
            .unwrap_or_else(|_| "5242880".to_string())
            .parse()
            .expect("MAX_IMAGE_BYTES must be a valid number");

        Config {
            database_url,
            jwt_secret,
            port,
            upload_dir,
            max_image_bytes,
        }
    }
}
