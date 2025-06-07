use std::{
    io::{Cursor, Write},
    str::from_utf8,
};

use anyhow::Result;
use binrw::{BinRead, BinWrite};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::{
    assert_matching, util::pointer::Pointer, write_at_pointer, CgfxDict, CgfxNode, WriteContext,
};

use super::{model::CgfxModel, texture::CgfxTexture};

#[derive(Clone, Debug, PartialEq, Eq, Default, BinRead, BinWrite)]
#[brw(little, magic = b"CGFX")]
pub struct CgfxHeader {
    pub byte_order_mark: u16,
    pub header_length: u16,
    pub revision: u32,
    pub file_length: u32,
    pub sections_count: u32,
    
    #[br(assert(content_magic_number == 0x41544144u32,
        "Invalid magic number for data, expected 'DATA' but got '{}'",
        from_utf8(&content_magic_number.to_le_bytes()).unwrap()))]
    pub content_magic_number: u32,
    pub content_length: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CgfxContainer {
    pub header: CgfxHeader,
    
    pub models: Option<CgfxDict<CgfxModel>>,
    pub textures: Option<CgfxDict<CgfxTexture>>,
    pub luts: Option<CgfxDict<()>>,
    pub materials: Option<CgfxDict<()>>,
    pub shaders: Option<CgfxDict<()>>,
    pub cameras: Option<CgfxDict<()>>,
    pub lights: Option<CgfxDict<()>>,
    pub fogs: Option<CgfxDict<()>>,
    pub scenes: Option<CgfxDict<()>>,
    pub skeletal_animations: Option<CgfxDict<()>>,
    pub material_animations: Option<CgfxDict<()>>,
    pub visibility_animations: Option<CgfxDict<()>>,
    pub camera_animations: Option<CgfxDict<()>>,
    pub light_animations: Option<CgfxDict<()>>,
    pub fog_animations: Option<CgfxDict<()>>,
    pub emitters: Option<CgfxDict<()>>,
}

impl CgfxContainer {
    pub fn new(buffer: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(buffer);
        
        let header = CgfxHeader::read(&mut cursor)?;
        let mut dict_references: [(u32, Option<Pointer>); 16] = [Default::default(); 16];
        
        for dict_ref in &mut dict_references {
            let position = Pointer::try_from(&cursor)?;
            
            *dict_ref = (
                cursor.read_u32::<LittleEndian>()?,
                Pointer::read(&mut cursor)?.map(|pointer| pointer + position + 4),
            );
        }
        
        let mut unit_dicts: [Option<CgfxDict<()>>; 16] = Default::default();
        
        for (i, (count, offset)) in dict_references.into_iter().enumerate() {
            // textures
            if i == 1 {
                continue;
            }
            
            let dict = match offset {
                Some(value) => Some(CgfxDict::from_buffer(buffer, value)?),
                None => None,
            };
            
            if let Some(dict) = &dict {
                assert_eq!(dict.nodes.len(), (count + 1).try_into().unwrap());
            } else {
                assert_eq!(count, 0);
            }
            
            unit_dicts[i] = dict;
        }
        
        let mut unit_dicts_iter = unit_dicts.into_iter();
        
        let models = match dict_references[0].1 {
            Some(pointer) => Some(CgfxDict::<CgfxModel>::from_buffer(buffer, pointer)?),
            None => None,
        };
        
        let textures = match dict_references[1].1 {
            Some(pointer) => Some(CgfxDict::<CgfxTexture>::from_buffer(buffer, pointer)?),
            None => None,
        };
        
        Ok(CgfxContainer {
            header,
            
            models,
            textures,
            luts: unit_dicts_iter.nth(2).unwrap(),
            materials: unit_dicts_iter.next().unwrap(),
            shaders: unit_dicts_iter.next().unwrap(),
            cameras: unit_dicts_iter.next().unwrap(),
            lights: unit_dicts_iter.next().unwrap(),
            fogs: unit_dicts_iter.next().unwrap(),
            scenes: unit_dicts_iter.next().unwrap(),
            skeletal_animations: unit_dicts_iter.next().unwrap(),
            material_animations: unit_dicts_iter.next().unwrap(),
            visibility_animations: unit_dicts_iter.next().unwrap(),
            camera_animations: unit_dicts_iter.next().unwrap(),
            light_animations: unit_dicts_iter.next().unwrap(),
            fog_animations: unit_dicts_iter.next().unwrap(),
            emitters: unit_dicts_iter.next().unwrap(),
        })
    }
    
    pub fn to_buffer(&self) -> Result<Vec<u8>> {
        self.to_buffer_debug(None)
    }
    
    pub fn to_buffer_debug(&self, original: Option<&[u8]>) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut writer = Cursor::new(&mut out);
        
        self.header.write(&mut writer)?;
        assert_matching!(writer, original);
        
        // write zeroes for all dicts for now and patch them later
        let dict_pointers_location = Pointer::try_from(&writer)?;
        
        for _ in 0..16 {
            writer.write_u32::<LittleEndian>(0)?;
            writer.write_u32::<LittleEndian>(0)?;
        }
        
        // write main content
        let mut ctx = WriteContext::new();
        
        if let Some(textures) = &self.textures {
            // write reference in dict pointer array above
            let reference_offset: Pointer = dict_pointers_location + 8;
            
            let current_offset: Pointer = Pointer::try_from(&writer)?;
            let relative_offset: Pointer = current_offset - (reference_offset + 4);
            let count = textures.nodes.len() - 1;
            
            write_at_pointer(&mut writer, reference_offset, count.try_into()?)?;
            write_at_pointer(&mut writer, reference_offset + 4, relative_offset.into())?;
            
            // write dict
            textures.to_writer(&mut writer, &mut ctx)?;
        }
        
        // apply string references
        let string_section_start = Pointer::try_from(&writer)?;
        
        for (location, target_string) in ctx.string_references {
            if let Some(string_offset_usize) = ctx.string_section.find(&target_string) {
                let string_offset = Pointer::from(string_offset_usize) + string_section_start;
                let relative_offset = string_offset - location;
                
                write_at_pointer(&mut writer, location, relative_offset.into())?;
            }
        }
        
        // write strings
        writer.write_all(ctx.string_section.as_bytes())?;
        
        // apply padding
        let alignment: i32 = 128;
        let buffer_size: i32 = writer.position().try_into()?;
        let padding_size = ((-buffer_size - 8) % alignment + alignment) % alignment; // weird padding calculation
        
        writer.write_all(&vec![0u8; padding_size.try_into()?])?;
        
        // apply image section references
        let image_section_offset: Pointer = Pointer::try_from(&writer)? + 8;
        
        for (location, image_offset) in ctx.image_references {
            let absolute_offset = image_section_offset + image_offset;
            let relative_offset = absolute_offset - location;
            
            write_at_pointer(&mut writer, location, relative_offset.into())?;
        }
        
        assert_matching!(writer, original);
        
        // write image data section
        let image_section_length: u32 = ctx.image_section.len().try_into()?;
        
        writer.write_all(b"IMAG")?;
        writer.write_u32::<LittleEndian>(image_section_length + 8)?;
        
        writer.write_all(&ctx.image_section)?;
        
        assert_matching!(writer, original);
        assert!(writer.get_ref().len() == self.header.file_length as usize,
            "Written file size does not match expected file size, expected 0x{:x} bytes but got 0x{:x} bytes",
            self.header.file_length,
            writer.get_ref().len());
        
        Ok(out)
    }
    
    #[allow(unused_variables)] // temporary until I figure out how this works
    pub fn from_single_texture(name: String, orig_reference_bit: u32, texture: CgfxTexture) -> CgfxContainer {
        let header = CgfxHeader {
            byte_order_mark: 0xfeff,
            header_length: 20,
            revision: 0x5000000,
            file_length: 0x180 + texture.size(),
            sections_count: 2,
            content_magic_number: 0x41544144,
            content_length: 356,
        };
        
        let name_len =  texture.metadata().cgfx_object_header.name.as_ref()
            .map_or(0, |name| name.len());
        
        // println!("{}: {} {}", texture.metadata().name.as_ref().unwrap_or(&"None".to_string()), (name_len << 3) - 2, orig_reference_bit);
        
        let textures = CgfxDict::<CgfxTexture> {
            magic_number: "DICT".to_string(),
            tree_length: 44,
            values_count: 1,
            nodes: vec![
                CgfxNode::<CgfxTexture> {
                    reference_bit: 0xFFFFFFFF,
                    left_node_index: 1,
                    right_node_index: 0,
                    name: None,
                    value_pointer: None,
                    value: None,
                },
                CgfxNode::<CgfxTexture> {
                    reference_bit: ((name_len << 3) - 2).try_into().unwrap(),
                    left_node_index: 0,
                    right_node_index: 1,
                    name: Some(name),
                    value_pointer: None,
                    value: Some(texture),
                },
            ],
        };
        
        CgfxContainer {
            header,
            
            models: None,
            textures: Some(textures),
            luts: None,
            materials: None,
            shaders: None,
            cameras: None,
            lights: None,
            fogs: None,
            scenes: None,
            skeletal_animations: None,
            material_animations: None,
            visibility_animations: None,
            camera_animations: None,
            light_animations: None,
            fog_animations: None,
            emitters: None,
        }
    }
}
