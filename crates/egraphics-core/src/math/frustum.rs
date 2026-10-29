use glam::{Mat4, Vec4};

/// Six clip planes in world space with inward-facing normals.
///
/// Each plane is stored as `Vec4(nx, ny, nz, d)` where a point `p` is inside
/// when `dot(n, p) + d >= 0`.
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    /// `[left, right, bottom, top, near, far]` in that order.
    pub planes: [Vec4; 6],
}

impl Frustum {
    /// Extracts the six frustum planes from a column-major view-projection
    /// matrix using the Gribb–Hartmann method (right-handed, depth 0..1).
    pub fn from_view_proj(vp: Mat4) -> Self {
        let c0 = vp.col(0);
        let c1 = vp.col(1);
        let c2 = vp.col(2);
        let c3 = vp.col(3);

        // Reconstruct matrix rows from columns.
        let row0 = Vec4::new(c0.x, c1.x, c2.x, c3.x);
        let row1 = Vec4::new(c0.y, c1.y, c2.y, c3.y);
        let row2 = Vec4::new(c0.z, c1.z, c2.z, c3.z);
        let row3 = Vec4::new(c0.w, c1.w, c2.w, c3.w);

        let planes = [
            row3 + row0, // left
            row3 - row0, // right
            row3 + row1, // bottom
            row3 - row1, // top
            row2,        // near (depth >= 0)
            row3 - row2, // far  (depth <= w)
        ];

        // Normalise plane normals so distances are consistent.
        let planes = planes.map(|p| {
            let len = Vec4::new(p.x, p.y, p.z, 0.0).length();
            if len > 1e-8 { p / len } else { p }
        });

        Self { planes }
    }
}
