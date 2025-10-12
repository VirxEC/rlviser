use crate::{
    mesh::{MeshBuilder, MeshBuilderError},
    rocketsim::Team,
    settings::cache_handler::{get_default_mesh_cache, get_material_cache, get_mesh_cache, get_texture_cache},
};
use ahash::AHashMap;
use bevy::{
    asset::{AssetLoader, io::Reader},
    color::palettes::css,
    mesh::CylinderMeshBuilder,
    prelude::*,
    render::renderer::RenderDevice,
    tasks::ConditionalSendFuture,
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::{
    ffi::OsStr,
    io::{self, Read},
    path::Path,
    sync::Mutex,
};
use thiserror::Error;

pub struct AssetsLoaderPlugin;

impl Plugin for AssetsLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.register_asset_loader(PskxLoader);
    }
}

#[derive(Resource)]
pub struct CarWheelMesh {
    pub mesh: Handle<Mesh>,
}

pub fn load_assets(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    render_device: Option<Res<RenderDevice>>,
) {
    commands.insert_resource(CarWheelMesh {
        mesh: get_default_mesh_cache("WHEEL_Star/StaticMesh3/WHEEL_Star_SM.pskx", &assets, &mut meshes),
    });

    commands.insert_resource(BoostPickupGlows {
        small: get_default_mesh_cache("Pickup_Boost/StaticMesh3/BoostPad_Small_02_SM.pskx", &assets, &mut meshes),
        small_hitbox: meshes.add(CylinderMeshBuilder::new(144. / 2., 165., 32)),
        large: get_default_mesh_cache("Pickup_Boost/StaticMesh3/BoostPad_Large_Glow.pskx", &assets, &mut meshes),
        large_hitbox: meshes.add(CylinderMeshBuilder::new(208. / 2., 168., 32)),
    });

    commands.insert_resource(BallAssets {
        ball_diffuse: get_texture_cache("Ball_Default00_D", &assets, &mut images, render_device.as_deref()),
        // ball_normal: get_texture_cache("Ball_Default00_N", &assets, &mut images, render_device.as_deref()),
        // ball_occlude: get_texture_cache("Ball_Default00_RGB", &assets, &mut images, render_device.as_deref()),
        ball: get_default_mesh_cache("Ball_Default/StaticMesh3/Ball_DefaultBall00.pskx", &assets, &mut meshes),
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

const BLOCK_MESHES: [&str; 9] = [
    "CollisionMeshes",
    "DecalBlocker",
    "FieldCollision_Standard",
    "Goal_STD_Outer",
    "SkySphere01",
    "Glow",
    "Fog",
    "FX_General",
    "Collision_Plane",
];

#[cfg(not(feature = "full_load"))]
const EXTRA_BLACKLIST: [&str; 1] = ["Side_Trim"];

#[cfg(not(feature = "full_load"))]
const WHITELIST_MESHES: [&str; 22] = [
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
    "BO_collision_03",
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
    if let Some(first) = split.next()
        && let Some(second) = split.next()
        && split.next().is_none()
    {
        local_path = format!("{first}/StaticMesh3/{second}");
    }

    let file_name = local_path.split('/').next_back().unwrap();
    let cache_path = format!("./cache/mesh/{file_name}.bin");

    let extension = if name.contains(".SkeletalMesh3") { "psk" } else { "pskx" };
    let asset_path = format!("./assets/{local_path}.{extension}");

    get_mesh_cache(cache_path, asset_path, name, meshes)
}

const DOUBLE_SIDED_MATS: [&str; 31] = [
    "Trees.Materials.LombardyPoplar_B_NoWind_MIC",
    "Trees.Materials.LombardyPoplar_B_Mat",
    "FutureTech.Materials.ForceField_HexGage_MIC",
    "FutureTech.Materials.HexGlass_WithArrows_Team2_MIC",
    "FutureTech.Materials.HexGlass_WithArrows_Team1_MIC",
    "FX_Lighting.Materials.LightCone_Simple_MIC",
    "Stadium.Materials.StadiumLight_Flare_Mat",
    "FutureTech.Materials.Frame_01_V2_Mat",
    "FutureTech.Materials.Reflective_Floor_V2_Mat",
    "FutureTech.Materials.Reflective_Floor_B_Mat",
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
    "OldCosmic_Assets.Materials.OLDCosmicGlass1",
    "Proto_BBall.Materials.BBall_DarkMetal_MIC",
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

fn retreive_material(
    name: &str,
    asset_server: &AssetServer,
    base_color: Color,
    side: Option<Team>,
    images: &mut Assets<Image>,
    render_device: Option<&RenderDevice>,
) -> Option<StandardMaterial> {
    if name.is_empty() {
        return None;
    }

    if !is_in_whitelist(name) {
        // load custom material instead
        return get_default_material(name, side);
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
    if let Some(first) = split.next()
        && let Some(second) = split.next()
        && split.next().is_none()
    {
        pre_path = format!("{first}/{}/{second}", &material_folder[1..material_folder.len() - 1]);
    }

    let file_name = pre_path.split('/').next_back().unwrap();
    let cache_path = format!("./cache/material/{file_name}.bin");
    let mesh_material = get_material_cache(cache_path, pre_path, name)?;

    let mut material = StandardMaterial {
        base_color,
        reflectance: 0.25,
        perceptual_roughness: 0.7,
        ..default()
    };

    if let Some(alpha_mode) = mesh_material.alpha_mode {
        material.alpha_mode = alpha_mode.into();
    } else if TRANSPARENT_MATS.contains(&name) {
        material.alpha_mode = AlphaMode::Blend;
    } else if ADD_MATS.contains(&name) {
        material.alpha_mode = AlphaMode::Add;
    }

    if mesh_material.double_sided || DOUBLE_SIDED_MATS.contains(&name) {
        material.cull_mode = None;
        material.double_sided = true;
    }

    if let Some(texture_name) = &mesh_material.diffuse {
        debug!("Found texture for {name}");
        if texture_name == "ForcefieldHex" {
            material.base_color = Color::srgba(0.3, 0.3, 0.3, 0.3);
        }
        material.base_color_texture = Some(get_texture_cache(texture_name, asset_server, images, render_device));
    }

    for texture_name in mesh_material.other {
        // idealy, the textures would be combined
        if mesh_material.diffuse.is_none() {
            material.base_color_texture = Some(get_texture_cache(&texture_name, asset_server, images, render_device));
        }
    }

    if let Some(texture_name) = mesh_material.normal {
        material.normal_map_texture = Some(get_texture_cache(&texture_name, asset_server, images, render_device));
    }

    Some(material)
}

fn get_default_material(name: &str, side: Option<Team>) -> Option<StandardMaterial> {
    let color = if [
        "Stadium_Assets.Materials.Grass_Base_Team1_MIC",
        "Proto_BBall.Materials.WoodFloor_Corrected_Mat_INST",
    ]
    .contains(&name)
    {
        // primary
        Color::srgb_u8(45, 49, 66)
    } else if [
        "FutureTech.Materials.Reflective_Floor_V2_Mat",
        "FutureTech.Materials.Reflective_Floor_B_Mat",
        "Proto_BBall.SM.BackBoard_Teams_MIC",
        "Proto_BBall.Materials.BBall_Rubber_MIC",
        "Proto_BBall.Materials.MIC_DarkGlass",
    ]
    .contains(&name)
    {
        // secondary
        match side {
            Some(Team::Blue) => Color::srgb_u8(86, 136, 199),
            Some(Team::Orange) => Color::srgb_u8(222, 145, 81),
            None => Color::srgb_u8(131, 144, 115),
        }
    } else if name == "OOBFloor_MAT_CUSTOM" {
        Color::srgb_u8(41, 2, 0)
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
        Color::srgb_u8(55, 30, 48)
    } else if [
        "FutureTech.Materials.Frame_01_White_MIC",
        "Graybox_Assets.Materials.ForceFieldCage_Solid_Mat",
    ]
    .contains(&name)
    {
        Color::from(css::SILVER)
    } else if name == "FutureTech.Materials.CrossHatched_Grate_MIC" {
        Color::from(css::TOMATO)
    } else if [
        "Pickup_Boost.Materials.BoostPad_Small_MIC",
        "Pickup_Boost.Materials.BoostPad_Large_MIC",
    ]
    .contains(&name)
    {
        Color::srgb_u8(152, 29, 23)
    } else if name == "Goal.Materials.GoalGenerator_Team2_MIC" || name.contains("CrossHatched") {
        Color::NONE
    } else if name.contains("Advert") || name.contains("DarkMetal") {
        Color::srgb_u8(191, 192, 192)
    } else if name.contains("Collision") {
        let mut color = css::SILVER;
        color.set_alpha(0.5);
        Color::from(color)
    } else {
        warn!("Unknown material {name}");
        return None;
    };

    let mut material = StandardMaterial::from(color);

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

type MaterialsKey = (&'static str, Option<Team>);
static MATERIALS: Mutex<Option<AHashMap<MaterialsKey, Handle<StandardMaterial>>>> = Mutex::new(None);

pub fn get_material(
    name: &str,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    base_color: Option<Color>,
    side: Option<Team>,
    images: &mut Assets<Image>,
    render_device: Option<&RenderDevice>,
) -> Handle<StandardMaterial> {
    let mut material_names_lock = MATERIALS.lock().unwrap();
    let material_names = material_names_lock.get_or_insert_with(AHashMap::new);

    if let Some(material) = material_names.get(&(name, side)) {
        return material.clone();
    }

    let name: &'static str = Box::leak(Box::from(name));
    let key = (name, side);

    let base_color = base_color.unwrap_or(Color::from(css::GREY));

    let mat = materials.add(
        retreive_material(name, asset_server, base_color, side, images, render_device).unwrap_or(StandardMaterial {
            base_color,
            metallic: 0.1,
            cull_mode: None,
            double_sided: true,
            ..default()
        }),
    );

    material_names.insert(key, mat.clone());

    mat
}

pub fn read_vertices(
    chunk_data: &[u8],
    data_count: usize,
    vertices: &mut Vec<f32>,
    uvs: &mut Vec<[f32; 2]>,
    mat_ids: &mut Vec<usize>,
) {
    vertices.reserve(data_count * 3);
    uvs.reserve(data_count * 3);
    mat_ids.reserve(data_count * 3);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        vertices.push(reader.read_f32::<LittleEndian>().unwrap());
        let y = reader.read_f32::<LittleEndian>().unwrap();
        vertices.push(reader.read_f32::<LittleEndian>().unwrap());
        vertices.push(-y);

        uvs.push([0.0, 0.0]);
        mat_ids.push(0);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Wedge {
    pub vertex_id: u32,
}

pub fn read_wedges(
    chunk_data: &[u8],
    data_count: usize,
    wedges: &mut Vec<Wedge>,
    uvs: &mut [[f32; 2]],
    mat_ids: &mut [usize],
) {
    wedges.reserve(data_count);

    let mut reader = io::Cursor::new(chunk_data);
    for _ in 0..data_count {
        let vertex_id = reader.read_u32::<LittleEndian>().unwrap();
        let u = reader.read_f32::<LittleEndian>().unwrap();
        let v = reader.read_f32::<LittleEndian>().unwrap();
        let material_index = reader.read_u8().unwrap() as usize;

        let idx = vertex_id as usize;
        uvs[idx][0] = u;
        uvs[idx][1] = v;
        mat_ids[idx] = material_index;

        wedges.push(Wedge { vertex_id });

        // read padding bytes
        reader.read_u8().unwrap();
        reader.read_u8().unwrap();
        reader.read_u8().unwrap();
    }
}

pub fn read_faces(chunk_data: &[u8], data_count: usize, wedges: &[Wedge], ids: &mut Vec<u32>) {
    ids.reserve(data_count * 3);

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

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;

            let asset_name = load_context.path().file_name().and_then(OsStr::to_str).unwrap();
            let mesh = MeshBuilder::from_pskx(asset_name, &bytes)?;

            let cache_path = format!("./cache/mesh/{}.bin", asset_name.trim_end_matches(".pskx"));
            mesh.create_cache(Path::new(&cache_path));

            Ok(mesh.build_mesh())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["pskx", "psk"]
    }
}

#[cfg(debug_assertions)]
pub mod umodel {
    use bevy::prelude::*;
    use rust_search::{SearchBuilder, similarity_sort};
    use std::{
        fs,
        io::{self, Write},
        panic,
        path::Path,
        process::{Command, Stdio},
    };

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

        assert!(input.trim().to_lowercase() == "y", "{CANT_FIND_FOLDER}");

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

        assert!(!search.is_empty(), "{CANT_FIND_FOLDER}");

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
            panic!(
                "Your 'assets.path' file is empty! Create the file with the path to your 'rocketleague/TAGame/CookedPCConsole' folder."
            );
        };

        let assets_path = Path::new(assets_dir);
        if assets_path.is_dir() && assets_path.exists() {
            assets_dir.to_string()
        } else {
            panic!("Couldn't find the directory specified in your 'assets.path'!");
        }
    }

    const UPK_FILES: [&str; 11] = [
        "Startup.upk",
        "MENU_Main_p.upk",
        "Stadium_P.upk",
        "HoopsStadium_P.upk",
        "ShatterShot_P.upk",
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
}
