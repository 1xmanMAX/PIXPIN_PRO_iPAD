// Lo que entra al chat soltando, pegando o desde la camara, y como se
// resuelve el fichero de un mensaje.
//
// En el iPad lo que se suelta (desde Archivos, Fotos, Safari u otra app en
// Split View) llega como `NSItemProvider` con la representacion del
// fichero, no con su ruta: se pide una copia (`loadFileRepresentation`), que
// el sistema borra al volver, asi que se copia antes a una carpeta temporal
// propia y de ahi al proyecto (`Almacen.adjuntar`, fuera del hilo
// principal). Lo mismo con «Pegar» y ⌘V.

import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// El lado mayor de una foto o un dibujo en su burbuja, en puntos.
let ladoDeBurbuja: CGFloat = 260

/// El fichero de un mensaje en este iPad, o `nil` (una sola vez por
/// burbuja: el almacen lo memoriza).
@MainActor
func resolverFichero(_ m: MensajeChat, _ chat: String, _ almacen: Almacen) async -> URL? {
    guard let ruta = m.ruta, !ruta.isEmpty else { return nil }
    if let hecha = almacen.urlResuelta(deRuta: ruta, en: chat) { return hecha }
    return await almacen.url(deRuta: ruta, en: chat)
}

// MARK: - Soltar y pegar en el chat

/// Lo que entra al chat al soltar o pegar: ficheros (se copian) e imagenes
/// sueltas (de Safari, de Fotos, del portapapeles).
@MainActor
enum EntradaDeFicheros {
    /// Los tipos que acepta el chat al soltar y al pegar.
    nonisolated static let tipos: [UTType] = [.fileURL, .image, .movie, .audio, .pdf, .data]

    /// Si en el portapapeles hay algo que no es texto y se adjunta.
    static func hayAdjuntosParaPegar() -> Bool {
        let pb = UIPasteboard.general
        if pb.hasStrings { return false }
        return pb.hasImages || (pb.hasURLs && (pb.urls ?? []).contains(where: \.isFileURL))
    }

    /// ⌘V con una foto (o ficheros) en el portapapeles: van al chat. Con
    /// texto, `false` (lo pega la caja de escribir, como siempre).
    static func pegar(en chat: String, almacen: Almacen) -> Bool {
        guard hayAdjuntosParaPegar() else { return false }
        let pb = UIPasteboard.general
        if let ficheros = pb.urls?.filter(\.isFileURL), !ficheros.isEmpty {
            Task { await almacen.adjuntar(urls: ficheros, en: chat) }
            return true
        }
        // Una imagen: los bytes tal cual (PNG, JPEG...) si los hay.
        for (tipo, ext) in [(UTType.png, "png"), (UTType.jpeg, "jpg"), (UTType.heic, "heic"), (UTType.gif, "gif")] {
            if let datos = pb.data(forPasteboardType: tipo.identifier) {
                Task { await almacen.adjuntar(datos: datos, formato: ext, en: chat) }
                return true
            }
        }
        if let imagen = pb.image, let png = imagen.pngData() {
            Task { await almacen.adjuntar(datos: png, formato: "png", en: chat) }
            return true
        }
        return false
    }

    /// Mete en el chat lo soltado (o pegado con el boton del sistema). Leer
    /// los proveedores es asincrono y adjuntar va fuera del hilo principal.
    /// Devuelve cuantos entraron.
    @discardableResult
    static func soltar(_ proveedores: [NSItemProvider], en chat: String, almacen: Almacen) async -> Int {
        let carpeta = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-soltado-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: carpeta) }
        var urls: [URL] = []
        for p in proveedores {
            if let u = await copia(de: p, en: carpeta) { urls.append(u) }
        }
        guard !urls.isEmpty else { return 0 }
        return await almacen.adjuntar(urls: urls, en: chat).count
    }

    /// El tipo con el que se pide el fichero: el primero que no sea texto
    /// suelto (un texto arrastrado no es un adjunto).
    nonisolated static func tipoDeFichero(_ registrados: [String]) -> UTType? {
        let tipos = registrados.compactMap { UTType($0) }
        if let f = tipos.first(where: { !$0.conforms(to: .text) || $0.conforms(to: .pdf) || $0 == .fileURL }) {
            return f == .fileURL ? nil : f
        }
        return nil
    }

    /// El nombre de lo soltado: el que trae, con su extension; si no trae,
    /// `pegado-<hora>.<ext>` como en Windows.
    nonisolated static func nombre(sugerido: String?, tipo: UTType) -> String {
        let ext = tipo.preferredFilenameExtension ?? "dat"
        let cuando = Int64(Date().timeIntervalSince1970 * 1000)
        guard let s = sugerido?.trimmingCharacters(in: .whitespacesAndNewlines), !s.isEmpty else {
            return "pegado-\(cuando).\(ext)"
        }
        let limpio = s.replacingOccurrences(of: "/", with: "-").replacingOccurrences(of: ":", with: "-")
        return (limpio as NSString).pathExtension.isEmpty ? "\(limpio).\(ext)" : limpio
    }

    /// Una copia de lo que trae `p` dentro de `carpeta`, o `nil`.
    private nonisolated static func copia(de p: NSItemProvider, en carpeta: URL) async -> URL? {
        // Un fichero de verdad (desde Archivos con su ruta): su copia.
        if p.registeredTypeIdentifiers.contains(UTType.fileURL.identifier),
           let u = await urlDeFichero(p), u.isFileURL {
            let destino = carpeta.appendingPathComponent(u.lastPathComponent)
            let accedido = u.startAccessingSecurityScopedResource()
            defer { if accedido { u.stopAccessingSecurityScopedResource() } }
            if (try? FileManager.default.copyItem(at: u, to: destino)) != nil { return destino }
        }
        guard let tipo = tipoDeFichero(p.registeredTypeIdentifiers) else { return nil }
        let nombre = nombre(sugerido: p.suggestedName, tipo: tipo)
        let destino = carpeta.appendingPathComponent(nombre)
        return await withCheckedContinuation { (c: CheckedContinuation<URL?, Never>) in
            _ = p.loadFileRepresentation(forTypeIdentifier: tipo.identifier) { temporal, _ in
                // El temporal se borra al volver: se copia aqui mismo.
                guard let temporal,
                      (try? FileManager.default.copyItem(at: temporal, to: destino)) != nil else {
                    c.resume(returning: nil)
                    return
                }
                c.resume(returning: destino)
            }
        }
    }

    private nonisolated static func urlDeFichero(_ p: NSItemProvider) async -> URL? {
        await withCheckedContinuation { (c: CheckedContinuation<URL?, Never>) in
            _ = p.loadObject(ofClass: URL.self) { url, _ in c.resume(returning: url) }
        }
    }
}

// MARK: - La camara

/// La camara del iPad (`UIImagePickerController`): la foto hecha va al chat.
struct CamaraChat: UIViewControllerRepresentable {
    let alHacer: (UIImage) -> Void
    /// Al acabar (con foto o sin ella): quien la abrio la cierra.
    let alAcabar: () -> Void

    func makeCoordinator() -> Coordinador { Coordinador(self) }

    func makeUIViewController(context: Context) -> UIImagePickerController {
        let c = UIImagePickerController()
        c.sourceType = .camera
        c.delegate = context.coordinator
        return c
    }

    func updateUIViewController(_ c: UIImagePickerController, context: Context) {
        context.coordinator.padre = self
    }

    @MainActor
    final class Coordinador: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate {
        var padre: CamaraChat
        init(_ padre: CamaraChat) { self.padre = padre }

        func imagePickerController(_ picker: UIImagePickerController,
                                   didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]) {
            if let foto = info[.originalImage] as? UIImage { padre.alHacer(foto) }
            padre.alAcabar()
        }

        func imagePickerControllerDidCancel(_ picker: UIImagePickerController) {
            padre.alAcabar()
        }
    }
}

// MARK: - Compartir

/// Lo que se comparte con la hoja del sistema: ficheros y textos.
struct CosasACompartir: Identifiable {
    let id = UUID()
    var cosas: [Any]
}

/// La hoja de compartir del sistema (`UIActivityViewController`).
struct ActividadCompartir: UIViewControllerRepresentable {
    let cosas: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: cosas, applicationActivities: nil)
    }

    func updateUIViewController(_ c: UIActivityViewController, context: Context) {}
}
