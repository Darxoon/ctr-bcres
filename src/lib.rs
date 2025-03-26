use std::{
    io::{Cursor, Read, Seek, SeekFrom, Write},
    str::from_utf8,
};

use anyhow::Result;
use bcres::texture::PicaTextureFormat;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use util::pointer::Pointer;

pub mod bcres;
pub mod util;

fn get_string(bytes: &[u8], start: Pointer) -> Result<String> {
	let bytes_slice = &bytes[start.into()..];
	let null_position_from_start = bytes_slice.iter().position(|&x| x == 0x0);
	
	let string = if let Some(null_position_from_start) = null_position_from_start {
		from_utf8(&bytes_slice[..null_position_from_start])?
	} else {
		from_utf8(bytes_slice)?
	};
	
	Ok(string.to_owned())
}

pub fn get_4_byte_string(reader: &mut impl Read) -> Result<String> {
	let mut bytes: [u8; 4] = [0; 4];
	reader.read(&mut bytes)?;
	
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
        
        Self {
            reader,
            start_pos,
        }
    }
}

impl<'a, R: Read + Seek> Drop for ReaderGuard<'a, R> {
    fn drop(&mut self) {
        self.reader.seek(SeekFrom::Start(self.start_pos)).unwrap();
    }
}

#[macro_export]
macro_rules! scoped_reader_pos {
    ($reader:ident) => {
        let guard = crate::ReaderGuard::new($reader);
        let $reader = &mut *guard.reader;
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryItem {
	pub id: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub is_readonly: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub image_format: Option<PicaTextureFormat>,
	#[serde(skip)]
	pub file_offset: u32,
	pub field_0x8: u32,
	#[serde(skip)]
	pub byte_length: u32,
}

impl RegistryItem {
	pub fn read(reader: &mut impl Read, get_string: &impl Fn(Pointer) -> Result<String>) -> Result<Self> {
		let id_pointer = Pointer::read(reader)?
			.unwrap_or(Pointer::default());
		let file_offset = reader.read_u32::<LittleEndian>()?;
		let field_0x8 = reader.read_u32::<LittleEndian>()?;
		let byte_length = reader.read_u32::<LittleEndian>()?;
        
		let id = get_string(id_pointer)?;
		
		Ok(Self {
			id,
			is_readonly: None,
			image_format: None,
			file_offset,
			field_0x8,
			byte_length,
		})
	}
	
	pub fn write(&self, writer: &mut impl Write, write_string: &mut impl FnMut(&str) -> Pointer) -> Result<()> {
		let id_pointer = write_string(&self.id);
		id_pointer.write(writer)?;
		
		writer.write_u32::<LittleEndian>(self.file_offset)?;
		writer.write_u32::<LittleEndian>(self.field_0x8)?;
		writer.write_u32::<LittleEndian>(self.byte_length)?;
		
		Ok(())
	}
}

pub struct ArchiveRegistry {
	pub items: Vec<RegistryItem>,
}

impl ArchiveRegistry {
	pub fn new(buffer: &[u8]) -> Result<Self> {
		let mut cursor = Cursor::new(buffer);
		
		let item_count = cursor.read_u32::<LittleEndian>()?;
		let mut items = Vec::default();
		
		let string_section_offset = 4 + item_count * 16;
		let get_string = |ptr| get_string(buffer, ptr + string_section_offset);
		
		for _ in 0..item_count {
			items.push(RegistryItem::read(&mut cursor, &get_string)?);
		}
		
        Ok(ArchiveRegistry { items })
	}
	
	pub fn to_buffer(&self) -> Result<Vec<u8>> {
		let mut main_buffer: Vec<u8> = Vec::new();
		let mut string_buffer: Vec<u8> = Vec::new();
		
		let mut write_string = |string: &str| {
			let current_offset: Pointer = string_buffer.len().into();
			
			string_buffer.extend(string.bytes());
			string_buffer.extend([0].iter());
			
			current_offset
		};
		
		main_buffer.write_u32::<LittleEndian>(self.items.len().try_into().unwrap())?;
		
		for item in &self.items {
			item.write(&mut main_buffer, &mut write_string)?;
		}
		
		main_buffer.extend(string_buffer);
		
		Ok(main_buffer)
	}
	
	pub fn to_yaml(&self) -> Result<String> {
		let yaml = serde_yaml::to_string(&self.items)?;
		Ok(yaml)
	}
	
	pub fn from_yaml(yaml: &str) -> Result<Self> {
		let items: Vec<RegistryItem> = serde_yaml::from_str(yaml)?;
		Ok(ArchiveRegistry { items })
	}
}
