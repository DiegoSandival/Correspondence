use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use correspondence::{
    AuthenticatedCellReadOutcome, CellDerivationOutcome, CellFusionOutcome, ChildSpec,
    Correspondence, CorrespondenceConfig, FreeMembraneReadOutcome, MembraneMutationOutcome,
    MembraneReadOutcome,
};
use ouroboros::Genoma;

type TerminalResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Session {
    working_dir: Option<PathBuf>,
    app: Option<Correspondence>,
}

fn main() -> TerminalResult<()> {
    let mut session = Session::default();

    print_banner();

    loop {
        print!("corr> ");
        io::stdout().flush()?;

        let mut line = String::new();
        let bytes_read = io::stdin().read_line(&mut line)?;
        if bytes_read == 0 {
            println!();
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        let command = parts[0];
        let args = &parts[1..];

        let outcome = match command {
            "help" => {
                print_help();
                Ok(())
            }
            "init" => handle_init(&mut session, args),
            "open" => handle_open(&mut session, args),
            "provision" => handle_provision(&mut session, args),
            "status" => handle_status(&session),
            "write" => handle_write(&mut session, args),
            "read" => handle_read(&mut session, args),
            "free-read" => handle_free_read(&session, args),
            "delete" => handle_delete(&mut session, args),
            "derive-self-rw" => handle_derive(&mut session, args, false),
            "derive-self-rw-fusion" => handle_derive(&mut session, args, true),
            "fuse" => handle_fuse(&mut session, args),
            "read-cell" => handle_read_cell(&session, args),
            "exit" | "quit" => break,
            _ => {
                eprintln!("unknown command: {command}");
                print_help();
                Ok(())
            }
        };

        if let Err(error) = outcome {
            eprintln!("error: {error}");
        }
    }

    Ok(())
}

fn print_banner() {
    println!("Correspondence terminal example");
    println!("Start from zero with: init ./terminal-data 64");
    println!("Then provision genesis with: provision diarsaba");
    println!("Type help to see all commands. Values and keys are whitespace-delimited.");
}

fn print_help() {
    println!("Commands:");
    println!("  init [dir] [max_records]");
    println!("  open <dir>");
    println!("  provision <secret> [default|fusion]");
    println!("  status");
    println!("  write <key> <value> <cell_index> <secret>");
    println!("  read <key> <cell_index> <secret>");
    println!("  free-read <key>");
    println!("  delete <key> <cell_index> <secret>");
    println!("  derive-self-rw <cell_index> <parent_secret> <child_secret>");
    println!("  derive-self-rw-fusion <cell_index> <parent_secret> <child_secret>");
    println!("  fuse <cell_a> <secret_a> <cell_b> <secret_b> <child_secret>");
    println!("  read-cell <cell_index> <secret>");
    println!("  exit");
}

fn handle_init(session: &mut Session, args: &[&str]) -> TerminalResult<()> {
    let dir = match args.first() {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir()?.join("terminal-data"),
    };
    let max_records = parse_optional_u32(args.get(1).copied(), 64)?;

    fs::create_dir_all(&dir)?;
    let config_path = dir.join("ouroboros.toml");
    let membranes_path = dir.join("membranes.redb");

    if !config_path.exists() {
        fs::write(
            &config_path,
            format!(
                "data_path = \"ouroboros.db\"\nmax_records = {max_records}\nsync_writes = false\n"
            ),
        )?;
        println!("created config at {}", config_path.display());
    } else {
        println!("reusing config at {}", config_path.display());
    }

    session.app = Some(
        Correspondence::builder(CorrespondenceConfig::new(&config_path, &membranes_path))
            .without_genesis_provisioning()
            .build()?,
    );
    session.working_dir = Some(dir.clone());

    println!("infrastructure ready at {}", dir.display());
    Ok(())
}

fn handle_open(session: &mut Session, args: &[&str]) -> TerminalResult<()> {
    let dir = PathBuf::from(required_arg(args, 0, "dir")?);
    let config_path = dir.join("ouroboros.toml");
    let membranes_path = dir.join("membranes.redb");

    if !config_path.exists() {
        return Err(format!("missing config file at {}", config_path.display()).into());
    }

    session.app = Some(
        Correspondence::builder(CorrespondenceConfig::new(&config_path, &membranes_path))
            .without_genesis_provisioning()
            .build()?,
    );
    session.working_dir = Some(dir.clone());

    println!("opened infrastructure at {}", dir.display());
    Ok(())
}

fn handle_provision(session: &mut Session, args: &[&str]) -> TerminalResult<()> {
    let secret = required_arg(args, 0, "secret")?;
    let mode = args.get(1).copied().unwrap_or("default");

    let genoma = match mode {
        "default" => Correspondence::default_genesis_genoma(),
        "fusion" => Correspondence::default_genesis_genoma() | Genoma::FUSIONAR,
        _ => return Err(format!("unknown provision mode: {mode}").into()),
    };

    let app = require_app_mut(session)?;
    let index = app.provision_genesis(secret.as_bytes(), genoma)?;
    println!("genesis provisioned at index {index} with mode {mode}");
    Ok(())
}

fn handle_status(session: &Session) -> TerminalResult<()> {
    let Some(app) = session.app.as_ref() else {
        println!("no open session. Use init or open first.");
        return Ok(());
    };

    match &session.working_dir {
        Some(dir) => println!("working dir: {}", dir.display()),
        None => println!("working dir: <none>"),
    }

    match app.genesis_index() {
        Ok(index) => println!("genesis index: {index}"),
        Err(_) => println!("genesis index: <not provisioned>"),
    }

    Ok(())
}

fn handle_write(session: &mut Session, args: &[&str]) -> TerminalResult<()> {
    let key = required_arg(args, 0, "key")?;
    let value = required_arg(args, 1, "value")?;
    let cell_index = parse_u32(required_arg(args, 2, "cell_index")?)?;
    let secret = required_arg(args, 3, "secret")?;

    let app = require_app_mut(session)?;
    match app.write_membrane(key, value.as_bytes(), cell_index, secret.as_bytes())? {
        MembraneMutationOutcome::Ok { new_cell_index } => {
            println!("ok new_cell_index={new_cell_index}");
        }
        MembraneMutationOutcome::Undefined { cell_index } => {
            println!("undefined cell_index={cell_index}");
        }
        MembraneMutationOutcome::Unauthorized(details) => {
            println!("unauthorized cell_index={:?}", details.cell_index);
        }
    }

    Ok(())
}

fn handle_read(session: &mut Session, args: &[&str]) -> TerminalResult<()> {
    let key = required_arg(args, 0, "key")?;
    let cell_index = parse_u32(required_arg(args, 1, "cell_index")?)?;
    let secret = required_arg(args, 2, "secret")?;

    let app = require_app_mut(session)?;
    match app.read_membrane(key, cell_index, secret.as_bytes())? {
        MembraneReadOutcome::Ok {
            value,
            new_cell_index,
        } => {
            println!(
                "ok new_cell_index={} value={}",
                new_cell_index,
                String::from_utf8_lossy(&value)
            );
        }
        MembraneReadOutcome::Undefined { cell_index } => {
            println!("undefined cell_index={cell_index}");
        }
        MembraneReadOutcome::Unauthorized(details) => {
            println!("unauthorized cell_index={:?}", details.cell_index);
        }
    }

    Ok(())
}

fn handle_free_read(session: &Session, args: &[&str]) -> TerminalResult<()> {
    let key = required_arg(args, 0, "key")?;
    let app = require_app_ref(session)?;

    match app.read_membrane_free(key)? {
        FreeMembraneReadOutcome::Ok { value } => {
            println!("ok value={}", String::from_utf8_lossy(&value));
        }
        FreeMembraneReadOutcome::Undefined => println!("undefined"),
        FreeMembraneReadOutcome::Unauthorized => println!("unauthorized"),
    }

    Ok(())
}

fn handle_delete(session: &mut Session, args: &[&str]) -> TerminalResult<()> {
    let key = required_arg(args, 0, "key")?;
    let cell_index = parse_u32(required_arg(args, 1, "cell_index")?)?;
    let secret = required_arg(args, 2, "secret")?;

    let app = require_app_mut(session)?;
    match app.delete_membrane(key, cell_index, secret.as_bytes())? {
        MembraneMutationOutcome::Ok { new_cell_index } => {
            println!("ok new_cell_index={new_cell_index}");
        }
        MembraneMutationOutcome::Undefined { cell_index } => {
            println!("undefined cell_index={cell_index}");
        }
        MembraneMutationOutcome::Unauthorized(details) => {
            println!("unauthorized cell_index={:?}", details.cell_index);
        }
    }

    Ok(())
}

fn handle_derive(session: &mut Session, args: &[&str], with_fusion: bool) -> TerminalResult<()> {
    let cell_index = parse_u32(required_arg(args, 0, "cell_index")?)?;
    let parent_secret = required_arg(args, 1, "parent_secret")?;
    let child_secret = required_arg(args, 2, "child_secret")?;

    let mut genoma = Genoma::LEER_SELF | Genoma::ESCRIBIR_SELF;
    if with_fusion {
        genoma |= Genoma::FUSIONAR;
    }

    let app = require_app_mut(session)?;
    match app.derive_cell(
        cell_index,
        parent_secret.as_bytes(),
        child_secret.as_bytes(),
        ChildSpec {
            salt: next_salt(),
            genoma,
            x: 0,
            y: 0,
            z: 0,
        },
    )? {
        CellDerivationOutcome::Ok {
            deferred_index,
            new_cell_index,
        } => {
            println!(
                "ok deferred_index={} parent_new_cell_index={}",
                deferred_index, new_cell_index
            );
        }
        CellDerivationOutcome::Unauthorized(details) => {
            println!("unauthorized cell_index={:?}", details.cell_index);
        }
    }

    Ok(())
}

fn handle_fuse(session: &mut Session, args: &[&str]) -> TerminalResult<()> {
    let cell_a = parse_u32(required_arg(args, 0, "cell_a")?)?;
    let secret_a = required_arg(args, 1, "secret_a")?;
    let cell_b = parse_u32(required_arg(args, 2, "cell_b")?)?;
    let secret_b = required_arg(args, 3, "secret_b")?;
    let child_secret = required_arg(args, 4, "child_secret")?;

    let app = require_app_mut(session)?;
    match app.fuse_cells(
        cell_a,
        secret_a.as_bytes(),
        cell_b,
        secret_b.as_bytes(),
        child_secret.as_bytes(),
        ChildSpec {
            salt: next_salt(),
            genoma: 0,
            x: 0,
            y: 0,
            z: 0,
        },
    )? {
        CellFusionOutcome::Ok {
            child_index,
            new_cell_index_a,
            new_cell_index_b,
        } => {
            println!(
                "ok child_index={} new_cell_index_a={} new_cell_index_b={}",
                child_index, new_cell_index_a, new_cell_index_b
            );
        }
        CellFusionOutcome::Unauthorized {
            cell_index_a,
            cell_index_b,
        } => {
            println!(
                "unauthorized cell_index_a={:?} cell_index_b={:?}",
                cell_index_a, cell_index_b
            );
        }
    }

    Ok(())
}

fn handle_read_cell(session: &Session, args: &[&str]) -> TerminalResult<()> {
    let cell_index = parse_u32(required_arg(args, 0, "cell_index")?)?;
    let secret = required_arg(args, 1, "secret")?;

    let app = require_app_ref(session)?;
    match app.read_authenticated_cell(cell_index, secret.as_bytes())? {
        AuthenticatedCellReadOutcome::Ok { celula, cell_index } => {
            println!(
                "ok resolved_index={} genoma={} x={} y={} z={}",
                cell_index, celula.genoma, celula.x, celula.y, celula.z
            );
        }
        AuthenticatedCellReadOutcome::Unauthorized => println!("unauthorized"),
    }

    Ok(())
}

fn require_app_mut(session: &mut Session) -> TerminalResult<&mut Correspondence> {
    session
        .app
        .as_mut()
        .ok_or_else(|| "no open session, use init or open first".into())
}

fn require_app_ref(session: &Session) -> TerminalResult<&Correspondence> {
    session
        .app
        .as_ref()
        .ok_or_else(|| "no open session, use init or open first".into())
}

fn required_arg<'a>(args: &'a [&str], index: usize, name: &str) -> TerminalResult<&'a str> {
    args.get(index)
        .copied()
        .ok_or_else(|| format!("missing argument: {name}").into())
}

fn parse_u32(raw: &str) -> TerminalResult<u32> {
    raw.parse::<u32>()
        .map_err(|error| format!("invalid u32 '{raw}': {error}").into())
}

fn parse_optional_u32(raw: Option<&str>, default: u32) -> TerminalResult<u32> {
    match raw {
        Some(value) => parse_u32(value),
        None => Ok(default),
    }
}

fn next_salt() -> [u8; 16] {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after unix epoch")
        .as_nanos();
    nanos.to_le_bytes()
}

#[allow(dead_code)]
fn _assert_path(_: &Path) {}