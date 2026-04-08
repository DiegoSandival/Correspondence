use ouroboros::Celula;
use uuid::Uuid;

use crate::{
    AuthenticatedCellReadOutcome, CellDerivationOutcome, CellFusionOutcome, ChildSpec,
    DatabaseRegistry, DatabaseSummary, FreeMembraneReadOutcome, MembraneMutationOutcome,
    MembraneReadOutcome, Result,
};

pub const OPCODE_LIST_DATABASES: u8 = 0x01;
pub const OPCODE_CREATE_DATABASE: u8 = 0x02;
pub const OPCODE_DELETE_DATABASE: u8 = 0x03;
pub const OPCODE_DATABASE_STATUS: u8 = 0x04;
pub const OPCODE_PROVISION_GENESIS: u8 = 0x05;
pub const OPCODE_WRITE_MEMBRANE: u8 = 0x10;
pub const OPCODE_READ_MEMBRANE: u8 = 0x11;
pub const OPCODE_READ_MEMBRANE_FREE: u8 = 0x12;
pub const OPCODE_DELETE_MEMBRANE: u8 = 0x13;
pub const OPCODE_DERIVE_CELL: u8 = 0x20;
pub const OPCODE_FUSE_CELLS: u8 = 0x21;
pub const OPCODE_READ_AUTHENTICATED_CELL: u8 = 0x22;

pub const STATUS_OK: u8 = 0x00;
pub const STATUS_UNDEFINED: u8 = 0x01;
pub const STATUS_UNAUTHORIZED: u8 = 0x02;
pub const STATUS_ERROR: u8 = 0xff;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RawCommand {
    ListDatabases,
    CreateDatabase { max_records: u32 },
    DeleteDatabase { db_id: Uuid },
    DatabaseStatus { db_id: Uuid },
    ProvisionGenesis {
        db_id: Uuid,
        secret: Vec<u8>,
        genoma: u32,
        x: u32,
        y: u32,
        z: u32,
    },
    WriteMembrane {
        db_id: Uuid,
        key: String,
        value: Vec<u8>,
        cell_index: u32,
        secret: Vec<u8>,
    },
    ReadMembrane {
        db_id: Uuid,
        key: String,
        cell_index: u32,
        secret: Vec<u8>,
    },
    ReadMembraneFree {
        db_id: Uuid,
        key: String,
    },
    DeleteMembrane {
        db_id: Uuid,
        key: String,
        cell_index: u32,
        secret: Vec<u8>,
    },
    DeriveCell {
        db_id: Uuid,
        cell_index: u32,
        parent_secret: Vec<u8>,
        child_secret: Vec<u8>,
        child: ChildSpec,
    },
    FuseCells {
        db_id: Uuid,
        cell_index_a: u32,
        secret_a: Vec<u8>,
        cell_index_b: u32,
        secret_b: Vec<u8>,
        child_secret: Vec<u8>,
        child: ChildSpec,
    },
    ReadAuthenticatedCell {
        db_id: Uuid,
        cell_index: u32,
        secret: Vec<u8>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RawResponse {
    DatabaseList { databases: Vec<DatabaseSummary> },
    DatabaseCreated { summary: DatabaseSummary },
    DatabaseDeleted { db_id: Uuid },
    DatabaseStatus { summary: DatabaseSummary },
    GenesisProvisioned { db_id: Uuid, genesis_index: u32 },
    MembraneRead {
        db_id: Uuid,
        value: Vec<u8>,
        new_cell_index: u32,
    },
    MembraneReadFree {
        db_id: Uuid,
        value: Vec<u8>,
    },
    MembraneMutated {
        db_id: Uuid,
        new_cell_index: u32,
    },
    CellDerived {
        db_id: Uuid,
        deferred_index: u32,
        new_cell_index: u32,
    },
    CellsFused {
        db_id: Uuid,
        child_index: u32,
        new_cell_index_a: u32,
        new_cell_index_b: u32,
    },
    AuthenticatedCell {
        db_id: Uuid,
        celula: Celula,
        cell_index: u32,
    },
    Undefined {
        db_id: Option<Uuid>,
        cell_index: Option<u32>,
    },
    Unauthorized {
        db_id: Option<Uuid>,
        cell_index: Option<u32>,
        cell_index_a: Option<u32>,
        cell_index_b: Option<u32>,
    },
    Error { message: String },
}

pub fn decode_command(bytes: &[u8]) -> Result<RawCommand> {
    let mut reader = ByteReader::new(bytes);
    let opcode = reader.read_u8()?;

    let command = match opcode {
        OPCODE_LIST_DATABASES => RawCommand::ListDatabases,
        OPCODE_CREATE_DATABASE => RawCommand::CreateDatabase {
            max_records: reader.read_u32()?,
        },
        OPCODE_DELETE_DATABASE => RawCommand::DeleteDatabase {
            db_id: reader.read_uuid()?,
        },
        OPCODE_DATABASE_STATUS => RawCommand::DatabaseStatus {
            db_id: reader.read_uuid()?,
        },
        OPCODE_PROVISION_GENESIS => RawCommand::ProvisionGenesis {
            db_id: reader.read_uuid()?,
            secret: reader.read_bytes()?,
            genoma: reader.read_u32()?,
            x: reader.read_u32()?,
            y: reader.read_u32()?,
            z: reader.read_u32()?,
        },
        OPCODE_WRITE_MEMBRANE => RawCommand::WriteMembrane {
            db_id: reader.read_uuid()?,
            key: reader.read_string()?,
            value: reader.read_bytes()?,
            cell_index: reader.read_u32()?,
            secret: reader.read_bytes()?,
        },
        OPCODE_READ_MEMBRANE => RawCommand::ReadMembrane {
            db_id: reader.read_uuid()?,
            key: reader.read_string()?,
            cell_index: reader.read_u32()?,
            secret: reader.read_bytes()?,
        },
        OPCODE_READ_MEMBRANE_FREE => RawCommand::ReadMembraneFree {
            db_id: reader.read_uuid()?,
            key: reader.read_string()?,
        },
        OPCODE_DELETE_MEMBRANE => RawCommand::DeleteMembrane {
            db_id: reader.read_uuid()?,
            key: reader.read_string()?,
            cell_index: reader.read_u32()?,
            secret: reader.read_bytes()?,
        },
        OPCODE_DERIVE_CELL => RawCommand::DeriveCell {
            db_id: reader.read_uuid()?,
            cell_index: reader.read_u32()?,
            parent_secret: reader.read_bytes()?,
            child_secret: reader.read_bytes()?,
            child: decode_child_spec(&mut reader)?,
        },
        OPCODE_FUSE_CELLS => RawCommand::FuseCells {
            db_id: reader.read_uuid()?,
            cell_index_a: reader.read_u32()?,
            secret_a: reader.read_bytes()?,
            cell_index_b: reader.read_u32()?,
            secret_b: reader.read_bytes()?,
            child_secret: reader.read_bytes()?,
            child: decode_child_spec(&mut reader)?,
        },
        OPCODE_READ_AUTHENTICATED_CELL => RawCommand::ReadAuthenticatedCell {
            db_id: reader.read_uuid()?,
            cell_index: reader.read_u32()?,
            secret: reader.read_bytes()?,
        },
        _ => return Err(crate::Error::InvalidProtocolFrame("unknown opcode")),
    };

    reader.finish()?;
    Ok(command)
}

pub fn encode_response(response: &RawResponse) -> Vec<u8> {
    let mut writer = ByteWriter::new();

    match response {
        RawResponse::DatabaseList { databases } => {
            writer.write_u8(STATUS_OK);
            writer.write_u8(OPCODE_LIST_DATABASES);
            writer.write_u32(databases.len() as u32);
            for database in databases {
                writer.write_database_summary(database);
            }
        }
        RawResponse::DatabaseCreated { summary } => {
            writer.write_u8(STATUS_OK);
            writer.write_u8(OPCODE_CREATE_DATABASE);
            writer.write_database_summary(summary);
        }
        RawResponse::DatabaseDeleted { db_id } => {
            writer.write_u8(STATUS_OK);
            writer.write_u8(OPCODE_DELETE_DATABASE);
            writer.write_uuid(*db_id);
        }
        RawResponse::DatabaseStatus { summary } => {
            writer.write_u8(STATUS_OK);
            writer.write_u8(OPCODE_DATABASE_STATUS);
            writer.write_database_summary(summary);
        }
        RawResponse::GenesisProvisioned { db_id, genesis_index } => {
            writer.write_u8(STATUS_OK);
            writer.write_u8(OPCODE_PROVISION_GENESIS);
            writer.write_uuid(*db_id);
            writer.write_u32(*genesis_index);
        }
        RawResponse::MembraneRead {
            db_id,
            value,
            new_cell_index,
        } => {
            writer.write_u8(STATUS_OK);
            writer.write_u8(OPCODE_READ_MEMBRANE);
            writer.write_uuid(*db_id);
            writer.write_u32(*new_cell_index);
            writer.write_bytes(value);
        }
        RawResponse::MembraneReadFree { db_id, value } => {
            writer.write_u8(STATUS_OK);
            writer.write_u8(OPCODE_READ_MEMBRANE_FREE);
            writer.write_uuid(*db_id);
            writer.write_bytes(value);
        }
        RawResponse::MembraneMutated { db_id, new_cell_index } => {
            writer.write_u8(STATUS_OK);
            writer.write_u8(OPCODE_WRITE_MEMBRANE);
            writer.write_uuid(*db_id);
            writer.write_u32(*new_cell_index);
        }
        RawResponse::CellDerived {
            db_id,
            deferred_index,
            new_cell_index,
        } => {
            writer.write_u8(STATUS_OK);
            writer.write_u8(OPCODE_DERIVE_CELL);
            writer.write_uuid(*db_id);
            writer.write_u32(*deferred_index);
            writer.write_u32(*new_cell_index);
        }
        RawResponse::CellsFused {
            db_id,
            child_index,
            new_cell_index_a,
            new_cell_index_b,
        } => {
            writer.write_u8(STATUS_OK);
            writer.write_u8(OPCODE_FUSE_CELLS);
            writer.write_uuid(*db_id);
            writer.write_u32(*child_index);
            writer.write_u32(*new_cell_index_a);
            writer.write_u32(*new_cell_index_b);
        }
        RawResponse::AuthenticatedCell {
            db_id,
            celula,
            cell_index,
        } => {
            writer.write_u8(STATUS_OK);
            writer.write_u8(OPCODE_READ_AUTHENTICATED_CELL);
            writer.write_uuid(*db_id);
            writer.write_u32(*cell_index);
            writer.write_fixed_bytes(&celula.serialize());
        }
        RawResponse::Undefined { db_id, cell_index } => {
            writer.write_u8(STATUS_UNDEFINED);
            writer.write_u8(0);
            writer.write_optional_uuid(*db_id);
            writer.write_optional_u32(*cell_index);
        }
        RawResponse::Unauthorized {
            db_id,
            cell_index,
            cell_index_a,
            cell_index_b,
        } => {
            writer.write_u8(STATUS_UNAUTHORIZED);
            writer.write_u8(0);
            writer.write_optional_uuid(*db_id);
            writer.write_optional_u32(*cell_index);
            writer.write_optional_u32(*cell_index_a);
            writer.write_optional_u32(*cell_index_b);
        }
        RawResponse::Error { message } => {
            writer.write_u8(STATUS_ERROR);
            writer.write_u8(0);
            writer.write_string(message);
        }
    }

    writer.finish()
}

pub fn execute_command(registry: &mut DatabaseRegistry, command: RawCommand) -> RawResponse {
    match execute_command_inner(registry, command) {
        Ok(response) => response,
        Err(error) => RawResponse::Error {
            message: error.to_string(),
        },
    }
}

fn execute_command_inner(registry: &mut DatabaseRegistry, command: RawCommand) -> Result<RawResponse> {
    match command {
        RawCommand::ListDatabases => Ok(RawResponse::DatabaseList {
            databases: registry.list_databases()?,
        }),
        RawCommand::CreateDatabase { max_records } => Ok(RawResponse::DatabaseCreated {
            summary: registry.create_database(max_records)?,
        }),
        RawCommand::DeleteDatabase { db_id } => {
            if registry.delete_database(db_id)? {
                Ok(RawResponse::DatabaseDeleted { db_id })
            } else {
                Ok(RawResponse::Undefined {
                    db_id: Some(db_id),
                    cell_index: None,
                })
            }
        }
        RawCommand::DatabaseStatus { db_id } => Ok(RawResponse::DatabaseStatus {
            summary: registry.database_summary(db_id)?,
        }),
        RawCommand::ProvisionGenesis {
            db_id,
            secret,
            genoma,
            x,
            y,
            z,
        } => {
            let genesis_index = registry.with_database_mut(db_id, |correspondence| {
                correspondence.provision_genesis_with_coordinates(&secret, genoma, x, y, z)
            })?;
            Ok(RawResponse::GenesisProvisioned { db_id, genesis_index })
        }
        RawCommand::WriteMembrane {
            db_id,
            key,
            value,
            cell_index,
            secret,
        } => registry.with_database_mut(db_id, |correspondence| {
            match correspondence.write_membrane(&key, &value, cell_index, &secret)? {
                MembraneMutationOutcome::Ok { new_cell_index } => {
                    Ok(RawResponse::MembraneMutated { db_id, new_cell_index })
                }
                MembraneMutationOutcome::Undefined { cell_index } => Ok(RawResponse::Undefined {
                    db_id: Some(db_id),
                    cell_index: Some(cell_index),
                }),
                MembraneMutationOutcome::Unauthorized(details) => Ok(RawResponse::Unauthorized {
                    db_id: Some(db_id),
                    cell_index: details.cell_index,
                    cell_index_a: None,
                    cell_index_b: None,
                }),
            }
        }),
        RawCommand::ReadMembrane {
            db_id,
            key,
            cell_index,
            secret,
        } => registry.with_database_mut(db_id, |correspondence| {
            match correspondence.read_membrane(&key, cell_index, &secret)? {
                MembraneReadOutcome::Ok { value, new_cell_index } => Ok(RawResponse::MembraneRead {
                    db_id,
                    value,
                    new_cell_index,
                }),
                MembraneReadOutcome::Undefined { cell_index } => Ok(RawResponse::Undefined {
                    db_id: Some(db_id),
                    cell_index: Some(cell_index),
                }),
                MembraneReadOutcome::Unauthorized(details) => Ok(RawResponse::Unauthorized {
                    db_id: Some(db_id),
                    cell_index: details.cell_index,
                    cell_index_a: None,
                    cell_index_b: None,
                }),
            }
        }),
        RawCommand::ReadMembraneFree { db_id, key } => registry.with_database_mut(db_id, |correspondence| {
            match correspondence.read_membrane_free(&key)? {
                FreeMembraneReadOutcome::Ok { value } => Ok(RawResponse::MembraneReadFree { db_id, value }),
                FreeMembraneReadOutcome::Undefined => Ok(RawResponse::Undefined {
                    db_id: Some(db_id),
                    cell_index: None,
                }),
                FreeMembraneReadOutcome::Unauthorized => Ok(RawResponse::Unauthorized {
                    db_id: Some(db_id),
                    cell_index: None,
                    cell_index_a: None,
                    cell_index_b: None,
                }),
            }
        }),
        RawCommand::DeleteMembrane {
            db_id,
            key,
            cell_index,
            secret,
        } => registry.with_database_mut(db_id, |correspondence| {
            match correspondence.delete_membrane(&key, cell_index, &secret)? {
                MembraneMutationOutcome::Ok { new_cell_index } => {
                    Ok(RawResponse::MembraneMutated { db_id, new_cell_index })
                }
                MembraneMutationOutcome::Undefined { cell_index } => Ok(RawResponse::Undefined {
                    db_id: Some(db_id),
                    cell_index: Some(cell_index),
                }),
                MembraneMutationOutcome::Unauthorized(details) => Ok(RawResponse::Unauthorized {
                    db_id: Some(db_id),
                    cell_index: details.cell_index,
                    cell_index_a: None,
                    cell_index_b: None,
                }),
            }
        }),
        RawCommand::DeriveCell {
            db_id,
            cell_index,
            parent_secret,
            child_secret,
            child,
        } => registry.with_database_mut(db_id, |correspondence| {
            match correspondence.derive_cell(cell_index, &parent_secret, &child_secret, child)? {
                CellDerivationOutcome::Ok {
                    deferred_index,
                    new_cell_index,
                } => Ok(RawResponse::CellDerived {
                    db_id,
                    deferred_index,
                    new_cell_index,
                }),
                CellDerivationOutcome::Unauthorized(details) => Ok(RawResponse::Unauthorized {
                    db_id: Some(db_id),
                    cell_index: details.cell_index,
                    cell_index_a: None,
                    cell_index_b: None,
                }),
            }
        }),
        RawCommand::FuseCells {
            db_id,
            cell_index_a,
            secret_a,
            cell_index_b,
            secret_b,
            child_secret,
            child,
        } => registry.with_database_mut(db_id, |correspondence| {
            match correspondence.fuse_cells(
                cell_index_a,
                &secret_a,
                cell_index_b,
                &secret_b,
                &child_secret,
                child,
            )? {
                CellFusionOutcome::Ok {
                    child_index,
                    new_cell_index_a,
                    new_cell_index_b,
                } => Ok(RawResponse::CellsFused {
                    db_id,
                    child_index,
                    new_cell_index_a,
                    new_cell_index_b,
                }),
                CellFusionOutcome::Unauthorized {
                    cell_index_a,
                    cell_index_b,
                } => Ok(RawResponse::Unauthorized {
                    db_id: Some(db_id),
                    cell_index: None,
                    cell_index_a,
                    cell_index_b,
                }),
            }
        }),
        RawCommand::ReadAuthenticatedCell {
            db_id,
            cell_index,
            secret,
        } => registry.with_database_mut(db_id, |correspondence| {
            match correspondence.read_authenticated_cell(cell_index, &secret)? {
                AuthenticatedCellReadOutcome::Ok { celula, cell_index } => Ok(
                    RawResponse::AuthenticatedCell {
                        db_id,
                        celula,
                        cell_index,
                    },
                ),
                AuthenticatedCellReadOutcome::Unauthorized => Ok(RawResponse::Unauthorized {
                    db_id: Some(db_id),
                    cell_index: None,
                    cell_index_a: None,
                    cell_index_b: None,
                }),
            }
        }),
    }
}

fn decode_child_spec(reader: &mut ByteReader<'_>) -> Result<ChildSpec> {
    Ok(ChildSpec {
        salt: reader.read_fixed_16()?,
        genoma: reader.read_u32()?,
        x: reader.read_u32()?,
        y: reader.read_u32()?,
        z: reader.read_u32()?,
    })
}

struct ByteReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_u8(&mut self) -> Result<u8> {
        if self.offset >= self.bytes.len() {
            return Err(crate::Error::InvalidProtocolFrame("unexpected end of frame"));
        }
        let value = self.bytes[self.offset];
        self.offset += 1;
        Ok(value)
    }

    fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().expect("fixed width u32")))
    }

    fn read_uuid(&mut self) -> Result<Uuid> {
        let bytes = self.read_exact(16)?;
        Uuid::from_slice(bytes).map_err(|_| crate::Error::InvalidProtocolFrame("invalid uuid bytes"))
    }

    fn read_bytes(&mut self) -> Result<Vec<u8>> {
        let len = self.read_u32()? as usize;
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_string(&mut self) -> Result<String> {
        let bytes = self.read_bytes()?;
        String::from_utf8(bytes)
            .map_err(|_| crate::Error::InvalidProtocolFrame("invalid utf-8 string"))
    }

    fn read_fixed_16(&mut self) -> Result<[u8; 16]> {
        Ok(self.read_exact(16)?.try_into().expect("fixed width 16"))
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(crate::Error::InvalidProtocolFrame("frame length overflow"))?;
        if end > self.bytes.len() {
            return Err(crate::Error::InvalidProtocolFrame("unexpected end of frame"));
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn finish(&self) -> Result<()> {
        if self.offset != self.bytes.len() {
            return Err(crate::Error::InvalidProtocolFrame("trailing bytes in frame"));
        }
        Ok(())
    }
}

struct ByteWriter {
    bytes: Vec<u8>,
}

impl ByteWriter {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_uuid(&mut self, value: Uuid) {
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn write_optional_uuid(&mut self, value: Option<Uuid>) {
        match value {
            Some(value) => {
                self.write_u8(1);
                self.write_uuid(value);
            }
            None => self.write_u8(0),
        }
    }

    fn write_bytes(&mut self, value: &[u8]) {
        self.write_u32(value.len() as u32);
        self.bytes.extend_from_slice(value);
    }

    fn write_fixed_bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn write_string(&mut self, value: &str) {
        self.write_bytes(value.as_bytes());
    }

    fn write_optional_u32(&mut self, value: Option<u32>) {
        match value {
            Some(value) => {
                self.write_u8(1);
                self.write_u32(value);
            }
            None => self.write_u8(0),
        }
    }

    fn write_database_summary(&mut self, summary: &DatabaseSummary) {
        self.write_uuid(summary.db_id);
        self.write_u32(summary.max_records);
        self.write_optional_u32(summary.genesis_index);
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("correspondence-transport-{unique}"));
        fs::create_dir_all(&path).expect("temporary directory should be created");
        path
    }

    #[test]
    fn decode_create_database_command() {
        let mut bytes = vec![OPCODE_CREATE_DATABASE];
        bytes.extend_from_slice(&64u32.to_le_bytes());

        let command = decode_command(&bytes).expect("command should decode");
        assert_eq!(command, RawCommand::CreateDatabase { max_records: 64 });
    }

    #[test]
    fn execute_create_list_and_provision_commands() {
        let dir = temp_dir();
        let mut registry = DatabaseRegistry::new(&dir).expect("registry should initialize");

        let created = execute_command(&mut registry, RawCommand::CreateDatabase { max_records: 32 });
        let RawResponse::DatabaseCreated { summary } = created else {
            panic!("create database should return created summary");
        };
        assert_eq!(summary.max_records, 32);

        let listed = execute_command(&mut registry, RawCommand::ListDatabases);
        let RawResponse::DatabaseList { databases } = listed else {
            panic!("list databases should return database list");
        };
        assert_eq!(databases.len(), 1);

        let provisioned = execute_command(
            &mut registry,
            RawCommand::ProvisionGenesis {
                db_id: summary.db_id,
                secret: b"diarsaba".to_vec(),
                genoma: crate::Correspondence::default_genesis_genoma(),
                x: 1,
                y: 2,
                z: 3,
            },
        );
        assert!(matches!(
            provisioned,
            RawResponse::GenesisProvisioned {
                db_id,
                genesis_index: 0
            } if db_id == summary.db_id
        ));

        fs::remove_dir_all(dir).expect("temporary directory should be removed");
    }
}
