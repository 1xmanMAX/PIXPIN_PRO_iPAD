// PixPin para iPad: una ventana con la barra lateral de secciones (lo que en
// Windows abre la bandeja) sobre el mismo nucleo y la misma sincronizacion
// que Windows, Mac y Android. Con Stage Manager se pueden abrir varias.

import PixPinKit
import SwiftUI

@main
struct PixPinApp: App {
    @StateObject private var entorno = EntornoApp()

    var body: some Scene {
        WindowGroup {
            RaizIPad(entorno: entorno)
        }
    }
}
