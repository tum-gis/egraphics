use crate::Error::NoElements;
use crate::{Error, Triangle, Vertex};

#[derive(Clone, Debug, PartialEq)]
pub struct TriangleMesh {
    triangles: Vec<Triangle>,
}

impl TriangleMesh {
    pub fn new(triangles: Vec<Triangle>) -> Result<Self, Error> {
        if triangles.is_empty() {
            return Err(NoElements);
        }

        Ok(Self { triangles })
    }

    fn buffer_length(&self) -> u32 {
        self.triangles.iter().map(|x| x.buffer_length()).sum()
    }

    fn number_of_vertices(&self) -> usize {
        self.triangles.iter().map(|x| x.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    pub fn all_vertices(&self) -> Vec<Vertex> {
        //let a = self.triangles.iter().map(|x| x.vertices).;

        let mut all_vertices: Vec<Vertex> = Vec::new();
        //let a = self.triangles.into_iter().map(|x| x.get_vertices()).flatten();
        for triangle in &self.triangles {
            all_vertices.extend(triangle.get_vertices());
        }

        //println!("all vertices");
        all_vertices
    }

    pub fn get_min(&self) -> nalgebra::Point3<f32> {
        let vertices: Vec<nalgebra::Point3<f32>> =
            self.all_vertices().iter().map(|v| v.position).collect();

        let min_x = vertices
            .iter()
            .map(|p| p.x)
            .fold(f32::INFINITY, |a, b| a.min(b));
        let min_y = vertices
            .iter()
            .map(|p| p.y)
            .fold(f32::INFINITY, |a, b| a.min(b));
        let min_z = vertices
            .iter()
            .map(|p| p.z)
            .fold(f32::INFINITY, |a, b| a.min(b));

        nalgebra::Point3::new(min_x, min_y, min_z)
    }

    pub fn get_max(&self) -> nalgebra::Point3<f32> {
        let vertices: Vec<nalgebra::Point3<f32>> =
            self.all_vertices().iter().map(|v| v.position).collect();

        let max_x = vertices
            .iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, |a, b| a.max(b));
        let max_y = vertices
            .iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, |a, b| a.max(b));
        let max_z = vertices
            .iter()
            .map(|p| p.z)
            .fold(f32::NEG_INFINITY, |a, b| a.max(b));

        nalgebra::Point3::new(max_x, max_y, max_z)
    }
}
