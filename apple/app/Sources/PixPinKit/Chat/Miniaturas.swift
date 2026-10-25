// Las miniaturas de las fotos del chat y de los lienzos.
//
// Rendimiento (§4bis.2 del plan):
// - NUNCA se decodifica la foto entera para la lista: ImageIO saca una
//   miniatura del tamano pedido (`CGImageSourceCreateThumbnailAtIndex`),
//   que en JPEG y HEIC decodifica ya reducido;
// - se hace fuera del hilo principal (`Task.detached`, `.utility`) y la
//   miniatura sale ya decodificada (`kCGImageSourceShouldCacheImmediately`),
//   asi pintarla no cuesta nada en el hilo de la interfaz;
// - van a una `NSCache` con tope en BYTES (`totalCostLimit`), que el sistema
//   tambien vacia por su cuenta, y se vacia entera con el aviso de memoria;
// - dos burbujas que piden la misma foto a la vez esperan la misma lectura.

import CoreGraphics
import Foundation
import ImageIO
import SwiftUI

public final class Miniaturas: @unchecked Sendable {
    /// La de toda la app: las burbujas y los lienzos del chat comparten
    /// fotos (la misma foto en el chat y dentro de un dibujo es una lectura).
    public static let compartidas = Miniaturas()

    /// Tope por omision: 64 MB de pixeles, unas ochenta fotos de burbuja
    /// (520 px de lado) ya decodificadas.
    public static let topePorOmision = 64 << 20

    private final class Caja {
        let imagen: CGImage
        init(_ imagen: CGImage) { self.imagen = imagen }
    }

    private let cache = NSCache<NSString, Caja>()
    private let cerrojo = NSLock()
    private var enCurso: [String: Task<CGImage?, Never>] = [:]
    private var vigiaMemoria: DispatchSourceMemoryPressure?

    public init(topeBytes: Int = Miniaturas.topePorOmision) {
        cache.totalCostLimit = topeBytes
        cache.name = "PixPin.miniaturas"
        let fuente = DispatchSource.makeMemoryPressureSource(eventMask: [.warning, .critical],
                                                             queue: .global(qos: .utility))
        fuente.setEventHandler { [weak self] in self?.vaciar() }
        fuente.resume()
        vigiaMemoria = fuente
    }

    deinit { vigiaMemoria?.cancel() }

    private static func clave(_ url: URL, _ lado: Int) -> String {
        "\(lado)|\(url.path)"
    }

    private func conCerrojo<R>(_ hacer: () -> R) -> R {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        return hacer()
    }

    /// La miniatura si ya esta hecha (para pintar sin esperar). No lee nada.
    public func enCache(_ url: URL, lado: Int) -> CGImage? {
        cache.object(forKey: Miniaturas.clave(url, lado) as NSString)?.imagen
    }

    /// La miniatura de `url` con su lado mayor de como mucho `lado` pixeles
    /// (una foto mas pequena sale con su tamano: no se agranda). La lee
    /// fuera del hilo principal si no esta en la cache. `nil` si el fichero
    /// no esta o no es una imagen.
    public func cargar(_ url: URL, lado: Int) async -> CGImage? {
        let clave = Miniaturas.clave(url, lado)
        if let hecha = cache.object(forKey: clave as NSString) { return hecha.imagen }
        let tarea: Task<CGImage?, Never> = conCerrojo {
            if let t = enCurso[clave] { return t }
            let t = Task.detached(priority: .utility) { [weak self] () -> CGImage? in
                let imagen = Miniaturas.leer(url, lado: lado)
                self?.terminar(clave, imagen)
                return imagen
            }
            enCurso[clave] = t
            return t
        }
        return await tarea.value
    }

    private func terminar(_ clave: String, _ imagen: CGImage?) {
        if let imagen {
            cache.setObject(Caja(imagen), forKey: clave as NSString, cost: imagen.bytesPerRow * imagen.height)
        }
        conCerrojo { enCurso[clave] = nil }
    }

    /// Suelta todas las miniaturas (aviso de memoria).
    public func vaciar() {
        cache.removeAllObjects()
    }

    /// Lee la miniatura AHORA, en el hilo que llama (las pruebas y quien ya
    /// esta fuera del principal). Sin cache de ImageIO para la foto entera.
    public static func leer(_ url: URL, lado: Int) -> CGImage? {
        let deFuente = [kCGImageSourceShouldCache: false] as CFDictionary
        guard lado > 0, let fuente = CGImageSourceCreateWithURL(url as CFURL, deFuente),
              CGImageSourceGetCount(fuente) > 0 else { return nil }
        let opciones = [
            // Siempre desde la foto (reducida al leer): la miniatura que
            // traiga dentro puede ser de otro tamano o estar mal girada.
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            // Con la orientacion EXIF aplicada: la foto del movil, derecha.
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: lado,
            // Decodificada ya aqui (fuera del principal), no al pintarla.
            kCGImageSourceShouldCacheImmediately: true,
        ] as [CFString: Any] as CFDictionary
        return CGImageSourceCreateThumbnailAtIndex(fuente, 0, opciones)
    }
}

/// Una foto del chat por su miniatura: un hueco mientras llega, la foto
/// cuando esta. Pide `2 x` el tamano en pantalla (pantallas Retina).
struct MiniaturaVista: View {
    let url: URL
    /// El lado mayor en pantalla, en puntos.
    let lado: CGFloat
    var miniaturas: Miniaturas = .compartidas
    @State private var imagen: CGImage?
    @State private var falta = false

    private var pixeles: Int { Int(lado * 2) }

    var body: some View {
        Group {
            if let imagen = imagen {
                // A su tamano (a 2x) y como mucho `lado`: una foto pequena
                // no se agranda hasta llenar la burbuja.
                let ancho = CGFloat(imagen.width) / 2, alto = CGFloat(imagen.height) / 2
                let k = min(1, lado / max(ancho, alto, 1))
                Image(decorative: imagen, scale: 2)
                    .resizable()
                    .interpolation(.high)
                    .frame(width: ancho * k, height: alto * k)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            } else {
                RoundedRectangle(cornerRadius: 8)
                    .fill(.quaternary)
                    .frame(width: lado * 0.75, height: lado * 0.5)
                    .overlay {
                        if falta {
                            Image(systemName: "photo.badge.exclamationmark").foregroundStyle(.secondary)
                        } else {
                            ProgressView().controlSize(.small)
                        }
                    }
            }
        }
        .task(id: url) {
            if let hecha = miniaturas.enCache(url, lado: pixeles) {
                imagen = hecha
                return
            }
            let leida = await miniaturas.cargar(url, lado: pixeles)
            imagen = leida
            falta = leida == nil
        }
    }
}
