//! Signed distance primitives and blend operators.
//!
//! Every formula here has a twin in `glsl.rs`; keep them in step so the
//! browser preview and the exported mesh describe the same surface.

use avicreator_core::{Quat, Vec3};

/// GLSL-style sign: zero maps to zero (`f32::signum` maps it to one).
fn sign(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn mul(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x * b.x, a.y * b.y, a.z * b.z)
}

fn div(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x / b.x, a.y / b.y, a.z / b.z)
}

/// Quadratic smooth minimum; `k` is the blend radius (0 gives a hard min).
pub fn smin(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

/// Smooth maximum, used for subtraction as `smax(d, -cut, k)`.
pub fn smax(a: f32, b: f32, k: f32) -> f32 {
    -smin(-a, -b, k)
}

/// Capsule whose radius varies linearly from `ra` at `a` to `rb` at `b`.
pub fn round_cone(p: Vec3, a: Vec3, b: Vec3, ra: f32, rb: f32) -> f32 {
    let ba = b.sub(a);
    let l2 = ba.dot(ba);
    let rr = ra - rb;
    let a2 = l2 - rr * rr;
    if l2 < 1e-12 || a2 <= 1e-12 {
        // One end sphere swallows the other.
        return if ra >= rb {
            p.sub(a).length() - ra
        } else {
            p.sub(b).length() - rb
        };
    }
    let il2 = 1.0 / l2;
    let pa = p.sub(a);
    let y = pa.dot(ba);
    let z = y - l2;
    let x2 = pa.scale(l2).sub(ba.scale(y)).length_squared();
    let y2 = y * y * l2;
    let z2 = z * z * l2;
    let k = sign(rr) * rr * rr * x2;
    if sign(z) * a2 * z2 > k {
        return (x2 + z2).sqrt() * il2 - rb;
    }
    if sign(y) * a2 * y2 < k {
        return (x2 + y2).sqrt() * il2 - ra;
    }
    ((x2 * a2 * il2).sqrt() + y * rr) * il2 - ra
}

/// Axis-aligned ellipsoid distance bound, evaluated in the ellipsoid's frame.
pub fn ellipsoid(local: Vec3, r: Vec3) -> f32 {
    let k0 = div(local, r).length();
    let k1 = div(local, mul(r, r)).length();
    if k1 < 1e-12 {
        return -r.x.min(r.y).min(r.z);
    }
    k0 * (k0 - 1.0) / k1
}

/// Unsigned distance to a triangle.
pub fn triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f32 {
    let ba = b.sub(a);
    let pa = p.sub(a);
    let cb = c.sub(b);
    let pb = p.sub(b);
    let ac = a.sub(c);
    let pc = p.sub(c);
    let nor = ba.cross(ac);
    let inside = sign(ba.cross(nor).dot(pa)) + sign(cb.cross(nor).dot(pb)) + sign(ac.cross(nor).dot(pc));
    let d2 = if inside < 2.0 {
        let e = |edge: Vec3, rel: Vec3| {
            let t = (edge.dot(rel) / edge.length_squared()).clamp(0.0, 1.0);
            edge.scale(t).sub(rel).length_squared()
        };
        e(ba, pa).min(e(cb, pb)).min(e(ac, pc))
    } else {
        let d = nor.dot(pa);
        d * d / nor.length_squared()
    };
    d2.sqrt()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub const EMPTY: Aabb = Aabb {
        min: Vec3 { x: f32::INFINITY, y: f32::INFINITY, z: f32::INFINITY },
        max: Vec3 { x: f32::NEG_INFINITY, y: f32::NEG_INFINITY, z: f32::NEG_INFINITY },
    };

    pub fn from_point(p: Vec3, r: f32) -> Aabb {
        Aabb {
            min: Vec3::new(p.x - r, p.y - r, p.z - r),
            max: Vec3::new(p.x + r, p.y + r, p.z + r),
        }
    }

    pub fn union(self, o: Aabb) -> Aabb {
        Aabb {
            min: Vec3::new(self.min.x.min(o.min.x), self.min.y.min(o.min.y), self.min.z.min(o.min.z)),
            max: Vec3::new(self.max.x.max(o.max.x), self.max.y.max(o.max.y), self.max.z.max(o.max.z)),
        }
    }

    pub fn inflate(self, r: f32) -> Aabb {
        Aabb {
            min: self.min.sub(Vec3::new(r, r, r)),
            max: self.max.add(Vec3::new(r, r, r)),
        }
    }

    pub fn intersects(&self, o: &Aabb) -> bool {
        self.min.x <= o.max.x
            && self.max.x >= o.min.x
            && self.min.y <= o.max.y
            && self.max.y >= o.min.y
            && self.min.z <= o.max.z
            && self.max.z >= o.min.z
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }

    pub fn center(&self) -> Vec3 {
        self.min.add(self.max).scale(0.5)
    }

    pub fn size(&self) -> Vec3 {
        self.max.sub(self.min)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    RoundCone { a: Vec3, b: Vec3, ra: f32, rb: f32 },
    Ellipsoid { center: Vec3, rot: Quat, radii: Vec3 },
    Slab { a: Vec3, b: Vec3, c: Vec3, half_thickness: f32 },
    /// A muscle, as relief on the body it lies on: over its footprint (an
    /// ellipse `radii[0]` wide, drawn out along local +Z from `origin` over
    /// `length` and full-width from `belly[0]` to `belly[1]`) it raises the
    /// surface by up to `height`, as a dome falling to zero at the edges.
    /// `radii[1]` is the belly's half-thickness, which sets how far from its
    /// centre (along local Y) a surface can lie and still be raised.
    /// `distance` returns minus the raise (negative inside the footprint).
    Muscle { origin: Vec3, rot: Quat, length: f32, belly: [f32; 2], radii: [f32; 2], height: f32 },
}

/// How far across its footprint (along local Y) a muscle's relief reaches:
/// outward (+Y, the side it bulges to) in full past its half-thickness plus
/// the soft tissue over it, inward only a little past its own depth, then
/// fading out over the last distance either way. Returns (outward, inward,
/// fade); the inward side stays short to leave the far side of a limb alone.
pub fn muscle_reach(half_thickness: f32, height: f32) -> (f32, f32, f32) {
    (half_thickness + 0.025, half_thickness + 0.005, (2.0 * height).max(0.015))
}

/// How far a muscle raises the surface at `local` (its frame): a parabolic
/// dome over the footprint, negative outside it.
pub fn muscle_relief(local: Vec3, length: f32, belly: [f32; 2], radii: [f32; 2], height: f32) -> f32 {
    let z = if local.z < belly[0] {
        (local.z - belly[0]) / belly[0]
    } else if local.z > belly[1] {
        (local.z - belly[1]) / (length - belly[1])
    } else {
        0.0
    };
    let x = local.x / radii[0];
    let (out, inward, fade) = muscle_reach(radii[1], height);
    let y = if local.y > 0.0 { local.y - out } else { -local.y - inward }.max(0.0) / fade;
    height * (1.0 - (x * x + y * y + z * z))
}

/// Steepest slope of a muscle's relief where, smoothed by `k` against its
/// neighbours and its edge, it still raises the surface.
pub fn muscle_slope(length: f32, belly: [f32; 2], radii: [f32; 2], height: f32, k: f32) -> f32 {
    let (_, _, fade) = muscle_reach(radii[1], height);
    let narrowest = radii[0].min(belly[0]).min(length - belly[1]).min(fade);
    2.0 * (height * height + height * k).sqrt() / narrowest
}

/// Box around everywhere a muscle's relief exceeds `-2k`, beyond which,
/// smoothed by `k`, it can no longer change the surface.
pub fn muscle_bounds(origin: Vec3, rot: Quat, length: f32, belly: [f32; 2], radii: [f32; 2], height: f32, k: f32) -> Aabb {
    let e = (1.0 + 2.0 * k / height).sqrt();
    let (out, inward, fade) = muscle_reach(radii[1], height);
    let axis = rot.rotate_vec3(Vec3::Z);
    // Centred between the inward and outward reach.
    let shift = rot.rotate_vec3(Vec3::Y).scale((out - inward) * 0.5);
    let half = |center: f32, z: f32| Shape::Ellipsoid {
        center: origin.add(axis.scale(center)).add(shift),
        rot,
        radii: Vec3::new(radii[0] * e, (out + inward) * 0.5 + fade * e, z * e),
    };
    half(belly[0], belly[0]).bounds().union(half(belly[1], length - belly[1]).bounds())
}

impl Shape {
    pub fn distance(&self, p: Vec3) -> f32 {
        match self {
            Shape::RoundCone { a, b, ra, rb } => round_cone(p, *a, *b, *ra, *rb),
            Shape::Ellipsoid { center, rot, radii } => {
                ellipsoid(rot.conjugate().rotate_vec3(p.sub(*center)), *radii)
            }
            Shape::Slab { a, b, c, half_thickness } => triangle(p, *a, *b, *c) - half_thickness,
            Shape::Muscle { origin, rot, length, belly, radii, height } => {
                -muscle_relief(rot.conjugate().rotate_vec3(p.sub(*origin)), *length, *belly, *radii, *height)
            }
        }
    }

    pub fn bounds(&self) -> Aabb {
        match self {
            Shape::RoundCone { a, b, ra, rb } => Aabb::from_point(*a, *ra).union(Aabb::from_point(*b, *rb)),
            Shape::Ellipsoid { center, rot, radii } => {
                // Exact extents of a rotated ellipsoid along each world axis.
                let ax = rot.rotate_vec3(Vec3::X).scale(radii.x);
                let ay = rot.rotate_vec3(Vec3::Y).scale(radii.y);
                let az = rot.rotate_vec3(Vec3::Z).scale(radii.z);
                let ext = Vec3::new(
                    (ax.x * ax.x + ay.x * ay.x + az.x * az.x).sqrt(),
                    (ax.y * ax.y + ay.y * ay.y + az.y * az.y).sqrt(),
                    (ax.z * ax.z + ay.z * ay.z + az.z * az.z).sqrt(),
                );
                Aabb { min: center.sub(ext), max: center.add(ext) }
            }
            Shape::Slab { a, b, c, half_thickness } => Aabb::from_point(*a, *half_thickness)
                .union(Aabb::from_point(*b, *half_thickness))
                .union(Aabb::from_point(*c, *half_thickness)),
            Shape::Muscle { origin, rot, length, belly, radii, height } => {
                muscle_bounds(*origin, *rot, *length, *belly, *radii, *height, 0.0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_cone_matches_capsule_and_sphere_limits() {
        let a = Vec3::ZERO;
        let b = Vec3::new(0.0, 2.0, 0.0);
        // Equal radii: a capsule.
        assert!((round_cone(Vec3::new(1.0, 1.0, 0.0), a, b, 0.5, 0.5) - 0.5).abs() < 1e-5);
        assert!((round_cone(Vec3::new(0.0, 3.0, 0.0), a, b, 0.5, 0.5) - 0.5).abs() < 1e-5);
        // Tapered: inside near the fat end, on the surface at the thin tip.
        assert!(round_cone(Vec3::new(0.0, 0.1, 0.0), a, b, 0.5, 0.1) < 0.0);
        assert!(round_cone(Vec3::new(0.0, 2.1, 0.0), a, b, 0.5, 0.1).abs() < 1e-4);
        // Degenerate: the larger sphere contains the smaller one.
        assert!((round_cone(Vec3::new(3.0, 0.0, 0.0), a, Vec3::new(0.1, 0.0, 0.0), 1.0, 0.2) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn ellipsoid_and_triangle_distances() {
        let r = Vec3::new(1.0, 2.0, 3.0);
        assert!(ellipsoid(Vec3::new(1.0, 0.0, 0.0), r).abs() < 1e-5);
        assert!(ellipsoid(Vec3::new(0.0, 0.0, 3.0), r).abs() < 1e-5);
        assert!(ellipsoid(Vec3::ZERO, r) < 0.0);
        assert!((ellipsoid(Vec3::new(2.0, 0.0, 0.0), r) - 1.0).abs() < 1e-5);

        let (a, b, c) = (Vec3::ZERO, Vec3::X, Vec3::Y);
        assert!((triangle(Vec3::new(0.2, 0.2, 0.5), a, b, c) - 0.5).abs() < 1e-5);
        assert!((triangle(Vec3::new(-1.0, 0.0, 0.0), a, b, c) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn smooth_blends_bulge_only_near_each_other() {
        assert_eq!(smin(0.0, 1.0, 0.1), 0.0);
        assert!(smin(0.0, 0.0, 0.1) < 0.0);
        assert_eq!(smin(0.3, 0.5, 0.0), 0.3);
        assert!(smax(0.0, 0.0, 0.1) > 0.0);
    }

    #[test]
    fn muscle_relief_is_a_dome_over_the_footprint() {
        let rot = Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2); // local Z -> world Y
        let m = Shape::Muscle { origin: Vec3::ZERO, rot, length: 0.3, belly: [0.1, 0.2], radii: [0.04, 0.02], height: 0.01 };
        let at = |x: f32, y: f32, z: f32| rot.rotate_vec3(Vec3::new(x, y, z));
        // Full height along the belly stretch, for surfaces near the belly.
        for z in [0.1, 0.15, 0.2] {
            assert!((m.distance(at(0.0, 0.02, z)) + 0.01).abs() < 1e-6);
        }
        // Falling to zero at the footprint's edges and ends, negative beyond.
        assert!(m.distance(at(0.04, 0.0, 0.15)).abs() < 1e-6);
        assert!(m.distance(Vec3::ZERO).abs() < 1e-6 && m.distance(at(0.0, 0.0, 0.3)).abs() < 1e-6);
        assert!(m.distance(at(0.05, 0.0, 0.15)) > 0.0);
        // Reaching out to surfaces over it, but not across the limb.
        let (out, inward, fade) = muscle_reach(0.02, 0.01);
        assert!((m.distance(at(0.0, out, 0.15)) + 0.01).abs() < 1e-6);
        assert!(m.distance(at(0.0, -(inward + fade + 0.001), 0.15)) > 0.0);
        assert!(m.distance(at(0.0, out + fade + 0.001, 0.15)) > 0.0);
        let bb = m.bounds();
        assert!(bb.min.y < 1e-4 && bb.max.y > 0.2999 && (bb.max.x - 0.04).abs() < 1e-5);
        // Local +Y is world -Z here.
        assert!((bb.min.z + out + fade).abs() < 1e-5 && (bb.max.z - inward - fade).abs() < 1e-5);
        assert!(muscle_slope(0.3, [0.1, 0.2], [0.04, 0.02], 0.01, 0.0) <= 1.0 + 1e-6);
        // Grown bounds reach exactly where the relief falls to -2k.
        let k = 0.005;
        let grown = muscle_bounds(Vec3::ZERO, rot, 0.3, [0.1, 0.2], [0.04, 0.02], 0.01, k);
        let e = 2.0f32.sqrt();
        for p in [at(0.04 * e, 0.0, 0.15), at(0.0, 0.0, 0.1 - 0.1 * e), at(0.0, -(inward + fade * e), 0.15), at(0.0, out + fade * e, 0.15)] {
            assert!((m.distance(p) - 2.0 * k).abs() < 1e-5, "{}", m.distance(p));
            assert!(grown.inflate(1e-5).intersects(&Aabb::from_point(p, 0.0)));
        }
        assert!(grown.max.x > 0.0565 && grown.min.y < -0.0414);
    }

    #[test]
    fn rotated_ellipsoid_bounds_are_tight() {
        let rot = Quat::from_axis_angle(Vec3::Z, std::f32::consts::FRAC_PI_2);
        let s = Shape::Ellipsoid { center: Vec3::ZERO, rot, radii: Vec3::new(2.0, 0.5, 0.5) };
        let bb = s.bounds();
        assert!((bb.max.y - 2.0).abs() < 1e-5);
        assert!((bb.max.x - 0.5).abs() < 1e-5);
    }
}
