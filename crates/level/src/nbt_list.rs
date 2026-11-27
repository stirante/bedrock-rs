//! Records that hold several NBT compounds back to back, with no count or framing: block entities
//! (`KeyVariant::BlockEntity`) and the legacy actor record (`KeyVariant::Entity`).

use nbtx::Value;

use crate::error::Result;

/// Reads every compound of a record, in order. An empty record is an empty list.
///
/// # Errors
///
/// Returns an NBT error if a compound is truncated or malformed.
pub fn read(mut bytes: &[u8]) -> Result<Vec<Value>> {
    let mut values = Vec::new();
    while !bytes.is_empty() {
        values.push(nbtx::from_le_bytes(&mut bytes)?);
    }
    Ok(values)
}

/// Writes `values` back to back, the way the game stores them.
///
/// # Errors
///
/// Returns an NBT error if a value cannot be serialized.
pub fn write(values: &[Value]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for value in values {
        nbtx::to_le_bytes_in(&mut out, value)?;
    }
    Ok(out)
}
