#!/usr/bin/env bash
# Compila el nucleo de Rust (`nucleo/pixpin-nucleo`) como
# `PixPinCore/PixPinCoreFFI.xcframework` para el iPad (y su simulador) y
# regenera los enlaces de Swift (`PixPinCore/Sources/PixPinCore/pixpin_nucleo.swift`).
# Para un Mac con Xcode; se ejecuta desde cualquier sitio:
#
#     apple/herramientas/xcframework.sh
#
# Es el mismo nucleo que el de la version de Mac (PIXPIN_PRO_MAC/apple/nucleo),
# solo cambian las plataformas:
#   - iPad:                aarch64-apple-ios
#   - simulador del iPad:  aarch64-apple-ios-sim (Apple Silicon)
#
# Perfil `ffi` (apple/Cargo.toml): el release con `panic = "unwind"`, para que
# un panico de Rust llegue a Swift como error en vez de cerrar la app.
# `PIXPIN_PERFIL=ffi-rapido` (la CI) compila en una fraccion del tiempo.
set -euo pipefail

APPLE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$APPLE"

PERFIL="${PIXPIN_PERFIL:-ffi}"
CRATE=pixpin-nucleo
LIB=libpixpin_nucleo.a
MODULO=pixpin_nucleoFFI
SALIDA="$APPLE/target/xcframework"
PAQUETE="$APPLE/PixPinCore"
XCF="$PAQUETE/PixPinCoreFFI.xcframework"

export IPHONEOS_DEPLOYMENT_TARGET=18.0

read -r -a OBJETIVOS <<< "${PIXPIN_OBJETIVOS:-aarch64-apple-ios aarch64-apple-ios-sim}"

echo "==> Objetivos de Rust"
rustup target add "${OBJETIVOS[@]}" >/dev/null

for t in "${OBJETIVOS[@]}"; do
    echo "==> cargo build $t ($PERFIL)"
    cargo build -p "$CRATE" --lib --profile "$PERFIL" --target "$t"
done

echo "==> Enlaces de Swift (uniffi-bindgen)"
# Se leen de una biblioteca dinamica de esta maquina SIN quitar simbolos: los
# metadatos de UniFFI viajan como simbolos.
cargo build -p "$CRATE" --lib
ENLACES="$SALIDA/enlaces"
rm -rf "$ENLACES"
cargo run -q -p "$CRATE" --features bindgen --bin uniffi-bindgen -- \
    generate --library "target/debug/libpixpin_nucleo.dylib" \
    --language swift --out-dir "$ENLACES"

CABECERAS="$SALIDA/Headers"
rm -rf "$CABECERAS"
mkdir -p "$CABECERAS/$MODULO"
cp "$ENLACES/$MODULO.h" "$CABECERAS/$MODULO/"
cp "$ENLACES/$MODULO.modulemap" "$CABECERAS/$MODULO/module.modulemap"

mkdir -p "$PAQUETE/Sources/PixPinCore"
cp "$ENLACES/pixpin_nucleo.swift" "$PAQUETE/Sources/PixPinCore/pixpin_nucleo.swift"

echo "==> xcodebuild -create-xcframework"
NUEVO="$SALIDA/PixPinCoreFFI.xcframework"
rm -rf "$NUEVO"
BIBLIOTECAS=()
for t in "${OBJETIVOS[@]}"; do
    BIBLIOTECAS+=(-library "target/$t/$PERFIL/$LIB" -headers "$CABECERAS")
done
xcodebuild -create-xcframework "${BIBLIOTECAS[@]}" -output "$NUEVO"
rm -rf "$XCF"
mv "$NUEVO" "$XCF"

echo "==> Listo: $XCF"
du -sh "$XCF"/*/ 2>/dev/null || true
