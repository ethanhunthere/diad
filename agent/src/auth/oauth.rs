//! OAuth flow management for LLM provider authentication.
//!
//! Opens the system browser (Brave) for OAuth consent and
//! receives the callback on a localhost server.

use std::net::SocketAddr;
use std::process::Command;

use tokio::io::AsyncBufReadExt;
use tokio::net::TcpListener;

/// Represents an OAuth 2.0 authorization flow.
pub struct OAuthFlow {
    /// The provider name (anthropic, gemini, etc.)
    provider: String,
    /// OAuth client ID
    client_id: String,
    /// Authorization endpoint URL
    auth_url: String,
    /// Token endpoint URL
    token_url: String,
    /// Local port for the redirect callback
    local_port: u16,
}

impl OAuthFlow {
    /// Create a new OAuth flow for a specific provider.
    ///
    /// Returns an error for providers with no known OAuth endpoints.
    pub fn new(provider: impl Into<String>, client_id: impl Into<String>) -> anyhow::Result<Self> {
        let provider = provider.into();
        let (auth_url, token_url) = match provider.as_str() {
            "anthropic" => (
                "https://console.anthropic.com/oauth/authorize".to_string(),
                "https://api.anthropic.com/oauth/token".to_string(),
            ),
            "gemini" => (
                "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
                "https://oauth2.googleapis.com/token".to_string(),
            ),
            "openai" => (
                "https://platform.openai.com/oauth/authorize".to_string(),
                "https://api.openai.com/oauth/token".to_string(),
            ),
            // Unknown provider: fail fast instead of guessing a URL.
            _ => {
                return Err(anyhow::anyhow!(
                    "unknown OAuth provider '{provider}': no authorization/token endpoints configured"
                ));
            }
        };

        Ok(Self {
            provider,
            client_id: client_id.into(),
            auth_url,
            token_url,
            local_port: 18920,
        })
    }

    /// Initiate the OAuth flow.
    ///
    /// Opens the system browser to the authorization URL.
    /// The redirect URI is `http://localhost:{port}/callback`.
    pub async fn initiate_auth(&self) -> anyhow::Result<String> {
        let redirect_uri = format!("http://localhost:{}/callback", self.local_port);
        let state = uuid::Uuid::new_v4().to_string();

        let auth_request_url = self.build_auth_url(&redirect_uri, &state);

        tracing::info!("Opening browser for OAuth: {}", self.provider);

        // Open browser
        let open_result = Command::new("xdg-open").arg(&auth_request_url).spawn();

        match open_result {
            Ok(_) => tracing::info!("Browser opened for authentication."),
            Err(e) => {
                // Try Brave directly
                let _ = Command::new("brave").arg(&auth_request_url).spawn();
                tracing::warn!("xdg-open failed ({e}), tried Brave directly.");
            }
        }

        // Start local server to receive the callback
        self.receive_callback(&state).await
    }

    /// Build the provider authorization URL with standard OAuth2 query params.
    fn build_auth_url(&self, redirect_uri: &str, state: &str) -> String {
        format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&state={}&scope=openai%20profile",
            self.auth_url,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(state),
        )
    }

    /// Listen on localhost for the OAuth callback.
    ///
    /// `expected_state` is the CSRF state sent in the authorization request;
    /// the callback's `state` parameter must match it exactly.
    async fn receive_callback(&self, expected_state: &str) -> anyhow::Result<String> {
        let addr: SocketAddr = ([127, 0, 0, 1], self.local_port).into();
        let listener = TcpListener::bind(&addr).await?;

        tracing::info!(
            "Waiting for OAuth callback on http://localhost:{}...",
            self.local_port
        );

        // Accept one connection
        let (mut stream, peer_addr) = listener.accept().await?;
        tracing::info!("OAuth callback received from {}", peer_addr);

        // Read the HTTP request
        let buf_reader = tokio::io::BufReader::new(&mut stream);
        let mut lines = buf_reader.lines();
        let mut request_line = String::new();

        while let Some(line) = lines.next_line().await? {
            if line.is_empty() {
                break;
            }
            if request_line.is_empty() {
                request_line = line;
            }
        }

        // Parse the authorization code and validate the CSRF state.
        let (auth_code, received_state) = Self::extract_code_from_request(&request_line);

        match received_state.as_deref() {
            Some(s) if s == expected_state => {}
            Some(_) => {
                tracing::warn!("OAuth callback state mismatch — rejecting");
                let resp = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
                    <html><body><h1>Authentication Failed</h1>\
                    <p>State parameter mismatch.</p></body></html>";
                use tokio::io::AsyncWriteExt;
                stream.write_all(resp.as_bytes()).await?;
                stream.shutdown().await?;
                return Err(anyhow::anyhow!(
                    "OAuth state parameter mismatch: possible CSRF attempt"
                ));
            }
            None => {
                tracing::warn!("OAuth callback missing state parameter — rejecting");
                return Err(anyhow::anyhow!(
                    "OAuth callback is missing the required state parameter"
                ));
            }
        }

        // Send success response
        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
            <html><body><h1>Authentication Complete</h1>\
            <p>You may close this tab and return to the DIAD Agent.</p>\
            </body></html>";

        use tokio::io::AsyncWriteExt;
        stream.write_all(response.as_bytes()).await?;
        stream.shutdown().await?;

        match auth_code {
            Some(code) => {
                tracing::info!("Authorization code received.");
                Ok(code)
            }
            None => Err(anyhow::anyhow!(
                "No authorization code found in callback request"
            )),
        }
    }

    /// Exchange an authorization code for an access token.
    pub async fn exchange_code_for_token(
        &self,
        code: &str,
        client_secret: &str,
    ) -> anyhow::Result<String> {
        let redirect_uri = format!("http://localhost:{}/callback", self.local_port);

        let client = reqwest::Client::new();
        let response = client
            .post(&self.token_url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", &redirect_uri),
                ("client_id", &self.client_id),
                ("client_secret", client_secret),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let body = response.text().await?;
            return Err(anyhow::anyhow!("Token exchange failed: {body}"));
        }

        let json: serde_json::Value = response.json().await?;
        let token = json["access_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No access_token in response"))?
            .to_string();

        Ok(token)
    }

    /// Extract the `code` and `state` parameters from an HTTP request line.
    fn extract_code_from_request(request_line: &str) -> (Option<String>, Option<String>) {
        // Parse: GET /callback?code=xxx&state=yyy HTTP/1.1
        let path_and_query = match request_line.split_whitespace().nth(1) {
            Some(p) => p,
            None => return (None, None),
        };

        let mut code = None;
        let mut state = None;

        if let Some(query_start) = path_and_query.find('?') {
            let query = &path_and_query[query_start + 1..];
            for param in query.split('&') {
                if let Some((key, value)) = param.split_once('=') {
                    match key {
                        "code" => code = urlencoding::decode(value).ok().map(String::from),
                        "state" => state = urlencoding::decode(value).ok().map(String::from),
                        _ => {}
                    }
                }
            }
        }

        (code, state)
    }
}
