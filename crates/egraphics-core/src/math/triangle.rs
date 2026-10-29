use crate::Ray;
use glam::Vec3;

/// A single triangle defined by three vertices.
#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub vertices: [Vec3; 3],
}

impl Triangle {
    pub fn new(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self {
            vertices: [a, b, c],
        }
    }

    /// Möller–Trumbore ray–triangle intersection.
    ///
    /// Returns `Some(t)` when the ray hits the front face at `t > 0`.
    pub fn intersect_ray(&self, ray: &Ray) -> Option<f32> {
        let edge1 = self.vertices[1] - self.vertices[0];
        let edge2 = self.vertices[2] - self.vertices[0];
        let h = ray.dir.cross(edge2);
        let a = edge1.dot(h);
        if a.abs() < f32::EPSILON {
            return None; // ray is parallel to triangle
        }
        let f = 1.0 / a;
        let s = ray.origin - self.vertices[0];
        let u = f * s.dot(h);
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let q = s.cross(edge1);
        let v = f * ray.dir.dot(q);
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        let t = f * edge2.dot(q);
        if t > f32::EPSILON { Some(t) } else { None }
    }
}
