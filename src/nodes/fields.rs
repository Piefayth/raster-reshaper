
use bevy::{
    prelude::*,
    render::render_resource::{Extent3d, TextureFormat},
};
use super::{macros::macros::define_field_enum};

define_field_enum! {
    #[derive(Clone, Debug)]
    pub enum Field {
        U32(u32),
        F32(f32),
        Vec4(Vec4),
        Extent3d(Extent3d),
        TextureFormat(TextureFormat),
        Image(Option<Image>)
    }
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