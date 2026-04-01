# Correspondence

Correspondence es una libreria Rust para modelar capacidades como celulas encadenadas.
Cada celula tiene un secreto, un candado derivado de ese secreto y un conjunto de permisos
que define si puede leer, escribir, borrar, derivar o combinar nuevas celulas.

La libreria mantiene dos almacenes internos:

- un registro circular con la definicion de las celulas
- un KV store temporal con los datos escritos por cada celula

## Modelo mental

- La celula genesis o "celula dios" vive en el indice `0`.
- Cada celula se desbloquea presentando su solucion de 32 bytes.
- Los permisos se expresan como flags `CMD_*`.
- `derivar` crea una celula hija con un subconjunto de permisos de la madre.
- `combinar` hace merge de dos celulas y une sus permisos compatibles.
- Algunas operaciones migran la celula a un nuevo indice real. La libreria resuelve eso internamente.

## Instalacion

Desde otro proyecto Rust puedes importar la crate desde GitHub:

```toml
[dependencies]
correspondence = { git = "https://github.com/TU_USUARIO/Correspondence.git", package = "Correspondence" }
dotenvy = "0.15"
blake2 = "0.10"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Configuracion

La inyeccion de genesis usa la variable de entorno `GENESIS_SECRET`.
La forma mas comoda es definirla en un archivo `.env`:

```dotenv
GENESIS_SECRET=tu_secreto_de_la_celula_dios
```

`CellEngine::new(...)` ya carga `.env` automaticamente.

## Uso minimo

```rust
use blake2::digest::consts::U32;
use blake2::digest::Digest;
use blake2::Blake2b;
use correspondence::CellEngine;
use dotenvy::dotenv;

fn blake2b32(input: &[u8]) -> [u8; 32] {
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(input);
    hasher.finalize().into()
}

fn params_key_value(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + key.len() + value.len());
    out.push(key.len() as u8);
    out.extend_from_slice(key);
    out.extend_from_slice(value);
    out
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenv();

    let genesis_secret = std::env::var("GENESIS_SECRET")?;
    let genesis_solution = blake2b32(genesis_secret.as_bytes());

    let engine = CellEngine::new("/tmp/cells.db", "/tmp/kv.redb").await?;
    let params = params_key_value(b"hola", b"mundo");

    engine.ejecutar(0, &genesis_solution, 0x02, &params).await?;
    Ok(())
}
```

## Operaciones disponibles

- `0x01`: leer
- `0x02`: escribir
- `0x03`: borrar
- `0x04`: derivar
- `0x05`: combinar
- `0x06`: inspeccionar

Las operaciones usan `CellEngine::ejecutar(...)`.
Los parametros binarios y los permisos requeridos estan descritos en [docs/API.md](docs/API.md).

## Ejemplos

- `cargo run --example probar`
- `cargo run --example mergin`

Si trabajas desde Windows sobre una ruta `\\wsl.localhost\...`, lo mas fiable es ejecutar los ejemplos dentro de WSL.

## Que deberia leer primero una IA

Si un agente necesita entender rapido la libreria, este es el orden recomendado:

1. este `README.md`
2. [docs/API.md](docs/API.md)
3. [examples/probar.rs](examples/probar.rs)
4. [examples/mergin.rs](examples/mergin.rs)
5. [src/lib.rs](src/lib.rs)

Con eso normalmente no hace falta releer toda la implementacion interna.