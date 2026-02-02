use std::path::Path;
use std::sync::Mutex;

use hashbrown::HashMap;
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb, Rgba};
use rayon::prelude::*;
use thiserror::Error;

use crate::{
    place::PlacedTextureGeometry,
    texture::{cache::TextureCache, ClusterBoundingTexture},
    ClusterID,
};

#[derive(Error, Debug)]
pub enum ExportError {
    #[error("Image error: {0}")]
    ImageError(#[from] image::ImageError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("WebP encoding failed: {0}")]
    WebpError(String),

    #[error("Texture not found for cluster: {0}")]
    TextureNotFound(String),

    #[error("Failed to convert image to expected format for cluster: {0}")]
    ImageConversionError(String),
}

pub trait AtlasExporter: Sync + Send {
    fn export(
        &self,
        atlas_data: &[PlacedTextureGeometry],
        textures: &HashMap<ClusterID, ClusterBoundingTexture>,
        output_path: &Path,
        texture_cache: &TextureCache,
        width: u32,
        height: u32,
    ) -> Result<(), ExportError>;

    fn get_extension(&self) -> &str;
    fn get_image_format(&self) -> ImageFormat;
}

#[derive(Clone)]
pub struct WebpAtlasExporter {
    pub ext: String,
}

impl Default for WebpAtlasExporter {
    fn default() -> Self {
        WebpAtlasExporter {
            ext: "webp".to_string(),
        }
    }
}

impl AtlasExporter for WebpAtlasExporter {
    fn get_extension(&self) -> &str {
        &self.ext
    }

    fn get_image_format(&self) -> ImageFormat {
        ImageFormat::WebP
    }

    fn export(
        &self,
        atlas_data: &[PlacedTextureGeometry],
        textures: &HashMap<ClusterID, ClusterBoundingTexture>,
        output_path: &Path,
        texture_cache: &TextureCache,
        width: u32,
        height: u32,
    ) -> Result<(), ExportError> {
        let output_path = output_path.with_extension(self.get_extension());

        let atlas_image = create_atlas_rgba(atlas_data, textures, texture_cache, width, height)?;
        let binding = DynamicImage::ImageRgba8(atlas_image);
        let webp_encoder = webp::Encoder::from_image(&binding)
            .map_err(|e| ExportError::WebpError(e.to_string()))?;
        let webp = webp_encoder.encode(75.0);
        std::fs::write(output_path, &*webp)?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct PngAtlasExporter {
    pub ext: String,
}

impl Default for PngAtlasExporter {
    fn default() -> Self {
        PngAtlasExporter {
            ext: "png".to_string(),
        }
    }
}

impl AtlasExporter for PngAtlasExporter {
    fn get_extension(&self) -> &str {
        &self.ext
    }

    fn get_image_format(&self) -> ImageFormat {
        ImageFormat::Png
    }

    fn export(
        &self,
        atlas_data: &[PlacedTextureGeometry],
        textures: &HashMap<ClusterID, ClusterBoundingTexture>,
        output_path: &Path,
        texture_cache: &TextureCache,
        width: u32,
        height: u32,
    ) -> Result<(), ExportError> {
        let atlas_image = create_atlas_rgba(atlas_data, textures, texture_cache, width, height)?;
        let output_path = output_path.with_extension(self.get_extension());
        atlas_image.save_with_format(output_path, self.get_image_format())?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct JpegAtlasExporter {
    pub ext: String,
}

impl Default for JpegAtlasExporter {
    fn default() -> Self {
        JpegAtlasExporter {
            ext: "jpg".to_string(),
        }
    }
}

impl AtlasExporter for JpegAtlasExporter {
    fn get_extension(&self) -> &str {
        &self.ext
    }

    fn get_image_format(&self) -> ImageFormat {
        ImageFormat::Jpeg
    }

    fn export(
        &self,
        atlas_data: &[PlacedTextureGeometry],
        textures: &HashMap<ClusterID, ClusterBoundingTexture>,
        output_path: &Path,
        texture_cache: &TextureCache,
        width: u32,
        height: u32,
    ) -> Result<(), ExportError> {
        let atlas_image =
            create_atlas_image_rgb(atlas_data, textures, texture_cache, width, height)?;
        let output_path = output_path.with_extension(self.get_extension());
        atlas_image.save_with_format(output_path, self.get_image_format())?;
        Ok(())
    }
}

fn create_atlas_rgba(
    atlas_data: &[PlacedTextureGeometry],
    textures: &HashMap<ClusterID, ClusterBoundingTexture>,
    texture_cache: &TextureCache,
    width: u32,
    height: u32,
) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, ExportError> {
    let atlas_image = Mutex::new(ImageBuffer::new(width, height));

    atlas_data
        .par_iter()
        .try_for_each(|info| -> Result<(), ExportError> {
            let texture = textures
                .get(&info.cluster_id)
                .ok_or_else(|| ExportError::TextureNotFound(info.cluster_id.clone()))?;
            let cropped = texture.crop(&texture_cache.get_image(&texture.image_path));
            let image = cropped
                .as_rgba8()
                .ok_or_else(|| ExportError::ImageConversionError(info.cluster_id.clone()))?;

            let mut atlas_image = atlas_image.lock().unwrap();
            for (x, y, pixel) in image.enumerate_pixels() {
                let atlas_x = info.origin.0 + x;
                let atlas_y = info.origin.1 + y;
                atlas_image.put_pixel(atlas_x, atlas_y, *pixel);
            }
            Ok(())
        })?;

    Ok(atlas_image.into_inner().unwrap())
}

fn create_atlas_image_rgb(
    atlas_data: &[PlacedTextureGeometry],
    textures: &HashMap<ClusterID, ClusterBoundingTexture>,
    texture_cache: &TextureCache,
    width: u32,
    height: u32,
) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>, ExportError> {
    let atlas_image = Mutex::new(ImageBuffer::new(width, height));

    atlas_data
        .par_iter()
        .try_for_each(|info| -> Result<(), ExportError> {
            let texture = textures
                .get(&info.cluster_id)
                .ok_or_else(|| ExportError::TextureNotFound(info.cluster_id.clone()))?;
            let cropped = texture.crop(&texture_cache.get_image(&texture.image_path));
            let image = cropped.to_rgb8();

            let mut atlas_image = atlas_image.lock().unwrap();
            for (x, y, pixel) in image.enumerate_pixels() {
                let atlas_x = info.origin.0 + x;
                let atlas_y = info.origin.1 + y;
                atlas_image.put_pixel(atlas_x, atlas_y, *pixel);
            }
            Ok(())
        })?;

    Ok(atlas_image.into_inner().unwrap())
}
