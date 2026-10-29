use slotmap::SlotMap;

use crate::math::{Aabb, Frustum, Ray};
use crate::scene::{MeshKey, Object, TriangleMesh};

/// Selects the algorithm used when building the BVH.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStrategy {
    /// Fast midpoint split along the longest centroid-AABB axis.
    Midpoint,
    /// Surface Area Heuristic — higher quality, slower to build.
    Sah,
}

/// A node in the flat BVH array.
///
/// Leaf nodes have `left_child == u32::MAX`; `right_child` then holds the
/// index into the objects slice.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "gpu", derive(bytemuck::Pod, bytemuck::Zeroable))]
#[repr(C)]
pub struct BvhNode {
    pub aabb: Aabb,
    /// Index of the left child, or `u32::MAX` for a leaf.
    pub left_child: u32,
    /// Index of the right child, or the object-slice index for a leaf.
    pub right_child: u32,
}

/// Flat BVH acceleration structure.
#[derive(Debug, Default)]
pub struct Bvh {
    nodes: Vec<BvhNode>,
}

impl Bvh {
    /// Builds a flat BVH over `objects` using the requested strategy.
    pub fn build(objects: &[&Object], strategy: BuildStrategy) -> Self {
        if objects.is_empty() {
            return Self::default();
        }
        let mut indices: Vec<usize> = (0..objects.len()).collect();
        let mut nodes = Vec::with_capacity(objects.len() * 2);
        build_recursive(&mut nodes, objects, &mut indices, strategy);
        Self { nodes }
    }

    /// Returns the indices (into the `objects` slice passed to [`Bvh::build`]) of objects
    /// whose world AABB intersects the frustum.
    pub fn query_frustum(&self, frustum: &Frustum) -> Vec<u32> {
        if self.nodes.is_empty() {
            return vec![];
        }
        let mut stack = vec![0u32];
        let mut results = Vec::new();
        while let Some(idx) = stack.pop() {
            let node = &self.nodes[idx as usize];
            if !node.aabb.intersect_frustum(frustum) {
                continue;
            }
            if node.left_child == u32::MAX {
                results.push(node.right_child);
            } else {
                stack.push(node.left_child);
                stack.push(node.right_child);
            }
        }
        results
    }

    /// Returns `(index, t)` pairs for all ray hits, sorted by `t` ascending, where `index`
    /// is the position in the `objects` slice passed to [`Bvh::build`].
    ///
    /// `objects` must be the same slice (same order) passed to [`Bvh::build`].
    pub fn query_ray(
        &self,
        ray: &Ray,
        objects: &[&Object],
        mesh_store: &SlotMap<MeshKey, TriangleMesh>,
    ) -> Vec<(u32, f32)> {
        if self.nodes.is_empty() {
            return vec![];
        }
        let mut stack = vec![0u32];
        let mut results = Vec::new();
        while let Some(idx) = stack.pop() {
            let node = &self.nodes[idx as usize];
            if node.aabb.intersect_ray(ray).is_none() {
                continue;
            }
            if node.left_child == u32::MAX {
                let obj = &objects[node.right_child as usize];
                if let Some(t) = obj.intersect_ray(ray, mesh_store) {
                    results.push((node.right_child, t));
                }
            } else {
                stack.push(node.left_child);
                stack.push(node.right_child);
            }
        }
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Returns a slice of all BVH nodes (used by the GPU backend).
    pub fn nodes(&self) -> &[BvhNode] {
        &self.nodes
    }
}

// ---------------------------------------------------------------------------
// Build helpers
// ---------------------------------------------------------------------------

fn build_recursive(
    nodes: &mut Vec<BvhNode>,
    objects: &[&Object],
    indices: &mut [usize],
    strategy: BuildStrategy,
) -> u32 {
    let node_idx = nodes.len() as u32;
    nodes.push(BvhNode {
        aabb: Aabb::new_invalid(),
        left_child: u32::MAX,
        right_child: 0,
    });

    // Compute combined AABB of all objects in this subtree.
    let combined = indices
        .iter()
        .map(|&i| objects[i].world_aabb())
        .fold(Aabb::new_invalid(), |a, b| a.union(&b));

    if indices.len() == 1 {
        nodes[node_idx as usize] = BvhNode {
            aabb: combined,
            left_child: u32::MAX,
            right_child: indices[0] as u32,
        };
        return node_idx;
    }

    // Choose split.
    let split = match strategy {
        BuildStrategy::Midpoint => midpoint_split(objects, indices),
        BuildStrategy::Sah => sah_split(objects, indices),
    };

    let (left_slice, right_slice) = indices.split_at_mut(split);
    let left_idx = build_recursive(nodes, objects, left_slice, strategy);
    let right_idx = build_recursive(nodes, objects, right_slice, strategy);

    nodes[node_idx as usize] = BvhNode {
        aabb: combined,
        left_child: left_idx,
        right_child: right_idx,
    };
    node_idx
}

fn centroid_aabb(objects: &[&Object], indices: &[usize]) -> Aabb {
    indices
        .iter()
        .map(|&i| {
            let c = objects[i].world_aabb().center();
            Aabb::new(c, c)
        })
        .fold(Aabb::new_invalid(), |a, b| a.union(&b))
}

fn midpoint_split(objects: &[&Object], indices: &mut [usize]) -> usize {
    let cab = centroid_aabb(objects, indices);
    let axis = cab.longest_axis();
    let mid = cab.center()[axis];

    let split = partition(indices, |&i| objects[i].world_aabb().center()[axis] < mid);

    if split == 0 || split == indices.len() {
        indices.len() / 2
    } else {
        split
    }
}

const SAH_BUCKETS: usize = 12;

fn sah_split(objects: &[&Object], indices: &mut [usize]) -> usize {
    let cab = centroid_aabb(objects, indices);
    let parent_sa = objects
        .iter()
        .enumerate()
        .filter(|(i, _)| indices.contains(i))
        .map(|(i, _)| objects[i].world_aabb())
        .fold(Aabb::new_invalid(), |a, b| a.union(&b))
        .surface_area();

    let mut best_cost = f32::INFINITY;
    let mut best_axis = 0;
    let mut best_split = indices.len() / 2;

    for axis in 0..3usize {
        let extent = cab.max[axis] - cab.min[axis];
        if extent < f32::EPSILON {
            continue;
        }

        // Assign each object to a bucket.
        let mut buckets: [(Aabb, u32); SAH_BUCKETS] = [(Aabb::new_invalid(), 0); SAH_BUCKETS];

        for &i in indices.iter() {
            let c = objects[i].world_aabb().center()[axis];
            let b =
                ((SAH_BUCKETS as f32 * (c - cab.min[axis]) / extent) as usize).min(SAH_BUCKETS - 1);
            buckets[b].0 = buckets[b].0.union(&objects[i].world_aabb());
            buckets[b].1 += 1;
        }

        // Evaluate cost at each split plane.
        for split_at in 1..SAH_BUCKETS {
            let left = buckets[..split_at]
                .iter()
                .filter(|(_, n)| *n > 0)
                .fold((Aabb::new_invalid(), 0u32), |(ab, cnt), (a, n)| {
                    (ab.union(a), cnt + n)
                });
            let right = buckets[split_at..]
                .iter()
                .filter(|(_, n)| *n > 0)
                .fold((Aabb::new_invalid(), 0u32), |(ab, cnt), (a, n)| {
                    (ab.union(a), cnt + n)
                });

            if left.1 == 0 || right.1 == 0 {
                continue;
            }

            let cost = 1.0
                + (left.1 as f32 * left.0.surface_area() + right.1 as f32 * right.0.surface_area())
                    / parent_sa;

            if cost < best_cost {
                best_cost = cost;
                best_axis = axis;
                best_split = split_at;
            }
        }
    }

    // Partition according to best axis / bucket split.
    let cab_min = cab.min[best_axis];
    let extent = cab.max[best_axis] - cab.min[best_axis];
    let split = partition(indices, |&i| {
        let c = objects[i].world_aabb().center()[best_axis];
        let b = if extent > f32::EPSILON {
            ((SAH_BUCKETS as f32 * (c - cab_min) / extent) as usize).min(SAH_BUCKETS - 1)
        } else {
            0
        };
        b < best_split
    });

    if split == 0 || split == indices.len() {
        indices.len() / 2
    } else {
        split
    }
}

/// Stable in-place partition: elements satisfying `pred` come first.
/// Returns the number of elements in the left partition.
fn partition<T, F: Fn(&T) -> bool>(slice: &mut [T], pred: F) -> usize {
    let mut left = 0;
    let mut right = slice.len();
    while left < right {
        if pred(&slice[left]) {
            left += 1;
        } else {
            right -= 1;
            slice.swap(left, right);
        }
    }
    left
}

#[cfg(test)]
mod tests {
    use glam::Mat4;

    use super::*;
    use crate::scene::{ClassKey, Object};

    fn make_env() -> (Vec<Object>, Bvh) {
        let mut classes: SlotMap<ClassKey, String> = SlotMap::default();
        let class_key = classes.insert("Test".to_string());
        // Three boxes along the X axis.
        let objs: Vec<Object> = (0..3u32)
            .map(|i| {
                let x = i as f32 * 3.0;
                let mut o = Object::new(vec![], class_key, String::new().into(), None);
                o.transform = Mat4::from_translation(glam::Vec3::new(x, 0.0, 0.0));
                // Override local AABB since there's no mesh.
                o.aabb_local = Aabb::new(
                    glam::Vec3::new(-0.5, -0.5, -0.5),
                    glam::Vec3::new(0.5, 0.5, 0.5),
                );
                o
            })
            .collect();
        let refs: Vec<&Object> = objs.iter().collect();
        let bvh = Bvh::build(&refs, BuildStrategy::Midpoint);
        (objs, bvh)
    }

    #[test]
    fn bvh_ray_query() {
        let (objs, bvh) = make_env();
        let refs: Vec<&Object> = objs.iter().collect();
        let empty_meshes = SlotMap::default();
        // Ray along +X from behind all boxes.
        let ray = Ray::new(glam::Vec3::new(-2.0, 0.0, 0.0), glam::Vec3::X);
        let hits = bvh.query_ray(&ray, &refs, &empty_meshes);
        assert!(!hits.is_empty());
        // Results must be sorted by t.
        for w in hits.windows(2) {
            assert!(w[0].1 <= w[1].1);
        }
        // First hit should be object 0 (closest to the ray origin).
        assert_eq!(hits[0].0, 0);
    }
}
