use std::sync::Arc;

use dotenvy::dotenv;
use ouroboros_db::{OuroborosConfig, OuroborosDB};
use redb::{Database, TableDefinition};
use tokio::sync::RwLock;

pub mod cell;
pub mod error;
mod genesis;
mod ops;

pub use cell::{
    Cell, GHOST_FLAG, CMD_ANCLA, CMD_BORRAR_ANY, CMD_BORRAR_SELF, CMD_COMBINAR, CMD_CONGELAMIENTO,
    CMD_DERIVAR, CMD_DOMINANTE, CMD_EFIMERA, CMD_EJECUTOR, CMD_ESCRIBIR_ANY, CMD_ESCRIBIR_SELF,
    CMD_ESTRICTA, CMD_INMORTAL, CMD_LEER_ANY, CMD_LEER_LIBRE, CMD_LEER_SELF, CMD_LINK_L_VERIF,
    CMD_LINK_R_VERIF, CMD_MIGRADA, CMD_TERMINAL,
};
pub use error::{CellError, CellResult};

const TEMP_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("temp_table");
const DEFAULT_DATA_SIZE: usize = 96;
const DEFAULT_MAX_RECORDS: u32 = (4 * 1024 * 1024 * 1024u64 / (DEFAULT_DATA_SIZE as u64 + 1)) as u32;

pub struct CellEngine {
    pub db: Arc<RwLock<OuroborosDB>>,
    pub kv: Arc<Database>,
}

impl CellEngine {
    pub async fn new(circular_db_path: &str, kv_path: &str) -> CellResult<Self> {
        let _ = dotenv();

        let data_size = std::env::var("OUROBOROS_DATA_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_DATA_SIZE);
        let max_records = std::env::var("OUROBOROS_MAX_RECORDS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(DEFAULT_MAX_RECORDS);

        let config = OuroborosConfig {
            data_size,
            max_records,
        };

        let db = OuroborosDB::open(circular_db_path, config).map_err(|_| CellError::Internal)?;
        let kv = Database::create(kv_path).map_err(|_| CellError::Internal)?;

        let write_txn = kv.begin_write().map_err(|_| CellError::Internal)?;
        write_txn
            .open_table(TEMP_TABLE)
            .map_err(|_| CellError::Internal)?;
        write_txn.commit().map_err(|_| CellError::Internal)?;

        let engine = Self {
            db: Arc::new(RwLock::new(db)),
            kv: Arc::new(kv),
        };

        genesis::inyectar_genesis_si_vacio(&engine).await?;

        Ok(engine)
    }

    pub async fn ejecutar(
        &self,
        target_index: u32,
        solucion: &[u8; 32],
        opcode: u8,
        params: &[u8],
    ) -> CellResult<Vec<u8>> {
        let db_read = Arc::clone(&self.db);
        let solucion_copy = *solucion;
        let (celula, index_real) = tokio::task::spawn_blocking(move || {
            let guard = db_read.blocking_read();
            crate::cell::ciclidb_read_estricto(&guard, target_index, &solucion_copy)
        })
        .await
        .map_err(|_| CellError::Internal)??;

        match opcode {
            0x01 => ops::op_leer(self, &celula, index_real, params).await,
            0x02 => ops::op_escribir(self, celula, index_real, solucion, params).await,
            0x03 => ops::op_borrar(self, celula, index_real, solucion, params).await,
            0x04 => ops::op_derivar(self, celula, index_real, solucion, params).await,
            0x05 => ops::op_combinar(self, celula, index_real, solucion, params).await,
            0x06 => ops::op_inspeccionar(self, target_index, solucion, index_real).await,
            _ => Err(CellError::BadRequest),
        }
    }
}
