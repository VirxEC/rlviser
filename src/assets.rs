use byteorder::{LittleEndian, ReadBytesExt};
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    fs,
    io::{self, Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::Mutex,
};

use bevy::{
    asset::{Asset, AssetLoader, LoadedAsset},
    prelude::*,
};
use bevy_asset_loader::prelude::*;

use crate::mesh::MeshBuilder;

pub trait GetMeshInfoFromName<T: Asset> {
    fn get(&self, name: &str) -> Option<(&Handle<T>, bool)>;
}

pub trait GetTextureFromName {
    fn get(&self, name: &str) -> Option<&Handle<Image>>;
}

#[derive(AssetCollection, Resource)]
pub struct BallAssets {
    #[asset(path = "Ball_Default_Textures/Texture2D/Ball_Default00_D.dds")]
    pub ball_diffuse: Handle<Image>,
    #[asset(path = "Ball_Default_Textures/Texture2D/Ball_Default00_N.dds")]
    pub ball_normal: Handle<Image>,
    #[asset(path = "Ball_Default_Textures/Texture2D/Ball_Default00_RGB.dds")]
    pub ball_occlude: Handle<Image>,
    #[asset(path = "Ball_Default/StaticMesh3/Ball_DefaultBall00.pskx")]
    pub ball: Handle<Mesh>,
}

#[derive(AssetCollection, Resource)]
pub struct TiledPatterns {
    #[asset(path = "TiledPatterns/Texture2D/Hexagons_N.tga")]
    pub hexagons_normal: Handle<Image>,
    #[asset(path = "TiledPatterns/Texture2D/Hexagons_Pack.dds")]
    pub hexagons_pack: Handle<Image>,
    #[asset(path = "TiledPatterns/Texture2D/Hexagons_Pack_B.dds")]
    pub hexagons_pack_b: Handle<Image>,
}

impl GetTextureFromName for TiledPatterns {
    fn get(&self, name: &str) -> Option<&Handle<Image>> {
        match name {
            "Hexagons_N" => Some(&self.hexagons_normal),
            "Hexagons_Pack" | "ForcefieldHex" => Some(&self.hexagons_pack),
            "Hexagons_Pack_B" => Some(&self.hexagons_pack_b),
            _ => None,
        }
    }
}

#[derive(AssetCollection, Resource)]
pub struct Details {
    #[asset(path = "Stadiums_DetailNormals/Texture2D/ENV_BrushedMetal_N.dds")]
    pub brushed_metal_normal: Handle<Image>,
    #[asset(path = "Stadiums_DetailNormals/Texture2D/ENV_CarbonFiber_N.dds")]
    pub carbon_fiber: Handle<Image>,
    #[asset(path = "Vehicle_Parent_Textures/Texture2D/ENVPack.dds")]
    pub env_pack: Handle<Image>,
}

impl GetTextureFromName for Details {
    fn get(&self, name: &str) -> Option<&Handle<Image>> {
        match name {
            "EnvPack" => Some(&self.env_pack),
            _ => None,
        }
    }
}

#[derive(AssetCollection, Resource)]
pub struct ParkStadium {
    #[asset(path = "Park_Assets/StaticMesh3/BoostPads_01_Combined.pskx")]
    pub boost_pads_01_combined: Handle<Mesh>,
    #[asset(path = "Park_Assets/StaticMesh3/BoostPads_02_Combined.pskx")]
    pub boost_pads_02_combined: Handle<Mesh>,
    #[asset(path = "Park_Assets/StaticMesh3/BoostPads_03_Combined.pskx")]
    pub boost_pads_03_combined: Handle<Mesh>,
}

impl GetMeshInfoFromName<Mesh> for ParkStadium {
    fn get(&self, name: &str) -> Option<(&Handle<Mesh>, bool)> {
        const START: &str = "Park_Assets.Meshes.";
        if name.len() < START.len() {
            return None;
        }

        match name.split_at(START.len()).1 {
            "BoostPads_01_Combined" => Some((&self.boost_pads_01_combined, false)),
            "BoostPads_02_Combined" => Some((&self.boost_pads_02_combined, false)),
            "BoostPads_03_Combined" => Some((&self.boost_pads_03_combined, false)),
            _ => None,
        }
    }
}

#[derive(AssetCollection, Resource)]
pub struct FutureStadium {
    // #[asset(path = "FutureTech_Textures/Texture2D/ForcefieldHex.dds")]
    // pub forecefield_hex: Handle<Image>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/OOBFloor.pskx")]
    pub oob_floor: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/OOBFloor_Trim.pskx")]
    pub oob_floor_trim: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Field_STD_Floor_Hex.pskx")]
    pub field_std_floor_hex: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/FFCage_Full.pskx")]
    pub ff_cage_full: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Field_STD_Frame.pskx")]
    pub field_std_frame: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/FF_Goal.pskx")]
    pub ff_goal: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/FF_Roof.pskx")]
    pub ff_roof: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/FF_Side.pskx")]
    pub ff_side: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Goal_STD_Floor.pskx")]
    pub goal_std_floor: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Field_STD_Trim.pskx")]
    pub field_std_trim: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Field_STD_TrimB.pskx")]
    pub field_std_trim_b: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Field_Center.pskx")]
    pub field_center: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Field_Center_Lines.pskx")]
    pub field_center_lines: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Field_Center_Trim.pskx")]
    pub field_center_trim: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Field_CenterField_Team1.pskx")]
    pub field_center_field_team1: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Field_CenterField_Team2.pskx")]
    pub field_center_field_team2: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Field_CenterVent.pskx")]
    pub field_center_vent: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Goal_Lines.pskx")]
    pub goal_lines: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Goal_STD_Glass.pskx")]
    pub goal_std_glass: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/FieldFrame_Outer.pskx")]
    pub field_frame_outer: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Goal_STD_Trim.pskx")]
    pub goal_std_trim: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Goal_STD_Frame.pskx")]
    pub goal_std_frame: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Goal_STD_Quarterpipe.pskx")]
    pub goal_std_quarterpipe: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Side_Trim.pskx")]
    pub side_trim: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Field_STD_Floor_Team1.pskx")]
    pub field_std_floor_team1: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Field_STD_Floor_Team2.pskx")]
    pub field_std_floor_team2: Handle<Mesh>,
    #[asset(path = "FutureStadium_Assets/StaticMesh3/Field_Side_Lines.pskx")]
    pub field_side_lines: Handle<Mesh>,
}

impl GetTextureFromName for FutureStadium {
    fn get(&self, name: &str) -> Option<&Handle<Image>> {
        // match name {
        //     "ForcefieldHex" => Some(&self.forecefield_hex),
        //     _ => None,
        // }
        None
    }
}

impl GetMeshInfoFromName<Mesh> for FutureStadium {
    fn get(&self, name: &str) -> Option<(&Handle<Mesh>, bool)> {
        const START: &str = "FutureStadium_Assets.Meshes.Modular.";
        if name.len() < START.len() {
            return None;
        }

        match name.split_at(START.len()).1 {
            "OOBFloor" => Some((&self.oob_floor, false)),
            "OOBFloor_Trim" => Some((&self.oob_floor_trim, false)),
            "Field_STD_Floor_Hex" => Some((&self.field_std_floor_hex, false)),
            "FFCage_Full" => Some((&self.ff_cage_full, true)),
            "Field_STD_Frame" => Some((&self.field_std_frame, true)),
            "FF_Goal" => Some((&self.ff_goal, true)),
            "FF_Roof" => Some((&self.ff_roof, true)),
            _ => None,
        }
    }
}

pub fn get_mesh_info<'a>(name: &str, query: &[&'a dyn GetMeshInfoFromName<Mesh>]) -> Option<(&'a Handle<Mesh>, bool)> {
    query.iter().find_map(|x| x.get(name))
}

static MATERIALS: Mutex<Lazy<HashMap<String, Handle<StandardMaterial>>>> = Mutex::new(Lazy::new(HashMap::new));

const DOUBLE_SIDED_MATS: [&str; 2] = ["FutureTech.Materials.ForceField_Mat", "FutureTech.Materials.ForceField_HexGage_MIC"];
const TRANSPARENT_MATS: [&str; 2] = ["FutureTech.Materials.ForceField_Mat", "FutureTech.Materials.ForceField_HexGage_MIC"];

fn retreive_material(name: &str, query: &[&dyn GetTextureFromName]) -> Option<StandardMaterial> {
    // replace "." with "/" in the name and append "assets/" to the start
    dbg!(name);

    let material_folder = if name.ends_with("MIC") {
        "MaterialInstanceConstant"
    } else {
        "Material3"
    };

    let path = format!("assets/{}.mat", name.replace("Materials", material_folder).replace('.', "/"));
    let mat_file = fs::read_to_string(&path).ok()?;

    let mut diffuse = None;
    let mut normal = None;

    for line in mat_file.lines() {
        // dbg!(&line);
        // split at the first "="
        let mut split = line.split('=');
        if let Some(key) = split.next() {
            let Some(value) = split.next() else {
                println!("No value for {key} in {path}");
                continue;
            };

            match key {
                "Diffuse" => {
                    diffuse = Some(value);
                }
                "Normal" => {
                    normal = Some(value);
                }
                _ => {}
            }
        }
    }

    dbg!(diffuse);
    dbg!(normal);

    let mut material = StandardMaterial {
        base_color: Color::rgb(0.3, 0.3, 0.3),
        metallic: 0.1,
        ..default()
    };

    if TRANSPARENT_MATS.contains(&name) {
        material.alpha_mode = AlphaMode::Blend;
    }

    if DOUBLE_SIDED_MATS.contains(&name) {
        material.cull_mode = None;
        material.double_sided = true;
    }

    if let Some(texture_name) = diffuse {
        if let Some(texture) = query.iter().find_map(|x| x.get(texture_name)) {
            println!("Found texture for {name}");
            material.base_color_texture = Some(texture.clone());
        }
    }

    if let Some(texture_name) = normal {
        if let Some(texture) = query.iter().find_map(|x| x.get(texture_name)) {
            material.normal_map_texture = Some(texture.clone());
        }
    }

    Some(material)
}

pub fn get_material(name: &str, materials: &mut ResMut<Assets<StandardMaterial>>, query: &[&dyn GetTextureFromName]) -> Handle<StandardMaterial> {
    let mut material_names = MATERIALS.lock().unwrap();

    if let Some(material) = material_names.get(name) {
        return material.clone();
    }

    material_names
        .entry(name.to_string())
        .or_insert_with(|| {
            materials.add(retreive_material(name, query).unwrap_or(StandardMaterial {
                base_color: Color::rgb(0.3, 0.3, 0.3),
                metallic: 0.1,
                cull_mode: None,
                double_sided: true,
                ..default()
            }))
        })
        .clone()
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
                "-uncook".to_string(),
                "-dds".to_string(),
                "-uc".to_string(),
                file.to_string(),
            ])
            .stdout(Stdio::null())
            .spawn()?;
        child.wait()?;
    }

    println!("Done processing files                                 ");

    Ok(())
}
