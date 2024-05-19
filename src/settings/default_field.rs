use bevy::{
    math::{Mat3A, Vec3, Vec3A},
    render::{
        mesh::{Mesh, PrimitiveTopology},
        render_asset::RenderAssetUsages,
    },
};
use byteorder::{LittleEndian, ReadBytesExt};
use include_flate::flate;
use std::io::Cursor;

fn extract_usize(cursor: &mut Cursor<&[u8]>) -> usize {
    cursor
        .read_u32::<LittleEndian>()
        .unwrap_or_else(|e| unreachable!("Problem parsing ***_ids.dat: {e:?}")) as usize
}

fn extract_f32(cursor: &mut Cursor<&[u8]>) -> f32 {
    cursor
        .read_f32::<LittleEndian>()
        .unwrap_or_else(|e| unreachable!("Problem parsing ***_vertices.dat: {e:?}"))
}

/// A collection of inter-connected triangles.
#[derive(Clone, Debug, Default)]
pub struct MeshBuilder {
    ids: Vec<usize>,
    vertices: Vec<Vec3A>,
}

impl MeshBuilder {
    pub fn from_bytes(ids_dat: &[u8], vertices_dat: &[u8]) -> Self {
        let ids = {
            let ids_len = ids_dat.len() / 4;
            let mut ids_cursor = Cursor::new(ids_dat);

            (0..ids_len).map(|_| extract_usize(&mut ids_cursor)).collect()
        };

        let vertices = {
            let vertices_len = vertices_dat.len() / 4;
            let mut vertices_cursor = Cursor::new(vertices_dat);

            (0..vertices_len / 3)
                .map(|_| {
                    Vec3A::new(
                        extract_f32(&mut vertices_cursor),
                        extract_f32(&mut vertices_cursor),
                        extract_f32(&mut vertices_cursor),
                    )
                })
                .collect()
        };

        Self { ids, vertices }
    }

    pub const fn new(ids: Vec<usize>, vertices: Vec<Vec3A>) -> Self {
        Self { ids, vertices }
    }

    pub fn combine<const N: usize>(other_meshes: [Self; N]) -> Self {
        let (n_ids, n_verts) = other_meshes.iter().fold((0, 0), |(n_ids, n_verts), m| {
            (n_ids + m.ids.len(), n_verts + m.vertices.len())
        });
        let mut id_offset = 0;

        let (ids, vertices) = other_meshes.into_iter().fold(
            (Vec::with_capacity(n_ids), Vec::with_capacity(n_verts)),
            |(mut ids, mut vertices), m| {
                ids.extend(m.ids.iter().map(|id| id + id_offset));
                id_offset += m.vertices.len();
                vertices.extend(m.vertices.iter());
                (ids, vertices)
            },
        );

        Self { ids, vertices }
    }

    pub fn transform(mut self, a: Mat3A) -> Self {
        debug_assert_eq!(self.ids.len() % 3, 0);

        for vertex in self.vertices.iter_mut() {
            *vertex = a * *vertex;
        }

        // for transformations that flip things
        // inside-out, change triangle winding
        if a.determinant() < 0. {
            for ids in self.ids.chunks_exact_mut(3) {
                ids.swap(1, 2);
            }
        }

        self
    }

    pub fn translate_y(mut self, p: f32) -> Self {
        for vertex in self.vertices.iter_mut() {
            vertex.y += p;
        }

        self
    }

    pub fn build(self) -> Mesh {
        let positions = self
            .ids
            .iter()
            .map(|&id| {
                let vert = self.vertices[id];
                [vert.x, vert.z, -vert.y]
            })
            .collect::<Vec<_>>();
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.compute_flat_normals();
        mesh
    }
}

macro_rules! include_mesh {
    ($ids:literal, $verts:literal) => {
        {
            flate!(static IDS: [u8] from $ids);
            flate!(static VERTS: [u8] from $verts);
            MeshBuilder::from_bytes(&IDS, &VERTS)
        }
    };
}

#[must_use]
pub fn load_standard() -> Mesh {
    let standard_corner = include_mesh!(
        "default_assets/standard/standard_corner_ids.bin",
        "default_assets/standard/standard_corner_vertices.bin"
    );
    let standard_goal = include_mesh!(
        "default_assets/standard/standard_goal_ids.bin",
        "default_assets/standard/standard_goal_vertices.bin"
    );
    let standard_ramps_0 = include_mesh!(
        "default_assets/standard/standard_ramps_0_ids.bin",
        "default_assets/standard/standard_ramps_0_vertices.bin"
    );
    let standard_ramps_1 = include_mesh!(
        "default_assets/standard/standard_ramps_1_ids.bin",
        "default_assets/standard/standard_ramps_1_vertices.bin"
    );

    initialize_standard(standard_corner, standard_goal, standard_ramps_0, standard_ramps_1)
}

#[must_use]
pub fn load_hoops() -> Mesh {
    let hoops_corner = include_mesh!(
        "default_assets/hoops/hoops_corner_ids.bin",
        "default_assets/hoops/hoops_corner_vertices.bin"
    );
    let hoops_net = include_mesh!(
        "default_assets/hoops/hoops_net_ids.bin",
        "default_assets/hoops/hoops_net_vertices.bin"
    );
    let hoops_rim = include_mesh!(
        "default_assets/hoops/hoops_rim_ids.bin",
        "default_assets/hoops/hoops_rim_vertices.bin"
    );
    let hoops_ramps_0 = include_mesh!(
        "default_assets/hoops/hoops_ramps_0_ids.bin",
        "default_assets/hoops/hoops_ramps_0_vertices.bin"
    );
    let hoops_ramps_1 = include_mesh!(
        "default_assets/hoops/hoops_ramps_1_ids.bin",
        "default_assets/hoops/hoops_ramps_1_vertices.bin"
    );

    initialize_hoops(hoops_corner, hoops_net, hoops_rim, hoops_ramps_0, hoops_ramps_1)
}

const FLIP_X: Mat3A = Mat3A::from_cols(Vec3A::NEG_X, Vec3A::Y, Vec3A::Z);
const FLIP_Y: Mat3A = Mat3A::from_cols(Vec3A::X, Vec3A::NEG_Y, Vec3A::Z);

fn quad(p: Vec3A, e1: Vec3A, e2: Vec3A) -> MeshBuilder {
    MeshBuilder::new(
        vec![0, 1, 3, 1, 2, 3],
        vec![p + e1 + e2, p - e1 + e2, p - e1 - e2, p + e1 - e2],
    )
}

pub fn get_standard_floor() -> Mesh {
    quad(Vec3A::ZERO, Vec3A::new(4096., 0., 0.), Vec3A::new(0., 5500., 0.)).build()
}

pub fn initialize_standard(
    standard_corner: MeshBuilder,
    standard_goal: MeshBuilder,
    standard_ramps_0: MeshBuilder,
    standard_ramps_1: MeshBuilder,
) -> Mesh {
    const Y_OFFSET: f32 = -5120.;

    let standard_goal_tf = standard_goal.translate_y(Y_OFFSET);

    let field_mesh = MeshBuilder::combine([
        standard_corner.clone().transform(FLIP_X),
        standard_corner.clone().transform(FLIP_Y),
        standard_corner.clone().transform(FLIP_X * FLIP_Y),
        standard_corner,
        standard_goal_tf.clone().transform(FLIP_X),
        standard_goal_tf.clone().transform(FLIP_Y),
        standard_goal_tf.clone().transform(FLIP_X * FLIP_Y),
        standard_goal_tf,
        standard_ramps_0.clone().transform(FLIP_X),
        standard_ramps_0.clone().transform(FLIP_Y),
        standard_ramps_0.clone().transform(FLIP_X * FLIP_Y),
        standard_ramps_0,
        standard_ramps_1.clone().transform(FLIP_X),
        standard_ramps_1.clone().transform(FLIP_Y),
        standard_ramps_1.clone().transform(FLIP_X * FLIP_Y),
        standard_ramps_1,
    ]);

    field_mesh.build()
}

pub fn get_hoops_floor() -> Mesh {
    quad(Vec3A::ZERO, Vec3A::new(2966., 0., 0.), Vec3A::new(0., 3581., 0.)).build()
}

pub fn initialize_hoops(
    hoops_corner: MeshBuilder,
    hoops_net: MeshBuilder,
    hoops_rim: MeshBuilder,
    hoops_ramps_0: MeshBuilder,
    hoops_ramps_1: MeshBuilder,
) -> Mesh {
    const SCALE: f32 = 0.9;
    const S: Mat3A = Mat3A::from_diagonal(Vec3::splat(SCALE));

    const Y_OFFSET: f32 = 431.664;

    let hoops_net_tf = hoops_net.transform(S).translate_y(Y_OFFSET);
    let hoops_rim_tf = hoops_rim.transform(S).translate_y(Y_OFFSET);

    let field_mesh = MeshBuilder::combine([
        hoops_corner.clone().transform(FLIP_X),
        hoops_corner.clone().transform(FLIP_Y),
        hoops_corner.clone().transform(FLIP_X * FLIP_Y),
        hoops_corner,
        hoops_net_tf.clone().transform(FLIP_Y),
        hoops_net_tf,
        hoops_rim_tf.clone().transform(FLIP_Y),
        hoops_rim_tf,
        hoops_ramps_0.clone().transform(FLIP_X),
        hoops_ramps_0,
        hoops_ramps_1.clone().transform(FLIP_Y),
        hoops_ramps_1,
    ]);

    field_mesh.build()
}
