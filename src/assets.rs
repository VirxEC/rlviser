use crate::mesh::{MeshBuilder, MeshBuilderError};
use bevy::{
    asset::{io::Reader, AssetLoader, AsyncReadExt},
    prelude::*,
};
use byteorder::{LittleEndian, ReadBytesExt};
use once_cell::sync::Lazy;
use rust_search::{similarity_sort, SearchBuilder};
use std::{
    collections::HashMap,
    ffi::OsStr,
    fs,
    io::{self, Read, Write},
    panic,
    path::{Path, MAIN_SEPARATOR},
    process::{Command, Stdio},
    sync::Mutex,
};
use thiserror::Error;
use walkdir::WalkDir;

pub struct AssetsLoaderPlugin;

impl Plugin for AssetsLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.register_asset_loader(PskxLoader)
            .init_asset::<Mesh>()
            .add_systems(Startup, load_assets);
    }
}

#[derive(Resource)]
pub struct CarWheelMesh {
    pub mesh: Handle<Mesh>,
}

fn load_assets(mut commands: Commands, assets: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(CarWheelMesh {
        mesh: assets.load("WHEEL_Star/StaticMesh3/WHEEL_Star_SM.pskx"),
    });

    commands.insert_resource(BoostPickupGlows {
        small: assets.load("Pickup_Boost/StaticMesh3/BoostPad_Small_02_SM.pskx"),
        small_hitbox: meshes.add(Cylinder::new(144. / 2., 165.)),
        large: assets.load("Pickup_Boost/StaticMesh3/BoostPad_Large_Glow.pskx"),
        large_hitbox: meshes.add(Cylinder::new(208. / 2., 168.)),
    });

    commands.insert_resource(BallAssets {
        ball_diffuse: assets.load("Ball_Default_Textures/Texture2D/Ball_Default00_D.tga"),
        // ball_normal: assets.load("Ball_Default_Textures/Texture2D/Ball_Default00_N.tga"),
        // ball_occlude: assets.load("Ball_Default_Textures/Texture2D/Ball_Default00_RGB.tga"),
        ball: assets.load("Ball_Default/StaticMesh3/Ball_DefaultBall00.pskx"),
    });
}

#[derive(Resource)]
pub struct BallAssets {
    pub ball_diffuse: Handle<Image>,
    // pub ball_normal: Handle<Image>,
    // pub ball_occlude: Handle<Image>,
    pub ball: Handle<Mesh>,
}

#[derive(Resource)]
pub struct BoostPickupGlows {
    pub small: Handle<Mesh>,
    pub small_hitbox: Handle<Mesh>,
    pub large: Handle<Mesh>,
    pub large_hitbox: Handle<Mesh>,
}

const BLOCK_MESHES: [&str; 8] = [
    "CollisionMeshes",
    "DecalBlocker",
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
const WHITELIST_MESHES: [&str; 21] = [
    "Field_STD",
    "FF",
    "BoostPads",
    "BoostPad_Small",
    "BoostPad_Large",
    "Goal",
    "AdvertStrip",
    "Field_Center",
    "Field_Mid",
    "Body",
    "Side",
    "Floor",
    "Lattice",
    "FoamAd",
    "Lines_Basketball",
    "Net_Collision",
    "BBall_Walls_03",
    "BBallRim01",
    "BBall_Edges_01",
    "BackBoard",
    "Net_Rim",
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
        .replace(".Collision", ".StaticMesh3")
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
        warn!("Failed to open mesh {path} for {name}");
        return None;
    };

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;

    let builder = MeshBuilder::from_pskx(name, &bytes).ok()?;
    Some(builder.build_meshes(1.).into_iter().map(|mesh| meshes.add(mesh)).collect())
}

fn load_texture(name: &str, asset_server: &AssetServer) -> Handle<Image> {
    let mut assets_path = String::from("assets");
    assets_path.push(MAIN_SEPARATOR);

    let path = WalkDir::new("assets")
        .into_iter()
        .flatten()
        .find(|x| x.file_name().to_string_lossy() == format!("{name}.tga"))
        .unwrap()
        .path()
        .to_string_lossy()
        .to_string()
        .replace(&assets_path, "");

    asset_server.load(path)
}

const DOUBLE_SIDED_MATS: [&str; 28] = [
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
    "Proto_BBall.Materials.BBall_Net_MAT_INST",
    "Graybox_Assets.Materials.NetNonmove_Mat",
    "Proto_BBall.Materials.BBall_Rubber_MIC",
    "Proto_BBall.Materials.MIC_DarkGlass",
    "Proto_BBall.SM.BackBoard_Teams_MIC",
    "Proto_BBall.Materials.BBall_Rim_MAT_INST",
    "Pickup_Boost.Materials.BoostPad_Large_MIC",
    "Pickup_Boost.Materials.BoostPad_Small_MIC",
];

const TRANSPARENT_MATS: [&str; 2] = [
    "Trees.Materials.LombardyPoplar_B_NoWind_MIC",
    "Trees.Materials.LombardyPoplar_B_Mat",
];

const ADD_MATS: [&str; 15] = [
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
    "Graybox_Assets.Materials.NetNonmove_Mat",
    "Proto_BBall.Materials.BBall_Net_MAT_INST",
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
        return get_default_material(name);
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

    let props = format!("./assets/{pre_path}.props.txt");
    let Ok(props_file) = fs::read_to_string(props) else {
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

fn get_default_material(name: &str) -> Option<StandardMaterial> {
    let mut material = if [
        "Stadium_Assets.Materials.Grass_Base_Team1_MIC",
        "Proto_BBall.Materials.WoodFloor_Corrected_Mat_INST",
    ]
    .contains(&name)
    {
        // primary
        StandardMaterial::from(Color::rgb_u8(45, 49, 66))
    } else if [
        "FutureTech.Materials.Reflective_Floor_V2_Mat",
        "Proto_BBall.Materials.BBall_Rubber_MIC",
        "Proto_BBall.SM.BackBoard_Teams_MIC",
        "Proto_BBall.Materials.MIC_DarkGlass",
    ]
    .contains(&name)
    {
        // secondary
        StandardMaterial::from(Color::rgb_u8(79, 93, 117))
    } else if [
        "FutureTech.Materials.Frame_01_MIC",
        "FutureTech.Materials.Frame_01_V2_Mat",
        "Proto_BBall.Materials.BBall_Net_MAT_INST",
        "Proto_BBall.Materials.BBall_Rim_MAT_INST",
        "Graybox_Assets.Materials.NetNonmove_Mat",
        "Proto_BBall.Materials.OLDCosmicGlass1_INST",
        "OldCosmic_Assets.Materials.OLDCosmicGlass1",
        "Proto_BBall.Materials.BBall_Rim2_MAT_INST",
        "Proto_BBall.Materials.BBall_RimF_MAT_INST",
    ]
    .contains(&name)
        || name.contains("PaintedLine_MIC")
    {
        // tertiary
        StandardMaterial::from(Color::rgb_u8(55, 30, 48))
    } else if [
        "FutureTech.Materials.Frame_01_White_MIC",
        "Graybox_Assets.Materials.ForceFieldCage_Solid_Mat",
    ]
    .contains(&name)
    {
        StandardMaterial::from(Color::SILVER)
    } else if name == "FutureTech.Materials.CrossHatched_Grate_MIC" {
        StandardMaterial::from(Color::TOMATO)
    } else if [
        "Pickup_Boost.Materials.BoostPad_Small_MIC",
        "Pickup_Boost.Materials.BoostPad_Large_MIC",
    ]
    .contains(&name)
    {
        StandardMaterial::from(Color::rgb_u8(152, 29, 23))
    } else if name.contains("Advert") || name.contains("DarkMetal") {
        StandardMaterial::from(Color::rgb_u8(191, 192, 192))
    } else {
        println!("Unknown material {name}");
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

    material.perceptual_roughness = 0.6;
    material.reflectance = 0.2;

    Some(material)
}

static MATERIALS: Mutex<Lazy<HashMap<Box<str>, Handle<StandardMaterial>>>> = Mutex::new(Lazy::new(HashMap::new));

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
        .entry(Box::from(name))
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

pub fn read_vertices(chunk_data: &[u8], data_count: usize, vertices: &mut Vec<f32>) {
    vertices.reserve(data_count * 3);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        vertices.push(reader.read_f32::<LittleEndian>().unwrap());
        let y = reader.read_f32::<LittleEndian>().unwrap();
        vertices.push(reader.read_f32::<LittleEndian>().unwrap());
        vertices.push(-y);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Wedge {
    pub vertex_id: usize,
    pub uv: [f32; 2],
    pub material_index: usize,
}

pub fn read_wedges(chunk_data: &[u8], data_count: usize, wedges: &mut Vec<Wedge>) {
    wedges.reserve(data_count);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        let vertex_id = reader.read_u32::<LittleEndian>().unwrap() as usize;
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
}

pub fn read_faces(
    chunk_data: &[u8],
    data_count: usize,
    wedges: &[Wedge],
    ids: &mut Vec<usize>,
    uvs: &mut Vec<[f32; 2]>,
    mat_ids: &mut Vec<usize>,
) {
    ids.reserve(data_count * 3);
    uvs.reserve(data_count * 3);
    mat_ids.reserve(data_count * 3);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        let wdg_idxs = [
            reader.read_u16::<LittleEndian>().unwrap() as usize,
            reader.read_u16::<LittleEndian>().unwrap() as usize,
            reader.read_u16::<LittleEndian>().unwrap() as usize,
        ];
        let _mat_index = reader.read_u8().unwrap();
        let _aux_mat_index = reader.read_u8().unwrap();
        let _smoothing_group = reader.read_u32::<LittleEndian>().unwrap();

        let verts = [&wedges[wdg_idxs[0]], &wedges[wdg_idxs[1]], &wedges[wdg_idxs[2]]];

        ids.extend([verts[1].vertex_id, verts[0].vertex_id, verts[2].vertex_id]);
        uvs.extend([verts[1].uv, verts[0].uv, verts[2].uv]);
        mat_ids.extend([verts[1].material_index, verts[0].material_index, verts[2].material_index]);
    }
}

pub fn read_vertex_colors(chunk_data: &[u8], data_count: usize) -> Vec<[f32; 4]> {
    let mut vertex_colors = Vec::with_capacity(data_count);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        vertex_colors.push([
            f32::from(reader.read_u8().unwrap()),
            f32::from(reader.read_u8().unwrap()),
            f32::from(reader.read_u8().unwrap()),
            f32::from(reader.read_u8().unwrap()),
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

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum PskxLoaderError {
    #[error("Couldn't read PSK(X): {0}")]
    Io(#[from] io::Error),
    #[error("Couldn't load PSK(X): {0}")]
    MeshBuilder(#[from] MeshBuilderError),
}

impl AssetLoader for PskxLoader {
    type Asset = Mesh;
    type Settings = ();
    type Error = PskxLoaderError;

    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        _settings: &'a Self::Settings,
        load_context: &'a mut bevy::asset::LoadContext,
    ) -> bevy::utils::BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;

            let asset_name = load_context.path().file_name().and_then(OsStr::to_str).unwrap();
            Ok(MeshBuilder::from_pskx(asset_name, &bytes)?.build_mesh(1.))
        })
    }

    fn extensions(&self) -> &[&str] {
        &["pskx", "psk"]
    }
}

const CANT_FIND_FOLDER: &str = "Couldn't find 'RocketLeague.exe' on your system! Please manually create the file 'assets.path' and add the path in plain text to your 'rocketleague/TAGame/CookedPCConsole' folder. This is needed for UModel to work.";
const UMODEL: &str = if cfg!(windows) { ".\\umodel.exe" } else { "./umodel" };
const OUT_DIR: &str = "./assets/";
const OUT_DIR_VER: &str = "./assets/files.txt";

fn find_input_dir() -> String {
    println!("Couldn't find 'assets.path' file in your base folder!");
    print!("Try to automatically find the path? (y/n): ");

    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    if input.trim().to_lowercase() != "y" {
        panic!("{CANT_FIND_FOLDER}");
    }

    println!("Searching system for 'RocketLeague.exe'...");

    let search_input = "RocketLeague";
    let start_dir = if cfg!(windows) { "C:\\" } else { "~" };

    let mut search = SearchBuilder::default()
        .location(start_dir)
        .search_input(search_input)
        .ext("exe")
        .strict()
        .hidden()
        .build()
        .collect();

    similarity_sort(&mut search, search_input);

    if search.is_empty() {
        panic!("{CANT_FIND_FOLDER}");
    }

    let mut input = String::new();

    let mut game_path = None;

    if search.len() == 1 {
        println!("Found (1) result!");
    } else {
        println!("Found ({}) results!", search.len());
    }

    for path in &search {
        print!("{path} - use this path? (y/n): ");
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut input).unwrap();

        if input.trim().to_lowercase() == "y" {
            game_path = Some(path);
            break;
        }
    }

    let input_dir = Path::new(game_path.expect(CANT_FIND_FOLDER))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("TAGame/CookedPCConsole");

    assert!(
        input_dir.is_dir(),
        "Couldn't find 'rocketleague/TAGame/CookedPCConsole' folder! Make sure you select the correct path to a Windows version of Rocket League."
    );
    let input_dir = input_dir.to_string_lossy().to_string();

    println!("Writing '{input_dir}' to 'assets.path'...");
    fs::write("assets.path", &input_dir).expect("Couldn't write to 'assets.path'!");

    input_dir
}

fn get_input_dir() -> String {
    let input_file = fs::read_to_string("assets.path").unwrap_or_else(|_| find_input_dir());

    let Some(assets_dir) = input_file.lines().next() else {
        panic!("Your 'assets.path' file is empty! Create the file with the path to your 'rocketleague/TAGame/CookedPCConsole' folder.");
    };

    let assets_path = Path::new(assets_dir);
    if assets_path.is_dir() && assets_path.exists() {
        assets_dir.to_string()
    } else {
        panic!("Couldn't find the directory specified in your 'assets.path'!");
    }
}

const UPK_FILES: [&str; 10] = [
    "Startup.upk",
    "MENU_Main_p.upk",
    "Stadium_P.upk",
    "HoopsStadium_P.upk",
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
        println!("Couldn't find UModel! Make sure it's in the same folder as the executable. Using default assets!");
        return Ok(());
    }

    let input_dir = get_input_dir();

    info!("Uncooking assets from Rocket League...");

    let num_files = UPK_FILES.len();
    // let num_files = upk_files.len();

    for (i, file) in UPK_FILES.into_iter().enumerate() {
        print!("Processing file {i}/{num_files} ({file})...                       \r");
        io::stdout().flush()?;

        // call umodel to uncook all the map files
        let mut child = Command::new(UMODEL)
            .args([
                format!("-path={input_dir}"),
                format!("-out={OUT_DIR}"),
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
