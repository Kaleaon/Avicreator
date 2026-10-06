/// Normalizes vertex group bone weights to sum to 1.0 per vertex.
/// Small weights below epsilon are zeroed out before normalization.
pub fn normalize_vertex_weights(weights: &mut [[f32; 4]], epsilon: f32) {
    for w in weights.iter_mut() {
        // Zero out small weights
        for val in w.iter_mut() {
            if *val < epsilon {
                *val = 0.0;
            }
        }

        let sum: f32 = w.iter().sum();
        if sum > 1e-6 {
            let inv_sum = 1.0 / sum;
            for val in w.iter_mut() {
                *val *= inv_sum;
            }
        } else {
            // Default to 1.0 on primary bone if all weights were tiny
            w[0] = 1.0;
            w[1] = 0.0;
            w[2] = 0.0;
            w[3] = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weight_normalization() {
        let mut weights = vec![
            [0.5, 0.5, 0.0, 0.0],
            [2.0, 2.0, 0.0, 0.0],
            [0.0001, 0.8, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
        ];

        normalize_vertex_weights(&mut weights, 0.001);

        // First vertex sum = 1.0
        assert!((weights[0][0] - 0.5).abs() < 1e-5);
        assert!((weights[0][1] - 0.5).abs() < 1e-5);

        // Second vertex unnormalized sum=4.0 -> normalized to [0.5, 0.5, 0, 0]
        assert!((weights[1][0] - 0.5).abs() < 1e-5);
        assert!((weights[1][1] - 0.5).abs() < 1e-5);

        // Third vertex small weight 0.0001 < 0.001 zeroed out -> [0, 1.0, 0, 0]
        assert_eq!(weights[2][0], 0.0);
        assert!((weights[2][1] - 1.0).abs() < 1e-5);

        // Zero sum defaults to primary bone weight 1.0
        assert_eq!(weights[3], [1.0, 0.0, 0.0, 0.0]);
    }
}
