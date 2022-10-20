extern crate core;
extern crate google_drive3 as google_drive;

use error_chain::ChainedError;
use google_drive::oauth2::read_authorized_user_secret;
use log::{error, info};

use google_backup_to_s3::{back_up, create_aus_from_env_vars, set_up_logging};
use google_backup_to_s3::errors;

use errors::Result;

#[tokio::main]
async fn main() -> Result<()> {
    set_up_logging();
    info!("Starting");

    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 3);

    let drive_folder_name = &args[1];
    let bucket_name = &args[2];

    let authorized_user_secret = match create_aus_from_env_vars() {
        Ok(authorized_user_secret) => authorized_user_secret,
        Err(_) => read_authorized_user_secret("private/authorized_user_secret.json").await.unwrap()
    };
    let result = back_up(authorized_user_secret, drive_folder_name, bucket_name).await;
    if let Err(ref e) = result {
        error!("{}", e.display_chain());
        ::std::process::exit(1);
    }
    Ok(())
}

