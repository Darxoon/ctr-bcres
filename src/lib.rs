use std::{
    collections::HashMap,
    io::{Cursor, Read, Seek, SeekFrom, Write},
    str::from_utf8,
};

use anyhow::Result;
use binrw::{BinRead, BinWrite};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use util::{pointer::Pointer, util::read_string};

pub mod cgfx_container;
pub mod image_codec;
pub mod model;
pub mod texture;

pub mod util;

pub fn get_4_byte_string(reader: &mut impl Read) -> Result<String> {
    let mut bytes: [u8; 4] = [0; 4];
    reader.read_exact(&mut bytes)?;

    Ok(from_utf8(&bytes)?.to_string())
}

pub fn write_at_pointer<W: Write + Seek>(writer: &mut W, pointer: Pointer, value: u32) -> Result<()> {
    let current_offset = writer.stream_position()?;
    
    writer.seek(SeekFrom::Start(pointer.into()))?;
    writer.write_u32::<LittleEndian>(value)?;
    
    writer.seek(SeekFrom::Start(current_offset))?;
    
    Ok(())
}

#[macro_export]
macro_rules! assert_matching {
    ($writer:ident, $base_option:ident) => {
        if let Some(base) = $base_option {
            assert!(&***$writer.get_ref() == &base[..$writer.get_ref().len()], "Not matching");
        }
    };
}

pub struct ReaderGuard<'a, R: Read + Seek> {
    pub reader: &'a mut R,
    start_pos: u64,
}

impl<'a, R: Read + Seek> ReaderGuard<'a, R> {
    pub fn new(reader: &'a mut R) -> Self {
        let start_pos = reader.stream_position().unwrap();

        Self { reader, start_pos }
    }
}

impl<R: Read + Seek> Drop for ReaderGuard<'_, R> {
    fn drop(&mut self) {
        self.reader.seek(SeekFrom::Start(self.start_pos)).unwrap();
    }
}

#[macro_export]
macro_rules! scoped_reader_pos {
    ($reader:ident) => {
        let guard = $crate::ReaderGuard::new($reader);
        let $reader = &mut *guard.reader;
    };
}

#[derive(Default)]
pub struct WriteContext {
    string_section: String,
    string_references: HashMap<Pointer, String>,
    
    image_section: Vec<u8>,
    // keys in image_references are relative to entire file
    // values are relative to the image section
    image_references: HashMap<Pointer, Pointer>,
}

impl WriteContext {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_string(&mut self, string: &str) -> Result<()> {
        if self.string_section.contains(string) {
            // string exists already, exiting early
            return Ok(());
        }
        
        self.string_section.push_str(string);
        self.string_section.push('\0');
        Ok(())
    }
    
    pub fn add_string_reference(&mut self, origin: Pointer, target_string: String) {
        self.string_references.insert(origin, target_string);
    }
    
    pub fn append_to_image_section(&mut self, content: &[u8]) -> Result<()> {
        // because binrw overwrites Vec::write
        // that's why you don't use "write" as a function name for a method
        // you are extending almost every fucking collection with
        Write::write(&mut self.image_section, content)?;
        Ok(())
    }
    
    pub fn add_image_reference_to_current_end(&mut self, origin: Pointer) -> Result<()> {
        self.image_references.insert(origin, self.image_section.len().into());
        Ok(())
    }
}

pub trait CgfxCollectionValue: Sized {
    fn read_dict_value<R: Read + Seek>(reader: &mut R) -> Result<Self>;
    // TODO: migrate this to use impl Read + Seek instead of Cursor
    fn write_dict_value(&self, writer: &mut Cursor<&mut Vec<u8>>, ctx: &mut WriteContext) -> Result<()>;
}

// auto implement CgfxCollectionValue for all binrw types
impl<T: BinRead + BinWrite> CgfxCollectionValue for T
where 
    for<'a> <T as BinRead>::Args<'a>: Default,
    for<'a> <T as BinWrite>::Args<'a>: Default,
{
    fn read_dict_value<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        Ok(Self::read_le(reader)?)
    }

    fn write_dict_value(&self, writer: &mut Cursor<&mut Vec<u8>>, _ctx: &mut WriteContext) -> Result<()> {
        self.write_le(writer)?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CgfxNode<T: CgfxCollectionValue> {
    pub reference_bit: u32,
    pub left_node_index: u16,
    pub right_node_index: u16,
    
    pub name: Option<String>,
    
    pub value_pointer: Option<Pointer>,
    pub value: Option<T>,
}

impl<T: CgfxCollectionValue> CgfxNode<T> {
    pub fn from_reader<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let reference_bit = reader.read_u32::<LittleEndian>()?;
        let left_node_index = reader.read_u16::<LittleEndian>()?;
        let right_node_index = reader.read_u16::<LittleEndian>()?;
        
        let name_pointer = Pointer::read_relative(reader)?;
        let value_pointer = Pointer::read_relative(reader)?;
        
        let name = if let Some(name_pointer) = name_pointer {
            scoped_reader_pos!(reader);
            reader.seek(SeekFrom::Start(name_pointer.into()))?;
            
            Some(read_string(reader)?)
        } else {
            None
        };
        
        let value = if let Some(value_pointer) = value_pointer {
            scoped_reader_pos!(reader);
            reader.seek(SeekFrom::Start(value_pointer.into()))?;
            
            Some(T::read_dict_value(reader)?)
        } else {
            None
        };
        
        Ok(CgfxNode {
            reference_bit,
            left_node_index,
            right_node_index,
            
            name,
            
            value_pointer,
            value,
        })
    }
    
    pub fn to_writer(&self, writer: &mut Cursor<&mut Vec<u8>>, ctx: &mut WriteContext) -> Result<Pointer> {
        writer.write_u32::<LittleEndian>(self.reference_bit)?;
        writer.write_u16::<LittleEndian>(self.left_node_index)?;
        writer.write_u16::<LittleEndian>(self.right_node_index)?;
        
        // name pointer and value pointer, write zero for now and patch it back later
        let name_pointer_location = Pointer::try_from(&writer)?;
        writer.write_u32::<LittleEndian>(0)?;
        let value_pointer_location = Pointer::try_from(&writer)?;
        writer.write_u32::<LittleEndian>(0)?;
        
        if let Some(name) = &self.name {
            ctx.add_string(name)?;
            ctx.add_string_reference(name_pointer_location, name.clone());
        }
        
        Ok(value_pointer_location)
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CgfxDict<T: CgfxCollectionValue> {
    pub magic_number: String,
    pub tree_length: u32,
    pub values_count: u32,
    pub nodes: Vec<CgfxNode<T>>,
}

impl<T: CgfxCollectionValue> CgfxDict<T> {
    pub fn from_buffer(buffer: &[u8], start_position: Pointer) -> Result<Self> {
        let mut cursor = Cursor::new(buffer);
        cursor.set_position(start_position.into());
        
        Self::from_reader(&mut cursor)
    }
    
    pub fn from_reader<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let magic_number = get_4_byte_string(reader)?;
        let tree_length = reader.read_u32::<LittleEndian>()?;
        let values_count = reader.read_u32::<LittleEndian>()?;
        
        let nodes = (0..values_count + 1)
            .map(|_| CgfxNode::from_reader(reader))
            .collect::<Result<Vec<CgfxNode<T>>>>()?;
        
        Ok(CgfxDict {
            magic_number,
            tree_length,
            values_count,
            nodes,
        })
    }
    
    pub fn to_writer(&self, writer: &mut Cursor<&mut Vec<u8>>, ctx: &mut WriteContext) -> Result<()> {
        assert!(self.values_count + 1 == self.nodes.len() as u32, "values_count does not match node count");
        
        write!(writer, "{}", self.magic_number)?;
        writer.write_u32::<LittleEndian>(self.tree_length)?;
        writer.write_u32::<LittleEndian>(self.values_count)?;
        
        for node in &self.nodes {
            let value_pointer_location = node.to_writer(writer, ctx)?;
            
            // TODO: when are the values serialized? here or in a separate loop
            if let Some(value) = &node.value {
                // update value pointer to point to current location
                let current_offset = Pointer::try_from(&writer)?;
                let relative_value_offset = current_offset - value_pointer_location;
                
                write_at_pointer(writer, value_pointer_location, relative_value_offset.into())?;
                
                // write value
                value.write_dict_value(writer, ctx)?;
            }
        }
        
        Ok(())
    }
}
