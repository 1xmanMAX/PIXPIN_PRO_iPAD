// Lo que ensenan los widgets de PixPin (`xcode/Widgets/`): las ultimas
// capturas de la galeria y las tareas de hoy. Es la alternativa del iPad al
// centro de la bandeja de Windows (`centro_bandeja/`: ultimas capturas,
// favoritos).
//
// Los widgets no ven la carpeta de datos (ni llevan el nucleo): la app les
// deja `widgets.json` y unas miniaturas en el buzon del App Group
// (`BuzonCompartido`) al abrirse, al volver a primer plano, al irse al
// fondo y con calma tras cada cambio, y le dice a WidgetKit que relea.

import Foundation
import ImageIO
import PixPinCore
import UIKit
import UniformTypeIdentifiers
import WidgetKit

@MainActor
public final class DatosDeWidgetsIPad {
    public static let compartido = DatosDeWidgetsIPad()

    /// Cuantas capturas y tareas se dejan (las que caben en el grande).
    nonisolated static let capturas = 6
    nonisolated static let tareas = 8
    /// El lado de las miniaturas (px).
    nonisolated static let ladoMiniatura = 360

    private weak var almacen: Almacen?
    private var observadores: [NSObjectProtocol] = []
    private var tarea: Task<Void, Never>?

    init() {}

    func empezar(_ almacen: Almacen) {
        guard self.almacen == nil else { return }
        self.almacen = almacen
        let nombres: [Notification.Name] = [
            .pixpinCambioLocal, .pixpinTrajoCambios, .pixpinTareasCambiadas,
            UIApplication.didBecomeActiveNotification, UIApplication.didEnterBackgroundNotification,
        ]
        for n in nombres {
            observadores.append(NotificationCenter.default.addObserver(forName: n, object: nil, queue: .main) { [weak self] aviso in
                let ya = aviso.name == UIApplication.didEnterBackgroundNotification
                MainActor.assumeIsolated { self?.escribir(conCalma: !ya) }
            })
        }
        escribir(conCalma: true)
    }

    /// Escribe lo de los widgets (con calma: tres segundos despues del
    /// ultimo cambio).
    func escribir(conCalma: Bool) {
        guard let almacen, let raiz = BuzonCompartido.raiz() else { return }
        let nucleo = almacen.nucleo
        tarea?.cancel()
        tarea = Task {
            if conCalma {
                try? await Task.sleep(for: .seconds(3))
                guard !Task.isCancelled else { return }
            }
            let cambio = await Task.detached(priority: .utility) {
                DatosDeWidgetsIPad.escribir(nucleo: nucleo, en: raiz)
            }.value
            if cambio { WidgetCenter.shared.reloadAllTimelines() }
        }
    }

    /// Lee del nucleo, escribe `widgets.json` y las miniaturas. `true` si
    /// cambio algo (bloqueante).
    nonisolated static func escribir(nucleo: Nucleo, en raiz: URL) -> Bool {
        let ahora = Date()
        let desfase = desfaseLocalMs(ahora)
        let ahoraMs = Int64(ahora.timeIntervalSince1970 * 1000)
        let fotos = ((try? nucleo.galeria(desde: 0, cuantos: 24, dias: 0, desfaseMs: desfase))?.capturas ?? [])
            .filter { !$0.esVideo }
            .prefix(capturas)
        let carpeta = DatosDeWidgets.carpetaMiniaturas(en: raiz)
        try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        var hechas: [DatosDeWidgets.Captura] = []
        for c in fotos {
            let nombre = nombreDeMiniatura(c)
            let destino = carpeta.appendingPathComponent(nombre)
            if !FileManager.default.fileExists(atPath: destino.path) {
                guard miniatura(de: URL(fileURLWithPath: c.ruta), a: destino) else { continue }
            }
            hechas.append(.init(miniatura: nombre, nombre: c.nombre, cuando: c.cuando))
        }
        // Las miniaturas que ya no se ensenan, fuera.
        let quedan = Set(hechas.map(\.miniatura))
        for n in (try? FileManager.default.contentsOfDirectory(atPath: carpeta.path)) ?? [] where !quedan.contains(n) {
            try? FileManager.default.removeItem(at: carpeta.appendingPathComponent(n))
        }
        let listas = (try? nucleo.tareas(desfaseMs: desfase)) ?? []
        let hoy = tareasDeHoy(listas, ahoraMs: ahoraMs, desfaseMs: desfase)
        let anterior = BuzonCompartido.leerWidgets(en: raiz)
        if let anterior, anterior.capturas == hechas, anterior.tareasDeHoy == hoy { return false }
        let datos = DatosDeWidgets(escrita: ahoraMs, capturas: hechas, tareasDeHoy: hoy)
        return (try? BuzonCompartido.escribirWidgets(datos, en: raiz)) != nil
    }

    /// El nombre de la miniatura de una captura: cambia si cambia la foto.
    nonisolated static func nombreDeMiniatura(_ c: CapturaGaleria) -> String {
        var h: UInt64 = 1469598103934665603
        for b in "\(c.nombre)|\(c.cuando)|\(c.bytes)".utf8 {
            h = (h ^ UInt64(b)) &* 1099511628211
        }
        return String(h, radix: 16) + ".jpg"
    }

    /// Una miniatura JPEG de `origen` (sin cargar la foto entera).
    nonisolated static func miniatura(de origen: URL, a destino: URL) -> Bool {
        guard let fuente = CGImageSourceCreateWithURL(origen as CFURL, nil) else { return false }
        let opciones: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: ladoMiniatura,
        ]
        guard let foto = CGImageSourceCreateThumbnailAtIndex(fuente, 0, opciones as CFDictionary),
              let salida = CGImageDestinationCreateWithURL(destino as CFURL, UTType.jpeg.identifier as CFString, 1, nil) else {
            return false
        }
        CGImageDestinationAddImage(salida, foto, [kCGImageDestinationLossyCompressionQuality: 0.8] as CFDictionary)
        return CGImageDestinationFinalize(salida)
    }

    /// **Las tareas de hoy**: lo pendiente que se recuerda hoy (por su hora)
    /// y lo apuntado hoy, lo que tiene hora primero.
    nonisolated static func tareasDeHoy(_ listas: [ListaTareas], ahoraMs: Int64, desfaseMs: Int64) -> [DatosDeWidgets.Tarea] {
        let dia: Int64 = 86_400_000
        let local = ahoraMs + desfaseMs
        let empieza = local - ((local % dia) + dia) % dia
        var con: [(Int64, DatosDeWidgets.Tarea)] = []
        var sin: [DatosDeWidgets.Tarea] = []
        for l in listas {
            let proyecto = l.guardados ? "Mensajes guardados" : l.nombreChat
            for t in l.tareas where !t.hecha {
                let fila = DatosDeWidgets.Tarea(titulo: t.texto, proyecto: proyecto, hecha: false)
                if let r = t.recuerda, r >= empieza, r < empieza + dia {
                    con.append((r, fila))
                } else if t.dias == 0 {
                    sin.append(fila)
                }
            }
        }
        let ordenadas = con.sorted { $0.0 < $1.0 }.map(\.1) + sin
        return Array(ordenadas.prefix(tareas))
    }
}
