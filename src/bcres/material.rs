use std::io::{Cursor, Seek, SeekFrom};

use anyhow::{bail, Result};
use array_init::try_array_init;
use binrw::{BinRead, BinWrite};
use byteorder::{LittleEndian, ReadBytesExt};
use na::Matrix3x4;

use crate::{
    scoped_reader_pos,
    util::{
        math::{SerializableMatrix, Vec2, Vec4},
        pointer::Pointer,
    },
};

use super::{
    bcres::{CgfxCollectionValue, WriteContext},
    image_codec::RgbaColor,
    util::{brw_read_string, brw_relative_pointer, brw_write_zero, CgfxBox, CgfxObjectHeader},
};

#[derive(Clone, Debug, PartialEq)]
pub struct CgfxMaterial {
    // object header
    pub cgfx_object_header: CgfxObjectHeader,
    
    // material stuff
    pub flags: u32,
    pub tex_coord_config: u32,
    pub render_layer: u32,
    pub colors: MaterialColors,
    pub rasterization: Rasterization,
    pub fragment_operation: FragmentOp,
    
    pub used_texture_coords_count: u32,
    pub texture_coords: [TextureCoord; 3],
    pub texture_mappers: [Option<TextureMapper>; 3],
}

impl CgfxCollectionValue for CgfxMaterial {
    fn read_dict_value(reader: &mut Cursor<&[u8]>) -> Result<Self> {
        let magic = reader.read_u32::<LittleEndian>()?;
        if magic != 0x8000000 {
            bail!("Incorrect magic number, expected 0x8000000 for Material but got 0x{magic:x}")
        }
        
        let cgfx_object_header = CgfxObjectHeader::read(reader)?;
        let flags = reader.read_u32::<LittleEndian>()?;
        let tex_coord_config = reader.read_u32::<LittleEndian>()?;
        let render_layer = reader.read_u32::<LittleEndian>()?;
        let colors = MaterialColors::read(reader)?;
        let rasterization = Rasterization::read(reader)?;
        let fragment_operation = FragmentOp::read(reader)?;
        let used_texture_coords_count = reader.read_u32::<LittleEndian>()?;
        
        let texture_coords: [TextureCoord; 3] = try_array_init(|_| TextureCoord::read(reader))?;
        
        let texture_mapper_ptrs: [Option<Pointer>; 3] =
            try_array_init(|_| Pointer::read_relative(reader))?;
        let mut texture_mappers: [Option<TextureMapper>; 3] = Default::default();
        
        for (i, ptr) in texture_mapper_ptrs.iter().enumerate() {
            if let Some(ptr) = *ptr {
                scoped_reader_pos!(reader);
                reader.seek(SeekFrom::Start(ptr.into()))?;
                
                texture_mappers[i] = Some(TextureMapper::read(reader)?);
            }
        }
        
        Ok(Self {
            cgfx_object_header,
            flags,
            tex_coord_config,
            render_layer,
            colors,
            rasterization,
            fragment_operation,
            used_texture_coords_count,
            texture_coords,
            texture_mappers,
        })
    }

    fn write_dict_value(&self, _writer: &mut Cursor<&mut Vec<u8>>, _ctx: &mut WriteContext) -> Result<()> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, BinRead, BinWrite)]
#[brw(little)]
pub struct MaterialColors {
    pub emission_float: Vec4,
    pub ambient_float: Vec4,
    pub diffuse_float: Vec4,
    pub specular0_float: Vec4,
    pub specular1_float: Vec4,
    pub constant0_float: Vec4,
    pub constant1_float: Vec4,
    pub constant2_float: Vec4,
    pub constant3_float: Vec4,
    pub constant4_float: Vec4,
    pub constant5_float: Vec4,
    
    pub emission: RgbaColor,
    pub ambient: RgbaColor,
    pub diffuse: RgbaColor,
    pub specular0: RgbaColor,
    pub specular1: RgbaColor,
    pub constant0: RgbaColor,
    pub constant1: RgbaColor,
    pub constant2: RgbaColor,
    pub constant3: RgbaColor,
    pub constant4: RgbaColor,
    pub constant5: RgbaColor,
    
    pub command_cache: u32,
}

#[derive(Clone, Debug, PartialEq, BinRead, BinWrite)]
#[brw(little)]
pub struct Rasterization {
    pub is_polygon_offset_enabled: u32,
    pub face_culling: u32,
    pub polygon_offset_unit: f32,
    
    pub face_culling_command: [u32; 2],
}

#[derive(Clone, Debug, PartialEq, BinRead, BinWrite)]
#[brw(little)]
pub struct FragmentOp {
    pub depth_flags: u32,
    pub depth_commands: [u32; 4],
    
    pub blend_mode: u32,
    pub blend_color: Vec4,
    pub blend_commands: [u32; 6],
    
    pub stencil_commands: [u32; 4],
}

#[derive(Clone, Debug, PartialEq, BinRead, BinWrite)]
#[brw(little)]
pub struct TextureCoord {
    pub source_coord_index: u32,
    pub mapping_type: u32,
    pub reference_camera_index: u32,
    pub transform_type: u32,
    
    pub scale: Vec2,
    pub rotation: f32,
    pub translation: Vec2,
    
    pub flags: u32,
    #[brw(repr = SerializableMatrix<3, 4>)]
    pub transform: Matrix3x4<f32>,
}

#[derive(Clone, Debug, PartialEq, BinRead, BinWrite)]
#[brw(little, magic = 0x80000000u32)]
pub struct TextureMapper {
    pub dynamic_alloc: u32,
    
    #[brw(repr = CgfxBox<TextureReference>)]
    pub texture: Option<TextureReference>,
    
    #[brw(repr = CgfxBox<TextureSampler>)]
    pub sampler: Option<TextureSampler>,
    
    pub commands: [u32; 14],
    pub commands_len: u32,
}

#[derive(Clone, Debug, PartialEq, BinRead, BinWrite)]
#[brw(little, magic = 0x20000004u32)]
pub struct TextureReference {
    pub cgfx_object_header: CgfxObjectHeader,
    
    #[br(parse_with = brw_read_string)]
    #[bw(write_with = brw_write_zero)]
    pub path: Option<String>,
    pub texture_ptr: u32,
}

#[derive(Clone, Debug, PartialEq, BinRead, BinWrite)]
#[brw(little, magic = 0x80000000u32)]
pub struct TextureSampler {
    #[br(parse_with = brw_relative_pointer)]
    #[bw(map = |_| 0u32)]
    pub parent_mapper: Option<Pointer>,
    pub min_filter: u32,
}
