use std::io::Cursor;

use bedrock_level::Greedy;
use bedrock_level::biome::Biomes;

/// `Data3D` as the game saves it: a height map, then one storage per sub chunk whose header has
/// the lowest bit set. 0x01 is a single biome id, 0xff repeats the storage below and
/// `bits << 1 | 1` is a packed array followed by an id palette.
fn game_data3d() -> Vec<u8> {
    let mut out: Vec<u8> = (0..256u16).flat_map(|h| (h % 40).to_le_bytes()).collect();
    out.extend([0x01, 1, 0, 0, 0]);
    out.push(0xff);
    // two biomes, one bit per entry: 128 words, every other position the second biome
    out.push(1 << 1 | 1);
    for _ in 0..128 {
        out.extend(0xaaaa_aaaau32.to_le_bytes());
    }
    out.extend(2u32.to_le_bytes());
    out.extend(7u32.to_le_bytes());
    out.extend(21u32.to_le_bytes());
    out
}

#[test]
fn data3d_reencodes_to_the_game_bytes() {
    let bytes = game_data3d();
    let biomes = Biomes::from_disk::<Greedy, _>(&mut Cursor::new(bytes.as_slice())).unwrap();

    let mut out = Cursor::new(Vec::new());
    biomes.to_disk(&mut out).unwrap();
    assert_eq!(out.into_inner(), bytes);
}
