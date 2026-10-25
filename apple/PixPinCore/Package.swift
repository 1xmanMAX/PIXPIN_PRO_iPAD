// swift-tools-version:6.0
//
// PixPinCore: el nucleo de Rust de PixPin (`apple/nucleo/pixpin-nucleo`) para
// la app de Mac, iPad e iPhone.
//
// - `PixPinCoreFFI.xcframework`: la biblioteca estatica de Rust para cada
//   plataforma, con su cabecera C. NO se versiona: la hace
//   `apple/herramientas/xcframework.sh` en un Mac.
// - `Sources/PixPinCore/pixpin_nucleo.swift`: los enlaces que genera UniFFI.
//   Si se versionan (para revisar los cambios de la API en un diff), pero
//   los regenera el mismo script: no se editan a mano.
import PackageDescription

let package = Package(
    name: "PixPinCore",
    platforms: [
        .macOS(.v15),
        .iOS(.v18),
    ],
    products: [
        // Dinamica: la app y sus extensiones (Compartir, Quick Look) cargan
        // UN solo nucleo de `Contents/Frameworks` en vez de llevar cada una
        // su copia (~50 MB por copia).
        .library(name: "PixPinCore", type: .dynamic, targets: ["PixPinCore"]),
    ],
    targets: [
        .binaryTarget(
            name: "PixPinCoreFFI",
            path: "PixPinCoreFFI.xcframework"
        ),
        .target(
            name: "PixPinCore",
            dependencies: ["PixPinCoreFFI"],
            path: "Sources/PixPinCore"
        ),
    ],
    // El codigo que genera UniFFI se escribe para Swift 5 (marca sus tipos
    // `@unchecked Sendable`); en modo Swift 6 estricto daria avisos que no
    // son nuestros.
    swiftLanguageModes: [.v5]
)
