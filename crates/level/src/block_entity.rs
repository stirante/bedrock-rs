//! Block entities of a chunk: chests, signs, spawners and the like, stored as one record per chunk.

use bedrock_shared::world::dimension::Dimension;
use nbtx::Value;

use crate::db::{Database, WriteBatch};
use crate::error::Result;
use crate::key::{Key, KeyVariant};
use crate::nbt_list;
use crate::types::ChunkPosition;

fn key(chunk: ChunkPosition, dimension: Dimension) -> Vec<u8> {
    let key = Key {
        chunk,
        dimension,
        data: KeyVariant::BlockEntity,
    };
    let mut buf = Vec::with_capacity(key.size_hint());
    key.serialize(&mut buf)
        .expect("writing to a Vec does not fail");
    buf
}

impl Database {
    /// Reads the block entities of a chunk. A chunk without the record has none.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the database fails or the record holds malformed NBT.
    pub fn block_entities(&self, chunk: ChunkPosition, dimension: Dimension) -> Result<Vec<Value>> {
        match self.get(key(chunk, dimension))? {
            Some(bytes) => nbt_list::read(&bytes),
            None => Ok(Vec::new()),
        }
    }
}

/// Adds the writes that replace the block entities of a chunk to `batch`. An empty list removes the
/// record, as the game does for a chunk without block entities.
///
/// # Errors
///
/// Returns an NBT error if a value cannot be serialized.
pub fn write_block_entities(
    batch: &mut WriteBatch,
    chunk: ChunkPosition,
    dimension: Dimension,
    block_entities: &[Value],
) -> Result<()> {
    let key = key(chunk, dimension);
    if block_entities.is_empty() {
        batch.remove(key);
    } else {
        batch.insert(key, nbt_list::write(block_entities)?);
    }
    Ok(())
}
