use std::fs::{self};

use anyhow::Result;
use gfx_model::load_bcres_model;
use mesh::{BasicMesh, RlMesh};
use nw_tex::bcres::bcres::CgfxContainer;
use raylib::{
    camera::Camera3D, color::Color, ffi::{self, CameraMode, KeyboardKey, DEG2RAD}, math::{Matrix, Vector3}, models::{Material, WeakMaterial}, prelude::{RaylibDraw, RaylibDraw3D, RaylibMode3DExt}, RaylibHandle
};

mod gfx_model;
mod mesh;

const MOVEMENT_SPEED: f32 = 8.0;
const MOUSE_SPEED: f32 = 0.1;

fn init_bcres_model() -> Result<Vec<BasicMesh>> {
    let buf = fs::read("testing/hei_5_00.bcres")?;
    let container = CgfxContainer::new(&buf)?;
    
    if container.models.is_none() {
        return Ok(Vec::new())
    }
    
    let mut result: Vec<BasicMesh> = Vec::new();
    
    for node in container.models.unwrap().nodes {
        if let Some(model) = node.value {
            result.extend(load_bcres_model(model.common(), 0.01)?);
        }
    }
    
    Ok(result)
}

fn update_cam(handle: &mut RaylibHandle, cam: &mut Camera3D) {
    handle.update_camera(cam, CameraMode::CAMERA_CUSTOM);
    
    let delta = handle.get_frame_time();
    
    let mouse_2d = handle.get_mouse_delta();
    let mouse = Vector3::new(mouse_2d.x, mouse_2d.y, 0.0) * MOUSE_SPEED;
    
    let left = handle.is_key_down(KeyboardKey::KEY_A) as u32 as f32;
    let right = handle.is_key_down(KeyboardKey::KEY_D) as u32 as f32;
    let north = handle.is_key_down(KeyboardKey::KEY_W) as u32 as f32;
    let south = handle.is_key_down(KeyboardKey::KEY_S) as u32 as f32;
    let up = handle.is_key_down(KeyboardKey::KEY_E) as u32 as f32;
    let down = handle.is_key_down(KeyboardKey::KEY_Q) as u32 as f32;
    
    let mut fficam: ffi::Camera3D = (*cam).into();
    
    unsafe {
        ffi::CameraYaw(&mut fficam, -mouse.x * (DEG2RAD as f32), false);
        ffi::CameraPitch(&mut fficam, -mouse.y * (DEG2RAD as f32), true, false, false);
        
        ffi::CameraMoveForward(&mut fficam, (north - south) * delta * MOVEMENT_SPEED, false);
        ffi::CameraMoveUp(&mut fficam, (up - down) * delta * MOVEMENT_SPEED);
        ffi::CameraMoveRight(&mut fficam, (right - left) * delta * MOVEMENT_SPEED, false);
    }
    
    *cam = fficam.into();
}

fn main() -> Result<()>{
    let (mut handle, thread) = raylib::init()
        .size(1280, 720)
        .resizable()
        .title("Sticker Star Scene Test")
        .build();
    
    let mut cam = Camera3D::perspective(
        Vector3::new(0.0, 2.0, 4.0),
        Vector3::new(0.0, 2.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        60.0,
    );
    
    let basic_meshes = init_bcres_model()?;
    let mut meshes: Vec<RlMesh> = basic_meshes.iter().map(RlMesh::new).collect::<Result<Vec<RlMesh>>>()?;
    let material: WeakMaterial = handle.load_material_default(&thread);
    
    for mesh in &mut meshes {
        let ffimesh: &mut ffi::Mesh = mesh.as_mut();
        
        unsafe {
            ffi::UploadMesh(ffimesh as *mut ffi::Mesh, false);
        }
    }
    
    handle.disable_cursor();
    
    while !handle.window_should_close() {
        update_cam(&mut handle, &mut cam);
        
        let mut draw = handle.begin_drawing(&thread);
        draw.clear_background(Color::GRAY);
        
        let mut mode3d = draw.begin_mode3D(cam);
        
        mode3d.draw_cube(Vector3::new(0.0, 0.1, 0.0), 1.0, 1.0, 1.0, Color::WHITE);
        
        for mesh in &meshes {
            mode3d.draw_mesh(mesh, material.clone(), Matrix::identity());
        }
    }
    
    Ok(())
}
