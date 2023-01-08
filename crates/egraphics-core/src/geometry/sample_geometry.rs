use crate::geometry::geometry;
use nalgebra;

pub fn generate_sample_triangle_mesh() -> geometry::TriangleMesh {
    let triangle_a_vertices = vec![
        geometry::Vertex {
            position: nalgebra::Point3::new(0.0, 0.5, 0.0),
            color: nalgebra::Point3::new(1.0, 0.0, 0.0),
        },
        geometry::Vertex {
            position: nalgebra::Point3::new(-0.5, -0.5, 0.0),
            color: nalgebra::Point3::new(0.0, 1.0, 0.0),
        },
        geometry::Vertex {
            position: nalgebra::Point3::new(0.5, -0.5, 0.0),
            color: nalgebra::Point3::new(1.0, 1.0, 1.0),
        },
    ];
    let triangle_a = geometry::Triangle {
        vertices: triangle_a_vertices,
    };

    let triangle_b_vertices = vec![
        geometry::Vertex {
            position: nalgebra::Point3::new(3.0, 3.5, 3.0),
            color: nalgebra::Point3::new(1.0, 0.0, 0.0),
        },
        geometry::Vertex {
            position: nalgebra::Point3::new(2.5, 2.5, 3.0),
            color: nalgebra::Point3::new(0.0, 1.0, 0.0),
        },
        geometry::Vertex {
            position: nalgebra::Point3::new(3.5, 2.5, 3.0),
            color: nalgebra::Point3::new(1.0, 1.0, 1.0),
        },
    ];
    let triangle_b = geometry::Triangle {
        vertices: triangle_b_vertices,
    };

    let triangle_list: Vec<geometry::Triangle> = vec![triangle_a, triangle_b];

    geometry::TriangleMesh {
        triangles: triangle_list,
    }
}
