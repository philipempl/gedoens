use std::collections::{BTreeMap, BTreeSet};

use crate::{DomainError, Scalar, SecretPath};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Ours,
    Theirs,
}

/// Eine Änderung an einem Wert. `None` heißt: der Eintrag fällt weg.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub path: SecretPath,
    pub value: Option<Scalar>,
}

/// Beide Seiten haben denselben Wert verschieden geändert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    pub path: SecretPath,
    pub ours: Option<Scalar>,
    pub theirs: Option<Scalar>,
}

/// Das Ergebnis eines Vergleichs der drei Stände im Klartext.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergePlan {
    /// Nur die andere Seite hat hier etwas geändert.
    pub theirs: Vec<Change>,
    /// Nur wir haben hier etwas geändert.
    pub ours: Vec<Change>,
    pub clashes: Vec<Clash>,
}

type Leaves = BTreeMap<SecretPath, Scalar>;

/// Vergleicht Wert für Wert. Der `sops`-Block mit Prüfsumme und Zeitstempel
/// spielt hier keine Rolle mehr, deshalb kollidiert er auch nicht.
pub fn merge(base: &Leaves, ours: &Leaves, theirs: &Leaves) -> MergePlan {
    let mut plan = MergePlan::default();
    let paths: BTreeSet<&SecretPath> = base.keys().chain(ours.keys()).chain(theirs.keys()).collect();
    for path in paths {
        let (b, o, t) = (base.get(path), ours.get(path), theirs.get(path));
        if o == t {
            continue;
        }
        if o == b {
            plan.theirs.push(Change { path: path.clone(), value: t.cloned() });
        } else if t == b {
            plan.ours.push(Change { path: path.clone(), value: o.cloned() });
        } else {
            plan.clashes.push(Clash { path: path.clone(), ours: o.cloned(), theirs: t.cloned() });
        }
    }
    plan
}

impl MergePlan {
    pub fn is_automatic(&self) -> bool {
        self.clashes.is_empty()
    }

    /// Die Änderungen, die auf den Stand von `base_side` anzuwenden sind,
    /// damit am Ende beide Seiten enthalten sind.
    pub fn resolve(&self, base_side: Side, choices: &[(SecretPath, Side)]) -> Result<Vec<Change>, DomainError> {
        let mut changes = match base_side {
            Side::Ours => self.theirs.clone(),
            Side::Theirs => self.ours.clone(),
        };
        for clash in &self.clashes {
            let chosen = choices
                .iter()
                .find(|(path, _)| path == &clash.path)
                .map(|(_, side)| *side)
                .ok_or_else(|| DomainError::MissingChoice(clash.path.label()))?;
            if chosen != base_side {
                let value = match chosen {
                    Side::Ours => clash.ours.clone(),
                    Side::Theirs => clash.theirs.clone(),
                };
                changes.push(Change { path: clash.path.clone(), value });
            }
        }
        Ok(changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaves(pairs: &[(&str, &str)]) -> Leaves {
        pairs
            .iter()
            .map(|(k, v)| (SecretPath::default().key(k), Scalar::Text((*v).to_owned())))
            .collect()
    }

    fn text(v: &str) -> Option<Scalar> {
        Some(Scalar::Text(v.to_owned()))
    }

    #[test]
    fn different_values_merge_without_asking() {
        let base = leaves(&[("a", "1"), ("b", "1")]);
        let ours = leaves(&[("a", "2"), ("b", "1")]);
        let theirs = leaves(&[("a", "1"), ("b", "3")]);
        let plan = merge(&base, &ours, &theirs);

        assert!(plan.is_automatic());
        let onto_ours = plan.resolve(Side::Ours, &[]).unwrap();
        assert_eq!(onto_ours, vec![Change { path: SecretPath::default().key("b"), value: text("3") }]);
        let onto_theirs = plan.resolve(Side::Theirs, &[]).unwrap();
        assert_eq!(onto_theirs, vec![Change { path: SecretPath::default().key("a"), value: text("2") }]);
    }

    #[test]
    fn the_same_value_changed_twice_needs_a_choice() {
        let base = leaves(&[("a", "1")]);
        let plan = merge(&base, &leaves(&[("a", "2")]), &leaves(&[("a", "3")]));
        let path = SecretPath::default().key("a");

        assert_eq!(plan.clashes, vec![Clash { path: path.clone(), ours: text("2"), theirs: text("3") }]);
        assert!(matches!(plan.resolve(Side::Ours, &[]), Err(DomainError::MissingChoice(_))));
        assert!(plan.resolve(Side::Ours, &[(path.clone(), Side::Ours)]).unwrap().is_empty());
        assert_eq!(
            plan.resolve(Side::Ours, &[(path.clone(), Side::Theirs)]).unwrap(),
            vec![Change { path, value: text("3") }]
        );
    }

    #[test]
    fn additions_and_removals_are_changes_too() {
        let base = leaves(&[("old", "1")]);
        let ours = leaves(&[("old", "1"), ("mine", "x")]);
        let theirs = leaves(&[]);
        let plan = merge(&base, &ours, &theirs);

        assert_eq!(plan.theirs, vec![Change { path: SecretPath::default().key("old"), value: None }]);
        assert_eq!(plan.ours, vec![Change { path: SecretPath::default().key("mine"), value: text("x") }]);
    }

    #[test]
    fn identical_edits_are_no_conflict() {
        let base = leaves(&[("a", "1")]);
        let both = leaves(&[("a", "2")]);
        assert_eq!(merge(&base, &both, &both), MergePlan::default());
    }
}
