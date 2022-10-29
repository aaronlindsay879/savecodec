use savecodec::decode_to_raw;

fn main() {
    let save = std::fs::read_to_string("save.txt").unwrap();

    println!("{:?}", decode_to_raw(&save));
}
