use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum AlphaMode {
    #[default]
    Opaque,
    Blend,
    Mask,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaterialSpec {
    pub name: String,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive_factor: [f32; 3],
    #[serde(default)]
    pub alpha_mode: AlphaMode,
    #[serde(default)]
    pub textures: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ktheme_properties: Option<HashMap<String, serde_json::Value>>,
}

impl MaterialSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive_factor: [0.0, 0.0, 0.0],
            alpha_mode: AlphaMode::Opaque,
            textures: HashMap::new(),
            ktheme_properties: None,
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
    fn test_material_json_roundtrip() {
        let mut mat = MaterialSpec::new("SkinMaterial");
        mat.base_color = [0.8, 0.6, 0.5, 1.0];
        mat.roughness = 0.4;
        mat.textures.insert("diffuse".to_string(), "skin_base.png".to_string());

        let json = mat.to_json().expect("Serialization failed");
        let parsed = MaterialSpec::from_json(&json).expect("Deserialization failed");

        assert_eq!(mat, parsed);
        assert_eq!(parsed.alpha_mode, AlphaMode::Opaque);
        assert_eq!(parsed.textures.get("diffuse").unwrap(), "skin_base.png");
    }
}
