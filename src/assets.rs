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
use walkdir::WalkDir;

use bevy::{
    asset::{AssetLoader, LoadedAsset},
    prelude::*,
};
use bevy_asset_loader::prelude::*;

use crate::mesh::MeshBuilder;

#[derive(AssetCollection, Resource)]
pub struct BallAssets {
    #[asset(path = "Ball_Default_Textures/Texture2D/Ball_Default00_D.tga")]
    pub ball_diffuse: Handle<Image>,
    #[asset(path = "Ball_Default_Textures/Texture2D/Ball_Default00_N.tga")]
    pub ball_normal: Handle<Image>,
    #[asset(path = "Ball_Default_Textures/Texture2D/Ball_Default00_RGB.tga")]
    pub ball_occlude: Handle<Image>,
    #[asset(path = "Ball_Default/StaticMesh3/Ball_DefaultBall00.pskx")]
    pub ball: Handle<Mesh>,
}

const BLOCK_MESHES: [&str; 4] = ["SkySphere01", "Glow", "Fog", "FX_General"];

pub fn get_mesh_info(name: &str, asset_server: &AssetServer) -> Option<Handle<Mesh>> {
    let mut path = name
        .replace(".Modular", "")
        .replace(".Meshes", ".StaticMesh3")
        .replace(".SM", ".StaticMesh3")
        .replace(".Materials", ".StaticMesh3")
        .replace("Park_Assets.Park_", "Park_Assets.StaticMesh3.Park_")
        .replace("Pickup_Boost.BoostPad", "Pickup_Boost.StaticMesh3.BoostPad")
        .replace("Grass.Grass", "Grass.StaticMesh3.Grass")
        .replace('.', "/");
    path.push_str(".pskx");

    // check if any item in BLOCK_MESHES is in path
    if BLOCK_MESHES.iter().any(|x| path.contains(x)) {
        return None;
    }

    Some(asset_server.load(path))
}

fn load_texture(name: &str, asset_server: &AssetServer) -> Handle<Image> {
    let path = WalkDir::new("assets")
        .into_iter()
        .flatten()
        .find(|x| x.file_name().to_string_lossy() == format!("{}.tga", name))
        .unwrap()
        .path()
        .to_string_lossy()
        .to_string()
        .replace("assets/", "");

    asset_server.load(path)
}

const DOUBLE_SIDED_MATS: [&str; 11] = [
    "FutureTech.Materials.ForceField_HexGage_MIC",
    "FutureTech.Materials.HexGlass_WithArrows_Team2_MIC",
    "FutureTech.Materials.HexGlass_WithArrows_Team1_MIC",
    "FX_Lighting.Materials.LightCone_Simple_MIC",
    "Stadium.Materials.StadiumLight_Flare_Mat",
    "FutureTech.Materials.Frame_01_V2_Mat",
    "FutureTech.Materials.Reflective_Floor_V2_Mat",
    "FutureTech.Materials.Frame_01_MIC",
    "Stadium.Materials.SeatBase_Mat",
    "Stadium.Materials.Crowd_ST_Team1_Mic",
    "Stadium.Materials.Crowd_ST_Team2_Mic",
];

const TRANSPARENT_MATS: [&str; 11] = [
    "FutureTech.Materials.ForceField_HexGage_MIC",
    "FutureTech.Materials.HexGlass_WithArrows_Team2_MIC",
    "FutureTech.Materials.HexGlass_WithArrows_Team1_MIC",
    "FX_Lighting.Materials.LightCone_Simple_MIC",
    "Stadium.Materials.StadiumLight_Flare_Mat",
    "FX_General.Mat.FogSheet_Mat",
    "Stadium_Assets.Materials.StadiumFog_Team1_MIC",
    "Stadium_Assets.Materials.StadiumFog_Team2_MIC",
    "FX_General.Mat.FogSheet_Team1_MIC",
    "FX_General.Mat.FogSheet_Team2_MIC",
    "FX_General.Mat.FogCylinder_Mat",
];

fn retreive_material(name: &str, asset_server: &AssetServer) -> Option<StandardMaterial> {
    println!("Retreiving material {name}");
    let material_folder = if name.ends_with("MIC") || name.ends_with("Mic") {
        ".MaterialInstanceConstant"
    } else {
        ".Material3"
    };
    let pre_path = name.replace(".Materials", material_folder).replace('.', "/");

    let path = format!("assets/{pre_path}.mat");
    let Ok(mat_file) = fs::read_to_string(&path) else {
        println!("Failed to read {path} ({name})");
        return None;
    };

    let props: String = format!("assets/{pre_path}.props.txt");
    let Ok(props_file) = fs::read_to_string(&props) else {
        println!("Failed to read {path} ({name})");
        return None;
    };

    let mut diffuse = None;
    let mut normal = None;
    let mut other = Vec::new();

    for line in mat_file.lines() {
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
                "Other[0]" => {
                    other.push(value);
                }
                x => {
                    println!("Unknown key {x} is {value} in {path} ({name})");
                }
            }
        }
    }

    let mut material = StandardMaterial {
        base_color: Color::rgb(0.3, 0.3, 0.3),
        metallic: 0.1,
        ..default()
    };

    let mut alpha_mode = None;
    let mut mask_clip_value = 0.333;
    let mut double_sided = None;

    for line in props_file.lines() {
        let mut split = line.split(" = ");
        if let Some(key) = split.next() {
            let Some(value) = split.next() else {
                continue;
            };

            if key == "TwoSided" {
                double_sided = Some(value == "true");
            } else if key == "BlendMode" {
                alpha_mode = match value {
                    "BLEND_Opaque (0)" => Some(AlphaMode::Opaque),
                    "BLEND_Masked (1)" => Some(AlphaMode::Mask(mask_clip_value)),
                    "BLEND_Translucent (2)" => Some(AlphaMode::Blend),
                    "BLEND_Additive (3)" => Some(AlphaMode::Add),
                    _ => {
                        println!("Unknown blend mode {value} in {path} ({name})");
                        None
                    }
                };
            } else if key == "OpacityMaskClipValue" {
                if let Ok(mask_value) = value.parse() {
                    mask_clip_value = mask_value;

                    if let Some(AlphaMode::Mask(_)) = alpha_mode {
                        alpha_mode = Some(AlphaMode::Mask(mask_clip_value));
                    }
                }
            }
        }
    }

    if let Some(alpha_mode) = alpha_mode {
        material.alpha_mode = alpha_mode;
    } else if TRANSPARENT_MATS.contains(&name) {
        material.alpha_mode = AlphaMode::Add;
    }

    if double_sided.unwrap_or_default() || DOUBLE_SIDED_MATS.contains(&name) {
        material.cull_mode = None;
        material.double_sided = true;
    }

    if let Some(texture_name) = diffuse {
        println!("Found texture for {name}");
        if texture_name == "ForcefieldHex" {
            material.base_color = Color::rgba(0.3, 0.3, 0.3, 0.3);
        }
        material.base_color_texture = Some(load_texture(texture_name, asset_server));
    }

    for texture_name in other {
        // idealy, the textures would be combined
        if diffuse.is_none() {
            material.base_color_texture = Some(load_texture(texture_name, asset_server));
        }
    }

    if let Some(texture_name) = normal {
        material.normal_map_texture = Some(load_texture(texture_name, asset_server));
    }

    Some(material)
}

static MATERIALS: Mutex<Lazy<HashMap<String, Handle<StandardMaterial>>>> = Mutex::new(Lazy::new(HashMap::new));

pub fn get_material(name: &str, materials: &mut Assets<StandardMaterial>, asset_server: &AssetServer) -> Handle<StandardMaterial> {
    let mut material_names = MATERIALS.lock().unwrap();

    if let Some(material) = material_names.get(name) {
        return material.clone();
    }

    material_names
        .entry(name.to_string())
        .or_insert_with(|| {
            materials.add(retreive_material(name, asset_server).unwrap_or(StandardMaterial {
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
