use blake2::digest::consts::U32;
use blake2::digest::{Digest, KeyInit, Mac};
use blake2::{Blake2b, Blake2bMac};
use correspondence::{
    CellEngine, CellError, CMD_BORRAR_ANY, CMD_COMBINAR, CMD_ESCRIBIR_ANY, CMD_ESCRIBIR_SELF,
    CMD_LEER_ANY,
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

fn seed32(tag: &str, ts: u128) -> [u8; 32] {
    blake2b32(format!("{tag}:{ts}").as_bytes())
}

fn params_write(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + key.len() + value.len());
    out.push(key.len() as u8);
    out.extend_from_slice(key);
    out.extend_from_slice(value);
    out
}

fn params_key(key: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + key.len());
    out.push(key.len() as u8);
    out.extend_from_slice(key);
    out
}

async fn derive_cell(
    engine: &CellEngine,
    parent_index: u32,
    parent_solution: &[u8; 32],
    secret_name: &str,
    cmd: u32,
    ts: u128,
) -> Result<(u32, [u8; 32]), Box<dyn std::error::Error>> {
    let solution = blake2b32(secret_name.as_bytes());
    let salt = seed32(&format!("derive:{secret_name}"), ts);
    let challenge = challenge_for(&solution, &salt);

    let mut params = Vec::with_capacity(76);
    params.extend_from_slice(&salt);
    params.extend_from_slice(&challenge);
    params.extend_from_slice(&cmd.to_le_bytes());
    params.extend_from_slice(&0u32.to_le_bytes());
    params.extend_from_slice(&0u32.to_le_bytes());

    let resp = engine
        .ejecutar(parent_index, parent_solution, 0x04, &params)
        .await?;
    if resp.len() != 4 {
        return Err(format!("respuesta DERIVAR invalida para {secret_name}").into());
    }

    Ok((u32::from_le_bytes(resp[..4].try_into()?), solution))
}

async fn combine_cells(
    engine: &CellEngine,
    index1: u32,
    solution1: &[u8; 32],
    index2: u32,
    solution2: &[u8; 32],
    secret_name: &str,
    ts: u128,
) -> Result<(u32, [u8; 32]), Box<dyn std::error::Error>> {
    let solution = blake2b32(secret_name.as_bytes());
    let salt = seed32(&format!("merge:{secret_name}"), ts);
    let challenge = challenge_for(&solution, &salt);

    let mut params = Vec::with_capacity(108);
    params.extend_from_slice(&index2.to_le_bytes());
    params.extend_from_slice(solution2);
    params.extend_from_slice(&salt);
    params.extend_from_slice(&challenge);
    params.extend_from_slice(&0u32.to_le_bytes());
    params.extend_from_slice(&0u32.to_le_bytes());

    let resp = engine.ejecutar(index1, solution1, 0x05, &params).await?;
    if resp.len() != 4 {
        return Err(format!("respuesta COMBINAR invalida para {secret_name}").into());
    }

    Ok((u32::from_le_bytes(resp[..4].try_into()?), solution))
}

async fn inspect_cell(
    engine: &CellEngine,
    index: u32,
    solution: &[u8; 32],
) -> Result<(u32, u32, u32, u32), Box<dyn std::error::Error>> {
    let resp = engine.ejecutar(index, solution, 0x06, &[]).await?;
    if resp.len() != 16 {
        return Err("respuesta INSPECCIONAR invalida".into());
    }

    Ok((
        u32::from_le_bytes(resp[0..4].try_into()?),
        u32::from_le_bytes(resp[4..8].try_into()?),
        u32::from_le_bytes(resp[8..12].try_into()?),
        u32::from_le_bytes(resp[12..16].try_into()?),
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenv();
    let genesis_secret = std::env::var("GENESIS_SECRET")?;
    let genesis_solution = blake2b32(genesis_secret.as_bytes());

    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let db_path = format!("/tmp/correspondence_mergin_cells_{ts}.db");
    let kv_path = format!("/tmp/correspondence_mergin_kv_{ts}.redb");

    println!("[setup] db={db_path} kv={kv_path}");
    let engine = CellEngine::new(&db_path, &kv_path).await?;

    println!(
        "[setup] derivando soy1, soy2 y soy3 con su permiso funcional mas CMD_COMBINAR para habilitar merge"
    );
    let (soy1_index, soy1_solution) = derive_cell(
        &engine,
        0,
        &genesis_solution,
        "soy1",
        CMD_ESCRIBIR_ANY | CMD_COMBINAR,
        ts,
    )
    .await?;
    let (soy2_index, soy2_solution) = derive_cell(
        &engine,
        0,
        &genesis_solution,
        "soy2",
        CMD_LEER_ANY | CMD_COMBINAR,
        ts,
    )
    .await?;
    let (soy3_index, soy3_solution) = derive_cell(
        &engine,
        0,
        &genesis_solution,
        "soy3",
        CMD_BORRAR_ANY | CMD_COMBINAR,
        ts,
    )
    .await?;

    println!("[setup] derivando soy4 solo con CMD_ESCRIBIR_SELF");
    let (soy4_index, soy4_solution) = derive_cell(
        &engine,
        0,
        &genesis_solution,
        "soy4",
        CMD_ESCRIBIR_SELF,
        ts,
    )
    .await?;

    println!(
        "[derive] soy1 index={soy1_index}, soy2 index={soy2_index}, soy3 index={soy3_index}, soy4 index={soy4_index}"
    );

    let key = b"yo soy";
    let value = b"la cuarta";

    println!("[soy4] escribe key='yo soy' value='la cuarta'");
    let write_resp = engine
        .ejecutar(soy4_index, &soy4_solution, 0x02, &params_write(key, value))
        .await?;
    if write_resp.len() != 4 {
        return Err("respuesta ESCRIBIR invalida para soy4".into());
    }
    let soy4_real = u32::from_le_bytes(write_resp[..4].try_into()?);
    println!("[soy4] escritura ok, index real actual={soy4_real}");

    println!("[soy4] intenta leer key='yo soy'");
    match engine.ejecutar(soy4_index, &soy4_solution, 0x01, &params_key(key)).await {
        Err(CellError::Forbidden) => {
            println!("[soy4] lectura denegada como se esperaba: forbidden");
        }
        Err(other) => {
            return Err(format!("soy4 devolvio error inesperado al leer: {other}").into());
        }
        Ok(bytes) => {
            return Err(format!(
                "soy4 no debio leer pero obtuvo: {}",
                String::from_utf8_lossy(&bytes)
            )
            .into());
        }
    }

    println!("[merge] combinando soy1 + soy2 => soy5");
    let (soy5_index, soy5_solution) = combine_cells(
        &engine,
        soy1_index,
        &soy1_solution,
        soy2_index,
        &soy2_solution,
        "soy5",
        ts,
    )
    .await?;
    let (soy5_cmd, soy5_l, soy5_r, soy5_real) = inspect_cell(&engine, soy5_index, &soy5_solution).await?;
    println!(
        "[merge] soy5 index={soy5_index} real={soy5_real} cmd=0x{soy5_cmd:08x} L={soy5_l} R={soy5_r}"
    );

    println!("[soy5] lee key='yo soy'");
    let soy5_read = engine
        .ejecutar(soy5_index, &soy5_solution, 0x01, &params_key(key))
        .await?;
    println!(
        "[soy5] lectura ok => {}",
        String::from_utf8_lossy(&soy5_read)
    );
    if soy5_read.as_slice() != value {
        return Err("soy5 leyo un valor distinto al esperado".into());
    }

    println!("[merge] combinando soy5 + soy3 => soy6");
    let (soy6_index, soy6_solution) = combine_cells(
        &engine,
        soy5_index,
        &soy5_solution,
        soy3_index,
        &soy3_solution,
        "soy6",
        ts,
    )
    .await?;
    let (soy6_cmd, soy6_l, soy6_r, soy6_real) = inspect_cell(&engine, soy6_index, &soy6_solution).await?;
    println!(
        "[merge] soy6 index={soy6_index} real={soy6_real} cmd=0x{soy6_cmd:08x} L={soy6_l} R={soy6_r}"
    );

    println!("[soy6] borra key='yo soy'");
    let delete_resp = engine
        .ejecutar(soy6_index, &soy6_solution, 0x03, &params_key(key))
        .await?;
    if delete_resp.len() != 4 {
        return Err("respuesta BORRAR invalida para soy6".into());
    }
    let soy6_after_delete = u32::from_le_bytes(delete_resp[..4].try_into()?);
    println!("[soy6] borrado ok, index real actual={soy6_after_delete}");

    println!("[dios] intenta leer key='yo soy' tras el borrado");
    match engine.ejecutar(0, &genesis_solution, 0x01, &params_key(key)).await {
        Err(CellError::NotFound) => {
            println!("[dios] lectura falla como se esperaba: not found");
        }
        Err(other) => {
            return Err(format!("dios devolvio error inesperado al leer: {other}").into());
        }
        Ok(bytes) => {
            return Err(format!(
                "dios no debio encontrar la key pero obtuvo: {}",
                String::from_utf8_lossy(&bytes)
            )
            .into());
        }
    }

    println!("[ok] flujo mergin completado correctamente");
    Ok(())
}