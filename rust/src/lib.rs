extern crate core;

use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use aws_config;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client;
use aws_smithy_http::{body::SdkBody, byte_stream::ByteStream};
use futures::{stream, StreamExt};
use google_drive3::hyper::{client::HttpConnector, Body, Response};
use google_drive3::hyper_rustls::HttpsConnector;
use google_drive3::{api::FileList, hyper, hyper_rustls, DriveHub};
use google_drive3::{oauth2, oauth2::authorized_user::AuthorizedUserSecret};
use tokio::sync::mpsc;

use errors::{Result, ResultExt};

pub mod errors;

pub async fn back_up(
    aus: AuthorizedUserSecret,
    drive_folder_name: &str,
    bucket_name: &str,
    folder_in_bucket: &str,
    s3_storage_class: &str,
) -> Result<()> {
    let hub = Arc::new(create_drive_hub(aus).await);
    let s3 = Arc::new(aws_sdk_s3::Client::new(
        &aws_config::from_env().region(RegionProviderChain::default_provider()).load().await,
    ));

    let files = hub
        .files()
        .list()
        .q(&*format!(
            "name = '{drive_folder_name}' and \
        mimeType = 'application/vnd.google-apps.folder' and \
        'root' in parents"
        ))
        .param("fields", "files(id,name,parents,size)")
        .doit()
        .await
        .chain_err(|| format!("Could not find {drive_folder_name} folder in drive."))?;
    assert_eq!(files.1.files.as_ref().unwrap().len(), 1);
    let folder_id = files.1.files.as_ref().unwrap().first().as_ref().unwrap().id.as_ref().unwrap();

    let mut page_token: Option<String> = None;

    let mut files = vec![];
    loop {
        let file_list_response = list_files_in_folder(&hub, folder_id, &mut page_token)
            .await
            .chain_err(|| format!("Could not list files in {drive_folder_name} folder in drive."))?
            .1;
        for file in file_list_response.files.as_ref().unwrap() {
            files.push(file.clone());
        }
        if file_list_response.next_page_token.is_none() {
            break;
        }
        page_token = file_list_response.next_page_token;
    }
    let (tx, mut rx) = mpsc::unbounded_channel();
    stream::iter(files)
        .for_each_concurrent(4, |file| {
            let (hub, s3) = (hub.clone(), s3.clone());
            let tx = tx.clone();
            async move {
                let result = copy_file(
                    &*hub,
                    &*s3,
                    file.clone(),
                    bucket_name,
                    folder_in_bucket,
                    s3_storage_class,
                )
                .await;
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

async fn list_files_in_folder(
    hub: &DriveHub<HttpsConnector<HttpConnector>>,
    folder_id: &String,
    page_token: &mut Option<String>,
) -> google_drive3::Result<(Response<Body>, FileList)> {
    let mut list_query = hub
        .files()
        .list()
        .q(format!("'{}' in parents", folder_id).as_str())
        .param("fields", "nextPageToken,files(id,name,parents,md5Checksum,size)");
    if page_token.is_some() {
        list_query = list_query.page_token(page_token.as_ref().unwrap().as_str());
    }
    list_query.doit().await
}

async fn create_drive_hub(aus: AuthorizedUserSecret) -> DriveHub<HttpsConnector<HttpConnector>> {
    DriveHub::new(
        hyper::Client::builder().build(
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()
                .https_or_http()
                .enable_http1()
                .enable_http2()
                .build(),
        ),
        oauth2::AuthorizedUserAuthenticator::builder(aus).build().await.unwrap(),
    )
}

pub fn create_aus_from_env_vars() -> Result<AuthorizedUserSecret> {
    Ok(AuthorizedUserSecret {
        client_id: std::env::var("CLIENT_ID")?,
        client_secret: std::env::var("CLIENT_SECRET")?,
        refresh_token: std::env::var("REFRESH_TOKEN")?,
        key_type: "".to_string(),
    })
}

async fn copy_file(
    hub: &DriveHub<HttpsConnector<HttpConnector>>,
    s3: &Client,
    file: google_drive3::api::File,
    bucket_name: &str,
    folder_name: &str,
    storage_class: &str,
) -> Result<()> {
    let filename = file.name.as_ref().unwrap();
    log::info!("Copying file {}", filename);
    let start_time = Instant::now();

    let file_content = hub
        .files()
        .get(file.id.as_ref().unwrap())
        .param("alt", "media")
        .doit()
        .await
        .chain_err(|| format!("Could not download file contents from drive for {filename}."))?
        .0;
    assert!(file_content.status().is_success());
    let _s3_response = s3
        .put_object()
        .bucket(bucket_name)
        .key(Path::new(folder_name).join(filename).to_str().unwrap())
        .content_md5(base64::encode(hex::decode(file.md5_checksum.as_ref().unwrap()).unwrap()))
        .body(ByteStream::new(SdkBody::from(file_content.into_body())))
        .storage_class(storage_class.into())
        .send()
        .await
        .chain_err(|| format!("Could not upload file contents for {filename}"))?;
    log::info!(
        "Throughput for file {} of size {} B: {} B/s",
        filename,
        file.size.as_ref().unwrap(),
        file.size.as_ref().unwrap().parse::<f64>().unwrap() / start_time.elapsed().as_secs() as f64
    );
    Ok(())
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
