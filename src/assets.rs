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

pub fn read_vertices(chunk_data: &[u8], data_count: usize) -> Vec<f32> {
    let mut vertices = Vec::with_capacity(data_count);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        vertices.push(reader.read_f32::<LittleEndian>().unwrap());
        let y = reader.read_f32::<LittleEndian>().unwrap();
        vertices.push(reader.read_f32::<LittleEndian>().unwrap());
        vertices.push(-y);
    }

    vertices
}

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

pub fn read_faces(chunk_data: &[u8], data_count: usize, wedges: &[Wedge]) -> Vec<[(u32, Vec2); 3]> {
    let mut faces = Vec::with_capacity(data_count * 3);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        let wdg_idx_1 = reader.read_u16::<LittleEndian>().unwrap() as usize;
        let wdg_idx_2 = reader.read_u16::<LittleEndian>().unwrap() as usize;
        let wdg_idx_3 = reader.read_u16::<LittleEndian>().unwrap() as usize;
        let _mat_index = reader.read_u8().unwrap();
        let _aux_mat_index = reader.read_u8().unwrap();
        let _smoothing_group = reader.read_u32::<LittleEndian>().unwrap();

        let verts = [wedges[wdg_idx_1], wedges[wdg_idx_2], wedges[wdg_idx_3]];

        faces.push([(verts[1].vertex_id, verts[1].uv), (verts[0].vertex_id, verts[0].uv), (verts[2].vertex_id, verts[2].uv)]);
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
