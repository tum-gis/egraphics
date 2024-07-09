use crate::geometry::triangle;
use nalgebra;

pub fn generate_sample_triangle_mesh() -> triangle::TriangleMesh {
    let triangle_a_vertices = vec![
        triangle::Vertex {
            position: nalgebra::Point3::new(0.0, 0.5, 0.0),
            color: nalgebra::Point3::new(1.0, 0.0, 0.0),
        },
        triangle::Vertex {
            position: nalgebra::Point3::new(-0.5, -0.5, 0.0),
            color: nalgebra::Point3::new(0.0, 1.0, 0.0),
        },
        triangle::Vertex {
            position: nalgebra::Point3::new(0.5, -0.5, 0.0),
            color: nalgebra::Point3::new(1.0, 1.0, 1.0),
        },
    ];
    let triangle_a = triangle::Triangle {
        vertices: triangle_a_vertices,
    };

    let triangle_b_vertices = vec![
        triangle::Vertex {
            position: nalgebra::Point3::new(3.0, 3.5, 3.0),
            color: nalgebra::Point3::new(1.0, 0.0, 0.0),
        },
        triangle::Vertex {
            position: nalgebra::Point3::new(2.5, 2.5, 3.0),
            color: nalgebra::Point3::new(0.0, 1.0, 0.0),
        },
        triangle::Vertex {
            position: nalgebra::Point3::new(3.5, 2.5, 3.0),
            color: nalgebra::Point3::new(1.0, 1.0, 1.0),
        },
    ];
    let triangle_b = triangle::Triangle {
        vertices: triangle_b_vertices,
    };

    let triangle_list: Vec<triangle::Triangle> = vec![triangle_a, triangle_b];

    triangle::TriangleMesh {
        triangles: triangle_list,
    }
}
