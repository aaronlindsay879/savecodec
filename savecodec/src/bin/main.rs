use binformat::format_source;
use savecodec::decode_to_raw;

#[format_source("save.format")]
pub struct Save;

fn main() {
    let save = std::fs::read_to_string("save.txt").unwrap();

    let raw = decode_to_raw(&save).unwrap();

    let save_decoded = Save::read(&mut raw.as_slice()).unwrap();
    let mut buf = Vec::new();
    save_decoded.write(&mut buf);

    let save2_decoded = Save::read(&mut buf.as_slice()).unwrap();
    println!(
        "encoded-decoded equality: {}",
        save_decoded == save2_decoded
    );
}
