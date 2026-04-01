use blake2::digest::consts::U32;
use blake2::digest::{KeyInit, Mac};
use blake2::Blake2bMac;
use ouroboros_db::{OuroborosDB, RecordIndex};

use crate::error::{CellError, CellResult};

/// Tamano serializado fijo de una celula en bytes.
pub const DATA_SIZE: usize = 96;

/// MAC Blake2b de 256 bits usada para validar el candado de una celula.
pub type Blake2bMac256 = Blake2bMac<U32>;

/// Permite leer datos propios o ligados mediante links verificados.
pub const CMD_LEER_SELF: u32 = 0x00000001;
/// Permite leer cualquier clave existente.
pub const CMD_LEER_ANY: u32 = 0x00000002;
/// Permite escribir datos propios y sobrescribir solo si la clave sigue siendo propia o ligada.
pub const CMD_ESCRIBIR_SELF: u32 = 0x00000004;
/// Permite escribir cualquier clave.
pub const CMD_ESCRIBIR_ANY: u32 = 0x00000008;
/// Permite borrar datos propios o ligados mediante links verificados.
pub const CMD_BORRAR_SELF: u32 = 0x00000010;
/// Permite borrar cualquier clave.
pub const CMD_BORRAR_ANY: u32 = 0x00000020;
/// Permite derivar una celula hija con un subconjunto de permisos.
pub const CMD_DERIVAR: u32 = 0x00000040;
/// Permite combinar la celula con otra celula compatible.
pub const CMD_COMBINAR: u32 = 0x00000080;

/// Al derivar desde una celula terminal, la hija pierde derivacion y terminalidad.
pub const CMD_TERMINAL: u32 = 0x00000100;
/// Impide combinar la celula si participa en un merge.
pub const CMD_DOMINANTE: u32 = 0x00000200;
/// Marca la celula como efimera para que el ciclo de vida pueda convertirla en ghost.
pub const CMD_EFIMERA: u32 = 0x00000400;
/// Evita el comportamiento efimero durante el ciclo de vida.
pub const CMD_INMORTAL: u32 = 0x00000800;
/// Congela los datos poseidos por la celula e impide modificarlos o borrarlos.
pub const CMD_CONGELAMIENTO: u32 = 0x00001000;
/// Flag reservada para ejecucion controlada por el sistema.
pub const CMD_EJECUTOR: u32 = 0x00002000;
/// Capability de lectura libre usada por genesis y reservada para logica de sistema.
pub const CMD_LEER_LIBRE: u32 = 0x00004000;

/// Marca interna que indica que `index_l` fue verificado contra una madre valida.
pub const CMD_LINK_L_VERIF: u32 = 0x00008000;
/// Marca interna que indica que `index_r` fue verificado contra una madre valida.
pub const CMD_LINK_R_VERIF: u32 = 0x00010000;
/// Obliga a que la celula no mantenga links externos en derivacion o merge.
pub const CMD_ANCLA: u32 = 0x00020000;
/// Obliga a que los links usados en derivacion o merge esten verificados.
pub const CMD_ESTRICTA: u32 = 0x00040000;

/// Marca interna para indicar que la celula ya fue migrada a un nuevo indice real.
pub const CMD_MIGRADA: u32 = 0x00080000;
/// Marca interna para distinguir celdas ghost en el ciclo de vida.
pub const GHOST_FLAG: u32 = 0x80000000;

/// Mascara de permisos de seguridad expuesta a derivacion y merge.
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
