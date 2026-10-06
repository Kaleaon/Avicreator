#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    pub const ONE: Vec3 = Vec3 { x: 1.0, y: 1.0, z: 1.0 };
    pub const X: Vec3 = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
    pub const Y: Vec3 = Vec3 { x: 0.0, y: 1.0, z: 0.0 };
    pub const Z: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 1.0 };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn add(self, rhs: Vec3) -> Vec3 {
        Vec3::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }

    pub fn sub(self, rhs: Vec3) -> Vec3 {
        Vec3::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }

    pub fn scale(self, factor: f32) -> Vec3 {
        Vec3::new(self.x * factor, self.y * factor, self.z * factor)
    }

    pub fn dot(self, rhs: Vec3) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    pub fn cross(self, rhs: Vec3) -> Vec3 {
        Vec3::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn normalize(self) -> Vec3 {
        let len = self.length();
        if len > 1e-6 {
            self.scale(1.0 / len)
        } else {
            Vec3::ZERO
        }
    }

    pub fn lerp(self, rhs: Vec3, t: f32) -> Vec3 {
        self.scale(1.0 - t).add(rhs.scale(t))
    }

    pub fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    pub fn from_array(arr: [f32; 3]) -> Self {
        Self::new(arr[0], arr[1], arr[2])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Default for Quat {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Quat {
    pub const IDENTITY: Quat = Quat { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };

    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn dot(self, rhs: Quat) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z + self.w * rhs.w
    }

    pub fn normalize(self) -> Quat {
        let len = (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt();
        if len > 1e-6 {
            let inv = 1.0 / len;
            Quat::new(self.x * inv, self.y * inv, self.z * inv, self.w * inv)
        } else {
            Quat::IDENTITY
        }
    }

    pub fn slerp(self, mut rhs: Quat, t: f32) -> Quat {
        let mut cos_theta = self.dot(rhs);

        if cos_theta < 0.0 {
            rhs = Quat::new(-rhs.x, -rhs.y, -rhs.z, -rhs.w);
            cos_theta = -cos_theta;
        }

        if cos_theta > 0.9995 {
            return Quat::new(
                self.x + t * (rhs.x - self.x),
                self.y + t * (rhs.y - self.y),
                self.z + t * (rhs.z - self.z),
                self.w + t * (rhs.w - self.w),
            )
            .normalize();
        }

        let theta = cos_theta.acos();
        let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
        let sin_t_theta = (t * theta).sin();
        let sin_one_minus_t_theta = ((1.0 - t) * theta).sin();

        let w1 = sin_one_minus_t_theta / sin_theta;
        let w2 = sin_t_theta / sin_theta;

        Quat::new(
            self.x * w1 + rhs.x * w2,
            self.y * w1 + rhs.y * w2,
            self.z * w1 + rhs.z * w2,
            self.w * w1 + rhs.w * w2,
        )
    }

    pub fn rotate_vec3(&self, v: Vec3) -> Vec3 {
        let u = Vec3::new(self.x, self.y, self.z);
        let s = self.w;
        let u_cross_v = u.cross(v);
        let u_cross_u_cross_v = u.cross(u_cross_v);

        v.add(u_cross_v.scale(2.0 * s)).add(u_cross_u_cross_v.scale(2.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4 {
    pub m: [f32; 16], // Column-major
}

impl Default for Mat4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4 {
        m: [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ],
    };

    pub fn from_translation(t: Vec3) -> Self {
        let mut mat = Self::IDENTITY;
        mat.m[12] = t.x;
        mat.m[13] = t.y;
        mat.m[14] = t.z;
        mat
    }

    pub fn from_scale(s: Vec3) -> Self {
        let mut mat = Self::IDENTITY;
        mat.m[0] = s.x;
        mat.m[5] = s.y;
        mat.m[10] = s.z;
        mat
    }

    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        let x = self.m[0] * p.x + self.m[4] * p.y + self.m[8] * p.z + self.m[12];
        let y = self.m[1] * p.x + self.m[5] * p.y + self.m[9] * p.z + self.m[13];
        let z = self.m[2] * p.x + self.m[6] * p.y + self.m[10] * p.z + self.m[14];
        let w = self.m[3] * p.x + self.m[7] * p.y + self.m[11] * p.z + self.m[15];

        if w.abs() > 1e-6 && (w - 1.0).abs() > 1e-6 {
            Vec3::new(x / w, y / w, z / w)
        } else {
            Vec3::new(x, y, z)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec3_operations() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);

        assert_eq!(a.add(b), Vec3::new(5.0, 7.0, 9.0));
        assert_eq!(b.sub(a), Vec3::new(3.0, 3.0, 3.0));
        assert_eq!(a.scale(2.0), Vec3::new(2.0, 4.0, 6.0));
        assert_eq!(a.dot(b), 4.0 + 10.0 + 18.0);

        let cross = Vec3::X.cross(Vec3::Y);
        assert_eq!(cross, Vec3::Z);
    }

    #[test]
    fn test_quat_rotation() {
        // 90 degree rotation around Z axis: quat = (0, 0, sin(45 deg), cos(45 deg))
        let sin45 = 0.5f32.sqrt();
        let q = Quat::new(0.0, 0.0, sin45, sin45);

        let v = Vec3::X; // (1, 0, 0)
        let rotated = q.rotate_vec3(v);

        // Rotated 90 deg around Z gives approximately (0, 1, 0)
        assert!((rotated.x - 0.0).abs() < 1e-5);
        assert!((rotated.y - 1.0).abs() < 1e-5);
        assert!((rotated.z - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_mat4_translation() {
        let t = Vec3::new(10.0, -5.0, 2.0);
        let mat = Mat4::from_translation(t);
        let p = Vec3::new(1.0, 1.0, 1.0);
        let transformed = mat.transform_point(p);

        assert_eq!(transformed, Vec3::new(11.0, -4.0, 3.0));
    }
}
