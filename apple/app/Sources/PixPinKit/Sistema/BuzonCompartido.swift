// El buzon que comparten la app y sus extensiones (Compartir y los
// widgets): una carpeta en el contenedor del App Group
// `group.io.github.1xmanmax.pixpin`.
//
// La app guarda sus datos en Application Support de SU caja de arena
// (`carpetaDeDatos()`), que las extensiones no ven. Por eso no se mueve la
// carpeta de datos: la app deja en el buzon lo que las extensiones necesitan
// leer (la lista de proyectos, lo de los widgets) y la extension de
// Compartir deja en el lo recibido (`entrantes/<id>/`); la app lo recoge al
// abrirse (`Sistema/RecogedorCompartido.swift`) y lo mete en el proyecto
// por el nucleo, como si se hubiera soltado en el chat.
//
//     PixPinCompartido/
//       proyectos.json            la lista de proyectos (la escribe la app)
//       widgets.json              lo que ensenan los widgets (la app)
//       entrantes/<ms>-<uuid>/
//         ficheros/...            lo compartido, tal cual
//         entrega.json            se escribe lo ULTIMO: sin el, la entrega
//                                 esta a medias y no se recoge
//
// Este fichero es solo Foundation: lo compilan tambien las extensiones
// (`xcode/project.yml`), que no llevan PixPinKit ni el nucleo.

import Foundation

public enum BuzonCompartido {
    /// El App Group de la app y sus extensiones (entitlements de los tres).
    public static let grupo = "group.io.github.1xmanmax.pixpin"
    /// El aviso de Darwin que manda la extension al dejar una entrega (por
    /// si la app esta delante, en Split View o Slide Over).
    public static let avisoDeEntrega = "io.github.1xmanmax.pixpin.entrega"
    /// Una entrega sin `entrega.json` mas vieja que esto se dio por perdida
    /// (la extension se cerro a medias) y se borra.
    public static let caducidadDeLoAMedias: TimeInterval = 24 * 3600

    static let carpeta = "PixPinCompartido"
    static let ficheroProyectos = "proyectos.json"
    static let ficheroWidgets = "widgets.json"
    static let carpetaEntrantes = "entrantes"
    static let ficheroEntrega = "entrega.json"
    static let ficheroFallida = "entrega.fallida.json"
    static let carpetaFicheros = "ficheros"

    /// La carpeta del buzon, o `nil` si este binario no tiene el App Group
    /// (compilado sin firmar, como en la CI: entonces no hay buzon y la
    /// extension lo dice).
    public static func raiz() -> URL? {
        #if canImport(Darwin)
        guard let base = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: grupo) else {
            return nil
        }
        return base.appendingPathComponent(carpeta, isDirectory: true)
        #else
        return nil
        #endif
    }

    // MARK: La lista de proyectos

    /// La escribe la app; la lee la extension de Compartir.
    public static func escribirProyectos(_ proyectos: [ProyectoCompartido], en raiz: URL, ahora: Date = Date()) throws {
        let lista = ListaCompartida(escrita: Int64(ahora.timeIntervalSince1970 * 1000), proyectos: proyectos)
        try FileManager.default.createDirectory(at: raiz, withIntermediateDirectories: true)
        let datos = try JSONEncoder().encode(lista)
        try datos.write(to: raiz.appendingPathComponent(ficheroProyectos), options: .atomic)
    }

    /// Los proyectos que dejo la app (vacio si nunca se abrio).
    public static func leerProyectos(en raiz: URL) -> [ProyectoCompartido] {
        guard let datos = try? Data(contentsOf: raiz.appendingPathComponent(ficheroProyectos)),
              let lista = try? JSONDecoder().decode(ListaCompartida.self, from: datos) else { return [] }
        return lista.proyectos
    }

    /// Como se ven en la extension: «Mensajes guardados» primero y luego
    /// el ultimo tocado arriba (la lista de proyectos de Windows); con
    /// `buscando`, solo los que lo llevan en el nombre (sin mirar
    /// mayusculas ni tildes).
    public static func ordenados(_ proyectos: [ProyectoCompartido], buscando: String = "") -> [ProyectoCompartido] {
        let q = buscando.trimmingCharacters(in: .whitespacesAndNewlines)
        let filtrados = q.isEmpty ? proyectos : proyectos.filter {
            $0.nombre.range(of: q, options: [.caseInsensitive, .diacriticInsensitive]) != nil
        }
        return filtrados.sorted { a, b in
            if a.esGuardados != b.esGuardados { return a.esGuardados }
            if a.tocado != b.tocado { return a.tocado > b.tocado }
            return a.nombre.localizedStandardCompare(b.nombre) == .orderedAscending
        }
    }

    // MARK: Lo de los widgets

    public static func escribirWidgets(_ w: DatosDeWidgets, en raiz: URL) throws {
        try FileManager.default.createDirectory(at: raiz, withIntermediateDirectories: true)
        try JSONEncoder().encode(w).write(to: raiz.appendingPathComponent(ficheroWidgets), options: .atomic)
    }

    public static func leerWidgets(en raiz: URL) -> DatosDeWidgets? {
        guard let datos = try? Data(contentsOf: raiz.appendingPathComponent(ficheroWidgets)) else { return nil }
        return try? JSONDecoder().decode(DatosDeWidgets.self, from: datos)
    }

    // MARK: Las entregas

    /// Empieza una entrega nueva (su carpeta, aun sin `entrega.json`).
    public static func nuevaEntrega(en raiz: URL, ahora: Date = Date()) throws -> CarpetaDeEntrega {
        let ms = Int64(ahora.timeIntervalSince1970 * 1000)
        let carpeta = raiz.appendingPathComponent(carpetaEntrantes, isDirectory: true)
            .appendingPathComponent("\(ms)-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: carpeta.appendingPathComponent(carpetaFicheros, isDirectory: true),
                                                withIntermediateDirectories: true)
        return CarpetaDeEntrega(url: carpeta)
    }

    /// Las entregas acabadas, la mas vieja primero (el orden en que se
    /// compartieron). De paso borra las que quedaron a medias hace mucho.
    public static func entregasPendientes(en raiz: URL, ahora: Date = Date()) -> [(carpeta: URL, entrega: EntregaCompartida)] {
        let fm = FileManager.default
        let dir = raiz.appendingPathComponent(carpetaEntrantes, isDirectory: true)
        guard let nombres = try? fm.contentsOfDirectory(atPath: dir.path) else { return [] }
        var hechas: [(URL, EntregaCompartida)] = []
        for nombre in nombres.sorted() where !nombre.hasPrefix(".") {
            let carpeta = dir.appendingPathComponent(nombre, isDirectory: true)
            let fichero = carpeta.appendingPathComponent(ficheroEntrega)
            if let datos = try? Data(contentsOf: fichero) {
                if let e = try? JSONDecoder().decode(EntregaCompartida.self, from: datos) {
                    hechas.append((carpeta, e))
                } else {
                    // Rota: no se reintenta cada vez.
                    try? fm.moveItem(at: fichero, to: carpeta.appendingPathComponent(ficheroFallida))
                }
                continue
            }
            if fm.fileExists(atPath: carpeta.appendingPathComponent(ficheroFallida).path) { continue }
            let atributos = try? fm.attributesOfItem(atPath: carpeta.path)
            let creada = (atributos?[.creationDate] as? Date) ?? (atributos?[.modificationDate] as? Date)
            if let creada, ahora.timeIntervalSince(creada) > caducidadDeLoAMedias {
                try? fm.removeItem(at: carpeta)
            }
        }
        return hechas
    }

    /// Una entrega ya metida en su proyecto: fuera.
    public static func quitar(_ carpeta: URL) {
        try? FileManager.default.removeItem(at: carpeta)
    }

    /// Una entrega que no se pudo meter: se aparta (no se reintenta a cada
    /// vuelta) y lo compartido no se borra.
    public static func apartar(_ carpeta: URL) {
        try? FileManager.default.moveItem(at: carpeta.appendingPathComponent(ficheroEntrega),
                                          to: carpeta.appendingPathComponent(ficheroFallida))
    }

    /// Las entregas apartadas (para decirlo y poder reintentarlas a mano).
    public static func cuantasApartadas(en raiz: URL) -> Int {
        let dir = raiz.appendingPathComponent(carpetaEntrantes, isDirectory: true)
        let nombres = (try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? []
        return nombres.filter {
            FileManager.default.fileExists(atPath: dir.appendingPathComponent($0).appendingPathComponent(ficheroFallida).path)
        }.count
    }

    /// Las apartadas vuelven a la cola (el boton «Reintentar»).
    public static func reintentarApartadas(en raiz: URL) {
        let dir = raiz.appendingPathComponent(carpetaEntrantes, isDirectory: true)
        let nombres = (try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? []
        for n in nombres {
            let c = dir.appendingPathComponent(n, isDirectory: true)
            try? FileManager.default.moveItem(at: c.appendingPathComponent(ficheroFallida),
                                              to: c.appendingPathComponent(ficheroEntrega))
        }
    }

    // MARK: Nombres y texto

    /// Un nombre de fichero que se puede escribir (sin barras ni dos
    /// puntos, sin empezar por punto) y que no choca con `usados`:
    /// `foto.png`, `foto (2).png`...
    public static func nombreLibre(_ nombre: String, usados: Set<String>) -> String {
        var limpio = nombre.map { "/\\:\u{0}".contains($0) ? "-" : String($0) }.joined()
            .trimmingCharacters(in: .whitespacesAndNewlines)
        while limpio.hasPrefix(".") { limpio.removeFirst() }
        if limpio.isEmpty { limpio = "archivo" }
        if limpio.count > 120 {
            let ext = (limpio as NSString).pathExtension
            let base = String((limpio as NSString).deletingPathExtension.prefix(100))
            limpio = ext.isEmpty ? base : base + "." + ext
        }
        let minusculas = Set(usados.map { $0.lowercased() })
        guard minusculas.contains(limpio.lowercased()) else { return limpio }
        let ext = (limpio as NSString).pathExtension
        let base = (limpio as NSString).deletingPathExtension
        var n = 2
        while true {
            let otro = ext.isEmpty ? "\(base) (\(n))" : "\(base) (\(n)).\(ext)"
            if !minusculas.contains(otro.lowercased()) { return otro }
            n += 1
        }
    }

    /// La nota que acompana a lo compartido: el comentario y luego los
    /// textos y enlaces, uno por linea, sin repetir (Safari manda el titulo
    /// de la pagina como texto y como comentario). `nil` si no hay nada.
    public static func textoDeLaNota(_ e: EntregaCompartida) -> String? {
        var vistos = Set<String>()
        var lineas: [String] = []
        for t in [e.comentario] + e.textos {
            let limpio = t.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !limpio.isEmpty, vistos.insert(limpio).inserted else { continue }
            lineas.append(limpio)
        }
        return lineas.isEmpty ? nil : lineas.joined(separator: "\n")
    }
}

/// Un proyecto en la lista que ve la extension.
public struct ProyectoCompartido: Codable, Hashable, Identifiable, Sendable {
    public var id: String
    public var nombre: String
    /// Cuando se toco por ultima vez (ms), para ordenar.
    public var tocado: Int64
    public var esGuardados: Bool

    public init(id: String, nombre: String, tocado: Int64, esGuardados: Bool) {
        self.id = id
        self.nombre = nombre
        self.tocado = tocado
        self.esGuardados = esGuardados
    }
}

struct ListaCompartida: Codable {
    var version = 1
    var escrita: Int64
    var proyectos: [ProyectoCompartido]
}

/// Lo que se compartio y a donde va (`entrega.json`).
public struct EntregaCompartida: Codable, Hashable, Sendable {
    public var version = 1
    /// El id del proyecto (chat) elegido.
    public var chat: String
    /// Su nombre al compartir (por si ya no existe: se dice cual era).
    public var nombreDelChat: String
    /// Lo que se escribio en la caja «Comentario».
    public var comentario: String
    /// Los textos y enlaces compartidos (van a la nota).
    public var textos: [String]
    /// Los ficheros, por su nombre dentro de `ficheros/`, en orden.
    public var ficheros: [String]
    /// Cuando (ms).
    public var creada: Int64
    /// Un enlace `pixpinmax://` que hacer en vez de meter nada (la
    /// extension del navegador: «Nueva tarea» con lo seleccionado...). Las
    /// entregas de antes no lo llevan.
    public var enlace: String?

    public init(chat: String, nombreDelChat: String, comentario: String, textos: [String], ficheros: [String], creada: Int64,
                enlace: String? = nil) {
        self.chat = chat
        self.nombreDelChat = nombreDelChat
        self.comentario = comentario
        self.textos = textos
        self.ficheros = ficheros
        self.creada = creada
        self.enlace = enlace
    }
}

// MARK: - La extension del navegador

/// **Lo que pide la extension de Safari** (`extensiones/navegador` de
/// Windows: las mismas acciones que su menu), traducido a un enlace
/// `pixpinmax://` de la casa (el mismo verbo que la linea de ordenes), que
/// la app hace al recoger la entrega. Puro: lo compilan la app y la
/// extension.
public enum PedidoDelNavegador {
    /// Lo mas largo que se manda de texto (el enlace tiene un tope de 16 KB).
    public static let topeTexto = 6000

    /// El enlace de `accion` con lo de la pagina; `nil` si la accion no
    /// existe o le falta lo que necesita (texto seleccionado). Las acciones
    /// que llevan una imagen (`captura_pagina`) no van por enlace.
    public static func enlace(accion: String, texto: String?, url: String?, titulo: String?,
                              pregunta: String? = nil) -> String? {
        let t = recortar(texto)
        let conTexto = ["tarea", "nota", "chat", "leccion", "leer", "asistente"]
        if conTexto.contains(accion), t.isEmpty { return nil }
        switch accion {
        case "tarea": return armar("tarea", ["texto": t])
        case "nota": return armar("nota", ["texto": t])
        case "chat": return armar("chat", ["texto": t])
        case "leccion": return armar("leccion", ["texto": t])
        case "leer": return armar("leer", ["texto": t, "titulo": recortar(titulo)])
        case "asistente":
            let p = recortar(pregunta)
            return armar("asistente", ["texto": p.isEmpty ? t : p + "\n\n«" + t + "»"])
        case "enlace_al_chat":
            let u = recortar(url)
            guard !u.isEmpty else { return nil }
            let n = recortar(titulo)
            return armar("chat", ["texto": n.isEmpty ? u : n + "\n" + u])
        default:
            return nil
        }
    }

    /// Las acciones que entiende (las de `nucleo.js`).
    public static let acciones = ["tarea", "nota", "chat", "leccion", "leer", "asistente", "enlace_al_chat",
                                  "captura_pagina"]

    private static func recortar(_ t: String?) -> String {
        let limpio = (t ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        return limpio.count > topeTexto ? String(limpio.prefix(topeTexto)) + "…" : limpio
    }

    /// `pixpinmax://verbo?campo=valor` con todo lo que no es letra o cifra
    /// en `%XX` (un `+` no se lee como espacio).
    static func armar(_ verbo: String, _ campos: [String: String]) -> String {
        var permitidos = CharacterSet.alphanumerics
        permitidos.insert(charactersIn: "-._~")
        let consulta = campos.filter { !$0.value.isEmpty }.sorted { $0.key < $1.key }.map { k, v in
            k + "=" + (v.addingPercentEncoding(withAllowedCharacters: permitidos) ?? "")
        }.joined(separator: "&")
        return "pixpinmax://" + verbo + (consulta.isEmpty ? "" : "?" + consulta)
    }
}

/// La carpeta de una entrega mientras la extension la llena.
public struct CarpetaDeEntrega: Sendable {
    public let url: URL

    public var ficheros: URL { url.appendingPathComponent(BuzonCompartido.carpetaFicheros, isDirectory: true) }

    /// Los nombres ya puestos.
    public func usados() -> Set<String> {
        Set((try? FileManager.default.contentsOfDirectory(atPath: ficheros.path)) ?? [])
    }

    /// Copia un fichero dentro; devuelve el nombre con que quedo.
    @discardableResult
    public func anadir(fichero origen: URL, nombre: String? = nil) throws -> String {
        let n = BuzonCompartido.nombreLibre(nombre ?? origen.lastPathComponent, usados: usados())
        try FileManager.default.copyItem(at: origen, to: ficheros.appendingPathComponent(n))
        return n
    }

    /// Escribe unos bytes dentro; devuelve el nombre con que quedo.
    @discardableResult
    public func anadir(datos: Data, nombre: String) throws -> String {
        let n = BuzonCompartido.nombreLibre(nombre, usados: usados())
        try datos.write(to: ficheros.appendingPathComponent(n), options: .atomic)
        return n
    }

    /// La da por acabada: escribe `entrega.json` (lo ultimo).
    public func cerrar(_ e: EntregaCompartida) throws {
        let datos = try JSONEncoder().encode(e)
        try datos.write(to: url.appendingPathComponent(BuzonCompartido.ficheroEntrega), options: .atomic)
    }

    /// Se cancelo: fuera todo.
    public func tirar() {
        try? FileManager.default.removeItem(at: url)
    }
}

/// Lo que ensenan los widgets (lo escribe la app al abrirse y al cambiar).
public struct DatosDeWidgets: Codable, Hashable, Sendable {
    public struct Captura: Codable, Hashable, Sendable {
        /// La miniatura, por su nombre dentro de `miniaturas/` del buzon.
        public var miniatura: String
        public var nombre: String
        public var cuando: Int64
        public init(miniatura: String, nombre: String, cuando: Int64) {
            self.miniatura = miniatura
            self.nombre = nombre
            self.cuando = cuando
        }
    }

    public struct Tarea: Codable, Hashable, Sendable {
        public var titulo: String
        public var proyecto: String
        public var hecha: Bool
        public init(titulo: String, proyecto: String, hecha: Bool) {
            self.titulo = titulo
            self.proyecto = proyecto
            self.hecha = hecha
        }
    }

    public var escrita: Int64
    public var capturas: [Captura]
    public var tareasDeHoy: [Tarea]

    public init(escrita: Int64, capturas: [Captura], tareasDeHoy: [Tarea]) {
        self.escrita = escrita
        self.capturas = capturas
        self.tareasDeHoy = tareasDeHoy
    }

    /// La carpeta de las miniaturas dentro del buzon.
    public static func carpetaMiniaturas(en raiz: URL) -> URL {
        raiz.appendingPathComponent("miniaturas", isDirectory: true)
    }
}
