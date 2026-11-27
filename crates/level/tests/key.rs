use std::io::Cursor;

use bedrock_level::key::{Key, KeyVariant};
use bedrock_level::types::ChunkPosition;
use bedrock_shared::world::dimension::Dimension;

/// The `LevelChunkTag` byte of every chunk record, as `DBChunkStorage::_serializeChunk` writes it.
const TAGS: [(KeyVariant, u8); 25] = [
    (KeyVariant::Biome3d, 43),
    (KeyVariant::ChunkVersion, 44),
    (KeyVariant::HeightMap, 45),
    (KeyVariant::LegacyHeightMap, 46),
    (KeyVariant::SubChunk { index: -4 }, 47),
    (KeyVariant::LegacyTerrain, 48),
    (KeyVariant::BlockEntity, 49),
    (KeyVariant::Entity, 50),
    (KeyVariant::PendingTicks, 51),
    (KeyVariant::LegacyBlockExtraData, 52),
    (KeyVariant::BiomeState, 53),
    (KeyVariant::FinalizedState, 54),
    (KeyVariant::ConversionData, 55),
    (KeyVariant::BorderBlocks, 56),
    (KeyVariant::HardcodedSpawnAreas, 57),
    (KeyVariant::RandomTicks, 58),
    (KeyVariant::Checksums, 59),
    (KeyVariant::GenerationSeed, 60),
    (KeyVariant::GeneratedBlending, 61),
    (KeyVariant::BlendingBiomeHeight, 62),
    (KeyVariant::MetadataHash, 63),
    (KeyVariant::BlendingData, 64),
    (KeyVariant::ActorDigestVersion, 65),
    (KeyVariant::LegacyVersion, 118),
    (KeyVariant::AabbVolumes, 119),
];

#[test]
fn chunk_tags_match_the_game() {
    for (variant, tag) in TAGS {
        assert_eq!(variant.discriminant(), tag, "{variant:?}");
    }
}

#[test]
fn every_chunk_tag_roundtrips_in_every_dimension() {
    for dimension in [Dimension::Overworld, Dimension::Nether, Dimension::End] {
        for (data, _) in TAGS {
            let key = Key {
                chunk: ChunkPosition(-3, 7),
                dimension,
                data,
            };
            let mut buf = Vec::new();
            key.serialize(&mut buf).unwrap();
            assert_eq!(buf.len(), key.size_hint());

            let read = Key::deserialize(&mut Cursor::new(buf.as_slice())).unwrap();
            assert_eq!(read, key);
        }
    }
}
