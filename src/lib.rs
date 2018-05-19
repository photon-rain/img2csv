//! The img2csv implementation.

extern crate image;
extern crate libc;
use image::{DynamicImage, GenericImage, Rgba};
mod ffi;
mod matrix;
mod swt;

use std::error::Error;
use std::path::Path;


use matrix::*;
use swt::*;

/// The minimum length of a stretch of pixels that can make up a line.
const LINE_MIN_LENGTH_PX: u32 = 50;

/// The fraction of pixels in a span of LINE_MIN_LENGTH_PX pixels
/// that must be featureful for all to be considered a solid line.
const LINE_FEATUREFUL_THRESHOLD: f32 = 0.95;

/// The boundary along the image border that should be ignored for
/// feature detection.
const FEATURE_BOUNDARY: u32 = 8;

/// The minimum height of a cell.
const CELL_MIN_HEIGHT_PX: u32 = 10;

/// The minimum width of a cell.
const CELL_MIN_WIDTH_PX: u32 = 8;


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

    let (width, height) = img.dimensions();
    if width < FEATURE_BOUNDARY || height < FEATURE_BOUNDARY {
        return features;
    }

    // Remove boundary features along the left side.
    for x in 0 .. u32::min(FEATURE_BOUNDARY, width) {
        for y in 0 .. height {
            features.put_pixel(x, y, black);
        }
    }

    // Remove boundary features along the right side.
    for x in (width - FEATURE_BOUNDARY) .. width {
        for y in 0 .. height {
            features.put_pixel(x, y, black);
        }
    }

    // Remove boundary features on the top.
    for y in 0 .. u32::min(FEATURE_BOUNDARY, height) {
        for x in 0 .. width {
            features.put_pixel(x, y, black);
        }
    }

    // Remove boundary features on the bottom.
    for y in (height - FEATURE_BOUNDARY) .. height {
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

    // For the benefit of the next phase, extend row lines all the way
    // to the left.
    // FIXME: This is a bad heuristic and should be more robust.
    for y in 0 .. height {
        if tmp.get_pixel(FEATURE_BOUNDARY + 1, y) != black {
            for x in 0 .. (FEATURE_BOUNDARY + 1) {
                tmp.put_pixel(x, y, magic);
            }
        }
    }

    tmp
}

#[derive(Debug)]
pub struct Cell {
    /// Row position in the image, with the top row beginning at 0.
    pub row: u32,
    /// Column position in the image, with the left column beginning at 0.
    /// Column number is for the given row.
    /// Different rows may have different column counts.
    pub col: u32,

    /// Position information for the Cell within the underlying image.
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

fn detect_cells_in_row(acc: &mut Vec<Cell>,
                       lines: &DynamicImage,
                       cur_row: u32,
                       y_top: u32,
                       y_bottom: u32)
{
    let (width, _) = lines.dimensions();
    let black = Rgba([0,0,0,255]);

    // All cells in this row have the same vertical characteristics.
    let cell_y = y_top;
    let cell_height = y_bottom - y_top - 1;

    // Y-position at which to test for vertical lines.
    let cell_y_median = (cell_y) + (cell_height / 2);

    // March to the right. If a line (or boundary) is encountered,
    // create a new Cell.
    let mut cur_col: u32 = 0;
    let mut prev_x = 0;
    let mut x = CELL_MIN_WIDTH_PX - 1;

    while x < width {
        // Line encountered! Make a Cell.
        if lines.get_pixel(x, cell_y_median) != black {
            // Use current values to produce a Cell.
            let cell = Cell {
                row: cur_row,
                col: cur_col,
                x: prev_x,
                y: cell_y,
                width: (x - prev_x - 1),
                height: cell_height,
            };

            acc.push(cell);

            // Update cursor.
            prev_x = x + 1;
            cur_col += 1;

            // New column defined: ignore lines within CELL_MIN_WIDTH_PX.
            x += CELL_MIN_WIDTH_PX;
        } else {
            // Check the next pixel for a line.
            x += 1;
        }
    }

    // Make a final cell with the border wall.
    if prev_x + CELL_MIN_WIDTH_PX < width {
        let cell = Cell {
            row: cur_row,
            col: cur_col,
            x: prev_x,
            y: cell_y,
            width: (width - prev_x - 1),
            height: cell_height,
        };
        acc.push(cell);
    }
}

/// Given an image with only lines, get a list of Cells.
fn detect_cells(lines: &DynamicImage) -> Vec<Cell> {
    let (_, height) = lines.dimensions();
    let black = Rgba([0,0,0,255]);

    // The final vector to be returned.
    let mut acc = Vec::<Cell>::new();

    // Current row and column information.
    let mut cur_row: u32 = 0;

    // The y-coordinate for the current row.
    let mut prev_y = 0;

    // The previous phase extended lines all the way to the left, so we
    // need only consider the leftmost column of pixels.
    let mut y = CELL_MIN_HEIGHT_PX - 1;
    while y < height {
        // If this pixel defines the bottom of a new row,
        if lines.get_pixel(0, y) != black || y == (height-1) {
            detect_cells_in_row(&mut acc, &lines, cur_row, prev_y, y);

            // End of row processing: skip by CELL_MIN_HEIGHT_PX.
            prev_y = y + 1;
            y += CELL_MIN_HEIGHT_PX;
            cur_row += 1;
        } else {
            // No row found: check the next pixel.
            y += 1;
        }
    }

    // Make a final row with the border wall.
    if prev_y + CELL_MIN_HEIGHT_PX < height {
        detect_cells_in_row(&mut acc, &lines, cur_row, prev_y, height - 1);
    }

    acc
}


pub fn get_cells(img: &DynamicImage) -> Vec<Cell> {
    let features = detect_features(&img);
    let lines = detect_lines(&features);
    detect_cells(&lines)
}


pub fn run(config: Config) -> Result<(), Box<Error>> {
    let mut img: DynamicImage = image::open(Path::new(&config.filename))?;

    let mut pix = Matrix::read(&config.filename, matrix::OpenAs::ToGray).expect("Could not read image");
    let words = pix.detect_words(Default::default());

    for cell in get_cells(&img) {
        let subimg = img.sub_image(cell.x, cell.y, cell.width, cell.height);
        let subimg2 = subimg.to_image();

        let dynimg = DynamicImage::ImageRgba8(subimg2).grayscale();

        dump(&dynimg, &format!("{}-{}.png", cell.row, cell.col));
    }

    Ok(())
}
