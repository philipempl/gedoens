use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use gedoens_application::ports::PolicyRepository;
use gedoens_application::{AppError, Result};
use gedoens_domain::{AccessPolicy, Area, DomainError, Recipient};
use regex::Regex;
use serde::Deserialize;

/// Liest und ändert die `.sops.yaml`.
///
/// Gelesen wird mit einem YAML-Parser, damit jede gültige Schreibweise verstanden
/// wird. Geschrieben wird gezielt im Text: Nur der `age`-Block der betroffenen
/// Regel wird ersetzt, alles andere bleibt Zeichen für Zeichen, wie es war.
/// Namen stehen als Kommentar hinter dem Key, also `- age1…  # Anna`.
pub struct SopsConfigFile;

#[derive(Deserialize)]
struct RawConfig {
    creation_rules: Option<Vec<RawRule>>,
}

#[derive(Deserialize)]
struct RawRule {
    path_regex: Option<String>,
    age: Option<Keys>,
    key_groups: Option<Vec<RawGroup>>,
}

#[derive(Deserialize)]
struct RawGroup {
    age: Option<Keys>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Keys {
    One(String),
    Many(Vec<String>),
}

impl Keys {
    fn recipients(&self) -> Vec<Recipient> {
        let parts: Vec<&str> = match self {
            Keys::One(text) => text.split(',').collect(),
            Keys::Many(items) => items.iter().flat_map(|item| item.split(',')).collect(),
        };
        parts.into_iter().filter_map(|part| Recipient::parse(part).ok()).collect()
    }
}

#[derive(Clone, Copy)]
struct RuleSpan {
    start: usize,
    end: usize,
    key_indent: usize,
}

#[derive(Clone, Copy)]
struct AgeBlock {
    start: usize,
    end: usize,
    key_indent: usize,
    /// Der Schlüssel `age:` steht in derselben Zeile wie der Spiegelstrich der Regel.
    on_item_line: bool,
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_filler(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

fn code(line: &str) -> &str {
    line.split('#').next().unwrap_or("")
}

/// Findet die Zeilenbereiche der einzelnen Regeln unter `creation_rules:`.
fn rule_spans(lines: &[&str]) -> Vec<RuleSpan> {
    let Some(head) = lines.iter().position(|l| indent(l) == 0 && code(l).trim_end() == "creation_rules:") else {
        return Vec::new();
    };
    let mut spans: Vec<RuleSpan> = Vec::new();
    let mut item_indent: Option<usize> = None;
    let mut section_end = lines.len();
    for (i, line) in lines.iter().enumerate().skip(head + 1) {
        if is_filler(line) {
            continue;
        }
        let depth = indent(line);
        let rest = &line[depth..];
        let is_item = rest == "-" || rest.starts_with("- ");
        if is_item && item_indent.map_or(true, |known| known == depth) {
            item_indent = Some(depth);
            spans.push(RuleSpan { start: i, end: lines.len(), key_indent: depth + 2 });
        } else if depth == 0 || item_indent.is_some_and(|known| depth < known) {
            section_end = i;
            break;
        }
    }
    for k in 0..spans.len() {
        spans[k].end = spans.get(k + 1).map_or(section_end, |next| next.start);
    }
    spans
}

/// Findet in einer Regel den Schlüssel `age:` samt allem, was zu ihm gehört.
fn age_block(lines: &[&str], span: RuleSpan) -> Option<AgeBlock> {
    for i in span.start..span.end {
        let line = lines[i];
        if is_filler(line) {
            continue;
        }
        let on_item_line = i == span.start;
        let (depth, text) = if on_item_line {
            (span.key_indent, line.trim_start().trim_start_matches('-').trim_start())
        } else {
            (indent(line), line.trim_start())
        };
        let is_age_key = depth == span.key_indent && code(text).trim_end().starts_with("age:");
        if !is_age_key {
            continue;
        }
        let mut end = i + 1;
        while end < span.end {
            let next = lines[end];
            let next_depth = indent(next);
            let deeper = next_depth > span.key_indent;
            let list_at_key_level = next_depth == span.key_indent && next.trim_start().starts_with("- ");
            if next.trim().is_empty() || !(deeper || list_at_key_level) {
                break;
            }
            end += 1;
        }
        return Some(AgeBlock { start: i, end, key_indent: span.key_indent, on_item_line });
    }
    None
}

fn render(block: AgeBlock, recipients: &[Recipient], names: &BTreeMap<Recipient, String>) -> Vec<String> {
    let pad = |n: usize| " ".repeat(n);
    let mut out = vec![if block.on_item_line {
        format!("{}- age:", pad(block.key_indent - 2))
    } else {
        format!("{}age:", pad(block.key_indent))
    }];
    for recipient in recipients {
        let comment = names.get(recipient).map(|name| format!("  # {name}")).unwrap_or_default();
        out.push(format!("{}- {}{}", pad(block.key_indent + 2), recipient.as_str(), comment));
    }
    out
}

fn names_in(text: &str) -> BTreeMap<Recipient, String> {
    static COMMENT: OnceLock<Regex> = OnceLock::new();
    static ANCHOR: OnceLock<Regex> = OnceLock::new();
    let comment = COMMENT.get_or_init(|| Regex::new(r"(age1[0-9a-z]{16,})\s*,?\s+#\s*(.+?)\s*$").expect("festes Muster"));
    let anchor = ANCHOR.get_or_init(|| Regex::new(r"&([A-Za-z0-9_.-]+)\s+(age1[0-9a-z]{16,})").expect("festes Muster"));

    let mut names = BTreeMap::new();
    for line in text.lines() {
        // Ein Anker wie `&anna age1…` ist ein Name. Ein Kommentar dahinter geht vor.
        if let Some(found) = anchor.captures(line) {
            if let Ok(recipient) = Recipient::parse(&found[2]) {
                names.insert(recipient, found[1].to_owned());
            }
        }
        if let Some(found) = comment.captures(line) {
            if let Ok(recipient) = Recipient::parse(&found[1]) {
                names.insert(recipient, found[2].to_owned());
            }
        }
    }
    names
}

fn parse(text: &str) -> Result<AccessPolicy> {
    let raw: RawConfig = serde_yaml_ng::from_str(text)
        .map_err(|e| AppError::Unsupported(format!("The .sops.yaml is not valid YAML: {e}")))?;
    let rules = raw.creation_rules.unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    let spans = rule_spans(&lines);
    let layout_known = spans.len() == rules.len();

    let areas = rules
        .iter()
        .enumerate()
        .map(|(index, rule)| {
            let groups = rule.key_groups.as_deref().unwrap_or_default();
            let recipients = match &rule.age {
                Some(keys) if groups.is_empty() => keys.recipients(),
                _ => groups.iter().filter_map(|g| g.age.as_ref()).flat_map(Keys::recipients).collect(),
            };
            let plain_block = layout_known
                && age_block(&lines, spans[index])
                    .is_some_and(|b| !lines[b.start..b.end].iter().any(|l| code(l).contains(['*', '&'])));
            Area {
                index,
                pattern: rule.path_regex.clone(),
                recipients,
                editable: rule.age.is_some() && groups.is_empty() && plain_block,
            }
        })
        .collect();
    Ok(AccessPolicy { areas, names: names_in(text) })
}

fn tidy_name(name: &str) -> Result<String> {
    let tidy: String = name.chars().map(|c| if c.is_control() || c == '#' { ' ' } else { c }).collect();
    let tidy = tidy.split_whitespace().collect::<Vec<_>>().join(" ");
    if tidy.is_empty() {
        Err(AppError::Unsupported("Enter a name.".into()))
    } else {
        Ok(tidy)
    }
}

fn locked() -> AppError {
    AppError::Unsupported("This rule uses key groups or YAML aliases. Please change it by hand in the .sops.yaml.".into())
}

impl SopsConfigFile {
    /// Liest, lässt die Änderung auf dem Modell ausführen und schreibt genau die
    /// Blöcke der Bereiche neu, die die Änderung nennt.
    fn edit(&self, path: &Path, change: impl FnOnce(&mut AccessPolicy) -> Result<Vec<usize>>) -> Result<()> {
        let text = fs::read_to_string(path)?;
        let mut policy = parse(&text)?;
        let touched = change(&mut policy)?;

        let mut blocks: Vec<(AgeBlock, usize)> = {
            let lines: Vec<&str> = text.lines().collect();
            let spans = rule_spans(&lines);
            let mut found = Vec::new();
            for area in touched {
                let span = *spans.get(area).ok_or_else(locked)?;
                found.push((age_block(&lines, span).ok_or_else(locked)?, area));
            }
            found
        };
        // Von unten nach oben ersetzen, damit die Zeilennummern darüber gültig bleiben.
        blocks.sort_by_key(|(block, _)| std::cmp::Reverse(block.start));
        blocks.dedup_by_key(|(block, _)| block.start);

        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        for (block, area) in blocks {
            let fresh = render(block, &policy.areas[area].recipients, &policy.names);
            lines.splice(block.start..block.end, fresh);
        }
        let mut out = lines.join("\n");
        if text.ends_with('\n') {
            out.push('\n');
        }
        Ok(fs::write(path, out)?)
    }
}

fn editable_areas_with(policy: &AccessPolicy, recipient: &Recipient) -> Vec<usize> {
    policy.areas.iter().filter(|a| a.editable && a.recipients.contains(recipient)).map(|a| a.index).collect()
}

impl PolicyRepository for SopsConfigFile {
    fn load(&self, path: &Path) -> Result<AccessPolicy> {
        parse(&fs::read_to_string(path)?)
    }

    fn add_recipient(&self, path: &Path, area: usize, recipient: &Recipient, name: Option<&str>) -> Result<()> {
        self.edit(path, |policy| {
            let target = policy.areas.get_mut(area).filter(|a| a.editable).ok_or_else(locked)?;
            if !target.recipients.contains(recipient) {
                target.recipients.push(recipient.clone());
            }
            if let Some(name) = name {
                policy.names.insert(recipient.clone(), tidy_name(name)?);
            }
            Ok(editable_areas_with(policy, recipient))
        })
    }

    fn remove_recipient(&self, path: &Path, area: usize, recipient: &Recipient) -> Result<()> {
        self.edit(path, |policy| {
            let target = policy.areas.get_mut(area).filter(|a| a.editable).ok_or_else(locked)?;
            target.recipients.retain(|r| r != recipient);
            if target.recipients.is_empty() {
                return Err(DomainError::EmptyArea.into());
            }
            Ok(vec![area])
        })
    }

    fn rename(&self, path: &Path, recipient: &Recipient, name: &str) -> Result<()> {
        self.edit(path, |policy| {
            policy.names.insert(recipient.clone(), tidy_name(name)?);
            let areas = editable_areas_with(policy, recipient);
            if areas.is_empty() {
                return Err(AppError::Unsupported(
                    "Dieser Key steht in keinem Bereich der .sops.yaml. Einen Namen kann er erst dort bekommen.".into(),
                ));
            }
            Ok(areas)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "age1aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "age1bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const C: &str = "age1cccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn key(text: &str) -> Recipient {
        Recipient::parse(text).unwrap()
    }

    fn sample() -> String {
        format!(
            "# Wer darf was lesen\ncreation_rules:\n  # Produktion\n  - path_regex: secrets/prod/.*\\.yaml$\n    encrypted_regex: ^(data)$\n    age:\n      - {A}  # Du\n      - {B}\n  - path_regex: secrets/dev/.*\n    age: {A},{B}\n  - age: >-\n      {A},\n      {B}\nstores:\n  yaml:\n    indent: 2\n"
        )
    }

    fn file_with(text: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join(".sops.yaml");
        fs::write(&path, text).unwrap();
        (folder, path)
    }

    #[test]
    fn reads_all_three_ways_to_list_keys_and_the_names() {
        let policy = parse(&sample()).unwrap();
        assert_eq!(policy.areas.len(), 3);
        for area in &policy.areas {
            assert_eq!(area.recipients, vec![key(A), key(B)]);
            assert!(area.editable);
        }
        assert_eq!(policy.areas[2].pattern, None);
        assert_eq!(policy.name_of(&key(A)), Some("Du"));
        assert_eq!(policy.name_of(&key(B)), None);
    }

    #[test]
    fn adding_someone_touches_only_that_block() {
        let (_keep, path) = file_with(&sample());
        SopsConfigFile.add_recipient(&path, 0, &key(C), Some("Anna")).unwrap();
        let text = fs::read_to_string(&path).unwrap();

        let expected = sample().replace(&format!("      - {B}\n"), &format!("      - {B}\n      - {C}  # Anna\n"));
        assert_eq!(text, expected);
        assert_eq!(parse(&text).unwrap().areas[0].recipients, vec![key(A), key(B), key(C)]);
    }

    #[test]
    fn inline_and_folded_lists_become_plain_lists() {
        let (_keep, path) = file_with(&sample());
        SopsConfigFile.remove_recipient(&path, 1, &key(B)).unwrap();
        SopsConfigFile.add_recipient(&path, 2, &key(C), None).unwrap();
        let text = fs::read_to_string(&path).unwrap();

        assert!(text.contains(&format!("  - path_regex: secrets/dev/.*\n    age:\n      - {A}  # Du\n  - age:\n      - {A}  # Du\n      - {B}\n      - {C}\nstores:")));
        let policy = parse(&text).unwrap();
        assert_eq!(policy.areas[1].recipients, vec![key(A)]);
        assert_eq!(policy.areas[2].recipients, vec![key(A), key(B), key(C)]);
    }

    #[test]
    fn a_name_is_written_wherever_the_key_stands() {
        let (_keep, path) = file_with(&sample());
        SopsConfigFile.rename(&path, &key(B), "CI # Runner").unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches(&format!("- {B}  # CI Runner")).count(), 3);
        assert!(SopsConfigFile.rename(&path, &key(C), "Niemand").is_err());
    }

    #[test]
    fn the_last_reader_cannot_be_removed() {
        let (_keep, path) = file_with(&format!("creation_rules:\n- age: {A}\n"));
        let result = SopsConfigFile.remove_recipient(&path, 0, &key(A));
        assert!(matches!(result, Err(AppError::Domain(DomainError::EmptyArea))));
    }

    #[test]
    fn key_groups_and_aliases_are_shown_but_not_edited() {
        let text = format!(
            "keys:\n  - &anna {B}\ncreation_rules:\n  - path_regex: a/.*\n    key_groups:\n      - age:\n          - {A}\n          - *anna\n  - path_regex: b/.*\n    age:\n      - *anna\n"
        );
        let policy = parse(&text).unwrap();
        assert_eq!(policy.areas[0].recipients, vec![key(A), key(B)]);
        assert_eq!(policy.areas[1].recipients, vec![key(B)]);
        assert!(!policy.areas[0].editable && !policy.areas[1].editable);
        assert_eq!(policy.name_of(&key(B)), Some("anna"));

        let (_keep, path) = file_with(&text);
        assert!(SopsConfigFile.add_recipient(&path, 1, &key(C), None).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
}
