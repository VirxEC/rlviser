use byteorder::{LittleEndian, ReadBytesExt};
use std::{
    fs,
    io::{self, Read, Write},
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
pub struct CoreAssets {
    #[asset(path = "MENU_Main_p/Texture2D/Ball_Default00_D.tga")]
    pub ball_diffuse: Handle<Image>,
    #[asset(path = "MENU_Main_p/Texture2D/Ball_Default00_N.tga")]
    pub ball_normal: Handle<Image>,
    #[asset(path = "MENU_Main_p/Texture2D/Ball_Default00_RGB.tga")]
    pub ball_emissive: Handle<Image>,
    #[asset(path = "MENU_Main_p/StaticMesh3/Ball_DefaultBall00.pskx")]
    pub ball: Handle<Mesh>,
    #[asset(path = "Startup/Texture2D/Hexagons_Pack.tga")]
    pub hexagons_pack: Handle<Image>,
}

#[derive(AssetCollection, Resource)]
pub struct DfhStadium {
    #[asset(path = "Stadium_P/Texture2D/Hexagons_Pack_B.tga")]
    pub hexagons_pack_b: Handle<Image>,
    #[asset(path = "Stadium_P/Texture2D/BrushedMetal_N.tga")]
    pub brushed_metal_normal: Handle<Image>,
    #[asset(path = "Stadium_P/StaticMesh3/OOBFloor.pskx")]
    pub oob_floor: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/OOBFloor_Trim.pskx")]
    pub oob_floor_trim: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/Field_STD_Floor_Hex.pskx")]
    pub field_std_floor_hex: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/FFCage_Full.pskx")]
    pub ff_cage_full: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/Field_STD_Frame.pskx")]
    pub field_std_frame: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/FF_Goal.pskx")]
    pub ff_goal: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/FF_Roof.pskx")]
    pub ff_roof: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/FF_Side.pskx")]
    pub ff_side: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/Goal_STD_Floor.pskx")]
    pub goal_std_floor: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/BoostPads_01_Combined.pskx")]
    pub boost_pads_01_combined: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/BoostPads_02_Combined.pskx")]
    pub boost_pads_02_combined: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/BoostPads_03_Combined.pskx")]
    pub boost_pads_03_combined: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/Field_STD_Trim.pskx")]
    pub field_std_trim: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/Field_STD_TrimB.pskx")]
    pub field_std_trim_b: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/Field_Center.pskx")]
    pub field_center: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/Field_Center_Lines.pskx")]
    pub field_center_lines: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/Field_Center_Trim.pskx")]
    pub field_center_trim: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/Field_CenterField_Team1.pskx")]
    pub field_center_field_team1: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/Field_CenterField_Team2.pskx")]
    pub field_center_field_team2: Handle<Mesh>,
    #[asset(path = "Stadium_P/StaticMesh3/Field_CenterVent.pskx")]
    pub field_center_vent: Handle<Mesh>,
    // Goal_Lines.mo
    #[asset(path = "Stadium_P/StaticMesh3/Goal_Lines.pskx")]
    pub goal_lines: Handle<Mesh>,
    // Goal_STD_Glass.mo
    #[asset(path = "Stadium_P/StaticMesh3/Goal_STD_Glass.pskx")]
    pub goal_std_glass: Handle<Mesh>,
    // FieldFrame_Outer.mo
    #[asset(path = "Stadium_P/StaticMesh3/FieldFrame_Outer.pskx")]
    pub field_frame_outer: Handle<Mesh>,
    // Goal_STD_Trim.mo
    #[asset(path = "Stadium_P/StaticMesh3/Goal_STD_Trim.pskx")]
    pub goal_std_trim: Handle<Mesh>,
    // Goal_STD_Frame.mo
    #[asset(path = "Stadium_P/StaticMesh3/Goal_STD_Frame.pskx")]
    pub goal_std_frame: Handle<Mesh>,
    // Goal_STD_Quarterpipe.mo
    #[asset(path = "Stadium_P/StaticMesh3/Goal_STD_Quarterpipe.pskx")]
    pub goal_std_quarterpipe: Handle<Mesh>,
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
    pub uv: [f32; 2],
    pub material_index: usize,
}

pub fn read_wedges(chunk_data: &[u8], data_count: usize) -> Vec<Wedge> {
    let mut wedges = Vec::with_capacity(data_count);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        let vertex_id = reader.read_u32::<LittleEndian>().unwrap();
        let u = reader.read_f32::<LittleEndian>().unwrap();
        let v = reader.read_f32::<LittleEndian>().unwrap();
        let material_index = reader.read_u8().unwrap() as usize;
        wedges.push(Wedge {
            vertex_id,
            uv: [u, v],
            material_index,
        });

        // read padding bytes
        reader.read_u8().unwrap();
        reader.read_u8().unwrap();
        reader.read_u8().unwrap();
    }

    wedges
}

pub fn read_faces(chunk_data: &[u8], data_count: usize, wedges: &[Wedge]) -> Vec<[(u32, [f32; 2], usize); 3]> {
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

        faces.push([
            (verts[1].vertex_id, verts[1].uv, verts[1].material_index),
            (verts[0].vertex_id, verts[0].uv, verts[0].material_index),
            (verts[2].vertex_id, verts[2].uv, verts[2].material_index),
        ]);
    }

    faces
}

pub fn read_vertex_colors(chunk_data: &[u8], data_count: usize) -> Vec<[f32; 4]> {
    let mut vertex_colors = Vec::with_capacity(data_count);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        vertex_colors.push([
            reader.read_u8().unwrap() as f32,
            reader.read_u8().unwrap() as f32,
            reader.read_u8().unwrap() as f32,
            reader.read_u8().unwrap() as f32,
        ]);
    }

    vertex_colors
}

pub fn read_extra_uvs(chunk_data: &[u8], data_count: usize) -> Vec<[f32; 2]> {
    let mut extra_uvs = Vec::with_capacity(data_count);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        extra_uvs.push([reader.read_f32::<LittleEndian>().unwrap(), reader.read_f32::<LittleEndian>().unwrap()]);
    }

    extra_uvs
}

pub fn read_materials(chunk_data: &[u8], data_count: usize) -> Vec<String> {
    let mut materials = Vec::with_capacity(data_count);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        // 64 bytes for material name
        // 24 padding bytes

        let mut material_name = [0; 64];
        reader.read_exact(&mut material_name).unwrap();

        // get index of first null byte
        let null_index = material_name.iter().position(|&x| x == 0).unwrap_or(material_name.len());

        materials.push(String::from_utf8_lossy(&material_name[..null_index]).to_string());

        // read padding bytes
        (0..24).for_each(|_| {
            reader.read_u8().unwrap();
        });
    }

    materials
}

// create new asset loader for pskx files
pub struct PskxLoader;

pub const PSK_FILE_HEADER: &[u8] = b"ACTRHEAD\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

impl AssetLoader for PskxLoader {
    fn load<'a>(&'a self, bytes: &'a [u8], load_context: &'a mut bevy::asset::LoadContext) -> bevy::utils::BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let asset_name = load_context.path().file_name().and_then(|name| name.to_str()).unwrap();
            load_context.set_default_asset(LoadedAsset::new(MeshBuilder::from_pskx(asset_name, bytes)?.build_mesh(1.)));
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

    let upk_files = ["Startup.upk", "MENU_Main_p.upk", "Stadium_P.upk"];
    // let upk_files = std::fs::read_dir(&input_dir)
    //         .unwrap()
    //         .flatten()
    //         .filter_map(|file| {
    //             let file_str = file.file_name().to_str().unwrap().to_string();
    //             if file_str.ends_with(".upk") {
    //                 Some(file_str)
    //             } else {
    //                 None
    //             }
    //         })
    //         .collect::<Vec<_>>();

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
                "-nooverwrite".to_string(),
                "-nolightmap".to_string(),
                file.to_string(),
            ])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()?;
        child.wait()?;

        // call umodel to uncook all the map files
        let mut child = Command::new(if cfg!(windows) { "umodel.exe" } else { "./umodel" })
            .args([
                format!("-path={}", input_dir),
                format!("-out={}", OUT_DIR),
                "-game=rocketleague".to_string(),
                "-export".to_string(),
                "-nooverwrite".to_string(),
                "-nolightmap".to_string(),
                "-gltf".to_string(),
                file.to_string(),
            ])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()?;
        child.wait()?;
    }

    println!("Done processing files                                 ");

    Ok(())
}
