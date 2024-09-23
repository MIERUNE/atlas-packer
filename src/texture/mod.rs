use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

use image::{DynamicImage, GenericImageView, ImageBuffer};
use rayon::prelude::*;
use utils::{calc_bbox, uv_to_pixel_coords};

pub mod cache;
mod utils;

#[derive(Debug, Clone)]
pub struct DownsampleFactor(f32);

impl DownsampleFactor {
    pub fn new(factor: &f32) -> Self {
        if (0.0..=1.0).contains(factor) {
            DownsampleFactor(*factor)
        } else {
            panic!("The argument must be entered between 0~1.") //FIXME: panic! is not recommended
        }
    }

    pub fn value(&self) -> f32 {
        self.0
    }
}

/// Texture with mapped polygon.
#[derive(Debug, Clone)]
pub struct PolygonMappedTexture {
    // texture
    pub image_path: PathBuf,
    pub downsample_factor: DownsampleFactor,
    // polygon
    pub pixel_coords: Vec<(u32, u32)>,
}

impl PolygonMappedTexture {
    pub fn new(
        image_path: &Path,
        size: (u32, u32),
        uv_coords: &[(f64, f64)],
        downsample_factor: DownsampleFactor,
    ) -> Self {
        let pixel_coords = uv_to_pixel_coords(uv_coords, size.0, size.1);

        PolygonMappedTexture {
            image_path: image_path.to_path_buf(),
            downsample_factor,
            pixel_coords,
        }
    }

    pub fn bbox_overlaps(&self, other: &Self) -> bool {
        if self.image_path != other.image_path {
            return false;
        }

        let (min_x_0, min_y_0, max_x_0, max_y_0) = calc_bbox(&self.pixel_coords);
        let (min_x_1, min_y_1, max_x_1, max_y_1) = calc_bbox(&other.pixel_coords);

        !(max_x_0 < min_x_1 || max_x_1 < min_x_0 || max_y_0 < min_y_1 || max_y_1 < min_y_0)
    }

    pub fn get_cropped_uv_coords(
        &self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Vec<(f64, f64)> {
        self.pixel_coords
            .iter()
            .map(|(px, py)| {
                (
                    (*px - x) as f64 / width as f64,
                    1.0 - (*py - y) as f64 / height as f64,
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ToplevelTexture {
    pub image_path: PathBuf,
    // The origin of the cropped image in the original image (top-left corner).
    crop_bbox: (u32, u32, u32, u32),
    pub downsample_factor: DownsampleFactor,
}

impl ToplevelTexture {
    pub fn new(texture: &PolygonMappedTexture) -> Self {
        let bounding_box = calc_bbox(&texture.pixel_coords);
        Self {
            image_path: texture.image_path.clone(),
            crop_bbox: bounding_box,
            downsample_factor: texture.downsample_factor.clone(),
        }
    }

    pub fn expand(&self, texture: &PolygonMappedTexture) -> Option<Self> {
        if self.image_path != texture.image_path {
            return None;
        }

        let bounding_box_0 = calc_bbox(&texture.pixel_coords);
        let (min_x_0, min_y_0, max_x_0, max_y_0) = bounding_box_0;

        let (min_x_1, min_y_1, max_x_1, max_y_1) = self.crop_bbox;

        let (min_x_new, min_y_new) = (min_x_0.min(min_x_1), min_y_0.min(min_y_1));
        let (max_x_new, max_y_new) = (max_x_0.max(max_x_1), max_y_0.max(max_y_1));
        Some(Self {
            image_path: texture.image_path.clone(),
            crop_bbox: (min_x_new, min_y_new, max_x_new, max_y_new),
            downsample_factor: DownsampleFactor::new(
                &self
                    .downsample_factor
                    .value()
                    .max(texture.downsample_factor.value()),
            ),
        })
    }

    pub fn get_child(&self, texture: &PolygonMappedTexture) -> ChildTexture {
        let (min_x, min_y, max_x, max_y) = (
            self.crop_bbox.0,
            self.crop_bbox.1,
            self.crop_bbox.2,
            self.crop_bbox.3,
        );
        let cropped_uv_coords =
            texture.get_cropped_uv_coords(min_x, min_y, max_x - min_x, max_y - min_y);
        ChildTexture { cropped_uv_coords }
    }

    pub fn width(&self) -> u32 {
        self.crop_bbox.2 - self.crop_bbox.0
    }

    pub fn height(&self) -> u32 {
        self.crop_bbox.3 - self.crop_bbox.1
    }

    pub fn crop(&self, image: &DynamicImage) -> DynamicImage {
        let (x, y) = (self.crop_bbox.0, self.crop_bbox.1);
        let cropped_image = image.view(x, y, self.width(), self.height()).to_image();

        // Collect pixels into a Vec and then process in parallel
        let pixels: Vec<_> = cropped_image.enumerate_pixels().collect();

        let samples = 1;
        let num_threads = rayon::current_num_threads();
        let chunk_size = (pixels.len() / num_threads).clamp(1, pixels.len() + 1);

        let (sender, receiver) = mpsc::channel();

        // If the center coordinates of the pixel are contained within a polygon composed of UV coordinates, the pixel is written
        pixels
            .par_chunks(chunk_size)
            .for_each_with(sender, |s, chunk| {
                let mut local_results = Vec::new();

                for &(px, py, pixel) in chunk {
                    let mut is_inside = false;

                    'subpixels: for sx in 0..samples {
                        for sy in 0..samples {
                            let x = (px as f64 + (sx as f64 + 0.5) / samples as f64)
                                / self.width() as f64;
                            let y = 1.0
                                - (py as f64 + (sy as f64 + 0.5) / samples as f64)
                                    / self.height() as f64;
                            // Adjust x and y to the center of the pixel
                            let center_x = x + 0.5 / self.width() as f64;
                            let center_y = y - 0.5 / self.height() as f64;

                            // TODO !!!
                            if
                            /*is_point_inside_polygon(
                                (center_x, center_y),
                                &self.cropped_uv_coords,
                            )*/
                            true {
                                is_inside = true;
                                break 'subpixels;
                            }
                        }
                    }

                    if is_inside {
                        local_results.push((px, py, *pixel));
                    } else {
                        // FIXME: Do not crop temporarily because pixel boundary jaggies will occur.
                        local_results.push((px, py, *pixel));
                    }
                }

                s.send(local_results).unwrap();
            });

        // Collect results in the main thread
        let mut clipped = ImageBuffer::new(self.width(), self.height());
        for received in receiver {
            for (px, py, pixel) in received {
                clipped.put_pixel(px, py, pixel);
            }
        }

        // Downsample
        let scaled_width = (clipped.width() as f32 * self.downsample_factor.value()) as u32;
        let scaled_height = (clipped.height() as f32 * self.downsample_factor.value()) as u32;

        DynamicImage::ImageRgba8(image::imageops::resize(
            &clipped,
            scaled_width,
            scaled_height,
            image::imageops::FilterType::Triangle,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct ChildTexture {
    // UV coordinates for the toplevel texture (bottom-left origin).
    pub cropped_uv_coords: Vec<(f64, f64)>,
}
