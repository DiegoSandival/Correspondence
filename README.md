# correspondence

Libreria Rust minima que combina dos capas persistentes:

- `ouroboros/minimal` para identidad, autorizacion y rotacion circular de `Celula`.
- `redb` para almacenar membranas key/value por `String`.

## Estado actual

El crate ya implementa:

- bootstrap automatico de una celula genesis usando `GENESIS_SECRET`
- apertura dual de `ouroboros` y `redb`
- resolucion de celulas migradas con `Genoma::MIGRADA`
- operaciones `read`, `read_free`, `write`, `delete`, `defer`, `cross` y `read_cell`
- pruebas base de bootstrap y ciclo `write/read/delete`

## Configuracion

Necesitas un archivo `ouroboros.toml` compatible con la rama `minimal`:

```toml
data_path = "ouroboros.db"
max_records = 32
sync_writes = false
```

Tambien necesitas exponer un secreto genesis por entorno:

```bash
export GENESIS_SECRET="diarsaba"
```

## Uso minimo

```rust
use correspondence::{Correspondence, CorrespondenceConfig, MutationResult, ReadResult};

fn demo() -> correspondence::Result<()> {
	let config = CorrespondenceConfig::new("./ouroboros.toml", "./membranes.redb");
	let mut app = Correspondence::open(config)?;

	let genesis_index = app.genesis_index()?;

	let MutationResult::Ok { new_cell_index } = app.write(
		"saludo",
		b"hola",
		genesis_index,
		b"diarsaba",
	)? else {
		unreachable!()
	};

	let ReadResult::Ok { value, new_cell_index: _ } = app.read(
		"saludo",
		new_cell_index,
		b"diarsaba",
	)? else {
		unreachable!()
	};

	assert_eq!(value, b"hola");
	Ok(())
}
```

## Verificacion

Si trabajas desde este workspace montado en WSL, ejecuta pruebas dentro de WSL:

```bash
wsl.exe sh -lc 'cd /home/starnet/Code/Correspondence; cargo test'
```
