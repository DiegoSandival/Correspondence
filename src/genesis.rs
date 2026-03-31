use std::sync::Arc;

use blake2::digest::consts::U32;
use blake2::digest::{Digest, KeyInit, Mac};
use ouroboros_db::RecordIndex;

use crate::cell::{
    Blake2bMac256, Cell, CMD_BORRAR_ANY, CMD_BORRAR_SELF, CMD_COMBINAR, CMD_CONGELAMIENTO,
    CMD_DERIVAR, CMD_DOMINANTE, CMD_EJECUTOR, CMD_ESCRIBIR_ANY, CMD_ESCRIBIR_SELF, CMD_INMORTAL,
    CMD_LEER_ANY, CMD_LEER_LIBRE, CMD_LEER_SELF,
};
use crate::error::{CellError, CellResult};
use crate::CellEngine;

pub async fn inyectar_genesis_si_vacio(engine: &CellEngine) -> CellResult<()> {
    let db_check = Arc::clone(&engine.db);
    let ya_existe = tokio::task::spawn_blocking(move || {
        let guard = db_check.blocking_read();
        let raw = guard.read(RecordIndex(0)).map_err(|_| CellError::Internal)?;
        let bytes: [u8; crate::cell::DATA_SIZE] = raw.try_into().map_err(|_| CellError::Internal)?;
        let c = Cell::from_bytes(&bytes);
        Ok::<bool, CellError>(c.cmd != 0)
    })
    .await
    .map_err(|_| CellError::Internal)??;

    if ya_existe {
        return Ok(());
    }

    let secreto = std::env::var("GENESIS_SECRET").map_err(|_| CellError::Internal)?;

    let mut hasher = blake2::Blake2b::<U32>::new();
    hasher.update(secreto.as_bytes());
    let solucion_genesis: [u8; 32] = hasher.finalize().into();

    let salt_genesis = [1u8; 32];
    let mut mac = <Blake2bMac256 as KeyInit>::new_from_slice(&solucion_genesis)
        .map_err(|_| CellError::Internal)?;
    mac.update(&salt_genesis);
    let challenge_genesis: [u8; 32] = mac.finalize().into_bytes().into();

    let poderes_dios = CMD_LEER_SELF
        | CMD_LEER_ANY
        | CMD_ESCRIBIR_SELF
        | CMD_ESCRIBIR_ANY
        | CMD_BORRAR_SELF
        | CMD_BORRAR_ANY
        | CMD_DERIVAR
        | CMD_COMBINAR
        | CMD_DOMINANTE
        | CMD_INMORTAL
        | CMD_CONGELAMIENTO
        | CMD_EJECUTOR
        | CMD_LEER_LIBRE;

    let celula_genesis = Cell {
        salt: salt_genesis,
        challenge: challenge_genesis,
        index_l: 0,
        index_r: 0,
        cmd: poderes_dios,
        padding: [0u8; 20],
    };

    let payload = celula_genesis.to_bytes();
    let db_write = Arc::clone(&engine.db);
    tokio::task::spawn_blocking(move || {
        let mut guard = db_write.blocking_write();
        guard.append(&payload).map_err(|_| CellError::Internal)?;
        Ok::<(), CellError>(())
    })
    .await
    .map_err(|_| CellError::Internal)??;

    Ok(())
}
