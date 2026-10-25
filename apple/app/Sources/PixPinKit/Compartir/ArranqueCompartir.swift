// **Lo que la zona de Compartir pone en marcha al arrancar** (sin tocar
// `Raiz/`):
//
// - `ManosDelAsistente.exportador`: la herramienta «exportar» del asistente
//   (`documentos_acciones.rs` de Windows) hace sus ficheros con lo mismo que
//   la hoja de compartir: una foto o un PDF a PNG, JPG, SVG, PDF o pagina
//   web, y el original tal cual.
//
// Lo llaman la lista de proyectos y el chat al aparecer (y, si la app lo
// apunta, `CompartirDelIPad.activar()` al arrancar). Una vez.

import Foundation
import PixPinCore

@MainActor
public enum CompartirDelIPad {
    private static var activado = false

    public static func activar() {
        guard !activado else { return }
        activado = true
        ManosDelAsistente.exportador = { url, formato, carpeta in
            try await ExportarConLaHoja.exportar(url, formato: formato, en: carpeta)
        }
    }
}

/// **Exportar un fichero a otro formato** con lo de la hoja de compartir.
@MainActor
enum ExportarConLaHoja {
    /// El formato de la hoja que corresponde a lo que pide el asistente
    /// («html», «jpeg», «png»...). `nil` si la hoja no lo sabe hacer.
    nonisolated static func idDeFormato(_ pedido: String) -> String? {
        switch pedido.trimmingCharacters(in: CharacterSet(charactersIn: ". ")).lowercased() {
        case "html", "htm", "web", "pagina", "página": IdFormato.web
        case "pdf": IdFormato.pdf
        case "png": IdFormato.png
        case "jpg", "jpeg": IdFormato.jpg
        case "svg": IdFormato.svg
        case "original": IdFormato.original
        case "txt", "texto": IdFormato.texto
        default: nil
        }
    }

    static func exportar(_ url: URL, formato: String, en carpeta: URL) async throws -> [URL] {
        let nombre = TextosCompartir.sinExtension(url.lastPathComponent)
        let p = PrepararCompartir.deFicheros(titulo: nombre, [url])
        let c = p.compartible()
        guard let id = idDeFormato(formato), let f = c.formatos.first(where: { $0.id == id }) else {
            throw ErrorNucleo.Argumento(detalle: "en el iPad no se puede pasar «\(url.lastPathComponent)» a \(formato)")
        }
        // Una sola pagina en una imagen; todas en un PDF o una pagina web.
        let claves = c.paginas.map(\.clave)
        let elegidas = f.cuantas == .una ? Array(claves.prefix(1)) : claves
        let hechos = try await p.generar(id, claves: elegidas, en: carpeta)
        // Lo que va entero no se copia: se da su ruta, y el asistente lo
        // quiere en su carpeta.
        return try hechos.map { u in
            guard u.deletingLastPathComponent().standardizedFileURL != carpeta.standardizedFileURL else { return u }
            let destino = carpeta.appendingPathComponent(u.lastPathComponent)
            if FileManager.default.fileExists(atPath: destino.path) { try FileManager.default.removeItem(at: destino) }
            try FileManager.default.copyItem(at: u, to: destino)
            return destino
        }
    }
}
