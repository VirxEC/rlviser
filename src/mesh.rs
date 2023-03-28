use bevy::{
    prelude::*,
    render::mesh::{self, PrimitiveTopology},
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::{
    f32::consts::PI,
    fs::{read_dir, File},
    io,
    path::Path,
};

pub struct FieldLoaderPlugin;

impl Plugin for FieldLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(load_field);
    }
}

fn load_field(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    // Get all files in ./collision_meshes/soccar/*.cmf
    let mesh = MeshBuilder::combine(
        &read_dir("./collision_meshes/soccar")
            .unwrap()
            .flatten()
            .flat_map(|entry| MeshBuilder::from_file(entry.path()))
            .collect::<Vec<MeshBuilder>>(),
    )
    .build_mesh(0.5);

    let wall_material = StandardMaterial {
        base_color: Color::rgba(0.2, 0.2, 0.2, 0.98),
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        ..default()
    };

    commands.spawn(PbrBundle {
        mesh: meshes.add(mesh),
        material: materials.add(wall_material),
        transform: Transform::default().looking_to(-Vec3::Y, Vec3::Z),
        ..default()
    });

    let floor_material = StandardMaterial {
        base_color: Color::rgb(0.7, 1., 0.7),
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        ..default()
    };

    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane::from_size(100.))),
        material: materials.add(Color::rgb(0.7, 1., 0.7).into()),
        ..default()
    });

    let ceiling_material = StandardMaterial {
        base_color: Color::rgba(0.2, 0.2, 0.2, 0.9),
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        ..default()
    };

    let mut ceiling_transform = Transform::from_xyz(0., 20., 0.);
    ceiling_transform.rotate_local_x(PI);
    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane::from_size(100.))),
        material: materials.add(ceiling_material),
        transform: ceiling_transform,
        ..default()
    });
}

/// A collection of inter-connected triangles.
#[derive(Clone, Debug, Default)]
struct MeshBuilder {
    ids: Vec<u32>,
    verts: Vec<f32>,
}

impl MeshBuilder {
    pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = File::open(path)?;

        let ids_len = file.read_i32::<LittleEndian>()? * 3;
        let verts_len = file.read_i32::<LittleEndian>()? * 3;

        let ids = (0..ids_len).map(|_| file.read_i32::<LittleEndian>().map(|x| x as u32)).collect::<io::Result<Vec<_>>>()?;
        let verts = (0..verts_len - verts_len % 3).map(|_| file.read_f32::<LittleEndian>()).collect::<io::Result<Vec<_>>>()?;

        // Verify that the triangle data is correct
        let max_vert = verts.len() as u32 / 3;
        for &id in &ids {
            if id >= max_vert {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid triangle data"));
            }
        }

        Ok(Self { ids, verts })
    }

    #[must_use]
    /// Combine different meshes all into one
    pub fn combine(other_meshes: &[Self]) -> Self {
        let n_ids = other_meshes.iter().map(|mesh| mesh.ids.len()).sum();
        let mut ids: Vec<u32> = Vec::with_capacity(n_ids);
        let mut id_offset = 0;

        for m in other_meshes {
            ids.extend(m.ids.iter().map(|id| id + id_offset));
            id_offset += m.verts.len() as u32 / 3;
        }

        let vertices: Vec<f32> = other_meshes.iter().flat_map(|m| &m.verts).copied().collect();

        Self { ids, verts: vertices }
    }

    #[must_use]
    // Build the Bevy Mesh
    pub fn build_mesh(self, scale: f32) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
        mesh.set_indices(Some(mesh::Indices::U32(self.ids)));
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            self.verts.chunks(3).map(|chunk| [chunk[0] * scale, chunk[1] * scale, chunk[2] * scale]).collect::<Vec<_>>(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, (0..self.verts.len() / 3).map(|_| [0.0, 1.0, 0.0]).collect::<Vec<_>>());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, (0..self.verts.len() / 3).map(|_| [1.0, 1.0]).collect::<Vec<_>>());
        mesh
    }
}
