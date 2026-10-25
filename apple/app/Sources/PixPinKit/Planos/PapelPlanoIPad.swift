// **El plano en papel** en el iPad (`plano_cad/imprimir.rs` y
// `plano_cad/exportar.rs` de Windows, `ImpresionPlano.swift` y
// `ExportarPlano.swift` del Mac): se pinta **en vectores** con Core Graphics
// (`CGPath`) desde los buferes que ya estan en la tarjeta
// (`MTLBuffer.contents()`, memoria compartida con la CPU), no una foto de la
// pantalla.
//
// - Imprimir (`UIPrintInteractionController` con un `UIPrintPageRenderer`):
//   cada marco en su hoja (sin marcos, el plano entero), con la lamina de la
//   norma ISO 5457 y el membrete de Windows (proyecto, lamina n/N, titulo,
//   dibujo, escala 1:n, fecha y unidades) o sin marco.
// - Exportar (la hoja de compartir del sistema): HTML que se mueve
//   (`Plano.paginaWeb`), PDF (una hoja por marco, el lado largo de un A3),
//   PNG y JPG (3200 pixeles), DXF (de un DWG, `Plano.exportarDxf`) y el
//   original.
//
// Lo que sale: rellenos, rayas (con el grosor de su capa), circulos y arcos
// exactos y las letras. Los sombreados con patron, como un tono suave de su
// color. Los colores, los del papel blanco (el 7 en negro y lo muy claro
// oscurecido). Encima, las cotas (naranja) y lo anotado con el lapiz.

import CoreGraphics
import CoreText
import Foundation
import Metal
import PixPinCore
import UIKit

/// **Lo que va encima del plano en papel**: las cotas (naranja, con su
/// medida en cada tramo) y lo anotado con el lapiz (la capa del motor del
/// lienzo, en su sitio por su encaje).
struct EncimaPapel {
    var cotas: [CotaPlano] = []
    /// El texto de una medida, con la unidad del plano.
    var medida: (Double) -> String = { String(format: "%.3f", $0) }
    var tinta: LienzoLeido?
    var encaje: EncajePlano?
}

/// **El plano en papel**, en cualquier `CGContext` (la impresora, un PDF,
/// una imagen): los buferes que ya estan en la tarjeta, en vectores.
enum PapelPlano {
    /// Pinta `region` del plano centrada y sin deformar en `destino` (Y hacia
    /// arriba, en puntos de papel), con lo de `encima`. El fondo no se pinta.
    /// Con `fija` (puntos de papel por unidad del dibujo), a esa escala.
    static func pintar(_ datos: PlanoEnTarjeta, visibles: Set<UInt32>, region r: CajaPlano, encima: EncimaPapel?,
                       en ctx: CGContext, destino: CGRect, fija: Double? = nil) {
        let w = r.x1 - r.x0, h = r.y1 - r.y0
        guard w > 0, h > 0, destino.width > 0, destino.height > 0 else { return }
        // Puntos de papel por unidad del dibujo, el plano centrado.
        let k = fija ?? min(Double(destino.width) / w, Double(destino.height) / h)
        guard k > 0, k.isFinite else { return }
        let dx = Double(destino.minX) + (Double(destino.width) - w * k) / 2
        let dy = Double(destino.minY) + (Double(destino.height) - h * k) / 2
        ctx.saveGState()
        defer { ctx.restoreGState() }
        ctx.clip(to: CGRect(x: dx, y: dy, width: w * k, height: h * k))
        ctx.setLineCap(.round)
        ctx.setLineJoin(.round)
        for g in datos.grupos where visibles.contains(g.ficha.indice) {
            let o = (g.ficha.origenX, g.ficha.origenY)
            ctx.saveGState()
            ctx.translateBy(x: dx + (o.0 - r.x0) * k, y: dy + (o.1 - r.y0) * k)
            ctx.scaleBy(x: k, y: k)
            let caja = CajaF(x0: Float(r.x0 - o.0), y0: Float(r.y0 - o.1), x1: Float(r.x1 - o.0), y1: Float(r.y1 - o.1))
            // Lo que en papel mide menos de un cuarto de punto no se ve.
            let minimo = Float(0.25 / k)
            // El grosor de la capa (centesimas de mm) en puntos; el normal, fino.
            let grosor = g.ficha.grosor > 0 ? Double(g.ficha.grosor) * 0.028346 : 0.25
            PintorPapel(ctx: ctx, grupo: g, caja: caja, minimo: minimo, ancho: max(grosor, 0.1) / k).pintar()
            ctx.restoreGState()
        }
        guard let encima else { return }
        let aPapel = { (p: PuntoPlano) in CGPoint(x: dx + (p.x - r.x0) * k, y: dy + (p.y - r.y0) * k) }
        // Lo anotado: x = cero_x + x_capa * u, y = cero_y - y_capa * u.
        if let tinta = encima.tinta, let e = encima.encaje, e.u > 0 {
            let a = e.u * k
            let t = CGAffineTransform(a: a, b: 0, c: 0, d: -a,
                                      tx: dx + (e.ceroX - r.x0) * k, ty: dy + (e.ceroY - r.y0) * k)
            ctx.saveGState()
            ctx.concatenate(t)
            PintorLienzo(lienzo: tinta).pintar(en: ctx, limites: destino.applying(t.inverted()))
            ctx.restoreGState()
        }
        // Las cotas, en naranja con su medida (del tamano del papel, sin escala).
        let naranja = CGColor(srgbRed: 0.88, green: 0.42, blue: 0, alpha: 1)
        for c in encima.cotas where c.puntos.count >= 2 {
            let q = c.puntos.map(aPapel)
            ctx.setStrokeColor(naranja)
            ctx.setLineWidth(0.8)
            ctx.addLines(between: q)
            ctx.strokePath()
            var total = 0.0
            for (i, par) in zip(c.puntos, c.puntos.dropFirst()).enumerated() {
                let d = hypot(par.1.x - par.0.x, par.1.y - par.0.y)
                total += d
                texto(encima.medida(d), en: CGPoint(x: (q[i].x + q[i + 1].x) / 2 + 2, y: (q[i].y + q[i + 1].y) / 2 + 2),
                      color: naranja, ctx: ctx)
            }
            if c.puntos.count > 2, let u = q.last {
                texto("Total \(encima.medida(total))", en: CGPoint(x: u.x + 4, y: u.y - 9), color: naranja, ctx: ctx)
            }
        }
    }

    /// Un texto pequeno con Core Text, en un contexto con la Y hacia arriba.
    static func texto(_ t: String, en p: CGPoint, tamano: CGFloat = 7, color: CGColor, ctx: CGContext) {
        let atributos: [NSAttributedString.Key: Any] = [
            NSAttributedString.Key(kCTFontAttributeName as String): CTFontCreateWithName("Helvetica" as CFString, tamano, nil),
            NSAttributedString.Key(kCTForegroundColorAttributeName as String): color,
        ]
        let linea = CTLineCreateWithAttributedString(NSAttributedString(string: t, attributes: atributos))
        ctx.saveGState()
        ctx.textMatrix = .identity
        ctx.textPosition = p
        CTLineDraw(linea, ctx)
        ctx.restoreGState()
    }

    /// Pinta `region` en `destino` de un contexto de UIKit (la Y hacia
    /// abajo: la impresora): se le da la vuelta a la Y solo para el plano.
    static func pintarEnUIKit(_ datos: PlanoEnTarjeta, visibles: Set<UInt32>, region: CajaPlano, encima: EncimaPapel?,
                              en ctx: CGContext, destino: CGRect, fija: Double? = nil) {
        ctx.saveGState()
        ctx.translateBy(x: 0, y: destino.minY + destino.maxY)
        ctx.scaleBy(x: 1, y: -1)
        pintar(datos, visibles: visibles, region: region, encima: encima, en: ctx, destino: destino, fija: fija)
        ctx.restoreGState()
    }
}

// MARK: - La lamina y el membrete (Windows: `plano_cad/imprimir.rs`)

/// **Lo que dice el membrete** (`Lamina` de Windows).
struct LaminaPlano: Equatable {
    var conMembrete = true
    var proyecto = ""
    var titulo = ""
    var autor = ""
    var fecha = ""
}

/// Las cuentas de la lamina, sin dibujar (para las pruebas).
enum CuentasLamina {
    /// Puntos de papel por milimetro.
    static let mm: Double = 72 / 25.4
    /// El margen del papel sin lamina: los 28 puntos del movil.
    static let margen: Double = 28

    /// Los milimetros que mide una unidad del plano (`$INSUNITS`).
    static func mmPorUnidad(_ unidades: UInt32) -> Double? {
        switch unidades {
        case 1: 25.4
        case 2: 304.8
        case 4: 1
        case 5: 10
        case 6: 1000
        case 7: 1_000_000
        case 14: 100
        default: nil
        }
    }

    /// Las escalas de siempre (1:n).
    static let escalas: [UInt32] = [
        1, 2, 5, 10, 20, 25, 50, 75, 100, 125, 150, 200, 250, 300, 400, 500, 750, 1000, 1250, 1500,
        2000, 2500, 5000, 7500, 10000, 12500, 15000, 20000, 25000, 50000, 75000, 100_000,
    ]

    /// La escala normal mas grande con que `anchoMm` x `altoMm` de la
    /// realidad caben en el papel (mm): la primera 1:n con n >= lo justo.
    static func escalaQueCabe(anchoMm: Double, altoMm: Double, papelAncho: Double, papelAlto: Double) -> UInt32? {
        guard papelAncho > 0, papelAlto > 0 else { return nil }
        let justa = max(anchoMm / papelAncho, altoMm / papelAlto)
        return escalas.first { Double($0) >= justa * 0.9999 }
    }

    /// La 1:n que cabe en `aw` x `ah` puntos y los puntos por unidad del plano.
    static func escala(unidades: UInt32, marco: CajaPlano, aw: Double, ah: Double) -> (n: UInt32, puntos: Double)? {
        guard let mmu = mmPorUnidad(unidades),
              let n = escalaQueCabe(anchoMm: (marco.x1 - marco.x0) * mmu, altoMm: (marco.y1 - marco.y0) * mmu,
                                    papelAncho: aw / mm, papelAlto: ah / mm) else { return nil }
        return (n, mmu / Double(n) * mm)
    }

    /// La unidad del plano por su `$INSUNITS` (`ventana::sufijo` de Windows).
    static func unidad(_ unidades: UInt32) -> String {
        switch unidades {
        case 1: "in"
        case 2: "ft"
        case 4: "mm"
        case 5: "cm"
        case 6: "m"
        case 7: "km"
        case 14: "dm"
        default: ""
        }
    }

    /// La fecha de hoy, «9/10/2026».
    static func hoy(_ fecha: Date = Date()) -> String {
        let c = Calendar(identifier: .gregorian).dateComponents([.day, .month, .year], from: fecha)
        return "\(c.day ?? 1)/\(c.month ?? 1)/\(c.year ?? 2026)"
    }

    /// El nombre del proyecto: el del proyecto de PixPin si el plano vive en
    /// uno (`proyectos/<id>/archivos/`), si no el de su carpeta.
    static func proyecto(de ruta: String) -> String {
        let carpeta = URL(fileURLWithPath: ruta).deletingLastPathComponent()
        if carpeta.lastPathComponent == "archivos" {
            let json = carpeta.deletingLastPathComponent().appendingPathComponent("proyecto.json")
            if let d = try? Data(contentsOf: json),
               let o = try? JSONSerialization.jsonObject(with: d) as? [String: Any],
               let n = o["nombre"] as? String {
                return n
            }
        }
        return carpeta.lastPathComponent
    }

    /// Las cajas de la hoja con lamina (puntos, Y hacia abajo): el recuadro,
    /// el membrete y lo que queda para el plano. `imprimible`: lo que la
    /// impresora alcanza (el recuadro no se sale).
    static func cajas(papel: CGRect, imprimible: CGRect) -> (recuadro: CGRect, membrete: CGRect, plano: CGRect, fila: CGFloat) {
        let w = Double(papel.width), h = Double(papel.height)
        let izq = min(20 * mm, w * 0.07), otro = min(10 * mm, w * 0.035)
        let bx0 = max(Double(papel.minX) + izq, Double(imprimible.minX))
        let by0 = max(Double(papel.minY) + otro, Double(imprimible.minY))
        let bx1 = min(Double(papel.maxX) - otro, Double(imprimible.maxX))
        let by1 = min(Double(papel.maxY) - otro, Double(imprimible.maxY))
        let anchoM = min(180 * mm, bx1 - bx0)
        let fila = min(9 * mm, (by1 - by0) * 0.06)
        let mx0 = bx1 - anchoM, my0 = by1 - fila * 4
        let aire = 4 * mm
        return (CGRect(x: bx0, y: by0, width: bx1 - bx0, height: by1 - by0),
                CGRect(x: mx0, y: my0, width: anchoM, height: fila * 4),
                CGRect(x: bx0 + aire, y: by0 + aire, width: bx1 - bx0 - 2 * aire, height: my0 - by0 - 2 * aire),
                CGFloat(fila))
    }
}

/// **Una hoja de papel del plano** (`hoja` de Windows) en un contexto de
/// UIKit (Y hacia abajo): con lamina, el recuadro, el membrete y el plano en
/// lo que queda (a la 1:n que quepa si el plano dice sus unidades); sin
/// ella, el plano con un margen.
enum HojaPlano {
    static func pintar(_ d: PlanoEnTarjeta, visibles: Set<UInt32>, marco: CajaPlano, encima: EncimaPapel?,
                       lamina: LaminaPlano, numero: Int, total: Int, papel: CGRect, imprimible: CGRect,
                       en ctx: CGContext) {
        guard lamina.conMembrete else {
            let m = CGFloat(CuentasLamina.margen)
            let caja = imprimible.intersection(papel.insetBy(dx: m, dy: m))
            PapelPlano.pintarEnUIKit(d, visibles: visibles, region: marco, encima: encima, en: ctx,
                                     destino: caja.isNull ? imprimible : caja)
            return
        }
        let c = CuentasLamina.cajas(papel: papel, imprimible: imprimible)
        let escala = CuentasLamina.escala(unidades: d.resumen.unidades, marco: marco,
                                          aw: Double(c.plano.width), ah: Double(c.plano.height))
        PapelPlano.pintarEnUIKit(d, visibles: visibles, region: marco, encima: encima, en: ctx, destino: c.plano,
                                 fija: escala?.puntos)
        let negro = UIColor.black.cgColor
        ctx.saveGState()
        defer { ctx.restoreGState() }
        ctx.setStrokeColor(negro)
        ctx.setLineWidth(1.2)
        ctx.stroke(c.recuadro)
        ctx.stroke(c.membrete)
        ctx.setLineWidth(0.5)
        let m = c.membrete, fila = c.fila
        for k in 1 ..< 4 {
            let y = m.minY + CGFloat(k) * fila
            ctx.move(to: CGPoint(x: m.minX, y: y))
            ctx.addLine(to: CGPoint(x: m.maxX, y: y))
        }
        let col = m.width * 0.72
        ctx.move(to: CGPoint(x: m.minX + col, y: m.minY))
        ctx.addLine(to: CGPoint(x: m.minX + col, y: m.minY + fila))
        let cuarto = m.width / 4
        for k in 1 ..< 4 {
            let x = m.minX + CGFloat(k) * cuarto
            ctx.move(to: CGPoint(x: x, y: m.minY + 2 * fila))
            ctx.addLine(to: CGPoint(x: x, y: m.maxY))
        }
        ctx.strokePath()
        let unidad = CuentasLamina.unidad(d.resumen.unidades)
        let esc = escala.map { "1:\($0.n)" } ?? "Sin escala"
        UIGraphicsPushContext(ctx)
        celda(m.minX, m.minY, col, "PROYECTO", lamina.proyecto, fila, fila: fila)
        celda(m.minX + col, m.minY, m.width - col, "LÁMINA", "\(numero) / \(total)", fila, fila: fila)
        celda(m.minX, m.minY + fila, m.width, "TÍTULO", lamina.titulo, fila * 2, fila: fila)
        celda(m.minX, m.minY + 3 * fila, cuarto, "DIBUJÓ", lamina.autor, fila, fila: fila)
        celda(m.minX + cuarto, m.minY + 3 * fila, cuarto, "ESCALA", esc, fila, fila: fila)
        celda(m.minX + 2 * cuarto, m.minY + 3 * fila, cuarto, "FECHA", lamina.fecha, fila, fila: fila)
        celda(m.minX + 3 * cuarto, m.minY + 3 * fila, cuarto, "UNIDADES", unidad.isEmpty ? "—" : unidad, fila, fila: fila)
        UIGraphicsPopContext()
    }

    /// Una celda del membrete: la etiqueta pequena arriba (gris) y el dato
    /// abajo, recortado con «…» si no cabe. Las letras van con la fila.
    private static func celda(_ x0: CGFloat, _ y0: CGFloat, _ ancho: CGFloat, _ etiqueta: String, _ valor: String,
                              _ alto: CGFloat, fila: CGFloat) {
        let mm = CGFloat(CuentasLamina.mm)
        let rotulo: [NSAttributedString.Key: Any] = [.font: UIFont.systemFont(ofSize: max(3, fila * 0.24)),
                                                     .foregroundColor: UIColor(white: 0.33, alpha: 1)]
        (etiqueta as NSString).draw(at: CGPoint(x: x0 + 1.5 * mm, y: y0 + 0.6 * mm), withAttributes: rotulo)
        let dato: [NSAttributedString.Key: Any] = [.font: UIFont.systemFont(ofSize: max(4, fila * 0.40)),
                                                   .foregroundColor: UIColor.black]
        var t = valor
        let cabe = ancho - 3 * mm
        while t.count > 1, (t as NSString).size(withAttributes: dato).width > cabe { t.removeLast() }
        if t != valor, !t.isEmpty {
            t.removeLast()
            t += "…"
        }
        let th = (t as NSString).size(withAttributes: dato).height
        (t as NSString).draw(at: CGPoint(x: x0 + 1.5 * mm, y: y0 + alto - th - 1 * mm), withAttributes: dato)
    }
}

/// Lo que se manda a la impresora: una hoja por marco.
final class ImpresoraPlano: UIPrintPageRenderer {
    let datos: PlanoEnTarjeta
    let visibles: Set<UInt32>
    let marcos: [CajaPlano]
    let encima: EncimaPapel?
    let lamina: LaminaPlano

    init(datos: PlanoEnTarjeta, visibles: Set<UInt32>, marcos: [CajaPlano], encima: EncimaPapel?, lamina: LaminaPlano) {
        self.datos = datos
        self.visibles = visibles
        self.marcos = marcos
        self.encima = encima
        self.lamina = lamina
        super.init()
    }

    override var numberOfPages: Int { marcos.count }

    override func drawPage(at indice: Int, in imprimible: CGRect) {
        guard indice < marcos.count, let ctx = UIGraphicsGetCurrentContext() else { return }
        HojaPlano.pintar(datos, visibles: visibles, marco: marcos[indice], encima: encima, lamina: lamina,
                         numero: indice + 1, total: marcos.count, papel: paperRect, imprimible: imprimible, en: ctx)
    }
}

// MARK: - Exportar

@MainActor
enum ExportarPlanoIPad {
    /// El lado largo de una hoja del PDF: un A3 (puntos), y su margen.
    static let ladoPdf: CGFloat = 1190.55
    static let margenPdf: CGFloat = 28
    /// El lado largo de la imagen, en pixeles, y su margen.
    static let ladoImagen: CGFloat = 3200
    static let margenImagen: CGFloat = 24

    /// Los formatos que se ofrecen (los de Windows: `exportable`).
    enum Formato: String, CaseIterable, Identifiable {
        case html, pdf, png, jpg, dxf, original
        var id: String { rawValue }

        var nombre: String {
            switch self {
            case .html: "HTML (se mueve y acerca)"
            case .pdf: "PDF (una hoja por marco)"
            case .png: "PNG"
            case .jpg: "JPG"
            case .dxf: "DXF"
            case .original: "El original"
            }
        }

        var simbolo: String {
            switch self {
            case .html: "globe"
            case .pdf: "doc.richtext"
            case .png: "photo"
            case .jpg: "photo.fill"
            case .dxf: "square.on.square.dashed"
            case .original: "doc"
            }
        }
    }

    /// El nombre de fichero sin lo que no deja el sistema.
    static func base(_ titulo: String) -> String {
        var b = (titulo as NSString).deletingPathExtension
            .components(separatedBy: CharacterSet(charactersIn: "/\\:?*\"<>|")).joined(separator: "-")
        if b.isEmpty { b = "Plano" }
        return b
    }

    /// **El PDF del plano**: una hoja por marco (sin marcos, `util`), cada
    /// una del tamano de su marco con el lado largo de un A3.
    static func pdf(_ d: PlanoEnTarjeta, visibles: Set<UInt32>, marcos: [CajaPlano], util: CajaPlano,
                    encima: EncimaPapel?) -> Data? {
        let hojas = (marcos.isEmpty ? [util] : marcos).filter { $0.x1 > $0.x0 && $0.y1 > $0.y0 }
        guard !hojas.isEmpty else { return nil }
        let datos = NSMutableData()
        guard let consumidor = CGDataConsumer(data: datos as CFMutableData) else { return nil }
        var caja = CGRect(x: 0, y: 0, width: ladoPdf, height: ladoPdf)
        guard let ctx = CGContext(consumer: consumidor, mediaBox: &caja, nil) else { return nil }
        for m in hojas {
            let tam = tamanoDeHoja(m, lado: ladoPdf, margen: margenPdf)
            var media = CGRect(origin: .zero, size: tam)
            ctx.beginPage(mediaBox: &media)
            PapelPlano.pintar(d, visibles: visibles, region: m, encima: encima, en: ctx,
                              destino: media.insetBy(dx: margenPdf, dy: margenPdf))
            ctx.endPage()
        }
        ctx.closePDF()
        return datos as Data
    }

    /// **El plano como imagen**, sobre blanco.
    static func imagen(_ d: PlanoEnTarjeta, visibles: Set<UInt32>, region: CajaPlano, encima: EncimaPapel?) -> CGImage? {
        guard region.x1 > region.x0, region.y1 > region.y0 else { return nil }
        let tam = tamanoDeHoja(region, lado: ladoImagen, margen: margenImagen)
        let w = Int(tam.width.rounded()), h = Int(tam.height.rounded())
        guard w > 0, h > 0, let espacio = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0, space: espacio,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        ctx.setFillColor(CGColor(gray: 1, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
        PapelPlano.pintar(d, visibles: visibles, region: region, encima: encima, en: ctx,
                          destino: CGRect(x: 0, y: 0, width: w, height: h).insetBy(dx: margenImagen, dy: margenImagen))
        return ctx.makeImage()
    }

    /// El tamano de la hoja de `region` con `lado` de lado largo, margen
    /// incluido (`HojaPdf::de` de Windows).
    nonisolated static func tamanoDeHoja(_ region: CajaPlano, lado: CGFloat, margen: CGFloat) -> CGSize {
        let mw = max(region.x1 - region.x0, 1e-9), mh = max(region.y1 - region.y0, 1e-9)
        let s = Double(lado - 2 * margen) / max(mw, mh)
        return CGSize(width: mw * s + 2 * Double(margen), height: mh * s + 2 * Double(margen))
    }

    /// Una carpeta temporal para lo que se comparte (se borra al cerrar la hoja).
    static func carpetaTemporal() -> URL {
        let c = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-plano-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: c, withIntermediateDirectories: true)
        return c
    }

    /// **Abre la hoja de compartir** del sistema con `urls` (Guardar en
    /// Archivos, AirDrop, Mail...). Al cerrarla se borra `carpeta`.
    static func compartir(_ urls: [URL], carpeta: URL?, desde vista: UIView?) {
        guard !urls.isEmpty, let arriba = ArribaUIKit.controladorDeArriba() else { return }
        let hoja = UIActivityViewController(activityItems: urls, applicationActivities: nil)
        hoja.completionWithItemsHandler = { _, _, _, _ in
            if let carpeta { try? FileManager.default.removeItem(at: carpeta) }
        }
        if let pop = hoja.popoverPresentationController {
            let v = vista ?? arriba.view
            pop.sourceView = v
            pop.sourceRect = CGRect(x: (v?.bounds.maxX ?? 0) - 60, y: 60, width: 1, height: 1)
        }
        arriba.present(hoja, animated: true)
    }

    /// Una foto (un `CGImage`) como PNG o JPG.
    static func codificar(_ img: CGImage, jpg: Bool) -> Data? {
        let u = UIImage(cgImage: img)
        return jpg ? u.jpegData(compressionQuality: 0.9) : u.pngData()
    }
}

// MARK: - Los buferes en papel

/// Un trazado que se va llenando mientras el color no cambie.
private struct Lote {
    var color: UInt32?
    var path = CGMutablePath()
    var vacio = true

    mutating func cambiar(_ c: UInt32, _ soltar: (CGPath, UInt32) -> Void) {
        if color != c {
            terminar(soltar)
            color = c
        }
    }

    mutating func terminar(_ soltar: (CGPath, UInt32) -> Void) {
        if !vacio, let c = color { soltar(path, c) }
        path = CGMutablePath()
        vacio = true
    }
}

/// `desde ..< desde + cuantos`, sin salirse de `n` (nunca al reves).
private func rango(_ desde: UInt32, _ cuantos: UInt32, _ n: Int) -> Range<Int> {
    let a = min(Int(desde), n)
    return a ..< max(a, min(Int(desde) + Int(cuantos), n))
}

/// Los buferes de un grupo en papel.
private struct PintorPapel {
    let ctx: CGContext
    let grupo: GrupoMetal
    let caja: CajaF
    let minimo: Float
    let ancho: Double

    private func ver<T>(_ b: MTLBuffer?, _: T.Type) -> UnsafeBufferPointer<T> {
        guard let b else { return UnsafeBufferPointer(start: nil, count: 0) }
        let n = b.length / MemoryLayout<T>.stride
        return UnsafeBufferPointer(start: b.contents().bindMemory(to: T.self, capacity: n), count: n)
    }

    private func rellenar(_ p: CGPath, _ c: UInt32, alfa: Double = 1) {
        let k = ColorCAD.enPapel(c)
        ctx.addPath(p)
        ctx.setFillColor(CGColor(srgbRed: k.r, green: k.g, blue: k.b, alpha: k.a * alfa))
        ctx.fillPath()
    }

    private func trazar(_ p: CGPath, _ c: UInt32) {
        ctx.addPath(p)
        ctx.setStrokeColor(ColorCAD.enPapel(c).cg)
        ctx.setLineWidth(ancho)
        ctx.strokePath()
    }

    func pintar() {
        let vs = ver(grupo.vertices, VerticeCAD.self)
        // Rellenos.
        let tri = ver(grupo.triangulos, UInt32.self)
        var lote = Lote()
        for (d, n) in tramosVisibles(grupo.tramosTriangulos, en: caja, minimo: minimo) {
            var i = Int(d)
            while i + 2 < Int(d + n), i + 2 < tri.count {
                let a = Int(tri[i]), b = Int(tri[i + 1]), c = Int(tri[i + 2])
                i += 3
                guard a < vs.count, b < vs.count, c < vs.count else { continue }
                lote.cambiar(vs[a].color) { rellenar($0, $1) }
                lote.path.addLines(between: [punto(vs[a]), punto(vs[b]), punto(vs[c])])
                lote.path.closeSubpath()
                lote.vacio = false
            }
        }
        lote.terminar { rellenar($0, $1) }
        // Sombreados: un tono de su color.
        let vt = ver(grupo.verticesTrama, VerticeTramaCAD.self)
        let tt = ver(grupo.triangulosTrama, UInt32.self)
        for (d, n) in tramosVisibles(grupo.tramosTrama, en: caja, minimo: minimo) {
            var i = Int(d)
            while i + 2 < Int(d + n), i + 2 < tt.count {
                let a = Int(tt[i]), b = Int(tt[i + 1]), c = Int(tt[i + 2])
                i += 3
                guard a < vt.count, b < vt.count, c < vt.count else { continue }
                lote.cambiar(vt[a].color) { rellenar($0, $1, alfa: 0.25) }
                lote.path.addLines(between: [CGPoint(x: Double(vt[a].x), y: Double(vt[a].y)),
                                             CGPoint(x: Double(vt[b].x), y: Double(vt[b].y)),
                                             CGPoint(x: Double(vt[c].x), y: Double(vt[c].y))])
                lote.path.closeSubpath()
                lote.vacio = false
            }
        }
        lote.terminar { rellenar($0, $1, alfa: 0.25) }
        // Rayas: tiras cortadas por 0xFFFFFFFF.
        let li = ver(grupo.lineas, UInt32.self)
        for (d, n) in tramosVisibles(grupo.tramosLineas, en: caja, minimo: minimo) {
            var empezada = false
            for i in rango(d, n, li.count) {
                let v = li[i]
                guard v != PasosCAD.corte, Int(v) < vs.count else {
                    empezada = false
                    continue
                }
                let p = punto(vs[Int(v)])
                if empezada {
                    lote.path.addLine(to: p)
                } else {
                    lote.cambiar(vs[Int(v)].color) { trazar($0, $1) }
                    lote.path.move(to: p)
                    empezada = true
                    lote.vacio = false
                }
            }
        }
        lote.terminar { trazar($0, $1) }
        // Circulos y arcos, exactos.
        let ar = ver(grupo.arcos, ArcoCAD.self)
        for (d, n) in tramosVisibles(grupo.tramosArcos, en: caja, minimo: minimo) {
            for i in rango(d, n, ar.count) {
                let a = ar[i]
                lote.cambiar(a.color) { trazar($0, $1) }
                let c = CGPoint(x: Double(a.centroX), y: Double(a.centroY))
                if a.barrido >= 6.2831 {
                    let r = Double(a.radio)
                    lote.path.addEllipse(in: CGRect(x: c.x - r, y: c.y - r, width: 2 * r, height: 2 * r))
                } else {
                    let desde = CGPoint(x: c.x + Double(a.radio) * cos(Double(a.inicio)), y: c.y + Double(a.radio) * sin(Double(a.inicio)))
                    lote.path.move(to: desde)
                    lote.path.addArc(center: c, radius: CGFloat(a.radio), startAngle: CGFloat(a.inicio),
                                     endAngle: CGFloat(a.inicio + a.barrido), clockwise: false)
                }
                lote.vacio = false
            }
        }
        lote.terminar { trazar($0, $1) }
        pintarLetras()
    }

    private func punto(_ v: VerticeCAD) -> CGPoint { CGPoint(x: Double(v.x), y: Double(v.y)) }

    /// Cada letra es una instancia de su glifo (triangulos, o pares de
    /// rayas si es de una fuente SHX), colocada con su matriz.
    private func pintarLetras() {
        let le = ver(grupo.letras, LetraCAD.self)
        let gl = ver(grupo.glifos, UInt32.self)
        let ma = ver(grupo.malla, Float.self)
        var rellenos = Lote()
        var rayas = Lote()
        for t in grupo.tramosLetras where t.tamano >= minimo * 3 && caja.corta(t) {
            for i in rango(t.desde, t.cuantos, le.count) {
                let l = le[i]
                let g = Int(l.glifo) * 2
                guard g + 1 < gl.count else { continue }
                let desde = Int(gl[g]), cuantos = Int(gl[g + 1] & ~PasosCAD.glifoDeRayas)
                let deRayas = gl[g + 1] & PasosCAD.glifoDeRayas != 0
                guard (desde + cuantos) * 2 <= ma.count else { continue }
                func p(_ k: Int) -> CGPoint {
                    let ex = Double(ma[(desde + k) * 2]), ey = Double(ma[(desde + k) * 2 + 1])
                    return CGPoint(x: Double(l.x) + Double(l.m0) * ex + Double(l.m1) * ey,
                                   y: Double(l.y) + Double(l.m2) * ex + Double(l.m3) * ey)
                }
                if deRayas {
                    rayas.cambiar(l.color) { trazar($0, $1) }
                    var k = 0
                    while k + 1 < cuantos {
                        rayas.path.move(to: p(k))
                        rayas.path.addLine(to: p(k + 1))
                        k += 2
                    }
                    rayas.vacio = false
                } else {
                    rellenos.cambiar(l.color) { rellenar($0, $1) }
                    var k = 0
                    while k + 2 < cuantos {
                        rellenos.path.addLines(between: [p(k), p(k + 1), p(k + 2)])
                        rellenos.path.closeSubpath()
                        k += 3
                    }
                    rellenos.vacio = false
                }
            }
        }
        rellenos.terminar { rellenar($0, $1) }
        rayas.terminar { trazar($0, $1) }
    }
}
