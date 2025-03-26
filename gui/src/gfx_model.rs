use std::{collections::HashMap, io::{Cursor, Seek, SeekFrom}, mem::transmute, ops::Deref};

use anyhow::{anyhow, Result};
use binrw::BinRead;
use nw_tex::{bcres::{bcres::CgfxContainer, image_codec::decode_swizzled_buffer, material::TextureMapper, model::{AttributeName, CgfxModelCommon, Face, FaceDescriptor, GlDataType, SubMesh, VertexBuffer, VertexBufferAttribute}, texture::{CgfxTexture, CgfxTextureCommon, ImageData}}, util::math::Vec3};
use raylib::math::Vector3;

use crate::{material::{BasicImage, BasicMaterial}, mesh::BasicMesh, BasicModel};

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
    
    for (i, node) in gfx_materials.iter().enumerate() {
        if let Some(material) = &node.value {
            let mut texture_mapper: Option<&TextureMapper> = None;
            
            for mapper in &material.texture_mappers {
                if let Some(mapper) = mapper.as_ref() {
                    texture_mapper = Some(mapper);
                    break;
                }
            }
            
            let image = if let Some(texture_mapper) = texture_mapper {
                let texture_path = texture_mapper.texture.as_ref().unwrap().path.as_deref().unwrap();
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
    let meshes = common.meshes.as_deref().unwrap();
    let shapes = common.shapes.as_deref().unwrap();
    let mut out_meshes: Vec<BasicMesh> = Vec::new();
    
    for mesh in meshes {
        let shape = shapes.get(mesh.shape_index as usize)
            .ok_or_else(|| anyhow!("Invalid shape index {}", mesh.shape_index))?;
        
        let vertex_buffers = shape.vertex_buffers.as_ref().unwrap();
        let mut current_vertices: Vec<Vector3> = Vec::new();
        let mut current_faces: Vec<[u16; 3]> = Vec::new();
        
        // collect all vertices
        for vb in vertex_buffers {
            match vb {
                VertexBuffer::Attribute(attribute) => {
                    if attribute.vertex_buffer_common.attribute_name == AttributeName::Position {
                        assert!(attribute.format == GlDataType::Float);
                        let raw_bytes: &[u8] = attribute.raw_bytes.as_ref().unwrap();
                        let mut reader = Cursor::new(raw_bytes);
                        
                        for _ in 0..raw_bytes.len() / attribute.elements as usize {
                            let pos: Vector3 = vec3_to_rl(Vec3::read(&mut reader)?) * attribute.scale * global_scale;
                            
                            current_vertices.push(pos);
                        }
                        
                        todo!();
                    }
                },
                VertexBuffer::Interleaved(interleaved) => {
                    let attributes: &[VertexBufferAttribute] = interleaved.attributes.as_ref().unwrap();
                    
                    // check if this vb contains a position attribute
                    if attributes.iter().all(|attr| attr.attribute_name != AttributeName::Position) {
                        continue;
                    }
                    
                    let raw_bytes: &[u8] = interleaved.raw_bytes.as_ref().unwrap();
                    let mut reader = Cursor::new(raw_bytes);
                    
                    let vertex_byte_size: u32 = attributes.iter()
                        .map(|attr| attr.format.byte_size() * attr.elements)
                        .sum();
                    let vertex_count = raw_bytes.len() / vertex_byte_size as usize;
                    
                    for _ in 0..vertex_count {
                        for attr in attributes {
                            if attr.attribute_name == AttributeName::Position {
                                assert!(attr.elements == 3 && attr.format == GlDataType::Float);
                                let pos: Vector3 = vec3_to_rl(Vec3::read(&mut reader)?) * attr.scale * global_scale;
                                
                                current_vertices.push(pos);
                            } else {
                                reader.seek(SeekFrom::Current((attr.format.byte_size() * attr.elements) as i64))?;
                            }
                        }
                    }
                },
                // it doesn't make sense for Position to be fixed so this is just ignored
                VertexBuffer::Fixed(_) => (),
            }
        }
        
        // collect all faces
        let sub_meshes: &[SubMesh] = shape.sub_meshes.as_ref().unwrap();
        
        for sub_mesh in sub_meshes {
            let gfx_faces: &[Face] = sub_mesh.faces.as_ref().unwrap();
            
            for gfx_face in gfx_faces {
                let face_descriptors: &[FaceDescriptor] = gfx_face.face_descriptors.as_ref().unwrap();
                
                for face_descriptor in face_descriptors {
                    let indices: &[u16] = face_descriptor.indices.as_ref().unwrap();
                    assert!(indices.len() % 3 == 0);
                    
                    let mut reader = indices.iter();
                    
                    for _ in 0..indices.len() / 3 {
                        let a_index = *reader.next().unwrap();
                        let b_index = *reader.next().unwrap();
                        let c_index = *reader.next().unwrap();
                        
                        current_faces.push([a_index, b_index, c_index]);
                    }
                }
            }
        }
        
        out_meshes.push(BasicMesh::new(current_vertices, current_faces, mesh.material_index + start_material_id));
    }
    
    Ok(BasicModel {
        meshes: out_meshes,
        materials: out_materials,
    })
}
