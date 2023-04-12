use byteorder::{LittleEndian, ReadBytesExt};
use glob::glob;
use std::{
    fs,
    io::{self, Write},
    path::Path,
    process::{Command, Stdio},
};

use bevy::{
    asset::{AssetLoader, LoadedAsset},
    prelude::*,
};
use bevy_asset_loader::prelude::*;

use crate::mesh::MeshBuilder;

#[derive(AssetCollection, Resource)]
pub struct ImageAssets {
    #[asset(path = "Ball_Default_Textures/Texture2D/Ball_Default00_D.tga")]
    pub ball: Handle<Image>,
    #[asset(path = "Ball_Default_Textures/Texture2D/Ball_Default00_N.tga")]
    pub ball_normal: Handle<Image>,
    #[asset(path = "Ball_Default_Textures/Texture2D/Ball_Default00_RGB.tga")]
    pub ball_emissive: Handle<Image>,
}

// create PskxAssets
#[derive(AssetCollection, Resource)]
pub struct PskxAssets {
    #[asset(path = "Ball_Default/StaticMesh3/Ball_DefaultBall00.pskx")]
    pub ball: Handle<Mesh>,
}

// # Vertices X | Y | Z
// def read_vertices():

//     if not bImportmesh:
//         return True

//     nonlocal Vertices

//     Vertices = [None] * chunk_datacount

//     unpack_data = Struct('3f').unpack_from

//     if bScaleDown:
//         for counter in range( chunk_datacount ):
//             (vec_x, vec_y, vec_z) = unpack_data(chunk_data, counter * chunk_datasize)
//             Vertices[counter]  = (vec_x*0.01, vec_y*0.01, vec_z*0.01)
//             # equal to gltf
//             # Vertices[counter]  = (vec_x*0.01, vec_z*0.01, -vec_y*0.01)
//     else:
//         for counter in range( chunk_datacount ):
//             Vertices[counter]  =  unpack_data(chunk_data, counter * chunk_datasize)

pub fn read_vertices(chunk_data: &[u8], data_count: usize) -> Vec<f32> {
    let mut vertices = Vec::with_capacity(data_count);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        vertices.push(reader.read_f32::<LittleEndian>().unwrap());
        let y = reader.read_f32::<LittleEndian>().unwrap();
        vertices.push(reader.read_f32::<LittleEndian>().unwrap());
        vertices.push(y);
    }

    vertices
}

// # Wedges (UV)   VertexId |  U |  V | MatIdx
// def read_wedges():

//     if not bImportmesh:
//         return True

//     nonlocal Wedges

//     Wedges = [None] * chunk_datacount

//     unpack_data = Struct('=IffBxxx').unpack_from

//     for counter in range( chunk_datacount ):
//         (vertex_id,
//          u, v,
//          material_index) = unpack_data( chunk_data, counter * chunk_datasize )

//         # print(vertex_id, u, v, material_index)
//         # Wedges[counter] = (vertex_id, u, v, material_index)
//         Wedges[counter] = [vertex_id, u, v, material_index]

#[derive(Clone, Copy, Debug)]
pub struct Wedge {
    pub vertex_id: u32,
    pub uv: Vec2,
    pub material_index: u8,
}

pub fn read_wedges(chunk_data: &[u8], data_count: usize) -> Vec<Wedge> {
    let mut wedges = Vec::with_capacity(data_count);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        // =IffBxxx
        // native endian
        // unsigned int
        // float
        // float
        // unsigned char
        // 3 bytes padding
        let vertex_id = reader.read_u32::<LittleEndian>().unwrap();
        let u = reader.read_f32::<LittleEndian>().unwrap();
        let v = reader.read_f32::<LittleEndian>().unwrap();
        let material_index = reader.read_u8().unwrap();
        wedges.push(Wedge {
            vertex_id,
            uv: Vec2::new(u, v),
            material_index,
        });

        // read padding bytes
        reader.read_u8().unwrap();
        reader.read_u8().unwrap();
        reader.read_u8().unwrap();
    }

    wedges
}

// # Faces WdgIdx1 | WdgIdx2 | WdgIdx3 | MatIdx | AuxMatIdx | SmthGrp
// def read_faces():

//     if not bImportmesh:
//         return True

//     nonlocal Faces, UV_by_face, WedgeIdx_by_faceIdx

//     UV_by_face = [None] * chunk_datacount
//     Faces = [None] * chunk_datacount
//     WedgeIdx_by_faceIdx = [None] * chunk_datacount

//     if len(Wedges) > 65536:
//         unpack_format = '=IIIBBI'
//     else:
//         unpack_format = '=HHHBBI'

//     unpack_data = Struct(unpack_format).unpack_from

//     for counter in range(chunk_datacount):
//         (WdgIdx1, WdgIdx2, WdgIdx3,
//          MatIndex,
//          AuxMatIndex, #unused
//          SmoothingGroup # Umodel is not exporting SmoothingGroups
//          ) = unpack_data(chunk_data, counter * chunk_datasize)

//         # looks ugly
//         # Wedges is (point_index, u, v, MatIdx)
//         ((vertid0, u0, v0, matid0), (vertid1, u1, v1, matid1), (vertid2, u2, v2, matid2)) = Wedges[WdgIdx1], Wedges[WdgIdx2], Wedges[WdgIdx3]

//         # note order: C,B,A
//         # Faces[counter] = (vertid2,  vertid1, vertid0)

//         Faces[counter] = (vertid1,  vertid0, vertid2)
//         # Faces[counter] = (vertid1,  vertid2, vertid0)
//         # Faces[counter] = (vertid0,  vertid1, vertid2)

//         # uv = ( ( u2, 1.0 - v2 ), ( u1, 1.0 - v1 ), ( u0, 1.0 - v0 ) )
//         uv = ( ( u1, 1.0 - v1 ), ( u0, 1.0 - v0 ), ( u2, 1.0 - v2 ) )

//         # Mapping: FaceIndex <=> UV data <=> FaceMatIndex
//         UV_by_face[counter] = (uv, MatIndex, (matid2, matid1, matid0))

//         # We need this for EXTRA UVs
//         WedgeIdx_by_faceIdx[counter] = (WdgIdx3, WdgIdx2, WdgIdx1)

pub fn read_faces(chunk_data: &[u8], data_count: usize, wedges: &[Wedge]) -> Vec<[(u32, Vec2); 3]> {
    let mut faces = Vec::with_capacity(data_count * 3);
    // let mut uvs = Vec::with_capacity(data_count * 3);
    // let mut uv_by_face = Vec::with_capacity(data_count);
    // let mut wedge_idx_by_face_idx = Vec::with_capacity(data_count);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        // HHHBBI
        // native endian
        // u16
        // u16
        // u16
        // u8
        // u8
        // u32
        let wdg_idx_1 = reader.read_u16::<LittleEndian>().unwrap() as usize;
        let wdg_idx_2 = reader.read_u16::<LittleEndian>().unwrap() as usize;
        let wdg_idx_3 = reader.read_u16::<LittleEndian>().unwrap() as usize;
        let _mat_index = reader.read_u8().unwrap();
        let _aux_mat_index = reader.read_u8().unwrap();
        let _smoothing_group = reader.read_u32::<LittleEndian>().unwrap();

        let verts = [wedges[wdg_idx_1], wedges[wdg_idx_2], wedges[wdg_idx_3]];

        faces.push([(verts[1].vertex_id, verts[1].uv), (verts[2].vertex_id, verts[2].uv), (verts[0].vertex_id, verts[0].uv)]);

        // faces.push((verts[1].vertex_id, verts[1].uv));
        // faces.push((verts[2].vertex_id, verts[2].uv));
        // faces.push((verts[0].vertex_id, verts[0].uv));

        // faces.push(verts[1].vertex_id);
        // faces.push(verts[2].vertex_id);
        // faces.push(verts[0].vertex_id);

        // uvs.push(Vec2::new(verts[1].uv.x, verts[1].uv.y));
        // uvs.push(Vec2::new(verts[2].uv.x, verts[2].uv.y));
        // uvs.push(Vec2::new(verts[0].uv.x, verts[0].uv.y));

        // uv_by_face.push((uv, mat_index, (verts[2].material_index, verts[1].material_index, verts[0].material_index)));
        // wedge_idx_by_face_idx.push((wdg_idx_3, wdg_idx_2, wdg_idx_1));
    }

    faces
}

// create new asset loader for pskx files
pub struct PskxLoader;

pub const PSK_FILE_HEADER: &[u8] = b"ACTRHEAD\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

impl AssetLoader for PskxLoader {
    fn load<'a>(&'a self, bytes: &'a [u8], load_context: &'a mut bevy::asset::LoadContext) -> bevy::utils::BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            load_context.set_default_asset(LoadedAsset::new(MeshBuilder::from_pskx(bytes)?.build_mesh(1.)));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["pskx"]
    }
}

const OUT_DIR: &str = "./assets/";

fn get_input_dir() -> Option<String> {
    let Ok(input_file) = fs::read_to_string("assets.path") else {
        println!("Couldn't find 'assets.path' file in your base folder! Create the file with the path to your 'rocketleague/TAGame/CookedPCConsole' folder.");
        return None;
    };

    let Some(assets_dir) = input_file.lines().next() else {
        println!("Your 'assets.path' file is empty! Create the file with the path to your 'rocketleague/TAGame/CookedPCConsole' folder.");
        return None;
    };

    let assets_path = Path::new(assets_dir);
    if assets_path.is_dir() && assets_path.exists() {
        Some(assets_dir.to_string())
    } else {
        println!("Couldn't find the directory specified in your 'assets.path'!");
        None
    }
}

pub fn uncook() -> io::Result<()> {
    if Path::new(OUT_DIR).exists() {
        println!("Found existing assets");
        return Ok(());
    }

    let input_dir = get_input_dir().unwrap();

    println!("Uncooking assets from Rocket League...");

    // use glob to get all "*_P.upk" files in the input directory
    let upk_files = glob(&format!("{}/*_P.upk", input_dir))
        .unwrap()
        .flatten()
        .flat_map(|path| path.file_name().and_then(|name| name.to_str()).map(|name| name.to_string()))
        .collect::<Vec<_>>();

    if upk_files.is_empty() {
        println!("No UPK files found in input directory");
        return Ok(());
    }

    let num_files = upk_files.len();

    for (i, file) in upk_files.into_iter().enumerate() {
        print!("Processing file {}/{} ({})...                       \r", i, num_files, file);
        io::stdout().flush()?;

        // call umodel to uncook all the map files
        let mut child = Command::new(if cfg!(windows) { "umodel.exe" } else { "./umodel" })
            .args([
                format!("-path={}", input_dir),
                format!("-out={}", OUT_DIR),
                "-game=rocketleague".to_string(),
                "-export".to_string(),
                "-uncook".to_string(),
                file,
            ])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()?;
        child.wait()?;
    }

    println!("Done processing files                                 ");

    Ok(())
}
