use url::Url;

use super::rule::{default_port_for_scheme, CredentialRule};

/// Pure domain credential matcher for URL site/path matching with strict boundaries.
#[derive(Clone, Debug, Default)]
pub struct CredentialMatcher {
    rules: Vec<CredentialRule>,
}

impl CredentialMatcher {
    pub fn new(rules: Vec<CredentialRule>) -> Self {
        Self { rules }
    }

    pub fn rules(&self) -> &[CredentialRule] {
        &self.rules
    }

    /// Finds the best matching credential rule for a target URL using deterministic longest-prefix matching.
    pub fn match_url<'a>(&'a self, url: &Url) -> Option<&'a CredentialRule> {
        let scheme = url.scheme().to_ascii_lowercase();
        let host = url.host_str()?.to_ascii_lowercase();
        let raw_port = url.port();
        let default_port = default_port_for_scheme(&scheme);
        let candidate_port = match raw_port {
            Some(p) if Some(p) == default_port => None,
            other => other,
        };

        let candidate_path = url.path();
        let norm_candidate_path = if candidate_path.is_empty() {
            "/"
        } else {
            candidate_path
        };

        let mut matches: Vec<&'a CredentialRule> = self
            .rules
            .iter()
            .filter(|rule| {
                // 1. Scheme match
                if rule.scheme != scheme {
                    return false;
                }

                // 2. Host match (case-insensitive exact)
                if rule.host != host {
                    return false;
                }

                // 3. Port match (None matches default port or unspecified port)
                let port_matches = match (rule.port, candidate_port) {
                    (None, None) => true,
                    (Some(rp), Some(cp)) => rp == cp,
                    (Some(rp), None) => Some(rp) == default_port,
                    (None, Some(cp)) => default_port == Some(cp),
                };
                if !port_matches {
                    return false;
                }

                // 4. Strict path boundary match
                path_prefix_matches(&rule.path_prefix, norm_candidate_path)
            })
            .collect();

        if matches.is_empty() {
            return None;
        }

        // Sort by path prefix length descending (longest-prefix wins)
        // If tied on length, rule with explicit port takes precedence over None
        matches.sort_by(|a, b| {
            b.path_prefix
                .len()
                .cmp(&a.path_prefix.len())
                .then_with(|| b.port.is_some().cmp(&a.port.is_some()))
                .then_with(|| a.id.cmp(&b.id))
        });

        matches.first().copied()
    }

    /// Detects duplicate rules sharing identical (scheme, host, port, path_prefix).
    pub fn detect_collisions(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        for (i, rule1) in self.rules.iter().enumerate() {
            for rule2 in self.rules.iter().skip(i + 1) {
                if rule1.signature() == rule2.signature() {
                    warnings.push(format!(
                        "Rule collision detected: '{}' and '{}' both match {}://{}:{}/{}",
                        rule1.id,
                        rule2.id,
                        rule1.scheme,
                        rule1.host,
                        rule1.port.unwrap_or_else(|| default_port_for_scheme(&rule1.scheme).unwrap_or(0)),
                        rule1.path_prefix
                    ));
                }
            }
        }
        warnings
    }
}

/// Evaluates whether a candidate path satisfies a normalized rule path prefix under strict segment boundary rules.
pub fn path_prefix_matches(rule_prefix: &str, candidate_path: &str) -> bool {
    if rule_prefix == "/" {
        return true;
    }

    // Both rule_prefix and candidate_path start with '/'
    let rule_without_trailing = rule_prefix.trim_end_matches('/');

    // Exact segment match: "/team" matches "/team/" or "/team"
    if candidate_path == rule_without_trailing || candidate_path == rule_prefix {
        return true;
    }

    // Sub-segment match: candidate must start with rule_prefix (e.g. "/team/doc.pdf" starts with "/team/")
    // This strictly prevents "/team-private" or "/teambuilding" from matching "/team/"
    candidate_path.starts_with(rule_prefix)
}

/// Formats HTTP Basic Authentication header value: "Basic <base64(user:secret)>".
pub fn format_basic_auth_header(username: &str, secret: &str) -> String {
    use base64::Engine;
    let raw = format!("{username}:{secret}");
    let encoded = base64::engine::general_purpose::STANDARD.encode(raw.as_bytes());
    format!("Basic {encoded}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_path_boundary_matching() {
        let rule_prefix = "/team/";
        assert!(path_prefix_matches(rule_prefix, "/team/"));
        assert!(path_prefix_matches(rule_prefix, "/team"));
        assert!(path_prefix_matches(rule_prefix, "/team/doc.pdf"));
        assert!(path_prefix_matches(rule_prefix, "/team/sub/doc.pdf"));

        // Must NEVER match adjacent segments or non-boundary prefixes
        assert!(!path_prefix_matches(rule_prefix, "/team-private/doc.pdf"));
        assert!(!path_prefix_matches(rule_prefix, "/teambuilding/event.ics"));
        assert!(!path_prefix_matches(rule_prefix, "/other/team/doc.pdf"));
    }

    #[test]
    fn test_root_path_matches_all() {
        assert!(path_prefix_matches("/", "/"));
        assert!(path_prefix_matches("/", "/anything"));
        assert!(path_prefix_matches("/", "/team/doc.pdf"));
    }

    #[test]
    fn test_longest_prefix_matching_precedence() {
        let rule_general = CredentialRule::new(
            "general",
            "https",
            "example.com",
            None,
            "/",
            "general_user",
            "pass1",
        )
        .unwrap();

        let rule_team = CredentialRule::new(
            "team",
            "https",
            "example.com",
            None,
            "/team/",
            "team_user",
            "pass2",
        )
        .unwrap();

        let rule_project = CredentialRule::new(
            "project",
            "https",
            "example.com",
            None,
            "/team/project/",
            "project_lead",
            "pass3",
        )
        .unwrap();

        let matcher = CredentialMatcher::new(vec![
            rule_general.clone(),
            rule_team.clone(),
            rule_project.clone(),
        ]);

        // Deepest path prefix wins
        let url_project = Url::parse("https://example.com/team/project/specs.pdf").unwrap();
        let matched = matcher.match_url(&url_project).unwrap();
        assert_eq!(matched.id, "project");
        assert_eq!(matched.username, "project_lead");

        // Intermediate path prefix wins
        let url_team = Url::parse("https://example.com/team/general.pdf").unwrap();
        let matched = matcher.match_url(&url_team).unwrap();
        assert_eq!(matched.id, "team");
        assert_eq!(matched.username, "team_user");

        // Adjacent path falls back to root
        let url_team_private = Url::parse("https://example.com/team-private/data.pdf").unwrap();
        let matched = matcher.match_url(&url_team_private).unwrap();
        assert_eq!(matched.id, "general");
        assert_eq!(matched.username, "general_user");

        // Unmatched host returns None
        let url_other_host = Url::parse("https://other.com/team/doc.pdf").unwrap();
        assert!(matcher.match_url(&url_other_host).is_none());
    }

    #[test]
    fn test_port_and_scheme_matching() {
        let rule_ftp = CredentialRule::new(
            "ftp-rule",
            "ftp",
            "ftp.example.com",
            Some(2121),
            "/",
            "ftpuser",
            "ftppass",
        )
        .unwrap();

        let matcher = CredentialMatcher::new(vec![rule_ftp]);

        // Matching port
        let url_ok = Url::parse("ftp://ftp.example.com:2121/files/archive.tar.gz").unwrap();
        assert_eq!(matcher.match_url(&url_ok).unwrap().id, "ftp-rule");

        // Non-matching port
        let url_bad_port = Url::parse("ftp://ftp.example.com:21/files/archive.tar.gz").unwrap();
        assert!(matcher.match_url(&url_bad_port).is_none());

        // Non-matching scheme
        let url_bad_scheme = Url::parse("http://ftp.example.com:2121/files/archive.tar.gz").unwrap();
        assert!(matcher.match_url(&url_bad_scheme).is_none());
    }

    #[test]
    fn test_collision_detection() {
        let r1 = CredentialRule::new("r1", "https", "example.com", None, "/team/", "u1", "p1").unwrap();
        let r2 = CredentialRule::new("r2", "https", "example.com", None, "/team", "u2", "p2").unwrap();
        let matcher = CredentialMatcher::new(vec![r1, r2]);
        let collisions = matcher.detect_collisions();
        assert_eq!(collisions.len(), 1);
        assert!(collisions[0].contains("Rule collision detected"));
    }
}
