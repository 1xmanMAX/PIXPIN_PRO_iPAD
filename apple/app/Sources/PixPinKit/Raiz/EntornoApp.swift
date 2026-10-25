// El estado de toda la app del iPad: el nucleo abierto sobre la carpeta de
// datos (`Almacen`) y la sincronizacion con los demas aparatos del grupo.
//
// En el iPad, iOS cierra las conexiones y el anuncio de Bonjour al pasar a
// segundo plano (§4.3 del plan): se sincroniza **mientras la app esta
// abierta**, como en Android, y al volver a primer plano se arranca otra vez
// y se hace una vuelta con quien este. Al irse a segundo plano se pide un
// rato (`beginBackgroundTask`) para acabar la vuelta que hubiera a medias.

import Foundation
import PixPinCore
import SwiftUI
import UIKit

@MainActor
public final class EntornoApp: ObservableObject {
    public let almacen: Almacen?
    public let sincro: Sincronizador?
    public let error: String?

    private var tareaDeFondo: UIBackgroundTaskIdentifier = .invalid

    public init() {
        var almacen: Almacen?
        var error: String?
        do {
            almacen = try Almacen()
        } catch let e {
            error = "\(e)"
        }
        self.almacen = almacen
        self.error = error
        sincro = almacen.map { Sincronizador(nucleo: $0.nucleo) }
        if let almacen, let sincro {
            sincro.alTraerCambios = { [weak almacen] in
                Task { await almacen?.cargarChats() }
                NotificationCenter.default.post(name: .pixpinTrajoCambios, object: nil)
            }
        }
        Arranque.alAbrir(self)
    }

    /// La app pasa a primer plano: escuchar, anunciarse y buscar a los del
    /// grupo (el `Sincronizador` hace una vuelta al ver a cada uno).
    public func alActivarse() {
        if tareaDeFondo != .invalid {
            UIApplication.shared.endBackgroundTask(tareaDeFondo)
            tareaDeFondo = .invalid
        }
        sincro?.arrancar()
    }

    /// La app se va a segundo plano: se pide un rato para acabar lo que se
    /// este sincronizando y luego se para todo (iOS lo pararia igual).
    public func alIrseAlFondo() {
        guard sincro != nil, tareaDeFondo == .invalid else { return }
        tareaDeFondo = UIApplication.shared.beginBackgroundTask(withName: "PixPin: acabar de sincronizar") { [weak self] in
            MainActor.assumeIsolated { self?.acabarElFondo() }
        }
        Task { [weak self] in
            // Lo que este a medias se acaba (como mucho lo que iOS deje).
            while let s = self?.sincro, s.ocupado {
                try? await Task.sleep(for: .milliseconds(500))
            }
            self?.acabarElFondo()
        }
    }

    private func acabarElFondo() {
        guard tareaDeFondo != .invalid else { return }
        sincro?.parar()
        UIApplication.shared.endBackgroundTask(tareaDeFondo)
        tareaDeFondo = .invalid
    }
}

/// Lo que cada parte de la app pone en marcha al arrancar (recordatorios,
/// avisos, pines que vuelven, automatizaciones...). Cada parte se apunta con
/// `Arranque.apuntar` desde su propio fichero, sin tocar este.
@MainActor
public enum Arranque {
    private static var pasos: [(EntornoApp) -> Void] = []
    private static var hecho: EntornoApp?

    /// Apunta algo que hacer con el entorno ya abierto. Si la app ya
    /// arranco, se hace en el momento.
    public static func apuntar(_ paso: @escaping (EntornoApp) -> Void) {
        if let hecho { paso(hecho) } else { pasos.append(paso) }
    }

    static func alAbrir(_ entorno: EntornoApp) {
        hecho = entorno
        let todos = pasos
        pasos.removeAll()
        todos.forEach { $0(entorno) }
    }
}

public extension Notification.Name {
    /// Una vuelta de sincronizar trajo cambios de otro aparato.
    static let pixpinTrajoCambios = Notification.Name("pixpin.trajoCambios")
}
