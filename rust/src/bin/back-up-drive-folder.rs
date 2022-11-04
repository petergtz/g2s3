extern crate core;

use aws_config::meta::region::RegionProviderChain;
use clap::Parser;
use error_chain::ChainedError;
use google_drive3::oauth2::read_authorized_user_secret;
use log::{error, info};
use std::sync::Arc;

use google_backup_to_s3::cli_factories::{create_aus_from_env_vars, set_up_logging};
use google_backup_to_s3::{back_up, drive, errors::Result};

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
    source: String,

    /// Where to copy the files. This must be in the format s3://bucket-name/some/folder
    /// Can also use {date} which will get substituted by the current date.
    #[arg()]
    destination: String,
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
    let s3 = Arc::new(aws_sdk_s3::Client::new(
        &aws_config::from_env().region(RegionProviderChain::default_provider()).load().await,
    ));

    let result = back_up(
        drive,
        s3,
        args.source.as_str(),
        substitute_date(&args.destination).as_str(),
        args.s3_storage_class.as_str(),
    )
    .await;
    if let Err(ref e) = result {
        error!("{}", e.display_chain());
        ::std::process::exit(1);
    }
    Ok(())
}

fn substitute_date(templated_string: &String) -> String {
    templated_string.replace("{date}", chrono::Local::now().format("%Y-%m-%d").to_string().as_str())
}

#[cfg(test)]
mod tests {
    use crate::substitute_date;

    #[test]
    fn it_works() {
        assert_eq!(substitute_date(&String::from("/some/{date}/path")), "/some/2022-11-04/path");
    }
}
