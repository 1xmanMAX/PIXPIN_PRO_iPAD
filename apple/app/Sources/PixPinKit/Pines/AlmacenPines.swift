// El almacen de los pines: lo que hace que un pin vuelva al arrancar (lo que
// en Windows es `pixpin-store/src/almacen.rs`, D21/D25/D27/D28).
//
// **El mismo formato que Windows**, escrito en Swift puro porque es un JSON
// sencillo: `<carpeta de datos>/almacen/indice.json` y los objetos en
// `almacen/objetos/<año>/<mes>/<id>.<ext>`.
//
// - Una captura es un PNG propio del almacen (`origen` «recorte» o
//   «portapapeles»); una foto abierta se copia tal cual con su extension
//   (`origen` «foto»).
// - Una nota es un `.txt` UTF-8; una mini-app, una nota con `origen`
//   `mini:<cual>` (o `mini:<cual>@<mensaje>|<chat>` si vive en un chat: en
//   Windows lo de detras de `|` es la carpeta del proyecto, en el Mac el id
//   del chat).
// - Un PDF o un video van **por referencia** (`archivo`): se apunta la ruta,
//   no se copia nada.
// - `pin` es donde esta abierto (y como: giro, filtros, opacidad, clic a
//   traves); `null` es cerrado. Cerrar no borra nada.
//
// Diferencia con Windows: `x`, `y`, `ancho` y `alto` van en **puntos** del
// Mac (origen arriba a la izquierda de la pantalla principal, y hacia
// abajo, como en Windows), no en pixeles; `escala_por_cien` es la de la
// pantalla donde estaba (200 en Retina). El almacen no viaja entre aparatos
// (no lo sincroniza nadie), asi que basta con que cada sistema entienda lo
// suyo y el fichero se lea igual.
//
// El indice es lo UNICO que se reescribe, siempre a temporal + rename. Un
// indice con claves desconocidas (de una version futura) abre igual.

import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// Donde y como esta abierto un pin (`PinGuardado` de Windows).
struct PinGuardado: Codable, Equatable {
    var x: Int
    var y: Int
    var ancho: Int
    var alto: Int
    var escalaPorCien: Int
    var zoomPorCien: Int = 100
    /// Cuartos de vuelta a la derecha (0 a 3).
    var giro: Int = 0
    var volteoH = false
    var volteoV = false
    /// Deja pasar los clics a lo de debajo (⌘T).
    var pasante = false
    var gris = false
    var invertido = false
    /// Pasos de brillo (−10 a 10).
    var brillo = 0
    var opacidad: Int = 100

    init(x: Int, y: Int, ancho: Int, alto: Int, escalaPorCien: Int) {
        self.x = x
        self.y = y
        self.ancho = ancho
        self.alto = alto
        self.escalaPorCien = escalaPorCien
    }

    enum CodingKeys: String, CodingKey {
        case x, y, ancho, alto
        case escalaPorCien = "escala_por_cien"
        case zoomPorCien = "zoom_por_cien"
        case giro
        case volteoH = "volteo_h"
        case volteoV = "volteo_v"
        case pasante, gris, invertido, brillo, opacidad
    }

    /// Lo que falta (un indice viejo) toma lo de Windows: sin girar, sin
    /// filtros, opaco.
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        x = try c.decode(Int.self, forKey: .x)
        y = try c.decode(Int.self, forKey: .y)
        ancho = try c.decode(Int.self, forKey: .ancho)
        alto = try c.decode(Int.self, forKey: .alto)
        escalaPorCien = try c.decodeIfPresent(Int.self, forKey: .escalaPorCien) ?? 100
        zoomPorCien = try c.decodeIfPresent(Int.self, forKey: .zoomPorCien) ?? 100
        giro = try c.decodeIfPresent(Int.self, forKey: .giro) ?? 0
        volteoH = try c.decodeIfPresent(Bool.self, forKey: .volteoH) ?? false
        volteoV = try c.decodeIfPresent(Bool.self, forKey: .volteoV) ?? false
        pasante = try c.decodeIfPresent(Bool.self, forKey: .pasante) ?? false
        gris = try c.decodeIfPresent(Bool.self, forKey: .gris) ?? false
        invertido = try c.decodeIfPresent(Bool.self, forKey: .invertido) ?? false
        brillo = try c.decodeIfPresent(Int.self, forKey: .brillo) ?? 0
        opacidad = try c.decodeIfPresent(Int.self, forKey: .opacidad) ?? 100
    }
}

enum TipoEntrada: String, Codable {
    case imagen, nota
    /// Por referencia: nunca se copia (D28).
    case archivo
}

/// La paleta de grupos: un grupo ES su color. El orden es el de
/// `ColoresGrupo` (y el de Windows).
enum ColorGrupo: String, Codable, CaseIterable {
    case rojo, naranja, ambar, verde, cian, azul, violeta, rosa

    var indice: Int { Self.allCases.firstIndex(of: self) ?? 0 }

    static func porIndice(_ i: Int) -> ColorGrupo? {
        allCases.indices.contains(i) ? allCases[i] : nil
    }
}

struct GrupoPines: Codable, Equatable {
    var id: UInt32
    var color: ColorGrupo
    var oculto = false

    init(id: UInt32, color: ColorGrupo, oculto: Bool = false) {
        self.id = id
        self.color = color
        self.oculto = oculto
    }

    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = try c.decode(UInt32.self, forKey: .id)
        color = try c.decode(ColorGrupo.self, forKey: .color)
        oculto = try c.decodeIfPresent(Bool.self, forKey: .oculto) ?? false
    }
}

struct EntradaAlmacen: Codable, Equatable {
    var id: UInt64
    var tipo: TipoEntrada
    /// ISO-8601 UTC.
    var creado: String
    var origen: String
    /// Ruta relativa a `almacen/`; vacia en las entradas por referencia.
    var objeto: String
    /// Ruta absoluta del fichero del usuario (solo `archivo`).
    var ruta: String?
    var grupo: UInt32?
    var pin: PinGuardado?

    enum CodingKeys: String, CodingKey {
        case id, tipo, creado, origen, objeto, ruta, grupo, pin
    }

    init(id: UInt64, tipo: TipoEntrada, creado: String, origen: String, objeto: String,
         ruta: String?, grupo: UInt32?, pin: PinGuardado?) {
        self.id = id
        self.tipo = tipo
        self.creado = creado
        self.origen = origen
        self.objeto = objeto
        self.ruta = ruta
        self.grupo = grupo
        self.pin = pin
    }

    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = try c.decode(UInt64.self, forKey: .id)
        tipo = try c.decode(TipoEntrada.self, forKey: .tipo)
        creado = try c.decodeIfPresent(String.self, forKey: .creado) ?? ""
        origen = try c.decodeIfPresent(String.self, forKey: .origen) ?? ""
        objeto = try c.decodeIfPresent(String.self, forKey: .objeto) ?? ""
        ruta = try c.decodeIfPresent(String.self, forKey: .ruta)
        grupo = try c.decodeIfPresent(UInt32.self, forKey: .grupo)
        pin = try c.decodeIfPresent(PinGuardado.self, forKey: .pin)
    }

    /// Como serde: los `None` se escriben como `null`.
    func encode(to e: Encoder) throws {
        var c = e.container(keyedBy: CodingKeys.self)
        try c.encode(id, forKey: .id)
        try c.encode(tipo, forKey: .tipo)
        try c.encode(creado, forKey: .creado)
        try c.encode(origen, forKey: .origen)
        try c.encode(objeto, forKey: .objeto)
        try c.encode(ruta, forKey: .ruta)
        try c.encode(grupo, forKey: .grupo)
        try c.encode(pin, forKey: .pin)
    }
}

/// El fichero `indice.json` entero.
struct IndiceAlmacen: Codable {
    var version: UInt32 = 1
    var siguienteId: UInt64 = 1
    var grupos: [GrupoPines] = []
    var entradas: [EntradaAlmacen] = []

    enum CodingKeys: String, CodingKey {
        case version
        case siguienteId = "siguiente_id"
        case grupos, entradas
    }

    init() {}

    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        version = try c.decodeIfPresent(UInt32.self, forKey: .version) ?? 1
        siguienteId = try c.decodeIfPresent(UInt64.self, forKey: .siguienteId) ?? 1
        grupos = try c.decodeIfPresent([GrupoPines].self, forKey: .grupos) ?? []
        entradas = try c.decodeIfPresent([EntradaAlmacen].self, forKey: .entradas) ?? []
    }
}

enum ErrorAlmacen: Error, Equatable {
    case noExiste(UInt64)
}

/// El almacen abierto: el indice en memoria y la carpeta.
final class AlmacenPines {
    /// Lo que va delante del origen de una mini-app (`mini:tareas`).
    static let prefijoMini = "mini:"
    static let origenFoto = "foto"

    let dir: URL
    private(set) var indice: IndiceAlmacen

    /// Abre (o crea) `<raiz>/almacen`. Un indice que no se puede leer no
    /// impide arrancar: se aparta como `indice.json.roto` y se empieza
    /// de cero (perder la lista es mejor que no poder pinear).
    init(raiz: URL) throws {
        dir = raiz.appendingPathComponent("almacen", isDirectory: true)
        let fm = FileManager.default
        try fm.createDirectory(at: dir.appendingPathComponent("objetos", isDirectory: true),
                               withIntermediateDirectories: true)
        let ruta = dir.appendingPathComponent("indice.json")
        if let datos = try? Data(contentsOf: ruta) {
            if let leido = try? JSONDecoder().decode(IndiceAlmacen.self, from: datos) {
                indice = leido
            } else {
                let roto = dir.appendingPathComponent("indice.json.roto")
                try? fm.removeItem(at: roto)
                try? fm.moveItem(at: ruta, to: roto)
                indice = IndiceAlmacen()
            }
        } else {
            indice = IndiceAlmacen()
        }
    }

    var entradas: [EntradaAlmacen] { indice.entradas }
    var grupos: [GrupoPines] { indice.grupos }

    func entrada(_ id: UInt64) -> EntradaAlmacen? {
        indice.entradas.first { $0.id == id }
    }

    func rutaObjeto(_ e: EntradaAlmacen) -> URL {
        dir.appendingPathComponent(e.objeto)
    }

    /// El color del grupo de la entrada, si tiene.
    func colorDe(_ id: UInt64) -> ColorGrupo? {
        guard let g = entrada(id)?.grupo else { return nil }
        return indice.grupos.first { $0.id == g }?.color
    }

    // MARK: Guardar

    private func nuevoId() -> UInt64 {
        let id = max(indice.siguienteId, 1)
        indice.siguienteId = id + 1
        return id
    }

    /// Escribe un objeto propio (una vez) y anota su entrada.
    private func guardarObjeto(extension ext: String, tipo: TipoEntrada, origen: String, pin: PinGuardado?,
                               escribir: (URL) throws -> Void) throws -> UInt64 {
        let id = nuevoId()
        let relativa = Self.rutaRelativa(id: id, extension: ext, fecha: Date())
        let ruta = dir.appendingPathComponent(relativa)
        try FileManager.default.createDirectory(at: ruta.deletingLastPathComponent(),
                                                withIntermediateDirectories: true)
        try escribir(ruta)
        indice.entradas.append(EntradaAlmacen(id: id, tipo: tipo, creado: Self.ahoraISO(), origen: origen,
                                              objeto: relativa, ruta: nil, grupo: nil, pin: pin))
        try persistir()
        return id
    }

    func guardarImagen(png: Data, origen: String, pin: PinGuardado?) throws -> UInt64 {
        try guardarObjeto(extension: "png", tipo: .imagen, origen: origen, pin: pin) {
            try png.write(to: $0)
        }
    }

    /// Una foto que ya es un fichero: se copia TAL CUAL, con su extension
    /// (en APFS la copia es un clon: no ocupa ni tarda).
    func guardarImagen(deFichero fichero: URL, origen: String, pin: PinGuardado?) throws -> UInt64 {
        try guardarObjeto(extension: Self.extensionDeObjeto(fichero), tipo: .imagen, origen: origen, pin: pin) {
            try FileManager.default.copyItem(at: fichero, to: $0)
        }
    }

    func guardarNota(_ texto: String, origen: String, pin: PinGuardado?) throws -> UInt64 {
        try guardarObjeto(extension: "txt", tipo: .nota, origen: origen, pin: pin) {
            try Data(texto.utf8).write(to: $0)
        }
    }

    /// Por referencia: se apunta la ruta, no se copia ni un byte.
    func guardarArchivo(_ ruta: URL, pin: PinGuardado?) throws -> UInt64 {
        let id = nuevoId()
        indice.entradas.append(EntradaAlmacen(id: id, tipo: .archivo, creado: Self.ahoraISO(),
                                              origen: "portapapeles", objeto: "",
                                              ruta: ruta.path, grupo: nil, pin: pin))
        try persistir()
        return id
    }

    /// Lo de dentro de una nota o mini-app cambio: se reescribe su `.txt`
    /// (a temporal + rename, como el indice).
    func reescribirNota(_ id: UInt64, texto: String) throws {
        guard let e = entrada(id), e.tipo == .nota, !e.objeto.isEmpty else { throw ErrorAlmacen.noExiste(id) }
        try Data(texto.utf8).write(to: rutaObjeto(e), options: .atomic)
    }

    /// Una foto anotada y fijada ya no es la de antes: un objeto nuevo
    /// (PNG) para la misma entrada, y el viejo fuera.
    func reemplazarImagen(_ id: UInt64, png: Data) throws {
        guard let i = indice.entradas.firstIndex(where: { $0.id == id }), indice.entradas[i].tipo == .imagen
        else { throw ErrorAlmacen.noExiste(id) }
        let viejo = indice.entradas[i].objeto
        let relativa = Self.rutaRelativa(id: id, extension: "png", fecha: Date(), sufijo: "-\(Int(Date().timeIntervalSince1970))")
        let ruta = dir.appendingPathComponent(relativa)
        try FileManager.default.createDirectory(at: ruta.deletingLastPathComponent(),
                                                withIntermediateDirectories: true)
        try png.write(to: ruta)
        indice.entradas[i].objeto = relativa
        indice.entradas[i].origen = "recorte"
        try persistir()
        if !viejo.isEmpty, viejo != relativa { try? FileManager.default.removeItem(at: dir.appendingPathComponent(viejo)) }
    }

    /// Abierto (con su sitio) o cerrado (`nil`).
    func actualizarPin(_ id: UInt64, _ pin: PinGuardado?) throws {
        guard let i = indice.entradas.firstIndex(where: { $0.id == id }) else { throw ErrorAlmacen.noExiste(id) }
        guard indice.entradas[i].pin != pin else { return }
        indice.entradas[i].pin = pin
        try persistir()
    }

    /// El color ES el grupo: si ya hay uno de ese color se reutiliza; un
    /// grupo sin entradas desaparece.
    @discardableResult
    func ponerGrupo(_ id: UInt64, color: ColorGrupo?) throws -> UInt32? {
        guard let i = indice.entradas.firstIndex(where: { $0.id == id }) else { throw ErrorAlmacen.noExiste(id) }
        var nuevo: UInt32?
        if let color {
            if let g = indice.grupos.first(where: { $0.color == color }) {
                nuevo = g.id
            } else {
                let gid = (indice.grupos.map(\.id).max() ?? 0) + 1
                indice.grupos.append(GrupoPines(id: gid, color: color))
                nuevo = gid
            }
        }
        guard indice.entradas[i].grupo != nuevo else { return nuevo }
        indice.entradas[i].grupo = nuevo
        let entradas = indice.entradas
        indice.grupos.removeAll { g in !entradas.contains { $0.grupo == g.id } }
        try persistir()
        return nuevo
    }

    /// El grupo de la entrada, si tiene.
    func grupoDe(_ id: UInt64) -> GrupoPines? {
        guard let g = entrada(id)?.grupo else { return nil }
        return indice.grupos.first { $0.id == g }
    }

    /// Oculta (o vuelve a ensenar) un grupo entero (D24 de Windows): sus
    /// entradas se quedan con su `pin`, que es lo que las devuelve a su sitio
    /// exacto al mostrarlo; mientras este oculto, no vuelven al arrancar.
    func ponerGrupoOculto(_ grupo: UInt32, _ oculto: Bool) throws {
        guard let i = indice.grupos.firstIndex(where: { $0.id == grupo }) else { throw ErrorAlmacen.noExiste(UInt64(grupo)) }
        guard indice.grupos[i].oculto != oculto else { return }
        indice.grupos[i].oculto = oculto
        try persistir()
    }

    /// Los grupos ocultos con cuantas entradas tiene cada uno (para el menu
    /// de la barra: es la unica via de vuelta de unos pines que no se ven).
    var gruposOcultos: [(grupo: GrupoPines, cuantos: Int)] {
        indice.grupos.filter(\.oculto).map { g in
            (g, indice.entradas.filter { $0.grupo == g.id }.count)
        }
    }

    /// Borra la entrada y su objeto propio; un archivo por referencia es del
    /// usuario y no se toca.
    func eliminar(_ id: UInt64) throws {
        guard let i = indice.entradas.firstIndex(where: { $0.id == id }) else { throw ErrorAlmacen.noExiste(id) }
        let e = indice.entradas.remove(at: i)
        if !e.objeto.isEmpty { try? FileManager.default.removeItem(at: rutaObjeto(e)) }
        let entradas = indice.entradas
        indice.grupos.removeAll { g in !entradas.contains { $0.grupo == g.id } }
        try persistir()
    }

    /// Las entradas abiertas que tienen que volver al arrancar: con `pin` y
    /// sin grupo oculto.
    var abiertas: [EntradaAlmacen] {
        let ocultos = Set(indice.grupos.filter(\.oculto).map(\.id))
        return indice.entradas.filter { e in
            e.pin != nil && !(e.grupo.map { ocultos.contains($0) } ?? false)
        }
    }

    /// Temporal + rename: un proceso que muera a mitad deja el indice de
    /// antes entero.
    func persistir() throws {
        let codificador = JSONEncoder()
        codificador.outputFormatting = [.prettyPrinted, .withoutEscapingSlashes]
        let datos = try codificador.encode(indice)
        let definitivo = dir.appendingPathComponent("indice.json")
        let temporal = dir.appendingPathComponent("indice.json.tmp")
        try datos.write(to: temporal)
        if FileManager.default.fileExists(atPath: definitivo.path) {
            _ = try FileManager.default.replaceItemAt(definitivo, withItemAt: temporal)
        } else {
            try FileManager.default.moveItem(at: temporal, to: definitivo)
        }
    }

    // MARK: Cuentas puras

    /// `objetos/<año>/<mes>/<id con 6 cifras>.<ext>` (UTC), como Windows.
    static func rutaRelativa(id: UInt64, extension ext: String, fecha: Date, sufijo: String = "") -> String {
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = TimeZone(identifier: "UTC")!
        let c = cal.dateComponents([.year, .month], from: fecha)
        return String(format: "objetos/%04d/%02d/%06llu%@.%@", c.year ?? 1970, c.month ?? 1, id, sufijo, ext)
    }

    /// La extension de la copia: la suya en minusculas si es corta y solo de
    /// letras y numeros; si no, `img`.
    static func extensionDeObjeto(_ fichero: URL) -> String {
        let e = fichero.pathExtension
        guard !e.isEmpty, e.count <= 5, e.allSatisfy({ $0.isASCII && ($0.isLetter || $0.isNumber) }) else { return "img" }
        return e.lowercased()
    }

    static func ahoraISO(_ fecha: Date = Date()) -> String {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime]
        f.timeZone = TimeZone(identifier: "UTC")
        return f.string(from: fecha)
    }

    // MARK: Origen de las mini-apps

    /// `mini:<cual>`, o `mini:<cual>@<mensaje>|<chat>` si vive en un chat.
    static func origenMini(_ cual: String, chat: String?, mensaje: String?) -> String {
        guard let chat, let mensaje, !chat.isEmpty, !mensaje.isEmpty else { return prefijoMini + cual }
        return "\(prefijoMini)\(cual)@\(mensaje)|\(chat)"
    }

    /// Que mini-app dice un origen y, si la tiene, de que mensaje y chat.
    static func miniDeOrigen(_ origen: String) -> (cual: String, chat: String?, mensaje: String?)? {
        guard origen.hasPrefix(prefijoMini) else { return nil }
        let resto = String(origen.dropFirst(prefijoMini.count))
        guard let arroba = resto.firstIndex(of: "@") else {
            return resto.isEmpty ? nil : (resto, nil, nil)
        }
        let cual = String(resto[..<arroba])
        let vinculo = resto[resto.index(after: arroba)...]
        guard !cual.isEmpty else { return nil }
        guard let barra = vinculo.firstIndex(of: "|") else { return (cual, nil, nil) }
        let mensaje = String(vinculo[..<barra])
        let chat = String(vinculo[vinculo.index(after: barra)...])
        guard !mensaje.isEmpty, !chat.isEmpty else { return (cual, nil, nil) }
        return (cual, chat, mensaje)
    }
}

/// Las cuentas entre el marco de AppKit (origen abajo, en puntos) y el del
/// almacen (origen arriba a la izquierda de la pantalla principal, hacia
/// abajo, como Windows).
enum ReglasAlmacen {
    /// El marco de AppKit a `x, y, ancho, alto` del almacen. `techo` es el
    /// `maxY` de la pantalla principal.
    static func aAlmacen(_ marco: CGRect, techo: CGFloat) -> (x: Int, y: Int, ancho: Int, alto: Int) {
        (Int(marco.minX.rounded()), Int((techo - marco.maxY).rounded()),
         max(1, Int(marco.width.rounded())), max(1, Int(marco.height.rounded())))
    }

    static func deAlmacen(_ g: PinGuardado, techo: CGFloat) -> CGRect {
        CGRect(x: CGFloat(g.x), y: techo - CGFloat(g.y) - CGFloat(g.alto),
               width: CGFloat(max(g.ancho, 1)), height: CGFloat(max(g.alto, 1)))
    }

    /// Lo mete en `area` sin cambiar su tamano (`recolocar_en_area` de
    /// Windows): si es mas grande, se pega arriba a la izquierda.
    static func recolocar(_ r: CGRect, en area: CGRect) -> CGRect {
        // En AppKit arriba es `maxY`.
        let x = r.width > area.width ? area.minX : min(max(r.minX, area.minX), area.maxX - r.width)
        let y = r.height > area.height ? area.maxY - r.height : min(max(r.minY, area.minY), area.maxY - r.height)
        return CGRect(x: x, y: y, width: r.width, height: r.height)
    }

    /// El area donde vuelve: la de la primera pantalla que lo toque; si
    /// ninguna (se desconecto), la principal.
    static func areaDeVuelta(_ r: CGRect, pantallas: [(marco: CGRect, visible: CGRect)]) -> CGRect? {
        pantallas.first { $0.marco.intersects(r) }?.visible ?? pantallas.first?.visible
    }
}

extension AlmacenPines {
    /// PNG de una imagen (para guardar una captura). `nil` si no se pudo.
    static func png(_ imagen: CGImage) -> Data? {
        let datos = NSMutableData()
        guard let destino = CGImageDestinationCreateWithData(datos as CFMutableData, UTType.png.identifier as CFString, 1, nil)
        else { return nil }
        CGImageDestinationAddImage(destino, imagen, nil)
        guard CGImageDestinationFinalize(destino) else { return nil }
        return datos as Data
    }
}
