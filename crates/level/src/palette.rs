//! Direct reading and writing of block palette entries.
//!
//! Almost every palette entry has the same shape: a root compound with a `name` string, a
//! `version` int and a `states` compound of scalar values. nbtx reads and writes any shape
//! through reflection, which costs far more than the entry itself, so this shape is handled
//! here and anything else is handed to nbtx. The results are the same either way: an entry this
//! module does not fully understand is read again by nbtx from its start.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use nbtx::Value;

use crate::error::Result;
use crate::subchunk::BlockDef;

const TAG_END: u8 = 0;
const TAG_BYTE: u8 = 1;
const TAG_SHORT: u8 = 2;
const TAG_INT: u8 = 3;
const TAG_LONG: u8 = 4;
const TAG_FLOAT: u8 = 5;
const TAG_DOUBLE: u8 = 6;
const TAG_STRING: u8 = 8;
const TAG_COMPOUND: u8 = 10;

/// Entries larger than this go through nbtx, so its decode limits apply to them as before.
const MAX_DIRECT_LEN: u64 = 1 << 20;

/// Reads one palette entry.
pub(crate) fn read_entry<R>(reader: &mut Cursor<R>) -> Result<BlockDef>
where
    Cursor<R>: Read,
{
    let start = reader.position();
    if let Some(entry) = read_common(reader)
        && reader.position() - start <= MAX_DIRECT_LEN
    {
        return Ok(entry);
    }
    reader.set_position(start);
    Ok(nbtx::from_le_bytes(reader)?)
}

/// Writes one palette entry, byte for byte as nbtx writes it.
pub(crate) fn write_entry<W>(writer: &mut Cursor<W>, entry: &BlockDef) -> Result<()>
where
    Cursor<W>: Write,
{
    if !writes_directly(entry) {
        nbtx::to_le_bytes_in(writer, entry)?;
        return Ok(());
    }

    writer.write_u8(TAG_COMPOUND)?;
    write_str(writer, b"")?;

    write_key(writer, TAG_STRING, b"name")?;
    write_str(writer, entry.name.as_bytes())?;
    if let Some(version) = entry.version {
        write_key(writer, TAG_INT, b"version")?;
        writer.write_i32::<LittleEndian>(version)?;
    }
    write_key(writer, TAG_COMPOUND, b"states")?;
    for (key, value) in &entry.states {
        write_scalar(writer, key.as_bytes(), value)?;
    }
    writer.write_u8(TAG_END)?;

    writer.write_u8(TAG_END)?;
    Ok(())
}

/// The common shape, or `None` for anything else (including errors, which nbtx then reports).
fn read_common<R: Read>(r: &mut R) -> Option<BlockDef> {
    if r.read_u8().ok()? != TAG_COMPOUND {
        return None;
    }
    read_bytes(r)?;

    let (mut name, mut version, mut states) = (None, None, None);
    loop {
        let tag = r.read_u8().ok()?;
        if tag == TAG_END {
            break;
        }
        let mut key = [0; 8];
        let key = read_field_name(r, &mut key)?;
        match (tag, key) {
            (TAG_STRING, b"name") if name.is_none() => {
                name = Some(String::from_utf8(read_bytes(r)?).ok()?);
            }
            (TAG_INT, b"version") if version.is_none() => {
                version = Some(r.read_i32::<LittleEndian>().ok()?);
            }
            (TAG_COMPOUND, b"states") if states.is_none() => states = Some(read_states(r)?),
            _ => return None,
        }
    }

    Some(BlockDef {
        name: name?,
        version,
        states: states.unwrap_or_default(),
    })
}

fn read_states<R: Read>(r: &mut R) -> Option<HashMap<String, Value>> {
    let mut states = HashMap::new();
    loop {
        let tag = r.read_u8().ok()?;
        if tag == TAG_END {
            return Some(states);
        }
        let key = String::from_utf8(read_bytes(r)?).ok()?;
        let value = match tag {
            TAG_BYTE => Value::Byte(r.read_i8().ok()?),
            TAG_SHORT => Value::Short(r.read_i16::<LittleEndian>().ok()?),
            TAG_INT => Value::Int(r.read_i32::<LittleEndian>().ok()?),
            TAG_LONG => Value::Long(r.read_i64::<LittleEndian>().ok()?),
            TAG_FLOAT => Value::Float(r.read_f32::<LittleEndian>().ok()?),
            TAG_DOUBLE => Value::Double(r.read_f64::<LittleEndian>().ok()?),
            TAG_STRING => Value::String(read_bytes(r)?.into()),
            _ => return None,
        };
        if states.insert(key, value).is_some() {
            return None;
        }
    }
}

/// A field name of the root compound into `buf`, without allocating. A name longer than `buf`
/// is none of the known fields.
fn read_field_name<'b, R: Read>(r: &mut R, buf: &'b mut [u8; 8]) -> Option<&'b [u8]> {
    let len = usize::from(r.read_u16::<LittleEndian>().ok()?);
    let name = buf.get_mut(..len)?;
    r.read_exact(name).ok()?;
    Some(name)
}

/// A string payload: a little endian `u16` length and the bytes.
fn read_bytes<R: Read>(r: &mut R) -> Option<Vec<u8>> {
    let len = r.read_u16::<LittleEndian>().ok()?;
    let mut buf = vec![0; usize::from(len)];
    r.read_exact(&mut buf).ok()?;
    Some(buf)
}

/// Whether every state is a scalar and every string fits its `u16` length; nbtx writes (or
/// rejects) the rest.
fn writes_directly(entry: &BlockDef) -> bool {
    let fits = |s: &[u8]| u16::try_from(s.len()).is_ok();
    fits(entry.name.as_bytes())
        && entry.states.iter().all(|(key, value)| {
            fits(key.as_bytes())
                && match value {
                    Value::String(s) => fits(s),
                    Value::Byte(_)
                    | Value::Short(_)
                    | Value::Int(_)
                    | Value::Long(_)
                    | Value::Float(_)
                    | Value::Double(_) => true,
                    _ => false,
                }
        })
}

fn write_scalar<W: Write>(w: &mut W, key: &[u8], value: &Value) -> Result<()> {
    match value {
        Value::Byte(v) => {
            write_key(w, TAG_BYTE, key)?;
            w.write_i8(*v)?;
        }
        Value::Short(v) => {
            write_key(w, TAG_SHORT, key)?;
            w.write_i16::<LittleEndian>(*v)?;
        }
        Value::Int(v) => {
            write_key(w, TAG_INT, key)?;
            w.write_i32::<LittleEndian>(*v)?;
        }
        Value::Long(v) => {
            write_key(w, TAG_LONG, key)?;
            w.write_i64::<LittleEndian>(*v)?;
        }
        Value::Float(v) => {
            write_key(w, TAG_FLOAT, key)?;
            w.write_f32::<LittleEndian>(*v)?;
        }
        Value::Double(v) => {
            write_key(w, TAG_DOUBLE, key)?;
            w.write_f64::<LittleEndian>(*v)?;
        }
        Value::String(v) => {
            write_key(w, TAG_STRING, key)?;
            write_str(w, v)?;
        }
        _ => unreachable!("only scalars are written directly"),
    }
    Ok(())
}

fn write_key<W: Write>(w: &mut W, tag: u8, key: &[u8]) -> Result<()> {
    w.write_u8(tag)?;
    write_str(w, key)
}

fn write_str<W: Write>(w: &mut W, s: &[u8]) -> Result<()> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "`writes_directly` checked that every string fits"
    )]
    w.write_u16::<LittleEndian>(s.len() as u16)?;
    w.write_all(s)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(states: &[(&str, Value)], version: Option<i32>) -> BlockDef {
        BlockDef {
            name: "minecraft:oak_stairs".to_owned(),
            version,
            states: states
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect(),
        }
    }

    fn samples() -> Vec<BlockDef> {
        vec![
            block(&[], None),
            block(&[], Some(18_168_865)),
            block(
                &[
                    ("upside_down_bit", Value::Byte(1)),
                    ("weirdo_direction", Value::Int(3)),
                    (
                        "minecraft:cardinal_direction",
                        Value::String("north".into()),
                    ),
                    ("short", Value::Short(-2)),
                    ("long", Value::Long(1 << 40)),
                    ("float", Value::Float(0.5)),
                    ("double", Value::Double(-1.25)),
                ],
                Some(17_959_425),
            ),
            block(
                &[("nested", Value::List(nbtx::ValueList::Int(vec![1, 2, 3])))],
                Some(1),
            ),
        ]
    }

    fn nbtx_bytes(entry: &BlockDef) -> Vec<u8> {
        let mut out = Vec::new();
        nbtx::to_le_bytes_in(&mut out, entry).unwrap();
        out
    }

    fn read_both(bytes: &[u8]) -> (Result<BlockDef>, Result<BlockDef>) {
        let mut cursor = Cursor::new(bytes);
        let direct = read_entry(&mut cursor);
        let generic = nbtx::from_le_bytes::<BlockDef>(&mut Cursor::new(bytes)).map_err(Into::into);
        if direct.is_ok() {
            assert_eq!(
                cursor.position(),
                bytes.len() as u64,
                "entry not fully read"
            );
        }
        (direct, generic)
    }

    #[test]
    fn written_entries_match_nbtx() {
        for entry in samples() {
            let mut direct = Cursor::new(Vec::new());
            write_entry(&mut direct, &entry).unwrap();
            assert_eq!(direct.into_inner(), nbtx_bytes(&entry), "{entry:?}");
        }
    }

    #[test]
    fn read_entries_match_nbtx() {
        for entry in samples() {
            let (direct, generic) = read_both(&nbtx_bytes(&entry));
            assert_eq!(direct.unwrap(), generic.unwrap());
        }
    }

    /// The game writes `name`, `states`, `version`, in that order.
    #[test]
    fn game_key_order_reads_like_nbtx() {
        let mut bytes = vec![TAG_COMPOUND, 0, 0];
        bytes.extend([TAG_STRING, 4, 0]);
        bytes.extend(b"name");
        bytes.extend([15, 0]);
        bytes.extend(b"minecraft:stone");
        bytes.extend([TAG_COMPOUND, 6, 0]);
        bytes.extend(b"states");
        bytes.extend([TAG_BYTE, 3, 0]);
        bytes.extend(b"bit");
        bytes.extend([1, TAG_END]);
        bytes.extend([TAG_INT, 7, 0]);
        bytes.extend(b"version");
        bytes.extend(18_168_865_i32.to_le_bytes());
        bytes.push(TAG_END);

        let (direct, generic) = read_both(&bytes);
        let direct = direct.unwrap();
        assert_eq!(direct, generic.unwrap());
        assert_eq!(direct.name, "minecraft:stone");
        assert_eq!(direct.version, Some(18_168_865));
        assert_eq!(direct.states["bit"], Value::Byte(1));
    }

    /// Shapes this module does not read itself still decode exactly as nbtx decodes them, or fail
    /// with nbtx's error.
    #[test]
    fn other_shapes_fall_back_to_nbtx() {
        let named = |tag: u8, key: &[u8], payload: &[u8]| {
            let mut v = vec![tag];
            v.extend(u16::try_from(key.len()).unwrap().to_le_bytes());
            v.extend(key);
            v.extend(payload);
            v
        };
        let name = named(TAG_STRING, b"name", b"\x03\x00abc");
        let compound = |fields: &[Vec<u8>]| {
            let mut v = vec![TAG_COMPOUND, 0, 0];
            for f in fields {
                v.extend(f);
            }
            v.push(TAG_END);
            v
        };
        let cases = [
            // An unknown key.
            compound(&[name.clone(), named(TAG_INT, b"extra", &[1, 0, 0, 0])]),
            // A duplicate key.
            compound(&[name.clone(), name.clone()]),
            // A state holding a compound.
            compound(&[
                name.clone(),
                named(
                    TAG_COMPOUND,
                    b"states",
                    &named(TAG_COMPOUND, b"c", &[TAG_END])
                        .into_iter()
                        .chain([TAG_END])
                        .collect::<Vec<_>>(),
                ),
            ]),
            // A duplicate state.
            compound(&[
                name.clone(),
                named(
                    TAG_COMPOUND,
                    b"states",
                    &[
                        named(TAG_BYTE, b"s", &[1]),
                        named(TAG_BYTE, b"s", &[2]),
                        vec![TAG_END],
                    ]
                    .concat(),
                ),
            ]),
            // No name.
            compound(&[named(TAG_INT, b"version", &[1, 0, 0, 0])]),
            // A name that is not UTF-8.
            compound(&[named(TAG_STRING, b"name", b"\x01\x00\xff")]),
            // A version of the wrong type.
            compound(&[name.clone(), named(TAG_SHORT, b"version", &[1, 0])]),
            // Not a compound.
            named(TAG_INT, b"", &[1, 0, 0, 0]),
            // Cut short.
            compound(std::slice::from_ref(&name))[..6].to_vec(),
        ];
        for bytes in cases {
            match read_both(&bytes) {
                (Ok(direct), Ok(generic)) => assert_eq!(direct, generic),
                (Err(direct), Err(generic)) => {
                    assert_eq!(direct.to_string(), generic.to_string());
                }
                (direct, generic) => panic!("{bytes:?}: {direct:?} vs {generic:?}"),
            }
        }
    }
}
