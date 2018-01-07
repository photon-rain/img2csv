//! The img2csv implementation.

extern crate image;
use image::{DynamicImage, GenericImage, Rgba};

use std::error::Error;
use std::path::Path;


/// The minimum length of a stretch of pixels that can make up a line.
const LINE_MIN_LENGTH_PX: u32 = 50;

/// The fraction of pixels in a span of LINE_MIN_LENGTH_PX pixels
/// that must be featureful for all to be considered a solid line.
const LINE_FEATUREFUL_THRESHOLD: f32 = 0.95;


/// Passes runtime configiration options.
pub struct Config {
    pub filename: String,
}

impl Config {
    pub fn new(mut args: std::env::Args) -> Result<Config, &'static str> {
        args.next();

        let filename = match args.next() {
            Some(arg) => arg,
            None => return Err("Missing filename argument."),
        };

        Ok(Config { filename })
    }
}


fn dump(img: &DynamicImage, filename: &str) {
    let mut file = std::fs::File::create(filename).unwrap();
    img.save(&mut file, image::PNG).unwrap();
}


/// Determines whether the pixel is sufficiently dark
/// to be considered part of a line.
#[inline]
fn could_be_feature(px: &Rgba<u8>) -> bool {
    const THRESHOLD: u8 = 130;
    px.data[0] <= THRESHOLD && px.data[1] <= THRESHOLD && px.data[2] <= THRESHOLD
}


/// Given an image, return another image where all
/// pixels that could be features have a magic color,
/// and where non-features are black.
fn detect_features(img: &DynamicImage) -> DynamicImage {
    // Grayscale is used to detect changes in brightness.
    let gray = img.grayscale();
    let mut features = img.clone();
    let black = Rgba([0, 0, 0, 255]);
    let magic = Rgba([255, 0, 255, 255]);

    // Only keeps pixels below a certain darkness.
    // Assumes that the text and lines will be black.
    for (x, y, px) in gray.pixels() {
        let sub = if could_be_feature(&px) { magic } else { black };
        features.put_pixel(x, y, sub);
    }

    const BOUNDARY: u32 = 8;

    let (width, height) = img.dimensions();
    if width < BOUNDARY || height < BOUNDARY {
        return features;
    }

    // Remove boundary features along the left side.
    for x in 0 .. u32::min(BOUNDARY, width) {
        for y in 0 .. height {
            features.put_pixel(x, y, black);
        }
    }

    // Remove boundary features along the right side.
    for x in (width - BOUNDARY) .. width {
        for y in 0 .. height {
            features.put_pixel(x, y, black);
        }
    }

    // Remove boundary features on the top.
    for y in 0 .. u32::min(BOUNDARY, height) {
        for x in 0 .. width {
            features.put_pixel(x, y, black);
        }
    }

    // Remove boundary features on the bottom.
    for y in (height - BOUNDARY) .. height {
        for x in 0 .. width {
            features.put_pixel(x, y, black);
        }
    }

    features
}


fn is_line_to_right(features: &DynamicImage, x: u32, y: u32) -> bool {
    let mut count = 0;
    let black = Rgba([0,0,0,255]);

    // Count the number of featureful pixels.
    for k in x .. (x + LINE_MIN_LENGTH_PX) {
        if features.get_pixel(k, y) != black {
            count += 1;
        }
    }

    // If the number of featureful pixels was above a certain threshold,
    // it was probably a line.
    (count as f32) / (LINE_MIN_LENGTH_PX as f32) >= LINE_FEATUREFUL_THRESHOLD
}


fn is_line_downward(features: &DynamicImage, x: u32, y: u32) -> bool {
    let mut count = 0;
    let black = Rgba([0,0,0,255]);

    // Count the number of featureful pixels.
    for k in y .. (y + LINE_MIN_LENGTH_PX) {
        if features.get_pixel(x, k) != black {
            count += 1;
        }
    }

    // If the number of featureful pixels was above a certain threshold,
    // it was probably a line.
    (count as f32) / (LINE_MIN_LENGTH_PX as f32) >= LINE_FEATUREFUL_THRESHOLD
}


/// Reduce features to just those that are probably in lines.
fn detect_lines(features: &DynamicImage) -> DynamicImage {
    let magic = Rgba([255,0,0,255]);
    let black = Rgba([0,0,0,255]);
    let (width, height) = features.dimensions();

    let mut tmp = DynamicImage::new_rgba8(width, height);
    for x in 0 .. width {
        for y in 0 .. height {
            tmp.put_pixel(x, y, black);
        }
    }

    // Scan for horizontal line segments.
    // Lines are always scanned to the right, or downward.
    for (x, y, px) in features.pixels() {
        // Only investigate features.
        if px == black {
            continue;
        }

        // If there is a line to the right, color all those pixels.
        if x < width - LINE_MIN_LENGTH_PX {
            if is_line_to_right(features, x, y) {
                for k in x .. (x + LINE_MIN_LENGTH_PX) {
                    tmp.put_pixel(k, y, magic);
                }
            }
        }

        // If there is a line downward, color all those pixels.
        if y < height - LINE_MIN_LENGTH_PX {
            if is_line_downward(features, x, y) {
                for k in y .. (y + LINE_MIN_LENGTH_PX) {
                    tmp.put_pixel(x, k, magic);
                }
            }
        }
    }

    tmp
}


pub fn run(config: Config) -> Result<(), Box<Error>> {
    let img: DynamicImage = image::open(Path::new(&config.filename))?;

    let features = detect_features(&img);
    let lines = detect_lines(&features);

    dump(&features, "features.png");
    dump(&lines, "lines.png");

    Ok(())
}
