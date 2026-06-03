use crate::errors::{Error, Result};
use crate::ResultExt;
use async_trait::async_trait;
use google_drive3::api::File;
use google_drive3::hyper::{client::HttpConnector, Body, Response};
use google_drive3::hyper_rustls::HttpsConnector;
use google_drive3::{api::FileList, hyper, hyper_rustls, DriveHub};
use google_drive3::{oauth2, oauth2::authorized_user::AuthorizedUserSecret};
use std::path::Path;

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

    pub async fn get_file_from(&self, path: &Path) -> Result<File> {
        get_file_from(path, self).await
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
    pub async fn trash_file(&self, file_id: &String) -> Result<()> {
        let resp = self
            .hub
            .files()
            .update(
                {
                    let mut d = File::default();
                    d.trashed = Some(true);
                    d
                },
                file_id,
            )
            .doit_without_upload()
            .await?;
        assert!(resp.0.status().is_success());
        Ok(())
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

#[async_trait]
impl GetFileFor for Drive {
    async fn call(&self, folder_id: &String, filename: &String) -> Result<File> {
        let resp = self
            .hub
            .files()
            .list()
            .q(&*format!(
                "name = '{filename}' and \
                 '{folder_id}' in parents"
            ))
            .param("fields", "files(id,name,parents,size,mimeType)")
            .doit()
            .await
            .chain_err(|| {
                format!("Could not find {filename} folder in drive in folder_id {folder_id}.")
            })?;
        assert!(resp.0.status().is_success());
        let files = resp.1.files.as_ref().unwrap();
        assert_eq!(files.len(), 1);
        Ok(files[0].clone())
    }
}

#[async_trait]
trait GetFileFor {
    async fn call(&self, folder_id: &String, filename: &String) -> Result<File>;
}

async fn get_file_from(path: &Path, get_file_for: &impl GetFileFor) -> Result<File> {
    if !path.is_absolute() {
        return Err(Error::from("Drive folder path muist be absolute (start with /)"));
    }
    let mut file = File::default();
    file.id = Some(String::from("root"));
    for part in path.strip_prefix("/").unwrap() {
        file = get_file_for
            .call(file.id.as_ref().unwrap(), &part.to_str().unwrap().to_string())
            .await?;
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use crate::drive::{get_file_from, GetFileFor};
    use crate::errors::{Error, Result};
    use async_trait::async_trait;
    use google_drive3::api::File;
    use std::path::Path;

    #[derive(Debug, Clone)]
    struct Node {
        id: String,
        name: String,
        children: Vec<Node>,
    }

    impl Node {
        fn find_node(&self, id: &String) -> Option<&Node> {
            if &self.id == id {
                return Some(self);
            }
            for c in self.children.iter() {
                if let Some(r) = c.find_node(id) {
                    return Some(r);
                }
            }
            None
        }
    }
    struct TestDrive {
        tree: Node,
    }

    impl TestDrive {}

    #[async_trait]
    impl GetFileFor for TestDrive {
        async fn call(&self, folder_id: &String, filename: &String) -> Result<File> {
            for c in self.tree.find_node(folder_id).unwrap().children.iter() {
                if &c.name == filename {
                    return Ok({
                        let mut f = File::default();
                        f.name = Some(c.name.clone());
                        f.id = Some(c.id.clone());
                        f
                    });
                }
            }
            Err(Error::from("NOT FOUND"))
        }
    }

    #[tokio::test]
    async fn it_works() {
        let tree = Node {
            id: "root".to_string(),
            name: "".to_string(),
            children: vec![
                Node { id: "1".to_string(), name: "one".to_string(), children: vec![] },
                Node {
                    id: "2".to_string(),
                    name: "two".to_string(),
                    children: vec![Node {
                        id: "4".to_string(),
                        name: "four".to_string(),
                        children: vec![Node {
                            id: "5".to_string(),
                            name: "five".to_string(),
                            children: vec![],
                        }],
                    }],
                },
                Node { id: "3".to_string(), name: "three".to_string(), children: vec![] },
            ],
        };

        let a = get_file_from(&Path::new("/two//four/"), &TestDrive { tree: tree.clone() })
            .await
            .unwrap();
        assert_eq!(a.id.unwrap(), "4");
        let a = get_file_from(&Path::new("two/four"), &TestDrive { tree: tree.clone() }).await;
        assert!(a.is_err());
        assert!(a.unwrap_err().to_string().contains("muist be absolute"));
    }
}
