use std::io::Cursor;

use bedrock_level::Greedy;
use bedrock_level::subchunk::SubChunk;

fn string(out: &mut Vec<u8>, s: &[u8]) {
    out.extend(u16::try_from(s.len()).unwrap().to_le_bytes());
    out.extend(s);
}

/// A palette entry as the game writes it: `name`, then `states` in the block's own state
/// order (not sorted), then `version`.
fn game_entry() -> Vec<u8> {
    let mut out = vec![10, 0, 0];
    out.push(8);
    string(&mut out, b"name");
    string(&mut out, b"minecraft:oak_stairs");
    out.push(10);
    string(&mut out, b"states");
    out.push(3);
    string(&mut out, b"weirdo_direction");
    out.extend(2i32.to_le_bytes());
    out.push(1);
    string(&mut out, b"upside_down_bit");
    out.push(0);
    out.push(8);
    string(&mut out, b"a_string_state");
    string(&mut out, b"x");
    out.push(0);
    out.push(3);
    string(&mut out, b"version");
    out.extend(18_168_865i32.to_le_bytes());
    out.push(0);
    out
}

#[test]
fn palette_entries_keep_the_game_field_and_state_order() {
    let mut bytes = vec![9, 1, 0, 0];
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(game_entry());

    let chunk = SubChunk::from_disk::<Greedy, _>(&mut Cursor::new(bytes.as_slice())).unwrap();
    let block = &chunk.get_layer(0).unwrap().palette()[0];
    let keys: Vec<&str> = block.states.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        ["weirdo_direction", "upside_down_bit", "a_string_state"]
    );

    let mut out = Cursor::new(Vec::new());
    chunk.to_disk::<Greedy, _>(&mut out).unwrap();
    assert_eq!(out.into_inner(), bytes);
}
