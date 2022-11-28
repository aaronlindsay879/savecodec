use savecodec::Save;

fn main() {
    let save_string = std::fs::read_to_string("save.txt").unwrap();

    let save_decoded = Save::parse_str(&save_string).unwrap();
    let new_str = save_decoded.to_str().unwrap();
    let new_decoded = Save::parse_str(&new_str).unwrap();

    println!("{}", save_decoded == new_decoded);
}
