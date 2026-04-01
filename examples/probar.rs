use blake2::digest::consts::U32;
use blake2::digest::{Digest, KeyInit, Mac};
use blake2::{Blake2b, Blake2bMac};
use correspondence::{
    CellEngine, CellError, CMD_ESCRIBIR_SELF, CMD_LEER_SELF,
};
use dotenvy::dotenv;
use std::time::{SystemTime, UNIX_EPOCH};

fn blake2b32(input: &[u8]) -> [u8; 32] {
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(input);
    hasher.finalize().into()
}

fn challenge_for(secret32: &[u8; 32], salt: &[u8; 32]) -> [u8; 32] {
    let mut mac = <Blake2bMac<U32> as KeyInit>::new_from_slice(secret32)
        .expect("secret de 32 bytes");
    mac.update(salt);
    mac.finalize().into_bytes().into()
}

fn params_write(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + key.len() + value.len());
    out.push(key.len() as u8);
    out.extend_from_slice(key);
    out.extend_from_slice(value);
    out
}

fn params_read(key: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + key.len());
    out.push(key.len() as u8);
    out.extend_from_slice(key);
    out
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenv();
    let genesis_secret = std::env::var("GENESIS_SECRET")?;
    let genesis_solution = blake2b32(genesis_secret.as_bytes());

    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let db_path = format!("/tmp/correspondence_cells_{ts}.db");
    let kv_path = format!("/tmp/correspondence_kv_{ts}.redb");

    let engine = CellEngine::new(&db_path, &kv_path).await?;

    // 1) Celula dios: escribe key="hola" value="hola mundo"
    let p_write_hola = params_write(b"hola", b"hola mundo");
    let write_hola_resp = engine.ejecutar(0, &genesis_solution, 0x02, &p_write_hola).await?;
    if write_hola_resp.len() != 4 {
        return Err("respuesta ESCRIBIR (dios) invalida".into());
    }

    // 2) Celula dios: lee key="hola" y debe obtener "hola mundo"
    let p_read_hola = params_read(b"hola");
    let read_hola_resp = engine.ejecutar(0, &genesis_solution, 0x01, &p_read_hola).await?;
    if read_hola_resp.as_slice() != b"hola mundo" {
        return Err("lectura dios inesperada para key hola".into());
    }

    let mut child_salt = [0u8; 32];
    child_salt[..8].copy_from_slice(&ts.to_le_bytes()[..8]);
    let child_solution = blake2b32(b"child-secret-demo");
    let child_challenge = challenge_for(&child_solution, &child_salt);

    // 3) Derivar celula hija con permisos solo SELF (leer/escribir self)
    let child_cmd = CMD_LEER_SELF | CMD_ESCRIBIR_SELF;

    let mut derivar_params = Vec::with_capacity(76);
    derivar_params.extend_from_slice(&child_salt);
    derivar_params.extend_from_slice(&child_challenge);
    derivar_params.extend_from_slice(&child_cmd.to_le_bytes());
    derivar_params.extend_from_slice(&0u32.to_le_bytes());
    derivar_params.extend_from_slice(&0u32.to_le_bytes());

    let derivar_resp = engine
        .ejecutar(0, &genesis_solution, 0x04, &derivar_params)
        .await?;
    if derivar_resp.len() != 4 {
        return Err("respuesta DERIVAR invalida".into());
    }
    let child_index = u32::from_le_bytes(derivar_resp[..4].try_into()?);

    // 4) La hija escribe key="hola self" value="hola mundo self"
    let p_write_self = params_write(b"hola self", b"hola mundo self");
    let write_self_resp = engine
        .ejecutar(child_index, &child_solution, 0x02, &p_write_self)
        .await?;
    if write_self_resp.len() != 4 {
        return Err("respuesta ESCRIBIR (self) invalida".into());
    }

    // 5) La hija lee su propia key y debe tener exito
    let p_read_self = params_read(b"hola self");
    let read_self_resp = engine
        .ejecutar(child_index, &child_solution, 0x01, &p_read_self)
        .await?;
    if read_self_resp.as_slice() != b"hola mundo self" {
        return Err("lectura self inesperada para key hola self".into());
    }

    // 6) La hija intenta leer key="hola" (de la celula dios) y debe fallar con Forbidden
    let deny_read = engine.ejecutar(child_index, &child_solution, 0x01, &p_read_hola).await;
    match deny_read {
        Err(CellError::Forbidden) => {}
        Err(other) => return Err(format!("error inesperado al negar acceso: {other}").into()),
        Ok(_) => return Err("se esperaba access denied al leer key hola".into()),
    }

    // 7) Inspeccion opcional de la hija (opcode 0x06)
    let inspeccionar_resp = engine.ejecutar(child_index, &child_solution, 0x06, &[]).await?;
    if inspeccionar_resp.len() != 16 {
        return Err("respuesta INSPECCIONAR invalida".into());
    }

    let cmd = u32::from_le_bytes(inspeccionar_resp[0..4].try_into()?);
    let index_l = u32::from_le_bytes(inspeccionar_resp[4..8].try_into()?);
    let index_r = u32::from_le_bytes(inspeccionar_resp[8..12].try_into()?);
    let index_real = u32::from_le_bytes(inspeccionar_resp[12..16].try_into()?);

    println!("DIOS ESCRIBIR/LEER OK -> hola => hola mundo");
    println!("DERIVAR OK -> child_index={child_index}");
    println!("SELF ESCRIBIR/LEER OK -> hola self => hola mundo self");
    println!("SELF READ FOREIGN OK -> access denied");
    println!(
        "INSPECCIONAR OK -> cmd=0x{cmd:08x}, L={index_l}, R={index_r}, real={index_real}"
    );

    Ok(())
}
