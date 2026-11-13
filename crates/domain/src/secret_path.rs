use crate::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Segment {
    Key(String),
    Index(usize),
}

/// Der Weg zu einem Wert in einer Datei, zum Beispiel `database → password`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SecretPath(Vec<Segment>);

impl SecretPath {
    pub fn new(segments: Vec<Segment>) -> Self {
        Self(segments)
    }

    pub fn segments(&self) -> &[Segment] {
        &self.0
    }

    pub fn key(&self, name: &str) -> Self {
        let mut next = self.0.clone();
        next.push(Segment::Key(name.to_owned()));
        Self(next)
    }

    pub fn index(&self, position: usize) -> Self {
        let mut next = self.0.clone();
        next.push(Segment::Index(position));
        Self(next)
    }

    /// Der letzte Abschnitt, so wie ein Mensch ihn liest.
    pub fn label(&self) -> String {
        match self.0.last() {
            Some(Segment::Key(k)) => k.clone(),
            Some(Segment::Index(i)) => format!("{}", i + 1),
            None => String::new(),
        }
    }

    /// Schreibweise, die `sops set` und `sops unset` erwarten: `["a"]["b"][0]`.
    ///
    /// SOPS kennt in dieser Schreibweise kein Escaping. Namen mit Anführungszeichen
    /// oder eckigen Klammern lehnen wir deshalb ab, statt etwas Falsches zu schreiben.
    pub fn to_sops_index(&self) -> Result<String, DomainError> {
        let mut out = String::new();
        for segment in &self.0 {
            match segment {
                Segment::Key(k) => {
                    if k.is_empty() || k.contains(['"', '\'', '[', ']']) {
                        return Err(DomainError::UnsupportedKey(k.clone()));
                    }
                    out.push_str(&format!("[\"{k}\"]"));
                }
                Segment::Index(i) => out.push_str(&format!("[{i}]")),
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_sops_index_syntax() {
        let path = SecretPath::default().key("an_array").index(1).key("uid");
        assert_eq!(path.to_sops_index().unwrap(), r#"["an_array"][1]["uid"]"#);
    }

    #[test]
    fn refuses_names_sops_cannot_address() {
        let path = SecretPath::default().key(r#"we"ird"#);
        assert!(matches!(path.to_sops_index(), Err(DomainError::UnsupportedKey(_))));
    }
}
