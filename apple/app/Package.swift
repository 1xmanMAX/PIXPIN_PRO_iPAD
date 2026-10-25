// swift-tools-version:6.0
//
// PixPin para iPad: la app nativa (SwiftUI + UIKit) sobre el mismo nucleo de
// Rust que el Mac, Windows y Android (`../PixPinCore`, que hace
// `herramientas/xcframework.sh`).
//
// - `PixPinKit`: toda la app (almacen, sincronizar, chats, lienzo, visores...).
// - El punto de entrada (`Sources/PixPin`, `@main`) lo compila el proyecto de
//   Xcode (`apple/xcode/project.yml`), que monta la `.app` del iPad.
//
// Muchas piezas vienen tal cual de la version de Mac (PIXPIN_PRO_MAC,
// `apple/app/Sources/PixPinKit`): las que no tocan AppKit. Lo de AppKit se
// escribe aqui con UIKit/SwiftUI para el tactil y el Apple Pencil.
import PackageDescription

let package = Package(
    name: "PixPinIPad",
    platforms: [.iOS(.v18)],
    products: [
        .library(name: "PixPinKit", targets: ["PixPinKit"]),
    ],
    dependencies: [
        .package(path: "../PixPinCore"),
    ],
    targets: [
        .target(
            name: "PixPinKit",
            dependencies: [.product(name: "PixPinCore", package: "PixPinCore")],
            path: "Sources/PixPinKit"
        ),
    ],
    // Swift 5, como el Mac: los enlaces de UniFFI son de Swift 5.
    swiftLanguageModes: [.v5]
)
