// Lo que comparten la galeria, el timeline, las tareas y buscar: los ids de
// sus ventanas, el huso de quien mira, la vista que espera al almacen y el
// vigia de una carpeta (sin sondear: el sistema avisa, §4bis.6 del plan).

import Foundation
import SwiftUI

/// Los ids de las ventanas de la app (`Window(..., id:)` en `PixPinApp`).
public enum IdVentana {
    public static let proyectos = "proyectos"
    public static let galeria = "galeria"
    public static let timeline = "timeline"
    public static let tareas = "tareas"
    public static let buscar = "buscar"
}

/// Lo que hay que sumar a UTC para tener la hora de aqui, en ms: el
/// `desfase_ms` que piden el timeline, la galeria y las tareas para contar
/// los dias de calendario como los cuenta quien mira.
public func desfaseLocalMs(_ fecha: Date = Date()) -> Int64 {
    Int64(TimeZone.current.secondsFromGMT(for: fecha)) * 1000
}

/// Milisegundos desde 1970 a `Date`.
func fechaDe(ms: Int64) -> Date {
    Date(timeIntervalSince1970: TimeInterval(ms) / 1000)
}

/// Una ventana que necesita el almacen: si no se pudo abrir la carpeta de
/// datos, lo dice en vez de quedarse en blanco.
public struct ConAlmacen<Contenido: View>: View {
    let almacen: Almacen?
    let error: String?
    let contenido: (Almacen) -> Contenido

    public init(_ almacen: Almacen?, error: String?, @ViewBuilder contenido: @escaping (Almacen) -> Contenido) {
        self.almacen = almacen
        self.error = error
        self.contenido = contenido
    }

    public var body: some View {
        if let almacen {
            contenido(almacen)
        } else {
            ContentUnavailableView("No se pudo abrir la carpeta de datos",
                                   systemImage: "exclamationmark.triangle",
                                   description: Text(error ?? ""))
        }
    }
}

/// Vigila una carpeta con un `DispatchSource` del sistema (kqueue): avisa
/// cuando entra, sale o cambia de nombre algo dentro, sin temporizadores.
/// Los avisos seguidos se juntan en uno (`espera`). Se deja de vigilar al
/// soltarlo.
final class VigiaCarpeta {
    private let fuente: DispatchSourceFileSystemObject
    private var pendiente: DispatchWorkItem?

    init?(_ url: URL, espera: TimeInterval = 0.3, alCambiar: @escaping () -> Void) {
        let fd = open(url.path, O_EVTONLY)
        guard fd >= 0 else { return nil }
        let fuente = DispatchSource.makeFileSystemObjectSource(fileDescriptor: fd,
                                                               eventMask: [.write, .delete, .rename],
                                                               queue: .main)
        self.fuente = fuente
        fuente.setEventHandler { [weak self] in
            guard let self else { return }
            self.pendiente?.cancel()
            let trabajo = DispatchWorkItem(block: alCambiar)
            self.pendiente = trabajo
            DispatchQueue.main.asyncAfter(deadline: .now() + espera, execute: trabajo)
        }
        fuente.setCancelHandler { close(fd) }
        fuente.resume()
    }

    deinit {
        pendiente?.cancel()
        fuente.cancel()
    }
}
