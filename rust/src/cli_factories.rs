use crate::errors::Result;
use core::result::Result::Ok;
use std::io::Write;
use yup_oauth2::authorized_user::AuthorizedUserSecret;

pub fn create_aus_from_env_vars() -> Result<AuthorizedUserSecret> {
    Ok(AuthorizedUserSecret {
        client_id: std::env::var("CLIENT_ID")?,
        client_secret: std::env::var("CLIENT_SECRET")?,
        refresh_token: std::env::var("REFRESH_TOKEN")?,
        key_type: "".to_string(),
    })
}

pub fn set_up_logging() {
    env_logger::Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{}:{} {} [{}] - {}",
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .filter_level(log::LevelFilter::Info)
        .init();
}
