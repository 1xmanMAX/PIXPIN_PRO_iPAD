// Las cuentas del lector de documentos, sin ventanas (se prueban solas):
// que es cada fichero, y como se pasa de un punto de la pantalla a las
// unidades de la tinta de una hoja y al reves.
//
// Las unidades de la tinta son las de Windows y Android
// (`pixpin-docs/src/vista.rs`): cada hoja mide 1400 de ancho, con el cero en
// su esquina de arriba a la izquierda y la Y hacia abajo; su alto es
// `1400 * alto / ancho` de la pagina tal como se ve (ya girada). Asi la
// tinta cae en el mismo sitio en los tres sistemas aunque cada uno pinte la
// hoja a otro tamano.

import CoreGraphics
import Foundation
import PixPinCore

/// Que se hace con un fichero al abrirlo.
public enum ClaseDeDocumento: Equatable, Sendable {
    /// PDF: el lector con tinta (PDFKit).
    case pdf
    /// Word o PowerPoint: Vista Rapida al momento, o el lector paginado como
    /// Office (`pixpin-office`) para anotarlo.
    case officePaginado
    /// Lo que ensena Vista Rapida y no se anota aqui (Excel, ODF, RTF, CSV,
    /// EPUB...).
    case vistaRapida
    /// Ninguno de los anteriores.
    case otro
}

public enum TiposDeDocumento {
    /// Los que `pixpin-office` sabe paginar (`pixpin_office::tipo_de`).
    public static let office: Set<String> = [
        "docx", "docm", "dotx", "dotm", "pptx", "pptm", "potx", "potm", "ppsx", "ppsm",
    ]
    /// Los que se abren al instante con Vista Rapida en un pin.
    public static let vistaRapida: Set<String> = [
        "xlsx", "xlsm", "xltx", "xls", "doc", "ppt", "odt", "ods", "odp", "rtf", "rtfd",
        "csv", "tsv", "epub", "pages", "numbers", "key", "txt", "md",
    ]

    public static func clase(de url: URL) -> ClaseDeDocumento {
        clase(deNombre: url.lastPathComponent)
    }

    public static func clase(deNombre nombre: String) -> ClaseDeDocumento {
        let ext = (nombre as NSString).pathExtension.lowercased()
        if ext == "pdf" { return .pdf }
        if office.contains(ext) { return .officePaginado }
        if vistaRapida.contains(ext) { return .vistaRapida }
        return .otro
    }
}

/// **Una hoja vista en pantalla**: su rectangulo en la vista (con la Y hacia
/// abajo) y las cuentas para ir y volver de las unidades de la tinta.
public struct GeometriaHoja: Equatable, Sendable {
    /// El ancho de una hoja en unidades de la tinta (`ANCHO_HOJA`).
    public static let ancho: CGFloat = 1400

    /// La hoja en la vista (Y hacia abajo).
    public var rect: CGRect

    public init(rect: CGRect) {
        self.rect = rect
    }

    /// Puntos de la vista por unidad de la tinta.
    public var escala: CGFloat { rect.width / GeometriaHoja.ancho }

    /// El alto de la hoja en unidades de la tinta.
    public var alto: CGFloat {
        GeometriaHoja.ancho * rect.height / max(rect.width, 0.0001)
    }

    /// El alto en unidades de la tinta de una pagina de `ancho` x `alto`
    /// (puntos, ya girada).
    public static func altoDeHoja(ancho: CGFloat, alto: CGFloat) -> Float {
        Float(GeometriaHoja.ancho * alto / max(ancho, 0.0001))
    }

    /// De la vista a la tinta.
    public func aHoja(_ p: CGPoint) -> CGPoint {
        let k = escala
        guard k > 0 else { return .zero }
        return CGPoint(x: (p.x - rect.minX) / k, y: (p.y - rect.minY) / k)
    }

    /// De la tinta a la vista.
    public func aVista(_ p: CGPoint) -> CGPoint {
        CGPoint(x: rect.minX + p.x * escala, y: rect.minY + p.y * escala)
    }

    /// La transformacion que lleva la tinta a la vista (para pintar con CG).
    public var transformacion: CGAffineTransform {
        CGAffineTransform(translationX: rect.minX, y: rect.minY).scaledBy(x: escala, y: escala)
    }

    /// Los espacios para anotar a cada lado, en la vista (tres cuartos de la
    /// hoja por lado, `vista::espacios_del_pdf`).
    public func espacios(_ bits: UInt8) -> (izquierda: CGRect?, derecha: CGRect?) {
        let m = rect.width * 0.75
        let izq = bits & 1 != 0 ? CGRect(x: rect.minX - m, y: rect.minY, width: m, height: rect.height) : nil
        let der = bits & 2 != 0 ? CGRect(x: rect.maxX, y: rect.minY, width: m, height: rect.height) : nil
        return (izq, der)
    }

    // MARK: Marcadores (`pixpin_motor2d::marcas`: x fraccion del ancho, y
    // pagina mas fraccion del alto)

    /// Donde cae un marcador de la pagina `pagina` en esta hoja, o `nil` si
    /// es de otra pagina.
    public func punto(de marca: MarcaDocumento, pagina: Int) -> CGPoint? {
        guard marca.y.isFinite, Int(marca.y.rounded(.down)) == pagina else { return nil }
        let fy = min(max(marca.y - Double(pagina), 0), 1)
        let fx = min(max(marca.x, 0), 1)
        return CGPoint(x: rect.minX + CGFloat(fx) * rect.width, y: rect.minY + CGFloat(fy) * rect.height)
    }

    /// El sitio de un punto de la vista como lo guarda un marcador.
    public func sitio(de p: CGPoint, pagina: Int) -> (x: Double, y: Double) {
        let fx = min(max((p.x - rect.minX) / max(rect.width, 1), 0), 1)
        let fy = min(max((p.y - rect.minY) / max(rect.height, 1), 0), 1)
        return (Double(fx), Double(pagina) + Double(fy))
    }
}

/// Un marcador nuevo en la lista (`marcas::con`): si ya hay 24, se va el mas
/// viejo; la lista queda en orden de lectura (de arriba abajo).
public func conMarca(_ lista: [MarcaDocumento], x: Double, y: Double, emoji: String,
                     ahora: Int64) -> [MarcaDocumento] {
    var v = lista
    var id = ahora
    while v.contains(where: { $0.id == id }) { id += 1 }
    v.append(MarcaDocumento(id: id, x: x, y: y, emoji: emoji))
    while v.count > 24, let viejo = v.min(by: { $0.id < $1.id }) {
        v.removeAll { $0.id == viejo.id }
    }
    v.sort { $0.y != $1.y ? $0.y < $1.y : $0.x < $1.x }
    return v
}
