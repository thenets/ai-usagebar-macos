//! OpenCode Go fetch — plain `Authorization: Bearer <KEY>`. Same cache /
//! stale-fallback semantics as the other API-key vendors.

use std::time::Duration;

use crate::cache::{Cache, acquire_lock};
use crate::error::{AppError, Result};
use crate::usage::OpencodeSnapshot;

use super::types::Envelope;

pub const USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const LOCK_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub struct Endpoints {
    pub usage: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            usage: USAGE_URL.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FetchOutcome {
    pub snapshot: OpencodeSnapshot,
    pub stale: bool,
    pub last_error: Option<(u16, String)>,
    pub cache_age: Option<Duration>,
}

pub async fn fetch_snapshot(
    client: &reqwest::Client,
    api_key: &str,
    cache: &Cache,
    endpoints: &Endpoints,
    cache_ttl: Duration,
) -> Result<FetchOutcome> {
    cache.ensure_dir()?;
    let _lock = acquire_lock(&cache.lock_path(), LOCK_TIMEOUT)?;

    if let Some(bytes) = cache.fresh_payload(cache_ttl)? {
        return Ok(reuse(bytes, cache, false));
    }

    match fetch_live(client, &endpoints.usage, api_key).await {
        Ok(bytes) => {
            cache.write_payload(&bytes)?;
            let env: Envelope = serde_json::from_slice(&bytes)?;
            Ok(FetchOutcome {
                snapshot: env.into_snapshot(),
                stale: false,
                last_error: None,
                cache_age: Some(Duration::ZERO),
            })
        }
        Err(e) if e.is_transient() => fallback_silent(cache),
        Err(AppError::Http { status, body }) => {
            cache.mark_stale();
            cache.write_last_error(status, &body);
            fallback_with_error(cache, Some((status, body)))
        }
        Err(e) => {
            cache.mark_stale();
            cache.write_last_error(0, &e.to_string());
            fallback_with_error(cache, Some((0, e.to_string())))
        }
    }
}

fn reuse(bytes: Vec<u8>, cache: &Cache, stale: bool) -> FetchOutcome {
    let snapshot = serde_json::from_slice::<Envelope>(&bytes)
        .unwrap_or_default()
        .into_snapshot();
    FetchOutcome {
        snapshot,
        stale,
        last_error: cache.read_last_error(),
        cache_age: cache.payload_age(),
    }
}

fn fallback_silent(cache: &Cache) -> Result<FetchOutcome> {
    let Some(bytes) = cache.maybe_payload()? else {
        return Err(AppError::Transport(
            "opencode: no cache and network unreachable".into(),
        ));
    };
    Ok(reuse(bytes, cache, true))
}

fn fallback_with_error(cache: &Cache, last_error: Option<(u16, String)>) -> Result<FetchOutcome> {
    let Some(bytes) = cache.maybe_payload()? else {
        return Err(AppError::Other("opencode: no usable cache".into()));
    };
    let mut out = reuse(bytes, cache, true);
    out.last_error = last_error;
    Ok(out)
}

async fn fetch_live(client: &reqwest::Client, url: &str, api_key: &str) -> Result<Vec<u8>> {
    let resp = tokio::time::timeout(
        HTTP_TIMEOUT,
        client
            .get(url)
            .bearer_auth(api_key)
            .header("Accept", "application/json")
            .send(),
    )
    .await
    .map_err(|_| AppError::Transport(format!("opencode timeout: {url}")))??;

    let status = resp.status();
    let bytes = resp.bytes().await?.to_vec();

    if !status.is_success() {
        let body = String::from_utf8_lossy(&bytes).chars().take(200).collect();
        return Err(AppError::Http {
            status: status.as_u16(),
            body,
        });
    }

    // Sanity check we got a valid envelope. Schema drift surfaces here.
    let _: Envelope = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::Schema(format!("opencode usage response: {e}")))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const PATH: &str = "/zen/go/v1/usage";

    fn cache_fixture() -> (TempDir, Cache) {
        let td = TempDir::new().unwrap();
        let cache = Cache::at(td.path().join("opencode"));
        cache.ensure_dir().unwrap();
        (td, cache)
    }

    fn endpoints(server: &mockito::Server) -> Endpoints {
        Endpoints {
            usage: format!("{}{PATH}", server.url()),
        }
    }

    #[tokio::test]
    async fn live_200_sends_bearer_and_parses_real_shape() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", PATH)
            .match_header("authorization", "Bearer fake-key")
            .with_status(200)
            .with_body(
                r#"{"usage":{
                    "rolling":{"status":"ok","percent":42,"resetsAt":"2026-10-03T04:27:31.000Z"},
                    "weekly":{"status":"ok","percent":15,"resetsAt":"2026-10-05T00:00:00.000Z"},
                    "monthly":{"status":"ok","percent":7,"resetsAt":"2026-11-02T23:18:00.000Z"}
                }}"#,
            )
            .create_async()
            .await;

        let (_td, cache) = cache_fixture();
        let client = reqwest::Client::new();
        let out = fetch_snapshot(
            &client,
            "fake-key",
            &cache,
            &endpoints(&server),
            Duration::from_secs(0),
        )
        .await
        .unwrap();
        mock.assert_async().await;
        assert!(!out.stale);
        assert_eq!(out.snapshot.plan, "OpenCode Go");
        assert_eq!(out.snapshot.session.as_ref().unwrap().utilization_pct, 42);
        assert_eq!(out.snapshot.weekly.as_ref().unwrap().utilization_pct, 15);
        assert_eq!(out.snapshot.monthly.as_ref().unwrap().utilization_pct, 7);
    }

    #[tokio::test]
    async fn http_401_falls_back_to_cache_when_present() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", PATH)
            .with_status(401)
            .with_body(r#"{"type":"error","error":{"type":"AuthError","message":"Unauthorized"}}"#)
            .create_async()
            .await;

        let (_td, cache) = cache_fixture();
        cache
            .write_payload(br#"{"usage":{"rolling":{"percent":10}}}"#)
            .unwrap();

        let client = reqwest::Client::new();
        let out = fetch_snapshot(
            &client,
            "k",
            &cache,
            &endpoints(&server),
            Duration::from_secs(0),
        )
        .await
        .unwrap();
        assert!(out.stale);
        assert_eq!(out.snapshot.session.as_ref().unwrap().utilization_pct, 10);
        assert_eq!(out.last_error.as_ref().map(|(c, _)| *c), Some(401));
    }

    #[tokio::test]
    async fn http_401_without_cache_is_an_error() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", PATH)
            .with_status(401)
            .with_body("Unauthorized")
            .create_async()
            .await;

        let (_td, cache) = cache_fixture();
        let client = reqwest::Client::new();
        let res = fetch_snapshot(
            &client,
            "k",
            &cache,
            &endpoints(&server),
            Duration::from_secs(0),
        )
        .await;
        assert!(res.is_err());
    }
}
