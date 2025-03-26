use std::{os::raw::c_void, pin::Pin, ptr, slice::from_raw_parts};

use anyhow::Result;
use nw_tex::bcres::image_codec::RgbaColor;
use raylib::{ffi::{self, MaterialMapIndex, PixelFormat}, models::{Material, RaylibMaterial, WeakMaterial}, texture::Image, RaylibHandle, RaylibThread};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct BasicImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<RgbaColor>,
}

pub struct RlImage {
    pub image: Image,
    
    _image_buffer: Pin<Box<[u8]>>,
}

impl RlImage {
    pub fn new(basic_image: &BasicImage, transparent: bool) -> Self {
        let mut image_buffer: Pin<Box<[u8]>>;
        let format: PixelFormat;
        
        if transparent {
            let slice = unsafe {
                from_raw_parts(basic_image.data.as_ptr() as *const u8, basic_image.data.len() * 4)
            };
            
            image_buffer = Pin::new(slice.to_owned().into_boxed_slice());
            format = PixelFormat::PIXELFORMAT_UNCOMPRESSED_R8G8B8A8;
        } else {
            let mut image_buffer_vec = Vec::with_capacity((basic_image.width * basic_image.height * 3) as usize);
            
            for color in &basic_image.data {
                let rgb = unsafe {
                    from_raw_parts(color as *const RgbaColor as *const u8, 3)
                };
                
                image_buffer_vec.extend_from_slice(rgb);
            }
            
            image_buffer = Pin::new(image_buffer_vec.into_boxed_slice());
            format = PixelFormat::PIXELFORMAT_UNCOMPRESSED_R8G8B8;
        }
        
        let image = ffi::Image {
            data: image_buffer.as_mut_ptr() as *mut c_void,
            width: basic_image.width as i32,
            height: basic_image.height as i32,
            mipmaps: 1,
            format: format as i32,
        };
        
        Self {
            image: unsafe { Image::from_raw(image) },
            _image_buffer: image_buffer,
        }
    }
}

impl Drop for RlImage {
    fn drop(&mut self) {
        // remove my own buffers from the ffi::Image
        // or else raylib will try to free them itself
        let raw_image: &mut ffi::Image = &mut self.image;
        raw_image.data = ptr::null_mut();
    }
}

impl AsRef<Image> for RlImage {
    fn as_ref(&self) -> &Image {
        &self.image
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BasicMaterial {
    pub diffuse_texture: Option<BasicImage>,
    pub is_transparent: bool,
}

impl BasicMaterial {
    pub fn new(diffuse_texture: Option<BasicImage>, is_transparent: bool) -> Self {
        Self {
            diffuse_texture,
            is_transparent,
        }
    }
}

pub struct RlMaterial {
    pub material: Material,
}

impl RlMaterial {
    pub fn new(handle: &mut RaylibHandle, thread: &RaylibThread, basic_mat: &BasicMaterial) -> Result<Self> {
        let mut material = unsafe { Material::from_raw(ffi::LoadMaterialDefault()) };
        
        if let Some(diffuse_texture) = basic_mat.diffuse_texture.as_ref() {
            let image = RlImage::new(diffuse_texture, basic_mat.is_transparent);
            let texture = handle.load_texture_from_image(&thread, image.as_ref())?;
            
            material.set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, texture);
        }
        
        Ok(Self {
            material,
        })
    }
}

impl AsRef<ffi::Material> for RlMaterial {
    fn as_ref(&self) -> &ffi::Material {
        &self.material
    }
}
