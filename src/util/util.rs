use std::{
    fmt::Debug,
    io::{Read, Seek, SeekFrom, Write},
    str::from_utf8,
};

use anyhow::Result;
use binrw::{parser, writer, BinRead, BinResult, BinWrite, Endian};
use byteorder::{LittleEndian, ReadBytesExt};

use crate::{
    scoped_reader_pos,
    util::{
        math::{Mat3x4, Vec3},
        pointer::Pointer,
    },
    CgfxCollectionValue, CgfxDict,
};

#[allow(path_statements)] // to disable warning on `endian;`
#[parser(reader, endian)]
pub fn brw_read_4_byte_string() -> BinResult<String> {
    // I don't need to know the endianness and I can't find a
    // better way to ignore the warning
    endian;
    
    let mut bytes: [u8; 4] = [0; 4];
    reader.read_exact(&mut bytes)?;
    
    Ok(from_utf8(&bytes).unwrap().to_string()) // ughhh error handling is so painful with binrw
}

#[writer(writer, endian)]
pub fn brw_write_4_byte_string(string: &String) -> BinResult<()> {
    let bytes = string.as_bytes();
    let out = u32::from_le_bytes(bytes.try_into().unwrap()); // unwrap because BinResult is a pain
    
    out.write_options(writer, endian, ())?;
    Ok(())
}

pub fn read_string(read: &mut impl Read) -> Result<String> {
    let mut string_buffer = Vec::new();
    
    loop {
        let b = read.read_u8().unwrap();
        
        if b != 0 {
            string_buffer.push(b);
        } else {
            break;
        }
    }
    
    Ok(String::from_utf8(string_buffer)?)
}

#[parser(reader, endian)]
pub fn brw_read_string() -> BinResult<Option<String>> {
    let reader_pos = reader.stream_position()?;
    let pointer: u64 = u32::read_options(reader, endian, ())?.into();
    
    if pointer == 0 {
        return Ok(None);
    }
    
    reader.seek(SeekFrom::Start(reader_pos + pointer))?;
    
    let string = read_string(reader)
        .map_err(|err| binrw::Error::Custom {
            pos: reader.stream_position().unwrap(),
            err: Box::new(err),
        })?;
    
    reader.seek(SeekFrom::Start(reader_pos + 4))?;
    
    Ok(Some(string))
}

#[writer(writer, endian)]
pub fn brw_write_zero(_: &Option<String>) -> BinResult<()> {
    0u32.write_options(writer, endian, ())?;
    Ok(())
}

#[parser(reader, endian)]
pub fn brw_relative_pointer() -> BinResult<Option<Pointer>> {
    let reader_pos: i64 = reader.stream_position()?.try_into().unwrap();
    let pointer: i64 = i32::read_options(reader, endian, ())?.into();
    
    if pointer == 0 {
        return Ok(None);
    }
    
    Ok(Some(Pointer::from(reader_pos + pointer)))
}

pub fn read_pointer_list<T: CgfxCollectionValue, R: Read + Seek>(reader: &mut R) -> Result<Vec<T>> {
    read_pointer_list_ext(reader, None)
}

pub fn read_pointer_list_ext<T: CgfxCollectionValue, R: Read + Seek>(reader: &mut R, magic: Option<u32>) -> Result<Vec<T>> {
    let count = reader.read_u32::<LittleEndian>()?;
    let list_ptr = Pointer::read_relative(reader)?;
    
    let values: Vec<T> = if let Some(list_ptr) = list_ptr {
        scoped_reader_pos!(reader);
        let mut values: Vec<T> = Vec::with_capacity(count as usize);
        
        reader.seek(SeekFrom::Start(list_ptr.into()))?;
        
        let object_pointers: Vec<Option<Pointer>> = (0..count)
            .map(|_| Pointer::read_relative(reader))
            .collect::<Result<Vec<Option<Pointer>>>>()?;
        
        for object_pointer in object_pointers.into_iter().flatten() {
            reader.seek(SeekFrom::Start(object_pointer.into()))?;
            
            if let Some(magic) = magic {
                assert!(reader.read_u32::<LittleEndian>()? == magic);
            }
            
            values.push(T::read_dict_value(reader)?);
        }
        
        values
    } else {
        Vec::new()
    };
    
    Ok(values)
}

pub fn read_inline_list<T: CgfxCollectionValue, R: Read + Seek>(reader: &mut R) -> Result<Vec<T>> {
    let count = reader.read_u32::<LittleEndian>()?;
    let list_ptr = Pointer::read(reader)?;
    
    let values: Vec<T> = if let Some(list_ptr) = list_ptr {
        scoped_reader_pos!(reader);
        
        reader.seek(SeekFrom::Current(i64::from(list_ptr) - 4))?;
        
        let values: Vec<T> = (0..count)
            .map(|_| T::read_dict_value(reader))
            .collect::<Result<Vec<T>>>()?;
        
        values
    } else {
        Vec::new()
    };
    
    Ok(values)
}

#[derive(Clone, Debug, PartialEq)]
pub struct CgfxBox<T: BinRead + BinWrite + Clone> {
    pub value: Option<T>,
}

impl<'b, T> BinRead for CgfxBox<T>
where
    T: BinRead<Args<'b> = ()> + BinWrite<Args<'b>= ()> + Clone,
{
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(reader: &mut R, endian: Endian, _args: ()) -> BinResult<Self> {
        let reader_pos = reader.stream_position()?;
        let pointer: u64 = u32::read_options(reader, endian, ())?.into();
        
        if pointer == 0 {
            return Ok(Self { value: None });
        }
        
        scoped_reader_pos!(reader);
        
        reader.seek(SeekFrom::Start(reader_pos + pointer))?;
        
        let value = Some(T::read_options(reader, endian, ())?);
        
        Ok(Self { value })
    }
}

impl<T: BinRead + BinWrite + Clone> BinWrite for CgfxBox<T> {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, _args: ()) -> BinResult<()> {
        0u32.write_options(writer, endian, ())?;
        Ok(())
    }
}

impl<T: BinRead + BinWrite + Clone> Into<Option<T>> for CgfxBox<T> {
    fn into(self) -> Option<T> {
        self.value
    }
}

impl<T: BinRead + BinWrite + Clone> From<Option<T>> for CgfxBox<T> {
    fn from(value: Option<T>) -> Self {
        Self { value }
    }
}

impl<T: BinRead + BinWrite + Clone> From<&Option<T>> for CgfxBox<T> {
    fn from(value: &Option<T>) -> Self {
        Self {
            value: value.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, BinRead, BinWrite)]
// vvv required because brw_write_4_byte_string might panic otherwise
#[brw(assert(magic.len() == 4, "Length of magic number {:?} must be 4 bytes", magic))]
// TODO: properly implement this
// #[br(assert(metadata_pointer == None, "CgfxTexture {:?} has metadata {:?}", name, metadata_pointer))]
#[brw(little)]
pub struct CgfxObjectHeader {
    #[br(parse_with = brw_read_4_byte_string)]
    #[bw(write_with = brw_write_4_byte_string)]
    pub magic: String,
    pub revision: u32,
    
    #[br(parse_with = brw_read_string)]
    #[bw(write_with = brw_write_zero)]
    pub name: Option<String>,
    pub metadata_count: u32,
    
    #[br(map = |x: u32| Pointer::new(x))]
    #[bw(map = |x: &Option<Pointer>| x.map_or(0, |ptr| ptr.0))]
    pub metadata_pointer: Option<Pointer>,
}

#[derive(Debug, Clone, PartialEq, BinRead, BinWrite)]
#[brw(little)]
pub struct CgfxNodeHeader {
    pub branch_visible: u32,
    pub is_branch_visible: u32,
    
    pub child_count: u32,
    pub children_pointer: Option<Pointer>,
    
    #[brw(ignore)]
    pub anim_groups: CgfxDict<()>,
    
    anim_group_count: u32,
    anim_group_pointer: Option<Pointer>,
}

#[derive(Debug, Clone, PartialEq, BinRead, BinWrite)]
#[brw(little)]
pub struct CgfxTransform {
    pub scale: Vec3,
    pub rotation: Vec3,
    pub translation: Vec3,
    
    pub local_transform: Mat3x4,
    pub world_transform: Mat3x4,
}
