use crate::wire::OperationCode;
use serde::{Deserialize, Serialize, Serializer};

use super::{EmptyResponse, Operation};

/// Text alignment for header/footer lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TextAlign {
    /// `1` — Left-aligned.
    Left = 1,
    /// `2` — Centered.
    Centered = 2,
    /// `3` — Right-aligned.
    Right = 3,
}

impl Serialize for TextAlign {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for TextAlign {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let code = u8::deserialize(de)?;
        Ok(match code {
            1 => Self::Left,
            2 => Self::Centered,
            3 => Self::Right,
            other => {
                return Err(serde::de::Error::custom(format!(
                    "unknown text alignment {other} (expected 1, 2, or 3)"
                )));
            }
        })
    }
}

/// A single line of header or footer text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TextLine {
    /// Alignment.
    #[cfg_attr(feature = "schema", schemars(with = "u8"))]
    pub align: TextAlign,
    /// Bold flag, serialised as a JSON boolean (`true`/`false`) per the spec's field-type column
    /// ("Boolean"). The spec's §4.6.3 JSON example loosely writes it as the integer `1`, but a
    /// Newland N950 was verified to accept the boolean form (op 7 returned success).
    pub bold: bool,
    /// Font size (1 through 5).
    pub fsize: u8,
    /// Text content.
    pub text: String,
}

/// Op 7 request: configure receipt header and footer lines.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct SetupHeaderFooterRequest {
    /// Header lines, in print order (top to bottom).
    #[serde(default)]
    pub headers: Vec<TextLine>,
    /// Footer lines, in print order (top to bottom).
    #[serde(default)]
    pub footers: Vec<TextLine>,
}

impl Operation for SetupHeaderFooterRequest {
    const CODE: OperationCode = OperationCode::SetupHeaderFooter;
    type Response = EmptyResponse;
}

/// Op 8 request: upload a header logo image (Base64-encoded BMP, colour depth ≤4 bits).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct SetupHeaderLogoRequest {
    /// Logo image bytes encoded as a Base64 string. Per spec §4.6.4 the image must be in **BMP**
    /// format with a colour depth of **no more than 4 bits** (≤16 colours; 1-bit monochrome is
    /// fine). Note: the English translation's "must not exceed 4 pixels in height" is a
    /// mistranslation of the original "must not contain more than 4 bits of colour".
    #[serde(rename = "headerLogo")]
    pub header_logo: String,
}

impl Operation for SetupHeaderLogoRequest {
    const CODE: OperationCode = OperationCode::SetupHeaderLogo;
    type Response = EmptyResponse;
}
