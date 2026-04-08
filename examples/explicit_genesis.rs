use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use correspondence::{
    CellReadResult, ChildSpec, Correspondence, CorrespondenceConfig, DeferResult, MutationResult,
    ReadResult, Result,
};
use ouroboros::Genoma;

fn temp_dir() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("correspondence-explicit-genesis-{unique}"));
    fs::create_dir_all(&path).expect("temporary directory should be created");
    path
}

fn write_ouroboros_config(dir: &std::path::Path) -> PathBuf {
    let config_path = dir.join("ouroboros.toml");
    fs::write(
        &config_path,
        "data_path = \"ouroboros.db\"\nmax_records = 32\nsync_writes = false\n",
    )
    .expect("config file should be written");
    config_path
}

fn main() -> Result<()> {
    let dir = temp_dir();
    let config_path = write_ouroboros_config(&dir);
    let membranes_path = dir.join("membranes.redb");
    let genesis_secret = b"diarsaba";

    let config = CorrespondenceConfig::new(&config_path, &membranes_path);
    let mut app = Correspondence::open_uninitialized(config)?;

    let genesis_index = app.initialize_genesis(
        genesis_secret,
        Correspondence::default_genesis_genoma(),
    )?;
    println!("genesis index: {genesis_index}");

    let MutationResult::Ok {
        new_cell_index: genesis_after_hola,
    } = app.write("hola", b"mundo", genesis_index, genesis_secret)?
    else {
        unreachable!("genesis write should succeed");
    };
    println!("genesis wrote hola -> mundo, refreshed to {genesis_after_hola}");

    let child_spec = ChildSpec {
        salt: [2u8; 16],
        genoma: Genoma::LEER_SELF | Genoma::ESCRIBIR_SELF,
        x: 1,
        y: 0,
        z: 0,
    };

    let child_secret = b"cel1-secret";
    let DeferResult::Ok {
        deferred_index: child_index,
        new_cell_index: refreshed_genesis,
    } = app.defer(genesis_after_hola, genesis_secret, child_secret, child_spec)?
    else {
        unreachable!("genesis defer should succeed");
    };
    println!("derived child index: {child_index}; genesis refreshed to {refreshed_genesis}");

    match app.read("hola", child_index, child_secret)? {
        ReadResult::Unauthorized(_) => {
            println!("child read hola -> unauthorized, as expected");
        }
        other => panic!("expected unauthorized for child reading hola, got {other:?}"),
    }

    let MutationResult::Ok {
        new_cell_index: child_after_write,
    } = app.write("yosoy", b"cel1", child_index, child_secret)?
    else {
        unreachable!("child write should succeed");
    };
    println!("child wrote yosoy -> cel1, refreshed to {child_after_write}");

    let ReadResult::Ok {
        value,
        new_cell_index: genesis_after_read,
    } = app.read("yosoy", refreshed_genesis, genesis_secret)?
    else {
        unreachable!("genesis should read child membrane");
    };
    println!(
        "genesis read yosoy -> {}, refreshed to {genesis_after_read}",
        String::from_utf8_lossy(&value)
    );

    let MutationResult::Ok {
        new_cell_index: genesis_after_delete,
    } = app.delete("yosoy", genesis_after_read, genesis_secret)?
    else {
        unreachable!("genesis should delete child membrane");
    };
    println!("genesis deleted yosoy, refreshed to {genesis_after_delete}");

    match app.read_cell(child_after_write, child_secret)? {
        CellReadResult::Ok { cell_index, .. } => {
            println!("child remains active at resolved index {cell_index}");
        }
        CellReadResult::Unauthorized => {
            panic!("child should still authenticate after its own write");
        }
    }

    drop(app);
    fs::remove_dir_all(dir)?;
    Ok(())
}