# PixPin Max Pro para iPad

**PixPin Max** es una app para **capturar, anotar y organizar**. Cada proyecto es un **chat**
donde se guardan fotos, notas, documentos, planos, lienzos y notas de voz. Todo funciona
**sin cuentas ni servidores**: los aparatos se sincronizan directamente por la Wi‑Fi de casa.

### La familia PixPin

| Versión | Repositorio | Estado |
|---|---|---|
| 🪟 **Windows** | [PIXPIN_PRO_WINDOWS](https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS) | La versión de referencia (Rust). |
| 🤖 **Android** | [PIXPIN_PRO_ANDROID](https://github.com/1xmanMAX/PIXPIN_PRO_ANDROID) | Captura, anota y fija notas flotantes en el móvil. |
| 🍎 **Mac** | [PIXPIN_PRO_MAC](https://github.com/1xmanMAX/PIXPIN_PRO_MAC) | Port nativo (Rust + Swift). |
| 📱 **iPad** | este repositorio | **En desarrollo**, sobre el mismo núcleo que el Mac. |

Todas usan **el mismo protocolo de sincronización y el mismo formato de carpetas**: un
proyecto, una nota, una tinta o un marcador hecho en un aparato aparece en los demás.

## Esta versión (iPad)

- Núcleo en **Rust** (`apple/nucleo/`): el mismo que el Mac y Windows, compilado como
  `PixPinCoreFFI.xcframework` para el iPad y su simulador y llamado desde Swift con UniFFI.
- App nativa en **SwiftUI + UIKit** (`apple/app/`), iPadOS 18 o posterior: PencilKit/UITouch
  para el Apple Pencil, PDFKit, AVFoundation, Vision, VisionKit, Metal, Bonjour.
- Plan, contratos entre zonas y diferencias con Windows: [`docs/plan-ipad.md`](docs/plan-ipad.md).

## Descargar y probar

Cada cambio compila la app en un Mac de GitHub (`.github/workflows/ipad.yml`), pasa las pruebas
en el simulador del iPad y deja para descargar en **Actions** > la última ejecución verde de
**iPad** > **Artifacts**:

- **PixPin-ipad-sin-firmar**: el `.ipa` para un iPad de verdad. Hay que firmarlo con tu Apple
  ID (gratis, 7 días) con **Sideloadly** o **AltStore**, o compilarlo en tu Mac con Xcode (abajo).
- **PixPin-ipad-simulador**: la app para el simulador de Xcode
  (`xcrun simctl install booted PixPin.app`).

## Compilar en un Mac

```sh
cd apple
herramientas/xcframework.sh          # núcleo de Rust -> PixPinCore (necesita Xcode y rustup)
brew install xcodegen
(cd xcode && xcodegen generate)      # xcode/PixPin.xcodeproj
open xcode/PixPin.xcodeproj          # elegir tu equipo en Signing y ejecutar en tu iPad
herramientas/app.sh pruebas          # o: las pruebas en el simulador
```

## Sincronizar con Windows, Mac y Android

El formato y el protocolo son los mismos: un arreglo en `pixpin-sincro` o `pixpin-proyecto`
se lleva también a los repositorios de Windows y Mac. En el iPad se sincroniza **mientras la
app está abierta** (iOS corta la red en segundo plano) y al volver a abrirla.
