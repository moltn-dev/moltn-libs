//! Version constraints in npm format (`^20 || >=22`, `24.x`, `v24.1.0`…).

use std::fmt;

use semver::{Version, VersionReq};
use serde::Serialize;

/// A version constraint as written in the project, and its parsed form.
///
/// Aliases that Node understands but that are not ranges (`lts/*`, `latest`)
/// are kept as they are and not checked.
///
/// ```
/// use moltn_toolchain::Requirement;
/// let req = Requirement::parse("^20 || >=22");
/// assert_eq!(req.matches(&"22.3.0".parse().unwrap()), Some(true));
/// assert_eq!(req.matches(&"21.0.0".parse().unwrap()), Some(false));
/// assert_eq!(Requirement::parse("lts/*").matches(&"24.0.0".parse().unwrap()), None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Requirement {
    raw: String,
    #[serde(skip)]
    alternatives: Option<Vec<VersionReq>>,
}

impl Requirement {
    /// Parse an npm constraint; never fails (at worst, the constraint cannot be checked).
    pub fn parse(raw: &str) -> Self {
        let raw = raw.trim().to_string();
        let alternatives = raw.split("||").map(alternative).collect::<Option<Vec<_>>>();
        Self { raw, alternatives }
    }

    /// Original text.
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// `Some(true|false)` if the constraint can be checked, `None` otherwise (`lts/*`…).
    pub fn matches(&self, version: &Version) -> Option<bool> {
        self.alternatives
            .as_ref()
            .map(|alts| alts.iter().any(|req| req.matches(version)))
    }

    /// Short version for a message (`24` for `>=24.1 <25`), otherwise the original text.
    pub fn hint(&self) -> String {
        let first = self.raw.split("||").next().unwrap_or_default();
        let digits: String = first
            .trim_start_matches(|c: char| !c.is_ascii_digit())
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if digits.is_empty() {
            self.raw.clone()
        } else {
            digits
        }
    }
}

impl fmt::Display for Requirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

/// One npm alternative (`>=18 <20`, `1.2.3 - 2`, `24.x`) to a `VersionReq` (Cargo syntax).
fn alternative(text: &str) -> Option<VersionReq> {
    let text = text.trim();
    if text.is_empty() || text == "*" || text.eq_ignore_ascii_case("x") {
        return Some(VersionReq::STAR);
    }
    let comparators: Vec<String> = match text.split_once(" - ") {
        Some((low, high)) => vec![
            format!(">={}", partial(low)?),
            format!("<={}", partial(high)?),
        ],
        None => text
            .split_whitespace()
            .map(comparator)
            .collect::<Option<_>>()?,
    };
    VersionReq::parse(&comparators.join(", ")).ok()
}

/// `>=24`, `^1.2`, `24.x`, `v24.1.0` (exact in npm, hence `=`).
fn comparator(text: &str) -> Option<String> {
    let op_len = text.find(|c: char| c.is_ascii_digit() || c == 'v' || c == 'x' || c == '*')?;
    let (op, version) = text.split_at(op_len);
    let version = partial(version)?;
    let full = version.split('.').count() == 3 && !version.contains('*');
    Some(match op {
        "" if full => format!("={version}"),
        "" => version,
        _ => format!("{op}{version}"),
    })
}

/// Drop the `v`, turn `x` into `*`; reject aliases (`lts/*`, `node`).
fn partial(text: &str) -> Option<String> {
    let text = text.trim().trim_start_matches('v');
    let parts: Vec<&str> = text.split('.').collect();
    let valid = !parts.is_empty()
        && parts.len() <= 3
        && parts.iter().all(|p| {
            matches!(*p, "x" | "X" | "*") || p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty()
        });
    valid.then(|| {
        parts
            .iter()
            .map(|p| if p.eq_ignore_ascii_case("x") { "*" } else { p })
            .collect::<Vec<_>>()
            .join(".")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(req: &str, version: &str) -> Option<bool> {
        Requirement::parse(req).matches(&version.parse().unwrap())
    }

    #[test]
    fn npm_forms() {
        assert_eq!(ok("24", "24.9.1"), Some(true));
        assert_eq!(ok("24", "25.0.0"), Some(false));
        assert_eq!(ok("24.x", "24.0.0"), Some(true));
        assert_eq!(ok("v24.1.0", "24.1.0"), Some(true));
        assert_eq!(
            ok("24.1.0", "24.2.0"),
            Some(false),
            "a full version is exact, as in npm"
        );
        assert_eq!(ok(">=18 <20", "19.1.0"), Some(true));
        assert_eq!(ok(">=18 <20", "20.0.0"), Some(false));
        assert_eq!(ok("^20 || >=22", "22.0.0"), Some(true));
        assert_eq!(ok("^20 || >=22", "21.5.0"), Some(false));
        assert_eq!(ok("18 - 20", "20.4.0"), Some(true));
        assert_eq!(ok("*", "1.0.0"), Some(true));
        assert_eq!(ok("lts/*", "24.0.0"), None);
        assert_eq!(ok("node", "24.0.0"), None);
    }

    #[test]
    fn hint_is_major() {
        assert_eq!(Requirement::parse(">=24.1 <25").hint(), "24");
        assert_eq!(Requirement::parse("^20 || >=22").hint(), "20");
        assert_eq!(Requirement::parse("lts/*").hint(), "lts/*");
    }
}
