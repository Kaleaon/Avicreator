use crate::mesh::Mesh;
use crate::vector_math::Vec3;

#[derive(Debug, Clone, PartialEq)]
pub struct MorphDelta {
    pub position_deltas: Vec<Vec3>,
    pub normal_deltas: Option<Vec<Vec3>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MorphTarget {
    pub name: String,
    pub deltas: MorphDelta,
    pub default_weight: f32,
}

impl MorphTarget {
    pub fn new(name: impl Into<String>, position_deltas: Vec<Vec3>) -> Self {
        Self {
            name: name.into(),
            deltas: MorphDelta {
                position_deltas,
                normal_deltas: None,
            },
            default_weight: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MorphEvaluator;

impl MorphEvaluator {
    pub fn new() -> Self {
        Self
    }

    pub fn evaluate(&self, base_mesh: &Mesh, active_targets: &[(&MorphTarget, f32)]) -> Mesh {
        let mut deformed = base_mesh.clone();

        if active_targets.is_empty() || base_mesh.positions.is_empty() {
            return deformed;
        }

        // Apply position deltas
        for (target, weight) in active_targets {
            let w = *weight;
            if w.abs() < 1e-6 {
                continue;
            }

            for (i, delta) in target.deltas.position_deltas.iter().enumerate() {
                if i < deformed.positions.len() {
                    deformed.positions[i] = deformed.positions[i].add(delta.scale(w));
                }
            }
        }

        // Recalculate normals after deformation
        deformed.recalculate_normals();
        deformed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_morph_target_deformation() {
        let base_positions = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let indices = vec![0, 1, 2];
        let base_mesh = Mesh::new(base_positions, indices);

        let smile_deltas = vec![
            Vec3::new(0.0, 0.2, 0.0),
            Vec3::new(0.1, 0.2, 0.0),
            Vec3::new(0.0, 0.2, 0.0),
        ];
        let smile_target = MorphTarget::new("smile", smile_deltas);

        let evaluator = MorphEvaluator::new();

        // 50% morph blending
        let active = vec![(&smile_target, 0.5)];
        let deformed = evaluator.evaluate(&base_mesh, &active);

        // V0 y = 0.0 + 0.2 * 0.5 = 0.1
        assert!((deformed.positions[0].y - 0.1).abs() < 1e-5);
        // V1 x = 1.0 + 0.1 * 0.5 = 1.05
        assert!((deformed.positions[1].x - 1.05).abs() < 1e-5);
    }
}
