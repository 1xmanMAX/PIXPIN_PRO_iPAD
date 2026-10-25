# De donde sale este crate

Es una **copia** del nucleo de pdfsqueeze, el compresor de PDF del propio autor
de PixPin.

| | |
|---|---|
| Repositorio | `github.com/1xmanMAX/Thesis` (privado) |
| Rama | `claude/laughing-davinci-l4jl8f` (la de por defecto) |
| Commit | `2db0a8338af4b565e7787eba85d456d023025ea7` (2026-09-20) |
| Carpeta de origen | `crates/core` (paquete `pdfsqueeze-core`) |
| Licencia | MIT, ver `LICENSE` (copiado de la raiz de Thesis) |
| Copiado | 2026-09-24 |

## Por que copiado y no como dependencia git

El repositorio es privado. Una dependencia `git = "..."` obliga a tener
credenciales de GitHub para compilar: la CI y cualquier equipo sin ellas no
compilarian PixPin. Copiado, compila en cualquier sitio.

## Que se cambio respecto al original

**Nada del codigo.** `src/` y `tests/` son byte a byte los de Thesis. Solo el
`Cargo.toml` es propio: alli las versiones venian de
`[workspace.dependencies]` del workspace de Thesis y aqui estan escritas a mano,
con los mismos numeros y las mismas features salvo lopdf (ver abajo). La
edicion se queda en 2021 (la
de Thesis) y no en la 2024 del workspace de PixPin.

En PixPin solo lo enlaza su propio ejecutable, `pixpin-aligerar.exe`
(`apps/pixpin-aligerar`), que `pixpinmax.exe` lanza como proceso desde
`apps/pixpin/src/aligerar.rs`. Asi el programa principal no carga ~1,8 MB en
cada arranque, y como el perfil release es `panic = "abort"`, un PDF que haga
entrar en panico al lector de PDF mata al compresor y no a la aplicacion. Es lo
mismo que hace Android con su proceso `:pdfsqueeze`. Una prueba de
`apps/pixpin/tests/capas.rs` vigila que `pixpin` no lo enlace.

## Como actualizarlo

```sh
gh repo clone 1xmanMAX/Thesis thesis
cd thesis && git log -1 --format=%H          # el commit nuevo
rm -rf "<pixpin>/crates/pdfsqueeze-core/src" "<pixpin>/crates/pdfsqueeze-core/tests"
cp -r crates/core/src crates/core/tests "<pixpin>/crates/pdfsqueeze-core/"
cp LICENSE "<pixpin>/crates/pdfsqueeze-core/LICENSE"
```

Despues:

1. Comparar `crates/core/Cargo.toml` y el `[workspace.dependencies]` de Thesis
   con el `Cargo.toml` de aqui, y copiar a mano cualquier dependencia nueva.
2. Apuntar el commit nuevo en la tabla de arriba.
3. `cargo test -p pdfsqueeze-core -p pixpin-aligerar -p pixpin --no-fail-fast -- --test-threads=1`
   y `cargo deny check` (una dependencia nueva tiene que tener licencia de la
   lista de `deny.toml`).

## Dos cosas del Cargo.toml que no son como en Thesis

- **lopdf 0.42 en vez de 0.36.** La 0.36 tiene el aviso RUSTSEC-2026-0187 (un
  PDF con arrays anidados miles de veces agota la pila y aborta el proceso) y
  `cargo deny` la rechaza. Con la 0.42 el codigo compila sin tocar una linea y
  las 21 pruebas de pdfsqueeze siguen en verde (medido en Thesis y aqui). Al
  actualizar, conviene subir tambien la de Thesis.
- **La licencia de jpeg-encoder** es `(MIT OR Apache-2.0) AND IJG`: parte de su
  codigo viene de la libjpeg. `deny.toml` la permite solo para ese crate. La
  licencia IJG pide decirlo en la documentacion: *este programa se basa en
  parte en el trabajo del Independent JPEG Group*.

## Pruebas en depuracion

pdfsqueeze cuenta con la aritmetica del release, que da la vuelta sin avisar (la
imagen integral de `mrc.rs` resta `u32` que se envuelven a proposito). En Thesis
las pruebas se corren con `--release`; aqui el `Cargo.toml` del workspace le
quita a este paquete las comprobaciones de desbordamiento tambien en
depuracion (`[profile.dev.package.pdfsqueeze-core]`), para que
`cargo test -p pdfsqueeze-core` pruebe el mismo codigo que va en el ejecutable.
