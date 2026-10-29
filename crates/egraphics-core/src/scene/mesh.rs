use std::collections::HashMap;

use glam::Vec3;

use crate::math::{Aabb, Ray, Triangle};

/// A triangle mesh stored as an indexed vertex list.
#[derive(Debug, Clone)]
pub struct TriangleMesh {
    pub vertices: Vec<Vec3>,
    /// Each element is a triple of vertex indices `[i0, i1, i2]`.
    pub indices: Vec<[u32; 3]>,

    pub geometry_id: Option<String>,
}

impl TriangleMesh {
    /// Constructs a new mesh. Identity is assigned by [`Scene::add_mesh`](super::Scene::add_mesh),
    /// which returns a [`MeshKey`](super::MeshKey).
    pub fn new(vertices: Vec<Vec3>, indices: Vec<[u32; 3]>) -> Self {
        Self {
            vertices,
            indices,
            geometry_id: None,
        }
    }

    pub fn from_triangles(triangles: Vec<Triangle>) -> Self {
        let mut vertices = Vec::with_capacity(triangles.len() * 3);
        let mut indices = Vec::with_capacity(triangles.len());
        for (i, tri) in triangles.iter().enumerate() {
            let base = (i * 3) as u32;
            vertices.extend_from_slice(&tri.vertices);
            indices.push([base, base + 1, base + 2]);
        }

        Self {
            vertices,
            indices,
            geometry_id: None,
        }
    }

    /// Consumes multiple meshes and combines them into a single mesh by
    /// concatenating their vertex and index buffers, offsetting each mesh's
    /// indices by the running vertex count.
    pub fn from_triangle_meshes(meshes: Vec<TriangleMesh>) -> Self {
        let total_vertices: usize = meshes.iter().map(|m| m.vertices.len()).sum();
        let total_indices: usize = meshes.iter().map(|m| m.indices.len()).sum();
        let mut vertices = Vec::with_capacity(total_vertices);
        let mut indices = Vec::with_capacity(total_indices);

        for mesh in meshes {
            let offset = vertices.len() as u32;
            indices.extend(
                mesh.indices
                    .iter()
                    .map(|&[i0, i1, i2]| [i0 + offset, i1 + offset, i2 + offset]),
            );
            vertices.extend(mesh.vertices);
        }

        Self {
            vertices,
            indices,
            geometry_id: None,
        }
    }

    /// Returns a new mesh with duplicate vertices merged and indices remapped.
    ///
    /// Equality is determined by exact bit patterns, so NaN vertices are treated
    /// as distinct and -0.0 / +0.0 as different values.
    pub fn deduplicate_vertices(&self) -> Self {
        let mut seen: HashMap<[u32; 3], u32> = HashMap::new();
        let mut vertices: Vec<Vec3> = Vec::new();
        let mut indices: Vec<[u32; 3]> = Vec::with_capacity(self.indices.len());

        for &[i0, i1, i2] in &self.indices {
            let mut remap = |i: u32| -> u32 {
                let v = self.vertices[i as usize];
                let key = [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()];
                *seen.entry(key).or_insert_with(|| {
                    let idx = vertices.len() as u32;
                    vertices.push(v);
                    idx
                })
            };
            indices.push([remap(i0), remap(i1), remap(i2)]);
        }

        Self {
            vertices,
            indices,
            geometry_id: self.geometry_id.clone(),
        }
    }

    /// Computes the tight AABB of all vertices.
    pub fn compute_aabb(&self) -> Aabb {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for &v in &self.vertices {
            min = min.min(v);
            max = max.max(v);
        }
        Aabb::new(min, max)
    }

    /// Returns `Some((t, triangle_index))` for the nearest ray hit.
    pub fn intersect_ray(&self, ray: &Ray) -> Option<(f32, usize)> {
        let mut nearest: Option<(f32, usize)> = None;
        for (i, &[i0, i1, i2]) in self.indices.iter().enumerate() {
            let tri = Triangle {
                vertices: [
                    self.vertices[i0 as usize],
                    self.vertices[i1 as usize],
                    self.vertices[i2 as usize],
                ],
            };
            if let Some(t) = tri.intersect_ray(ray)
                && nearest.is_none_or(|(tn, _)| t < tn)
            {
                nearest = Some((t, i));
            }
        }
        nearest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_triangle() -> Triangle {
        Triangle {
            vertices: [
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
        }
    }

    #[test]
    fn moller_trumbore_hit() {
        let tri = flat_triangle();
        // Ray from below aimed at the centroid (0, 1/3, 0)
        let ray = Ray::new(Vec3::new(0.0, 1.0 / 3.0, -2.0), Vec3::Z);
        let t = tri.intersect_ray(&ray).expect("should hit");
        approx::assert_abs_diff_eq!(t, 2.0, epsilon = 1e-5);
    }

    #[test]
    fn moller_trumbore_miss() {
        let tri = flat_triangle();
        let ray = Ray::new(Vec3::new(5.0, 0.0, -1.0), Vec3::Z);
        assert!(tri.intersect_ray(&ray).is_none());
    }

    #[test]
    fn mesh_nearest_hit() {
        // Two triangles at z=0 and z=1; ray should hit z=0 first.
        let verts = vec![
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
        ];
        let idxs = vec![[0, 1, 2], [3, 4, 5]];
        let mesh = TriangleMesh::new(verts, idxs);
        let ray = Ray::new(Vec3::new(0.0, 0.0, -2.0), Vec3::Z);
        let (t, idx) = mesh.intersect_ray(&ray).expect("should hit");
        assert_eq!(idx, 0);
        approx::assert_abs_diff_eq!(t, 2.0, epsilon = 1e-5);
    }
}
