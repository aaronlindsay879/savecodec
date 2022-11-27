use binformat::format_source;

#[format_source("save.format")]
pub struct Save;

impl Save {
    pub fn from_str(save: &str) -> Option<Self> {
        let raw = savecodec::decode_to_raw(save).ok()?;

        Save::read(&mut raw.as_slice())
    }

    pub fn to_str(&self) -> Option<String> {
        let mut raw = Vec::new();
        self.write(&mut raw)?;

        let checksum = crc32fast::hash(&raw).to_be_bytes();
        raw.extend_from_slice(&checksum);

        savecodec::encode_from_raw(&raw, self.save_version).ok()
    }
}

fn main() {
    let save_string = std::fs::read_to_string("save.txt").unwrap();

    let save_decoded = Save::from_str(&save_string).unwrap();
    let new_str = save_decoded.to_str().unwrap();
    let new_decoded = Save::from_str(&new_str).unwrap();

    println!("{}", save_decoded == new_decoded);
}
