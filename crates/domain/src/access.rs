use std::collections::BTreeMap;

use regex::Regex;

use crate::Recipient;

/// Ein Bereich: eine Regel aus der `.sops.yaml`. Sie sagt, wer die Dateien
/// lesen darf, auf die ihr Pfadmuster passt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Area {
    /// Position in der Datei. Die Reihenfolge zählt, die erste passende Regel gewinnt.
    pub index: usize,
    pub pattern: Option<String>,
    pub recipients: Vec<Recipient>,
    /// Falsch, wenn die Regel Key Groups oder YAML-Verweise nutzt. Solche Regeln
    /// zeigt die App an, ändert sie aber nicht.
    pub editable: bool,
}

impl Area {
    pub fn matches(&self, relative_path: &str) -> bool {
        match &self.pattern {
            None => true,
            Some(pattern) => Regex::new(pattern).map(|re| re.is_match(relative_path)).unwrap_or(false),
        }
    }

    /// Short labels for people, derived from the path pattern. Nothing is stored.
    ///
    /// `environments/prod/api\.yaml$` gives folder `prod` and file `api`,
    /// `environments/prod/.*\.yaml$` gives folder `prod` and no single file.
    pub fn label(&self) -> AreaLabel {
        let Some(pattern) = &self.pattern else {
            return AreaLabel { folder: None, file: None };
        };
        let plain = pattern.trim_start_matches('^').trim_end_matches('$').replace("\\.", ".");
        let segments: Vec<&str> = plain.split('/').collect();
        let word = |part: &str| -> Option<String> {
            let stem = part.split('.').next().unwrap_or("");
            let is_word = !stem.is_empty() && stem.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');
            let is_plain_part = part.chars().all(|c| c.is_alphanumeric() || "._-".contains(c));
            (is_word && is_plain_part).then(|| stem.to_owned())
        };
        let (last, before) = segments.split_last().expect("split never yields nothing");
        let folder = || before.iter().rev().find_map(|part| word(part));
        match word(last) {
            Some(file) if before.is_empty() => AreaLabel { folder: None, file: Some(file) },
            Some(file) => AreaLabel { folder: folder(), file: Some(file) },
            None => match folder() {
                Some(folder) => AreaLabel { folder: Some(folder), file: None },
                None => AreaLabel { folder: None, file: Some(pattern.clone()) },
            },
        }
    }

    /// One line for people: `prod/api`, `prod` or `everything`.
    pub fn display_name(&self) -> String {
        match self.label() {
            AreaLabel { folder: Some(folder), file: Some(file) } => format!("{folder}/{file}"),
            AreaLabel { folder: Some(only), file: None } | AreaLabel { folder: None, file: Some(only) } => only,
            AreaLabel { folder: None, file: None } => "everything".to_owned(),
        }
    }
}

/// What an area covers, in words: a folder, a single file in it, or both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaLabel {
    pub folder: Option<String>,
    pub file: Option<String>,
}

/// Die ganze `.sops.yaml` aus fachlicher Sicht: Bereiche und die Namen der Keys.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccessPolicy {
    pub areas: Vec<Area>,
    pub names: BTreeMap<Recipient, String>,
    /// Names the file hints at without stating them per key, for example a comment
    /// above a rule that lists its owners in the order of the keys. Offered, never assumed.
    pub hints: BTreeMap<Recipient, String>,
}

impl AccessPolicy {
    pub fn area_for(&self, relative_path: &str) -> Option<&Area> {
        self.areas.iter().find(|area| area.matches(relative_path))
    }

    pub fn name_of(&self, recipient: &Recipient) -> Option<&str> {
        self.names.get(recipient).map(String::as_str)
    }

    /// A likely name for a key that has none yet.
    pub fn hint_for(&self, recipient: &Recipient) -> Option<&str> {
        if self.names.contains_key(recipient) {
            return None;
        }
        self.hints.get(recipient).map(String::as_str)
    }

    pub fn recipients(&self) -> Vec<Recipient> {
        let mut all: Vec<Recipient> = Vec::new();
        for recipient in self.areas.iter().flat_map(|a| &a.recipients) {
            if !all.contains(recipient) {
                all.push(recipient.clone());
            }
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(index: usize, pattern: Option<&str>) -> Area {
        Area { index, pattern: pattern.map(str::to_owned), recipients: vec![], editable: true }
    }

    #[test]
    fn first_matching_rule_wins() {
        let policy = AccessPolicy {
            areas: vec![
                area(0, Some(r"secrets/prod/payments\.yaml$")),
                area(1, Some(r"secrets/prod/.*\.yaml$")),
                area(2, None),
            ],
            names: BTreeMap::new(),
            hints: BTreeMap::new(),
        };
        assert_eq!(policy.area_for("secrets/prod/payments.yaml").unwrap().index, 0);
        assert_eq!(policy.area_for("secrets/prod/api.yaml").unwrap().index, 1);
        assert_eq!(policy.area_for("README.md").unwrap().index, 2);
    }

    #[test]
    fn derives_a_readable_name_from_the_pattern() {
        assert_eq!(area(0, Some(r"secrets/prod/.*\.yaml$")).display_name(), "prod");
        assert_eq!(area(0, Some(r"secrets/dev/.*")).display_name(), "dev");
        assert_eq!(area(0, Some(r"secrets/prod/payments\.yaml$")).display_name(), "prod/payments");
        assert_eq!(area(0, Some(r"payments\.yaml$")).display_name(), "payments");
        assert_eq!(area(0, Some(r"\.prod\.yaml$")).display_name(), r"\.prod\.yaml$");
        assert_eq!(area(0, None).display_name(), "everything");
    }

    #[test]
    fn same_file_names_in_different_folders_stay_apart() {
        let a = area(0, Some(r"environments/acme/production\.yaml$")).label();
        let b = area(1, Some(r"environments/globex/production\.yaml$")).label();
        assert_eq!(a, AreaLabel { folder: Some("acme".into()), file: Some("production".into()) });
        assert_ne!(a, b);
    }
}
