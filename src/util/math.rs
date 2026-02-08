use std::{io::{Read, Seek, Write}, mem::MaybeUninit};

use binrw::{BinRead, BinResult, BinWrite, Endian};
#[cfg(feature = "bytemuck")]
use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Debug, PartialEq, Default, BinRead, BinWrite)]
#[cfg_attr(feature = "bytemuck", derive(Pod, Zeroable))]
#[brw(little)]
#[repr(C)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec2> for Vec2 {
    fn from(value: glam::Vec2) -> Self {
        Self::new(value.x, value.y)
    }
}

#[cfg(feature = "glam")]
impl From<Vec2> for glam::Vec2 {
    fn from(value: Vec2) -> Self {
        glam::Vec2::new(value.x, value.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default, BinRead, BinWrite)]
#[cfg_attr(feature = "bytemuck", derive(Pod, Zeroable))]
#[brw(little)]
#[repr(C)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for Vec3 {
    fn from(value: glam::Vec3) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

#[cfg(feature = "glam")]
impl From<Vec3> for glam::Vec3 {
    fn from(value: Vec3) -> Self {
        glam::Vec3::new(value.x, value.y, value.z)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, BinRead, BinWrite)]
#[cfg_attr(feature = "bytemuck", derive(Pod, Zeroable))]
#[brw(little)]
#[repr(C)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Vec4 { x, y, z, w }
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec4> for Vec4 {
    fn from(value: glam::Vec4) -> Self {
        Self::new(value.x, value.y, value.z, value.w)
    }
}

#[cfg(feature = "glam")]
impl From<Vec4> for glam::Vec4 {
    fn from(value: Vec4) -> Self {
        glam::Vec4::new(value.x, value.y, value.z, value.w)
    }
}

// binrw matrix helper
#[derive(Clone, Debug, PartialEq)]
#[repr(C)]
pub struct SerializableMatrix<const R: usize, const C: usize> {
    data: [[f32; R]; C],
}

impl<const R: usize, const C: usize> BinRead for SerializableMatrix<R, C> {
    type Args<'a> = ();

    fn read_options<T: Read + Seek>(reader: &mut T, endian: Endian, _: Self::Args<'_>) -> BinResult<Self> {
        // SAFETY: all zeroes is a valid bit pattern of floats
        let mut data: [[f32; R]; C] = unsafe { MaybeUninit::zeroed().assume_init() };
        
        for i in 0..C {
            for j in 0..R {
                data[i][j] = f32::read_options(reader, endian, ())?;
            }
        }
        
        Ok(Self {
            data,
        })
    }
}

impl<const R: usize, const C: usize> BinWrite for SerializableMatrix<R, C> {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(&self, writer: &mut W, endian: Endian, _: Self::Args<'_>) -> BinResult<()> {
        self.data.write_options(writer, endian, ())
    }
}

pub type Mat3 = SerializableMatrix<3, 3>;
pub type Mat3x4 = SerializableMatrix<3, 4>;
pub type Mat4 = SerializableMatrix<4, 4>;

#[cfg(feature = "glam")]
impl From<glam::Mat3> for Mat3 {
    fn from(value: glam::Mat3) -> Self {
        // SAFETY: they have the same byte representation
        unsafe { transmute(value) }
    }
}

#[cfg(feature = "glam")]
impl From<Mat3> for glam::Mat3 {
    fn from(value: Mat3) -> Self {
        // SAFETY: they have the same byte representation
        unsafe { transmute(value) }
    }
}

#[cfg(feature = "glam")]
impl From<glam::Mat4> for Mat4 {
    fn from(value: glam::Mat4) -> Self {
        // SAFETY: they have the same byte representation
        unsafe { transmute(value) }
    }
}

#[cfg(feature = "glam")]
impl From<Mat4> for glam::Mat4 {
    fn from(value: Mat4) -> Self {
        // SAFETY: they have the same byte representation
        unsafe { transmute(value) }
    }
}
