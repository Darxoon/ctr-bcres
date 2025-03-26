use std::{io::{Cursor, Seek, SeekFrom}, mem::transmute};

use anyhow::Result;
use binrw::BinRead;
use nw_tex::{bcres::model::{AttributeName, CgfxModelCommon, Face, FaceDescriptor, GlDataType, SubMesh, VertexBuffer, VertexBufferAttribute}, util::math::Vec3};
use raylib::math::Vector3;

use crate::mesh::BasicMesh;

fn vec3_to_rl(vector: Vec3) -> Vector3 {
    // Vec3 and Vector3 have exactly the same layouts
    unsafe { transmute(vector) }
}

pub fn load_bcres_model(common: &CgfxModelCommon, global_scale: f32) -> Result<Vec<BasicMesh>> {
    let shapes = common.shapes.as_ref().unwrap();
    let mut meshes: Vec<BasicMesh> = Vec::new();
    
    for shape in shapes {
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
        
        meshes.push(BasicMesh::new(current_vertices, current_faces));
    }
    
    Ok(meshes)
}
