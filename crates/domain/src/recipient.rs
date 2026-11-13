use crate::DomainError;

/// Ein Public Key, für den SOPS verschlüsselt: ein age-Key oder ein SSH-Key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Recipient(String);

impl Recipient {
    /// Nimmt Eingaben großzügig an (Leerzeichen, Zeilenumbrüche) und prüft streng.
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let trimmed = raw.trim();
        if trimmed.starts_with("ssh-ed25519 ") || trimmed.starts_with("ssh-rsa ") {
            let mut parts = trimmed.split_whitespace();
            let (kind, body) = (parts.next(), parts.next());
            return match (kind, body) {
                (Some(kind), Some(body)) if !body.is_empty() => Ok(Self(format!("{kind} {body}"))),
                _ => Err(DomainError::InvalidRecipient),
            };
        }
        let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
        let valid = compact.starts_with("age1")
            && compact.len() >= 20
            && compact.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        if valid {
            Ok(Self(compact))
        } else {
            Err(DomainError::InvalidRecipient)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Kurzform für die Anzeige, zum Beispiel `age1qz8f3mh2…7k4du`.
    pub fn short(&self) -> String {
        let chars: Vec<char> = self.0.chars().collect();
        if chars.len() <= 20 {
            return self.0.clone();
        }
        let head: String = chars[..12].iter().collect();
        let tail: String = chars[chars.len() - 5..].iter().collect();
        format!("{head}…{tail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "age1xktqs7cxsgq0epqkcjq0rvj3cmrwakgmycmsm05q3nwmtuqaq3asfsch7t";

    #[test]
    fn accepts_keys_with_stray_whitespace() {
        let messy = format!("  {}\n {} ", &KEY[..30], &KEY[30..]);
        assert_eq!(Recipient::parse(&messy).unwrap().as_str(), KEY);
    }

    #[test]
    fn rejects_everything_else() {
        assert!(Recipient::parse("AGE-SECRET-KEY-1ABC").is_err());
        assert!(Recipient::parse("age1").is_err());
        assert!(Recipient::parse("").is_err());
    }

    #[test]
    fn keeps_ssh_keys_without_comment() {
        let r = Recipient::parse("ssh-ed25519 AAAAC3Nza me@laptop").unwrap();
        assert_eq!(r.as_str(), "ssh-ed25519 AAAAC3Nza");
    }

    #[test]
    fn shortens_for_display() {
        assert_eq!(Recipient::parse(KEY).unwrap().short(), "age1xktqs7cx…sch7t");
    }
}
