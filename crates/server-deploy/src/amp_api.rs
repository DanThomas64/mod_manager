use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

/// Thin client for AMP's session-based JSON API. Login gets a session ID
/// which every subsequent call includes in its body as `SESSIONID`. The
/// exact set of available methods (e.g. the right restart call) varies by
/// AMP version/module — every running instance self-documents its own API
/// at `<base_url>/API`, which is the source of truth if `call()` fails with
/// an unexpected response.
pub struct AmpClient {
    base_url: String,
    client: reqwest::blocking::Client,
    session_id: String,
}

impl AmpClient {
    pub fn login(base_url: &str, username: &str, password: &str) -> Result<Self> {
        let client = reqwest::blocking::Client::new();
        let base_url = base_url.trim_end_matches('/').to_string();
        let body = json!({
            "Username": username,
            "Password": password,
            "Token": "",
            "RememberMe": false,
        });

        let response: Value = client
            .post(format!("{base_url}/API/Core/Login"))
            .json(&body)
            .send()
            .context("sending AMP login request")?
            .error_for_status()
            .context("AMP login returned an error status")?
            .json()
            .context("parsing AMP login response")?;

        let success = response.get("success").and_then(Value::as_bool).unwrap_or(false);
        if !success {
            let reason = response.get("result").and_then(Value::as_str).unwrap_or("unknown reason");
            bail!("AMP login failed: {reason}");
        }
        let session_id = response
            .get("sessionID")
            .and_then(Value::as_str)
            .context("AMP login response had no sessionID")?
            .to_string();

        Ok(Self {
            base_url,
            client,
            session_id,
        })
    }

    /// Call an arbitrary AMP API method (e.g. `"Core/Restart"`), merging
    /// `params` into the request body alongside the session ID.
    pub fn call(&self, method: &str, params: Value) -> Result<Value> {
        let mut body: Map<String, Value> = match params {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        body.insert("SESSIONID".to_string(), Value::String(self.session_id.clone()));

        self.client
            .post(format!("{}/API/{method}", self.base_url))
            .json(&Value::Object(body))
            .send()
            .with_context(|| format!("calling AMP API {method}"))?
            .error_for_status()
            .with_context(|| format!("AMP API {method} returned an error status"))?
            .json()
            .with_context(|| format!("parsing AMP API {method} response"))
    }
}
