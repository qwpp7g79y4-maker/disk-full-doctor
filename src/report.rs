//! Findings and the text / JSON report.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Outcome of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Ok,
    Warn,
    NeedsRoot,
    NotPresent,
    Error,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "Ok",
            Self::Warn => "Warn",
            Self::NeedsRoot => "NeedsRoot",
            Self::NotPresent => "NotPresent",
            Self::Error => "Error",
        }
    }

    /// Warn, NeedsRoot, and Error are the results that set exit code 1.
    pub fn is_actionable(self) -> bool {
        matches!(self, Self::Warn | Self::NeedsRoot | Self::Error)
    }
}

/// One row in the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub check: String,
    pub status: Status,
    pub bytes: Option<u64>,
    pub inodes: Option<u64>,
    pub evidence: String,
    pub hint: String,
}

impl Finding {
    pub fn new(
        check: impl Into<String>,
        status: Status,
        bytes: Option<u64>,
        inodes: Option<u64>,
        evidence: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            check: check.into(),
            status,
            bytes,
            inodes,
            evidence: evidence.into(),
            hint: hint.into(),
        }
    }
}

/// Collected findings, sorted by bytes descending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub findings: Vec<Finding>,
    /// Checks that hit `NeedsRoot` at least once, sorted and deduplicated.
    pub needs_root: Vec<String>,
}

impl Report {
    pub fn from_findings(mut findings: Vec<Finding>) -> Self {
        findings.sort_by(cmp_findings);
        let mut needs_root: Vec<String> = findings
            .iter()
            .filter(|finding| finding.status == Status::NeedsRoot)
            .map(|finding| finding.check.clone())
            .collect();
        needs_root.sort();
        needs_root.dedup();
        Self {
            findings,
            needs_root,
        }
    }

    /// `0` when nothing actionable was found, `1` when something was.
    pub fn exit_code(&self) -> i32 {
        if self
            .findings
            .iter()
            .any(|finding| finding.status.is_actionable())
        {
            1
        } else {
            0
        }
    }

    pub fn render_text(&self) -> String {
        let mut out = String::from("disk-full-doctor\n");
        if self.findings.is_empty() {
            out.push_str("\nNo findings.\n");
            return out;
        }
        out.push('\n');
        for finding in &self.findings {
            let bytes = match finding.bytes {
                Some(value) => value.to_string(),
                None => "-".to_string(),
            };
            let inodes = match finding.inodes {
                Some(value) => value.to_string(),
                None => "-".to_string(),
            };
            out.push_str(&format!(
                "[{status}] {check} bytes={bytes} inodes={inodes}\nevidence: {evidence}\nhint: {hint}\n\n",
                status = finding.status.as_str(),
                check = escape(&finding.check),
                evidence = escape(&finding.evidence),
                hint = escape(&finding.hint),
            ));
        }
        if !self.needs_root.is_empty() {
            out.push_str(&format!(
                "Run as root to see more: {} need root to read everything.\n",
                self.needs_root.join(", ")
            ));
        }
        out
    }

    pub fn render_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("report values are json-safe");
        text.push('\n');
        text
    }
}

fn cmp_findings(a: &Finding, b: &Finding) -> Ordering {
    match (a.bytes, b.bytes) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
    .then_with(|| a.check.cmp(&b.check))
    .then_with(|| a.evidence.cmp(&b.evidence))
}

/// Escape control characters so one path cannot break the text report.
pub fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if other.is_control() => {
                out.push_str(&format!("\\u{{{:x}}}", u32::from(other)));
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_is_text_and_json() {
        let report = Report::from_findings(Vec::new());
        assert_eq!(report.render_text(), "disk-full-doctor\n\nNo findings.\n");
        assert_eq!(report.exit_code(), 0);
        let value: serde_json::Value = serde_json::from_str(&report.render_json()).expect("json");
        assert!(value["findings"].as_array().expect("array").is_empty());
    }

    #[test]
    fn needs_root_renders_in_text_and_json() {
        let report = Report::from_findings(vec![Finding::new(
            "deleted_open",
            Status::NeedsRoot,
            None,
            None,
            "cannot read /proc/1/fd",
            "run with sudo to see deleted-but-open files",
        )]);
        let text = report.render_text();
        assert!(text.contains("[NeedsRoot] deleted_open"));
        assert!(text.contains("run with sudo to see deleted-but-open files"));
        assert!(text.contains("Run as root to see more: deleted_open need root"));
        assert_eq!(report.exit_code(), 1);
        assert_eq!(report.needs_root, vec!["deleted_open".to_string()]);
        let value: serde_json::Value = serde_json::from_str(&report.render_json()).expect("json");
        assert_eq!(value["findings"][0]["status"], "NeedsRoot");
        assert!(value["findings"][0]["hint"]
            .as_str()
            .expect("hint")
            .contains("run with sudo to see"));
        assert_eq!(value["needs_root"][0], "deleted_open");
    }

    #[test]
    fn needs_root_summary_lists_each_check_once() {
        let report = Report::from_findings(vec![
            Finding::new("inodes", Status::NeedsRoot, None, None, "a", "hint a"),
            Finding::new("inodes", Status::NeedsRoot, None, None, "b", "hint b"),
            Finding::new("buildkit", Status::NeedsRoot, None, None, "c", "hint c"),
            Finding::new("docker_logs", Status::Ok, Some(1), None, "d", "hint d"),
        ]);
        assert_eq!(
            report.needs_root,
            vec!["buildkit".to_string(), "inodes".to_string()]
        );
        assert!(report
            .render_text()
            .contains("Run as root to see more: buildkit, inodes need root"));
    }

    #[test]
    fn text_sorts_by_bytes_descending() {
        let report = Report::from_findings(vec![
            Finding::new("small", Status::Warn, Some(10), None, "a", ""),
            Finding::new("none", Status::Ok, None, None, "c", ""),
            Finding::new("large", Status::Warn, Some(50), None, "b", ""),
        ]);
        let checks: Vec<_> = report
            .findings
            .iter()
            .map(|finding| finding.check.as_str())
            .collect();
        assert_eq!(checks, ["large", "small", "none"]);
    }

    #[test]
    fn text_escapes_control_characters() {
        let report = Report::from_findings(vec![Finding::new(
            "deleted_open",
            Status::Warn,
            Some(1),
            None,
            "pid 1 /tmp/a\nb\tname",
            "hint",
        )]);
        let text = report.render_text();
        assert!(text.contains("evidence: pid 1 /tmp/a\\nb\\tname\n"));
        assert!(!text.contains("evidence: pid 1 /tmp/a\nb"));
    }

    #[test]
    fn text_escapes_backslash_cr_and_other_controls() {
        let report = Report::from_findings(vec![Finding::new(
            "journald",
            Status::Error,
            None,
            Some(2),
            "cannot read\r\n\\x",
            "hint\u{1}",
        )]);
        let text = report.render_text();
        assert!(text.contains("[Error] journald bytes=- inodes=2\n"));
        assert!(text.contains("evidence: cannot read\\r\\n\\\\x\n"));
        assert!(text.contains("hint: hint\\u{1}\n"));
        assert_eq!(report.exit_code(), 1);
        assert!(report.needs_root.is_empty());
        assert!(!text.contains("Run as root"));
    }

    #[test]
    fn ok_and_not_present_leave_exit_code_0() {
        let report = Report::from_findings(vec![
            Finding::new("journald", Status::Ok, Some(1), Some(2), "ok", "hint"),
            Finding::new(
                "buildkit",
                Status::NotPresent,
                None,
                None,
                "missing",
                "hint",
            ),
        ]);
        assert_eq!(report.exit_code(), 0);
        let text = report.render_text();
        assert!(text.contains("[Ok] journald bytes=1 inodes=2\n"));
        assert!(text.contains("[NotPresent] buildkit bytes=- inodes=-\n"));
        assert!(!text.contains("Run as root"));
    }

    #[test]
    fn equal_bytes_sort_by_check_then_evidence() {
        let report = Report::from_findings(vec![
            Finding::new("b", Status::Ok, Some(5), None, "z", ""),
            Finding::new("a", Status::Ok, Some(5), None, "m", ""),
            Finding::new("a", Status::Ok, Some(5), None, "a", ""),
            Finding::new("m", Status::NotPresent, None, None, "b", ""),
            Finding::new("m", Status::NotPresent, None, None, "a", ""),
        ]);
        let keys: Vec<_> = report
            .findings
            .iter()
            .map(|finding| (finding.check.as_str(), finding.evidence.as_str()))
            .collect();
        assert_eq!(
            keys,
            [("a", "a"), ("a", "m"), ("b", "z"), ("m", "a"), ("m", "b")]
        );
    }
}
