use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use correspondence::{Correspondence, CorrespondenceConfig, MutationResult, ReadResult, Result};

fn temp_dir() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("correspondence-example-{unique}"));
    fs::create_dir_all(&path).expect("temporary directory should be created");
    path
}

fn write_ouroboros_config(dir: &std::path::Path) -> PathBuf {
    let config_path = dir.join("ouroboros.toml");
    fs::write(
        &config_path,
        "data_path = \"ouroboros.db\"\nmax_records = 16\nsync_writes = false\n",
    )
    .expect("config file should be written");
    config_path
}

fn main() -> Result<()> {
    let dir = temp_dir();
    let config_path = write_ouroboros_config(&dir);
    let membranes_path = dir.join("membranes.redb");
    let env_name = "GENESIS_SECRET_EXAMPLE";
    let genesis_secret = "diarsaba";

    unsafe {
        std::env::set_var(env_name, genesis_secret);
    }

    let config = CorrespondenceConfig::new(&config_path, &membranes_path)
        .with_genesis_secret_env(env_name);
    let mut app = Correspondence::open(config)?;

    let genesis_index = app.genesis_index()?;
    println!("genesis index: {genesis_index}");

    let MutationResult::Ok { new_cell_index } = app.write(
        "saludo",
        b"hola desde correspondence",
        genesis_index,
        genesis_secret.as_bytes(),
    )?
    else {
        unreachable!("genesis write should succeed");
    };
    println!("write refreshed cell index: {new_cell_index}");

    let ReadResult::Ok {
        value,
        new_cell_index,
    } = app.read("saludo", new_cell_index, genesis_secret.as_bytes())?
    else {
        unreachable!("genesis read should succeed");
    };
    println!(
        "read refreshed cell index: {new_cell_index}, value: {}",
        String::from_utf8_lossy(&value)
    );

    fs::remove_dir_all(dir)?;
    Ok(())
}