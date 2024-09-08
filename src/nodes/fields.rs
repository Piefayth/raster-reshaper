use super::macros::macros::define_field_enum;
use bevy::{
    prelude::*,
    render::render_resource::{Extent3d, TextureFormat},
};

define_field_enum! {
    #[derive(Clone, Debug)]
    pub enum Field {
        U32(u32),
        F32(f32),
        Vec4(Vec4),
        LinearRgba(LinearRgba),
        Extent3d(Extent3d),
        TextureFormat(TextureFormat),
        Image(Option<Image>)
    }
}

impl PartialEq for Field {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Field::U32(a), Field::U32(b)) => a == b,
            (Field::F32(a), Field::F32(b)) => a == b,
            (Field::Vec4(a), Field::Vec4(b)) => a == b,
            (Field::LinearRgba(a), Field::LinearRgba(b)) => a == b,
            (Field::Extent3d(a), Field::Extent3d(b)) => a == b,
            (Field::TextureFormat(a), Field::TextureFormat(b)) => a == b,
            (Field::Image(_), Field::Image(_)) => false, // Always return false for Image
            _ => false, // Different variants are never equal
        }
    }
}

#[derive(Clone, Debug)]
pub struct FieldMeta {
    pub visible: bool,
    pub storage: Field,
}

impl TryFrom<Field> for u32 {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::U32(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to u32", value))
        }
    }
}

impl TryFrom<Field> for f32 {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::F32(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to f32", value))
        }
    }
}

impl TryFrom<Field> for Vec4 {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::Vec4(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to Vec4", value))
        }
    }
}

impl TryFrom<Field> for Extent3d {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::Extent3d(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to Extent3d", value))
        }
    }
}

impl TryFrom<Field> for TextureFormat {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::TextureFormat(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to TextureFormat", value))
        }
    }
}

impl TryFrom<Field> for Option<Image> {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::Image(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to Option<Image>", value))
        }
    }
}

impl TryFrom<Field> for Image {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::Image(Some(v)) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to Image", value))
        }
    }
}

impl TryFrom<Field> for LinearRgba {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::LinearRgba(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to LinearRgba", value))
        }
    }
}


pub fn can_convert_field(from: &Field, to: &Field) -> bool {
    match to {
        Field::U32(_) => u32::try_from(from.clone()).is_ok(),
        Field::F32(_) => f32::try_from(from.clone()).is_ok(),
        Field::Vec4(_) => Vec4::try_from(from.clone()).is_ok(),
        Field::LinearRgba(_) => LinearRgba::try_from(from.clone()).is_ok(),
        Field::Extent3d(_) => Extent3d::try_from(from.clone()).is_ok(),
        Field::TextureFormat(_) => TextureFormat::try_from(from.clone()).is_ok(),
        Field::Image(_) => Option::<Image>::try_from(from.clone()).is_ok(),
    }
}