//! Was die Oberfläche zu sehen bekommt. Werte kommen hier nie vor,
//! außer dort, wo ein Mensch zwischen zweien wählen muss.

use gedoens_domain::{Recipient, SecretPath, Segment};
use serde::{Deserialize, Serialize};

/// Ein Abschnitt eines Wegs, so wie die Oberfläche ihn schickt und bekommt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Step {
    Key(String),
    Index(usize),
}

pub fn to_path(steps: &[Step]) -> SecretPath {
    SecretPath::new(
        steps
            .iter()
            .map(|step| match step {
                Step::Key(k) => Segment::Key(k.clone()),
                Step::Index(i) => Segment::Index(*i),
            })
            .collect(),
    )
}

pub fn to_steps(path: &SecretPath) -> Vec<Step> {
    path.segments()
        .iter()
        .map(|segment| match segment {
            Segment::Key(k) => Step::Key(k.clone()),
            Segment::Index(i) => Step::Index(*i),
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub recipient: String,
    pub short: String,
    pub name: Option<String>,
    pub me: bool,
}

impl Person {
    pub fn new(recipient: &Recipient, name: Option<&str>, me: bool) -> Self {
        Self { recipient: recipient.as_str().to_owned(), short: recipient.short(), name: name.map(str::to_owned), me }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyStatus {
    pub public_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftView {
    pub missing: Vec<Person>,
    pub extra: Vec<Person>,
}

/// Eine offene Sache auf der Startseite.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Issue {
    Conflict { file: String },
    #[serde(rename_all = "camelCase")]
    Drift { file: String, drift: DriftView },
    Unnamed { person: Person },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSummary {
    pub path: String,
    pub name: String,
    pub readable: bool,
    pub drifted: bool,
    pub conflicted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileGroup {
    pub folder: String,
    pub files: Vec<FileSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub issues: Vec<Issue>,
    pub groups: Vec<FileGroup>,
    pub checks_total: usize,
    pub checks_passed: usize,
    pub unnamed: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub path: Vec<Step>,
    pub label: String,
    pub depth: usize,
    pub group: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileView {
    pub path: String,
    pub name: String,
    pub readable: bool,
    pub conflicted: bool,
    pub area: Option<String>,
    pub readers: Vec<Person>,
    pub drift: Option<DriftView>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaView {
    pub index: usize,
    pub name: String,
    pub pattern: Option<String>,
    pub files: usize,
    /// Falsch, wenn uns der Zugriff fehlt oder die Regel zu besonders ist.
    pub editable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonAccess {
    #[serde(flatten)]
    pub person: Person,
    /// Ein Eintrag pro Bereich, in derselben Reihenfolge wie `AccessView::areas`.
    pub areas: Vec<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessView {
    pub has_policy: bool,
    pub areas: Vec<AreaView>,
    pub people: Vec<PersonAccess>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClashView {
    pub path: Vec<Step>,
    pub label: String,
    pub ours: Option<String>,
    pub theirs: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePreview {
    /// So viele Änderungen der anderen Seite lassen sich ohne Rückfrage übernehmen.
    pub automatic: usize,
    pub clashes: Vec<ClashView>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub path: Vec<Step>,
    /// Wahr: der fremde Wert gilt. Falsch: unserer.
    pub take_theirs: bool,
}
