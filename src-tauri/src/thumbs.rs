//! Image thumbnails for the grid view, cached on disk.
//!
//! Fast path: read only the head of the file and use the EXIF-embedded
//! thumbnail (JPEG from cameras and phones, also inside HEIC). Slow path:
//! fetch the whole image (bounded size) and downscale it.

use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageReader};
use smb2::FileReader;
use tokio::sync::Semaphore;

use crate::error::AppError;

const HEAD_BYTES: u64 = 256 * 1024;
const MAX_FULL_DECODE: u64 = 24 * 1024 * 1024;
const THUMB_PX: u32 = 320;

/// Limits concurrent decodes so a big folder doesn't saturate the CPU or
/// the SMB connection.
pub struct ThumbLimiter {
    pub sem: Semaphore,
}

impl Default for ThumbLimiter {
    fn default() -> Self {
        Self { sem: Semaphore::new(3) }
    }
}

pub fn is_thumbnailable(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "heic" | "heif" | "avif" | "dng" | "cr2" | "nef" | "arw"
    )
}

fn can_full_decode(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tif" | "tiff")
}

pub fn cache_path(cache_dir: &Path, server_id: &str, share: &str, path: &str, size: u64, mtime: u64) -> PathBuf {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (server_id, share, path, size, mtime).hash(&mut h);
    cache_dir.join("thumbs").join(format!("{:016x}.jpg", h.finish()))
}

pub fn to_data_url(jpeg: &[u8]) -> String {
    format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(jpeg))
}

/// Build a thumbnail for the file behind `reader`, returning JPEG bytes.
pub async fn build(reader: &FileReader, name: &str) -> Result<Vec<u8>, AppError> {
    let size = reader.size();
    let head = reader.read_at(0, HEAD_BYTES.min(size)).await?;
    let name_owned = name.to_string();

    // Fast path: EXIF thumbnail from the head bytes only.
    let from_exif = tokio::task::spawn_blocking({
        let head = head.clone();
        move || exif_thumbnail(&head)
    })
    .await?;
    if let Some(jpeg) = from_exif {
        return Ok(jpeg);
    }

    if !can_full_decode(&name_owned) {
        return Err(AppError::new("no_thumbnail", "no embedded thumbnail and format needs a full decode"));
    }
    if size > MAX_FULL_DECODE {
        return Err(AppError::new("too_large", "image too large to thumbnail"));
    }
    let mut data = head;
    if size > data.len() as u64 {
        let rest = reader.read_at(data.len() as u64, size - data.len() as u64).await?;
        data.extend_from_slice(&rest);
    }
    tokio::task::spawn_blocking(move || decode_and_shrink(&data)).await?
}

fn exif_thumbnail(head: &[u8]) -> Option<Vec<u8>> {
    let exif = exif::Reader::new().read_from_container(&mut Cursor::new(head)).ok()?;
    let offset = exif
        .get_field(exif::Tag::JPEGInterchangeFormat, exif::In::THUMBNAIL)?
        .value
        .get_uint(0)? as usize;
    let len = exif
        .get_field(exif::Tag::JPEGInterchangeFormatLength, exif::In::THUMBNAIL)?
        .value
        .get_uint(0)? as usize;
    let buf = exif.buf();
    let jpeg = buf.get(offset..offset.checked_add(len)?)?;
    let orientation = exif
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .unwrap_or(1);
    let img = image::load_from_memory(jpeg).ok()?;
    if img.width() < 80 || img.height() < 80 {
        return None; // useless postage stamp; fall through to a real decode
    }
    encode(apply_orientation(img, orientation)).ok()
}

fn decode_and_shrink(data: &[u8]) -> Result<Vec<u8>, AppError> {
    let orientation = exif::Reader::new()
        .read_from_container(&mut Cursor::new(data))
        .ok()
        .and_then(|e| e.get_field(exif::Tag::Orientation, exif::In::PRIMARY).and_then(|f| f.value.get_uint(0)))
        .unwrap_or(1);
    let img = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| AppError::new("decode_error", e.to_string()))?
        .decode()
        .map_err(|e| AppError::new("decode_error", e.to_string()))?;
    let img = img.thumbnail(THUMB_PX, THUMB_PX);
    encode(apply_orientation(img, orientation))
}

fn apply_orientation(img: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

fn encode(img: DynamicImage) -> Result<Vec<u8>, AppError> {
    let img = if img.width() > THUMB_PX || img.height() > THUMB_PX {
        img.thumbnail(THUMB_PX, THUMB_PX)
    } else {
        img
    };
    let rgb = img.to_rgb8();
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 82)
        .encode_image(&rgb)
        .map_err(|e| AppError::new("encode_error", e.to_string()))?;
    Ok(out)
}
