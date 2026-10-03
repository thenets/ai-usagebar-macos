//! OpenCode Go renderer — bar text + bordered Pango tooltip.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::countdown;
use crate::format::{placeholders, substitute, updated_at_hm};
use crate::pacing::PaceSeverity;
use crate::pango::{self, color_span, escape, severity_color, severity_for};
use crate::theme::Theme;
use crate::tooltip::{Line as TooltipLine, render_bordered};
use crate::usage::{OpencodeSnapshot, UsageWindow};
use crate::vendor::{RenderOpts, VendorOutcome};
use crate::waybar::{Class, WaybarOutput};

use super::fetch::FetchOutcome;

pub const DEFAULT_FORMAT: &str = "{opencode_session_pct}% · {opencode_session_reset}";

fn pct(w: &Option<UsageWindow>) -> i32 {
    w.as_ref().map(|w| w.utilization_pct).unwrap_or(0)
}

fn reset(w: &Option<UsageWindow>, now: DateTime<Utc>) -> String {
    countdown::format(w.as_ref().and_then(|w| w.resets_at), now)
}

pub fn build_placeholders(
    snap: &OpencodeSnapshot,
    now: DateTime<Utc>,
) -> HashMap<&'static str, String> {
    placeholders(vec![
        ("icon", "󰚩".to_string()),
        ("vendor_short", "opencode".to_string()),
        // Cross-vendor aliases for scroll-cycle friendly formats.
        ("session_pct", pct(&snap.session).to_string()),
        ("session_reset", reset(&snap.session, now)),
        ("weekly_pct", pct(&snap.weekly).to_string()),
        ("weekly_reset", reset(&snap.weekly, now)),
        ("plan", snap.plan.clone()),
        ("opencode_plan", snap.plan.clone()),
        ("opencode_session_pct", pct(&snap.session).to_string()),
        ("opencode_session_reset", reset(&snap.session, now)),
        ("opencode_weekly_pct", pct(&snap.weekly).to_string()),
        ("opencode_weekly_reset", reset(&snap.weekly, now)),
        ("opencode_monthly_pct", pct(&snap.monthly).to_string()),
        ("opencode_monthly_reset", reset(&snap.monthly, now)),
    ])
}

pub fn severity(snap: &OpencodeSnapshot) -> PaceSeverity {
    severity_for(
        [pct(&snap.session), pct(&snap.weekly), pct(&snap.monthly)]
            .into_iter()
            .max()
            .unwrap_or(0),
    )
}

pub fn render(
    outcome: &VendorOutcome,
    snap: &OpencodeSnapshot,
    theme: &Theme,
    opts: &RenderOpts,
    now: DateTime<Utc>,
) -> WaybarOutput {
    let class = Class::from(severity(snap));
    let format = opts
        .format
        .clone()
        .unwrap_or_else(|| DEFAULT_FORMAT.to_string());
    let values = build_placeholders(snap, now);

    let mut text = substitute(&format, &values);
    if outcome.stale {
        text.push_str(" ⏸");
    }
    let wrapper_color = severity_color(severity(snap), theme).to_string();
    let icon_prefix = match opts.icon.as_deref() {
        Some(ic) if !ic.is_empty() => format!("{ic} "),
        _ => String::new(),
    };
    let bar_text = color_span(&wrapper_color, &format!("{icon_prefix}{text}"));

    let tooltip = if let Some(fmt) = opts.tooltip_format.as_deref() {
        substitute(fmt, &values)
    } else {
        render_tooltip(outcome, snap, theme, now)
    };

    WaybarOutput {
        text: bar_text,
        tooltip,
        class,
    }
}

fn render_tooltip(
    outcome: &VendorOutcome,
    snap: &OpencodeSnapshot,
    theme: &Theme,
    now: DateTime<Utc>,
) -> String {
    let blue = &theme.blue;
    let dim = &theme.dim;
    let mut lines: Vec<TooltipLine> = Vec::new();
    lines.push(TooltipLine::Center(format!(
        "<span font_weight='bold' foreground='{blue}'>{plan}</span>",
        plan = escape(&snap.plan)
    )));
    lines.push(TooltipLine::Sep);
    lines.push(TooltipLine::Body("".into()));

    let windows = [
        ("  󰔟  Session (5h)", &snap.session),
        ("  󰃰  Weekly", &snap.weekly),
        ("  󰸗  Monthly", &snap.monthly),
    ];
    let mut first = true;
    for (label, w) in windows {
        let Some(w) = w.as_ref() else { continue };
        if !first {
            lines.push(TooltipLine::Body("".into()));
        }
        first = false;
        push_window(&mut lines, label, w, theme, now);
    }
    if first {
        lines.push(TooltipLine::Body(format!(
            " <span foreground='{dim}'>no usage windows reported</span>"
        )));
    }

    if let Some((code, msg)) = outcome.last_error.as_ref()
        && *code != 0
    {
        let (icon, ecolor) = if *code >= 500 {
            ("󰅚", theme.red.as_str())
        } else {
            ("󰀪", theme.orange.as_str())
        };
        lines.push(TooltipLine::Body("".into()));
        lines.push(TooltipLine::Sep);
        lines.push(TooltipLine::Body(format!(
            " <span foreground='{ecolor}'>  {icon}  HTTP {code}</span>"
        )));
        lines.push(TooltipLine::Body(format!(
            "     <span foreground='{dim}'>{}</span>",
            escape(msg)
        )));
    }

    let updated = updated_at_hm(now, outcome.cache_age);
    lines.push(TooltipLine::Body("".into()));
    lines.push(TooltipLine::Sep);
    lines.push(TooltipLine::Body(format!(
        " <span foreground='{dim}'>  󰅐  Updated {updated}</span>"
    )));

    render_bordered(&lines, theme)
}

fn push_window(
    lines: &mut Vec<TooltipLine>,
    label: &str,
    w: &UsageWindow,
    theme: &Theme,
    now: DateTime<Utc>,
) {
    let color = severity_color(severity_for(w.utilization_pct), theme);
    let bar = pango::progress_bar(w.utilization_pct, color, theme, None);
    let fg = &theme.fg;
    let dim = &theme.dim;
    lines.push(TooltipLine::Body(format!(
        " <span foreground='{fg}'>{label}</span>"
    )));
    lines.push(TooltipLine::Body(format!(
        "   {bar}  <span font_weight='bold' foreground='{color}'>{pct}%</span>",
        pct = w.utilization_pct
    )));
    lines.push(TooltipLine::Body(format!(
        " <span foreground='{dim}'>  ⏱  Resets in {cd}</span>",
        cd = escape(&countdown::format(w.resets_at, now))
    )));
}

impl From<FetchOutcome> for VendorOutcome {
    fn from(o: FetchOutcome) -> Self {
        Self {
            snapshot: crate::usage::VendorSnapshot::Opencode(o.snapshot),
            stale: o.stale,
            last_error: o.last_error,
            cache_age: o.cache_age,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(pct: i32, dur: chrono::Duration) -> Option<UsageWindow> {
        Some(UsageWindow {
            utilization_pct: pct,
            resets_at: Some(Utc::now() + chrono::Duration::hours(2)),
            window_duration: dur,
        })
    }

    fn sample_snap() -> OpencodeSnapshot {
        OpencodeSnapshot {
            plan: "OpenCode Go".into(),
            session: window(42, chrono::Duration::hours(5)),
            weekly: window(15, chrono::Duration::days(7)),
            monthly: window(8, chrono::Duration::days(30)),
        }
    }

    fn outcome(s: OpencodeSnapshot) -> VendorOutcome {
        VendorOutcome {
            snapshot: crate::usage::VendorSnapshot::Opencode(s),
            stale: false,
            last_error: None,
            cache_age: Some(std::time::Duration::from_secs(10)),
        }
    }

    fn opts() -> RenderOpts {
        RenderOpts {
            format: None,
            tooltip_format: None,
            icon: None,
            pace_tolerance: 5,
            format_pace_color: false,
            tooltip_pace_pts: false,
        }
    }

    #[test]
    fn default_format_renders_session_pct() {
        let snap = sample_snap();
        let out = render(
            &outcome(snap.clone()),
            &snap,
            &Theme::default(),
            &opts(),
            Utc::now(),
        );
        assert!(out.text.contains("42%"));
    }

    #[test]
    fn tooltip_lists_all_three_windows() {
        let snap = sample_snap();
        let out = render(
            &outcome(snap.clone()),
            &snap,
            &Theme::default(),
            &opts(),
            Utc::now(),
        );
        assert!(out.tooltip.contains("OpenCode Go"));
        assert!(out.tooltip.contains("Session"));
        assert!(out.tooltip.contains("Weekly"));
        assert!(out.tooltip.contains("Monthly"));
    }

    #[test]
    fn empty_snapshot_renders_no_windows_message() {
        let snap = OpencodeSnapshot {
            plan: "OpenCode Go".into(),
            session: None,
            weekly: None,
            monthly: None,
        };
        let out = render(
            &outcome(snap.clone()),
            &snap,
            &Theme::default(),
            &opts(),
            Utc::now(),
        );
        assert!(out.tooltip.contains("no usage windows reported"));
    }

    #[test]
    fn severity_picks_worst_window() {
        let mut snap = sample_snap();
        snap.monthly.as_mut().unwrap().utilization_pct = 95;
        assert_eq!(severity(&snap), PaceSeverity::Critical);
    }

    #[test]
    fn custom_tooltip_uses_placeholders() {
        let snap = sample_snap();
        let mut o = opts();
        o.tooltip_format = Some(
            "S:{opencode_session_pct} W:{opencode_weekly_pct} M:{opencode_monthly_pct}".into(),
        );
        let out = render(
            &outcome(snap.clone()),
            &snap,
            &Theme::default(),
            &o,
            Utc::now(),
        );
        assert_eq!(out.tooltip, "S:42 W:15 M:8");
    }
}
