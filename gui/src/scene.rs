use std::{fs, io::ErrorKind};

use anyhow::Result;
use nw_tex::bcres::CgfxContainer;
use raylib::{ffi, models::RaylibMaterial, RaylibHandle, RaylibThread};
use rfd::FileDialog;

use crate::{gfx_model::{load_bcres_model, load_bcres_textures}, material::{BasicMaterial, RlMaterial}, mesh::{BasicMesh, RlMesh}, GLOBAL_WORLD_SCALE};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct BasicModel {
    pub meshes: Vec<BasicMesh>,
    pub materials: Vec<BasicMaterial>,
}

fn load_scene(buf: &[u8]) -> Result<BasicModel> {
    let container = CgfxContainer::new(buf)?;
    
    if container.models.is_none() {
        return Ok(BasicModel::default())
    }
    
    fs::write("out.txt", format!("{:#?}", container.models.as_ref().unwrap()))?;
    
    let textures = load_bcres_textures(&container)?;
    
    let mut materials: Vec<BasicMaterial> = Vec::new();
    let mut meshes: Vec<BasicMesh> = Vec::new();
    
    for node in container.models.unwrap().nodes {
        if let Some(model) = node.value {
            let model = load_bcres_model(&model, &textures, GLOBAL_WORLD_SCALE,
                materials.len() as u32)?;
            
            materials.extend_from_slice(&model.materials);
            meshes.extend_from_slice(&model.meshes);
        }
    }
    Ok(BasicModel { meshes, materials })
}

pub fn try_load_recent_scene() -> Result<Option<BasicModel>> {
    let recent: String = match fs::read_to_string("most_recent_bcres_file.txt") {
        Ok(value) => value,
        Err(error) => match error.kind() {
            ErrorKind::NotFound => return Ok(None),
            _ => return Err(error.into()),
        },
    };
    let recent = recent.trim();
    
    let buf: Vec<u8> = match fs::read(recent) {
        Ok(buf) => buf,
        Err(error) => match error.kind() {
            ErrorKind::NotFound => return Ok(None),
            _ => return Err(error.into()),
        },
    };
    
    Ok(Some(load_scene(&buf)?))
}

pub fn prompt_new_scene() -> Result<Option<BasicModel>> {
    let file = FileDialog::new()
        .add_filter("3DS bcres Scene", &["bcres", "bcrez"])
        .pick_file();
    
    if let Some(file) = file {
        let content = fs::read(&file)?;
        let scene = load_scene(&content)?;
        
        // write path to disk for next time
        let path = file.as_os_str().to_str().unwrap();
        fs::write("most_recent_bcres_file.txt", path)?;
        
        Ok(Some(scene))
    } else {
        Ok(None)
    }
}

pub struct RlScene {
    pub materials: Vec<RlMaterial>,
    pub meshes: Vec<RlMesh>,
}

impl RlScene {
    pub fn new(handle: &mut RaylibHandle, thread: &RaylibThread, model: &BasicModel) -> Result<Self> {
        let mut materials: Vec<RlMaterial> = Vec::with_capacity(model.materials.len());
        for mat in &model.materials {
            let mut mat = RlMaterial::new(handle, &thread, mat)?;
            assert!(mat.material.is_material_valid());
            materials.push(mat);
        }
        
        let mut meshes: Vec<RlMesh> = model
            .meshes
            .iter()
            .map(RlMesh::new)
            .collect::<Result<Vec<RlMesh>>>()?;
        
        for mesh in &mut meshes {
            let ffimesh: &mut ffi::Mesh = mesh.as_mut();
            
            unsafe {
                ffi::UploadMesh(ffimesh as *mut ffi::Mesh, false);
            }
        }
        
        Ok(Self {
            materials,
            meshes,
        })
    }
}
