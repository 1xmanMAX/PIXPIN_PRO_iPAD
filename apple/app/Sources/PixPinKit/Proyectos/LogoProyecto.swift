// **El logo de un proyecto**: una imagen en lugar del circulo con iniciales
// (`ventana_chat/logo.rs` de Windows). Como alli, es un `logo.png` dentro de
// la carpeta del proyecto: el cuadrado central de la foto elegida, reducido
// a 256 de lado. No viaja (el movil no tiene logos de imagen): es de este
// aparato, igual que en el PC.
//
// La carpeta la dice el nucleo (`rutaAbsoluta` del proyecto), no se adivina.
// Leer y escribir van fuera del hilo principal; lo leido se guarda aqui
// (son pocos y pequenos).

import ImageIO
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

@MainActor
final class LogosProyecto: ObservableObject {
    static let compartidos = LogosProyecto()

    /// El nombre del fichero (`NOMBRE` de Windows).
    nonisolated static let nombre = "logo.png"
    /// El lado con el que se guarda (`LADO` de Windows).
    nonisolated static let lado = 256

    /// Lo leido de cada proyecto: la imagen, o `nil` si no tiene.
    @Published private(set) var leidos: [String: UIImage?] = [:]
    private var leyendo: Set<String> = []

    func logo(_ id: String) -> UIImage? { leidos[id] ?? nil }

    func tiene(_ id: String) -> Bool { logo(id) != nil }

    /// Lo lee si aun no se leyo.
    func mirar(_ id: String, nucleo: Nucleo) {
        guard leidos[id] == nil, !leyendo.contains(id) else { return }
        leyendo.insert(id)
        Task {
            let img = await Task.detached(priority: .utility) { () -> UIImage? in
                guard let url = LogosProyecto.ruta(id, nucleo: nucleo),
                      FileManager.default.fileExists(atPath: url.path) else { return nil }
                return UIImage(contentsOfFile: url.path)
            }.value
            leyendo.remove(id)
            leidos[id] = .some(img)
        }
    }

    /// **Pone de logo** la imagen `datos`: su cuadrado central, reducido.
    /// `false` si no se pudo (no es una imagen, o no se pudo escribir).
    func poner(_ id: String, datos: Data, nucleo: Nucleo) async -> Bool {
        let hecho = await Task.detached(priority: .userInitiated) { () -> UIImage? in
            guard let png = LogosProyecto.cuadrado(datos, lado: LogosProyecto.lado),
                  let url = LogosProyecto.ruta(id, nucleo: nucleo) else { return nil }
            do {
                try FileManager.default.createDirectory(at: url.deletingLastPathComponent(),
                                                        withIntermediateDirectories: true)
                // Entero o nada: un logo a medio escribir se quedaria roto.
                try png.write(to: url, options: .atomic)
            } catch {
                return nil
            }
            return UIImage(data: png)
        }.value
        guard let hecho else { return false }
        leidos[id] = .some(hecho)
        return true
    }

    /// Quita el logo: vuelve el circulo con iniciales.
    func quitar(_ id: String, nucleo: Nucleo) async {
        await Task.detached(priority: .userInitiated) {
            if let url = LogosProyecto.ruta(id, nucleo: nucleo) { try? FileManager.default.removeItem(at: url) }
        }.value
        leidos[id] = .some(nil)
    }

    /// Donde vive el logo de un proyecto.
    nonisolated static func ruta(_ id: String, nucleo: Nucleo) -> URL? {
        (try? nucleo.rutaAbsoluta(chat: id, rutaRelativa: nombre)).map { URL(fileURLWithPath: $0) }
    }

    /// El cuadrado central de una imagen (ya girada), reducido a `lado`, en
    /// PNG. `nil` si no es una imagen.
    nonisolated static func cuadrado(_ datos: Data, lado: Int) -> Data? {
        guard let fuente = CGImageSourceCreateWithData(datos as CFData, nil) else { return nil }
        let opciones: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: lado * 4,
        ]
        guard let img = CGImageSourceCreateThumbnailAtIndex(fuente, 0, opciones as CFDictionary) else { return nil }
        let corto = min(img.width, img.height)
        guard corto > 0 else { return nil }
        let recorte = recorteCentral(ancho: img.width, alto: img.height)
        guard let central = img.cropping(to: recorte),
              let ctx = CGContext(data: nil, width: lado, height: lado, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        ctx.interpolationQuality = .high
        ctx.draw(central, in: CGRect(x: 0, y: 0, width: lado, height: lado))
        guard let hecha = ctx.makeImage() else { return nil }
        let salida = NSMutableData()
        guard let d = CGImageDestinationCreateWithData(salida, UTType.png.identifier as CFString, 1, nil) else {
            return nil
        }
        CGImageDestinationAddImage(d, hecha, nil)
        return CGImageDestinationFinalize(d) ? salida as Data : nil
    }

    /// El cuadrado del centro de una imagen de `ancho` x `alto`.
    nonisolated static func recorteCentral(ancho: Int, alto: Int) -> CGRect {
        let corto = min(ancho, alto)
        return CGRect(x: (ancho - corto) / 2, y: (alto - corto) / 2, width: corto, height: corto)
    }
}

/// El avatar de un proyecto: su foto del chat (imagen, emoji o color, la
/// que viaja: `Chat/FotoDelChat.swift`); si no tiene, el logo viejo de este
/// aparato; si no, el circulo con sus iniciales.
struct AvatarProyecto: View {
    let chat: FichaChat
    var logo: UIImage?
    var lado: CGFloat = 46

    var body: some View {
        ZStack {
            if chat.foto == nil, let logo {
                Image(uiImage: logo).resizable().scaledToFill()
            } else {
                CirculoDelChat(id: chat.id, nombre: chat.nombre, lado: lado, foto: chat.foto, letra: lado * 0.39)
            }
        }
        .frame(width: lado, height: lado)
        .clipShape(Circle())
    }
}
