use crate::Recipient;

/// Die Abweichung zwischen den Empfängern einer Datei und denen ihres Bereichs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Drift {
    /// Stehen im Bereich, fehlen aber in der Datei.
    pub missing: Vec<Recipient>,
    /// Stehen in der Datei, obwohl der Bereich sie nicht vorsieht.
    pub extra: Vec<Recipient>,
}

impl Drift {
    pub fn between(file: &[Recipient], area: &[Recipient]) -> Self {
        Self {
            missing: area.iter().filter(|r| !file.contains(r)).cloned().collect(),
            extra: file.iter().filter(|r| !area.contains(r)).cloned().collect(),
        }
    }

    pub fn is_none(&self) -> bool {
        self.missing.is_empty() && self.extra.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> Recipient {
        Recipient::parse(&format!("age1{}", n.to_string().repeat(30))).unwrap()
    }

    #[test]
    fn names_who_is_missing_and_who_is_too_much() {
        let drift = Drift::between(&[key(1), key(3)], &[key(1), key(2)]);
        assert_eq!(drift.missing, vec![key(2)]);
        assert_eq!(drift.extra, vec![key(3)]);
        assert!(Drift::between(&[key(1)], &[key(1)]).is_none());
    }
}
