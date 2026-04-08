# correspondence

Libreria Rust minima que combina dos capas persistentes:

- `ouroboros/minimal` para identidad, autorizacion y rotacion circular de `Celula`.
- `redb` para almacenar membranas key/value por `String`.

## Estado actual

El crate ya implementa:

- apertura explicita de infraestructura con `open_infrastructure(...)`
- bootstrap/provision automatica opcional con `open_with_env_bootstrap(...)`
- provision explicita de genesis con `provision_genesis(...)`
- organizacion fisica del crate por intencion: `infrastructure`, `provisioning` y `domain`
- resolucion de celulas migradas con `Genoma::MIGRADA`
- operaciones `read_membrane`, `read_membrane_free`, `write_membrane`, `delete_membrane`, `derive_cell`, `fuse_cells` y `read_authenticated_cell`
- resultados de dominio con nombres explicitos: `MembraneReadOutcome`, `MembraneMutationOutcome`, `AuthenticatedCellReadOutcome`, `CellDerivationOutcome` y `CellFusionOutcome`
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

## Superficie de apertura

- `Correspondence::open_infrastructure(config)` abre `ouroboros` y `redb`, pero no crea dominio inicial.
- `Correspondence::provision_genesis(secret, genoma)` crea explicitamente la celula genesis si aun no existe.
- `Correspondence::open_with_env_bootstrap(config)` combina ambos pasos y provisiona genesis desde la variable de entorno configurada.
- `Correspondence::builder(config)` permite describir el flujo de apertura y provision como pasos declarativos.
- el builder ahora exige elegir una sola estrategia de provision por instancia; si mezclas modos, `build()` falla con `InvalidBuilderState`.

## Uso minimo

```rust
use correspondence::{
	Correspondence, CorrespondenceConfig, MembraneMutationOutcome, MembraneReadOutcome,
};

fn demo() -> correspondence::Result<()> {
	let config = CorrespondenceConfig::new("./ouroboros.toml", "./membranes.redb");
	let mut app = Correspondence::builder(config)
		.with_configured_env_bootstrap()
		.build()?;

	let genesis_index = app.genesis_index()?;

	let MembraneMutationOutcome::Ok { new_cell_index } = app.write_membrane(
		"saludo",
		b"hola",
		genesis_index,
		b"diarsaba",
	)? else {
		unreachable!()
	};

	let MembraneReadOutcome::Ok { value, new_cell_index: _ } = app.read_membrane(
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

## Provision explicita

```rust
use correspondence::{Correspondence, CorrespondenceConfig};

fn demo() -> correspondence::Result<()> {
	let config = CorrespondenceConfig::new("./ouroboros.toml", "./membranes.redb");
	let mut app = Correspondence::builder(config)
		.with_explicit_genesis(b"diarsaba", Correspondence::default_genesis_genoma())
		.build()?;
	let genesis_index = app.genesis_index()?;

	assert_eq!(genesis_index, 0);
	Ok(())
}
```

## Verificacion

Si trabajas desde este workspace montado en WSL, ejecuta pruebas dentro de WSL:

```bash
wsl.exe sh -lc 'cd /home/starnet/Code/Correspondence; cargo test'
```

Ejemplos disponibles:

- `cargo run --example basic`
- `cargo run --example explicit_genesis`
- `cargo run --example fuse_cells`
- `cargo run --example terminal`
- `cargo run --example websocket_server -- ./ws-data 127.0.0.1:3000`

## Transporte WebSocket Binario

La ruta HTTP `GET /` ya queda servida por Rust con una UI minima en HTML/JS vanilla.
La ruta HTTP `GET /app.js` sirve el cliente del navegador desde el mismo proceso Rust.
La ruta `GET /ws` acepta solo frames binarios de WebSocket. No usa JSON.

El servidor ahora es multi-base de datos. Cada DB vive bajo un UUID generado por el servidor y cada operacion de dominio incluye ese `db_uuid` en el frame binario.

Formato general:

- request: `opcode:u8 + payload`
- response: `status:u8 + opcode:u8 + payload`
- enteros: little-endian
- strings y bytes: `len:u32 + raw bytes`

Operaciones de registro de DB:

- `0x01` list databases
- `0x02` create database
- `0x03` delete database
- `0x04` database status
- `0x05` provision genesis

Opcodes actuales:

- `0x10` write membrane
- `0x11` read membrane
- `0x12` free read membrane
- `0x13` delete membrane
- `0x20` derive cell
- `0x21` fuse cells
- `0x22` read authenticated cell

Estados de respuesta:

- `0x00` ok
- `0x01` undefined
- `0x02` unauthorized
- `0xff` error

Notas practicas:

- `create database` recibe `max_records:u32` y devuelve `db_uuid + max_records + genesis_index?`
- `database status` devuelve `db_uuid + max_records + genesis_index?`
- `provision genesis` recibe `db_uuid + secret + genoma + x + y + z`
- el cliente web expone 31 genes editables: bits `0..11` con nombre y `12..30` como `customN`
- los valores de membrana se pueden editar como texto UTF-8 o hex crudo

El contrato binario vive en la libreria y se exporta desde:

- `RawCommand`
- `RawResponse`
- `decode_command(...)`
- `encode_response(...)`
- `execute_command(...)`
