// **Las acciones rapidas del icono** (mantener pulsado el icono de PixPin
// en la pantalla de inicio o el Dock): la Jump List de Windows
// (`lista_del_icono.rs`). Son las de `UIApplicationShortcutItems` en
// `xcode/Soporte/App-Info.plist`: Capturar, Nueva nota, Buscar y
// Sincronizar.
//
// iOS las da al delegado de la app (con la app cerrada, al conectar la
// escena) o al de la escena (con la app abierta). La app de SwiftUI pone
// `DelegadoAppIPad` con `@UIApplicationDelegateAdaptor` y este devuelve
// `DelegadoEscenaIPad` como delegado de cada escena; SwiftUI sigue llevando
// las ventanas.
//
// Y lo que el sistema pone en marcha al arrancar: `SistemaDelIPad.activar()`
// (el buzon del App Group, los widgets).

import Foundation
import UIKit

/// Lo que esta carpeta pone en marcha al arrancar (lo llama la app antes de
/// crear el entorno, como las demas zonas).
@MainActor
public enum SistemaDelIPad {
    private static var activado = false

    public static func activar() {
        guard !activado else { return }
        activado = true
        Arranque.apuntar { entorno in
            guard let almacen = entorno.almacen else { return }
            RecogedorCompartido.compartido.empezar(almacen)
            DatosDeWidgetsIPad.compartido.empezar(almacen)
        }
    }
}

/// Las acciones rapidas.
@MainActor
public enum AccionesRapidas {
    public enum Accion: String, CaseIterable, Sendable {
        case capturar = "io.github.1xmanmax.pixpin.capturar"
        case nota = "io.github.1xmanmax.pixpin.nota"
        case buscar = "io.github.1xmanmax.pixpin.buscar"
        case sincronizar = "io.github.1xmanmax.pixpin.sincronizar"

        /// El pedido de Windows que hace lo mismo (`nil`: capturar, que en
        /// el iPad abre la hoja de capturar).
        public var pedido: Pedido? {
            switch self {
            case .capturar: nil
            case .nota: Pedido("nota_nueva")
            case .buscar: Pedido("ventana", ["cual": .texto("buscar")])
            case .sincronizar: Pedido("sincronizar")
            }
        }
    }

    /// Hace la accion de tipo `tipo`. `false` si no es nuestra.
    @discardableResult
    public static func hacer(_ tipo: String) -> Bool {
        guard let accion = Accion(rawValue: tipo) else { return false }
        // Con la app recien abierta, cuando ya este todo (y la ventana).
        Arranque.apuntar { _ in
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(500))
                if let p = accion.pedido {
                    HacerPedidos.compartidos.hacer(p)
                } else {
                    Navegacion.compartida.cerrarVisor()
                    CapturadorIPad.compartido.empezar()
                }
            }
        }
        return true
    }
}

/// El delegado de la app (solo para las acciones rapidas y el delegado de
/// las escenas). Lo pone la app con `@UIApplicationDelegateAdaptor`.
public final class DelegadoAppIPad: NSObject, UIApplicationDelegate {
    public func application(_ application: UIApplication,
                            configurationForConnecting sesion: UISceneSession,
                            options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        // La app estaba cerrada y se abrio con una accion rapida.
        if let item = options.shortcutItem {
            let tipo = item.type
            MainActor.assumeIsolated { _ = AccionesRapidas.hacer(tipo) }
        }
        // La pantalla externa (proyector) la monta la vista del orador por su
        // cuenta: sin el delegado de las ventanas.
        if sesion.role == .windowExternalDisplayNonInteractive {
            return UISceneConfiguration(name: "Pantalla externa", sessionRole: sesion.role)
        }
        let c = UISceneConfiguration(name: sesion.configuration.name, sessionRole: sesion.role)
        c.delegateClass = DelegadoEscenaIPad.self
        return c
    }
}

/// El delegado de cada escena: la accion rapida con la app ya abierta.
public final class DelegadoEscenaIPad: NSObject, UIWindowSceneDelegate {
    public func windowScene(_ windowScene: UIWindowScene,
                            performActionFor shortcutItem: UIApplicationShortcutItem,
                            completionHandler: @escaping (Bool) -> Void) {
        let tipo = shortcutItem.type
        let hecho = MainActor.assumeIsolated { AccionesRapidas.hacer(tipo) }
        completionHandler(hecho)
    }
}
