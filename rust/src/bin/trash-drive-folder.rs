extern crate core;

use clap::Parser;
use error_chain::ChainedError;
use google_drive3::oauth2::read_authorized_user_secret;
use log::{error, info};
use std::path::Path;
use std::sync::Arc;

use google_backup_to_s3::cli_factories::{create_aus_from_env_vars, set_up_logging};
use google_backup_to_s3::drive::Drive;
use google_backup_to_s3::{drive, errors::Result};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The Google Drive folder to move to trash
    #[arg()]
    folder: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    set_up_logging();
    info!("Starting");

    let authorized_user_secret = match create_aus_from_env_vars() {
        Ok(authorized_user_secret) => authorized_user_secret,
        Err(_) => read_authorized_user_secret("private/authorized_user_secret.json").await.unwrap(),
    };
    let drive = Arc::new(drive::Drive::new(drive::create_drive_hub(authorized_user_secret).await));

    let result = move_to_trash(drive, args.folder.as_str()).await;
    if let Err(ref e) = result {
        error!("{}", e.display_chain());
        ::std::process::exit(1);
    }
    Ok(())
}

async fn move_to_trash(drive: Arc<Drive>, folder: &str) -> Result<()> {
    drive.trash_file(&drive.get_file_from(&Path::new(folder)).await?.id.as_ref().unwrap()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn path_can_be_iterated() {
        let mut parts = vec![];
        for part in Path::new("/some//path/in/here/") {
            parts.push(part);
        }
        assert_eq!(parts, vec!["/", "some", "path", "in", "here"]);
    }
}
