use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetCategory {
    Character,
    Clothing,
    Hair,
    Preset,
    Accessory,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub category: AssetCategory,
    #[serde(default)]
    pub mesh_files: Vec<String>,
    #[serde(default)]
    pub morph_targets: Vec<String>,
    pub skeleton_type: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
}

impl AssetManifest {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        category: AssetCategory,
        skeleton_type: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            author: None,
            category,
            mesh_files: Vec::new(),
            morph_targets: Vec::new(),
            skeleton_type: skeleton_type.into(),
            tags: Vec::new(),
            properties: HashMap::new(),
        }
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_manifest_json_roundtrip() {
        let mut manifest = AssetManifest::new(
            "hero_v1",
            "Hero Character",
            "1.0.0",
            AssetCategory::Character,
            "biped",
        );
        manifest.author = Some("Kaleaon Studio".to_string());
        manifest.mesh_files.push("body.obj".to_string());
        manifest.morph_targets.push("smile".to_string());
        manifest.tags.push("hero".to_string());

        let json = manifest.to_json().expect("Failed to serialize manifest");
        let parsed = AssetManifest::from_json(&json).expect("Failed to deserialize manifest");

        assert_eq!(manifest, parsed);
        assert_eq!(parsed.category, AssetCategory::Character);
        assert_eq!(parsed.skeleton_type, "biped");
    }
}
