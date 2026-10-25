// **Lo compartible, ya preparado** (D1): las paginas de lo que se comparte,
// como se pinta cada una y lo que va entero. Es `compartir.rs` de Windows:
// cada sitio que ofrece «Compartir…» dice que es (`CosaCompartida`) y de
// aqui en adelante todo va igual, salga de donde salga: el mismo PDF, la
// misma imagen y la misma pagina web.
//
// En Windows todo se convierte en hojas del motor. Aqui cada pagina sabe
// pintarse en un `CGContext` (con el pintor de siempre: `PintorDeHojas` en
// un dibujo, PDFKit en una pagina de PDF, el texto con su letra) y dar su
// SVG; de eso salen el PDF, el PNG, el JPG, el SVG y la pagina web, que es
// la del movil y la de Windows (`paginaWebCompartida` del nucleo).
//
// Lo que no es paginas va **entero**: el fichero original, el `.pixpin` del
// proyecto, el `.excalidraw` del lienzo o el texto de una nota.
//
// Traido del Mac (`Compartir/PreparadoCompartir.swift`) casi tal cual: en el
// iPad la letra es `UIFont` y el texto se pinta con UIKit
// (`UIGraphicsPushContext`) en vez de `NSGraphicsContext`.

import CoreText
import ImageIO
import PDFKit
import PixPinCore
import UIKit
import UniformTypeIdentifiers

/// **Lo que se comparte.** Cada sitio que ofrece «Compartir…» dice que es.
enum CosaCompartida {
    /// Uno o varios mensajes de un proyecto del chat: la burbuja o lo
    /// marcado.
    case mensajes(chat: String, titulo: String, [MensajeChat])
    /// Uno o varios proyectos enteros de la lista.
    case proyectos([FichaChat])
    /// Un documento que se esta leyendo (un PDF, un Word...).
    case documento(URL)
    /// Ficheros sueltos (un pin, lo que sea): tal cual, y en paginas lo que
    /// se pueda (fotos, un PDF solo).
    case ficheros(titulo: String, [URL])
    /// Una imagen sin fichero (la captura de un pin).
    case imagen(titulo: String, CGImage)
    /// Ya preparado por quien lo comparte, con sus formatos propios (un
    /// plano: HTML, PDF, PNG y DXF; `exportable` de Windows).
    case preparado(PreparadoCompartir)
}

/// Una pagina de lo que se comparte: su fila en la hoja y como se pinta.
struct PiezaCompartir {
    var pagina: PaginaCompartir
    /// Lo que mide, en puntos (la pagina del PDF).
    var tamano: CGSize
    /// El papel, `#rrggbb`.
    var fondo: String = "#ffffff"
    /// Los pixeles por punto del PNG y el JPG: 2 en un dibujo o un texto, 1
    /// en una foto (que ya trae los suyos).
    var escalaImagen: CGFloat = 2
    /// Pinta la pagina llenando `destino`, en un contexto con la Y hacia
    /// abajo (el de `PintorDeHojas`).
    var pintar: @MainActor (CGContext, CGRect) -> Void
    /// La pagina como documento SVG.
    var svg: @MainActor () -> String?
}

/// Lo que va entero.
enum EnteroCompartir {
    /// Ficheros que ya existen, tal cual (los originales).
    case ficheros([URL])
    /// Un fichero que se escribe al pedirlo (puede pesar: un `.pixpin`).
    case escrito(nombre: String, hacer: @MainActor () async throws -> Data)
    /// Varios ficheros que se escriben al pedirlos en la carpeta dada.
    case lote(@MainActor (URL) async throws -> [URL])
}

struct ExtraCompartir {
    var formato: FormatoCompartir
    var que: EnteroCompartir
}

enum ErrorCompartir: Error, LocalizedError {
    case nada
    case sinPaginas
    case noSePudo(String)

    var errorDescription: String? {
        switch self {
        case .nada: "No hay nada que compartir aquí."
        case .sinPaginas: "Marca alguna página"
        case let .noSePudo(que): "No se pudo preparar: \(que)"
        }
    }
}

// MARK: - Lo preparado

@MainActor
final class PreparadoCompartir {
    var titulo: String
    private(set) var piezas: [PiezaCompartir] = []
    var extras: [ExtraCompartir] = []
    /// Lo marcado al abrir; `nil`, todo.
    private(set) var marcadas: Set<String>?
    /// Si lo natural es mandar el original (una foto, un PDF): va el primero.
    var originalPrimero = false
    /// El lienzo que viaja dentro de la pagina web (solo cuando lo
    /// compartido es UN lienzo).
    var excalidraw: String?
    /// Lleva alguna foto anotada (con lo dibujado encima, fusionado).
    var conAnotadas = false
    /// El formato puesto al abrir la hoja (`nil`: el primero).
    var formatoInicial: String?

    init(titulo: String) { self.titulo = titulo }

    func anadir(_ p: PiezaCompartir, marcada: Bool = true) {
        piezas.append(p)
        if marcada { marcar(p.pagina.clave) }
    }

    func marcar(_ clave: String) {
        var m = marcadas ?? []
        m.insert(clave)
        marcadas = m
    }

    func pieza(_ clave: String) -> PiezaCompartir? { piezas.first { $0.pagina.clave == clave } }

    func cambiarDetalle(_ i: Int, _ detalle: String) { piezas[i].pagina.detalle = detalle }

    /// **Lo que ve la hoja** (`compartible` de Windows): los formatos de
    /// paginas (HTML, PDF, PNG, JPG, SVG) y despues lo que va entero, o al
    /// reves si lo natural es mandar el original.
    func compartible() -> Compartible {
        var dePaginas: [FormatoCompartir] = []
        if !piezas.isEmpty {
            dePaginas = [
                FormatoCompartir(id: IdFormato.web, nombre: "HTML", cuantas: .varias, simbolo: "globe"),
                FormatoCompartir(id: IdFormato.pdf, nombre: "PDF", cuantas: .varias, simbolo: "doc.richtext"),
                FormatoCompartir(id: IdFormato.png, nombre: "PNG", cuantas: .una, simbolo: "photo"),
                FormatoCompartir(id: IdFormato.jpg, nombre: "JPG", cuantas: .una, simbolo: "photo.fill"),
                FormatoCompartir(id: IdFormato.svg, nombre: "SVG", cuantas: .una, simbolo: "scribble"),
            ]
        }
        let enteros = extras.map(\.formato)
        return Compartible(titulo: titulo, paginas: piezas.map(\.pagina),
                           formatos: originalPrimero ? enteros + dePaginas : dePaginas + enteros,
                           marcadas: marcadas)
    }

    // MARK: Hacer los ficheros

    /// El nombre de fichero de lo compartido.
    var base: String { nombreDeFicheroCompartido(titulo: titulo) }

    /// El nombre de un fichero de una sola pagina: el de todo, y el de la
    /// pagina detras si hay varias.
    func nombreDePagina(_ p: PiezaCompartir) -> String {
        guard piezas.count > 1, p.pagina.nombre != titulo else { return base }
        return "\(base) - \(nombreDeFicheroCompartido(titulo: p.pagina.nombre))"
    }

    /// **Hace el fichero** de `formato` con las paginas `claves` en
    /// `carpeta` (`generar` de Windows). Lo que va entero tal cual no se
    /// copia: se da su ruta.
    func generar(_ formato: String, claves: [String], en carpeta: URL) async throws -> [URL] {
        try FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        if let extra = extras.first(where: { $0.formato.id == formato }) {
            switch extra.que {
            case let .ficheros(urls):
                let hay = urls.filter { FileManager.default.fileExists(atPath: $0.path) }
                guard !hay.isEmpty else { throw ErrorCompartir.nada }
                return hay
            case let .escrito(nombre, hacer):
                let url = carpeta.appendingPathComponent(nombre)
                try await hacer().write(to: url, options: .atomic)
                return [url]
            case let .lote(hacer):
                let urls = try await hacer(carpeta)
                guard !urls.isEmpty else { throw ErrorCompartir.nada }
                return urls
            }
        }
        let elegidas = claves.compactMap { pieza($0) }
        guard !elegidas.isEmpty else { throw ErrorCompartir.sinPaginas }
        switch formato {
        case IdFormato.web:
            let hojas = elegidas.compactMap { p in
                p.svg().map { HojaParaWeb(nombre: p.pagina.nombre, svg: $0, fondo: p.fondo) }
            }
            guard let html = paginaWebCompartida(titulo: titulo, hojas: hojas, excalidraw: excalidraw) else {
                throw ErrorCompartir.nada
            }
            let url = carpeta.appendingPathComponent(base).appendingPathExtension("html")
            try Data(html.utf8).write(to: url, options: .atomic)
            return [url]
        case IdFormato.pdf:
            guard let datos = Self.pdf(elegidas) else { throw ErrorCompartir.noSePudo("PDF") }
            let url = carpeta.appendingPathComponent(base).appendingPathExtension("pdf")
            try datos.write(to: url, options: .atomic)
            return [url]
        case IdFormato.png, IdFormato.jpg:
            let p = elegidas[0]
            let jpg = formato == IdFormato.jpg
            guard let img = Self.imagen(p, opaca: jpg),
                  let datos = Self.codificar(img, jpg ? .jpeg : .png) else {
                throw ErrorCompartir.noSePudo(formato.uppercased())
            }
            let url = carpeta.appendingPathComponent(nombreDePagina(p)).appendingPathExtension(formato)
            try datos.write(to: url, options: .atomic)
            return [url]
        case IdFormato.svg:
            let p = elegidas[0]
            guard let svg = p.svg() else { throw ErrorCompartir.noSePudo("SVG") }
            let url = carpeta.appendingPathComponent(nombreDePagina(p)).appendingPathExtension("svg")
            try Data(svg.utf8).write(to: url, options: .atomic)
            return [url]
        default:
            throw ErrorCompartir.noSePudo(formato)
        }
    }

    /// Un PDF con una pagina por pieza, del tamano de cada una (vectorial:
    /// lo pinta el mismo pintor de la pantalla).
    static func pdf(_ piezas: [PiezaCompartir]) -> Data? {
        let datos = NSMutableData()
        guard !piezas.isEmpty, let consumidor = CGDataConsumer(data: datos as CFMutableData),
              let ctx = CGContext(consumer: consumidor, mediaBox: nil, nil) else { return nil }
        for p in piezas {
            var caja = CGRect(origin: .zero, size: p.tamano)
            ctx.beginPage(mediaBox: &caja)
            ctx.saveGState()
            ctx.translateBy(x: 0, y: caja.height)
            ctx.scaleBy(x: 1, y: -1)
            p.pintar(ctx, caja)
            ctx.restoreGState()
            ctx.endPage()
        }
        ctx.closePDF()
        return datos as Data
    }

    /// La pieza como imagen, a su escala (como mucho 8192 de lado, el
    /// `LADO_MAXIMO_PNG` de Windows). `opaca`: con el papel debajo (JPG).
    static func imagen(_ p: PiezaCompartir, opaca: Bool) -> CGImage? {
        let k = min(p.escalaImagen, 8192 / max(p.tamano.width, p.tamano.height, 1))
        let w = max(1, Int((p.tamano.width * k).rounded())), h = max(1, Int((p.tamano.height * k).rounded()))
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        let todo = CGRect(x: 0, y: 0, width: w, height: h)
        if opaca {
            ctx.setFillColor(ColorHex.cg(p.fondo))
            ctx.fill(todo)
        }
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: 1, y: -1)
        p.pintar(ctx, todo)
        return ctx.makeImage()
    }

    static func codificar(_ img: CGImage, _ tipo: UTType) -> Data? {
        let datos = NSMutableData()
        guard let d = CGImageDestinationCreateWithData(datos, tipo.identifier as CFString, 1, nil) else { return nil }
        let opciones = tipo == .jpeg ? [kCGImageDestinationLossyCompressionQuality: 0.9] as CFDictionary : nil
        CGImageDestinationAddImage(d, img, opciones)
        return CGImageDestinationFinalize(d) ? datos as Data : nil
    }
}

/// Un color `#rrggbb`.
enum ColorHex {
    static func cg(_ hex: String) -> CGColor {
        let s = hex.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
        guard s.count == 6, let v = UInt32(s, radix: 16) else { return CGColor(gray: 1, alpha: 1) }
        return CGColor(srgbRed: CGFloat((v >> 16) & 0xff) / 255, green: CGFloat((v >> 8) & 0xff) / 255,
                       blue: CGFloat(v & 0xff) / 255, alpha: 1)
    }
}

// MARK: - Las piezas de cada clase

@MainActor
enum PiezasCompartir {
    /// **Una foto**: la imagen entera (con su giro EXIF), a sus pixeles.
    static func foto(_ img: CGImage, clave: String, nombre: String, detalle: String,
                     datos: @escaping () -> (Data, String)?) -> PiezaCompartir {
        let tamano = CGSize(width: img.width, height: img.height)
        return PiezaCompartir(
            pagina: PaginaCompartir(clave: clave, nombre: nombre, detalle: detalle),
            tamano: tamano, escalaImagen: 1,
            pintar: { ctx, destino in dibujar(img, en: ctx, destino) },
            svg: {
                guard let (bytes, mime) = datos() else { return nil }
                return svgDeImagen(bytes, mime: mime, tamano: tamano)
            })
    }

    /// Una foto de un fichero; `nil` si no se lee como imagen.
    static func foto(_ url: URL, clave: String, nombre: String, detalle: String) -> PiezaCompartir? {
        guard let img = leerImagen(url) else { return nil }
        return foto(img, clave: clave, nombre: nombre, detalle: detalle) {
            // Tal cual si es un PNG o un JPEG (pesa lo que pesa); si no, PNG.
            if let d = try? Data(contentsOf: url), let mime = mimeDeImagen(d) { return (d, mime) }
            return PreparadoCompartir.codificar(img, .png).map { ($0, "image/png") }
        }
    }

    /// Una imagen ya en memoria (la captura de un pin).
    static func foto(_ img: CGImage, clave: String, nombre: String) -> PiezaCompartir {
        foto(img, clave: clave, nombre: nombre, detalle: "Foto") {
            PreparadoCompartir.codificar(img, .png).map { ($0, "image/png") }
        }
    }

    /// **Una hoja de un dibujo** (el lienzo entero o un marco), pintada con
    /// el pintor del lienzo y con el SVG del motor.
    static func dibujo(_ h: HojaDeDibujo, pagina: PaginaCompartir, fotos: CacheDeFotos) -> PiezaCompartir {
        let caja = CGRect(x: CGFloat(h.x0), y: CGFloat(h.y0), width: CGFloat(h.x1 - h.x0), height: CGFloat(h.y1 - h.y0))
        let svg = h.svg
        return PiezaCompartir(
            pagina: pagina, tamano: caja.size, fondo: h.fondo,
            pintar: { ctx, destino in
                PintorDeHojas.pintar(h.plano, caja: caja, en: ctx, destino: destino, transparente: false,
                                     tapados: nil, version: 0, fotos: { fotos.imagen($0) })
            },
            svg: { svg })
    }

    /// **Una pagina de un PDF**, vectorial en el PDF y como imagen en el SVG
    /// y la pagina web.
    static func paginaPdf(_ pagina: PDFPage, clave: String, nombre: String, detalle: String,
                          nivel: Int) -> PiezaCompartir {
        let caja = pagina.bounds(for: .cropBox)
        let giro = ((pagina.rotation % 360) + 360) % 360
        let tamano = giro == 90 || giro == 270 ? CGSize(width: caja.height, height: caja.width) : caja.size
        let pintar: @MainActor (CGContext, CGRect) -> Void = { ctx, destino in
            ctx.saveGState()
            ctx.setFillColor(CGColor(gray: 1, alpha: 1))
            ctx.fill(destino)
            // PDFKit pinta con la Y hacia arriba.
            ctx.translateBy(x: destino.minX, y: destino.maxY)
            ctx.scaleBy(x: destino.width / tamano.width, y: -destino.height / tamano.height)
            pagina.draw(with: .cropBox, to: ctx)
            ctx.restoreGState()
        }
        var pieza = PiezaCompartir(
            pagina: PaginaCompartir(clave: clave, nombre: nombre, detalle: detalle, nivel: nivel),
            tamano: tamano, pintar: pintar, svg: { nil })
        let copia = pieza
        pieza.svg = {
            guard let img = PreparadoCompartir.imagen(copia, opaca: true),
                  let png = PreparadoCompartir.codificar(img, .jpeg) else { return nil }
            return svgDeImagen(png, mime: "image/jpeg", tamano: tamano)
        }
        return pieza
    }

    // MARK: Texto

    /// La pagina de texto: A4 en puntos, con el margen y la letra de
    /// Windows (`hojas_de_texto`: 794 x 1123 a 96 ppp, margen 64, letra 14
    /// y titulo 22, aqui pasados a puntos).
    static let a4 = CGSize(width: 595.28, height: 841.89)
    static let margen: CGFloat = 48
    static let letra: CGFloat = 10.5
    static let letraTitulo: CGFloat = 16.5
    static let interlinea: CGFloat = 1.35

    /// Una linea ya colocada: su texto, su altura desde arriba y su letra.
    struct Linea: Equatable {
        var texto: String
        var y: CGFloat
        var tam: CGFloat
    }

    static func fuente(_ tam: CGFloat) -> UIFont {
        tam >= letraTitulo ? .boldSystemFont(ofSize: tam) : .systemFont(ofSize: tam)
    }

    /// Parte `texto` en lineas que caben en `ancho` con la letra `tam`.
    static func partir(_ texto: String, tam: CGFloat, ancho: CGFloat) -> [String] {
        var v: [String] = []
        let atributos: [NSAttributedString.Key: Any] = [.font: fuente(tam)]
        for parrafo in texto.components(separatedBy: .newlines) {
            if parrafo.isEmpty {
                v.append("")
                continue
            }
            let a = NSAttributedString(string: parrafo, attributes: atributos)
            let tipografo = CTTypesetterCreateWithAttributedString(a)
            let largo = (parrafo as NSString).length
            var desde = 0
            while desde < largo {
                let n = max(1, CTTypesetterSuggestLineBreak(tipografo, desde, Double(ancho)))
                let trozo = (parrafo as NSString).substring(with: NSRange(location: desde, length: min(n, largo - desde)))
                v.append(trozo.trimmingCharacters(in: .newlines))
                desde += n
            }
        }
        return v
    }

    /// **Un texto en paginas A4**: el titulo grande arriba y el resto en
    /// lineas, pagina nueva cuando no cabe (`hojas_de_texto`).
    static func paginasDeTexto(titulo: String?, texto: String) -> [[Linea]] {
        let ancho = a4.width - 2 * margen
        var lineas: [(String, CGFloat)] = []
        if let titulo, !titulo.isEmpty {
            lineas += partir(titulo, tam: letraTitulo, ancho: ancho).map { ($0, letraTitulo) }
            lineas.append(("", letra * 0.6))
        }
        lineas += partir(texto, tam: letra, ancho: ancho).map { ($0, letra) }
        var paginas: [[Linea]] = []
        var actual: [Linea] = []
        var y = margen
        for (l, tam) in lineas {
            let alto = tam * interlinea
            if y + alto > a4.height - margen, !actual.isEmpty {
                paginas.append(actual)
                actual = []
                y = margen
            }
            if !l.trimmingCharacters(in: .whitespaces).isEmpty { actual.append(Linea(texto: l, y: y, tam: tam)) }
            y += alto
        }
        if !actual.isEmpty || paginas.isEmpty { paginas.append(actual) }
        return paginas
    }

    /// Las piezas de un texto: la primera con la clave y el nombre del
    /// mensaje; las demas, «Página 2», sangradas.
    static func texto(clave: String, nombre: String, detalle: String, titulo: String?,
                      texto: String) -> [PiezaCompartir] {
        paginasDeTexto(titulo: titulo, texto: texto).enumerated().map { i, lineas in
            let pagina = i == 0
                ? PaginaCompartir(clave: clave, nombre: nombre, detalle: detalle)
                : PaginaCompartir(clave: "\(clave)|p\(i + 1)", nombre: "Página \(i + 1)", detalle: "", nivel: 1)
            return PiezaCompartir(
                pagina: pagina, tamano: a4,
                pintar: { ctx, destino in pintarTexto(lineas, en: ctx, destino) },
                svg: { svgDeTexto(lineas) })
        }
    }

    static func pintarTexto(_ lineas: [Linea], en ctx: CGContext, _ destino: CGRect) {
        ctx.saveGState()
        ctx.setFillColor(CGColor(gray: 1, alpha: 1))
        ctx.fill(destino)
        ctx.translateBy(x: destino.minX, y: destino.minY)
        ctx.scaleBy(x: destino.width / a4.width, y: destino.height / a4.height)
        // UIKit pinta con la Y hacia abajo, que es como ya viene `ctx`.
        UIGraphicsPushContext(ctx)
        for l in lineas {
            (l.texto as NSString).draw(at: CGPoint(x: margen, y: l.y), withAttributes: [
                .font: fuente(l.tam), .foregroundColor: UIColor(white: 0.1, alpha: 1),
            ])
        }
        UIGraphicsPopContext()
        ctx.restoreGState()
    }

    static func svgDeTexto(_ lineas: [Linea]) -> String {
        var s = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
        s += "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"\(num(a4.width))\" height=\"\(num(a4.height))\" "
        s += "viewBox=\"0 0 \(num(a4.width)) \(num(a4.height))\">\n"
        s += "<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\n"
        for l in lineas {
            let base = l.y + fuente(l.tam).ascender
            let peso = l.tam >= letraTitulo ? " font-weight=\"bold\"" : ""
            s += "<text x=\"\(num(margen))\" y=\"\(num(base))\" font-size=\"\(num(l.tam))\"\(peso) "
            s += "font-family=\"-apple-system, 'Segoe UI', Helvetica, Arial, sans-serif\" fill=\"#1a1a1a\" "
            s += "xml:space=\"preserve\">\(escaparXml(l.texto))</text>\n"
        }
        return s + "</svg>\n"
    }

    // MARK: Ayudas

    static func num(_ v: CGFloat) -> String {
        let r = (v * 100).rounded() / 100
        return r == r.rounded() ? String(Int(r)) : String(format: "%.2f", r)
    }

    static func escaparXml(_ s: String) -> String {
        s.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;").replacingOccurrences(of: "\"", with: "&quot;")
    }

    static func svgDeImagen(_ bytes: Data, mime: String, tamano: CGSize) -> String {
        let w = num(tamano.width), h = num(tamano.height)
        return "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
            + "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" "
            + "width=\"\(w)\" height=\"\(h)\" viewBox=\"0 0 \(w) \(h)\">\n"
            + "<image x=\"0\" y=\"0\" width=\"\(w)\" height=\"\(h)\" preserveAspectRatio=\"none\" "
            + "href=\"data:\(mime);base64,\(bytes.base64EncodedString())\"/>\n</svg>\n"
    }

    /// El tipo de una imagen por sus primeros bytes (lo que admite un
    /// `data:`), como `mime_de` del nucleo.
    static func mimeDeImagen(_ d: Data) -> String? {
        let b = [UInt8](d.prefix(12))
        if b.starts(with: [0x89, 0x50, 0x4E, 0x47]) { return "image/png" }
        if b.starts(with: [0xFF, 0xD8, 0xFF]) { return "image/jpeg" }
        if b.starts(with: Array("GIF8".utf8)) { return "image/gif" }
        if b.count >= 12, b.starts(with: Array("RIFF".utf8)), Array(b[8 ..< 12]) == Array("WEBP".utf8) { return "image/webp" }
        return nil
    }

    /// Una imagen de un fichero, derecha (con su giro EXIF) y como mucho de
    /// 8192 de lado.
    static func leerImagen(_ url: URL) -> CGImage? {
        guard let fuente = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
        let opciones: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: 8192,
        ]
        return CGImageSourceCreateThumbnailAtIndex(fuente, 0, opciones as CFDictionary)
    }

    /// Pinta `img` llenando `destino` en un contexto con la Y hacia abajo.
    static func dibujar(_ img: CGImage, en ctx: CGContext, _ destino: CGRect) {
        ctx.saveGState()
        ctx.translateBy(x: destino.minX, y: destino.maxY)
        ctx.scaleBy(x: 1, y: -1)
        ctx.interpolationQuality = .high
        ctx.draw(img, in: CGRect(origin: .zero, size: destino.size))
        ctx.restoreGState()
    }

    static func esImagen(_ url: URL) -> Bool {
        UTType(filenameExtension: url.pathExtension)?.conforms(to: .image) == true
    }

    static func esPdf(_ url: URL) -> Bool { url.pathExtension.lowercased() == "pdf" }

    /// Lo ancho de una imagen en pixeles, ya girada, sin leerla entera.
    nonisolated static func anchoEnPixeles(_ url: URL) -> CGFloat? {
        guard let fuente = CGImageSourceCreateWithURL(url as CFURL, nil),
              let p = CGImageSourceCopyPropertiesAtIndex(fuente, 0, nil) as? [CFString: Any],
              let w = p[kCGImagePropertyPixelWidth] as? Int, let h = p[kCGImagePropertyPixelHeight] as? Int else {
            return nil
        }
        let giro = p[kCGImagePropertyOrientation] as? Int ?? 1
        return CGFloat(giro >= 5 ? h : w)
    }
}

/// Las fotos de un dibujo, leidas una vez al pedirlas.
@MainActor
final class CacheDeFotos {
    private let rutas: [UInt64: URL]
    private var leidas: [UInt64: CGImage] = [:]

    init(_ rutas: [UInt64: URL]) { self.rutas = rutas }

    func imagen(_ id: UInt64) -> CGImage? {
        if let i = leidas[id] { return i }
        guard let url = rutas[id], let i = PiezasCompartir.leerImagen(url) else { return nil }
        leidas[id] = i
        return i
    }
}
