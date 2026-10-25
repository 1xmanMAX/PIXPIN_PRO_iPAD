// Las cuentas puras de la captura del iPad (las del Mac, `GeometriaCaptura`
// y `SelectorProyecto`): el nombre de una captura en la galeria, el del
// fichero que va al chat y como se ordenan y filtran los proyectos de «Al
// chat». Sin pantalla, para las pruebas (`CapturaIPadTests`).

import Foundation

enum NombresCaptura {
    /// Las extensiones que ensena la galeria (las de Windows).
    static let extensionesDeGaleria = ["png", "jpg", "jpeg", "bmp", "gif", "webp", "mp4"]

    /// `captura-AAAAMMDD-HHMMSS` en la hora local, el mismo nombre que pone
    /// Windows (`galeria_capturas::nombre_por_hora`).
    static func nombreDeCaptura(fecha: Date, zona: TimeZone = .current) -> String {
        var calendario = Calendar(identifier: .gregorian)
        calendario.timeZone = zona
        let c = calendario.dateComponents([.year, .month, .day, .hour, .minute, .second], from: fecha)
        return String(format: "captura-%04d%02d%02d-%02d%02d%02d",
                      c.year ?? 0, c.month ?? 0, c.day ?? 0, c.hour ?? 0, c.minute ?? 0, c.second ?? 0)
    }

    /// La ruta de una captura nueva en `carpeta`: el nombre por la hora y
    /// `-2`, `-3`... si en ese segundo ya hay otra (con cualquier extension
    /// de la galeria).
    static func rutaNueva(en carpeta: URL, fecha: Date, zona: TimeZone = .current,
                          existe: (URL) -> Bool = { FileManager.default.fileExists(atPath: $0.path) }) -> URL {
        let base = nombreDeCaptura(fecha: fecha, zona: zona)
        func ocupado(_ nombre: String) -> Bool {
            extensionesDeGaleria.contains { existe(carpeta.appendingPathComponent("\(nombre).\($0)")) }
        }
        var nombre = base
        var n = 2
        while ocupado(nombre) {
            nombre = "\(base)-\(n)"
            n += 1
        }
        return carpeta.appendingPathComponent("\(nombre).png")
    }
}

/// Ordenar y filtrar los proyectos de «Al chat» (`captura2/al_chat.rs` de
/// Windows, igual que el Mac).
enum ListaDeProyectos {
    static let clave = "captura.ultimoChat"
    /// Cuantos tienen numero (1-6).
    static let conTecla = 6

    /// El ultimo usado primero y lo demas en su orden. Uno que ya no existe
    /// no cambia nada.
    static func ordenar<T>(_ v: [T], id: (T) -> String, ultimo: String?) -> [T] {
        guard let ultimo, let i = v.firstIndex(where: { id($0) == ultimo }) else { return v }
        var o = v
        let p = o.remove(at: i)
        o.insert(p, at: 0)
        return o
    }

    /// Los que contienen `texto` en el nombre, sin mirar mayusculas ni
    /// tildes. Vacio: todos.
    static func filtrar<T>(_ v: [T], nombre: (T) -> String, texto: String) -> [T] {
        let t = texto.trimmingCharacters(in: .whitespaces)
        guard !t.isEmpty else { return v }
        return v.filter { nombre($0).range(of: t, options: [.caseInsensitive, .diacriticInsensitive]) != nil }
    }

    /// El nombre del fichero de la captura en el chat: `captura-<ms>.png`.
    static func nombreDeFichero(fecha: Date) -> String {
        "captura-\(Int64((fecha.timeIntervalSince1970 * 1000).rounded(.down))).png"
    }
}
