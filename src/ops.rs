use std::sync::Arc;

use blake2::digest::KeyInit;
use blake2::digest::Mac;
use ouroboros_db::RecordIndex;
use rand::Rng;
use redb::{ReadableTable, TableDefinition};

use crate::cell::{
    ciclidb_read_estricto, ciclidb_read_libre, Blake2bMac256, Cell, CMD_ANCLA, CMD_BORRAR_ANY,
    CMD_BORRAR_SELF, CMD_COMBINAR, CMD_CONGELAMIENTO, CMD_DERIVAR, CMD_DOMINANTE, CMD_EFIMERA,
    CMD_ESCRIBIR_ANY, CMD_ESCRIBIR_SELF, CMD_ESTRICTA, CMD_INMORTAL, CMD_LEER_ANY, CMD_LEER_SELF,
    CMD_LINK_L_VERIF, CMD_LINK_R_VERIF, CMD_MIGRADA, GHOST_FLAG, MASK_SEGURIDAD,
};
use crate::error::{CellError, CellResult};
use crate::CellEngine;

const TEMP_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("temp_table");

fn u32_le(bytes: &[u8]) -> CellResult<u32> {
    let arr: [u8; 4] = bytes.try_into().map_err(|_| CellError::BadRequest)?;
    Ok(u32::from_le_bytes(arr))
}

fn arr32(bytes: &[u8]) -> CellResult<[u8; 32]> {
    bytes.try_into().map_err(|_| CellError::BadRequest)
}

fn parse_key_payload(params: &[u8]) -> CellResult<(&str, &[u8])> {
    if params.is_empty() {
        return Err(CellError::BadRequest);
    }
    let key_len = params[0] as usize;
    if params.len() < 1 + key_len {
        return Err(CellError::BadRequest);
    }
    let key = std::str::from_utf8(&params[1..1 + key_len]).map_err(|_| CellError::BadRequest)?;
    Ok((key, &params[1 + key_len..]))
}

pub async fn procesar_ciclo_de_vida(
    engine: &CellEngine,
    celula: Cell,
    index_real: u32,
    solucion_cliente: &[u8; 32],
) -> CellResult<u32> {
    if (celula.cmd & CMD_EFIMERA) != 0 && (celula.cmd & CMD_INMORTAL) == 0 {
        let mut ghost = celula.clone();
        ghost.cmd |= GHOST_FLAG;

        let payload = ghost.to_bytes();
        let db = Arc::clone(&engine.db);
        tokio::task::spawn_blocking(move || {
            let mut guard = db.blocking_write();
            guard
                .update(RecordIndex(index_real), &payload)
                .map_err(|_| CellError::Internal)?;
            Ok::<(), CellError>(())
        })
        .await
        .map_err(|_| CellError::Internal)??;

        Ok(index_real)
    } else {
        let (nuevo_salt, nuevo_challenge) = {
            let mut rng = rand::thread_rng();
            let mut s = [0u8; 32];
            rng.fill(&mut s);

            let mut mac = <Blake2bMac256 as KeyInit>::new_from_slice(solucion_cliente)
                .map_err(|_| CellError::Internal)?;
            mac.update(&s);
            let chall: [u8; 32] = mac.finalize().into_bytes().into();
            (s, chall)
        };

        let celula_fresca = Cell {
            salt: nuevo_salt,
            challenge: nuevo_challenge,
            ..celula.clone()
        };

        let payload_fresca = celula_fresca.to_bytes();
        let db_append = Arc::clone(&engine.db);
        let nuevo_index = tokio::task::spawn_blocking(move || {
            let mut guard = db_append.blocking_write();
            let idx = guard
                .append(&payload_fresca)
                .map_err(|_| CellError::Internal)?;
            Ok::<u32, CellError>(idx.0)
        })
        .await
        .map_err(|_| CellError::Internal)??;

        let mut celula_apuntador = celula;
        celula_apuntador.index_l = nuevo_index;
        celula_apuntador.index_r = 0;
        celula_apuntador.cmd |= CMD_MIGRADA;

        let payload_apuntador = celula_apuntador.to_bytes();
        let db_overwrite = Arc::clone(&engine.db);
        tokio::task::spawn_blocking(move || {
            let mut guard = db_overwrite.blocking_write();
            guard
                .update(RecordIndex(index_real), &payload_apuntador)
                .map_err(|_| CellError::Internal)?;
            Ok::<(), CellError>(())
        })
        .await
        .map_err(|_| CellError::Internal)??;

        Ok(nuevo_index)
    }
}

pub async fn op_leer(
    engine: &CellEngine,
    celula: &Cell,
    index_real: u32,
    params: &[u8],
) -> CellResult<Vec<u8>> {
    let (key_str, _) = parse_key_payload(params)?;

    let read_txn = engine.kv.begin_read().map_err(|_| CellError::Internal)?;
    let table = read_txn
        .open_table(TEMP_TABLE)
        .map_err(|_| CellError::Internal)?;

    if let Some(entry) = table.get(key_str).map_err(|_| CellError::Internal)? {
        let value = entry.value();
        if value.len() < 4 {
            return Err(CellError::Internal);
        }

        let owner_index_original = u32_le(&value[0..4])?;
        let data_real = &value[4..];

        let db_for_owner = Arc::clone(&engine.db);
        let owner_current_index = tokio::task::spawn_blocking(move || {
            let guard = db_for_owner.blocking_read();
            Ok::<u32, CellError>(
                if let Ok((_, idx)) = ciclidb_read_libre(&guard, owner_index_original) {
                    idx
                } else {
                    owner_index_original
                },
            )
        })
        .await
        .map_err(|_| CellError::Internal)??;

        if (celula.cmd & CMD_LEER_ANY) != 0 {
            return Ok(data_real.to_vec());
        }
        if (celula.cmd & CMD_LEER_SELF) != 0 {
            let valid_self = owner_current_index == index_real;
            let valid_l = owner_current_index == celula.index_l && (celula.cmd & CMD_LINK_L_VERIF) != 0;
            let valid_r = owner_current_index == celula.index_r && (celula.cmd & CMD_LINK_R_VERIF) != 0;
            if valid_self || valid_l || valid_r {
                return Ok(data_real.to_vec());
            }
        }
        return Err(CellError::Forbidden);
    }

    Err(CellError::NotFound)
}

pub async fn op_escribir(
    engine: &CellEngine,
    celula: Cell,
    index_real: u32,
    solucion: &[u8; 32],
    params: &[u8],
) -> CellResult<Vec<u8>> {
    let (key_str, payload) = parse_key_payload(params)?;

    let mut is_overwrite = false;
    let mut is_owner = false;

    {
        let read_txn = engine.kv.begin_read().map_err(|_| CellError::Internal)?;
        let table = read_txn
            .open_table(TEMP_TABLE)
            .map_err(|_| CellError::Internal)?;

        if let Some(entry) = table.get(key_str).map_err(|_| CellError::Internal)? {
            let existing_val = entry.value();
            if existing_val.len() >= 4 {
                is_overwrite = true;
                let owner_index_original = u32_le(&existing_val[0..4])?;

                let db_for_owner = Arc::clone(&engine.db);
                let ownership = tokio::task::spawn_blocking(move || {
                    let guard = db_for_owner.blocking_read();
                    if let Ok((owner_cell, idx)) = ciclidb_read_libre(&guard, owner_index_original) {
                        if (owner_cell.cmd & CMD_CONGELAMIENTO) != 0 {
                            return Err(CellError::Forbidden);
                        }
                        Ok::<u32, CellError>(idx)
                    } else {
                        Ok::<u32, CellError>(owner_index_original)
                    }
                })
                .await
                .map_err(|_| CellError::Internal)??;

                is_owner = ownership == index_real
                    || (ownership == celula.index_l && (celula.cmd & CMD_LINK_L_VERIF) != 0)
                    || (ownership == celula.index_r && (celula.cmd & CMD_LINK_R_VERIF) != 0);
            }
        }
    }

    let can_write_any = (celula.cmd & CMD_ESCRIBIR_ANY) != 0;
    let can_write_self = (celula.cmd & CMD_ESCRIBIR_SELF) != 0 && (!is_overwrite || is_owner);

    if can_write_any || can_write_self {
        let write_txn = engine.kv.begin_write().map_err(|_| CellError::Internal)?;
        {
            let mut table = write_txn
                .open_table(TEMP_TABLE)
                .map_err(|_| CellError::Internal)?;
            let mut final_value = Vec::with_capacity(4 + payload.len());
            final_value.extend_from_slice(&index_real.to_le_bytes());
            final_value.extend_from_slice(payload);
            table
                .insert(key_str, final_value.as_slice())
                .map_err(|_| CellError::Internal)?;
        }
        write_txn.commit().map_err(|_| CellError::Internal)?;

        let nuevo_idx = procesar_ciclo_de_vida(engine, celula, index_real, solucion).await?;
        return Ok(nuevo_idx.to_le_bytes().to_vec());
    }

    Err(CellError::Forbidden)
}

pub async fn op_borrar(
    engine: &CellEngine,
    celula: Cell,
    index_real: u32,
    solucion: &[u8; 32],
    params: &[u8],
) -> CellResult<Vec<u8>> {
    let (key_str, _) = parse_key_payload(params)?;

    let write_txn = engine.kv.begin_write().map_err(|_| CellError::Internal)?;
    let mut table = write_txn
        .open_table(TEMP_TABLE)
        .map_err(|_| CellError::Internal)?;

    let owner_index_opt = if let Some(entry) = table.get(key_str).map_err(|_| CellError::Internal)? {
        let val = entry.value();
        if val.len() >= 4 {
            Some(u32_le(&val[0..4])?)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(owner_index_original) = owner_index_opt {
        let db_for_owner = Arc::clone(&engine.db);
        let owner_current_index = tokio::task::spawn_blocking(move || {
            let guard = db_for_owner.blocking_read();
            if let Ok((owner_cell, idx)) = ciclidb_read_libre(&guard, owner_index_original) {
                if (owner_cell.cmd & CMD_CONGELAMIENTO) != 0 {
                    return Err(CellError::Forbidden);
                }
                Ok::<u32, CellError>(idx)
            } else {
                Ok::<u32, CellError>(owner_index_original)
            }
        })
        .await
        .map_err(|_| CellError::Internal)??;

        let can_delete_any = (celula.cmd & CMD_BORRAR_ANY) != 0;
        let can_delete_self = (celula.cmd & CMD_BORRAR_SELF) != 0
            && (owner_current_index == index_real
                || (owner_current_index == celula.index_l && (celula.cmd & CMD_LINK_L_VERIF) != 0)
                || (owner_current_index == celula.index_r && (celula.cmd & CMD_LINK_R_VERIF) != 0));

        if can_delete_any || can_delete_self {
            table.remove(key_str).map_err(|_| CellError::Internal)?;
            drop(table);
            write_txn.commit().map_err(|_| CellError::Internal)?;

            let nuevo_idx = procesar_ciclo_de_vida(engine, celula, index_real, solucion).await?;
            return Ok(nuevo_idx.to_le_bytes().to_vec());
        }

        return Err(CellError::Forbidden);
    }

    Err(CellError::NotFound)
}

pub async fn op_derivar(
    engine: &CellEngine,
    celula: Cell,
    index_real: u32,
    solucion: &[u8; 32],
    params: &[u8],
) -> CellResult<Vec<u8>> {
    if params.len() < 76 {
        return Err(CellError::BadRequest);
    }

    let nuevo_salt = arr32(&params[0..32])?;
    let nuevo_challenge = arr32(&params[32..64])?;
    let nuevos_cmd = u32_le(&params[64..68])?;
    let user_index_l = u32_le(&params[68..72])?;
    let user_index_r = u32_le(&params[72..76])?;

    if (celula.cmd & CMD_DERIVAR) == 0 {
        return Err(CellError::Forbidden);
    }

    let l_is_verified = user_index_l == index_real;
    let r_is_verified = user_index_r == index_real;

    if (celula.cmd & CMD_ANCLA) != 0 && (user_index_l != 0 || user_index_r != 0) {
        return Err(CellError::Forbidden);
    }
    if (celula.cmd & CMD_ESTRICTA) != 0
        && ((user_index_l != 0 && !l_is_verified) || (user_index_r != 0 && !r_is_verified))
    {
        return Err(CellError::Forbidden);
    }

    let mut nuevos_cmd_limpios = nuevos_cmd & MASK_SEGURIDAD;
    let cmd_madre_limpios = celula.cmd & MASK_SEGURIDAD;

    if (nuevos_cmd_limpios & cmd_madre_limpios) == nuevos_cmd_limpios {
        if (celula.cmd & crate::cell::CMD_TERMINAL) != 0 {
            nuevos_cmd_limpios &= !(CMD_DERIVAR | crate::cell::CMD_TERMINAL);
        }
        if l_is_verified {
            nuevos_cmd_limpios |= CMD_LINK_L_VERIF;
        }
        if r_is_verified {
            nuevos_cmd_limpios |= CMD_LINK_R_VERIF;
        }

        let cmd_final = nuevos_cmd_limpios | (celula.cmd & GHOST_FLAG);

        let nueva_celula = Cell {
            salt: nuevo_salt,
            challenge: nuevo_challenge,
            index_l: user_index_l,
            index_r: user_index_r,
            cmd: cmd_final,
            padding: [0u8; 20],
        };

        let payload = nueva_celula.to_bytes();
        let db_append = Arc::clone(&engine.db);
        let nuevo_index = tokio::task::spawn_blocking(move || {
            let mut guard = db_append.blocking_write();
            let idx = guard.append(&payload).map_err(|_| CellError::Internal)?;
            Ok::<u32, CellError>(idx.0)
        })
        .await
        .map_err(|_| CellError::Internal)??;

        let _ = procesar_ciclo_de_vida(engine, celula, index_real, solucion).await?;
        return Ok(nuevo_index.to_le_bytes().to_vec());
    }

    Err(CellError::Forbidden)
}

pub async fn op_combinar(
    engine: &CellEngine,
    celula: Cell,
    index_real: u32,
    _solucion: &[u8; 32],
    params: &[u8],
) -> CellResult<Vec<u8>> {
    if params.len() < 108 {
        return Err(CellError::BadRequest);
    }

    let index2 = u32_le(&params[0..4])?;
    let solucion2 = arr32(&params[4..36])?;
    let nuevo_salt = arr32(&params[36..68])?;
    let nuevo_challenge = arr32(&params[68..100])?;
    let user_index_l = u32_le(&params[100..104])?;
    let user_index_r = u32_le(&params[104..108])?;

    if (celula.cmd & CMD_COMBINAR) == 0 {
        return Err(CellError::Forbidden);
    }

    let db_read_madre2 = Arc::clone(&engine.db);
    let (celula2, index_real2) = tokio::task::spawn_blocking(move || {
        let guard = db_read_madre2.blocking_read();
        ciclidb_read_estricto(&guard, index2, &solucion2)
    })
    .await
    .map_err(|_| CellError::Internal)??;

    if (celula2.cmd & CMD_COMBINAR) == 0 {
        return Err(CellError::Forbidden);
    }
    if (celula.cmd & CMD_DOMINANTE) != 0 || (celula2.cmd & CMD_DOMINANTE) != 0 {
        return Err(CellError::Forbidden);
    }

    let l_is_verified = user_index_l == index_real || user_index_l == index_real2;
    let r_is_verified = user_index_r == index_real || user_index_r == index_real2;

    if ((celula.cmd & CMD_ANCLA) != 0 || (celula2.cmd & CMD_ANCLA) != 0)
        && (user_index_l != 0 || user_index_r != 0)
    {
        return Err(CellError::Forbidden);
    }
    if ((celula.cmd & CMD_ESTRICTA) != 0 || (celula2.cmd & CMD_ESTRICTA) != 0)
        && ((user_index_l != 0 && !l_is_verified) || (user_index_r != 0 && !r_is_verified))
    {
        return Err(CellError::Forbidden);
    }

    let mut poderes_combinados = (celula.cmd | celula2.cmd) & MASK_SEGURIDAD;
    if l_is_verified {
        poderes_combinados |= CMD_LINK_L_VERIF;
    }
    if r_is_verified {
        poderes_combinados |= CMD_LINK_R_VERIF;
    }

    let nueva_celula = Cell {
        salt: nuevo_salt,
        challenge: nuevo_challenge,
        index_l: user_index_l,
        index_r: user_index_r,
        cmd: poderes_combinados,
        padding: [0u8; 20],
    };

    let payload_nueva = nueva_celula.to_bytes();
    let db_append = Arc::clone(&engine.db);
    let nuevo_index = tokio::task::spawn_blocking(move || {
        let mut guard = db_append.blocking_write();
        let idx = guard.append(&payload_nueva).map_err(|_| CellError::Internal)?;
        Ok::<u32, CellError>(idx.0)
    })
    .await
    .map_err(|_| CellError::Internal)??;

    let mut madre1 = celula.clone();
    madre1.index_l = nuevo_index;
    madre1.index_r = 0;
    madre1.cmd |= CMD_MIGRADA;

    let mut madre2 = celula2.clone();
    madre2.index_l = nuevo_index;
    madre2.index_r = 0;
    madre2.cmd |= CMD_MIGRADA;

    let payload_m1 = madre1.to_bytes();
    let payload_m2 = madre2.to_bytes();
    let db_overwrite = Arc::clone(&engine.db);
    tokio::task::spawn_blocking(move || {
        let mut guard = db_overwrite.blocking_write();
        guard
            .update(RecordIndex(index_real), &payload_m1)
            .map_err(|_| CellError::Internal)?;
        guard
            .update(RecordIndex(index_real2), &payload_m2)
            .map_err(|_| CellError::Internal)?;
        Ok::<(), CellError>(())
    })
    .await
    .map_err(|_| CellError::Internal)??;

    Ok(nuevo_index.to_le_bytes().to_vec())
}

pub async fn op_inspeccionar(
    engine: &CellEngine,
    target_index: u32,
    solucion: &[u8; 32],
    index_real: u32,
) -> CellResult<Vec<u8>> {
    let db_read = Arc::clone(&engine.db);
    let celda_raw = tokio::task::spawn_blocking(move || {
        let guard = db_read.blocking_read();
        let raw = guard
            .read(RecordIndex(target_index))
            .map_err(|_| CellError::NotFound)?;
        let bytes: [u8; crate::cell::DATA_SIZE] = raw.try_into().map_err(|_| CellError::Internal)?;
        Ok::<Cell, CellError>(Cell::from_bytes(&bytes))
    })
    .await
    .map_err(|_| CellError::Internal)??;

    if !crate::cell::validar_candado(&celda_raw.salt, &celda_raw.challenge, solucion) {
        return Err(CellError::Unauthorized);
    }

    let mut resp = Vec::with_capacity(16);
    resp.extend_from_slice(&celda_raw.cmd.to_le_bytes());
    resp.extend_from_slice(&celda_raw.index_l.to_le_bytes());
    resp.extend_from_slice(&celda_raw.index_r.to_le_bytes());
    resp.extend_from_slice(&index_real.to_le_bytes());
    Ok(resp)
}
