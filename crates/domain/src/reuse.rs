use std::collections::BTreeMap;

use crate::{Scalar, SecretPath, Segment};

/// Shorter texts are mostly settings (`true`, `prod`, `1Gi`), not secrets.
pub const MIN_SECRET_LENGTH: usize = 8;

/// One value that turns up in more than one file of the same folder, for instance
/// the same password in `athenyx/preview.yaml` and `athenyx/production.yaml`.
/// The value itself is not kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedValue {
    /// Every place the value appears, as (file, path), in file order.
    pub places: Vec<(String, SecretPath)>,
}

/// Whether a value is one of the team's own secrets: its key is written in capitals,
/// like `DATABASE_PASSWORD` or `API_KEY_2`. Kubernetes settings (`clusterIssuer`,
/// `resources.requests.cpu`) are camelCase and do not count.
pub fn is_secret_name(path: &SecretPath) -> bool {
    match path.segments().last() {
        Some(Segment::Key(name)) => {
            name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        }
        _ => false,
    }
}

/// Finds secrets (see [`is_secret_name`]) whose value appears in at least two
/// different files of the same folder.
/// A folder holds the environments of one thing (preview, production), so a value
/// shared there is a secret used twice. Across folders it is not compared.
///
/// Numbers, booleans and short texts are ignored: `enabled: true` in every
/// environment is configuration, not a reused secret.
pub fn shared_values(files: &[(String, BTreeMap<SecretPath, Scalar>)]) -> Vec<SharedValue> {
    let mut by_value: BTreeMap<(&str, &str), Vec<(String, SecretPath)>> = BTreeMap::new();
    for (file, leaves) in files {
        let folder = file.rsplit_once('/').map(|(folder, _)| folder).unwrap_or("");
        for (path, scalar) in leaves {
            if !is_secret_name(path) {
                continue;
            }
            let Scalar::Text(text) = scalar else { continue };
            if text.trim().chars().count() < MIN_SECRET_LENGTH {
                continue;
            }
            by_value.entry((folder, text.as_str())).or_default().push((file.clone(), path.clone()));
        }
    }
    let mut shared: Vec<SharedValue> = by_value
        .into_values()
        .filter(|places| places.iter().any(|(file, _)| file != &places[0].0))
        .map(|places| SharedValue { places })
        .collect();
    // Stable and value-independent: by where the value first appears.
    shared.sort_by(|a, b| a.places[0].cmp(&b.places[0]));
    shared
}

/// What one file holds for one variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// The variable is not in this file.
    Missing,
    /// It is there, and its value appears nowhere else in the folder.
    Unique,
    /// Its value is shared: every cell with the same number holds the same value,
    /// in this row or in another one.
    Same(usize),
}

/// One secret variable across the files of a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareRow {
    pub path: SecretPath,
    /// One cell per file, in the order of [`Comparison::files`].
    pub cells: Vec<Cell>,
}

/// The secrets of one folder side by side: variables down, files across.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub files: Vec<String>,
    pub rows: Vec<CompareRow>,
}

/// Lays the secrets (see [`is_secret_name`]) of the given files side by side.
/// Meant for the files of one folder. Values are compared, never returned.
pub fn compare(files: &[(String, BTreeMap<SecretPath, Scalar>)]) -> Comparison {
    let mut paths: Vec<&SecretPath> = files.iter().flat_map(|(_, leaves)| leaves.keys()).filter(|p| is_secret_name(p)).collect();
    paths.sort();
    paths.dedup();

    let comparable = |scalar: &Scalar| match scalar {
        Scalar::Text(text) if text.trim().chars().count() >= MIN_SECRET_LENGTH => Some(text.clone()),
        _ => None,
    };
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for path in &paths {
        for (_, leaves) in files {
            if let Some(value) = leaves.get(path).and_then(comparable) {
                *seen.entry(value).or_default() += 1;
            }
        }
    }
    // Numbers in the order the values first show up, so the picture is stable.
    let mut numbers: BTreeMap<String, usize> = BTreeMap::new();
    let rows = paths
        .into_iter()
        .map(|path| CompareRow {
            path: path.clone(),
            cells: files
                .iter()
                .map(|(_, leaves)| match leaves.get(path) {
                    None => Cell::Missing,
                    Some(scalar) => match comparable(scalar) {
                        Some(value) if seen[&value] > 1 => {
                            let next = numbers.len();
                            Cell::Same(*numbers.entry(value).or_insert(next))
                        }
                        _ => Cell::Unique,
                    },
                })
                .collect(),
        })
        .collect();
    Comparison { files: files.iter().map(|(name, _)| name.clone()).collect(), rows }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, values: &[(&str, Scalar)]) -> (String, BTreeMap<SecretPath, Scalar>) {
        let leaves = values.iter().map(|(key, value)| (SecretPath::default().key(key), value.clone())).collect();
        (name.to_owned(), leaves)
    }

    fn text(value: &str) -> Scalar {
        Scalar::Text(value.to_owned())
    }

    #[test]
    fn finds_the_same_password_in_two_environments() {
        let files = vec![
            file("env/preview.yaml", &[("DB_PASSWORD", text("s3cr3t-password")), ("OTHER", text("only-in-preview"))]),
            file("env/production.yaml", &[("DATABASE_PASS", text("s3cr3t-password"))]),
        ];
        let shared = shared_values(&files);
        assert_eq!(shared.len(), 1);
        assert_eq!(
            shared[0].places,
            vec![
                ("env/preview.yaml".to_owned(), SecretPath::default().key("DB_PASSWORD")),
                ("env/production.yaml".to_owned(), SecretPath::default().key("DATABASE_PASS")),
            ]
        );
    }

    #[test]
    fn only_capitalised_keys_are_secrets() {
        let files = vec![
            file("env/preview.yaml", &[("clusterIssuer", text("letsencrypt-prod")), ("API_TOKEN", text("tok-1234567890"))]),
            file("env/production.yaml", &[("clusterIssuer", text("letsencrypt-prod")), ("API_TOKEN", text("tok-1234567890"))]),
        ];
        let shared = shared_values(&files);
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].places[0].1, SecretPath::default().key("API_TOKEN"));
        assert!(is_secret_name(&SecretPath::default().key("app").key("DB_PASSWORD_2")));
        assert!(!is_secret_name(&SecretPath::default().key("dbPassword")));
        assert!(!is_secret_name(&SecretPath::default().key("_X")));
    }

    #[test]
    fn lays_secrets_side_by_side() {
        let files = vec![
            file("preview.yaml", &[("DB_PASSWORD", text("same-everywhere")), ("API_KEY", text("preview-only-key")), ("logLevel", text("debug-verbose"))]),
            file("production.yaml", &[("DB_PASSWORD", text("same-everywhere")), ("DB_PASS", text("same-everywhere"))]),
        ];
        let c = compare(&files);
        let row = |name: &str| c.rows.iter().find(|r| r.path == SecretPath::default().key(name)).unwrap().cells.clone();
        assert_eq!(c.rows.len(), 3, "camelCase keys are not secrets");
        assert_eq!(row("DB_PASSWORD"), vec![Cell::Same(0), Cell::Same(0)]);
        assert_eq!(row("DB_PASS"), vec![Cell::Missing, Cell::Same(0)]);
        assert_eq!(row("API_KEY"), vec![Cell::Unique, Cell::Missing]);
    }

    #[test]
    fn compares_only_within_one_folder() {
        let files = vec![
            file("env/acme/preview.yaml", &[("DB", text("s3cr3t-password"))]),
            file("env/globex/preview.yaml", &[("DB", text("s3cr3t-password"))]),
        ];
        assert!(shared_values(&files).is_empty());
    }

    #[test]
    fn ignores_settings_and_repeats_within_one_file() {
        let files = vec![
            file("a.yaml", &[("enabled", Scalar::Bool(true)), ("size", text("1Gi")), ("X", text("same-in-one-file")), ("Y", text("same-in-one-file"))]),
            file("b.yaml", &[("enabled", Scalar::Bool(true)), ("size", text("1Gi")), ("port", Scalar::Number("54321234".into()))]),
        ];
        assert!(shared_values(&files).is_empty());
    }
}
