use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateSource {
    Official,
    Github,
}

impl UpdateSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Official => "YtbDownGUI 官方源",
            Self::Github => "GitHub",
        }
    }
}
