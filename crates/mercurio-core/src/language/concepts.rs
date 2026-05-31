use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceLanguage {
    Kerml,
    Sysml,
}

impl SourceLanguage {
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some(extension) if extension.eq_ignore_ascii_case("sysml") => Some(Self::Sysml),
            Some(extension) if extension.eq_ignore_ascii_case("kerml") => Some(Self::Kerml),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sysml => "sysml",
            Self::Kerml => "kerml",
        }
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Sysml => &["sysml"],
            Self::Kerml => &["kerml"],
        }
    }
}

impl fmt::Display for SourceLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticConcept {
    AttributeUsage,
    ConstraintUsage,
    Feature,
    ItemDefinition,
    ItemUsage,
    Package,
    PartDefinition,
    PartUsage,
    RequirementUsage,
    Type,
    VerificationCaseUsage,
    View,
    Viewpoint,
}
