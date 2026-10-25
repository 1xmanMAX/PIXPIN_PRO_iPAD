// Las manos del asistente para las herramientas de documentos (E1,
// `asistente/documentos_acciones.rs`): lo que solo sabe hacer Apple desde
// Swift. PDFKit mide y pinta las hojas de un PDF; Vision lee las palabras
// de una imagen con su sitio (para la tabla de una foto y para buscar y
// anotar en un PDF escaneado); exportar lo hace quien sepa compartir
// (`ManosDelAsistente.exportador`, lo pone la zona de Compartir). Todo se
// llama desde el hilo del turno (nunca el principal).

import Foundation
import ImageIO
import PDFKit
import PixPinCore
import UIKit
import Vision

extension ManosDelAsistente {
    /// **Quien exporta** un archivo a otro formato (pdf, png, docx...)
    /// dejando lo hecho en una carpeta; devuelve las rutas hechas. Lo pone
    /// la parte de la app que sabe compartir; sin el, exportar dice que aun
    /// no se puede en el iPad.
    @MainActor static var exportador: ((_ url: URL, _ formato: String, _ carpeta: URL) async throws -> [URL])?

    func paginasDePdf(ruta: String) -> [MedidaPagina] {
        ManosDelAsistente.medidas(URL(fileURLWithPath: ruta))
    }

    func pintarPagina(ruta: String, pagina: UInt32, ancho: UInt32) -> Data? {
        ManosDelAsistente.pintar(URL(fileURLWithPath: ruta), pagina: Int(pagina), ancho: Int(ancho))
    }

    func palabrasDeImagen(imagen: Data) -> [PalabraLeida] {
        guard let src = CGImageSourceCreateWithData(imagen as CFData, nil),
              let img = CGImageSourceCreateImageAtIndex(src, 0, nil) else { return [] }
        return ManosDelAsistente.palabras(img)
    }

    func exportar(ruta: String, formato: String, carpeta: String) throws -> [String] {
        let url = URL(fileURLWithPath: ruta)
        guard FileManager.default.fileExists(atPath: ruta) else {
            throw ErrorNucleo.NoExiste(que: url.lastPathComponent)
        }
        let r: Result<[String], Error>? = esperar(nil, tope: 120) { responder in
            Task { @MainActor in
                guard let exportador = ManosDelAsistente.exportador else {
                    return responder(.failure(ErrorNucleo.Argumento(detalle: "exportar aún no se puede en el iPad")))
                }
                do {
                    let hechos = try await exportador(url, formato.lowercased(), URL(fileURLWithPath: carpeta))
                    responder(.success(hechos.map(\.path)))
                } catch {
                    responder(.failure(error))
                }
            }
        }
        switch r {
        case let .success(v): return v
        case let .failure(e as ErrorNucleo): throw e
        case let .failure(e): throw ErrorNucleo.Argumento(detalle: "\(e)")
        case .none: throw ErrorNucleo.Argumento(detalle: "exportar tardó demasiado")
        }
    }

    // MARK: Lo puro (y probado)

    /// Lo que mide cada hoja: la caja que se ve, ya girada (como el lector).
    static func medidas(_ url: URL) -> [MedidaPagina] {
        guard let doc = PDFDocument(url: url) else { return [] }
        return (0 ..< doc.pageCount).compactMap { i in
            guard let p = doc.page(at: i) else { return nil }
            let c = p.bounds(for: .cropBox)
            let girada = abs(p.rotation) % 180 == 90
            return MedidaPagina(ancho: Float(girada ? c.height : c.width), alto: Float(girada ? c.width : c.height))
        }
    }

    /// La hoja `pagina` pintada a `ancho` pixeles, en PNG, sobre blanco.
    static func pintar(_ url: URL, pagina: Int, ancho: Int) -> Data? {
        guard let doc = PDFDocument(url: url), let p = doc.page(at: pagina) else { return nil }
        let c = p.bounds(for: .cropBox)
        let girada = abs(p.rotation) % 180 == 90
        let (w, h) = girada ? (c.height, c.width) : (c.width, c.height)
        guard w > 0, h > 0 else { return nil }
        let escala = CGFloat(ancho) / w
        let img = p.thumbnail(of: CGSize(width: w * escala, height: h * escala), for: .cropBox)
        return img.pngData()
    }

    /// **Las palabras de una imagen** con su caja en pixeles (y abajo), en
    /// orden de lectura; cada renglon de Vision, su numero.
    static func palabras(_ img: CGImage) -> [PalabraLeida] {
        let peticion = VNRecognizeTextRequest()
        peticion.recognitionLevel = .accurate
        peticion.recognitionLanguages = ["es-ES", "en-US"]
        peticion.usesLanguageCorrection = true
        let manejador = VNImageRequestHandler(cgImage: img, options: [:])
        guard (try? manejador.perform([peticion])) != nil else { return [] }
        let (w, h) = (CGFloat(img.width), CGFloat(img.height))
        var v: [PalabraLeida] = []
        let lineas = (peticion.results ?? []).sorted {
            // De arriba abajo (Vision mide desde abajo), y de izquierda a derecha.
            abs($0.boundingBox.midY - $1.boundingBox.midY) > 0.01
                ? $0.boundingBox.midY > $1.boundingBox.midY
                : $0.boundingBox.minX < $1.boundingBox.minX
        }
        for (n, linea) in lineas.enumerated() {
            guard let c = linea.topCandidates(1).first else { continue }
            let s = c.string
            var i = s.startIndex
            while i < s.endIndex {
                guard let inicio = s[i...].firstIndex(where: { !$0.isWhitespace }) else { break }
                let fin = s[inicio...].firstIndex(where: \.isWhitespace) ?? s.endIndex
                let caja = (try? c.boundingBox(for: inicio ..< fin))?.boundingBox ?? linea.boundingBox
                v.append(PalabraLeida(texto: String(s[inicio ..< fin]),
                                      x: Float(caja.minX * w), y: Float((1 - caja.maxY) * h),
                                      ancho: Float(caja.width * w), alto: Float(caja.height * h),
                                      renglon: UInt32(n)))
                i = fin
            }
        }
        return v
    }
}
