use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use rayon::prelude::*;

use atlas_packer::{
    export::JpegAtlasExporter,
    pack::AtlasPacker,
    place::{GuillotineTexturePlacer, TexturePlacerConfig},
    texture::{
        cache::{TextureCache, TextureSizeCache},
        CroppedTexture, DownsampleFactor,
    },
};

#[derive(Debug, Clone)]
struct Polygon {
    id: String,
    uv_coords: Vec<(f64, f64)>,
    texture_uri: PathBuf,
    downsample_factor: DownsampleFactor,
}

fn random_in_range(min: f64, max: f64) -> f64 {
    min + (max - min) * rand::random::<f64>()
}

fn main() {
    let all_process_start = Instant::now();

    // 3D Tiles Sink passes the texture path and UV coordinates for each polygon
    let mut polygons: Vec<Polygon> = Vec::new();
    let downsample_factor = 1.0;
    for i in 0..200 {
        for j in 1..11 {
            // Specify a polygon to crop around the center of the image

            // generate random polygon
            let edge_radius = 0.3;
            let center_x = random_in_range(edge_radius, 1.0 - edge_radius);
            let center_y = random_in_range(edge_radius, 1.0 - edge_radius);

            let num_points = rand::random::<usize>() % 10 + 3;
            let mut radians = (0..num_points)
                .map(|_| random_in_range(0.0, 6.28))
                .collect::<Vec<f64>>();
            radians.sort_by(|a, b| a.total_cmp(b));

            let uv_coords = radians
                .iter()
                .map(|radian| {
                    let radius = random_in_range(edge_radius * 0.1, edge_radius);
                    let x = center_x + radius * radian.cos();
                    let y = center_y + radius * radian.sin();
                    (x, y)
                })
                .collect::<Vec<(f64, f64)>>();

            let path_string: String = format!("./examples/assets/{}.png", j);
            let image_path = PathBuf::from(path_string.as_str());
            polygons.push(Polygon {
                id: format!("texture_{}_{}", i, j),
                uv_coords,
                texture_uri: image_path,
                downsample_factor: DownsampleFactor::new(&downsample_factor),
            });
        }
    }

    // initialize texture packer
    let config = TexturePlacerConfig {
        width: 4096,
        height: 4096,
        padding: 0,
    };

    let packer = Mutex::new(AtlasPacker::default());

    let packing_start = Instant::now();

    // cache image size
    let texture_size_cache = TextureSizeCache::new();
    // place textures on the atlas
    polygons.par_iter().for_each(|polygon| {
        let place_start = Instant::now();
        let texture_size = texture_size_cache.get_or_insert(&polygon.texture_uri);
        let cropped_texture = CroppedTexture::new(
            &polygon.texture_uri,
            texture_size,
            &polygon.uv_coords,
            polygon.downsample_factor.clone(),
        );

        let _ = packer
            .lock()
            .unwrap()
            .add_texture(polygon.id.clone(), cropped_texture);
        let place_duration = place_start.elapsed();
        println!("{}, texture place process {:?}", polygon.id, place_duration);
    });

    let packer = packer.into_inner().unwrap();
    let packed = packer.pack(GuillotineTexturePlacer::new(config.clone()));

    let duration = packing_start.elapsed();
    println!("all packing process {:?}", duration);

    let start = Instant::now();

    // Caches the original textures for exporting to an atlas.
    let texture_cache = TextureCache::new(100_000_000);
    let output_dir = Path::new("./examples/output/");

    packed.export(
        JpegAtlasExporter::default(),
        output_dir,
        &texture_cache,
        config.width(),
        config.height(),
    );
    let duration = start.elapsed();
    println!("all atlas export process {:?}", duration);

    let duration = all_process_start.elapsed();
    println!("all process {:?}", duration);
}
