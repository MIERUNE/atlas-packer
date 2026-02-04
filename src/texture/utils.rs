use std::path::Path;

use image::ImageReader;

use crate::file_reader::FileReader;

#[allow(dead_code)]
pub fn is_point_inside_polygon(test_point: (f64, f64), polygon: &[(f64, f64)]) -> bool {
    let mut is_inside = false;
    let mut previous_vertex_index = polygon.len() - 1;

    for current_vertex_index in 0..polygon.len() {
        let (current_x, current_y) = polygon[current_vertex_index];
        let (previous_x, previous_y) = polygon[previous_vertex_index];

        let is_y_between_vertices = (current_y > test_point.1) != (previous_y > test_point.1);
        let does_ray_intersect = test_point.0
            < (previous_x - current_x) * (test_point.1 - current_y) / (previous_y - current_y)
                + current_x;

        if is_y_between_vertices && does_ray_intersect {
            is_inside = !is_inside;
        }

        previous_vertex_index = current_vertex_index;
    }

    is_inside
}

pub fn get_image_size<P: AsRef<Path>>(file_path: P) -> Result<(u32, u32), image::ImageError> {
    let path = file_path.as_ref();
    if let Some(path_str) = path.to_str() {
        if FileReader::is_zip_path(path_str) {
            let file_reader = FileReader::open(path_str).map_err(image::ImageError::IoError)?;
            let reader = ImageReader::new(file_reader).with_guessed_format()?;
            return Ok(reader.into_dimensions()?);
        }
    }

    let reader = ImageReader::open(path)?;
    Ok(reader.into_dimensions()?)
}

pub fn uv_to_pixel_coords(uv_coords: &[(f64, f64)], width: u32, height: u32) -> Vec<(u32, u32)> {
    uv_coords
        .iter()
        .map(|(u, v)| {
            (
                (u.clamp(0.0, 1.0) * width as f64).min(width as f64 - 1.0) as u32,
                ((1.0 - v.clamp(0.0, 1.0)) * height as f64).min(height as f64 - 1.0) as u32,
            )
        })
        .collect()
}

#[inline]
pub fn calc_bbox(pixel_coords: &[(u32, u32)]) -> (u32, u32, u32, u32) {
    pixel_coords.iter().fold(
        (u32::MAX, u32::MAX, 0, 0),
        |(min_x, min_y, max_x, max_y), (x, y)| {
            (min_x.min(*x), min_y.min(*y), max_x.max(*x), max_y.max(*y))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::get_image_size;
    use image::{DynamicImage, ImageFormat};
    use std::fs::File;
    use std::io::{Cursor, Write};
    use tempfile::tempdir;
    use zip::write::FileOptions;
    use zip::ZipWriter;

    #[test]
    fn get_image_size_from_zip_png() {
        let dir = tempdir().expect("failed to create temp dir");
        let zip_path = dir.path().join("assets.zip");
        let inner_name = "inner.png";

        let img = DynamicImage::new_rgba8(8, 8);
        let mut png_bytes = Vec::new();
        img.write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
            .expect("failed to encode png");

        let file = File::create(&zip_path).expect("failed to create zip file");
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::<()>::default();
        zip.start_file(inner_name, options)
            .expect("failed to start zip entry");
        zip.write_all(&png_bytes)
            .expect("failed to write png to zip");
        zip.finish().expect("failed to finish zip");

        let path_in_zip = format!("{}/{}", zip_path.display(), inner_name);
        let (width, height) = get_image_size(path_in_zip).expect("failed to read size from zip");
        assert_eq!((width, height), (8, 8));
    }
}
