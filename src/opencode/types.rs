//! Wire types for the OpenCode Go usage endpoint
//! `https://opencode.ai/zen/go/v1/usage`.
//!
//! Real response shape (captured 2026-10-02):
//!
//! ```json
//! {"usage":{
//!   "rolling":{"status":"ok","percent":0,"resetsAt":"2026-10-03T04:27:31.000Z"},
//!   "weekly": {"status":"ok","percent":0,"resetsAt":"2026-10-05T00:00:00.000Z"},
//!   "monthly":{"status":"ok","percent":0,"resetsAt":"2026-11-02T23:18:00.000Z"}
//! }}
//! ```
//!
//! `rolling` is the 5-hour window. A bad key returns HTTP 401 with
//! `{"type":"error","error":{"type":"AuthError","message":"Unauthorized"}}`.

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::usage::{OpencodeSnapshot, UsageWindow};

pub const PLAN_LABEL: &str = "OpenCode Go";

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Envelope {
    pub usage: Option<UsageBlock>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct UsageBlock {
    pub rolling: Option<WindowEntry>,
    pub weekly: Option<WindowEntry>,
    pub monthly: Option<WindowEntry>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct WindowEntry {
    pub status: String,
    pub percent: f64,
    /// ISO-8601; an unparseable or missing value becomes `None`.
    #[serde(rename = "resetsAt", deserialize_with = "de_opt_rfc3339")]
    pub resets_at: Option<DateTime<Utc>>,
}

fn de_opt_rfc3339<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<String>::deserialize(d)?;
    Ok(v.and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc)))
}

impl Envelope {
    /// Project into the canonical [`OpencodeSnapshot`]. Missing windows stay
    /// `None` so the renderers can say "no usage windows reported".
    pub fn into_snapshot(self) -> OpencodeSnapshot {
        let usage = self.usage.unwrap_or_default();
        OpencodeSnapshot {
            plan: PLAN_LABEL.to_string(),
            session: usage
                .rolling
                .as_ref()
                .map(|w| to_window(w, chrono::Duration::hours(5))),
            weekly: usage
                .weekly
                .as_ref()
                .map(|w| to_window(w, chrono::Duration::days(7))),
            monthly: usage
                .monthly
                .as_ref()
                .map(|w| to_window(w, chrono::Duration::days(30))),
        }
    }
}

fn to_window(w: &WindowEntry, dur: chrono::Duration) -> UsageWindow {
    UsageWindow {
        utilization_pct: w.percent.round().clamp(0.0, 100.0) as i32,
        resets_at: w.resets_at,
        window_duration: dur,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL_BODY: &str = r#"{"usage":{
        "rolling":{"status":"ok","percent":22,"resetsAt":"2026-10-03T04:27:31.000Z"},
        "weekly":{"status":"ok","percent":62,"resetsAt":"2026-10-05T00:00:00.000Z"},
        "monthly":{"status":"ok","percent":59,"resetsAt":"2026-11-02T23:18:00.000Z"}
    }}"#;

    #[test]
    fn parses_real_response_shape() {
        let snap = serde_json::from_str::<Envelope>(REAL_BODY)
            .unwrap()
            .into_snapshot();
        assert_eq!(snap.plan, "OpenCode Go");
        let session = snap.session.unwrap();
        assert_eq!(session.utilization_pct, 22);
        assert_eq!(session.window_duration, chrono::Duration::hours(5));
        assert_eq!(
            session.resets_at.unwrap().to_rfc3339(),
            "2026-10-03T04:27:31+00:00"
        );
        assert_eq!(snap.weekly.unwrap().utilization_pct, 62);
        assert_eq!(snap.monthly.unwrap().utilization_pct, 59);
    }

    #[test]
    fn missing_usage_yields_empty_snapshot() {
        let snap = serde_json::from_str::<Envelope>("{}")
            .unwrap()
            .into_snapshot();
        assert!(snap.session.is_none());
        assert!(snap.weekly.is_none());
        assert!(snap.monthly.is_none());
    }

    #[test]
    fn percent_rounds_and_clamps() {
        let body = r#"{"usage":{
            "rolling":{"percent":42.6},
            "weekly":{"percent":180},
            "monthly":{"percent":-3}
        }}"#;
        let snap = serde_json::from_str::<Envelope>(body)
            .unwrap()
            .into_snapshot();
        assert_eq!(snap.session.unwrap().utilization_pct, 43);
        assert_eq!(snap.weekly.unwrap().utilization_pct, 100);
        assert_eq!(snap.monthly.unwrap().utilization_pct, 0);
    }

    #[test]
    fn bad_or_null_reset_becomes_none() {
        let body = r#"{"usage":{
            "rolling":{"percent":1,"resetsAt":null},
            "weekly":{"percent":1,"resetsAt":"not a date"}
        }}"#;
        let snap = serde_json::from_str::<Envelope>(body)
            .unwrap()
            .into_snapshot();
        assert!(snap.session.unwrap().resets_at.is_none());
        assert!(snap.weekly.unwrap().resets_at.is_none());
    }
}
