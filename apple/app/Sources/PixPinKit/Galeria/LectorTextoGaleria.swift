// **Lee el texto de las capturas en segundo plano** con Vision, para que el
// buscador de la galeria encuentre lo que pone dentro (no se ensena: en
// Windows el usuario lo quito de la vista el 5-oct), como
// `lanzar_lector_de_texto` de Windows con su OCR.
//
// De una en una, la mas nueva primero (son las que mas se buscan), con un
// respiro entre una y otra para no quitarle el procesador al equipo. Lo
// leido lo guarda el nucleo (`galeria_poner_texto`, en `cache/galeria/`):
// la segunda vez que se abre la galeria no se relee nada.

import Foundation
import ImageIO
import PixPinCore
import Vision

@MainActor
final class LectorTextoGaleria {
    private let nucleo: Nucleo
    private let alLeer: () -> Void
    private var tarea: Task<Void, Never>?
    private var otraVuelta = false

    init(nucleo: Nucleo, alLeer: @escaping () -> Void) {
        self.nucleo = nucleo
        self.alLeer = alLeer
    }

    /// Hay capturas nuevas (o se acaba de abrir): a leer las que falten.
    func despertar() {
        guard tarea == nil else {
            otraVuelta = true
            return
        }
        tarea = Task { [weak self] in
            await self?.vuelta()
        }
    }

    func parar() {
        tarea?.cancel()
        tarea = nil
    }

    private func vuelta() async {
        let nucleo = self.nucleo
        // Las ya intentadas: si una no se puede apuntar (el disco), no se
        // vuelve a ella en bucle.
        var intentadas: Set<String> = []
        repeat {
            otraVuelta = false
            while !Task.isCancelled {
                let pendientes = await Task.detached(priority: .utility) { nucleo.galeriaSinTexto(cuantos: 8) }.value
                    .filter { !intentadas.contains($0.nombre) }
                if pendientes.isEmpty { break }
                for c in pendientes {
                    if Task.isCancelled { return }
                    intentadas.insert(c.nombre)
                    let url = URL(fileURLWithPath: c.ruta)
                    let texto = await Task.detached(priority: .utility) { LectorTextoGaleria.leer(url) }.value
                    try? await Task.detached(priority: .utility) {
                        try nucleo.galeriaPonerTexto(nombre: c.nombre, texto: texto ?? "")
                    }.value
                    try? await Task.sleep(for: .milliseconds(40))
                }
                alLeer()
            }
        } while otraVuelta && !Task.isCancelled
        tarea = nil
    }

    /// El texto de una imagen, renglon a renglon. `nil` si no se pudo abrir
    /// (se apunta vacio: no se reintenta en cada vuelta).
    nonisolated static func leer(_ url: URL) -> String? {
        guard let fuente = CGImageSourceCreateWithURL(url as CFURL, nil),
              let imagen = CGImageSourceCreateImageAtIndex(fuente, 0, nil) else { return nil }
        let pedido = VNRecognizeTextRequest()
        pedido.recognitionLevel = .accurate
        pedido.usesLanguageCorrection = true
        pedido.recognitionLanguages = ["es-ES", "en-US"]
        let manejador = VNImageRequestHandler(cgImage: imagen, options: [:])
        guard (try? manejador.perform([pedido])) != nil else { return nil }
        return (pedido.results ?? []).compactMap { $0.topCandidates(1).first?.string }.joined(separator: "\n")
    }
}
