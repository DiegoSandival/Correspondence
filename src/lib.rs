mod error;
mod model;
mod storage;

use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use ouroboros::{Celula, Config as OuroborosConfig, Genoma, OuroborosDb};
use redb::Database;

pub use crate::error::{Error, Result};
pub use crate::model::{
    CellReadResult, ChildSpec, CorrespondenceConfig, CrossResult, DeferResult, FreeReadResult,
    Membrane, MutationResult, ReadResult, ResolvedCell, Unauthorized,
};
use crate::storage::{
    delete_membrane, get_membrane, get_meta_u32, open_database, set_membrane, set_meta_u32,
    META_GENESIS_INDEX,
};

pub struct Correspondence {
    ouroboros: OuroborosDb,
    membranes: Database,
    max_records: u32,
}

impl Correspondence {
    pub fn open(config: CorrespondenceConfig) -> Result<Self> {
        let genesis_secret_env = config.genesis_secret_env.clone();
        let mut correspondence = Self::open_uninitialized(config)?;
        correspondence.initialize_genesis_from_env(&genesis_secret_env)?;
        Ok(correspondence)
    }

    pub fn open_uninitialized(config: CorrespondenceConfig) -> Result<Self> {
        let ouroboros_config = OuroborosConfig::from_path(&config.ouroboros_config_path)?;
        let ouroboros = OuroborosDb::open_with_config(&ouroboros_config)?;
        let membranes = open_database(&config.membranes_path)?;

        Ok(Self {
            ouroboros,
            membranes,
            max_records: ouroboros_config.max_records,
        })
    }

    pub fn default_genesis_genoma() -> u32 {
        genesis_genoma()
    }

    pub fn genesis_index(&self) -> Result<u32> {
        get_meta_u32(&self.membranes, META_GENESIS_INDEX)?.ok_or(Error::BootstrapMetadataMissing)
    }

    pub fn initialize_genesis_from_env(&mut self, genesis_secret_env: &str) -> Result<u32> {
        if let Some(index) = get_meta_u32(&self.membranes, META_GENESIS_INDEX)? {
            return Ok(index);
        }

        let secret = env::var(genesis_secret_env)
            .map_err(|_| Error::MissingGenesisSecret(genesis_secret_env.to_string()))?;
        self.initialize_genesis(secret.as_bytes(), genesis_genoma())
    }

    pub fn initialize_genesis(&mut self, secret: &[u8], genoma: u32) -> Result<u32> {
        if let Some(index) = get_meta_u32(&self.membranes, META_GENESIS_INDEX)? {
            return Ok(index);
        }

        self.ensure_ring_is_empty()?;
        let genesis_cell = Celula::with_secret(genesis_salt(), secret, genoma, 0, 0, 0);
        let genesis_index = self.ouroboros.append(genesis_cell)?;
        set_meta_u32(&self.membranes, META_GENESIS_INDEX, genesis_index)?;
        Ok(genesis_index)
    }

    pub fn read(&mut self, key: &str, cell_index: u32, secret: &[u8]) -> Result<ReadResult> {
        let Some(active) = self.resolve_cell(cell_index, secret)? else {
            return Ok(ReadResult::Unauthorized(Unauthorized { cell_index: None }));
        };

        let Some(membrane) = get_membrane(&self.membranes, key)? else {
            return Ok(ReadResult::Undefined {
                cell_index: active.index,
            });
        };

        let required_flag = if membrane.owner_index == active.original_index {
            Genoma::LEER_SELF
        } else {
            Genoma::LEER_ANY
        };

        if (active.celula.genoma & required_flag) == 0 {
            return Ok(ReadResult::Unauthorized(Unauthorized {
                cell_index: Some(active.index),
            }));
        }

        let new_cell_index = self
            .refresh(active.index)?
            .ok_or(Error::BootstrapMetadataMissing)?;

        Ok(ReadResult::Ok {
            value: membrane.value,
            new_cell_index,
        })
    }

    pub fn read_free(&self, key: &str) -> Result<FreeReadResult> {
        let Some(membrane) = get_membrane(&self.membranes, key)? else {
            return Ok(FreeReadResult::Undefined);
        };

        let Some(owner) = self.resolve_cell_system(membrane.owner_index)? else {
            return Ok(FreeReadResult::Unauthorized);
        };

        if (owner.celula.genoma & Genoma::LEER_LIBRE) == 0 {
            return Ok(FreeReadResult::Unauthorized);
        }

        Ok(FreeReadResult::Ok {
            value: membrane.value,
        })
    }

    pub fn write(
        &mut self,
        key: &str,
        value: impl AsRef<[u8]>,
        cell_index: u32,
        secret: &[u8],
    ) -> Result<MutationResult> {
        let Some(active) = self.resolve_cell(cell_index, secret)? else {
            return Ok(MutationResult::Unauthorized(Unauthorized { cell_index: None }));
        };

        let value = value.as_ref();
        match get_membrane(&self.membranes, key)? {
            None => {
                set_membrane(
                    &self.membranes,
                    key,
                    &Membrane {
                        owner_index: active.original_index,
                        value: value.to_vec(),
                    },
                )?;
            }
            Some(mut membrane) => {
                let required_flag = if membrane.owner_index == active.original_index {
                    Genoma::ESCRIBIR_SELF
                } else {
                    Genoma::ESCRIBIR_ANY
                };

                if (active.celula.genoma & required_flag) == 0 {
                    return Ok(MutationResult::Unauthorized(Unauthorized {
                        cell_index: Some(active.index),
                    }));
                }

                membrane.value = value.to_vec();
                set_membrane(&self.membranes, key, &membrane)?;
            }
        }

        let new_cell_index = self
            .refresh(active.index)?
            .ok_or(Error::BootstrapMetadataMissing)?;

        Ok(MutationResult::Ok { new_cell_index })
    }

    pub fn delete(&mut self, key: &str, cell_index: u32, secret: &[u8]) -> Result<MutationResult> {
        let Some(active) = self.resolve_cell(cell_index, secret)? else {
            return Ok(MutationResult::Unauthorized(Unauthorized { cell_index: None }));
        };

        let Some(membrane) = get_membrane(&self.membranes, key)? else {
            return Ok(MutationResult::Undefined {
                cell_index: active.index,
            });
        };

        let required_flag = if membrane.owner_index == active.original_index {
            Genoma::BORRAR_SELF
        } else {
            Genoma::BORRAR_ANY
        };

        if (active.celula.genoma & required_flag) == 0 {
            return Ok(MutationResult::Unauthorized(Unauthorized {
                cell_index: Some(active.index),
            }));
        }

        let _ = delete_membrane(&self.membranes, key)?;
        let new_cell_index = self
            .refresh(active.index)?
            .ok_or(Error::BootstrapMetadataMissing)?;

        Ok(MutationResult::Ok { new_cell_index })
    }

    pub fn defer(
        &mut self,
        cell_index: u32,
        parent_secret: &[u8],
        child_secret: &[u8],
        child: ChildSpec,
    ) -> Result<DeferResult> {
        let Some(active) = self.resolve_cell(cell_index, parent_secret)? else {
            return Ok(DeferResult::Unauthorized(Unauthorized { cell_index: None }));
        };

        if (active.celula.genoma & Genoma::DIFERIR) == 0 {
            return Ok(DeferResult::Unauthorized(Unauthorized {
                cell_index: Some(active.index),
            }));
        }

        if (active.celula.genoma & child.genoma) != child.genoma {
            return Ok(DeferResult::Unauthorized(Unauthorized {
                cell_index: Some(active.index),
            }));
        }

        let child_cell = Celula::with_secret(
            child.salt,
            child_secret,
            child.genoma,
            child.x,
            child.y,
            child.z,
        );

        let deferred_index = self.ouroboros.append(child_cell)?;
        let new_cell_index = self
            .refresh(active.index)?
            .ok_or(Error::BootstrapMetadataMissing)?;

        Ok(DeferResult::Ok {
            deferred_index,
            new_cell_index,
        })
    }

    pub fn cross(
        &mut self,
        cell_index_a: u32,
        secret_a: &[u8],
        cell_index_b: u32,
        secret_b: &[u8],
        child_secret: &[u8],
        child: ChildSpec,
    ) -> Result<CrossResult> {
        let active_a = self.resolve_cell(cell_index_a, secret_a)?;
        let active_b = self.resolve_cell(cell_index_b, secret_b)?;

        if active_a.is_none() || active_b.is_none() {
            return Ok(CrossResult::Unauthorized {
                cell_index_a: active_a.as_ref().map(|cell| cell.index),
                cell_index_b: active_b.as_ref().map(|cell| cell.index),
            });
        }

        let active_a = active_a.expect("checked is_some above");
        let active_b = active_b.expect("checked is_some above");

        if (active_a.celula.genoma & Genoma::FUSIONAR) == 0
            || (active_b.celula.genoma & Genoma::FUSIONAR) == 0
        {
            return Ok(CrossResult::Unauthorized {
                cell_index_a: Some(active_a.index),
                cell_index_b: Some(active_b.index),
            });
        }

        let child_genoma = active_a.celula.genoma | active_b.celula.genoma;
        let child_cell = Celula::with_secret(
            child.salt,
            child_secret,
            child_genoma,
            child.x,
            child.y,
            child.z,
        );

        let child_index = self.ouroboros.append(child_cell)?;
        let new_cell_index_a = self
            .refresh(active_a.index)?
            .ok_or(Error::BootstrapMetadataMissing)?;
        let new_cell_index_b = self
            .refresh(active_b.index)?
            .ok_or(Error::BootstrapMetadataMissing)?;

        Ok(CrossResult::Ok {
            child_index,
            new_cell_index_a,
            new_cell_index_b,
        })
    }

    pub fn read_cell(&self, cell_index: u32, secret: &[u8]) -> Result<CellReadResult> {
        let Some(active) = self.resolve_cell(cell_index, secret)? else {
            return Ok(CellReadResult::Unauthorized);
        };

        Ok(CellReadResult::Ok {
            celula: active.celula,
            cell_index: active.index,
        })
    }

    fn refresh(&mut self, cell_index: u32) -> Result<Option<u32>> {
        let Some(original_cell) = self.read_cell_system_raw(cell_index)? else {
            return Ok(None);
        };

        let renewed_cell = Celula::new(
            original_cell.hash,
            original_cell.salt,
            original_cell.genoma,
            original_cell.x,
            original_cell.y,
            original_cell.z,
        );

        let new_index = self.ouroboros.append(renewed_cell)?;
        let migrated_cell = Celula::new(
            original_cell.hash,
            original_cell.salt,
            original_cell.genoma | Genoma::MIGRADA,
            new_index,
            original_cell.y,
            original_cell.z,
        );
        self.ouroboros.update(cell_index, migrated_cell)?;

        Ok(Some(new_index))
    }

    fn resolve_cell(&self, cell_index: u32, secret: &[u8]) -> Result<Option<ResolvedCell>> {
        self.resolve_cell_inner(cell_index, |index| self.read_cell_auth_raw(index, secret))
    }

    fn resolve_cell_system(&self, cell_index: u32) -> Result<Option<ResolvedCell>> {
        self.resolve_cell_inner(cell_index, |index| self.read_cell_system_raw(index))
    }

    fn resolve_cell_inner<F>(&self, cell_index: u32, mut read: F) -> Result<Option<ResolvedCell>>
    where
        F: FnMut(u32) -> Result<Option<Celula>>,
    {
        let mut index = cell_index;
        let Some(mut cell) = read(index)? else {
            return Ok(None);
        };

        for _ in 0..self.max_records {
            if (cell.genoma & Genoma::MIGRADA) == 0 {
                return Ok(Some(ResolvedCell {
                    celula: cell,
                    index,
                    original_index: cell_index,
                }));
            }

            index = cell.x;
            let Some(next_cell) = read(index)? else {
                return Ok(None);
            };
            cell = next_cell;
        }

        Err(Error::ResolutionLoop {
            start_index: cell_index,
        })
    }

    fn read_cell_auth_raw(&self, cell_index: u32, secret: &[u8]) -> Result<Option<Celula>> {
        match self.ouroboros.read_auth(cell_index, secret) {
            Ok(cell) if cell.is_empty() => Ok(None),
            Ok(cell) => Ok(Some(cell)),
            Err(ouroboros::Error::Unauthorized) => Ok(None),
            Err(ouroboros::Error::IndexOutOfBounds { .. }) => Ok(None),
            Err(error) => Err(Error::from(error)),
        }
    }

    fn read_cell_system_raw(&self, cell_index: u32) -> Result<Option<Celula>> {
        match self.ouroboros.read(cell_index) {
            Ok(cell) if cell.is_empty() => Ok(None),
            Ok(cell) => Ok(Some(cell)),
            Err(ouroboros::Error::IndexOutOfBounds { .. }) => Ok(None),
            Err(error) => Err(Error::from(error)),
        }
    }

    fn ensure_ring_is_empty(&self) -> Result<()> {
        match self.ouroboros.read(0) {
            Ok(cell) if cell.is_empty() => Ok(()),
            Ok(_) => Err(Error::BootstrapMetadataMissing),
            Err(ouroboros::Error::IndexOutOfBounds { .. }) if self.max_records == 0 => {
                Err(Error::BootstrapMetadataMissing)
            }
            Err(error) => Err(Error::from(error)),
        }
    }
}

fn genesis_genoma() -> u32 {
    Genoma::LEER_SELF
        | Genoma::LEER_ANY
        | Genoma::ESCRIBIR_SELF
        | Genoma::ESCRIBIR_ANY
        | Genoma::BORRAR_SELF
        | Genoma::BORRAR_ANY
        | Genoma::DIFERIR
        | Genoma::LEER_LIBRE
}

fn genesis_salt() -> [u8; 16] {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after unix epoch")
        .as_nanos();
    nanos.to_le_bytes()
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
        let path = std::env::temp_dir().join(format!("correspondence-{unique}"));
        fs::create_dir_all(&path).expect("temporary directory should be created");
        path
    }

    fn write_ouroboros_config(dir: &std::path::Path) -> PathBuf {
        let config_path = dir.join("ouroboros.toml");
        fs::write(&config_path, "data_path = \"ouroboros.db\"\nmax_records = 32\n")
            .expect("config file should be written");
        config_path
    }

    #[test]
    fn bootstrap_creates_genesis_cell() {
        let dir = temp_dir();
        let config_path = write_ouroboros_config(&dir);
        let membranes_path = dir.join("membranes.redb");
        let env_name = format!("GENESIS_SECRET_TEST_{}", std::process::id());
        unsafe {
            std::env::set_var(&env_name, "diarsaba");
        }

        let config = CorrespondenceConfig::new(config_path, membranes_path)
            .with_genesis_secret_env(&env_name);
        let correspondence = Correspondence::open(config).expect("correspondence should open");

        assert_eq!(correspondence.genesis_index().expect("genesis index"), 0);

        fs::remove_dir_all(dir).expect("temporary directory should be removed");
    }

    #[test]
    fn explicit_genesis_initialization_works() {
        let dir = temp_dir();
        let config_path = write_ouroboros_config(&dir);
        let membranes_path = dir.join("membranes.redb");

        let config = CorrespondenceConfig::new(config_path, membranes_path);
        let mut correspondence =
            Correspondence::open_uninitialized(config).expect("correspondence should open");

        let index = correspondence
            .initialize_genesis(b"diarsaba", Correspondence::default_genesis_genoma())
            .expect("genesis should initialize");

        assert_eq!(index, 0);
        assert_eq!(correspondence.genesis_index().expect("genesis index"), 0);

        fs::remove_dir_all(dir).expect("temporary directory should be removed");
    }

    #[test]
    fn genesis_genoma_matches_requested_permissions() {
        let expected = Genoma::LEER_SELF
            | Genoma::LEER_ANY
            | Genoma::ESCRIBIR_SELF
            | Genoma::ESCRIBIR_ANY
            | Genoma::BORRAR_SELF
            | Genoma::BORRAR_ANY
            | Genoma::DIFERIR
            | Genoma::LEER_LIBRE;

        assert_eq!(genesis_genoma(), expected);
        assert_eq!(genesis_genoma() & Genoma::FUSIONAR, 0);
        assert_eq!(genesis_genoma() & Genoma::CLONAR, 0);
        assert_eq!(genesis_genoma() & Genoma::DOMINANTE, 0);
    }

    #[test]
    fn write_read_delete_round_trip_refreshes_index() {
        let dir = temp_dir();
        let config_path = write_ouroboros_config(&dir);
        let membranes_path = dir.join("membranes.redb");
        let env_name = format!("GENESIS_SECRET_TEST_{}", std::process::id());
        unsafe {
            std::env::set_var(&env_name, "diarsaba");
        }

        let config = CorrespondenceConfig::new(config_path, membranes_path)
            .with_genesis_secret_env(&env_name);
        let mut correspondence = Correspondence::open(config).expect("correspondence should open");

        let genesis_index = correspondence.genesis_index().expect("genesis index");
        let MutationResult::Ok { new_cell_index } = correspondence
            .write("hola", b"mundo", genesis_index, b"diarsaba")
            .expect("write should succeed")
        else {
            panic!("write should return ok");
        };

        let ReadResult::Ok {
            value,
            new_cell_index: read_index,
        } = correspondence
            .read("hola", new_cell_index, b"diarsaba")
            .expect("read should succeed")
        else {
            panic!("read should return ok");
        };
        assert_eq!(value, b"mundo");

        let FreeReadResult::Ok { value } = correspondence
            .read_free("hola")
            .expect("free read should succeed")
        else {
            panic!("free read should return ok");
        };
        assert_eq!(value, b"mundo");

        let MutationResult::Ok {
            new_cell_index: delete_index,
        } = correspondence
            .delete("hola", read_index, b"diarsaba")
            .expect("delete should succeed")
        else {
            panic!("delete should return ok");
        };

        let MutationResult::Undefined { cell_index } = correspondence
            .delete("hola", delete_index, b"diarsaba")
            .expect("second delete should succeed")
        else {
            panic!("second delete should return undefined");
        };
        assert_eq!(cell_index, delete_index);

        fs::remove_dir_all(dir).expect("temporary directory should be removed");
    }
}