use glam::Vec3;

/// A ray defined by an origin point and a unit-length direction.
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    /// World-space origin.
    pub origin: Vec3,
    /// Unit-length direction vector.
    pub dir: Vec3,
}

impl Ray {
    /// Constructs a new ray. `dir` is normalised before storage.
    #[inline]
    pub fn new(origin: Vec3, dir: Vec3) -> Self {
        Self {
            origin,
            dir: dir.normalize(),
        }
    }

    /// Returns the point at parameter `t` along the ray.
    #[inline]
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.dir * t
    }
}
