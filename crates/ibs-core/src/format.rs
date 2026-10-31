use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ImageFormat {
    Png,
    #[default]
    Jpeg,
    Webp,
    Avif,
    Bmp,
    Tiff,
    Gif,
    Ico,
}

impl ImageFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Avif => "avif",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
            Self::Gif => "gif",
            Self::Ico => "ico",
        }
    }

    /// Formats accepted as input. AVIF input is planned for a later phase.
    pub fn from_input_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "png" => Self::Png,
            "jpg" | "jpeg" | "jpe" | "jfif" => Self::Jpeg,
            "webp" => Self::Webp,
            "bmp" => Self::Bmp,
            "tif" | "tiff" => Self::Tiff,
            "gif" => Self::Gif,
            "ico" => Self::Ico,
            _ => return None,
        })
    }

    /// Whether this build can encode the format.
    pub fn can_encode(self) -> bool {
        !matches!(self, Self::Avif)
    }
}

/// What to do when an output file already exists. Source files are never
/// overwritten regardless of this policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ConflictPolicy {
    #[default]
    Rename,
    Skip,
    Overwrite,
}
