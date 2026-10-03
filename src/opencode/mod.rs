//! OpenCode Go vendor — `GET https://opencode.ai/zen/go/v1/usage`.
//! Auth is `Authorization: Bearer <KEY>` with the workspace API key that
//! `opencode auth login` stores (see `creds.rs` for where it lives).

pub mod creds;
pub mod fetch;
pub mod types;
pub mod vendor;

pub use fetch::{FetchOutcome, fetch_snapshot};
