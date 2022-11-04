extern crate core;

use std::sync::Arc;
use std::time::Instant;

use aws_sdk_s3::Client;
use aws_smithy_http::{body::SdkBody, byte_stream::ByteStream};
use byte_unit::{Byte, ByteUnit::B};
use futures::{stream, StreamExt};
use std::path::PathBuf;
use tokio::sync::mpsc;
use url::Url;

use crate::drive::Drive;
use errors::{Result, ResultExt};

pub mod cli_factories;
pub mod drive;
pub mod errors;

pub async fn back_up(
    drive: Arc<drive::Drive>,
    s3: Arc<aws_sdk_s3::Client>,
    source: &str,
    destination: &str,
    s3_storage_class: &str,
) -> Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel();

    stream::iter(drive.list_files_in_folder(&String::from(source)).await?)
        .for_each_concurrent(4, |file| {
            let (drive, s3, tx) = (drive.clone(), s3.clone(), tx.clone());
            async move {
                let result =
                    copy_file(&*drive, &*s3, file.clone(), destination, s3_storage_class).await;
                tx.send(result).unwrap();
            }
        })
        .await;

    rx.close();

    let mut result = Ok(());
    while let Some(r) = rx.recv().await {
        match r {
            Ok(_) => {}
            Err(e) => {
                log::error!("Error during copy: {}", e);
                result = Err(e);
            }
        }
    }
    result
}

trait Run
where
    Self: Sized,
{
    fn run<F, R>(self, func: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        func(self)
    }
}

impl<T> Run for T where T: Sized {}

async fn copy_file(
    drive: &Drive,
    s3: &Client,
    file: google_drive3::api::File,
    destination: &str,
    storage_class: &str,
) -> Result<()> {
    let filename = file.name.as_ref().unwrap();
    log::info!("Copying file {filename} (mime type: {})", file.mime_type.as_ref().unwrap());
    let start_time = Instant::now();

    let (bucket_name, folder_name) =
        parse_s3_url(destination).chain_err(|| format!("Could not parse S3 URL {destination}."))?;
    s3.put_object()
        .bucket(bucket_name)
        .key(folder_name.join(filename).to_str().unwrap())
        .run(|r| match file.md5_checksum.as_ref() {
            Some(md5_checksum) => r.content_md5(base64::encode(hex::decode(md5_checksum).unwrap())),
            None => r,
        })
        // .content_md5(base64::encode(hex::decode(file.md5_checksum.as_ref().unwrap()).unwrap()))
        .body(ByteStream::new(SdkBody::from(drive.get_content_for(&file).await?.into_body())))
        .storage_class(storage_class.into())
        .send()
        .await
        .chain_err(|| format!("Could not upload file contents for {filename}"))?;

    if let Some(filesize) = file.size.as_ref() {
        let filesize = Byte::from_str(filesize).unwrap();
        log::info!(
            "Throughput for file {} of size {}: {}/s",
            filename,
            filesize.get_appropriate_unit(false),
            Byte::from_unit(filesize.get_bytes() as f64 / start_time.elapsed().as_secs_f64(), B)
                .unwrap()
                .get_appropriate_unit(false)
        );
    } else {
        log::info!("Throughput for file {filename} unknown due to unknown size");
    }
    Ok(())
}

fn parse_s3_url(u: &str) -> Result<(String, PathBuf)> {
    let u = Url::parse(u).unwrap();
    assert_eq!(u.scheme(), "s3");
    Ok((String::from(u.domain().unwrap()), PathBuf::from(&u.path().trim_start_matches("/"))))
}

#[cfg(test)]
mod tests {
    use crate::parse_s3_url;

    #[test]
    fn parse_s3_url_splits_bucket_name_and_path_correctly() {
        let (bucket, path) = parse_s3_url("s3://mybucket/some/path/").unwrap();

        assert_eq!(bucket, "mybucket");
        assert_eq!(path.to_str().unwrap(), "some/path/");

        let (bucket, path) = parse_s3_url("s3://mybucket").unwrap();

        assert_eq!(bucket, "mybucket");
        assert_eq!(path.to_str().unwrap(), "");

        let (bucket, path) = parse_s3_url("s3://mybucket/").unwrap();

        assert_eq!(bucket, "mybucket");
        assert_eq!(path.to_str().unwrap(), "");
    }
}
