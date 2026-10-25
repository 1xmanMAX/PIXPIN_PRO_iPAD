#!/usr/bin/env bash
# Compila PixPin para iPad con el proyecto de Xcode (XcodeGen, `xcode/project.yml`).
#
#     herramientas/app.sh simulador   # la app para el simulador -> dist/PixPin-simulador.zip
#     herramientas/app.sh pruebas     # las pruebas en el simulador de un iPad
#     herramientas/app.sh ipa         # la app para el iPad, sin firmar -> dist/PixPin-sin-firmar.ipa
#
# Antes: `herramientas/xcframework.sh` (el nucleo de Rust) y `xcodegen generate`
# en `xcode/` (lo hace este script si falta el proyecto).
set -euo pipefail

APPLE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$APPLE"
DIST="$APPLE/dist"
DERIVADOS="$APPLE/target/xcode"
mkdir -p "$DIST"

if [[ ! -d xcode/PixPin.xcodeproj ]]; then
    (cd xcode && xcodegen generate)
fi

# Un iPad del simulador que exista en esta maquina (el primero que haya).
simulador() {
    xcrun simctl list devices available -j | python3 -c '
import json, sys
d = json.load(sys.stdin)["devices"]
for runtime, lista in sorted(d.items(), reverse=True):
    if "iOS" not in runtime:
        continue
    for x in lista:
        if "iPad" in x["name"]:
            print(x["udid"]); sys.exit(0)
sys.exit(1)'
}

# Corre xcodebuild con todo al registro y, si falla, ensena los errores.
correr() {
    local registro="$1"; shift
    if ! xcodebuild "$@" > "$registro" 2>&1; then
        grep -E "error:|Test Case.*failed|XCTAssert|fatal" "$registro" | sort -u | head -1500 || true
        tail -40 "$registro"
        echo "==> Fallo (registro entero: $registro)" >&2
        return 1
    fi
    grep -E "Executed|\*\* (BUILD|TEST)" "$registro" | tail -5 || true
}

# Que Xcode siga compilando los demas ficheros tras un error.
defaults write com.apple.dt.Xcode IDEBuildingContinueBuildingAfterErrors -bool YES 2>/dev/null || true

comun=(-project xcode/PixPin.xcodeproj -scheme AppPixPin -derivedDataPath "$DERIVADOS"
       CODE_SIGNING_ALLOWED=NO COMPILER_INDEX_STORE_ENABLE=NO)

case "${1:-simulador}" in
simulador)
    # Fichero a fichero y sin pararse al primer error: asi una pasada de la
    # CI ensena todos los errores, no solo los primeros.
    correr "$DIST/simulador.log" build "${comun[@]}" -configuration Debug -sdk iphonesimulator \
        -destination "generic/platform=iOS Simulator" \
        SWIFT_COMPILATION_MODE=singlefile ARCHS=arm64 ONLY_ACTIVE_ARCH=YES
    app="$DERIVADOS/Build/Products/Debug-iphonesimulator/PixPin.app"
    (cd "$(dirname "$app")" && rm -f "$DIST/PixPin-simulador.zip" && zip -qry "$DIST/PixPin-simulador.zip" PixPin.app)
    echo "==> $DIST/PixPin-simulador.zip"
    ;;
pruebas)
    udid="$(simulador)"
    echo "==> Simulador: $udid"
    # Sin quitar codigo muerto: las pruebas usan del nucleo cosas que la app no.
    correr "$DIST/pruebas.log" test "${comun[@]}" -configuration Debug -destination "id=$udid" \
        ARCHS=arm64 ONLY_ACTIVE_ARCH=YES DEAD_CODE_STRIPPING=NO
    ;;
ipa)
    correr "$DIST/ipa.log" build "${comun[@]}" -configuration Release -sdk iphoneos \
        -destination "generic/platform=iOS"
    app="$DERIVADOS/Build/Products/Release-iphoneos/PixPin.app"
    tmp="$(mktemp -d)"
    mkdir -p "$tmp/Payload"
    cp -R "$app" "$tmp/Payload/"
    (cd "$tmp" && rm -f "$DIST/PixPin-sin-firmar.ipa" && zip -qry "$DIST/PixPin-sin-firmar.ipa" Payload)
    rm -rf "$tmp"
    echo "==> $DIST/PixPin-sin-firmar.ipa"
    ;;
*)
    echo "uso: $0 simulador|pruebas|ipa" >&2
    exit 2
    ;;
esac
