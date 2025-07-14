use std::io::{Read, Seek, SeekFrom, Write};

use anyhow::{anyhow, bail, ensure, Result};
use binrw::{BinRead, BinWrite};
use byteorder::{LittleEndian, ReadBytesExt};
use na::Matrix3x4;

use crate::{
    scoped_reader_pos,
    util::{
        math::{SerializableMatrix, Vec3},
        pointer::Pointer,
        util::{brw_read_string, brw_relative_pointer, brw_write_zero, CgfxObjectHeader},
    },
    CgfxDict, WriteContext,
};

#[derive(Clone, Debug, PartialEq)]
pub struct CgfxSkeleton {
    pub cgfx_object_header: CgfxObjectHeader,
    
    pub bones: CgfxDict<CgfxBone>,
    pub root_bone: Pointer,
    pub scaling_rule: SkeletonScalingRule,
    pub flags: u32,
}

impl CgfxSkeleton {
    pub fn from_reader<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let magic = reader.read_u32::<LittleEndian>()?;
        assert!(magic == 0x02000000u32, "Expected magic number 0x02000000, got 0x{magic:x}");
        
        let cgfx_object_header = CgfxObjectHeader::read(reader)?;
        
        let bone_count = reader.read_u32::<LittleEndian>()?;
        let bone_ptr = Pointer::read_relative(reader)?;
        
        let bones = if let Some(bone_ptr) = bone_ptr {
            scoped_reader_pos!(reader);
            reader.seek(SeekFrom::Start(bone_ptr.into()))?;
            let dict: CgfxDict<CgfxBone> = CgfxDict::from_reader(reader)?;
            
            ensure!(dict.values_count == bone_count);
            dict
        } else {
            bail!("Cgfx Skeleton is missing a bone dictionary");
        };
        
        let root_bone = Pointer::read_relative(reader)?
            .ok_or_else(|| anyhow!("Cgfx Skeleton is missing a root bone"))?;
        
        let scaling_rule = SkeletonScalingRule::read(reader)?;
        let flags = reader.read_u32::<LittleEndian>()?;
        
        Ok(Self {
            cgfx_object_header,
            bones,
            root_bone,
            scaling_rule,
            flags,
        })
    }
    
    pub fn to_writer<W: Write + Seek>(&self, _writer: &mut W, _ctx: &mut WriteContext) -> Result<()> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, BinRead, BinWrite)]
#[brw(little, repr = u32)]
pub enum SkeletonScalingRule {
    Standard,
    Maya,
    SoftImage, // rip
}

#[derive(Clone, Debug, BinRead, BinWrite, PartialEq)]
#[brw(little)]
pub struct CgfxBone {
    #[br(parse_with = brw_read_string)]
    #[bw(write_with = brw_write_zero)]
    pub name: Option<String>,
    
    pub flags: u32,
    pub index: u32,
    pub parent_index: u32,
    
    // TODO
    #[br(parse_with = brw_relative_pointer)]
    #[bw(map = |_| 0u32)]
    pub parent_ptr: Option<Pointer>,
    #[br(parse_with = brw_relative_pointer)]
    #[bw(map = |_| 0u32)]
    child_ptr: Option<Pointer>,
    #[br(parse_with = brw_relative_pointer)]
    #[bw(map = |_| 0u32)]
    prev_sibling_ptr: Option<Pointer>,
    #[br(parse_with = brw_relative_pointer)]
    #[bw(map = |_| 0u32)]
    next_sibling_ptr: Option<Pointer>,
    
    pub scale: Vec3,
    pub rotation: Vec3,
    pub translation: Vec3,
    
    #[brw(repr = SerializableMatrix<3, 4>)]
    pub local_transform: Matrix3x4<f32>,
    #[brw(repr = SerializableMatrix<3, 4>)]
    pub world_transform: Matrix3x4<f32>,
    #[brw(repr = SerializableMatrix<3, 4>)]
    pub inv_world_transform: Matrix3x4<f32>,
    
    pub billboard_mode: u32,
    
    #[br(parse_with = brw_relative_pointer)]
    #[bw(map = |_| 0u32)]
    pub metadata_ptr: Option<Pointer>,
}
