extern crate core;

use clap::Parser;
use error_chain::ChainedError;
use google_drive3::oauth2::read_authorized_user_secret;
use log::{error, info};

use google_backup_to_s3::{back_up, create_aus_from_env_vars, errors::Result, set_up_logging};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Storage class to use when storing objects in S3.
    /// Possible values: DEEP_ARCHIVE, GLACIER, GLACIER_IR, INTELLIGENT_TIERING,
    ///                  ONEZONE_IA, OUTPOSTS, REDUCED_REDUNDANCY, STANDARD, STANDARD_IA
    #[arg(short, long, default_value_t = String::from("STANDARD"))]
    s3_storage_class: String,

    /// The Google Drive folder to back up
    #[arg()]
    drive_folder: String,

    /// The S3 bucket to which Google Drive files are stored to
    #[arg()]
    s3_bucket: String,

    /// Folder in S3 bucket
    #[arg(long, default_value_t = String::from(""))]
    s3_folder: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    set_up_logging();
    info!("Starting");

    let authorized_user_secret = match create_aus_from_env_vars() {
        Ok(authorized_user_secret) => authorized_user_secret,
        Err(_) => read_authorized_user_secret("private/authorized_user_secret.json").await.unwrap()
    };
    let result = back_up(authorized_user_secret,
                         args.drive_folder.as_str(),
                         args.s3_bucket.as_str(),
                         args.s3_folder.as_str(),
                         args.s3_storage_class.as_str()).await;
    if let Err(ref e) = result {
        error!("{}", e.display_chain());
        ::std::process::exit(1);
    }
    Ok(())
}

