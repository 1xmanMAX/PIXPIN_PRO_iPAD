// **Las manos del asistente en la app**: el `AppIa` que pide el nucleo de
// Rust (`asistente/turno.rs`). Rust lo llama desde el hilo del turno (nunca
// el principal) y espera la respuesta. Es el del Mac sin AppKit:
//
// - `confirmar`: lo que borra o saca algo del iPad pregunta antes, en la
//   burbuja con «Sí» y «No» (como Windows desde el 13-oct). Sin respuesta
//   (parado, ventana cerrada, 10 min) es un no.
// - `pedido`: una accion de la app (`{"pixpin":1,"accion":...}`, el
//   protocolo de `docs/protocolo-pedidos.md`). Apuntar y marcar tareas se
//   hace aqui con el nucleo (con su resultado); lo demas va a
//   `HacerPedidos`, el mismo camino que `pixpinmax://` y Atajos.
//   `PedidosDelAsistente.hacer` lo cambia todo (las pruebas).
// - `leerTexto`: el texto de un PDF (su capa de texto con PDFKit; las
//   paginas escaneadas, con Vision) o de una foto (Vision).

import Foundation
import ImageIO
import PDFKit
import PixPinCore
import UIKit

/// **Otro que haga los pedidos del asistente** (las pruebas); devuelve
/// `nil` si se hizo o el motivo si no. Sin el, las tareas se hacen aqui y
/// lo demas en `HacerPedidos`.
@MainActor
public enum PedidosDelAsistente {
    public static var hacer: ((String) async -> String?)?
}

/// Lo que llega de otro hilo, con su semaforo.
final class CajaDeEspera<T>: @unchecked Sendable {
    var valor: T?
    let s = DispatchSemaphore(value: 0)
}

final class ManosDelAsistente: AppIa, @unchecked Sendable {
    /// Pregunta en la burbuja y llama con la respuesta (en el principal).
    let preguntar: @MainActor (String, @escaping @MainActor (Bool) -> Void) -> Void
    let nucleo: Nucleo?
    let corte: CorteIa
    /// Cuanto se espera un «Sí»/«No» como mucho.
    var caduca: TimeInterval = 600

    init(nucleo: Nucleo?, corte: CorteIa,
         preguntar: @escaping @MainActor (String, @escaping @MainActor (Bool) -> Void) -> Void) {
        self.nucleo = nucleo
        self.corte = corte
        self.preguntar = preguntar
    }

    /// Espera en este hilo (el del turno) a lo que haga `trabajo` en el
    /// principal; se rinde si se pulsa «Parar» o pasa `tope`.
    func esperar<T: Sendable>(_ porDefecto: T, tope: TimeInterval,
                              _ trabajo: @escaping @MainActor (@escaping @Sendable (T) -> Void) -> Void) -> T {
        let caja = CajaDeEspera<T>()
        DispatchQueue.main.async {
            MainActor.assumeIsolated {
                trabajo { v in
                    caja.valor = v
                    caja.s.signal()
                }
            }
        }
        let limite = Date().addingTimeInterval(tope)
        while caja.s.wait(timeout: .now() + 0.1) == .timedOut {
            if corte.cortado() || Date() > limite { return porDefecto }
        }
        return caja.valor ?? porDefecto
    }

    func confirmar(pregunta: String) -> Bool {
        let preguntar = self.preguntar
        return esperar(false, tope: caduca) { responder in
            preguntar(pregunta) { si in responder(si) }
        }
    }

    func pedido(json: String) -> String? {
        let nucleo = self.nucleo
        return esperar(Optional("PixPin no contestó"), tope: 120) { responder in
            Task { @MainActor in
                if let hacer = PedidosDelAsistente.hacer {
                    responder(await hacer(json))
                } else if ManosDelAsistente.esDeTareas(json) {
                    responder(await ManosDelAsistente.hacerAqui(json, nucleo: nucleo))
                } else {
                    responder(ManosDelAsistente.aLaApp(json))
                }
            }
        }
    }

    func leerTexto(ruta: String, paginas: [RangoPaginas]) throws -> String {
        let url = URL(fileURLWithPath: ruta)
        guard FileManager.default.fileExists(atPath: ruta) else {
            throw ErrorNucleo.NoExiste(que: url.lastPathComponent)
        }
        if url.pathExtension.lowercased() == "pdf" {
            return try ManosDelAsistente.textoDePdf(url, paginas: paginas)
        }
        guard let img = ManosDelAsistente.imagen(url) else {
            throw ErrorNucleo.Argumento(detalle: "\(url.lastPathComponent) no es un PDF ni una imagen que se pueda leer")
        }
        return ManosDelAsistente.ocr(img)
    }

    // MARK: Leer

    /// Una imagen del disco, ya girada (ImageIO, sin UIKit en este hilo).
    static func imagen(_ url: URL, lado: Int = 4096) -> CGImage? {
        let opciones: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: lado,
        ]
        guard let src = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
        return CGImageSourceCreateThumbnailAtIndex(src, 0, opciones as CFDictionary)
    }

    /// Letras por pagina a partir de las cuales vale la capa de texto del PDF
    /// (menos suele ser un sello sobre una pagina escaneada: Windows).
    static let textoMinimo = 40

    static func textoDePdf(_ url: URL, paginas: [RangoPaginas]) throws -> String {
        guard let doc = PDFDocument(url: url) else {
            throw ErrorNucleo.Argumento(detalle: "no se pudo abrir \(url.lastPathComponent) (¿cifrado?)")
        }
        let total = doc.pageCount
        var indices: [Int] = []
        if paginas.isEmpty {
            indices = Array(0 ..< total)
        } else {
            for r in paginas {
                let desde = max(1, Int(r.desde)), hasta = min(total, Int(r.hasta))
                if desde <= hasta { indices += (desde - 1) ... (hasta - 1) }
            }
        }
        var partes: [String] = []
        for i in indices.prefix(60) {
            guard let p = doc.page(at: i) else { continue }
            var t = p.string ?? ""
            if t.trimmingCharacters(in: .whitespacesAndNewlines).count < textoMinimo {
                // Escaneada: la pagina como imagen, con Vision.
                let caja = p.bounds(for: .mediaBox)
                let escala = 2000 / max(caja.width, caja.height, 1)
                let img = p.thumbnail(of: CGSize(width: caja.width * escala, height: caja.height * escala), for: .mediaBox)
                if let cg = img.cgImage {
                    t = ocr(cg)
                }
            }
            partes.append("[página \(i + 1)]\n" + t)
        }
        return partes.joined(separator: "\n\n")
    }

    /// Vision, esperando aqui (el hilo del turno).
    static func ocr(_ img: CGImage) -> String {
        final class Caja: @unchecked Sendable { var texto = "" }
        let caja = Caja()
        let s = DispatchSemaphore(value: 0)
        Task.detached(priority: .utility) {
            caja.texto = (try? await sacarTexto(de: img)) ?? ""
            s.signal()
        }
        s.wait()
        return caja.texto
    }

    // MARK: Pedidos

    /// Las tareas que se hacen aqui mismo (con su resultado de verdad).
    static func esDeTareas(_ json: String) -> Bool {
        guard let datos = json.data(using: .utf8),
              let p = (try? JSONSerialization.jsonObject(with: datos)) as? [String: Any]
        else { return false }
        switch p["accion"] as? String {
        case "anadir_tarea": return p["proyecto"] == nil && p["codigo"] == nil
        case "marcar_tarea": return true
        default: return false
        }
    }

    /// Lo demas, a quien atiende `pixpinmax://` y Atajos (`HacerPedidos`),
    /// como el plugin de Windows: aceptado ya; si algo sale mal lo dice su
    /// aviso.
    @MainActor
    static func aLaApp(_ json: String) -> String? {
        do {
            HacerPedidos.compartidos.hacer(try Pedido.de(json: json))
            return nil
        } catch {
            return "PixPin no entendió el pedido: \(error.localizedDescription)"
        }
    }

    // MARK: Pedidos sin quien los atienda

    /// Los pedidos de tareas, hechos aqui con el nucleo (lo demas lo dice).
    @MainActor
    static func hacerAqui(_ json: String, nucleo: Nucleo?) async -> String? {
        guard let datos = json.data(using: .utf8),
              let p = (try? JSONSerialization.jsonObject(with: datos)) as? [String: Any],
              let accion = p["accion"] as? String
        else { return "PixPin no entendió el pedido" }
        guard let nucleo else { return "aquí no se puede hacer «\(accion)»" }
        let desfase = Int64(TimeZone.current.secondsFromGMT()) * 1000
        let r: String? = await Task.detached(priority: .userInitiated) { () -> String? in
            do {
                switch accion {
                case "anadir_tarea" where p["proyecto"] == nil && p["codigo"] == nil:
                    guard let texto = p["texto"] as? String else { return "falta el texto de la tarea" }
                    _ = try nucleo.apuntarTarea(texto: texto, desfaseMs: desfase)
                    return nil
                case "marcar_tarea":
                    guard let codigo = p["codigo"] as? String, let indice = p["indice"] as? Int else {
                        return "falta la lista o la tarea"
                    }
                    let hecha = p["hecha"] as? Bool ?? true
                    guard let lista = try nucleo.tareas(desfaseMs: desfase).first(where: { $0.mensaje == codigo }),
                          let t = lista.tareas.first(where: { Int($0.indice) == indice })
                    else { return "esa tarea ya no está" }
                    return try nucleo.marcarTarea(chat: lista.chat, mensaje: codigo, indice: t.indice,
                                                  crudo: t.crudo, hecha: hecha) ? nil : "la lista cambió; vuelve a pedirlo"
                default:
                    return "PixPin para iPad aún no hace «\(accion)» desde el asistente"
                }
            } catch {
                return ModeloAsistente.texto(de: error)
            }
        }.value
        if r == nil { NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil) }
        return r
    }
}
