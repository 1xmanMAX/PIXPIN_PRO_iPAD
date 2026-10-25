// **Los enlaces `pixpinmax://`** (`orden::de_enlace` de Windows, 15-oct):
// `pixpinmax://<verbo>/<trozos>?campo=valor` es el mismo verbo de la linea
// de ordenes con sus campos con nombre, y `pixpin://atajo/<verbo>/<id>` es
// la seña de los atajos de Android, para que un enlace sirva en los tres
// aparatos. Por enlace solo pasa directo lo inofensivo (`seguridad`): lo
// demas pregunta y lo que abre una ruta de fuera no pasa nunca. Los enlaces
// que hace la propia app van firmados (`FirmaEnlaces`) y no preguntan.
//
// Todo es puro; lo que toca la app lo hace `HacerPedidos`.

import CryptoKit
import Foundation

/// De donde vino la orden: decide cuanto se confia en ella.
public enum OrigenOrden: Equatable, Sendable {
    /// El menu del Dock, los Servicios del Finder, Atajos (App Intents):
    /// quien lo pide ya esta delante del Mac.
    case consola
    /// Un enlace `pixpinmax://`. `firmado`: lo hizo esta misma PixPin.
    case enlace(firmado: Bool)
}

/// Una orden ya leida.
public struct LlamadaOrden: Equatable, Sendable {
    public var orden: Orden
    public var origen: OrigenOrden
}

/// Cuanto se fia uno de un pedido que llega por enlace. El orden importa:
/// de varios pedidos, manda el peor.
public enum Seguridad: Comparable, Sendable {
    /// Inofensivo: abrir algo, escribir un texto, empezar una captura.
    case directa
    /// Hace algo que el usuario tiene que ver antes. El motivo es un codigo
    /// (`ficheros`, `asistente`, `microfono`, `otros-aparatos`,
    /// `automatizacion`, `borra`, `cambia`).
    case confirmar(String)
    /// Por enlace, nunca: `ruta-no-local` o `crear-automatizacion`.
    case prohibida(String)
}

public enum Enlaces {
    /// Lo mas largo que se acepta en un enlace (`TOPE_ENLACE`).
    public static let tope = 16 * 1024

    /// Los esquemas que se entienden. `pixpinmax:` es el nuestro (el que
    /// declara el Info.plist); `pixpin:` es el de Android y tambien el del
    /// PixPin original de DepthPixel, asi que el Mac no lo reclama, pero si
    /// llega uno (pegado en `pedido`, o abierto a mano) se entiende igual.
    public static let esquemas = ["pixpinmax:", "pixpin:"]

    /// El esquema con que la app arma sus enlaces.
    public static let esquemaDeLaCasa = "pixpinmax"

    /// El largo del esquema si un texto es un enlace nuestro.
    static func largoDelEsquema(_ texto: String) -> Int? {
        let bytes = Array(texto.utf8)
        for e in esquemas {
            let eb = Array(e.utf8)
            guard bytes.count >= eb.count else { continue }
            if zip(bytes[..<eb.count], eb).allSatisfy({ ascii($0) == $1 }) { return eb.count }
        }
        return nil
    }

    private static func ascii(_ b: UInt8) -> UInt8 {
        (b >= 65 && b <= 90) ? b + 32 : b
    }

    /// Si un texto es un enlace `pixpinmax:` (o `pixpin:`).
    public static func esEnlace(_ texto: String) -> Bool {
        largoDelEsquema(texto) != nil
    }

    // MARK: Leer

    /// Un enlace en trozos, ya descodificado.
    public struct Partes: Equatable, Sendable {
        public var verbo: String
        public var sueltos: [String]
        public var campos: [String: String]
        public var firma: String?

        /// Lo que se firma: el verbo, los trozos y los campos ya
        /// descodificados y en orden (por bytes, como el `BTreeMap` de
        /// Windows). Asi reescribir el enlace por el camino no lo invalida.
        public var canonico: String {
            var s = verbo
            for t in sueltos { s += "\n" + t }
            let claves = campos.keys.sorted { Array($0.utf8).lexicographicallyPrecedes(Array($1.utf8)) }
            for k in claves { s += "\n" + k + "=" + (campos[k] ?? "") }
            return s
        }
    }

    /// **Parte un enlace** en verbo, trozos y campos. Las mayusculas del
    /// esquema y del verbo dan igual, la barra final tambien; un campo
    /// repetido: vale el primero.
    public static func partir(_ url: String) throws -> Partes {
        let url = url.trimmingCharacters(in: .whitespacesAndNewlines)
        if url.utf8.count > tope { throw FalloOrden.noSeEntiende("el enlace es demasiado largo") }
        guard let largo = largoDelEsquema(url) else { throw FalloOrden.noSeEntiende("no es un enlace pixpinmax:") }
        var resto = String(decoding: Array(url.utf8)[largo...], as: UTF8.self)
        if let almohadilla = resto.firstIndex(of: "#") { resto = String(resto[..<almohadilla]) }
        while resto.hasPrefix("/") { resto.removeFirst() }
        let camino: Substring
        let consulta: Substring
        if let q = resto.firstIndex(of: "?") {
            camino = resto[..<q]
            consulta = resto[resto.index(after: q)...]
        } else {
            camino = resto[...]
            consulta = ""
        }
        var trozos = try camino.split(separator: "/", omittingEmptySubsequences: true)
            .map { try descodificar(String($0), consulta: false) }
        if let primero = trozos.first, primero.lowercased() == "atajo" { trozos.removeFirst() }
        guard !trozos.isEmpty else { throw FalloOrden.noSeEntiende("el enlace no dice que hacer") }
        let verbo = trozos.removeFirst().lowercased()
        var campos: [String: String] = [:]
        var firma: String?
        for par in consulta.split(separator: "&", omittingEmptySubsequences: true) {
            let (k, v): (Substring, Substring)
            if let igual = par.firstIndex(of: "=") {
                k = par[..<igual]
                v = par[par.index(after: igual)...]
            } else {
                (k, v) = (par, "")
            }
            let clave = try descodificar(String(k), consulta: true).lowercased()
            let valor = try descodificar(String(v), consulta: true)
            if clave == FirmaEnlaces.campo {
                firma = valor
                continue
            }
            // El primero gana: uno repetido no cambia lo ya leido (ni lo firmado).
            if campos[clave] == nil { campos[clave] = valor }
        }
        return Partes(verbo: verbo, sueltos: trozos, campos: campos, firma: firma)
    }

    /// `%XX` a bytes y bytes a UTF-8; en la consulta, `+` es un espacio. Un
    /// `%` sin dos cifras o un UTF-8 roto es un enlace roto.
    public static func descodificar(_ t: String, consulta: Bool) throws -> String {
        let b = Array(t.utf8)
        var fuera: [UInt8] = []
        fuera.reserveCapacity(b.count)
        var i = 0
        while i < b.count {
            switch b[i] {
            case UInt8(ascii: "%"):
                guard i + 2 < b.count, let alto = hex(b[i + 1]), let bajo = hex(b[i + 2]) else {
                    throw FalloOrden.noSeEntiende("el enlace tiene un % mal escrito")
                }
                fuera.append(alto << 4 | bajo)
                i += 3
            case UInt8(ascii: "+") where consulta:
                fuera.append(UInt8(ascii: " "))
                i += 1
            case let c:
                fuera.append(c)
                i += 1
            }
        }
        guard let s = String(validating: fuera, as: UTF8.self) else {
            throw FalloOrden.noSeEntiende("el enlace no es UTF-8")
        }
        return s
    }

    private static func hex(_ c: UInt8) -> UInt8? {
        switch c {
        case 48...57: return c - 48
        case 65...70: return c - 55
        case 97...102: return c - 87
        default: return nil
        }
    }

    /// Lo contrario de `descodificar`: todo lo que no sea letra, cifra o
    /// `-_.~`, en `%XX`.
    public static func codificar(_ t: String) -> String {
        var s = ""
        for b in t.utf8 {
            let c = Character(UnicodeScalar(b))
            if (b < 128 && (c.isLetter || c.isNumber)) || "-_.~".utf8.contains(b) {
                s.append(c)
            } else {
                s += String(format: "%%%02X", b)
            }
        }
        return s
    }

    /// Arma `pixpinmax://<verbo>?campo=valor…` (sin firmar).
    public static func enlace(_ verbo: String, _ campos: [(String, String)] = [],
                              esquema: String = esquemaDeLaCasa) -> String {
        var s = "\(esquema)://\(verbo)"
        for (i, (k, v)) in campos.enumerated() {
            s += (i == 0 ? "?" : "&") + codificar(k) + "=" + codificar(v)
        }
        return s
    }

    /// El enlace de un pedido (`pixpinmax://pedido?json=…`).
    public static func enlace(de pedido: Pedido) -> String {
        enlace("pedido", [("json", pedido.json)])
    }

    /// **Lee un enlace**: el verbo de la tabla con sus campos, y si lo firmo
    /// esta PixPin.
    public static func leer(_ url: String, firmas: FirmaEnlaces? = .delMac) throws -> LlamadaOrden {
        let partes = try partir(url)
        guard let fila = Verbos.fila(partes.verbo) else {
            throw FalloOrden.noSeEntiende("PixPin no conoce «\(partes.verbo)» por enlace")
        }
        guard fila.porEnlace else {
            throw FalloOrden(codigo: FalloOrden.Codigo.rechazado,
                             motivo: "«\(fila.nombres[0])» no se puede pedir por enlace")
        }
        let firmado = partes.firma.map { firmas?.verificar(partes.canonico, firma: $0) ?? false } ?? false
        let entrada = Verbos.Entrada(sueltos: partes.sueltos, campos: partes.campos,
                                     esFichero: { _ in false }, enlace: true)
        return LlamadaOrden(orden: try fila.armar(entrada), origen: .enlace(firmado: firmado))
    }

    // MARK: La seguridad

    /// **La lista blanca** de lo que un enlace sin firmar puede pedir sin
    /// preguntar (`orden::SEGURAS`): nada borra, nada sincroniza ni manda al
    /// movil, nada graba y nada abre una ruta del disco.
    public static let seguras: Set<String> = [
        "abrir", "chat", "nota_nueva", "lienzo_nuevo", "lista_nueva", "anadir_tarea",
        "ventana_principal", "ventana", "capturar", "lecciones", "leccion_nueva", "soltar",
        "pinear", "pinear_ultima", "marcar_tarea", "posponer", "conservar_captura",
    ]

    /// **Cuanto se fia uno de un pedido llegado por enlace.** Se mira el
    /// pedido ya armado, no el verbo, para que `pedido?json=` pase por lo
    /// mismo que los verbos.
    public static func seguridad(_ p: Pedido) -> Seguridad {
        let rutas = rutas(de: p)
        if rutas.contains(where: { !esRutaLocal($0) }) { return .prohibida("ruta-no-local") }
        if p.accion == "automatizacion", p["operacion"]?.texto == "crear" {
            return .prohibida("crear-automatizacion")
        }
        if !rutas.isEmpty, p.accion != "conservar_captura" { return .confirmar("ficheros") }
        let vacio = { (c: String) in p[c]?.vacio ?? true }
        switch p.accion {
        case "asistente" where vacio("texto") && vacio("imagenes") && vacio("automatizacion"):
            return .directa
        case "asistente": return .confirmar("asistente")
        case let a where seguras.contains(a): return .directa
        case "grabar": return .confirmar("microfono")
        case "sincronizar", "enviar_al_movil": return .confirmar("otros-aparatos")
        case "automatizacion": return .confirmar("automatizacion")
        case let a where a.hasPrefix("borrar") || a.hasPrefix("quitar"): return .confirmar("borra")
        default: return .confirmar("cambia")
        }
    }

    /// Un enlace firmado lo hizo la propia app: se hace sin preguntar, salvo
    /// una ruta de fuera, que no pasa nunca.
    public static func seguridadFirmado(_ p: Pedido) -> Seguridad {
        if case .prohibida("ruta-no-local") = seguridad(p) { return .prohibida("ruta-no-local") }
        return .directa
    }

    /// Todas las rutas de un pedido (`ruta`, `rutas`, `imagenes`,
    /// `archivos`, `que.ruta`), tal cual vienen.
    public static func rutas(de p: Pedido) -> [String] {
        var v: [String] = []
        func de(_ x: ValorPedido?) {
            switch x {
            case nil, .nulo?: break
            case let .texto(s)?: v.append(s)
            case let .lista(l)?: v += l.map { $0.texto ?? $0.json }
            case let otro?: v.append(otro.json)
            }
        }
        for c in ["ruta", "rutas", "imagenes", "archivos"] { de(p[c]) }
        de(p["que"]?.objeto?["ruta"])
        return v
    }

    /// **Una ruta absoluta de este Mac.** En Windows, la de una unidad
    /// (`C:\…`); aqui, la que empieza por `/`. Ni de red (`//servidor/…`,
    /// `smb:`, `file:`, ni el automontaje de `/net` y `/Network`, que
    /// conectaria con el servidor que diga el enlace solo por mirarla), ni
    /// relativa (por enlace no hay «carpeta actual»), ni con `~`.
    public static func esRutaLocal(_ r: String) -> Bool {
        guard r.hasPrefix("/"), !r.hasPrefix("//"), !r.contains("\0") else { return false }
        let bajo = r.lowercased()
        for red in ["/net", "/network", "/system/volumes/data/net"] where bajo == red || bajo.hasPrefix(red + "/") {
            return false
        }
        return true
    }

    /// Lo que se ensena al confirmar: la accion y sus campos, en texto corto.
    public static func describir(_ p: Pedido) -> String {
        var s = p.accion
        for k in p.campos.keys.sorted() {
            guard let v = p.campos[k] else { continue }
            let texto = v.texto ?? v.json
            let corto = String(texto.prefix(160))
            s += "\n  \(k): \(corto)\(corto.count < texto.count ? "…" : "")"
        }
        return s
    }

    /// Los textos de los cuadros (`orden-*` del catalogo de Windows).
    public static func textoDelMotivo(_ motivo: String) -> String {
        switch motivo {
        case "ficheros": return "Abre o manda ficheros de tu disco."
        case "asistente": return "Le pide algo al asistente, que puede hacer cosas en PixPin."
        case "microfono": return "Graba con el micrófono."
        case "otros-aparatos": return "Manda datos a tus otros aparatos."
        case "automatizacion": return "Toca una automatización del asistente."
        case "borra": return "Borra algo."
        case "ruta-no-local":
            return "Un enlace no puede abrir ficheros de red ni rutas que no sean de este Mac. No se hizo nada."
        case "crear-automatizacion": return "Un enlace no puede crear automatizaciones. No se hizo nada."
        default: return "Cambia datos de PixPin."
        }
    }
}

// MARK: - La firma

/// **Los enlaces firmados** (`orden::firma`): HMAC-SHA256 de la forma
/// canonica con una clave de 32 bytes al azar que vive en la carpeta de
/// datos de este Mac (`enlaces.clave`, solo legible por el usuario) y que
/// una web no puede conocer. Un enlace con una firma buena se hace sin
/// preguntar (salvo una ruta de fuera).
public struct FirmaEnlaces: Sendable {
    /// El nombre del campo que lleva la firma.
    public static let campo = "firma"

    /// La de este Mac: `<carpeta de datos>/enlaces.clave`.
    public static var delMac: FirmaEnlaces {
        FirmaEnlaces(ruta: carpetaDeDatos().appendingPathComponent("enlaces.clave"))
    }

    public let ruta: URL?
    let claveFija: [UInt8]?

    public init(ruta: URL) {
        self.ruta = ruta
        claveFija = nil
    }

    /// Con una clave dada (las pruebas).
    public init(clave: [UInt8]) {
        ruta = nil
        claveFija = clave
    }

    /// La clave, sin crearla (`nil` si nunca se firmo nada aqui).
    func claveSiHay() -> [UInt8]? {
        if let claveFija { return claveFija }
        guard let ruta, let d = try? Data(contentsOf: ruta), d.count == 32 else { return nil }
        return [UInt8](d)
    }

    /// La clave; la crea la primera vez.
    func clave() throws -> [UInt8] {
        if let c = claveSiHay() { return c }
        guard let ruta else { throw FalloOrden.fallo("sin clave") }
        var nueva = [UInt8](repeating: 0, count: 32)
        guard SecRandomCopyBytes(kSecRandomDefault, nueva.count, &nueva) == errSecSuccess else {
            throw FalloOrden.fallo("no se pudo sacar una clave al azar")
        }
        try FileManager.default.createDirectory(at: ruta.deletingLastPathComponent(),
                                                withIntermediateDirectories: true)
        // Si otra copia la escribio a la vez, gana la suya.
        if FileManager.default.createFile(atPath: ruta.path, contents: Data(nueva),
                                          attributes: [.posixPermissions: 0o600]) {
            if let c = claveSiHay() { return c }
        }
        try Data(nueva).write(to: ruta, options: .atomic)
        try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: ruta.path)
        return nueva
    }

    /// La firma de un texto canonico, en hexadecimal.
    static func firma(clave: [UInt8], _ canonico: String) -> String {
        let mac = HMAC<SHA256>.authenticationCode(for: Data(canonico.utf8), using: SymmetricKey(data: clave))
        return mac.map { String(format: "%02x", $0) }.joined()
    }

    /// Si `firma` (hex) es la de `canonico`. En tiempo constante.
    public func verificar(_ canonico: String, firma: String) -> Bool {
        guard let clave = claveSiHay(), let bytes = FirmaEnlaces.deHex(firma) else { return false }
        return HMAC<SHA256>.isValidAuthenticationCode(bytes, authenticating: Data(canonico.utf8),
                                                      using: SymmetricKey(data: clave))
    }

    static func deHex(_ t: String) -> [UInt8]? {
        let b = Array(t.utf8)
        guard b.count == 64 else { return nil }
        var v: [UInt8] = []
        var i = 0
        while i < 64 {
            guard let s = String(bytes: b[i..<i + 2], encoding: .ascii), let n = UInt8(s, radix: 16) else { return nil }
            v.append(n)
            i += 2
        }
        return v
    }

    /// **Firma un enlace**: el mismo con `&firma=…` al final (la que llevara
    /// ya, fuera).
    public func firmar(_ url: String) throws -> String {
        let partes = try Enlaces.partir(url)
        let f = FirmaEnlaces.firma(clave: try clave(), partes.canonico)
        var base = url
        if let r = url.range(of: "firma="), r.lowerBound > url.startIndex {
            let antes = url[url.index(before: r.lowerBound)]
            if antes == "?" || antes == "&" { base = String(url[..<url.index(before: r.lowerBound)]) }
        }
        return base + (base.contains("?") ? "&" : "?") + "\(FirmaEnlaces.campo)=\(f)"
    }
}
