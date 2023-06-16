use oauth2::basic::BasicClient;
use oauth2::reqwest::http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    ResponseType, Scope, TokenResponse, TokenUrl,
};
fn main() -> Result<(), anyhow::Error> {
    let oauth_client = BasicClient::new(
        ClientId::new("hXaf3vs5oeL0y2AJPBh9BEeGkD3e3u3e".to_string()),
        Some(ClientSecret::new(
            "PakBX-b-sV4dplt5tjbIr56FoXkBFWQEvm2ZVgzej2rySRWcqr9qvVScF3btmKEc".to_string(),
        )),
        AuthUrl::new("https://dev-cha-9aet.us.auth0.com/authorize".to_string())?,
        Some(TokenUrl::new(
            "https://dev-cha-9aet.us.auth0.com/oauth/token".to_string(),
        )?),
    )
    .set_redirect_uri(RedirectUrl::new("https://dev-cha-9aet.us.auth0.com/authorize".to_string())?);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, _csrf_token) = oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .set_response_type(&ResponseType::new("code".to_string())) // Add this line to set the response type
        .url();

    // Open the authorization URL in the default browser
    open::that(auth_url.to_string())?;

    // Retrieve the authorization code manually after authentication
    println!("Please enter the authorization code:");
    let mut auth_code = String::new();
    std::io::stdin().read_line(&mut auth_code)?;

    // exchange authorization code for access token
    let token_response = oauth_client
        .exchange_code(AuthorizationCode::new(auth_code.trim().to_string()))
        .set_pkce_verifier(pkce_verifier)
        .request(http_client)?;

    println!("{}", token_response.access_token().secret());
    Ok(())
}
