# API de Correspondence

Esta guia resume la API publica y el formato de uso esperado para integraciones y agentes.

## Tipos principales

### `CellEngine`

Motor principal de la libreria.

- `CellEngine::new(circular_db_path, kv_path)` crea el motor, carga `.env` y asegura que exista la celula genesis.
- `CellEngine::ejecutar(target_index, solucion, opcode, params)` ejecuta una operacion sobre la celula objetivo.

### `CellError`

Errores de alto nivel devueltos por la API:

- `BadRequest`
- `Unauthorized`
- `Forbidden`
- `NotFound`
- `Internal`

## Conceptos del modelo

- `target_index`: indice inicial de la celula a usar.
- `solucion`: secreto derivado a 32 bytes que desbloquea la celula.
- `index_real`: indice resuelto despues de seguir migraciones.
- `cmd`: bitmask de permisos y flags.

Una celula puede migrar despues de algunas operaciones. Por eso una escritura o borrado puede devolver un nuevo indice real en la respuesta.

## Permisos principales

- `CMD_LEER_SELF`: puede leer claves propias o ligadas por `LINK_L_VERIF` o `LINK_R_VERIF`.
- `CMD_LEER_ANY`: puede leer cualquier clave existente.
- `CMD_ESCRIBIR_SELF`: puede escribir para si misma y sobrescribir solo si sigue siendo propietaria.
- `CMD_ESCRIBIR_ANY`: puede escribir cualquier clave.
- `CMD_BORRAR_SELF`: puede borrar claves propias o ligadas.
- `CMD_BORRAR_ANY`: puede borrar cualquier clave.
- `CMD_DERIVAR`: puede derivar hijas.
- `CMD_COMBINAR`: puede participar en merge con otra celula.

## Flags adicionales

- `CMD_TERMINAL`: si una celula terminal deriva, la hija pierde `CMD_DERIVAR` y `CMD_TERMINAL`.
- `CMD_DOMINANTE`: bloquea `combinar` si alguna de las dos madres lo tiene.
- `CMD_EFIMERA`: la operacion puede convertir la celula en ghost en lugar de migrarla.
- `CMD_INMORTAL`: evita el comportamiento efimero.
- `CMD_CONGELAMIENTO`: impide modificar o borrar valores poseidos por esa celula.
- `CMD_LEER_LIBRE`: capability incluida en genesis.
- `CMD_ANCLA`: en derivacion y merge fuerza `index_l = 0` e `index_r = 0`.
- `CMD_ESTRICTA`: solo permite links verificados contra alguna madre.
- `CMD_LINK_L_VERIF` y `CMD_LINK_R_VERIF`: flags internas de verificacion de enlaces.
- `CMD_MIGRADA`: flag interna para indicar que la celula ya apunta a una version nueva.
- `GHOST_FLAG`: marca interna asociada al ciclo de vida.

## Opcodes

### `0x01` leer

Parametros:

```text
[key_len: u8][key_bytes]
```

Respuesta:

- `Ok(Vec<u8>)` con el valor leido.

Requiere:

- `CMD_LEER_ANY`, o
- `CMD_LEER_SELF` si la clave pertenece a la celula actual o a un link verificado.

### `0x02` escribir

Parametros:

```text
[key_len: u8][key_bytes][payload]
```

Respuesta:

- `Ok([u8; 4])` con el nuevo `index_real` de la celula tras el ciclo de vida.

Requiere:

- `CMD_ESCRIBIR_ANY`, o
- `CMD_ESCRIBIR_SELF` si la sobrescritura sigue siendo sobre una clave propia o ligada.

### `0x03` borrar

Parametros:

```text
[key_len: u8][key_bytes]
```

Respuesta:

- `Ok([u8; 4])` con el nuevo `index_real`.

Requiere:

- `CMD_BORRAR_ANY`, o
- `CMD_BORRAR_SELF` si la clave pertenece a la celula actual o a un link verificado.

### `0x04` derivar

Parametros:

```text
[nuevo_salt: 32]
[nuevo_challenge: 32]
[nuevos_cmd: u32 little endian]
[user_index_l: u32 little endian]
[user_index_r: u32 little endian]
```

Respuesta:

- `Ok([u8; 4])` con el indice de la hija.

Reglas:

- la madre debe tener `CMD_DERIVAR`
- `nuevos_cmd` debe ser subconjunto de los permisos de seguridad de la madre
- `CMD_ANCLA` obliga a no usar links
- `CMD_ESTRICTA` obliga a que los links apunten a la madre actual para ser aceptados

### `0x05` combinar

Parametros:

```text
[index2: u32 little endian]
[solucion2: 32]
[nuevo_salt: 32]
[nuevo_challenge: 32]
[user_index_l: u32 little endian]
[user_index_r: u32 little endian]
```

Respuesta:

- `Ok([u8; 4])` con el indice de la nueva celula combinada.

Reglas:

- ambas madres deben tener `CMD_COMBINAR`
- si alguna tiene `CMD_DOMINANTE`, el merge falla
- los permisos finales son la union de permisos de seguridad de ambas madres
- las madres quedan migradas para apuntar a la nueva celula

### `0x06` inspeccionar

Parametros:

- vacio

Respuesta:

```text
[cmd: u32 little endian]
[index_l: u32 little endian]
[index_r: u32 little endian]
[index_real: u32 little endian]
```

Uso tipico: depuracion, trazabilidad y observacion de migraciones.

## Helpers de parametros

Patron minimo para lectura o borrado:

```rust
fn params_key(key: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + key.len());
    out.push(key.len() as u8);
    out.extend_from_slice(key);
    out
}
```

Patron minimo para escritura:

```rust
fn params_write(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + key.len() + value.len());
    out.push(key.len() as u8);
    out.extend_from_slice(key);
    out.extend_from_slice(value);
    out
}
```

## Ejemplos recomendados

- [examples/probar.rs](../examples/probar.rs): flujo basico de genesis, derivacion self y lectura denegada.
- [examples/mergin.rs](../examples/mergin.rs): flujo de escritura self, merge de capacidades y borrado final.

## Recomendaciones para proyectos consumidores

- Define `GENESIS_SECRET` en `.env`.
- Centraliza helpers para construir `params` binarios.
- Trata el `index_real` devuelto por escribir y borrar como el indice vigente de la celula si vas a seguir operando con ella.
- Si un agente necesita contexto rapido, apúntalo primero a este archivo y a los ejemplos antes de explorar toda la implementacion.