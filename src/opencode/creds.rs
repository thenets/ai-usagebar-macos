//! Where the OpenCode Go API key comes from.
//!
//! Resolution order (first non-empty wins):
//! 1. the env var named by `[opencode] api_key_env` (`OPENCODE_GO_API_KEY`);
//! 2. the inline `[opencode] api_key`;
//! 3. OpenCode 2's SQLite credential store, `<data>/opencode.db`
//!    (`credential` table, `value` is `{"type":"api","key":"…"}`);
//! 4. OpenCode 1's legacy `<data>/auth.json` (`{"opencode-go":{"key":…}}`).
//!
//! `<data>` is `$XDG_DATA_HOME/opencode`, else `~/.local/share/opencode` —
//! OpenCode uses XDG paths on macOS too, not `~/Library`.
//!
//! The `opencode-go` entry is preferred; the Zen (`opencode`) workspace key is
//! accepted as a fallback because the usage endpoint takes any workspace key.
//!
//! Like `anthropic::keychain`, the SQLite read shells out to the system
//! `sqlite3(1)` instead of linking a SQLite crate — it ships with macOS and
//! keeps the dependency tree untouched.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::OpencodeConfig;
use crate::error::{AppError, Result};

/// Credential ids in preference order.
const PROVIDER_IDS: [&str; 2] = ["opencode-go", "opencode"];

/// Default OpenCode data dir: `$XDG_DATA_HOME/opencode` or `~/.local/share/opencode`.
pub fn default_data_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME")
        && !xdg.is_empty()
    {
        return Ok(PathBuf::from(xdg).join("opencode"));
    }
    Ok(crate::cache::home_dir()?.join(".local/share/opencode"))
}

/// Resolve the API key using the full order above.
pub fn resolve(config: &OpencodeConfig) -> Result<String> {
    if let Ok(key) =
        crate::config::resolve_api_key("OpenCode", &config.api_key_env, config.api_key.as_deref())
    {
        return Ok(key);
    }
    let data_dir = match config.data_dir.as_deref() {
        Some(p) => p.to_path_buf(),
        None => default_data_dir()?,
    };
    read_from(&data_dir)?.ok_or_else(|| {
        AppError::Credentials(format!(
            "OpenCode Go: no API key. Run `opencode auth login` (OpenCode Go), \
             export {}, or set `api_key` under [opencode] in {}. Looked in {}.",
            config.api_key_env,
            crate::config::config_path_hint(),
            data_dir.display()
        ))
    })
}

/// Read the key OpenCode itself stored under `data_dir`. `Ok(None)` when
/// neither store has one. Test seam: point it at a temp dir.
pub fn read_from(data_dir: &Path) -> Result<Option<String>> {
    if let Some(key) = read_db(&data_dir.join("opencode.db")) {
        return Ok(Some(key));
    }
    read_auth_json(&data_dir.join("auth.json"))
}

/// Best-effort read of the OpenCode 2 credential table. Any failure (no db,
/// no `sqlite3`, older schema without the table) is "not found" so the
/// caller falls through to `auth.json`.
fn read_db(db: &Path) -> Option<String> {
    if !db.is_file() {
        return None;
    }
    // Ids are compile-time constants, so inlining them into the SQL is safe.
    let ids = PROVIDER_IDS
        .iter()
        .map(|id| format!("'{id}'"))
        .collect::<Vec<_>>()
        .join(",");
    let order = PROVIDER_IDS
        .iter()
        .enumerate()
        .map(|(i, id)| format!("WHEN '{id}' THEN {i}"))
        .collect::<Vec<_>>()
        .join(" ");
    let sql = format!(
        "SELECT json_extract(value, '$.key') FROM credential \
         WHERE integration_id IN ({ids}) \
           AND coalesce(json_extract(value, '$.key'), '') <> '' \
         ORDER BY CASE integration_id {order} END, active DESC, time_updated DESC \
         LIMIT 1;"
    );
    let out = Command::new("/usr/bin/sqlite3")
        .arg("-readonly")
        .arg("-batch")
        .arg("-noheader")
        .arg(db)
        .arg(sql)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let key = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!key.is_empty()).then_some(key)
}

fn read_auth_json(path: &Path) -> Result<Option<String>> {
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(AppError::io_at(path, e)),
    };
    let doc: serde_json::Value = serde_json::from_str(&raw)?;
    Ok(PROVIDER_IDS.iter().find_map(|id| {
        doc.get(id)
            .and_then(|entry| entry.get("key"))
            .and_then(|k| k.as_str())
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(str::to_string)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_db(dir: &Path, rows: &[(&str, &str, Option<i64>, i64)]) {
        let mut sql = String::from(
            "CREATE TABLE credential (id text PRIMARY KEY, integration_id text, \
             label text NOT NULL, value text NOT NULL, connector_id text, \
             method_id text, active integer, time_created integer NOT NULL, \
             time_updated integer NOT NULL);",
        );
        for (i, (id, key, active, updated)) in rows.iter().enumerate() {
            let active = active.map_or("NULL".to_string(), |a| a.to_string());
            sql.push_str(&format!(
                "INSERT INTO credential VALUES ('c{i}', '{id}', 'x', \
                 '{{\"type\":\"api\",\"key\":\"{key}\"}}', NULL, NULL, {active}, 0, {updated});"
            ));
        }
        let status = Command::new("/usr/bin/sqlite3")
            .arg(dir.join("opencode.db"))
            .arg(sql)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn empty_dir_has_no_key() {
        let td = TempDir::new().unwrap();
        assert_eq!(read_from(td.path()).unwrap(), None);
    }

    #[test]
    fn db_prefers_go_key_over_zen_and_other_providers() {
        let td = TempDir::new().unwrap();
        make_db(
            td.path(),
            &[
                ("openrouter", "or-key", None, 9),
                ("opencode", "zen-key", Some(1), 9),
                ("opencode-go", "go-key", Some(1), 1),
            ],
        );
        assert_eq!(read_from(td.path()).unwrap().as_deref(), Some("go-key"));
    }

    #[test]
    fn db_falls_back_to_zen_key() {
        let td = TempDir::new().unwrap();
        make_db(td.path(), &[("opencode", "zen-key", None, 1)]);
        assert_eq!(read_from(td.path()).unwrap().as_deref(), Some("zen-key"));
    }

    #[test]
    fn db_without_matching_row_falls_through_to_auth_json() {
        let td = TempDir::new().unwrap();
        make_db(td.path(), &[("openrouter", "or-key", None, 1)]);
        std::fs::write(
            td.path().join("auth.json"),
            r#"{"opencode-go":{"type":"api","key":"legacy-key"}}"#,
        )
        .unwrap();
        assert_eq!(read_from(td.path()).unwrap().as_deref(), Some("legacy-key"));
    }

    #[test]
    fn auth_json_ignores_unrelated_and_empty_entries() {
        let td = TempDir::new().unwrap();
        std::fs::write(
            td.path().join("auth.json"),
            r#"{"openrouter":{"type":"api","key":"or"},"opencode-go":{"type":"api","key":"  "}}"#,
        )
        .unwrap();
        assert_eq!(read_from(td.path()).unwrap(), None);
    }

    #[test]
    fn resolve_uses_data_dir_override_and_names_it_on_error() {
        let td = TempDir::new().unwrap();
        let cfg = OpencodeConfig {
            // An env var name nobody sets keeps this hermetic.
            api_key_env: "AI_USAGEBAR_TEST_UNSET_OPENCODE_KEY".into(),
            data_dir: Some(td.path().to_path_buf()),
            ..OpencodeConfig::default()
        };
        let err = resolve(&cfg).unwrap_err().to_string();
        assert!(err.contains("opencode auth login"), "{err}");

        std::fs::write(
            td.path().join("auth.json"),
            r#"{"opencode-go":{"type":"api","key":"from-disk"}}"#,
        )
        .unwrap();
        assert_eq!(resolve(&cfg).unwrap(), "from-disk");
    }

    #[test]
    fn resolve_inline_key_wins_over_disk() {
        let td = TempDir::new().unwrap();
        std::fs::write(
            td.path().join("auth.json"),
            r#"{"opencode-go":{"type":"api","key":"from-disk"}}"#,
        )
        .unwrap();
        let cfg = OpencodeConfig {
            api_key_env: "AI_USAGEBAR_TEST_UNSET_OPENCODE_KEY".into(),
            api_key: Some("inline".into()),
            data_dir: Some(td.path().to_path_buf()),
            ..OpencodeConfig::default()
        };
        assert_eq!(resolve(&cfg).unwrap(), "inline");
    }
}
