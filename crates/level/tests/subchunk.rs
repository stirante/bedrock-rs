use std::collections::HashMap;
use std::io::Cursor;

use bedrock_level::subchunk::{BlockDef, SubChunk, SubChunkVersion};
use bedrock_level::{Greedy, Lazy};

fn stone() -> BlockDef {
    BlockDef {
        name: "minecraft:stone".to_owned(),
        version: Some(18_168_865),
        states: HashMap::new(),
    }
}

fn palette_entry() -> Vec<u8> {
    let mut out = Vec::new();
    nbtx::to_le_bytes_in(&mut out, &stone()).unwrap();
    out
}

/// A version 8 sub chunk whose only layer has no indices and, as old saves store it, no
/// palette size: the palette entry follows the layer header directly.
#[test]
fn old_single_block_layer_without_palette_size() {
    let mut bytes = vec![8, 1, 0];
    bytes.extend(palette_entry());

    for chunk in [
        SubChunk::from_disk::<Greedy, _>(&mut Cursor::new(bytes.as_slice())).unwrap(),
        SubChunk::from_disk::<Lazy, _>(&mut Cursor::new(bytes.as_slice())).unwrap(),
    ] {
        let layer = chunk.get_layer(0).unwrap();
        assert_eq!(layer.palette(), &[stone()]);
        assert_eq!(layer.get((3, 4, 5)), Some(&stone()));
    }
}

#[test]
fn single_block_layer_with_palette_size() {
    let mut bytes = vec![9, 1, 2, 0];
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(palette_entry());

    let chunk = SubChunk::from_disk_greedy(&mut Cursor::new(bytes.as_slice())).unwrap();
    assert_eq!(chunk.index(), 2);
    assert_eq!(chunk.get_layer(0).unwrap().palette(), &[stone()]);
}

/// Version 1 sub chunks have one layer and no layer count byte, in both directions.
#[test]
fn legacy_sub_chunk_has_no_layer_count() {
    let mut bytes = vec![1, 0];
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(palette_entry());

    let chunk = SubChunk::from_disk_greedy(&mut Cursor::new(bytes.as_slice())).unwrap();
    assert_eq!(chunk.version(), SubChunkVersion::Legacy);

    let mut out = Cursor::new(Vec::new());
    chunk.to_disk::<Greedy, _>(&mut out).unwrap();
    let out = out.into_inner();
    assert_eq!(&out[..2], &[1, 0], "version byte, then the layer header");

    let again = SubChunk::from_disk_greedy(&mut Cursor::new(out.as_slice())).unwrap();
    assert_eq!(again, chunk);
}
