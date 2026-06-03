extern crate core;

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use aws_sdk_s3::Client;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, StorageClass};
use byte_unit::{Byte, ByteUnit::B};
use futures::{stream, StreamExt};
use google_drive3::hyper::body::HttpBody;
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

async fn upload_part(
    s3: &Client,
    bucket: &str,
    key: &str,
    upload_id: &str,
    part_number: i32,
    data: Vec<u8>,
) -> Result<CompletedPart> {
    let resp = s3
        .upload_part()
        .bucket(bucket)
        .key(key)
        .upload_id(upload_id)
        .part_number(part_number)
        .body(aws_sdk_s3::primitives::ByteStream::from(data))
        .send()
        .await
        .chain_err(|| format!("upload_part failed for part {part_number}"))?;

    Ok(CompletedPart::builder()
        .part_number(part_number)
        .e_tag(resp.e_tag().unwrap_or_default())
        .build())
}

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
    let key = folder_name.join(filename).to_str().unwrap().to_string();
    let sc: StorageClass = storage_class.into();

    let response = drive.get_content_for(&file).await?;
    let mut body = response.into_body();

    // Use multipart upload: stream in chunks of 64MB
    const PART_SIZE: usize = 64 * 1024 * 1024;

    let create_resp = s3
        .create_multipart_upload()
        .bucket(&bucket_name)
        .key(&key)
        .storage_class(sc)
        .send()
        .await
        .chain_err(|| format!("Could not create multipart upload for {filename}"))?;

    let upload_id = create_resp.upload_id().unwrap().to_string();
    let mut parts: Vec<CompletedPart> = Vec::new();
    let mut part_number: i32 = 1;
    let mut buf = Vec::with_capacity(PART_SIZE);

    loop {
        match body.data().await {
            Some(Ok(chunk)) => {
                buf.extend_from_slice(&chunk);
                if buf.len() >= PART_SIZE {
                    let part = upload_part(s3, &bucket_name, &key, &upload_id, part_number, std::mem::replace(&mut buf, Vec::with_capacity(PART_SIZE))).await;
                    if let Err(e) = part {
                        let _ = s3.abort_multipart_upload()
                            .bucket(&bucket_name).key(&key).upload_id(&upload_id)
                            .send().await;
                        return Err(e).chain_err(|| format!("Could not upload part {part_number} for {filename}"));
                    }
                    parts.push(part.unwrap());
                    part_number += 1;
                }
            }
            Some(Err(e)) => {
                let _ = s3.abort_multipart_upload()
                    .bucket(&bucket_name).key(&key).upload_id(&upload_id)
                    .send().await;
                return Err(errors::Error::from(format!("Download error for {filename}: {e}")));
            }
            None => break,
        }
    }

    // Upload remaining data
    if !buf.is_empty() || parts.is_empty() {
        let part = upload_part(s3, &bucket_name, &key, &upload_id, part_number, buf).await;
        if let Err(e) = part {
            let _ = s3.abort_multipart_upload()
                .bucket(&bucket_name).key(&key).upload_id(&upload_id)
                .send().await;
            return Err(e).chain_err(|| format!("Could not upload final part for {filename}"));
        }
        parts.push(part.unwrap());
    }

    // Complete the multipart upload
    s3.complete_multipart_upload()
        .bucket(&bucket_name)
        .key(&key)
        .upload_id(&upload_id)
        .multipart_upload(
            CompletedMultipartUpload::builder()
                .set_parts(Some(parts))
                .build(),
        )
        .send()
        .await
        .chain_err(|| format!("Could not complete multipart upload for {filename}"))?;

    log::info!("Uploaded {filename} in {} parts", part_number);

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

        println!(
            "{}",
            format!(
                r#"{{
                "_aws": {{
                    "Timestamp": "{}",
                    "CloudWatchMetrics": [
                    {{
                        "Namespace": "google-backup",
                        "Metrics": [
                        {{
                            "Name": "copy_duration",
                            "Unit": "Milliseconds"
                        }}
                        ]
                    }}
                    ]
                }},
                "copy_duration": "{}",
            }}"#,
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis().to_string(),
                start_time.elapsed().as_millis()
            )
            .replace("\n", "")
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
