use crate::errors::Result;
use crate::ResultExt;
use google_drive3::api::File;
use google_drive3::hyper::{client::HttpConnector, Body, Response};
use google_drive3::hyper_rustls::HttpsConnector;
use google_drive3::{api::FileList, hyper, hyper_rustls, DriveHub};
use google_drive3::{oauth2, oauth2::authorized_user::AuthorizedUserSecret};

pub struct Drive {
    hub: DriveHub<HttpsConnector<HttpConnector>>,
}

impl Drive {
    pub fn new(hub: DriveHub<HttpsConnector<HttpConnector>>) -> Drive {
        Drive { hub }
    }

    pub async fn list_files_in_folder(&self, folder: &String) -> Result<Vec<File>> {
        let folder_id = self.folder_id_from_folder_name(folder, "root").await?;

        let mut files = vec![];

        let mut page_token: Option<String> = None;
        loop {
            let file_list_response = self
                .list_files_in_folder_id_per_page(&folder_id, &mut page_token)
                .await
                .chain_err(|| format!("Could not list files in {folder} folder in drive."))?
                .1;
            for file in file_list_response.files.as_ref().unwrap() {
                files.push(file.clone());
            }
            if file_list_response.next_page_token.is_none() {
                break;
            }
            page_token = file_list_response.next_page_token;
        }
        Ok(files)
    }

    pub async fn folder_id_from_folder_name(
        &self,
        folder: &String,
        parent: &str,
    ) -> Result<String> {
        let file_list_response = self
            .hub
            .files()
            .list()
            .q(&*format!(
                "name = '{folder}' and \
                 mimeType = 'application/vnd.google-apps.folder' and \
                 '{parent}' in parents"
            ))
            .param("fields", "files(id,name,parents,size,mimeType)")
            .doit()
            .await
            .chain_err(|| format!("Could not find {folder} folder in drive."))?
            .1;
        let folders = file_list_response.files.as_ref().unwrap();
        assert_eq!(folders.len(), 1);
        Ok(folders.first().as_ref().unwrap().id.as_ref().unwrap().clone())
    }

    pub async fn list_files_in_folder_id_per_page(
        &self,
        folder_id: &String,
        page_token: &mut Option<String>,
    ) -> google_drive3::Result<(Response<Body>, FileList)> {
        let mut list_query = self
            .hub
            .files()
            .list()
            .q(format!(
                "'{folder_id}' in parents and mimeType != 'application/vnd.google-apps.folder'"
            )
            .as_str())
            .param("fields", "nextPageToken,files(id,name,parents,md5Checksum,size,mimeType)");
        if page_token.is_some() {
            list_query = list_query.page_token(page_token.as_ref().unwrap().as_str());
        }
        list_query.doit().await
    }
    pub async fn get_content_for(&self, file: &File) -> Result<hyper::Response<Body>> {
        let export_mime_type =
            Drive::export_mime_types_from(file.mime_type.as_ref().unwrap().as_str());

        let file_content = if !export_mime_type.is_empty() {
            log::info!(
                "Export mimetype for file {} is {}",
                file.name.as_ref().unwrap(),
                export_mime_type
            );
            self.hub
                .files()
                .export(file.id.as_ref().unwrap(), export_mime_type)
                .doit()
                .await
                .chain_err(|| {
                    format!(
                        "Could not export file contents from drive for {}.",
                        file.name.as_ref().unwrap()
                    )
                })?
        } else {
            self.hub
                .files()
                .get(file.id.as_ref().unwrap())
                .param("alt", "media")
                .doit()
                .await
                .chain_err(|| {
                    format!(
                        "Could not download file contents from drive for {}.",
                        file.name.as_ref().unwrap()
                    )
                })?
                .0
        };
        assert!(file_content.status().is_success());

        Ok(file_content)
    }

    fn export_mime_types_from(k: &str) -> &str {
        match k {
            "application/vnd.google-apps.spreadsheet" => {
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            }
            "application/vnd.google-apps.document" => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
            "application/vnd.google-apps.drawing" => "image/svg+xml",
            "application/vnd.google-apps.presentation" => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            }
            _ => "",
        }
    }
}

pub async fn create_drive_hub(
    aus: AuthorizedUserSecret,
) -> DriveHub<HttpsConnector<HttpConnector>> {
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
