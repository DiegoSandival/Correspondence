use redb::{Database, ReadableDatabase, TableDefinition};

use crate::error::{Error, Result};
use crate::model::Membrane;

pub const MEMBRANES_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("membranes");
pub const META_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("meta");
pub const META_GENESIS_INDEX: &str = "genesis_index";

pub fn open_database(path: &std::path::Path) -> Result<Database> {
    let db = Database::create(path)?;
    let write_txn = db.begin_write()?;
    {
        let _ = write_txn.open_table(MEMBRANES_TABLE)?;
        let _ = write_txn.open_table(META_TABLE)?;
    }
    write_txn.commit()?;
    Ok(db)
}

pub fn get_membrane(db: &Database, key: &str) -> Result<Option<Membrane>> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(MEMBRANES_TABLE)?;
    let Some(value) = table.get(key)? else {
        return Ok(None);
    };

    decode_membrane(value.value()).map(Some)
}

pub fn set_membrane(db: &Database, key: &str, membrane: &Membrane) -> Result<()> {
    let bytes = encode_membrane(membrane);
    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(MEMBRANES_TABLE)?;
        table.insert(key, bytes.as_slice())?;
    }
    write_txn.commit()?;
    Ok(())
}

pub fn delete_membrane(db: &Database, key: &str) -> Result<bool> {
    let write_txn = db.begin_write()?;
    let removed = {
        let mut table = write_txn.open_table(MEMBRANES_TABLE)?;
        table.remove(key)?.is_some()
    };
    write_txn.commit()?;
    Ok(removed)
}

pub fn get_meta_u32(db: &Database, key: &str) -> Result<Option<u32>> {
    let read_txn = db.begin_read()?;
    let table = read_txn.open_table(META_TABLE)?;
    let Some(value) = table.get(key)? else {
        return Ok(None);
    };

    let bytes = value.value();
    if bytes.len() != 4 {
        return Err(Error::InvalidMembraneRecord);
    }

    Ok(Some(u32::from_le_bytes(bytes.try_into().expect("fixed width u32"))))
}

pub fn set_meta_u32(db: &Database, key: &str, value: u32) -> Result<()> {
    let bytes = value.to_le_bytes();
    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(META_TABLE)?;
        table.insert(key, bytes.as_slice())?;
    }
    write_txn.commit()?;
    Ok(())
}

fn encode_membrane(membrane: &Membrane) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + membrane.value.len());
    bytes.extend_from_slice(&membrane.owner_index.to_le_bytes());
    bytes.extend_from_slice(&membrane.value);
    bytes
}

fn decode_membrane(bytes: &[u8]) -> Result<Membrane> {
    if bytes.len() < 4 {
        return Err(Error::InvalidMembraneRecord);
    }

    Ok(Membrane {
        owner_index: u32::from_le_bytes(bytes[0..4].try_into().expect("fixed width owner index")),
        value: bytes[4..].to_vec(),
    })
}
