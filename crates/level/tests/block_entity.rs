use std::fs::File;
use std::io::Cursor;

use bedrock_level::block_entity::write_block_entities;
use bedrock_level::db::{Database, WriteBatch};
use bedrock_level::key::{Key, KeyVariant};
use bedrock_level::nbt_list;
use bedrock_level::types::ChunkPosition;
use bedrock_shared::world::dimension::Dimension;
use flate2::read::GzDecoder;
use tar::Archive;

/// A little-endian NBT compound `{ id: <id>, x: <x>, CustomName: <name bytes> }` with an empty root name.
fn compound(id: &str, x: i32, name: &[u8]) -> Vec<u8> {
    fn string(out: &mut Vec<u8>, s: &[u8]) {
        out.extend(u16::try_from(s.len()).unwrap().to_le_bytes());
        out.extend(s);
    }
    let mut out = vec![10, 0, 0];
    out.push(8);
    string(&mut out, b"id");
    string(&mut out, id.as_bytes());
    out.push(3);
    string(&mut out, b"x");
    out.extend(x.to_le_bytes());
    out.push(8);
    string(&mut out, b"CustomName");
    string(&mut out, name);
    out.push(0);
    out
}

fn record() -> Vec<u8> {
    let mut bytes = compound("Chest", -3, b"loot");
    // names are not always valid UTF-8 in real worlds
    bytes.extend(compound("Sign", 7, &[0xff, 0x00, 0xc3]));
    bytes
}

#[test]
fn nbt_list_roundtrips_byte_exact() {
    let bytes = record();
    let values = nbt_list::read(&bytes).unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(nbt_list::write(&values).unwrap(), bytes);
    assert!(nbt_list::read(&[]).unwrap().is_empty());
}

#[test]
fn truncated_nbt_list_is_an_error() {
    let bytes = record();
    assert!(nbt_list::read(&bytes[..bytes.len() - 1]).is_err());
}

#[test]
fn block_entities_write_read_and_remove() {
    let tmp = tempfile::tempdir().unwrap();
    let tar = GzDecoder::new(File::open("tests/level.tar.gz").unwrap());
    Archive::new(tar).unpack(tmp.path()).unwrap();
    let path = tmp.path().join("test_level/db");
    let db = Database::open(path.to_str().unwrap()).unwrap();

    let chunk = ChunkPosition(1000, -1000);
    let values = nbt_list::read(&record()).unwrap();
    for dimension in [Dimension::Overworld, Dimension::Nether] {
        assert!(db.block_entities(chunk, dimension).unwrap().is_empty());

        let mut batch = WriteBatch::new();
        write_block_entities(&mut batch, chunk, dimension, &values).unwrap();
        db.write(batch).unwrap();
        assert_eq!(db.block_entities(chunk, dimension).unwrap(), values);

        let key = Key {
            chunk,
            dimension,
            data: KeyVariant::BlockEntity,
        };
        let mut raw_key = Vec::new();
        key.serialize(&mut raw_key).unwrap();
        assert_eq!(&*db.get(&raw_key).unwrap().unwrap(), record().as_slice());

        let mut batch = WriteBatch::new();
        write_block_entities(&mut batch, chunk, dimension, &[]).unwrap();
        db.write(batch).unwrap();
        assert!(db.get(&raw_key).unwrap().is_none());
    }
}

/// Every block entity record of the test world reads and writes back to the same bytes.
#[test]
fn test_world_block_entities_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let tar = GzDecoder::new(File::open("tests/level.tar.gz").unwrap());
    Archive::new(tar).unpack(tmp.path()).unwrap();
    let path = tmp.path().join("test_level/db");
    let db = Database::open(path.to_str().unwrap()).unwrap();

    let mut checked = 0;
    let mut keys = db.keys().unwrap();
    for kv in &mut keys {
        let Ok(key) = Key::deserialize(&mut Cursor::new(kv.key())) else {
            continue;
        };
        if key.data == KeyVariant::BlockEntity {
            let value = kv.value();
            let values = nbt_list::read(&value).unwrap();
            assert_eq!(nbt_list::write(&values).unwrap(), &*value);
            checked += 1;
        }
    }
    println!("{checked} block entity records");
}
