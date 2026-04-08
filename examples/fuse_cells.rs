use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use correspondence::{
    AuthenticatedCellReadOutcome, CellDerivationOutcome, CellFusionOutcome, ChildSpec,
    Correspondence, CorrespondenceConfig, Result,
};
use ouroboros::Genoma;

fn temp_dir() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("correspondence-fuse-cells-{unique}"));
    fs::create_dir_all(&path).expect("temporary directory should be created");
    path
}

fn write_ouroboros_config(dir: &std::path::Path) -> PathBuf {
    let config_path = dir.join("ouroboros.toml");
    fs::write(
        &config_path,
        "data_path = \"ouroboros.db\"\nmax_records = 64\nsync_writes = false\n",
    )
    .expect("config file should be written");
    config_path
}

fn main() -> Result<()> {
    let dir = temp_dir();
    let config_path = write_ouroboros_config(&dir);
    let membranes_path = dir.join("membranes.redb");
    let genesis_secret = b"diarsaba";

    let genesis_genoma = Correspondence::default_genesis_genoma() | Genoma::FUSIONAR;
    let config = CorrespondenceConfig::new(&config_path, &membranes_path);
    let mut app = Correspondence::builder(config)
        .with_explicit_genesis(genesis_secret, genesis_genoma)
        .build()?;

    let genesis_index = app.genesis_index()?;
    println!("genesis index: {genesis_index}");

    let fusion_genoma = Genoma::LEER_SELF | Genoma::ESCRIBIR_SELF | Genoma::FUSIONAR;

    let CellDerivationOutcome::Ok {
        deferred_index: cell_a,
        new_cell_index: genesis_after_a,
    } = app.derive_cell(
        genesis_index,
        genesis_secret,
        b"cell-a-secret",
        ChildSpec {
            salt: [3u8; 16],
            genoma: fusion_genoma,
            x: 10,
            y: 20,
            z: 30,
        },
    )?
    else {
        unreachable!("first derived cell should succeed");
    };
    println!("derived cell A: {cell_a}; genesis refreshed to {genesis_after_a}");

    let CellDerivationOutcome::Ok {
        deferred_index: cell_b,
        new_cell_index: genesis_after_b,
    } = app.derive_cell(
        genesis_after_a,
        genesis_secret,
        b"cell-b-secret",
        ChildSpec {
            salt: [4u8; 16],
            genoma: fusion_genoma,
            x: 40,
            y: 50,
            z: 60,
        },
    )?
    else {
        unreachable!("second derived cell should succeed");
    };
    println!("derived cell B: {cell_b}; genesis refreshed to {genesis_after_b}");

    let CellFusionOutcome::Ok {
        child_index,
        new_cell_index_a,
        new_cell_index_b,
    } = app.fuse_cells(
        cell_a,
        b"cell-a-secret",
        cell_b,
        b"cell-b-secret",
        b"cell-child-secret",
        ChildSpec {
            salt: [5u8; 16],
            genoma: 0,
            x: 70,
            y: 80,
            z: 90,
        },
    )?
    else {
        unreachable!("cell fusion should succeed");
    };
    println!(
        "fused child index: {child_index}; cell A refreshed to {new_cell_index_a}; cell B refreshed to {new_cell_index_b}"
    );

    match app.read_authenticated_cell(child_index, b"cell-child-secret")? {
        AuthenticatedCellReadOutcome::Ok { celula, cell_index } => {
            println!(
                "child resolved at {cell_index} with genoma={} x={} y={} z={}",
                celula.genoma, celula.x, celula.y, celula.z
            );
        }
        AuthenticatedCellReadOutcome::Unauthorized => {
            panic!("fused child should authenticate with its secret");
        }
    }

    drop(app);
    fs::remove_dir_all(dir)?;
    Ok(())
}