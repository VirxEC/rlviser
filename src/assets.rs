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
    // #[asset(path = "Ball_Default_Textures/Texture2D/Ball_Default00_N.tga")]
    // pub ball_normal: Handle<Image>,
    // #[asset(path = "Ball_Default_Textures/Texture2D/Ball_Default00_RGB.tga")]
    // pub ball_occlude: Handle<Image>,
    #[asset(path = "Ball_Default/StaticMesh3/Ball_DefaultBall00.pskx")]
    pub ball: Handle<Mesh>,
}

#[derive(AssetCollection, Resource)]
pub struct BoostPickupGlows {
    #[asset(path = "Pickup_Boost/StaticMesh3/BoostPad_Small_02_SM.pskx")]
    pub small: Handle<Mesh>,
    #[asset(path = "Pickup_Boost/StaticMesh3/BoostPad_Large_Glow.pskx")]
    pub large: Handle<Mesh>,
}

const BLOCK_MESHES: [&str; 7] = [
    "CollisionMeshes",
    "FieldCollision_Standard",
    "Goal_STD_Outer",
    "SkySphere01",
    "Glow",
    "Fog",
    "FX_General",
];

#[cfg(not(feature = "full_load"))]
const EXTRA_BLACKLIST: [&str; 1] = ["Side_Trim"];

#[cfg(not(feature = "full_load"))]
const WHITELIST_MESHES: [&str; 11] = [
    "Field_STD",
    "FF",
    "BoostPads",
    "BoostPad_Large",
    "Goal",
    "AdvertStrip",
    "Field_Center",
    "Field_Mid",
    "Body",
    "Side",
    "Floor",
];

#[cfg(not(feature = "full_load"))]
#[inline]
fn load_mesh(name: &str) -> bool {
    WHITELIST_MESHES.into_iter().any(|x| name.contains(x)) && !EXTRA_BLACKLIST.into_iter().any(|x| name.contains(x))
}

#[cfg(feature = "full_load")]
#[inline]
fn load_mesh(_name: &str) -> bool {
    true
}

pub fn get_mesh_info(name: &str, meshes: &mut Assets<Mesh>) -> Option<Vec<Handle<Mesh>>> {
    // check if any item in BLOCK_MESHES is in the name
    if BLOCK_MESHES.into_iter().any(|x| name.contains(x)) || !load_mesh(name) {
        return None;
    }

    let mut local_path = name
        .replace(".Modular", "")
        .replace(".Meshes", ".StaticMesh3")
        .replace(".SM", ".StaticMesh3")
        .replace(".Materials", ".StaticMesh3")
        // .replace("Park_Assets.Park_", "Park_Assets.StaticMesh3.Park_")
        // .replace("Pickup_Boost.BoostPad", "Pickup_Boost.StaticMesh3.BoostPad")
        // .replace("Grass.Grass", "Grass.StaticMesh3.Grass")
        .replace('.', "/");

    let mut split = local_path.split('/');
    if let Some(first) = split.next() {
        if let Some(second) = split.next() {
            if split.next().is_none() {
                local_path = format!("{first}/StaticMesh3/{second}");
            }
        }
    }

    let extension = if name.contains(".SkeletalMesh3") { "psk" } else { "pskx" };
    let path = format!("./assets/{local_path}.{extension}");

    // read bytes from path
    let Ok(mut file) = fs::File::open(&path) else {
        error!("Failed to open mesh {path} for {name}");
        return None;
    };

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;

    let builder = MeshBuilder::from_pskx(name, &bytes).ok()?;
    Some(builder.build_meshes(1.).into_iter().map(|mesh| meshes.add(mesh)).collect())
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

const DOUBLE_SIDED_MATS: [&str; 20] = [
    "Trees.Materials.LombardyPoplar_B_NoWind_MIC",
    "Trees.Materials.LombardyPoplar_B_Mat",
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
    "Stadium.Materials.Stairs_Mat",
    "City.Materials.MIC_Sidewalk00",
    "City.Materials.Concrete_MAT",
    "Grass.Materials.Grass_Base_Mat",
    "Stadium.Materials.HandRail_MIC",
    "Stadium_Assets.Materials.GroomedGrass_FakeLight_Team1_MIC",
    "Stadium_Assets.Materials.GroomedGrass_FakeLight_Team2_MIC",
];

const TRANSPARENT_MATS: [&str; 2] = [
    "Trees.Materials.LombardyPoplar_B_NoWind_MIC",
    "Trees.Materials.LombardyPoplar_B_Mat",
];

const ADD_MATS: [&str; 13] = [
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
    "FutureTech.Materials.Glass_Projected_V2_Mat",
    "FutureTech.Materials.Glass_Projected_V2_Team2_MIC",
];

#[cfg(not(feature = "full_load"))]
const WHITELIST_MATS: [&str; 4] = [
    "FutureTech.Materials.ForceField_Mat",
    "FutureTech.Materials.ForceField_HexGage_MIC",
    "FutureTech.Materials.HexGlass_WithArrows_Team2_MIC",
    "FutureTech.Materials.HexGlass_WithArrows_Team1_MIC",
];

#[cfg(not(feature = "full_load"))]
#[inline]
fn is_in_whitelist(name: &str) -> bool {
    WHITELIST_MATS.contains(&name)
}

#[cfg(feature = "full_load")]
#[inline]
fn is_in_whitelist(_name: &str) -> bool {
    true
}

fn retreive_material(name: &str, asset_server: &AssetServer, base_color: Color) -> Option<StandardMaterial> {
    if name.is_empty() {
        return None;
    }

    if !is_in_whitelist(name) {
        // load custom material instead
        let mut material = if name == "Stadium_Assets.Materials.Grass_Base_Team1_MIC" {
            StandardMaterial::from(Color::rgb(0.1, 0.6, 0.1))
        } else if name == "FutureTech.Materials.Reflective_Floor_V2_Mat" {
            StandardMaterial::from(Color::rgb(0.1, 0.1, 0.8))
        } else if name == "FutureTech.Materials.Frame_01_V2_Mat" {
            StandardMaterial::from(Color::rgb(0.25, 0.1, 0.25))
        } else if name == "FutureTech.Materials.Frame_01_White_MIC" {
            StandardMaterial::from(Color::SILVER)
        } else if name == "FutureTech.Materials.CrossHatched_Grate_MIC" {
            StandardMaterial::from(Color::TOMATO)
        } else if [
            "Pickup_Boost.Materials.BoostPad_Small_MIC",
            "Pickup_Boost.Materials.BoostPad_Large_MIC",
        ]
        .contains(&name)
        {
            StandardMaterial::from(Color::rgb(0.8, 0.1, 0.1))
        } else if name.contains("Advert") {
            StandardMaterial::from(Color::BISQUE)
        } else {
            return None;
        };

        if TRANSPARENT_MATS.contains(&name) {
            material.alpha_mode = AlphaMode::Blend;
        } else if ADD_MATS.contains(&name) {
            material.alpha_mode = AlphaMode::Add;
        }

        if DOUBLE_SIDED_MATS.contains(&name) {
            material.cull_mode = None;
            material.double_sided = true;
        }

        return Some(material);
    }

    debug!("Retreiving material {name}");
    let material_folder = if name.ends_with("MIC") || name.contains(".MIC_") || name.ends_with("Mic") {
        ".MaterialInstanceConstant."
    } else {
        ".Material3."
    };
    let mut pre_path = name
        .replace(".Materials.", material_folder)
        .replace(".Mat.", material_folder)
        .replace('.', "/");

    let mut split = pre_path.split('/');
    if let Some(first) = split.next() {
        if let Some(second) = split.next() {
            if split.next().is_none() {
                pre_path = format!("{first}/{}/{second}", &material_folder[1..material_folder.len() - 1]);
            }
        }
    }

    let path = format!("./assets/{pre_path}.mat");
    let Ok(mat_file) = fs::read_to_string(&path) else {
        error!("Failed to read {path} ({name})");
        return None;
    };

    let props: String = format!("./assets/{pre_path}.props.txt");
    let Ok(props_file) = fs::read_to_string(&props) else {
        error!("Failed to read {path} ({name})");
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
                error!("No value for {key} in {path}");
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
                    warn!("Unknown key {x} is {value} in {path} ({name})");
                }
            }
        }
    }

    let mut material = StandardMaterial {
        base_color,
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
                        error!("Unknown blend mode {value} in {path} ({name})");
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
        material.alpha_mode = AlphaMode::Blend;
    } else if ADD_MATS.contains(&name) {
        material.alpha_mode = AlphaMode::Add;
    }

    if double_sided.unwrap_or_default() || DOUBLE_SIDED_MATS.contains(&name) {
        material.cull_mode = None;
        material.double_sided = true;
    }

    if let Some(texture_name) = diffuse {
        debug!("Found texture for {name}");
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

pub fn get_material(
    name: &str,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    base_color: Option<Color>,
) -> Handle<StandardMaterial> {
    let mut material_names = MATERIALS.lock().unwrap();

    if let Some(material) = material_names.get(name) {
        return material.clone();
    }

    let base_color = base_color.unwrap_or(Color::rgb(0.3, 0.3, 0.3));

    material_names
        .entry(name.to_string())
        .or_insert_with(|| {
            materials.add(retreive_material(name, asset_server, base_color).unwrap_or(StandardMaterial {
                base_color,
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
        extra_uvs.push([
            reader.read_f32::<LittleEndian>().unwrap(),
            reader.read_f32::<LittleEndian>().unwrap(),
        ]);
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
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut bevy::asset::LoadContext,
    ) -> bevy::utils::BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let asset_name = load_context.path().file_name().and_then(|name| name.to_str()).unwrap();
            load_context.set_default_asset(LoadedAsset::new(MeshBuilder::from_pskx(asset_name, bytes)?.build_mesh(1.)));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["pskx", "psk"]
    }
}

const UMODEL: &str = if cfg!(windows) { "umodel.exe" } else { "./umodel" };
const OUT_DIR: &str = "./assets/";
const OUT_DIR_VER: &str = "./assets/files.txt";

fn get_input_dir() -> Option<String> {
    let Ok(input_file) = fs::read_to_string("assets.path") else {
        error!("Couldn't find 'assets.path' file in your base folder! Create the file with the path to your 'rocketleague/TAGame/CookedPCConsole' folder.");
        return None;
    };

    let Some(assets_dir) = input_file.lines().next() else {
        error!("Your 'assets.path' file is empty! Create the file with the path to your 'rocketleague/TAGame/CookedPCConsole' folder.");
        return None;
    };

    let assets_path = Path::new(assets_dir);
    if assets_path.is_dir() && assets_path.exists() {
        Some(assets_dir.to_string())
    } else {
        error!("Couldn't find the directory specified in your 'assets.path'!");
        None
    }
}

const UPK_FILES: [&str; 9] = [
    "Startup.upk",
    "MENU_Main_p.upk",
    "Stadium_P.upk",
    "Body_MuscleCar_SF.upk",
    "Body_Darkcar_SF.upk",
    "Body_CarCar_SF.upk",
    "Body_Venom_SF.upk",
    "Body_Force_SF.upk",
    "Body_Vanquish_SF.upk",
];

fn has_existing_assets() -> io::Result<bool> {
    //ensure all upk files are listen in ver_file
    let ver_file = fs::read_to_string(OUT_DIR_VER)?;
    let file_count = ver_file.lines().filter(|line| UPK_FILES.contains(line)).count();

    Ok(file_count == UPK_FILES.len())
}

pub fn uncook() -> io::Result<()> {
    if has_existing_assets().unwrap_or_default() {
        info!("Found existing assets");
        return Ok(());
    }

    let input_dir = get_input_dir().unwrap();

    info!("Uncooking assets from Rocket League...");

    // let upk_files = fs::read_dir(&input_dir)?
    //     .filter_map(|entry| {
    //         let entry = entry.unwrap();
    //         let path = entry.path();
    //         if path.is_file() && path.extension().unwrap_or_default() == "upk" {
    //             Some(path.file_name().unwrap().to_str().unwrap().to_string())
    //         } else {
    //             None
    //         }
    //     })
    //     .collect::<Vec<_>>();

    if !Path::new(UMODEL).exists() {
        panic!("Couldn't find umodel.exe! Make sure it's in the same folder as the executable.");
    }

    for (i, file) in UPK_FILES.into_iter().enumerate() {
        print!("Processing file {i}/{} ({file})...                       \r", UPK_FILES.len());
        io::stdout().flush()?;

        // call umodel to uncook all the map files
        let mut child = Command::new(UMODEL)
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

    // write each item in the list to "OUTDIR/files.txt"
    fs::write(OUT_DIR_VER, UPK_FILES.join("\n"))?;

    println!("Done processing files                                 ");

    Ok(())
}
