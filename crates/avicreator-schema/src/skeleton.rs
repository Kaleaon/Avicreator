use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkeletonNode {
    pub id: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<u32>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4], // Quaternion [x, y, z, w]
    pub scale: [f32; 3],
    #[serde(default)]
    pub children: Vec<u32>,
    #[serde(default)]
    pub custom_props: HashMap<String, f32>,
}

impl SkeletonNode {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            parent_id: None,
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            children: Vec::new(),
            custom_props: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Skeleton {
    pub name: String,
    pub nodes: Vec<SkeletonNode>,
    #[serde(default)]
    pub root_indices: Vec<u32>,
}

impl Skeleton {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: Vec::new(),
            root_indices: Vec::new(),
        }
    }

    pub fn add_node(&mut self, mut node: SkeletonNode, parent_id: Option<u32>) {
        node.parent_id = parent_id;
        let node_id = node.id;
        self.nodes.push(node);

        if let Some(pid) = parent_id {
            if let Some(parent) = self.nodes.iter_mut().find(|n| n.id == pid) {
                if !parent.children.contains(&node_id) {
                    parent.children.push(node_id);
                }
            }
        } else if !self.root_indices.contains(&node_id) {
            self.root_indices.push(node_id);
        }
    }

    pub fn find_node_by_name(&self, name: &str) -> Option<&SkeletonNode> {
        self.nodes.iter().find(|n| n.name == name)
    }

    pub fn find_node_by_id(&self, id: u32) -> Option<&SkeletonNode> {
        self.nodes.iter().find(|n| n.id == id)
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
    fn test_skeleton_building_and_json_roundtrip() {
        let mut skel = Skeleton::new("StandardBiped");
        let root = SkeletonNode::new(0, "Hips");
        let chest = SkeletonNode::new(1, "Chest");
        let head = SkeletonNode::new(2, "Head");

        skel.add_node(root, None);
        skel.add_node(chest, Some(0));
        skel.add_node(head, Some(1));

        assert_eq!(skel.root_indices, vec![0]);
        assert_eq!(skel.find_node_by_name("Chest").unwrap().parent_id, Some(0));
        assert_eq!(skel.find_node_by_name("Hips").unwrap().children, vec![1]);

        let json = skel.to_json().expect("Serialization failed");
        let parsed = Skeleton::from_json(&json).expect("Deserialization failed");

        assert_eq!(skel, parsed);
    }
}
