use glam::{Mat4, Vec3};

use super::{Frustum, Ray};

/// Axis-aligned bounding box stored as min/max corner pairs.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "gpu", derive(bytemuck::Pod, bytemuck::Zeroable))]
#[repr(C)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    /// Constructs an AABB from explicit min/max corners.
    #[inline]
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Returns an invalid AABB with `min = +∞` and `max = -∞`.
    ///
    /// This is the identity element for [`Aabb::union`]: unioning any valid box with this
    /// yields that box unchanged. Use it as the starting value when accumulating bounds
    /// over a collection of objects.
    ///
    /// The resulting box has no geometric meaning on its own; make sure `min <= max` holds on
    /// every axis before using it for intersection tests or spatial queries.
    #[inline]
    pub fn new_invalid() -> Aabb {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
        }
    }

    /// Returns the smallest AABB containing both `self` and `other`.
    #[inline]
    pub fn union(&self, other: &Aabb) -> Aabb {
        Aabb {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// Surface area of the box (sum of six face areas).
    #[inline]
    pub fn surface_area(&self) -> f32 {
        let d = (self.max - self.min).max(Vec3::ZERO);
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }

    /// Geometric centre of the box.
    #[inline]
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Index of the longest axis: 0 = X, 1 = Y, 2 = Z.
    #[inline]
    pub fn longest_axis(&self) -> usize {
        let d = self.max - self.min;
        if d.x >= d.y && d.x >= d.z {
            0
        } else if d.y >= d.z {
            1
        } else {
            2
        }
    }

    /// Returns all eight corners of the box in `(x, y, z)` bit-index order:
    /// `[0]` = min, `[7]` = max, bit 0 → X, bit 1 → Y, bit 2 → Z.
    #[inline]
    pub fn corners(&self) -> [Vec3; 8] {
        let (mn, mx) = (self.min, self.max);
        [
            Vec3::new(mn.x, mn.y, mn.z),
            Vec3::new(mx.x, mn.y, mn.z),
            Vec3::new(mn.x, mx.y, mn.z),
            Vec3::new(mx.x, mx.y, mn.z),
            Vec3::new(mn.x, mn.y, mx.z),
            Vec3::new(mx.x, mn.y, mx.z),
            Vec3::new(mn.x, mx.y, mx.z),
            Vec3::new(mx.x, mx.y, mx.z),
        ]
    }

    /// Transforms the AABB by `mat` using the eight-corner method.
    pub fn transform(&self, mat: Mat4) -> Aabb {
        self.corners().iter().fold(Aabb::new_invalid(), |acc, &c| {
            let t = mat.transform_point3(c);
            Aabb {
                min: acc.min.min(t),
                max: acc.max.max(t),
            }
        })
    }

    /// Slab-method ray–AABB intersection.
    ///
    /// Returns `Some(t_near)` when the ray hits and `t_near >= 0`.
    pub fn intersect_ray(&self, ray: &Ray) -> Option<f32> {
        let inv = ray.dir.recip();
        let t1 = (self.min - ray.origin) * inv;
        let t2 = (self.max - ray.origin) * inv;
        let t_near = t1.min(t2);
        let t_far = t1.max(t2);
        let t_enter = t_near.max_element();
        let t_exit = t_far.min_element();
        if t_exit >= t_enter && t_enter >= 0.0 {
            Some(t_enter)
        } else {
            None
        }
    }

    /// Returns `true` when the AABB is not fully outside any frustum half-space.
    ///
    /// Uses the p-vertex optimisation: for each plane test the corner most
    /// aligned with the inward normal.
    pub fn intersect_frustum(&self, frustum: &Frustum) -> bool {
        for plane in &frustum.planes {
            let n = glam::Vec3::new(plane.x, plane.y, plane.z);
            let p = Vec3::new(
                if n.x >= 0.0 { self.max.x } else { self.min.x },
                if n.y >= 0.0 { self.max.y } else { self.min.y },
                if n.z >= 0.0 { self.max.z } else { self.min.z },
            );
            if n.dot(p) + plane.w < 0.0 {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_aabb() -> Aabb {
        Aabb::new(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5))
    }

    #[test]
    fn aabb_ray_hit() {
        let aabb = unit_aabb();
        let ray = Ray::new(Vec3::new(-1.0, 0.0, 0.0), Vec3::X);
        let t = aabb.intersect_ray(&ray).expect("should hit");
        approx::assert_abs_diff_eq!(t, 0.5, epsilon = 1e-5);
    }

    #[test]
    fn aabb_frustum() {
        use super::super::frustum::Frustum;
        use glam::Vec3;
        use glam::camera::rh::{proj, view};
        // Camera at (0,0,5) looking at the origin — the unit AABB at origin
        // must be inside its frustum.
        let view = view::look_at_mat4(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        // Simple symmetric perspective (fov ~53°).
        let proj = proj::directx::perspective(std::f32::consts::FRAC_PI_3, 1.0, 0.1, 100.0);
        let frustum = Frustum::from_view_proj(proj * view);
        assert!(unit_aabb().intersect_frustum(&frustum));
    }

    #[test]
    fn aabb_ray_miss() {
        let aabb = unit_aabb();
        let ray = Ray::new(Vec3::new(-1.0, 2.0, 0.0), Vec3::X);
        assert!(aabb.intersect_ray(&ray).is_none());
    }
}
