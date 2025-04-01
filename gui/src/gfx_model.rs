use std::{
    collections::HashMap,
    io::{Cursor, Seek, SeekFrom},
    mem::transmute,
    ops::Deref,
};

use anyhow::{anyhow, Result};
use binrw::BinRead;
use nw_tex::{
    bcres::{
        bcres::CgfxContainer,
        image_codec::{decode_swizzled_buffer, RgbaColor},
        material::TextureMapper,
        model::{
            AttributeName, CgfxModelCommon, GlDataType, VertexBuffer,
        },
        texture::{CgfxTexture, CgfxTextureCommon, ImageData},
    },
    util::math::{Vec2, Vec3},
};
use raylib::math::{Vector2, Vector3};

use crate::{
    material::{BasicImage, BasicMaterial},
    mesh::BasicMesh,
    BasicModel,
};

fn vec2_to_rl(vector: Vec2) -> Vector2 {
    // Vec3 and Vector3 have exactly the same layouts
    unsafe { transmute(vector) }
}

fn vec3_to_rl(vector: Vec3) -> Vector3 {
    // Vec3 and Vector3 have exactly the same layouts
    unsafe { transmute(vector) }
}

fn cgfx_image_to_basic_image(common: &CgfxTextureCommon, image_data: &ImageData) -> Result<BasicImage> {
    let CgfxTextureCommon { texture_format, width, height, .. } = *common;
    let decoded = decode_swizzled_buffer(&image_data.image_bytes, texture_format, width, height)?;
    
    Ok(BasicImage {
        width,
        height,
        data: decoded,
    })
}

pub fn load_bcres_textures(container: &CgfxContainer) -> Result<HashMap<String, BasicImage>> {
    if container.textures.is_none() {
        return Ok(HashMap::new())
    }
    
    // this code is rough
    let textures = container.textures.as_ref().unwrap();
    let mut result = HashMap::new();
    
    for node in &textures.nodes {
        if let Some(texture) = &node.value {
            let metadata = texture.metadata();
            let image_data = match texture {
                CgfxTexture::Image(_, image_data) => image_data.as_ref().unwrap(),
                _ => panic!(),
            };
            
            result.insert(
                node.name.as_ref().unwrap().clone(),
                cgfx_image_to_basic_image(metadata, image_data)?
            );
        }
    }
    
    Ok(result)
}

pub fn load_bcres_model(common: &CgfxModelCommon, textures: &HashMap<String, BasicImage>,
            global_scale: f32, start_material_id: u32) -> Result<BasicModel> {
    
    // materials
    let gfx_materials = common.materials.as_ref().unwrap().nodes.deref();
    let mut out_materials: Vec<BasicMaterial> = Vec::new();
    
    for node in gfx_materials {
        if let Some(material) = &node.value {
            assert!(material.render_layer == 0);
            
            let mut texture_mapper: Option<&TextureMapper> = None;
            
            for mapper in &material.texture_mappers {
                if let Some(mapper) = &mapper {
                    texture_mapper = Some(mapper);
                    break;
                }
            }
            
            let image = if let Some(texture_mapper) = texture_mapper {
                let texture_path = texture_mapper
                    .texture
                    .as_ref()
                    .unwrap()
                    .path
                    .as_deref()
                    .unwrap();
                Some(textures.get(texture_path).unwrap().clone())
            } else {
                None
            };
            
            out_materials.push(BasicMaterial {
                diffuse_texture: image,
                is_transparent: true, // TODO: figure this out better
            });
        }
    }
    
    // meshes
    let mut out_meshes: Vec<BasicMesh> = Vec::new();
    
    for mesh in &common.meshes {
        assert!(mesh.render_priority == 0);
        
        let shape = common.shapes.get(mesh.shape_index as usize)
            .ok_or_else(|| anyhow!("Invalid shape index {}", mesh.shape_index))?;
        
        let mut vertex_positions: Vec<Vector3> = Vec::new();
        let mut vertex_uvs: Vec<Vector2> = Vec::new();
        let mut vertex_colors: Vec<RgbaColor> = Vec::new();
        let mut faces: Vec<[u16; 3]> = Vec::new();
        
        // collect all vertices
        for vb in &shape.vertex_buffers {
            match vb {
                VertexBuffer::Attribute(attribute) => {
                    if attribute.vertex_buffer_common.attribute_name == AttributeName::Position {
                        assert!(attribute.format == GlDataType::Float);
                        let mut reader: Cursor<&[u8]> = Cursor::new(&attribute.raw_bytes);
                        
                        for _ in 0..attribute.raw_bytes.len() / attribute.elements as usize {
                            let pos: Vector3 = vec3_to_rl(Vec3::read(&mut reader)?) * attribute.scale * global_scale;
                            
                            vertex_positions.push(pos);
                        }
                        
                        todo!();
                    } else if attribute.vertex_buffer_common.attribute_name == AttributeName::TexCoord0 {
                        todo!()
                    }
                },
                VertexBuffer::Interleaved(interleaved) => {
                    // check if this vb contains a position attribute
                    let contains_position_attr = interleaved.attributes
                        .iter()
                        .any(|attr| attr.attribute_name == AttributeName::Position);
                    
                    if !contains_position_attr {
                        continue;
                    }
                    
                    // parse raw bytes
                    let mut reader: Cursor<&[u8]> = Cursor::new(&interleaved.raw_bytes);
                    
                    let vertex_byte_size: u32 = interleaved.attributes.iter()
                        .map(|attr| attr.format.byte_size() * attr.elements)
                        .sum();
                    let vertex_count = interleaved.raw_bytes.len() / vertex_byte_size as usize;
                    
                    for _ in 0..vertex_count {
                        for attr in &interleaved.attributes {
                            match attr.attribute_name {
                                AttributeName::Position => {
                                    assert!(attr.elements == 3 && attr.format == GlDataType::Float);
                                    let pos: Vector3 = vec3_to_rl(Vec3::read(&mut reader)?)
                                        * attr.scale
                                        * global_scale;
                                    vertex_positions.push(pos);
                                },
                                AttributeName::TexCoord0 => {
                                    assert!(attr.elements == 2 && attr.format == GlDataType::Float);
                                    let mut uv: Vector2 = vec2_to_rl(Vec2::read(&mut reader)?) * attr.scale;
                                    uv.y *= -1.0;
                                    
                                    vertex_uvs.push(uv);
                                },
                                AttributeName::Color => {
                                    assert!(attr.elements == 4 && attr.format == GlDataType::UByte);
                                    let color = RgbaColor::read(&mut reader)?;
                                    
                                    vertex_colors.push(color);
                                }
                                _ => {
                                    reader.seek(SeekFrom::Current((attr.format.byte_size() * attr.elements) as i64))?;
                                },
                            }
                        }
                    }
                },
                // it doesn't make sense for Position to be fixed so this is just ignored
                VertexBuffer::Fixed(_) => {
                    // TODO: vertex colors might be in here
                },
            }
        }
        
        // collect all faces
        for sub_mesh in &shape.sub_meshes {
            for gfx_face in &sub_mesh.faces {
                for face_descriptor in &gfx_face.face_descriptors {
                    let indices: &[u16] = &face_descriptor.indices;
                    assert!(indices.len() % 3 == 0);
                    
                    let mut reader = indices.iter();
                    
                    for _ in 0..indices.len() / 3 {
                        let a_index = *reader.next().unwrap();
                        let b_index = *reader.next().unwrap();
                        let c_index = *reader.next().unwrap();
                        
                        faces.push([a_index, b_index, c_index]);
                    }
                }
            }
        }
        
        out_meshes.push(BasicMesh {
            vertex_positions,
            vertex_uvs,
            vertex_colors,
            faces,
            
            center: vec3_to_rl(shape.bounding_box.as_ref().unwrap().center),
            material_id: mesh.material_index + start_material_id,
        });
    }
    
    Ok(BasicModel {
        meshes: out_meshes,
        materials: out_materials,
    })
}
