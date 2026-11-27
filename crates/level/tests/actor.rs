use std::fs::File;

use bedrock_level::actor::{ACTOR_PREFIX, ActorId, DIGEST_PREFIX, digest_key, write_actors};
use bedrock_level::db::{Database, WriteBatch};
use bedrock_level::nbt_list;
use bedrock_level::types::ChunkPosition;
use bedrock_shared::world::dimension::Dimension;
use flate2::read::GzDecoder;
use nbtx::Value;
use tar::Archive;

fn open_test_db() -> (tempfile::TempDir, Database) {
    let tmp = tempfile::tempdir().unwrap();
    let tar = GzDecoder::new(File::open("tests/level.tar.gz").unwrap());
    Archive::new(tar).unpack(tmp.path()).unwrap();
    let path = tmp.path().join("test_level/db");
    let db = Database::open(path.to_str().unwrap()).unwrap();
    (tmp, db)
}

/// `{ identifier, UniqueID, StorageKey }`, as the game stores an actor.
fn actor(identifier: &str, unique_id: i64) -> (ActorId, Vec<u8>) {
    fn string(out: &mut Vec<u8>, s: &[u8]) {
        out.extend(u16::try_from(s.len()).unwrap().to_le_bytes());
        out.extend(s);
    }
    let id = ActorId::from_unique_id(unique_id);
    let mut out = vec![10, 0, 0, 8];
    string(&mut out, b"identifier");
    string(&mut out, identifier.as_bytes());
    out.push(4);
    string(&mut out, b"UniqueID");
    out.extend(unique_id.to_le_bytes());
    // the id bytes, stored as a string tag although they are rarely valid UTF-8
    out.push(8);
    string(&mut out, b"StorageKey");
    string(&mut out, &id.0);
    out.push(0);
    (id, out)
}

fn value(bytes: &[u8]) -> Value {
    nbtx::from_le_bytes(&mut &bytes[..]).unwrap()
}

#[test]
fn keys() {
    let chunk = ChunkPosition(-1, 2);
    let mut overworld = DIGEST_PREFIX.to_vec();
    overworld.extend((-1i32).to_le_bytes());
    overworld.extend(2i32.to_le_bytes());
    assert_eq!(digest_key(chunk, Dimension::Overworld), overworld);
    let mut nether = overworld.clone();
    nether.extend(1i32.to_le_bytes());
    assert_eq!(digest_key(chunk, Dimension::Nether), nether);

    let id = ActorId::from_unique_id(-4_294_967_295);
    assert_eq!(
        id.key(),
        [ACTOR_PREFIX, &[1, 0, 0, 0, 255, 255, 255, 255]].concat()
    );
}

#[test]
fn actors_write_read_replace_and_remove() {
    let (_tmp, db) = open_test_db();
    let chunk = ChunkPosition(4000, 4000);
    let (pig_id, pig) = actor("minecraft:pig", -12_884_901_887);
    let (cow_id, cow) = actor("minecraft:cow", 77);
    let both = vec![(pig_id, value(&pig)), (cow_id, value(&cow))];

    let mut batch = WriteBatch::new();
    write_actors(&mut batch, chunk, Dimension::End, &[], &both).unwrap();
    db.write(batch).unwrap();
    assert_eq!(
        db.actor_ids(chunk, Dimension::End).unwrap(),
        vec![pig_id, cow_id]
    );
    assert_eq!(db.actors(chunk, Dimension::End).unwrap(), both);
    assert_eq!(&*db.get(pig_id.key()).unwrap().unwrap(), pig.as_slice());
    assert!(db.actors(chunk, Dimension::Overworld).unwrap().is_empty());

    let old = db.actor_ids(chunk, Dimension::End).unwrap();
    let mut batch = WriteBatch::new();
    write_actors(&mut batch, chunk, Dimension::End, &old, &both[1..]).unwrap();
    db.write(batch).unwrap();
    assert_eq!(db.actor_ids(chunk, Dimension::End).unwrap(), vec![cow_id]);
    assert!(db.get(pig_id.key()).unwrap().is_none());

    let old = db.actor_ids(chunk, Dimension::End).unwrap();
    let mut batch = WriteBatch::new();
    write_actors(&mut batch, chunk, Dimension::End, &old, &[]).unwrap();
    db.write(batch).unwrap();
    assert!(db.get(digest_key(chunk, Dimension::End)).unwrap().is_none());
    assert!(db.get(cow_id.key()).unwrap().is_none());
}

#[test]
fn ids_missing_from_the_database_are_skipped() {
    let (_tmp, db) = open_test_db();
    let chunk = ChunkPosition(-4000, 0);
    let (cow_id, cow) = actor("minecraft:cow", 5);
    let ghost = ActorId::from_unique_id(6);
    db.insert(
        digest_key(chunk, Dimension::Overworld),
        [ghost.0, cow_id.0].concat(),
    )
    .unwrap();
    db.insert(cow_id.key(), &cow).unwrap();
    assert_eq!(
        db.actors(chunk, Dimension::Overworld).unwrap(),
        vec![(cow_id, value(&cow))]
    );
}

#[test]
fn legacy_actors_move_to_the_digest() {
    let (_tmp, db) = open_test_db();
    let chunk = ChunkPosition(9, 9);
    let (pig_id, pig) = actor("minecraft:pig", 1);
    let mut legacy_key = Vec::new();
    legacy_key.extend(9i32.to_le_bytes());
    legacy_key.extend(9i32.to_le_bytes());
    legacy_key.push(0x32);
    db.insert(&legacy_key, &pig).unwrap();

    let legacy = db.legacy_actors(chunk, Dimension::Overworld).unwrap();
    assert_eq!(legacy, nbt_list::read(&pig).unwrap());

    let actors: Vec<_> = legacy.into_iter().map(|v| (pig_id, v)).collect();
    let mut batch = WriteBatch::new();
    write_actors(&mut batch, chunk, Dimension::Overworld, &[], &actors).unwrap();
    db.write(batch).unwrap();
    assert!(db.get(&legacy_key).unwrap().is_none());
    assert!(
        db.legacy_actors(chunk, Dimension::Overworld)
            .unwrap()
            .is_empty()
    );
    assert_eq!(db.actors(chunk, Dimension::Overworld).unwrap(), actors);
}

#[test]
fn malformed_actor_is_an_error() {
    let (_tmp, db) = open_test_db();
    let chunk = ChunkPosition(0, -4000);
    let (cow_id, cow) = actor("minecraft:cow", 5);
    db.insert(digest_key(chunk, Dimension::Overworld), cow_id.0)
        .unwrap();
    db.insert(cow_id.key(), &cow[..cow.len() - 2]).unwrap();
    assert!(db.actors(chunk, Dimension::Overworld).is_err());
}

/// Every actor record of the test world reads and writes back to the same bytes.
#[test]
fn test_world_actors_roundtrip() {
    let (_tmp, db) = open_test_db();
    let mut checked = 0;
    let mut keys = db.keys().unwrap();
    for kv in &mut keys {
        if kv.key().starts_with(ACTOR_PREFIX) {
            let bytes = kv.value();
            assert_eq!(nbtx::to_le_bytes(&value(&bytes)).unwrap(), &*bytes);
            checked += 1;
        }
    }
    println!("{checked} actor records");
}
