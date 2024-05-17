use std::process::Command;

use oauth2::{
    AuthorizationCode, AuthUrl, basic::BasicClient, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, RedirectUrl, reqwest::http_client, Scope, TokenResponse, TokenUrl,
};
use url::Url;

fn main() -> Result<(), anyhow::Error> {
    // construct OAuth query: authorization code flow with PKCE

    let oauth_client = BasicClient::new(
        ClientId::new("client-id".to_string()),
        Some(ClientSecret::new("client-secret".to_string())),
        AuthUrl::new("http://localhost:8090/authorize".to_string())?,
        Some(TokenUrl::new("http://localhost:8090/token".to_string())?),
    )
        .set_redirect_uri(RedirectUrl::new("http://example.com/callback".to_string())?);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token) = oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    // use curl to handle HTTP basic authentication

    let args = vec![
        "-w".to_string(),
        "%{redirect_url}\n".to_string(),
        "-u".to_string(),
        "rmalina1:test-password".to_string(),
        auth_url.to_string(),
    ];

    let curl_output = Command::new("curl").args(args).output()?;

    // parse URL from redirect to callback with authorization code

    let url = Url::parse(std::str::from_utf8(&curl_output.stdout)?.trim())?;

    let mut query_state = None;
    let mut query_code = None;

    for (key, value) in url.query_pairs() {
        match key.to_string().as_str() {
            "state" => query_state = Some(value),
            "code" => query_code = Some(value),
            _ => {}
        }
    }

    assert_eq!(*csrf_token.secret(), query_state.unwrap().to_string());

    // exchange authorization code for access token

    let auth_code = query_code.unwrap();
    let token_response = oauth_client
        .exchange_code(AuthorizationCode::new(auth_code.to_string()))
        .set_pkce_verifier(pkce_verifier)
        .request(http_client)?;

    println!("{}", token_response.access_token().secret());
    Ok(())
}
