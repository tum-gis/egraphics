use nalgebra;
use std::mem;

#[derive(Copy, Clone, Debug)]
pub struct Vector3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct Vertex {
    pub(crate) position: nalgebra::Point3<f32>,
    pub(crate) color: nalgebra::Point3<f32>,
}

pub struct Triangle {
    pub(crate) vertices: Vec<Vertex>,
}

impl Triangle {
    fn buffer_length(&self) -> u32 {
        (self.vertices.len() * mem::size_of::<Vertex>()) as u32
    }

    fn len(&self) -> usize {
        self.vertices.len()
    }

    fn get_vertices(&self) -> &Vec<Vertex> {
        &self.vertices
    }
}

impl From<Vec<nalgebra::Point3<f32>>> for Triangle {
    fn from(item: Vec<nalgebra::Point3<f32>>) -> Self {
        let vertices = item
            .iter()
            .map(|p| Vertex {
                position: *p,
                color: nalgebra::Point3::new(0.5, 0.5, 0.5),
            })
            .collect();
        Triangle { vertices }
    }
}

pub struct TriangleMesh {
    pub triangles: Vec<Triangle>,
}

impl TriangleMesh {
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
            .collect::<Vec<f32>>()
            .iter()
            .fold(f32::INFINITY, |a, &b| a.min(b));
        let min_y = vertices
            .iter()
            .map(|p| p.y)
            .collect::<Vec<f32>>()
            .iter()
            .fold(f32::INFINITY, |a, &b| a.min(b));
        let min_z = vertices
            .iter()
            .map(|p| p.z)
            .collect::<Vec<f32>>()
            .iter()
            .fold(f32::INFINITY, |a, &b| a.min(b));

        nalgebra::Point3::new(min_x, min_y, min_z)
    }

    pub fn get_max(&self) -> nalgebra::Point3<f32> {
        let vertices: Vec<nalgebra::Point3<f32>> =
            self.all_vertices().iter().map(|v| v.position).collect();

        let max_x = vertices
            .iter()
            .map(|p| p.x)
            .collect::<Vec<f32>>()
            .iter()
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let max_y = vertices
            .iter()
            .map(|p| p.y)
            .collect::<Vec<f32>>()
            .iter()
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let max_z = vertices
            .iter()
            .map(|p| p.z)
            .collect::<Vec<f32>>()
            .iter()
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b));

        nalgebra::Point3::new(max_x, max_y, max_z)
    }
}
