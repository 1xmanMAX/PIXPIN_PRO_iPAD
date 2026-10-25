// PixPin para iPad: una ventana con la barra lateral de secciones (lo que en
// Windows abre la bandeja) sobre el mismo nucleo y la misma sincronizacion
// que Windows, Mac y Android. Con Stage Manager se pueden abrir varias.

import PixPinKit
import SwiftUI

@main
struct PixPinApp: App {
    @StateObject private var entorno = EntornoApp()
    /// La fase de TODA la app (todas las ventanas juntas): con Stage Manager
    /// una ventana puede irse al fondo mientras otra sigue delante, y la
    /// sincronizacion solo se para cuando se va la app entera.
    @Environment(\.scenePhase) private var fase

    init() {
        // Antes de crear el entorno: lo que cada zona pone en marcha al abrir.
        ArranqueOrganizar.apuntar()
        Tema.aplicarAlArrancar()
        SincroDeFondo.activar()
        LienzoDelIPad.activar()
    }

    var body: some Scene {
        WindowGroup {
            RaizIPad(entorno: entorno)
        }
        .onChange(of: fase, initial: true) { _, nueva in
            switch nueva {
            case .active: entorno.alActivarse()
            case .background: entorno.alIrseAlFondo()
            default: break
            }
        }
        .backgroundTask(.appRefresh(SincroDeFondo.identificador)) {
            await SincroDeFondo.alDespertar()
        }
    }
}
