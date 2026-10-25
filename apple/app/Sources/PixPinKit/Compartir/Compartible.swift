// **La hoja de compartir como logica pura** (D1): que formatos se ven, que
// paginas van y que hay que hacer. Es `pixpin_ui::hoja_compartir` de
// Windows (y `ui/HojaDeCompartir.kt` del movil) con sus mismas reglas:
//
// - los formatos, en redondeles que se reconocen sin leer;
// - que paginas, cuando hay para elegir. En los formatos de una sola pagina
//   (una imagen, un SVG) se elige una; en los demas, las que se quiera.
//   **Con mas de una marcada, los de una sola desaparecen** (menos el que
//   este puesto): ofrecer «PNG» con doce paginas marcadas invita a un clic
//   que no puede hacer lo que promete;
// - el peso y las salidas.
//
// Que formatos hay y como se hace cada uno lo decide quien abre la hoja
// (`PreparadoCompartir`): esto no sabe de lienzos, PDF ni proyectos.

import Foundation

/// Cuantas paginas admite un formato (`Cuantas` de Windows).
enum CuantasPaginas: Equatable {
    /// Va entero: el original, el `.pixpin`, el texto de una nota.
    case ninguna
    /// Una sola: una imagen o un SVG no tienen paginas.
    case una
    /// Las que se marquen: un PDF, una pagina web.
    case varias
}

/// Una pagina que se puede elegir.
struct PaginaCompartir: Equatable, Identifiable {
    var clave: String
    var nombre: String
    /// Debajo del nombre: de que proyecto, que tipo.
    var detalle: String
    /// Sangria: un marco va debajo de su lienzo.
    var nivel: Int = 0

    var id: String { clave }
}

/// Los formatos, por el nombre que entiende quien los hace (los de
/// Windows: `compartir::WEB`, `PDF`...).
enum IdFormato {
    static let web = "web"
    static let pdf = "pdf"
    static let png = "png"
    static let jpg = "jpg"
    static let svg = "svg"
    static let original = "original"
    static let originales = "originales"
    static let editable = "editable"
    static let pixpin = "pixpin"
    static let texto = "texto"
    /// Los propios de una grabacion (y `word` de una nota en Windows).
    static let word = "word"
    static let md = "md"
    static let anki = "anki"
}

/// Un formato que se ofrece.
struct FormatoCompartir: Equatable {
    var id: String
    var nombre: String
    var cuantas: CuantasPaginas
    /// El simbolo de su redondel.
    var simbolo: String
}

/// Lo que se comparte, visto desde la hoja.
struct Compartible: Equatable {
    var titulo: String
    var paginas: [PaginaCompartir]
    var formatos: [FormatoCompartir]
    /// Las marcadas al abrir; `nil`, todas.
    var marcadas: Set<String>?
}

/// **Lo que se ha elegido en la hoja** (`Estado` de Windows).
struct EstadoHoja: Equatable {
    /// El formato puesto, por su posicion en `Compartible.formatos`.
    var formato: Int
    var marcadas: Set<String>

    /// Al abrir: el primer formato y lo que diga `marcadas` (o todo).
    init(_ c: Compartible) {
        formato = 0
        marcadas = c.marcadas ?? Set(c.paginas.map(\.clave))
    }

    func formato(_ c: Compartible) -> FormatoCompartir? {
        c.formatos.indices.contains(formato) ? c.formatos[formato] : nil
    }

    /// Los formatos que se ven, por su posicion: con mas de una pagina
    /// marcada, los de una sola desaparecen, menos el que este puesto.
    func visibles(_ c: Compartible) -> [Int] {
        c.formatos.indices.filter { i in
            !(c.formatos[i].cuantas == .una && marcadas.count > 1 && i != formato)
        }
    }

    /// Lo que va, en el orden de la lista. Vacio con un formato que va
    /// entero.
    func elegidas(_ c: Compartible) -> [String] {
        guard let f = formato(c), f.cuantas != .ninguna else { return [] }
        return c.paginas.map(\.clave).filter { marcadas.contains($0) }
    }

    /// Si con lo elegido hay algo que hacer: un formato de paginas sin
    /// ninguna marcada no lo tiene («Marca alguna página»).
    func hayQuePreparar(_ c: Compartible) -> Bool {
        guard let f = formato(c) else { return false }
        if f.cuantas == .ninguna || c.paginas.isEmpty { return true }
        return !elegidas(c).isEmpty
    }

    /// Pone el formato `i`. Si es de una sola pagina y habia varias
    /// marcadas, se queda la primera.
    mutating func elegirFormato(_ c: Compartible, _ i: Int) {
        guard c.formatos.indices.contains(i) else { return }
        formato = i
        if c.formatos[i].cuantas == .una, marcadas.count > 1 {
            marcadas = Set(c.paginas.map(\.clave).first { marcadas.contains($0) }.map { [$0] } ?? [])
        }
    }

    /// Un clic en una pagina: con un formato de una sola, pasa a ser la
    /// unica; con uno de varias, se pone o se quita.
    mutating func tocarPagina(_ c: Compartible, _ clave: String) {
        guard let f = formato(c), c.paginas.contains(where: { $0.clave == clave }) else { return }
        switch f.cuantas {
        case .ninguna: break
        case .una: marcadas = [clave]
        case .varias:
            if marcadas.contains(clave) { marcadas.remove(clave) } else { marcadas.insert(clave) }
        }
    }

    mutating func todas(_ c: Compartible) { marcadas = Set(c.paginas.map(\.clave)) }

    mutating func ninguna() { marcadas = [] }

    /// Pasar al formato visible de al lado (← →).
    mutating func moverFormato(_ c: Compartible, _ paso: Int) {
        let v = visibles(c)
        guard let i = v.firstIndex(of: formato) else { return }
        let j = min(max(0, i + paso), v.count - 1)
        elegirFormato(c, v[j])
    }
}

/// Los nombres y textos que se repiten.
enum TextosCompartir {
    /// «Qué incluir · 3 de 5».
    static func queIncluir(_ marcadas: Int, de total: Int) -> String { "Qué incluir · \(marcadas) de \(total)" }

    /// «1,2 MB · 3 archivos».
    static func peso(_ bytes: Int64, ficheros n: Int) -> String {
        let peso = ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file)
        return n == 1 ? peso : "\(peso) · \(n) archivos"
    }

    /// El nombre de una fila: el del mensaje o lo primero de su texto,
    /// cortado a 48 letras (`nombre_de` de Windows).
    static func nombreDeFila(nombre: String, resumen: String) -> String {
        let limpio = nombre.trimmingCharacters(in: .whitespacesAndNewlines)
        let base = limpio.isEmpty
            ? (resumen.split(whereSeparator: \.isNewline).first.map(String.init) ?? "")
                .trimmingCharacters(in: .whitespaces)
            : limpio
        guard !base.isEmpty else { return "Sin nombre" }
        return base.count > 48 ? String(base.prefix(48)) + "…" : base
    }

    /// El nombre sin la extension (`sin_extension`).
    static func sinExtension(_ nombre: String) -> String {
        let n = (nombre as NSString).deletingPathExtension
        return n.isEmpty ? nombre : n
    }
}
