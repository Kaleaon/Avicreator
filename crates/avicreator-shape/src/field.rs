//! Evaluates a compiled shape: signed distance, gradient and influence weights.
//!
//! Influence is continuous over space, so meshes of any resolution can be
//! skinned by sampling it at their vertices:
//!
//! `w_bone(p) = sum over parts i of membership_i(p) * share_i,bone(p)`
//!
//! * `membership_i` is a soft-min over part distances, so where geometry
//!   blends between parts, their bones blend too.
//!   It is attenuated by *visibility*: a part only influences points whose
//!   straight path to its bones stays inside the body, so weights never jump
//!   across gaps (inner thighs, fingers, arms resting near the torso).
//! * `share_i,bone` splits a part's region among its own bones: by position
//!   along a chain (neighbours cross-fade around joints), by nearest bone for
//!   membranes, or rigidly for blobs and arrays.

use avicreator_core::Vec3;

use crate::compile::{CompiledShape, Influence};
use crate::program::Op;
use crate::sdf::{muscle_slope, smax, smin, Shape};

/// Distances beyond this are treated as "far outside".
pub const FAR: f32 = 1e10;

/// Evenly spaced probes along the path from a point to a part's bones,
/// at `s / (N + 1)` for `s = 1..=N` (mirrored in the GLSL `vis` functions).
pub const VISIBILITY_SAMPLES: usize = 9;

impl CompiledShape {
    /// Signed distance (negative inside).
    pub fn distance(&self, p: Vec3) -> f32 {
        self.blend_parts(|i| {
            let part = &self.parts[i];
            Some(self.part_distance_in(part.prims.clone(), part.smooth, p))
        })
    }

    /// Signed distance using only the listed primitives (sorted ascending).
    /// Exact wherever the omitted primitives are farther than the blend radii.
    pub fn distance_subset(&self, p: Vec3, active: &[u32]) -> f32 {
        let mut cursor = 0;
        self.blend_parts(|i| {
            let part = &self.parts[i];
            let mut dp = None;
            while cursor < active.len() && self.prims[active[cursor] as usize].part == i {
                let d = self.prims[active[cursor] as usize].shape.distance(p);
                dp = Some(dp.map_or(d, |acc| smin(acc, d, part.smooth)));
                cursor += 1;
            }
            dp
        })
    }

    /// Combines part distances (requested in part order; `None` = absent)
    /// through blend groups: smooth within a group, a smooth fillet between a
    /// group and each partner, hard union otherwise, unattached cuts last.
    /// Muscle relief (smooth maximum of the muscles' raises) lifts its
    /// group's surface before the group meets its partners.
    fn blend_parts(&self, mut part_distance: impl FnMut(usize) -> Option<f32>) -> f32 {
        const STACK: usize = 128;
        let n = self.groups.len();
        let mut stack = [FAR; STACK];
        let mut relief_stack = [-FAR; STACK];
        let mut heap = Vec::new();
        let mut relief_heap = Vec::new();
        let (g, relief): (&mut [f32], &mut [f32]) = if n <= STACK {
            (&mut stack[..n], &mut relief_stack[..n])
        } else {
            heap.resize(n, FAR);
            relief_heap.resize(n, -FAR);
            (&mut heap, &mut relief_heap)
        };
        let mut cuts: [(usize, f32); 8] = [(0, FAR); 8];
        let mut cut_count = 0;
        let mut late_cuts = Vec::new();
        for (i, part) in self.parts.iter().enumerate() {
            let Some(dp) = part_distance(i) else { continue };
            match part.group {
                Some(gi) if part.relief => relief[gi] = smax(relief[gi], -dp, part.blend),
                Some(gi) => g[gi] = combine(g[gi], dp, part.op, part.blend),
                None if cut_count < cuts.len() => {
                    cuts[cut_count] = (i, dp);
                    cut_count += 1;
                }
                None => late_cuts.push((i, dp)),
            }
        }
        let mut d = FAR;
        for (gi, group) in self.groups.iter().enumerate() {
            if let Some(k) = group.relief {
                g[gi] -= smax(relief[gi], 0.0, k);
            }
            let mut dg = g[gi];
            for &h in &group.partners {
                dg = dg.min(fillet(g[gi], g[h], group.blend));
            }
            d = d.min(dg);
        }
        for &(i, dp) in cuts[..cut_count].iter().chain(&late_cuts) {
            d = smax(d, -dp, self.parts[i].blend);
        }
        d
    }

    /// Bound on how fast the field changes, relative to a true distance:
    /// 1 for solids, more where muscle relief tilts the surface.
    pub fn lipschitz(&self) -> f32 {
        self.prims
            .iter()
            .filter_map(|p| match p.shape {
                Shape::Muscle { length, belly, radii, height, .. } => {
                    let k = self.parts[p.part].group.and_then(|g| self.groups[g].relief).unwrap_or(0.0);
                    Some(muscle_slope(length, belly, radii, height, k.max(self.parts[p.part].blend)))
                }
                _ => None,
            })
            .fold(1.0, |l, slope| l.max(1.0 + slope))
    }

    /// Distance to one part on its own, before blending with the rest
    /// (for a muscle: minus how far it raises the surface).
    pub fn part_distance(&self, part: usize, p: Vec3) -> f32 {
        let part = &self.parts[part];
        self.part_distance_in(part.prims.clone(), part.smooth, p)
    }

    fn part_distance_in(&self, prims: std::ops::Range<usize>, smooth: f32, p: Vec3) -> f32 {
        let mut dp = FAR;
        for prim in &self.prims[prims] {
            dp = smin(dp, prim.shape.distance(p), smooth);
        }
        dp
    }

    /// Unnormalized gradient of the full field.
    pub fn gradient(&self, p: Vec3, eps: f32) -> Vec3 {
        tetra_gradient(p, eps, |q| self.distance(q))
    }

    /// Unnormalized gradient by the tetrahedral central-difference stencil.
    pub fn gradient_subset(&self, p: Vec3, active: &[u32], eps: f32) -> Vec3 {
        tetra_gradient(p, eps, |q| self.distance_subset(q, active))
    }
}

fn tetra_gradient(p: Vec3, eps: f32, f: impl Fn(Vec3) -> f32) -> Vec3 {
    {
        let k = [
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(1.0, 1.0, 1.0),
        ];
        let mut g = Vec3::ZERO;
        for dir in k {
            g = g.add(dir.scale(f(p.add(dir.scale(eps)))));
        }
        g.scale(1.0 / (4.0 * eps))
    }
}

impl CompiledShape {

    /// Up to four `(bone, weight)` pairs, heaviest first, summing to one.
    /// Empty when the shape has no bones.
    pub fn weights(&self, p: Vec3) -> Vec<(usize, f32)> {
        let mut dists = Vec::with_capacity(self.parts.len());
        let mut dmin = FAR;
        for (i, part) in self.parts.iter().enumerate() {
            if part.op == Op::Subtract || matches!(part.influence, Influence::None) {
                continue;
            }
            let d = self.part_distance(i, p);
            dmin = dmin.min(d);
            dists.push((i, d));
        }

        let owner = dists.iter().min_by(|a, b| a.1.total_cmp(&b.1)).map(|e| e.0);
        let mut acc: Vec<(usize, f32)> = Vec::new();
        for (i, d) in dists {
            let part = &self.parts[i];
            let mut membership = (-(d - dmin) / part.membership_softness()).exp();
            if membership < 1e-4 {
                continue;
            }
            if owner.is_some_and(|o| o != i && !self.blend_connected(o, i)) {
                membership *= self.visibility(i, p);
                if membership < 1e-4 {
                    continue;
                }
            }
            for (bone, share) in self.part_shares(i, p) {
                let w = membership * share;
                match acc.iter_mut().find(|(b, _)| *b == bone) {
                    Some(entry) => entry.1 += w,
                    None => acc.push((bone, w)),
                }
            }
        }
        acc.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        acc.truncate(4);
        let sum: f32 = acc.iter().map(|(_, w)| w).sum();
        if sum > 0.0 {
            for entry in &mut acc {
                entry.1 /= sum;
            }
        }
        acc.retain(|(_, w)| *w > 1e-4);
        acc
    }

    /// Bones that carry part `i`'s influence, in a fixed order.
    pub fn influence_bones(&self, i: usize) -> Vec<usize> {
        let part = &self.parts[i];
        match &part.influence {
            Influence::None => Vec::new(),
            Influence::Rigid { bone } => vec![*bone],
            Influence::Chain { bones, .. } | Influence::Nearest { bones } => bones.clone(),
            Influence::PerPrimitive => {
                let mut bones: Vec<usize> = Vec::new();
                for b in self.prims[part.prims.clone()].iter().filter_map(|p| p.bone) {
                    if !bones.contains(&b) {
                        bones.push(b);
                    }
                }
                bones
            }
        }
    }

    /// True when parts `a` and `b` share a blend group or their groups are
    /// fillet partners: their surfaces are joined, so no gap can separate them.
    pub fn blend_connected(&self, a: usize, b: usize) -> bool {
        match (self.parts[a].group, self.parts[b].group) {
            (Some(ga), Some(gb)) => {
                ga == gb || self.groups[ga].partners.contains(&gb) || self.groups[gb].partners.contains(&ga)
            }
            _ => false,
        }
    }

    /// Width of the gap (outside distance) over which visibility fades out.
    pub fn visibility_width(&self, i: usize) -> f32 {
        0.1 * self.parts[i].blend + 0.002
    }

    /// 1 when the straight path from `p` to the nearest of part `i`'s bones
    /// stays inside the body, falling to 0 as it crosses a gap.
    pub fn visibility(&self, i: usize, p: Vec3) -> f32 {
        let mut anchor = None;
        let mut best = f32::INFINITY;
        for b in self.influence_bones(i) {
            let (h, t) = (self.bones[b].head, self.bones[b].tail);
            let c = closest_on_segment(p, h, t);
            let d = p.sub(c).length_squared();
            if d < best {
                best = d;
                anchor = Some(c);
            }
        }
        let Some(anchor) = anchor else { return 1.0 };
        let worst = (1..=VISIBILITY_SAMPLES)
            .map(|s| self.distance(p.lerp(anchor, s as f32 / (VISIBILITY_SAMPLES + 1) as f32)))
            .fold(f32::NEG_INFINITY, f32::max);
        (1.0 - worst / self.visibility_width(i)).clamp(0.0, 1.0)
    }

    /// How part `i` splits its own region among its bones at `p`.
    fn part_shares(&self, i: usize, p: Vec3) -> Vec<(usize, f32)> {
        let part = &self.parts[i];
        match &part.influence {
            Influence::None => Vec::new(),
            Influence::Rigid { bone } => vec![(*bone, 1.0)],
            Influence::PerPrimitive => {
                let nearest = self.prims[part.prims.clone()]
                    .iter()
                    .filter_map(|prim| prim.bone.map(|b| (b, prim.shape.distance(p))))
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                nearest.map(|(b, _)| vec![(b, 1.0)]).unwrap_or_default()
            }
            Influence::Nearest { bones } => {
                let mut near: Vec<(usize, f32)> = bones
                    .iter()
                    .map(|&b| {
                        let bone = &self.bones[b];
                        (b, segment_distance(p, bone.head, bone.tail))
                    })
                    .collect();
                near.sort_by(|a, b| a.1.total_cmp(&b.1));
                near.truncate(2);
                let inv: Vec<(usize, f32)> = near.iter().map(|&(b, d)| (b, 1.0 / (d * d + 1e-4))).collect();
                let sum: f32 = inv.iter().map(|(_, w)| w).sum();
                inv.into_iter().map(|(b, w)| (b, w / sum)).collect()
            }
            Influence::Chain { bones, joints } => chain_shares(p, bones, joints, self.settings.joint_blend),
        }
    }
}

fn combine(d: f32, dp: f32, op: Op, k: f32) -> f32 {
    match op {
        Op::Union => smin(d, dp, k),
        Op::Subtract => smax(d, -dp, k),
    }
}

/// Smooth fillet between a group and a partner. Group values already carry
/// their own cuts, so blending them cannot restore carved-away regions, and
/// siblings never meet here, so they cannot web together.
pub fn fillet(a: f32, b: f32, k: f32) -> f32 {
    smin(a, b, k)
}

pub(crate) fn closest_on_segment(p: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let ab = b.sub(a);
    let t = (p.sub(a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
    a.add(ab.scale(t))
}

fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    p.sub(closest_on_segment(p, a, b)).length()
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Bone shares along a chain: find the closest point on the joint polyline as
/// `s = segment + fraction`, then cross-fade neighbouring bones over a window
/// of `blend` segments centred on each joint (50/50 exactly at the joint).
pub fn chain_shares(p: Vec3, bones: &[usize], joints: &[Vec3], blend: f32) -> Vec<(usize, f32)> {
    let n = bones.len();
    let mut best = (f32::INFINITY, 0usize, 0.0f32);
    for i in 0..n {
        let (a, b) = (joints[i], joints[i + 1]);
        let ab = b.sub(a);
        let t = (p.sub(a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
        let d = p.sub(a.add(ab.scale(t))).length_squared();
        if d < best.0 {
            best = (d, i, t);
        }
    }
    let (_, seg, u) = best;
    let half = blend * 0.5;
    if half > 0.0 && u < half && seg > 0 {
        let t = smoothstep(-half, half, u);
        vec![(bones[seg], t), (bones[seg - 1], 1.0 - t)]
    } else if half > 0.0 && u > 1.0 - half && seg + 1 < n {
        let t = smoothstep(-half, half, u - 1.0);
        vec![(bones[seg], 1.0 - t), (bones[seg + 1], t)]
    } else {
        vec![(bones[seg], 1.0)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile;
    use crate::program::ShapeProgram;

    fn shape(json: &str) -> CompiledShape {
        compile(&ShapeProgram::from_json(json).unwrap()).unwrap()
    }

    #[test]
    fn subset_evaluation_matches_full_near_the_surface() {
        let s = shape(
            r#"{"parts":[
                {"kind":"chain","name":"a","from":[0,0,0],"dir":[1,0,0],"length":1,"radius":0.2},
                {"kind":"blob","name":"b","from":[3,0,0],"radii":0.3}
            ]}"#,
        );
        let only_a: Vec<u32> = s.parts[0].prims.clone().map(|i| i as u32).collect();
        let p = Vec3::new(0.5, 0.21, 0.0);
        assert!((s.distance(p) - s.distance_subset(p, &only_a)).abs() < 1e-6);
        assert!(s.distance(Vec3::new(0.5, 0.0, 0.0)) < 0.0);
        assert!(s.distance(Vec3::new(2.0, 0.0, 0.0)) > 0.0);
    }

    #[test]
    fn chain_weights_cross_fade_at_joints_and_stay_rigid_mid_bone() {
        let s = shape(
            r#"{"parts":[{"kind":"chain","name":"t","from":[0,0,0],"dir":[1,0,0],"length":3,"segments":3,"radius":0.2}]}"#,
        );
        let mid = s.weights(Vec3::new(1.5, 0.2, 0.0));
        assert_eq!(mid, vec![(1, 1.0)]);
        let joint = s.weights(Vec3::new(1.0, 0.2, 0.0));
        assert_eq!(joint.len(), 2);
        assert!((joint[0].1 - 0.5).abs() < 1e-4 && (joint[1].1 - 0.5).abs() < 1e-4);
        // Continuous through the joint.
        let before = s.weights(Vec3::new(0.999, 0.2, 0.0));
        let after = s.weights(Vec3::new(1.001, 0.2, 0.0));
        let w = |ws: &[(usize, f32)], b: usize| ws.iter().find(|e| e.0 == b).map_or(0.0, |e| e.1);
        assert!((w(&before, 0) - w(&after, 0)).abs() < 0.01);
    }

    #[test]
    fn membership_blends_bones_where_parts_meet() {
        let s = shape(
            r#"{"settings":{"blend":0.1},"parts":[
                {"kind":"chain","name":"body","from":[0,0,0],"dir":[0,0,1],"length":1,"radius":0.3},
                {"kind":"chain","name":"leg","attach":"body","at":0.5,"dir":[0,-1,0],"length":1,"radius":0.1}
            ]}"#,
        );
        let foot = s.weights(Vec3::new(0.1, -0.9, 0.5));
        assert_eq!(foot[0].0, 1, "far down the leg only the leg bone matters");
        assert!(foot[0].1 > 0.999);
        let hip = s.weights(Vec3::new(0.1, -0.3, 0.5));
        assert_eq!(hip.len(), 2, "where leg meets body both bones share: {:?}", hip);
        let sum: f32 = hip.iter().map(|e| e.1).sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn weights_do_not_leak_across_gaps_between_limbs() {
        // Two legs 3 cm apart: close enough that distance alone would blend them.
        let s = shape(
            r#"{"settings":{"blend":0.06},"parts":[
                {"kind":"chain","name":"hips","from":[0,1,0],"dir":[0,1,0],"length":0.3,"radius":0.16},
                {"kind":"chain","name":"leg","mirror":true,"attach":"hips","at":0,"offset":[0.075,-0.05,0],
                 "dir":[0,-1,0],"length":0.9,"radius":0.06}
            ]}"#,
        );
        let left = s.bone_index("leg.L").unwrap();
        let right = s.bone_index("leg.R").unwrap();
        // Inner thigh of the left leg, facing the right leg across the gap.
        let inner = Vec3::new(0.015, 0.5, 0.0);
        assert!(s.distance(inner).abs() < 1e-3);
        let w = s.weights(inner);
        assert!(w.iter().all(|e| e.0 != right), "right leg leaked into left thigh: {:?}", w);
        assert_eq!(w[0].0, left);
        // Without the visibility test the right leg would hold a large share here.
        let naive = (-(s.part_distance(2, inner) - s.part_distance(1, inner)) / s.parts[2].membership_softness()).exp();
        assert!(naive > 0.2, "test setup should be a genuine near-gap case ({})", naive);
        assert_eq!(s.visibility(2, inner), 0.0);
    }
}
