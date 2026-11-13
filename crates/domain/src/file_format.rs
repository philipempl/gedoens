/// Die Formate, die SOPS als Baum aus Namen und Werten behandelt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Yaml,
    Json,
    Dotenv,
    Ini,
}

impl FileFormat {
    /// SOPS erkennt das Format nur an der Endung und hält `.env.production`
    /// deshalb für eine Binärdatei. Wir erkennen es am ganzen Namen und geben
    /// den Typ bei jedem Aufruf ausdrücklich mit.
    pub fn from_file_name(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();
        if lower.ends_with(".yaml") || lower.ends_with(".yml") {
            Some(Self::Yaml)
        } else if lower.ends_with(".json") {
            Some(Self::Json)
        } else if lower.ends_with(".ini") {
            Some(Self::Ini)
        } else if lower.ends_with(".env") || lower.starts_with(".env.") || lower.contains(".env.") {
            Some(Self::Dotenv)
        } else {
            None
        }
    }

    pub fn sops_type(self) -> &'static str {
        match self {
            Self::Yaml => "yaml",
            Self::Json => "json",
            Self::Dotenv => "dotenv",
            Self::Ini => "ini",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FileFormat::{self, *};

    #[test]
    fn recognises_env_files_sops_would_miss() {
        assert_eq!(FileFormat::from_file_name(".env.production"), Some(Dotenv));
        assert_eq!(FileFormat::from_file_name("app.env"), Some(Dotenv));
        assert_eq!(FileFormat::from_file_name("api.YAML"), Some(Yaml));
        assert_eq!(FileFormat::from_file_name("notes.txt"), None);
    }
}
