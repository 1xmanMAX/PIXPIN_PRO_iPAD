// **Abrir varios planos a la vez** (Windows y el Mac: cada plano en su
// ventana, `Planos/VentanasDePlano.swift`): desde el visor de un plano,
// «Abrir otro plano…» elige uno o varios en Archivos y cada uno se abre en
// su propia ventana (Stage Manager, `VentanasDeVisor`). Sin varias
// ventanas, el primero sustituye al de ahora.
//
// Lo que llega de Archivos es de fuera de la app (con permiso solo
// mientras se lee): se copia a `Application Support/Planos abiertos/`, con
// su nombre, para que la ventana se pueda volver a abrir al volver a la app
// y lo anotado siga con el (la capa va por la ruta del plano).

import Foundation
import UniformTypeIdentifiers

@MainActor
enum OtroPlanoIPad {
    /// Planos y modelos que se dejan elegir.
    static let tipos: [UTType] = {
        let ext = ["dwg", "dxf", "dwf", "dgn", "ifc", "ifczip", "rvt", "rfa", "landxml", "las", "laz", "e57",
                   "ply", "obj", "stl", "gltf", "glb"]
        let t = ext.compactMap { UTType(filenameExtension: $0) }
        return t.isEmpty ? [.data] : t
    }()

    /// La carpeta de las copias.
    static func carpeta() -> URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        return base.appendingPathComponent("Planos abiertos", isDirectory: true)
    }

    /// Donde va la copia de `nombre` (el mismo nombre: la misma copia, con
    /// su tinta, la siguiente vez).
    nonisolated static func destino(de nombre: String, en carpeta: URL) -> URL {
        let limpio = nombre.replacingOccurrences(of: "/", with: "_").trimmingCharacters(in: .whitespaces)
        return carpeta.appendingPathComponent(limpio.isEmpty ? "plano" : limpio)
    }

    /// Copia y abre cada uno en su ventana.
    static func abrir(_ urls: [URL]) {
        let carpeta = carpeta()
        _ = Task<Void, Never> {
            let copias = await Task.detached(priority: .userInitiated) { () -> [URL] in
                try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
                return urls.compactMap { origen in
                    let permiso = origen.startAccessingSecurityScopedResource()
                    defer { if permiso { origen.stopAccessingSecurityScopedResource() } }
                    let destino = OtroPlanoIPad.destino(de: origen.lastPathComponent, en: carpeta)
                    if origen.standardizedFileURL == destino.standardizedFileURL { return destino }
                    try? FileManager.default.removeItem(at: destino)
                    return (try? FileManager.default.copyItem(at: origen, to: destino)) != nil ? destino : nil
                }
            }.value
            for (i, url) in copias.enumerated() {
                let visor: Visor = TipoDeFichero(url) == .modelo3d
                    ? .modelo3d(url: url, titulo: url.lastPathComponent)
                    : .plano(url: url, titulo: url.lastPathComponent)
                if VentanasDeVisor.sePuede {
                    VentanasDeVisor.abrir(visor)
                } else if i == 0 {
                    Navegacion.compartida.abrir(visor)
                }
            }
        }
    }
}
