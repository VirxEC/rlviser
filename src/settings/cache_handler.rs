use crate::{GameLoadState, assets::load_assets, mesh::MeshBuilder};
use ahash::AHashMap;
use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::*,
    render::renderer::RenderDevice,
};
use std::{
    fs::{File, copy, create_dir_all, read_to_string},
    io::Read,
    path::{MAIN_SEPARATOR, Path},
    sync::RwLock,
};
use walkdir::WalkDir;

static MESHES: RwLock<Option<AHashMap<String, Vec<Handle<Mesh>>>>> = RwLock::new(None);
static MESH_MATERIALS: RwLock<Option<AHashMap<String, Vec<MeshMaterial>>>> = RwLock::new(None);
static TEXTURES: RwLock<Option<AHashMap<String, Handle<Image>>>> = RwLock::new(None);

#[cfg(debug_assertions)]
mod cache {
    use crate::GameLoadState;
    use bevy::prelude::*;

    pub fn load_cache(mut state: ResMut<NextState<GameLoadState>>) {
        state.set(GameLoadState::Connect);
    }
}

#[cfg(not(debug_assertions))]
mod cache {
    use crate::GameLoadState;
    use ahash::AHashMap;
    use bevy::{prelude::*, render::renderer::RenderDevice};
    use include_flate::flate;
    use std::io::Cursor;
    use zip::ZipArchive;

    flate!(static CACHED_ASSETS: [u8] from "cache.zip");

    pub fn load_cache(
        mut state: ResMut<NextState<GameLoadState>>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut images: ResMut<Assets<Image>>,
        render_device: Option<Res<RenderDevice>>,
    ) {
        let seeker = Cursor::new(&*CACHED_ASSETS);
        let mut archive = ZipArchive::new(seeker).unwrap();

        let mut mesh_cache_lock = super::MESHES.write().unwrap();
        let mut material_cache_lock = super::MESH_MATERIALS.write().unwrap();
        let mut texture_cache_lock = super::TEXTURES.write().unwrap();

        let mesh_cache = mesh_cache_lock.get_or_insert_with(AHashMap::new);
        let material_cache = material_cache_lock.get_or_insert_with(AHashMap::new);
        let texture_cache = texture_cache_lock.get_or_insert_with(AHashMap::new);

        for i in 0..archive.len() {
            let file = archive.by_index(i).unwrap();

            if !file.is_file() {
                continue;
            }

            let file_name = file.enclosed_name().unwrap();
            let name = file_name.file_stem().unwrap().to_string_lossy().to_string();
            let parent = file_name.parent().unwrap().file_name().unwrap().to_string_lossy();

            match parent.as_ref() {
                "mesh" => {
                    let builder = super::MeshBuilder::from_cache(file);
                    let meshes = builder.build_meshes().into_iter().map(|mesh| meshes.add(mesh)).collect();
                    mesh_cache.insert(name, meshes);
                }
                "textures" => {
                    let texture = super::read_tga(file, render_device.as_deref());
                    texture_cache.insert(name, images.add(texture));
                }
                "material" => {
                    let material = super::MeshMaterial::from_cache(file);
                    material_cache.insert(name, vec![material]);
                }
                _ => {
                    warn!("Unknown cache type {parent}");
                }
            }
        }

        state.set(GameLoadState::Connect);
    }
}

pub struct CachePlugin;

impl Plugin for CachePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (cache::load_cache, load_assets)
                .chain()
                .run_if(in_state(GameLoadState::Cache)),
        );
    }
}

pub fn get_default_mesh_cache(path: &'static str, assets: &AssetServer, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
    let name = path.split('/').next_back().unwrap().trim_end_matches(".pskx");

    if let Some(meshes) = check_mesh_cache(name) {
        return meshes[0].clone();
    }

    assert!(cfg!(debug_assertions), "Failed to load mesh {name}");

    let cache_path = format!("./cache/mesh/{name}.bin");
    if let Ok(mesh) = File::open(&cache_path) {
        return meshes.add(MeshBuilder::from_cache(mesh).build_mesh());
    }

    warn!("Cache not found for mesh {name}");

    let handle = assets.load(path);
    MESHES
        .write()
        .unwrap()
        .get_or_insert_with(AHashMap::new)
        .insert(name.to_string(), vec![handle.clone()]);

    handle
}

pub fn get_mesh_cache<P: AsRef<Path>>(
    cache_path: P,
    asset_path: P,
    name: &str,
    meshes: &mut Assets<Mesh>,
) -> Option<Vec<Handle<Mesh>>> {
    fn inner(cache_path: &Path, asset_path: &Path, name: &str, meshes: &mut Assets<Mesh>) -> Option<Vec<Handle<Mesh>>> {
        let name = name.split('.').next_back().unwrap();
        if let Some(meshes) = check_mesh_cache(name) {
            return Some(meshes);
        }

        if !cfg!(debug_assertions) {
            return None;
        }

        if let Ok(file) = File::open(cache_path) {
            let builder = MeshBuilder::from_cache(file);
            return Some(insert_mesh_cache(name.to_string(), builder, meshes));
        }

        warn!("Cache not found for mesh {name}");

        // read bytes from path
        let Ok(mut file) = File::open(asset_path) else {
            error!("Failed to open mesh {} for {name}", asset_path.display());
            return None;
        };

        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).ok()?;

        let builder = MeshBuilder::from_pskx(name, &bytes).ok()?;
        builder.create_cache(cache_path);
        Some(insert_mesh_cache(name.to_string(), builder, meshes))
    }

    inner(cache_path.as_ref(), asset_path.as_ref(), name, meshes)
}

fn check_mesh_cache(name: &str) -> Option<Vec<Handle<Mesh>>> {
    MESHES.read().ok()?.as_ref().and_then(|map| map.get(name).cloned())
}

fn insert_mesh_cache(name: String, builder: MeshBuilder, meshes: &mut Assets<Mesh>) -> Vec<Handle<Mesh>> {
    let meshes: Vec<_> = builder.build_meshes().into_iter().map(|mesh| meshes.add(mesh)).collect();

    let mut map_lock = MESHES.write().unwrap();
    map_lock.get_or_insert_with(AHashMap::new).insert(name, meshes.clone());

    meshes
}

#[derive(Clone, Copy, bincode::Encode, bincode::Decode)]
pub enum CAlphaMode {
    Opaque,
    Mask(f32),
    Blend,
    Premultiplied,
    AlphaToCoverage,
    Add,
    Multiply,
}

impl From<CAlphaMode> for AlphaMode {
    fn from(mode: CAlphaMode) -> Self {
        match mode {
            CAlphaMode::Opaque => Self::Opaque,
            CAlphaMode::Mask(threshold) => Self::Mask(threshold),
            CAlphaMode::Blend => Self::Blend,
            CAlphaMode::Premultiplied => Self::Premultiplied,
            CAlphaMode::AlphaToCoverage => Self::AlphaToCoverage,
            CAlphaMode::Add => Self::Add,
            CAlphaMode::Multiply => Self::Multiply,
        }
    }
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
pub struct MeshMaterial {
    pub diffuse: Option<String>,
    pub normal: Option<String>,
    pub other: Vec<String>,
    pub alpha_mode: Option<CAlphaMode>,
    pub mask_clip_value: f32,
    pub double_sided: bool,
}

impl MeshMaterial {
    fn new(name: &str, pre_path: String) -> Option<Self> {
        let path = format!("./assets/{pre_path}.mat");
        let Ok(mat_file) = read_to_string(&path) else {
            error!("Failed to read {path} ({name})");
            return None;
        };

        let props = format!("./assets/{pre_path}.props.txt");
        let Ok(props_file) = read_to_string(props) else {
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
                        diffuse = Some(value.to_string());
                    }
                    "Normal" => {
                        normal = Some(value.to_string());
                    }
                    "Other[0]" => {
                        other.push(value.to_string());
                    }
                    x => {
                        warn!("Unknown key {x} is {value} in {path} ({name})");
                    }
                }
            }
        }

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
                        "BLEND_Opaque (0)" => Some(CAlphaMode::Opaque),
                        "BLEND_Masked (1)" => Some(CAlphaMode::Mask(mask_clip_value)),
                        "BLEND_Translucent (2)" => Some(CAlphaMode::Blend),
                        "BLEND_Additive (3)" => Some(CAlphaMode::Add),
                        _ => {
                            error!("Unknown blend mode {value} in {path} ({name})");
                            None
                        }
                    };
                } else if key == "OpacityMaskClipValue"
                    && let Ok(mask_value) = value.parse()
                {
                    mask_clip_value = mask_value;

                    if let Some(CAlphaMode::Mask(_)) = alpha_mode {
                        alpha_mode = Some(CAlphaMode::Mask(mask_clip_value));
                    }
                }
            }
        }

        Some(Self {
            diffuse,
            normal,
            other,
            alpha_mode,
            mask_clip_value,
            double_sided: double_sided.unwrap_or_default(),
        })
    }

    fn create_cache(&self, path: &Path) {
        create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = File::create(path).unwrap();
        bincode::encode_into_std_write(self, &mut file, bincode::config::legacy()).unwrap();
    }

    fn from_cache<R: Read>(mut file: R) -> Self {
        bincode::decode_from_std_read(&mut file, bincode::config::legacy()).unwrap()
    }
}

pub fn get_material_cache<P: AsRef<Path>>(cache_path: P, asset_path: P, name: &str) -> Option<MeshMaterial> {
    fn inner(cache_path: &Path, asset_path: &Path, name: &str) -> Option<MeshMaterial> {
        let name = name.split('.').next_back().unwrap();
        if let Some(materials) = MESH_MATERIALS.read().ok()?.as_ref().and_then(|map| map.get(name)) {
            return Some(materials[0].clone());
        }

        if !cfg!(debug_assertions) {
            return None;
        }

        if let Ok(file) = File::open(cache_path) {
            return Some(MeshMaterial::from_cache(file));
        }

        warn!("Cache not found for material {name}");

        let Some(material) = MeshMaterial::new(name, asset_path.to_string_lossy().to_string()) else {
            error!("Failed to create material for {name}");
            return None;
        };

        material.create_cache(cache_path);
        Some(material)
    }

    inner(cache_path.as_ref(), asset_path.as_ref(), name)
}

fn read_tga<R: Read>(mut reader: R, render_device: Option<&RenderDevice>) -> Image {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();

    let image_type = ImageType::Extension("tga");

    let supported_compressed_formats = render_device.map_or(CompressedImageFormats::NONE, |render_device| {
        CompressedImageFormats::from_features(render_device.features())
    });

    Image::from_buffer(
        &bytes,
        image_type,
        supported_compressed_formats,
        true,
        ImageSampler::Default,
        RenderAssetUsages::default(),
    )
    .unwrap()
}

pub fn get_texture_cache(
    name: &str,
    asset_server: &AssetServer,
    images: &mut Assets<Image>,
    render_device: Option<&RenderDevice>,
) -> Handle<Image> {
    if let Some(texture) = TEXTURES.read().unwrap().as_ref().and_then(|map| map.get(name)) {
        return texture.clone();
    }

    assert!(cfg!(debug_assertions), "Failed to load texture {name}");

    let cache_path_name = format!("./cache/textures/{name}.tga");
    let cache_path = Path::new(&cache_path_name);
    if cache_path.exists() {
        let file = File::open(cache_path).unwrap();
        return images.add(read_tga(file, render_device));
    }

    warn!("Cache not found for texture {name}");

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

    // copy file to cache_path
    create_dir_all(cache_path.parent().unwrap()).unwrap();
    copy(format!("./assets/{path}"), cache_path).unwrap();

    asset_server.load(path)
}
