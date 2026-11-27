//! Actors (mobs, items, minecarts, ...) of a chunk.
//!
//! Current worlds keep one digest per chunk at `digp` + chunk position, a list of 8-byte actor ids,
//! and the NBT of each actor at `actorprefix` + id. Older worlds keep all actors of a chunk in the
//! [`KeyVariant::Entity`] record instead; the game moves them to the digest when it saves the chunk.

use bedrock_shared::world::dimension::Dimension;
use nbtx::Value;

use crate::db::{Database, WriteBatch};
use crate::error::Result;
use crate::key::{Key, KeyVariant};
use crate::nbt_list;
use crate::types::ChunkPosition;

/// Prefix of the key of a chunk's actor digest.
pub const DIGEST_PREFIX: &[u8] = b"digp";
/// Prefix of the key of an actor.
pub const ACTOR_PREFIX: &[u8] = b"actorprefix";

/// The storage id of an actor: its `UniqueID` as 8 little-endian bytes, the bytes the game stores
/// after [`ACTOR_PREFIX`] and in `StorageKey`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActorId(pub [u8; 8]);

impl ActorId {
    /// The id the game gives an actor with this `UniqueID`.
    pub fn from_unique_id(unique_id: i64) -> Self {
        Self(unique_id.to_le_bytes())
    }

    /// The key of the NBT record of this actor.
    pub fn key(&self) -> Vec<u8> {
        [ACTOR_PREFIX, &self.0].concat()
    }
}

/// The key of a chunk's actor digest: [`DIGEST_PREFIX`], x, z and the dimension unless it is the
/// overworld.
pub fn digest_key(chunk: ChunkPosition, dimension: Dimension) -> Vec<u8> {
    let mut key = DIGEST_PREFIX.to_vec();
    key.extend(chunk.0.to_le_bytes());
    key.extend(chunk.1.to_le_bytes());
    if dimension != Dimension::Overworld {
        key.extend(i32::from(dimension).to_le_bytes());
    }
    key
}

fn legacy_key(chunk: ChunkPosition, dimension: Dimension) -> Vec<u8> {
    let key = Key {
        chunk,
        dimension,
        data: KeyVariant::Entity,
    };
    let mut buf = Vec::with_capacity(key.size_hint());
    key.serialize(&mut buf)
        .expect("writing to a Vec does not fail");
    buf
}

impl Database {
    /// Reads the ids listed in a chunk's actor digest. A chunk without a digest has none.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the database fails.
    pub fn actor_ids(&self, chunk: ChunkPosition, dimension: Dimension) -> Result<Vec<ActorId>> {
        let Some(digest) = self.get(digest_key(chunk, dimension))? else {
            return Ok(Vec::new());
        };
        Ok(digest
            .as_chunks::<8>()
            .0
            .iter()
            .copied()
            .map(ActorId)
            .collect())
    }

    /// Reads the actors of a chunk listed in its digest, in digest order. Ids without an actor
    /// record are skipped, as the game skips them.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the database fails or an actor record holds malformed NBT.
    pub fn actors(
        &self,
        chunk: ChunkPosition,
        dimension: Dimension,
    ) -> Result<Vec<(ActorId, Value)>> {
        let mut actors = Vec::new();
        for id in self.actor_ids(chunk, dimension)? {
            if let Some(bytes) = self.get(id.key())? {
                actors.push((id, nbtx::from_le_bytes(&mut &bytes[..])?));
            }
        }
        Ok(actors)
    }

    /// Reads the actors an older world keeps in the [`KeyVariant::Entity`] record of the chunk.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the database fails or the record holds malformed NBT.
    pub fn legacy_actors(&self, chunk: ChunkPosition, dimension: Dimension) -> Result<Vec<Value>> {
        match self.get(legacy_key(chunk, dimension))? {
            Some(bytes) => nbt_list::read(&bytes),
            None => Ok(Vec::new()),
        }
    }
}

/// Adds the writes that replace the actors of a chunk to `batch`: one record per actor, the digest
/// in the order of `actors`, and removals for the ids in `old` that are no longer present. Pass the
/// ids read with [`Database::actor_ids`] as `old`. An empty `actors` removes the digest. The legacy
/// [`KeyVariant::Entity`] record is removed as well, so actors read with
/// [`Database::legacy_actors`] move to the digest.
///
/// # Errors
///
/// Returns an NBT error if a value cannot be serialized.
pub fn write_actors(
    batch: &mut WriteBatch,
    chunk: ChunkPosition,
    dimension: Dimension,
    old: &[ActorId],
    actors: &[(ActorId, Value)],
) -> Result<()> {
    let mut digest = Vec::with_capacity(actors.len() * 8);
    for (id, value) in actors {
        batch.insert(id.key(), nbtx::to_le_bytes(value)?);
        digest.extend(id.0);
    }
    for id in old {
        if !actors.iter().any(|(new, _)| new == id) {
            batch.remove(id.key());
        }
    }

    let key = digest_key(chunk, dimension);
    if digest.is_empty() {
        batch.remove(key);
    } else {
        batch.insert(key, digest);
    }
    batch.remove(legacy_key(chunk, dimension));
    Ok(())
}
