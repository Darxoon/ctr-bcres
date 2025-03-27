use std::{pin::Pin, ptr, slice::from_raw_parts};

use anyhow::Result;
use raylib::{
    ffi,
    math::{Vector2, Vector3},
    models,
};

#[derive(Clone, Debug, PartialEq)]
pub struct BasicMesh {
    pub vertex_positions: Vec<Vector3>,
    pub vertex_uvs: Vec<Vector2>,
    pub faces: Vec<[u16; 3]>,
    
    pub material_id: u32,
}

pub struct RlMesh {
    pub mesh: models::Mesh,
    pub material_id: u32,
    
    // are pointed to by the Mesh
    _vertex_buffer: Pin<Box<[f32]>>,
    _vertex_uvs: Option<Pin<Box<[f32]>>>,
    _index_buffer: Pin<Box<[u16]>>,
}

impl RlMesh {
    pub fn new(basic_mesh: &BasicMesh) -> Result<Self> {
        let mut vertices = Pin::new(
            unsafe {
                from_raw_parts(
                    basic_mesh.vertex_positions.as_ptr() as *const f32,
                    basic_mesh.vertex_positions.len() * 3,
                )
            }
            .to_owned()
            .into_boxed_slice(),
        );
        let mut vertex_uvs = if basic_mesh.vertex_uvs.len() != 0 {
            Some(Pin::new(
                unsafe {
                    from_raw_parts(
                        basic_mesh.vertex_uvs.as_ptr() as *const f32,
                        basic_mesh.vertex_uvs.len() * 2,
                    )
                }
                .to_owned()
                .into_boxed_slice(),
            ))
        } else {
            None
        };
        let mut indices = Pin::new(
            unsafe {
                from_raw_parts(
                    basic_mesh.faces.as_ptr() as *const u16,
                    basic_mesh.faces.len() * 3,
                )
            }
            .to_owned()
            .into_boxed_slice(),
        );
        let mesh = ffi::Mesh {
            vertexCount: basic_mesh.vertex_positions.len().try_into()?,
            vertices: vertices.as_mut_ptr(),
            
            triangleCount: basic_mesh.faces.len().try_into()?,
            indices: indices.as_mut_ptr(),
            
            texcoords: if let Some(vertex_uvs) = &mut vertex_uvs {
                vertex_uvs.as_mut_ptr()
            } else {
                ptr::null_mut()
            },
            
            texcoords2: ptr::null_mut(),
            normals: ptr::null_mut(),
            tangents: ptr::null_mut(),
            colors: ptr::null_mut(),
            animVertices: ptr::null_mut(),
            animNormals: ptr::null_mut(),
            boneIds: ptr::null_mut(),
            boneWeights: ptr::null_mut(),
            boneMatrices: ptr::null_mut(),
            boneCount: 0,
            vaoId: 0,
            vboId: ptr::null_mut(),
        };
        
        Ok(Self {
            mesh: unsafe { models::Mesh::from_raw(mesh) },
            material_id: basic_mesh.material_id,
            _vertex_buffer: vertices,
            _vertex_uvs: vertex_uvs,
            _index_buffer: indices,
        })
    }
}

impl Drop for RlMesh {
    fn drop(&mut self) {
        // remove my own buffers from the ffi::Mesh
        // or else raylib will try to free them itself
        self.mesh.vertices = ptr::null_mut();
        self.mesh.indices = ptr::null_mut();
        self.mesh.texcoords = ptr::null_mut();
    }
}

impl AsRef<ffi::Mesh> for RlMesh {
    fn as_ref(&self) -> &ffi::Mesh {
        self.mesh.as_ref()
    }
}

impl AsMut<ffi::Mesh> for RlMesh {
    fn as_mut(&mut self) -> &mut ffi::Mesh {
        self.mesh.as_mut()
    }
}
