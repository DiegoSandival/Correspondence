# Plantilla para el proyecto principal

Este archivo sirve como base para un `copilot-instructions.md` en el proyecto consumidor de Correspondence.

## Objetivo

Este proyecto usa la libreria `correspondence` para modelar capacidades mediante celulas, secretos y permisos `CMD_*`.
Antes de proponer cambios, el agente debe asumir que la logica de seguridad y autorizacion vive en la crate `correspondence` y que no debe reimplementar esa semantica localmente salvo que el codigo existente ya lo haga.

## Fuente de verdad

Cuando necesites entender la libreria o usarla correctamente, consulta en este orden:

1. `README.md` del repo de `correspondence`
2. `docs/API.md` del repo de `correspondence`
3. `examples/probar.rs`
4. `examples/mergin.rs`

## Reglas de uso

- Trata `CellEngine` como la API principal de ejecucion.
- Usa `CellEngine::new(...)` para inicializar el motor.
- Usa `CellEngine::ejecutar(...)` con opcodes en lugar de acceder a detalles internos de almacenamiento.
- Asume que algunas operaciones migran la celula y que el indice real puede cambiar.
- Si una operacion de escritura o borrado devuelve 4 bytes, interpretalos como el `u32` del indice real vigente.
- No infieras permisos por nombre; usa los flags `CMD_*` documentados por la libreria.
- Un merge requiere `CMD_COMBINAR` en ambas madres.
- `GENESIS_SECRET` debe existir en el entorno o en `.env`.

## Convenciones recomendadas en este proyecto

- Centraliza helpers para construir parametros binarios de lectura, escritura, borrado, derivacion y merge.
- Evita hardcodear opcodes sueltos en muchos sitios; define wrappers o funciones auxiliares.
- Si aparece `Forbidden`, revisa primero permisos y propiedad de la clave antes de asumir un bug.
- Si aparece `Unauthorized`, revisa la solucion de la celula y si el indice apuntaba a una celula migrada.

## Al responder o editar codigo

- Prefiere apoyarte en la API publica de `correspondence` antes que replicar reglas del modelo.
- Si falta contexto, explica que la semantica oficial vive en la documentacion y ejemplos de la libreria.
- Si un cambio requiere nuevas capacidades, documenta que flag `CMD_*` es necesario y por que.

## Nota para agentes

No hace falta releer toda la implementacion interna de `correspondence` en cada tarea.
Normalmente basta con revisar la documentacion y los ejemplos, y solo bajar a `src/ops.rs` o `src/cell.rs` si hay una duda puntual de semantica.