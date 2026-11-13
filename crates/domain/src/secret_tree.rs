use std::collections::BTreeMap;

use crate::SecretPath;

/// Ein einzelner Wert. Der Typ bleibt erhalten, damit aus `5432` kein Text wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scalar {
    Text(String),
    Number(String),
    Bool(bool),
    Null,
}

impl Scalar {
    pub fn display(&self) -> String {
        match self {
            Scalar::Text(s) | Scalar::Number(s) => s.clone(),
            Scalar::Bool(b) => b.to_string(),
            Scalar::Null => String::new(),
        }
    }

    /// Deutet eine Eingabe so, wie der bisherige Wert getippt war.
    /// War es eine Zahl und ist die Eingabe wieder eine, bleibt es eine Zahl.
    pub fn retyped(previous: Option<&Scalar>, input: &str) -> Scalar {
        match previous {
            Some(Scalar::Number(_)) if input.parse::<f64>().is_ok() => Scalar::Number(input.to_owned()),
            Some(Scalar::Bool(_)) if input == "true" || input == "false" => Scalar::Bool(input == "true"),
            _ => Scalar::Text(input.to_owned()),
        }
    }
}

/// Der entschlüsselte Inhalt einer Datei als Baum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Leaf(Scalar),
    Map(Vec<(String, Node)>),
    List(Vec<Node>),
}

/// Eine Zeile der Ansicht: entweder eine Gruppe oder ein Wert, nie der Wert selbst.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineItem {
    pub path: SecretPath,
    pub label: String,
    pub depth: usize,
    pub is_group: bool,
}

impl Node {
    pub fn get(&self, path: &SecretPath) -> Option<&Node> {
        let mut current = self;
        for segment in path.segments() {
            current = match (current, segment) {
                (Node::Map(entries), crate::Segment::Key(k)) => {
                    entries.iter().find(|(name, _)| name == k).map(|(_, node)| node)?
                }
                (Node::List(items), crate::Segment::Index(i)) => items.get(*i)?,
                _ => return None,
            };
        }
        Some(current)
    }

    pub fn scalar(&self, path: &SecretPath) -> Option<&Scalar> {
        match self.get(path)? {
            Node::Leaf(scalar) => Some(scalar),
            _ => None,
        }
    }

    /// Alle Werte mit ihrem Weg. Grundlage für Vergleich und Zusammenführen.
    pub fn leaves(&self) -> BTreeMap<SecretPath, Scalar> {
        let mut out = BTreeMap::new();
        self.walk(&SecretPath::default(), 0, &mut |path, _, node| {
            if let Node::Leaf(scalar) = node {
                out.insert(path.clone(), scalar.clone());
            }
        });
        out
    }

    /// Die Gliederung ohne Werte. Nur sie verlässt das Backend beim Öffnen.
    pub fn outline(&self) -> Vec<OutlineItem> {
        let mut out = Vec::new();
        self.walk(&SecretPath::default(), 0, &mut |path, depth, node| {
            if !path.segments().is_empty() {
                out.push(OutlineItem {
                    path: path.clone(),
                    label: path.label(),
                    depth: depth - 1,
                    is_group: !matches!(node, Node::Leaf(_)),
                });
            }
        });
        out
    }

    fn walk(&self, path: &SecretPath, depth: usize, visit: &mut dyn FnMut(&SecretPath, usize, &Node)) {
        visit(path, depth, self);
        match self {
            Node::Leaf(_) => {}
            Node::Map(entries) => {
                for (name, child) in entries {
                    child.walk(&path.key(name), depth + 1, visit);
                }
            }
            Node::List(items) => {
                for (i, child) in items.iter().enumerate() {
                    child.walk(&path.index(i), depth + 1, visit);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Node {
        Node::Map(vec![
            (
                "db".into(),
                Node::Map(vec![
                    ("user".into(), Node::Leaf(Scalar::Text("api".into()))),
                    ("port".into(), Node::Leaf(Scalar::Number("5432".into()))),
                ]),
            ),
            ("hosts".into(), Node::List(vec![Node::Leaf(Scalar::Text("a".into()))])),
        ])
    }

    #[test]
    fn outline_lists_groups_and_values_without_content() {
        let labels: Vec<(String, usize, bool)> =
            sample().outline().into_iter().map(|i| (i.label, i.depth, i.is_group)).collect();
        assert_eq!(
            labels,
            vec![
                ("db".into(), 0, true),
                ("user".into(), 1, false),
                ("port".into(), 1, false),
                ("hosts".into(), 0, true),
                ("1".into(), 1, false),
            ]
        );
    }

    #[test]
    fn finds_values_by_path() {
        let tree = sample();
        let path = SecretPath::default().key("db").key("port");
        assert_eq!(tree.scalar(&path), Some(&Scalar::Number("5432".into())));
        assert_eq!(tree.leaves().len(), 3);
    }

    #[test]
    fn keeps_the_type_of_a_value_when_it_still_fits() {
        let number = Scalar::Number("5432".into());
        assert_eq!(Scalar::retyped(Some(&number), "6543"), Scalar::Number("6543".into()));
        assert_eq!(Scalar::retyped(Some(&number), "kein"), Scalar::Text("kein".into()));
        assert_eq!(Scalar::retyped(None, "42"), Scalar::Text("42".into()));
    }
}
