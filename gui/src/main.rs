use anyhow::{Ok, Result};
use mesh::RlMesh;
use nw_tex::bcres::model::material::FaceCulling;
use raylib::{
    camera::Camera3D,
    color::Color,
    ffi::{self, rlCullMode, rlDisableBackfaceCulling, rlEnableBackfaceCulling, rlSetCullFace, CameraMode, KeyboardKey, DEG2RAD, RL_CULL_DISTANCE_FAR},
    math::Vector3,
    prelude::{RaylibDraw, RaylibDraw3D, RaylibMode3DExt},
    RaylibHandle,
};
use scene::{prompt_new_scene, try_load_recent_scene, RlScene};

mod gfx_model;
mod material;
mod mesh;
mod scene;

const MOVEMENT_SPEED: f32 = 8.0;
const MOUSE_SPEED: f32 = 0.1;
const GLOBAL_WORLD_SCALE: f32 = 0.01;

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

fn main() -> Result<()> {
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
    
    let mut model = try_load_recent_scene()?;
    let mut scene = model.as_ref().map(|model| RlScene::new(&mut handle, &thread, &model)).transpose()?;
    
    handle.disable_cursor();
    
    while !handle.window_should_close() {
        update_cam(&mut handle, &mut cam);
        
        if model.is_none() || handle.is_key_pressed(KeyboardKey::KEY_TAB) {
            handle.enable_cursor();
            
            if let Some(new_model) = prompt_new_scene()? {
                scene = Some(RlScene::new(&mut handle, &thread, &new_model)?);
                model = Some(new_model);
            } else if model.is_none() {
                return Ok(());
            }
            
            handle.disable_cursor();
        }
        
        // setup rendering
        let mut draw = handle.begin_drawing(&thread);
        draw.clear_background(Color::GRAY);
        
        let mut mode3d = draw.begin_mode3D(cam);
        
        if let Some(scene) = &scene {
            // sort meshes
            let mut sortable_meshes: Vec<(&RlMesh, f32)> = Vec::with_capacity(scene.meshes.len());
            for mesh in &scene.meshes {
                sortable_meshes.push((mesh, -cam.position.distance_to(mesh.center_position.transform_with(mesh.bone_matrix))));
            }
            
            sortable_meshes.sort_by(|a, b| a.1.total_cmp(&b.1));
            
            // render meshes
            for (mesh, _) in sortable_meshes {
                // unsafe { rlSetCullFace(rlCullMode::RL_CULL_FACE_BACK); }
                let material = &scene.materials[mesh.material_id as usize];
                
                match material.culling {
                    FaceCulling::FrontFace => {
                        unsafe { rlEnableBackfaceCulling(); }
                        unsafe { rlSetCullFace(rlCullMode::RL_CULL_FACE_FRONT as i32); }
                    },
                    FaceCulling::BackFace => {
                        unsafe { rlEnableBackfaceCulling(); }
                        unsafe { rlSetCullFace(rlCullMode::RL_CULL_FACE_BACK as i32); }
                    },
                    FaceCulling::Always => todo!(),
                    FaceCulling::Never => {
                        unsafe { rlDisableBackfaceCulling(); }
                    },
                }
                
                mode3d.draw_mesh(mesh, material.into(), mesh.bone_matrix);
            }
        }
    }
    
    Ok(())
}
