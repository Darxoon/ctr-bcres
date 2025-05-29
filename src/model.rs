use std::io::{Cursor, Read, Seek, SeekFrom};

use anyhow::{anyhow, Result};
use binrw::BinRead;
use byteorder::{LittleEndian, ReadBytesExt};
use material::CgfxMaterial;
use mesh::{Mesh, Shape};
use skeleton::CgfxSkeleton;

use crate::{
    scoped_reader_pos,
    util::{
        pointer::Pointer,
        util::{read_pointer_list, CgfxNodeHeader, CgfxObjectHeader, CgfxTransform},
    },
    CgfxCollectionValue, CgfxDict, WriteContext,
};

pub mod material;
pub mod mesh;
pub mod skeleton;

#[derive(Debug, Clone, PartialEq)]
pub struct CgfxModelCommon {
    // header stuff
    pub cgfx_object_header: CgfxObjectHeader,
    pub cgfx_node_header: CgfxNodeHeader,
    pub transform_node_header: CgfxTransform,
    
    // model data
    pub meshes: Vec<Mesh>,
    pub materials: Option<CgfxDict<CgfxMaterial>>,
    pub shapes: Vec<Shape>,
    pub mesh_node_visibilities: Option<CgfxDict<()>>, // TODO: implement
    
    pub flags: u32,
    pub face_culling: u32,
    pub layer_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CgfxModel {
    Standard(CgfxModelCommon),
    Skeletal(CgfxModelCommon, CgfxSkeleton),
}

impl CgfxModel {
    pub fn from_reader<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let discriminant = reader.read_u32::<LittleEndian>()?;
        let cgfx_object_header = CgfxObjectHeader::read(reader)?;
        let cgfx_node_header = CgfxNodeHeader::read(reader)?;
        let transform_node_header = CgfxTransform::read(reader)?;
        
        // TODO: anim groups in node header
        
        // meshes
        let meshes: Vec<Mesh> = read_pointer_list(reader)?;
        
        // materials
        let material_count = reader.read_u32::<LittleEndian>()?;
        let material_ptr = Pointer::read_relative(reader)?;
        
        let materials = if let Some(material_ptr) = material_ptr {
            scoped_reader_pos!(reader);
            reader.seek(SeekFrom::Start(material_ptr.into()))?;
            let dict: CgfxDict<CgfxMaterial> = CgfxDict::from_reader(reader)?;
            
            assert!(dict.values_count == material_count);
            Some(dict)
        } else {
            None
        };
        
        // shapes
        let shapes: Vec<Shape> = read_pointer_list(reader)?;
        
        // mesh node visibilities
        let mesh_node_visibility_count = reader.read_u32::<LittleEndian>()?;
        let mesh_node_visibility_ptr = Pointer::read_relative(reader)?;
        
        let mesh_node_visibilities = if let Some(mesh_node_visibility_ptr) = mesh_node_visibility_ptr {
            scoped_reader_pos!(reader);
            reader.seek(SeekFrom::Start(mesh_node_visibility_ptr.into()))?;
            let dict: CgfxDict<()> = CgfxDict::from_reader(reader)?;
            
            assert!(dict.values_count == mesh_node_visibility_count);
            Some(dict)
        } else {
            None
        };
        
        let flags = reader.read_u32::<LittleEndian>()?;
        let face_culling = reader.read_u32::<LittleEndian>()?;
        let layer_id = reader.read_u32::<LittleEndian>()?;
        
        let common = CgfxModelCommon {
            cgfx_object_header,
            cgfx_node_header,
            transform_node_header,
            meshes,
            materials,
            shapes,
            mesh_node_visibilities,
            flags,
            face_culling,
            layer_id,
        };
        
        let model = match discriminant {
            0x40000012 => CgfxModel::Standard(common),
            0x40000092 => {
                let skeleton_ptr = Pointer::read_relative(reader)?
                    .ok_or_else(|| anyhow!("Skeleton can not be null"))?;
                
                scoped_reader_pos!(reader);
                reader.seek(SeekFrom::Start(skeleton_ptr.into()))?;
                
                let skeleton = CgfxSkeleton::from_reader(reader)?;
                
                CgfxModel::Skeletal(common, skeleton)
            },
            _ => return Err(anyhow!("Invalid model type discriminant {:x}", discriminant)),
        };
        
        Ok(model)
    }

    pub fn common(&self) -> &CgfxModelCommon {
        match self {
            CgfxModel::Standard(common) => common,
            CgfxModel::Skeletal(common, _) => common,
        }
    }

    pub fn common_mut(&mut self) -> &mut CgfxModelCommon {
        match self {
            CgfxModel::Standard(common) => common,
            CgfxModel::Skeletal(common, _) => common,
        }
    }
}

impl CgfxCollectionValue for CgfxModel {
    fn read_dict_value<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        Self::from_reader(reader)
    }

    fn write_dict_value(&self, _writer: &mut Cursor<&mut Vec<u8>>, _ctx: &mut WriteContext) -> Result<()> {
        todo!()
    }
}
