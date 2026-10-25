// **Las conversaciones del asistente**, guardadas en este Mac
// (`<datos>/asistente/conversaciones/<id>.json`): la lista de la izquierda
// de la ventana, para volver a una y seguir. Cada una lleva sus burbujas
// (con lo pensado, las herramientas y lo dibujado), sus adjuntos y su hoja
// de trabajo (lo que el asistente recuerda de ella, `pixpin_ia::hoja_trabajo`).
//
// No viajan a los otros aparatos: el chat «Asistente» que viaja es la parte
// 2 (E1). Escribir es atomico (un `.json` nuevo sustituye al de antes).

import Foundation

/// Una conversacion con el asistente.
public struct Conversacion: Codable, Identifiable, Equatable, Sendable {
    public var id = UUID()
    public var titulo = ""
    public var creada = Date()
    public var cambiada = Date()
    public var burbujas: [ModeloAsistente.Burbuja] = []
    /// La hoja de trabajo (JSON del nucleo), vacia al empezar.
    public var hoja = ""
    /// Todo lo adjuntado en ella (lo que el asistente puede leer o calcar).
    public var adjuntos: [ModeloAsistente.Adjunto] = []

    public init() {}

    /// El titulo que se ensena: el primer pedido, corto.
    public var rotulo: String {
        if !titulo.isEmpty { return titulo }
        let primero = burbujas.first { $0.delUsuario }?.texto ?? ""
        let linea = primero.split(separator: "\n").first.map(String.init) ?? ""
        return linea.isEmpty ? "Conversación nueva" : String(linea.prefix(60))
    }
}

public final class HistorialAsistente: @unchecked Sendable {
    let carpeta: URL

    public init(carpeta: URL = carpetaDeDatos().appendingPathComponent("asistente/conversaciones", isDirectory: true)) {
        self.carpeta = carpeta
    }

    private func fichero(_ id: UUID) -> URL {
        carpeta.appendingPathComponent(id.uuidString + ".json")
    }

    private static let codificador: JSONEncoder = {
        let e = JSONEncoder()
        e.dateEncodingStrategy = .millisecondsSince1970
        return e
    }()

    private static let decodificador: JSONDecoder = {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .millisecondsSince1970
        return d
    }()

    /// Todas, la ultima tocada primero. Las que no se entienden se saltan.
    public func todas() -> [Conversacion] {
        let nombres = (try? FileManager.default.contentsOfDirectory(at: carpeta, includingPropertiesForKeys: nil)) ?? []
        return nombres.filter { $0.pathExtension == "json" }
            .compactMap { try? HistorialAsistente.decodificador.decode(Conversacion.self, from: Data(contentsOf: $0)) }
            .sorted { $0.cambiada > $1.cambiada }
    }

    /// Guarda una (si tiene algo que guardar).
    public func guardar(_ c: Conversacion) {
        guard !c.burbujas.isEmpty else { return }
        do {
            try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
            try HistorialAsistente.codificador.encode(c).write(to: fichero(c.id), options: .atomic)
        } catch {
            // Sin disco no se pierde la conversacion abierta: sigue en memoria.
        }
    }

    public func borrar(_ id: UUID) {
        try? FileManager.default.removeItem(at: fichero(id))
    }
}
