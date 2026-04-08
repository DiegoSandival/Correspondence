use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use ouroboros::{Celula, Genoma};

use crate::storage::{get_meta_u32, set_meta_u32, META_GENESIS_INDEX};
use crate::{Correspondence, Error, Result};

impl Correspondence {
    pub fn default_genesis_genoma() -> u32 {
        genesis_genoma()
    }

    pub fn provision_genesis_from_env(&mut self, genesis_secret_env: &str) -> Result<u32> {
        if let Some(index) = get_meta_u32(&self.membranes, META_GENESIS_INDEX)? {
            return Ok(index);
        }

        let secret = env::var(genesis_secret_env)
            .map_err(|_| Error::MissingGenesisSecret(genesis_secret_env.to_string()))?;
        self.provision_genesis(secret.as_bytes(), genesis_genoma())
    }

    pub fn provision_genesis(&mut self, secret: &[u8], genoma: u32) -> Result<u32> {
        self.provision_genesis_with_coordinates(secret, genoma, 0, 0, 0)
    }

    pub fn provision_genesis_with_coordinates(
        &mut self,
        secret: &[u8],
        genoma: u32,
        x: u32,
        y: u32,
        z: u32,
    ) -> Result<u32> {
        if let Some(index) = get_meta_u32(&self.membranes, META_GENESIS_INDEX)? {
            return Ok(index);
        }

        self.ensure_ring_is_empty()?;
        let genesis_cell = Celula::with_secret(genesis_salt(), secret, genoma, x, y, z);
        let genesis_index = self.ouroboros.append(genesis_cell)?;
        set_meta_u32(&self.membranes, META_GENESIS_INDEX, genesis_index)?;
        Ok(genesis_index)
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

pub(crate) fn genesis_genoma() -> u32 {
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