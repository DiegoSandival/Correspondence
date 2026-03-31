use blake2::digest::consts::U32;
use blake2::digest::{KeyInit, Mac};
use blake2::Blake2bMac;
use ouroboros_db::{OuroborosDB, RecordIndex};

use crate::error::{CellError, CellResult};

pub const DATA_SIZE: usize = 96;

pub type Blake2bMac256 = Blake2bMac<U32>;

pub const CMD_LEER_SELF: u32 = 0x00000001;
pub const CMD_LEER_ANY: u32 = 0x00000002;
pub const CMD_ESCRIBIR_SELF: u32 = 0x00000004;
pub const CMD_ESCRIBIR_ANY: u32 = 0x00000008;
pub const CMD_BORRAR_SELF: u32 = 0x00000010;
pub const CMD_BORRAR_ANY: u32 = 0x00000020;
pub const CMD_DERIVAR: u32 = 0x00000040;
pub const CMD_COMBINAR: u32 = 0x00000080;

pub const CMD_TERMINAL: u32 = 0x00000100;
pub const CMD_DOMINANTE: u32 = 0x00000200;
pub const CMD_EFIMERA: u32 = 0x00000400;
pub const CMD_INMORTAL: u32 = 0x00000800;
pub const CMD_CONGELAMIENTO: u32 = 0x00001000;
pub const CMD_EJECUTOR: u32 = 0x00002000;
pub const CMD_LEER_LIBRE: u32 = 0x00004000;

pub const CMD_LINK_L_VERIF: u32 = 0x00008000;
pub const CMD_LINK_R_VERIF: u32 = 0x00010000;
pub const CMD_ANCLA: u32 = 0x00020000;
pub const CMD_ESTRICTA: u32 = 0x00040000;

pub const CMD_MIGRADA: u32 = 0x00080000;
pub const GHOST_FLAG: u32 = 0x80000000;

pub const MASK_SEGURIDAD: u32 = !(GHOST_FLAG | CMD_LINK_L_VERIF | CMD_LINK_R_VERIF | CMD_MIGRADA);

#[derive(Debug, Clone)]
pub struct Cell {
    pub salt: [u8; 32],
    pub challenge: [u8; 32],
    pub index_l: u32,
    pub index_r: u32,
    pub cmd: u32,
    pub padding: [u8; 20],
}

impl Cell {
    pub fn from_bytes(bytes: &[u8; DATA_SIZE]) -> Self {
        let mut salt = [0u8; 32];
        salt.copy_from_slice(&bytes[0..32]);

        let mut challenge = [0u8; 32];
        challenge.copy_from_slice(&bytes[32..64]);

        let index_l = u32::from_le_bytes(bytes[64..68].try_into().unwrap());
        let index_r = u32::from_le_bytes(bytes[68..72].try_into().unwrap());
        let cmd = u32::from_le_bytes(bytes[72..76].try_into().unwrap());

        let mut padding = [0u8; 20];
        padding.copy_from_slice(&bytes[76..96]);

        Self {
            salt,
            challenge,
            index_l,
            index_r,
            cmd,
            padding,
        }
    }

    pub fn to_bytes(&self) -> [u8; DATA_SIZE] {
        let mut bytes = [0u8; DATA_SIZE];
        bytes[0..32].copy_from_slice(&self.salt);
        bytes[32..64].copy_from_slice(&self.challenge);
        bytes[64..68].copy_from_slice(&self.index_l.to_le_bytes());
        bytes[68..72].copy_from_slice(&self.index_r.to_le_bytes());
        bytes[72..76].copy_from_slice(&self.cmd.to_le_bytes());
        bytes[76..96].copy_from_slice(&self.padding);
        bytes
    }
}

pub fn validar_candado(salt: &[u8; 32], challenge: &[u8; 32], solucion: &[u8; 32]) -> bool {
    if let Ok(mut mac) = <Blake2bMac256 as KeyInit>::new_from_slice(solucion) {
        mac.update(salt);
        return mac.verify_slice(challenge).is_ok();
    }
    false
}

pub fn ciclidb_read_estricto(
    db: &OuroborosDB,
    index_inicial: u32,
    solucion_cliente: &[u8; 32],
) -> CellResult<(Cell, u32)> {
    let mut index_actual = index_inicial;

    loop {
        let raw = db.read(RecordIndex(index_actual)).map_err(|_| CellError::Internal)?;
        if raw.len() != DATA_SIZE {
            return Err(CellError::Internal);
        }

        let raw_bytes: [u8; DATA_SIZE] = raw.try_into().map_err(|_| CellError::Internal)?;
        let cell = Cell::from_bytes(&raw_bytes);

        if !validar_candado(&cell.salt, &cell.challenge, solucion_cliente) {
            return Err(CellError::Unauthorized);
        }

        if (cell.cmd & CMD_MIGRADA) != 0 {
            index_actual = cell.index_l;
        } else {
            return Ok((cell, index_actual));
        }
    }
}

pub fn ciclidb_read_libre(db: &OuroborosDB, index_inicial: u32) -> CellResult<(Cell, u32)> {
    let mut index_actual = index_inicial;

    loop {
        let raw = db.read(RecordIndex(index_actual)).map_err(|_| CellError::Internal)?;
        if raw.len() != DATA_SIZE {
            return Err(CellError::Internal);
        }

        let raw_bytes: [u8; DATA_SIZE] = raw.try_into().map_err(|_| CellError::Internal)?;
        let cell = Cell::from_bytes(&raw_bytes);

        if (cell.cmd & CMD_MIGRADA) != 0 {
            index_actual = cell.index_l;
        } else {
            return Ok((cell, index_actual));
        }
    }
}
