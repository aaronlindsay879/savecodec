## This repo contains two crates:
* [binformat](#binformat)
* [savecodec](#savecodec)

## binformat
Binformat is a crate for generating code to convert binary data to and from a struct defined in yaml. As a simple example, imagine the following yaml format:
```yaml
items:
  - id: a
    type: u16
  - id: b
    type: u64
```
By defining a struct as follows:
```rust
#[format_source("example.format")]
struct Example;
``` 
It will generate the following type and implementation block (the context can be ignored in this situation, as it is used to support conditional statements for composite types):
```rust
struct Example {
    pub a: u16,
    pub b: u64,
}

impl Example {
    pub fn read<R: ::byteorder::ReadBytesExt>(
        reader: &mut R,
    ) -> ::std::io::Result<Self> {
        let a = reader.read_u16::<::byteorder::LittleEndian>()?;
        let b = reader.read_u64::<::byteorder::LittleEndian>()?;
        let _root = ExampleContext { a, b };
        Ok(Self { a, b })
    }
    pub fn write<W: ::byteorder::WriteBytesExt>(
        &self,
        writer: &mut W,
    ) -> ::std::io::Result<()> {
        writer.write_u16::<::byteorder::LittleEndian>(self.a)?;
        writer.write_u64::<::byteorder::LittleEndian>(self.b)?;
        Ok(())
    }
}
```

While this is a simple example, the crate supports more features (which can all be used together as needed) which makes it powerful for a range of uses:
* [Simple types](#simple-types)
* [User defined types](#composite-user-defined-types)
* [Conditional types](#conditional-types)
* [Repeated types](#repeated-types)

##### Simple types
* Signed and unsigned integers (u8 to u64, i8 to i64)
* Boolean (with true defined as 1 and false as 0)
* Floats (f32 and f64)

##### Composite, user defined types
You can define your own types with creating a `types` entry in the config file as follows:
```yaml
types:
  upgrade:
    - id: id
      type: u32
    - id: u1
      type: bool
    - id: u2
      type: bool
    - id: u3
      type: bool
    - id: rng_state
      type: u32
```
These types can then be used in any situation where a type could be used, for example:
```yaml
items:
  - id: a
    type: upgrade
  - id: b
    type: upgrade
```
This will generate code that simply reads/writes two upgrades (where the upgrade read/writers work as shown in the first example), as follows:
```rust
pub fn read<R: ::byteorder::ReadBytesExt>(
    reader: &mut R,
) -> ::std::io::Result<Self> {
    let _root = ExampleContext {};
    let a = upgrade::read(reader, &_root)?;
    let b = upgrade::read(reader, &_root)?;
    Ok(Self { a, b })
}
pub fn write<W: ::byteorder::WriteBytesExt>(
    &self,
    writer: &mut W,
) -> ::std::io::Result<()> {
    self.a.write(writer)?;
    self.b.write(writer)?;
    Ok(())
}
```

##### Conditional types
If you have a value that might not exist in every piece of data you're parsing, you can create a conditional type as follows:
```yaml
items:
  - id: save_version
    type: u64
  - id: item
    type: u64
    if: 'save_version > 1'
```
This will generate code that only reads the value if save_version is above 1, and only writes if it exists in the data you're parsing:
```rust
pub fn read<R: ::byteorder::ReadBytesExt>(
    reader: &mut R,
) -> ::std::io::Result<Self> {
    let save_version = reader.read_u64::<::byteorder::LittleEndian>()?;
    let _root = ExampleContext { save_version };
    let item = if save_version > 1 {
        Some(reader.read_u64::<::byteorder::LittleEndian>()?)
    } else {
        None
    };
    Ok(Self { save_version, item })
}
pub fn write<W: ::byteorder::WriteBytesExt>(
    &self,
    writer: &mut W,
) -> ::std::io::Result<()> {
    writer.write_u64::<::byteorder::LittleEndian>(self.save_version)?;
    if let Some(item) = self.item {
        writer.write_u64::<::byteorder::LittleEndian>(item)?
    }
    Ok(())
}
```
This is where the `_root` context variable comes in handy - if you were to then try and parse a composite type, it would be passed to that type such that it could also be conditional on values in the parent type (such as `_root.save_version > 1`)

##### Repeated types
If you want to read/write a variable a number of times depending on something else parsed, you can create a config file as follows:
```yaml
items:
  - id: number
    type: u64
  - id: values
    type: u64
    repeat: Count(number)
```
This first reads a number, and then reads however many values that specified - which is shown in the generated code:
```rust
pub fn read<R: ::byteorder::ReadBytesExt>(
    reader: &mut R,
) -> ::std::io::Result<Self> {
    let number = reader.read_u64::<::byteorder::LittleEndian>()?;
    let _root = ExampleContext { number };
    let values = (0..number)
        .map(|_| reader.read_u64::<::byteorder::LittleEndian>())
        .collect::<::std::io::Result<Vec<_>>>()?;
    Ok(Self { number, values })
}
pub fn write<W: ::byteorder::WriteBytesExt>(
    &self,
    writer: &mut W,
) -> ::std::io::Result<()> {
    writer.write_u64::<::byteorder::LittleEndian>(self.number)?;
    self.values
        .iter()
        .map(|values| writer.write_u64::<::byteorder::LittleEndian>(values))
        .collect::<::std::io::Result<Vec<_>>>()?;
    Ok(())
}
```
## savecodec
Savecodec is a simple usage of `binformat` to work on creating a save format handler for the game realm grinder in rust