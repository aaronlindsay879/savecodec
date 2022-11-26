use binformat::format_source;
use savecodec::decode_to_raw;

#[format_source("save.format")]
pub struct Save;

fn main() {
    let save = std::fs::read_to_string("save.txt").unwrap();

    let raw = decode_to_raw(&save).unwrap();
    let save = Save::read(&mut raw.as_slice()).unwrap();
    println!("{:?}", save.buildings);
}
