extern crate img2csv;
extern crate image;

use img2csv::Cell;
use image::DynamicImage;
use std::path::Path;


/// Asserts that each row of cells has the expected number of columns.
/// The cells vector must be sorted by (row, col).
fn assert_row_lengths(cells: &Vec<Cell>, lengths: &Vec<u32>) {
    let mut cur_row = 0;
    let mut col_count = 0;

    for cell in cells {
        if cell.row == cur_row {
            col_count += 1;
        } else {
            assert_eq!(col_count, lengths[cur_row as usize]);
            col_count = 1;
            cur_row += 1;
        }
    }

    assert_eq!(col_count, lengths[cur_row as usize]);
    assert_eq!(cur_row + 1, lengths.len() as u32);
}


#[test]
fn test_gpc_aus_act_winter_classic_png() {
    let filename = "tests/regression/gpc-aus-act-winter-classic.png";

    let img: DynamicImage = image::open(Path::new(&filename)).unwrap();
    let cells = img2csv::get_cells(&img);

    let row_lengths = vec!(2,23,23,23,23,23,23,23,23,23,23,23,23,23,23);
    assert_row_lengths(&cells, &row_lengths);
}
