use std::{
    fmt::Debug,
    io::{Read, Seek, SeekFrom, Write},
};

use anyhow::{Error, Result};
use array_init::try_array_init;
use binrw::{BinRead, BinWrite};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};

use crate::{
    scoped_reader_pos,
    util::{
        pointer::Pointer,
        util::{brw_relative_pointer, CgfxObjectHeader},
    },
    CgfxCollectionValue, WriteContext,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, BinRead, BinWrite, Serialize, Deserialize)]
#[brw(repr(u32), little)]
pub enum PicaTextureFormat {
    RGBA8,
    RGB8,
    RGBA5551,
    RGB565,
    RGBA4,
    LA8,
    HiLo8,
    L8,
    A8,
    LA4,
    L4,
    A4,
    ETC1,
    ETC1A4,
}

impl PicaTextureFormat {
    pub fn get_bpp(&self) -> u32 {
        match self {
            PicaTextureFormat::RGBA8 => 32,
            PicaTextureFormat::RGB8 => 24,
            PicaTextureFormat::RGBA5551 => 16,
            PicaTextureFormat::RGB565 => 16,
            PicaTextureFormat::RGBA4 => 16,
            PicaTextureFormat::LA8 => 16,
            PicaTextureFormat::HiLo8 => 16,
            PicaTextureFormat::L8 => 8,
            PicaTextureFormat::A8 => 8,
            PicaTextureFormat::LA4 => 8,
            PicaTextureFormat::L4 => 4,
            PicaTextureFormat::A4 => 4,
            PicaTextureFormat::ETC1 => 4,
            PicaTextureFormat::ETC1A4 => 8,
        }
    }
}

#[derive(Clone, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
#[br(assert(location_ptr == 0, "ImageData has location_ptr {}", location_ptr))]
pub struct ImageData {
    pub height: u32,
    pub width: u32,
    
    #[brw(ignore)]
    pub image_bytes: Vec<u8>,
    
    buffer_length: u32,
    #[br(parse_with = brw_relative_pointer)]
    #[bw(map = |_| 0u32)]
    buffer_pointer: Option<Pointer>,
    
    pub dynamic_alloc: u32,
    pub bits_per_pixel: u32,
    pub location_ptr: u32, // ?
    pub memory_area: u32,
}

impl Debug for ImageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageData")
            .field("height", &self.height)
            .field("width", &self.width)
            .field("image_bytes", &format!("<buffer, {} bytes>", self.image_bytes.len()))
            .field("buffer_length", &self.buffer_length)
            .field("buffer_pointer", &self.buffer_pointer)
            .field("dynamic_alloc", &self.dynamic_alloc)
            .field("bits_per_pixel", &self.bits_per_pixel)
            .field("location_ptr", &self.location_ptr)
            .field("memory_area", &self.memory_area)
            .finish()
    }
}

#[derive(Debug, Clone, BinRead, BinWrite, PartialEq)]
#[brw(little)]
pub struct CgfxTextureCommon {
    // cgfx object header
    pub cgfx_object_header: CgfxObjectHeader,
    
    // common texture fields
    pub height: u32,
    pub width: u32,
    pub gl_format: u32,
    pub gl_type: u32,
    pub mipmap_size: u32,
    pub texture_obj: u32,
    pub location_flag: u32,
    pub texture_format: PicaTextureFormat,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CgfxTexture {
    Cube(CgfxTextureCommon, Box<[ImageData; 6]>),
    Image(CgfxTextureCommon, Option<ImageData>),
}

fn image_data<R: Read + Seek>(reader: &mut R) -> Result<Option<ImageData>> {
    let image_data_pointer = Pointer::read(reader)?;
    
    let data = image_data_pointer
        .map(|pointer| {
            scoped_reader_pos!(reader);
            reader.seek(SeekFrom::Current(i64::from(pointer) - 4))?;
            
            let mut data = ImageData::read(reader)?;
            reader.seek(SeekFrom::Start(data.buffer_pointer.unwrap().into()))?;
            
            let mut image_bytes: Vec<u8> = vec![0; data.buffer_length.try_into()?];
            reader.read_exact(&mut image_bytes)?;
            data.image_bytes = image_bytes;
            
            Ok::<ImageData, Error>(data)
        })
        .transpose()?;
    
    Ok(data)
}

impl CgfxTexture {
    pub fn from_reader<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let texture_type_discriminant = reader.read_u32::<LittleEndian>()?;
        
        let common = CgfxTextureCommon::read(reader)?;
        
        let result = match texture_type_discriminant {
            0x20000009 => CgfxTexture::Cube(common,
                Box::new(try_array_init(|_| image_data(reader).transpose().unwrap())?)),
            0x20000011 => CgfxTexture::Image(common, image_data(reader)?),
            
            _ => return Err(Error::msg(format!("Invalid Texture discriminant {:x}", texture_type_discriminant)))
        };
        
        Ok(result)
    }
    
    pub fn to_writer<W: Write + Seek>(&self, writer: &mut W, ctx: &mut WriteContext) -> Result<()> {
        // write discriminant
        let discriminant: u32 = match self {
            CgfxTexture::Cube(_, _) => 0x20000009,
            CgfxTexture::Image(_, _) => 0x20000011,
        };
        
        writer.write_u32::<LittleEndian>(discriminant)?;
        
        // write common stuff
        let common = match self {
            CgfxTexture::Cube(common, _) => common,
            CgfxTexture::Image(common, _) => common,
        };
        
        let common_offset = Pointer::current(writer)?;
        let name_offset = common_offset + 8;
        assert!(common.cgfx_object_header.metadata_pointer.is_none());
        
        if let Some(name) = &common.cgfx_object_header.name {
            ctx.add_string(name)?;
            ctx.add_string_reference(name_offset, name.clone());
        }
        
        common.write(writer)?;
        
        // write texture specific stuff
        match self {
            CgfxTexture::Cube(_, _images) => todo!(),
            CgfxTexture::Image(_, image) => {
                writer.write_u32::<LittleEndian>(4)?;
                
                if let Some(image) = image {
                    // make sure image.buffer_pointer gets updated
                    let current_offset = Pointer::current(writer)?;
                    ctx.add_image_reference_to_current_end(current_offset + 12)?;
                    ctx.append_to_image_section(&image.image_bytes)?;
                }
                
                // when are they serialized? here or after the textures in general?
                image.write(writer)?;
            },
        }
        
        Ok(())
    }
    
    pub fn metadata(&self) -> &CgfxTextureCommon {
        match self {
            CgfxTexture::Image(common, _) => common,
            CgfxTexture::Cube(common, _) => common,
        }
    }
    
    pub fn metadata_mut(&mut self) -> &mut CgfxTextureCommon {
        match self {
            CgfxTexture::Image(common, _) => common,
            CgfxTexture::Cube(common, _) => common,
        }
    }
    
    pub fn size(&self) -> u32 {
        match self {
            CgfxTexture::Image(_, image_data) => {
                if let Some(image_data) = image_data {
                    image_data.image_bytes.len().try_into().unwrap()
                } else {
                    0
                }
            },
            CgfxTexture::Cube(_, vec) =>
                vec.iter().map(|image| image.image_bytes.len() as u32).sum(),
        }
    }
}

impl CgfxCollectionValue for CgfxTexture {
    fn read_dict_value<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        Self::from_reader(reader)
    }
    
    fn write_dict_value<W: Write + Seek>(&self, writer: &mut W, ctx: &mut WriteContext) -> Result<()> {
        self.to_writer(writer, ctx)
    }
}
