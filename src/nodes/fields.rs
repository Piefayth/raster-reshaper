use bevy::{
    prelude::*,
    render::render_resource::{Extent3d, TextureFormat},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::kinds::shape::Shape;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Field {
    U32(u32),
    F32(f32),
    Vec4(Vec4),
    LinearRgba(LinearRgba),
    Extent3d(Extent3d),
    TextureFormat(TextureFormat),
    Shape(Shape),

     // we never serialize images since they can't be manually input, always from an edge
    Image(#[serde(serialize_with = "serialize_none_image", deserialize_with = "deserialize_none_image")]Option<Image>),
}

fn serialize_none_image<S>(_: &Option<Image>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer
{
    serializer.serialize_none()
}

fn deserialize_none_image<'de, D>(deserializer: D) -> Result<Option<Image>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<()> = Option::deserialize(deserializer)?;
    Ok(opt.and_then(|_| None))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldMeta {
    pub visible: bool,
    pub storage: Field,
}

impl From<u32> for Field {
    fn from(value: u32) -> Self {
        Field::U32(value)
    }
}
impl From<f32> for Field {
    fn from(value: f32) -> Self {
        Field::F32(value)
    }
}
impl From<Vec4> for Field {
    fn from(value: Vec4) -> Self {
        Field::Vec4(value)
    }
}
impl From<LinearRgba> for Field {
    fn from(value: LinearRgba) -> Self {
        Field::LinearRgba(value)
    }
}
impl From<Extent3d> for Field {
    fn from(value: Extent3d) -> Self {
        Field::Extent3d(value)
    }
}
impl From<TextureFormat> for Field {
    fn from(value: TextureFormat) -> Self {
        Field::TextureFormat(value)
    }
}
impl From<Option<Image>> for Field {
    fn from(value: Option<Image>) -> Self {
        Field::Image(value)
    }
}
impl From<Shape> for Field {
    fn from(value: Shape) -> Self {
        Field::Shape(value)
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

impl TryFrom<Field> for Shape {
    type Error = String;

    fn try_from(value: Field) -> Result<Self, Self::Error> {
        if let Field::Shape(v) = value {
            Ok(v)
        } else {
            Err(format!("Cannot convert {:?} to Shape", value))
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
        Field::Shape(_) => Shape::try_from(from.clone()).is_ok(),
        
    }
}