use std::collections::HashMap;
use std::io::Cursor;

use bedrock_level::subchunk::{BlockDef, SubChunk};

fn fence(version: i32) -> BlockDef {
    let states = [
        "east",
        "north",
        "south",
        "west",
        "up",
        "waterlogged",
        "in_wall",
        "open",
    ]
    .into_iter()
    .enumerate()
    .map(|(i, k)| (k.to_owned(), nbtx::Value::Byte(i8::try_from(i).unwrap())))
    .collect::<HashMap<_, _>>();
    BlockDef {
        name: "minecraft:fence_gate".to_owned(),
        version: Some(version),
        states,
    }
}

fn layer_bytes(palette: &[BlockDef]) -> Vec<u8> {
    // one layer, one bit per block, every block index 0
    let mut out = vec![9, 1, 0, 1 << 1];
    out.extend(std::iter::repeat_n(0u8, 128 * 4));
    out.extend(u32::try_from(palette.len()).unwrap().to_le_bytes());
    for block in palette {
        nbtx::to_le_bytes_in(&mut out, block).unwrap();
    }
    out
}

/// Equal blocks built separately (each `HashMap` with its own iteration order) are found in the
/// palette, so setting one does not add a duplicate entry.
#[test]
fn equal_blocks_find_the_same_palette_entry() {
    let bytes = layer_bytes(&[fence(1), fence(2)]);
    let mut chunk = SubChunk::from_disk_greedy(&mut Cursor::new(bytes.as_slice())).unwrap();
    let layer = chunk.get_layer_mut(0).unwrap();

    for _ in 0..32 {
        assert_eq!(layer.palette_index(&fence(1)), Some(0));
        assert_eq!(layer.palette_index(&fence(2)), Some(1));
    }

    layer.set((1, 2, 3), fence(2));
    assert_eq!(layer.palette().len(), 2);
    assert_eq!(layer.get((1, 2, 3)), Some(&fence(2)));
}
