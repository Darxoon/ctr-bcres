use raylib::{
    camera::Camera3D,
    color::Color,
    ffi::{self, CameraMode, KeyboardKey, DEG2RAD},
    math::Vector3,
    prelude::{RaylibDraw, RaylibDraw3D, RaylibMode3DExt},
    RaylibHandle,
};

const MOVEMENT_SPEED: f32 = 8.0;
const MOUSE_SPEED: f32 = 0.1;

// main update cam function
pub fn update_cam(handle: &mut RaylibHandle, cam: &mut Camera3D) {
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

fn main() {
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
    
    handle.disable_cursor();
    
    while !handle.window_should_close() {
        update_cam(&mut handle, &mut cam);
        
        let mut draw = handle.begin_drawing(&thread);
        draw.clear_background(Color::GRAY);
        
        let mut mode3d = draw.begin_mode3D(cam);
        mode3d.draw_cube(Vector3::new(0.0, 1.0, 0.0), 1.0, 1.0, 1.0, Color::WHITE);
        mode3d.draw_cube(Vector3::new(0.0, 2.0, 0.0), 1.0, 1.0, 1.0, Color::RED);
    }
}
