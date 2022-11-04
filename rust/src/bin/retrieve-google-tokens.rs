use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::str;

use oauth2::basic::BasicClient;
use oauth2::reqwest::http_client;
use oauth2::url::Url;
use oauth2::{
    AuthType, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, HttpRequest,
    HttpResponse, RedirectUrl, RevocationUrl, Scope, TokenUrl,
};
use reqwest;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct OAuthData {
    installed: Installed,
}

#[derive(Serialize, Deserialize, Debug)]
struct Installed {
    client_id: String,
    project_id: String,
    auth_uri: String,
    token_uri: String,
    auth_provider_x509_cert_url: String,
    client_secret: String,
    redirect_uris: Vec<String>,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let file_path = &args[1];
    let token_file_path = &args[2];

    let oauth: OAuthData =
        serde_json::from_str(fs::read_to_string(file_path).unwrap().as_str()).unwrap();

    let client = BasicClient::new(
        ClientId::new(oauth.installed.client_id),
        Some(ClientSecret::new(oauth.installed.client_secret)),
        AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(),
        Some(TokenUrl::new(oauth.installed.token_uri).unwrap()),
    )
    .set_redirect_uri(RedirectUrl::new("http://locahost:7777".to_string()).unwrap())
    .set_revocation_uri(
        RevocationUrl::new("https://oauth2.googleapis.com/revoke".to_string()).unwrap(),
    )
    .set_auth_type(AuthType::RequestBody);
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("https://www.googleapis.com/auth/photoslibrary.readonly".to_string()))
        .add_scope(Scope::new("https://www.googleapis.com/auth/drive.readonly".to_string()))
        .add_extra_param("access_type", "offline")
        .add_extra_param("include_granted_scopes", "true")
        .url();
    println!(
        "Browse to: {}",
        auth_url
            .to_string()
            .replace("http%3A%2F%2Flocahost%3A7777", "http%3A//localhost:7777")
            .replace("%2F", "/")
    );

    let listener = TcpListener::bind("127.0.0.1:7777").unwrap();
    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            let code;
            let state;
            {
                let mut reader = BufReader::new(&stream);

                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();

                let redirect_url = request_line.split_whitespace().nth(1).unwrap();
                let url = Url::parse(&("http://localhost".to_string() + redirect_url)).unwrap();

                let (_, value) = url
                    .query_pairs()
                    .find(|pair| {
                        let &(ref key, _) = pair;
                        key == "code"
                    })
                    .unwrap();
                code = AuthorizationCode::new(value.into_owned());

                let (_, value) = url
                    .query_pairs()
                    .find(|pair| {
                        let &(ref key, _) = pair;
                        key == "state"
                    })
                    .unwrap();
                state = CsrfToken::new(value.into_owned());
            }

            let message = "Go back to your terminal :)";
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{}",
                        message.len(),
                        message
                    )
                    .as_bytes(),
                )
                .unwrap();

            println!("Google returned the following code:\n{}\n", code.secret());
            println!(
                "Google returned the following state:\n{} (expected `{}`)\n",
                state.secret(),
                csrf_token.secret()
            );

            // Exchange the code with a token.
            let token_response = client.exchange_code(code).request(my_http_client);

            println!("Google returned the following token:\n{:?}\n", token_response);

            fs::write(
                token_file_path,
                serde_json::to_string_pretty(&token_response.unwrap()).unwrap(),
            )
            .unwrap();

            break;
        }
    }
}

fn my_http_client(
    mut request: HttpRequest,
) -> Result<HttpResponse, oauth2::reqwest::Error<reqwest::Error>> {
    dbg!(&request.url);
    dbg!(&request.headers);
    dbg!(&request.method);
    let new_body = str::from_utf8(&request.body)
        .unwrap()
        .replace("http%3A%2F%2Flocahost%3A7777", "http%3A//localhost:7777")
        .replace("%2F", "/");
    dbg!(&new_body);
    request.body = new_body.as_bytes().to_vec();
    http_client(request)
}
