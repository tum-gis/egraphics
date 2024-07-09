use crate::Error;
use crate::Error::NoElements;
use std::mem;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Vector3D {
    x: f32,
    y: f32,
    z: f32,
}

impl Vector3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(C)]
pub struct Vertex {
    pub(crate) position: nalgebra::Point3<f32>,
    pub(crate) color: nalgebra::Point3<f32>,
}

impl Vertex {
    pub fn new(position: nalgebra::Point3<f32>, color: nalgebra::Point3<f32>) -> Self {
        Self { position, color }
    }

    pub fn position(&self) -> &nalgebra::Point3<f32> {
        &self.position
    }

    pub fn color(&self) -> &nalgebra::Point3<f32> {
        &self.color
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Triangle {
    vertices: Vec<Vertex>,
}

impl Triangle {
    pub fn new(vertices: Vec<Vertex>) -> Result<Self, Error> {
        if vertices.is_empty() {
            return Err(NoElements);
        }

        Ok(Self { vertices })
    }

    pub(crate) fn buffer_length(&self) -> u32 {
        (self.vertices.len() * mem::size_of::<Vertex>()) as u32
    }

    pub(crate) fn len(&self) -> usize {
        self.vertices.len()
    }

    pub(crate) fn get_vertices(&self) -> &Vec<Vertex> {
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
