// **Sacar un documento del lector**: compartirlo y imprimirlo **con lo
// anotado** encima, como Windows (`anotado_del_adjunto.rs`,
// `marco_imprimir.rs`) y el Mac (`LectorDocumento.pdfConTinta`):
//
// - el PDF con la tinta quemada: cada hoja como esta y su tinta encima (en
//   las unidades de la hoja, 1400 de ancho y la Y hacia abajo), en una
//   carpeta temporal;
// - compartir va por la hoja del sistema (`UIActivityViewController`) con el
//   original y, si tiene tinta, «<nombre> (anotado).pdf»;
// - imprimir va por `UIPrintInteractionController`, con lo anotado.

import CoreGraphics
import PDFKit
import PixPinCore
import UIKit

@MainActor
enum SalidaDocumentoIPad {
    /// **El PDF con la tinta quemada**: cada hoja de `doc` con la tinta que
    /// diga `tinta` encima. `nil` si no se pudo.
    static func pdfConTinta(_ doc: PDFDocument, nombre: String,
                            tinta: (Int, PDFPage) -> LienzoLeido?) -> URL? {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("anotado-\(UUID().uuidString)")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let salida = dir.appendingPathComponent("\(nombre) (anotado).pdf")
        guard let ctx = CGContext(salida as CFURL, mediaBox: nil, nil) else { return nil }
        for i in 0 ..< doc.pageCount {
            guard let p = doc.page(at: i) else { continue }
            var caja = p.bounds(for: .cropBox)
            if p.rotation % 180 != 0 { caja = CGRect(x: 0, y: 0, width: caja.height, height: caja.width) }
            var marco = CGRect(origin: .zero, size: caja.size)
            ctx.beginPage(mediaBox: &marco)
            ctx.saveGState()
            p.draw(with: .cropBox, to: ctx)
            ctx.restoreGState()
            if let l = tinta(i, p), !l.figuras.isEmpty {
                let k = caja.width / GeometriaHoja.ancho
                ctx.saveGState()
                ctx.clip(to: marco)
                ctx.translateBy(x: 0, y: caja.height)
                ctx.scaleBy(x: k, y: -k)
                PintorTintaHoja(lienzo: l).pintar(en: ctx, limites: CGRect(x: 0, y: 0, width: GeometriaHoja.ancho,
                                                                            height: caja.height / k))
                ctx.restoreGState()
            }
            ctx.endPage()
        }
        ctx.closePDF()
        return FileManager.default.fileExists(atPath: salida.path) ? salida : nil
    }

    /// El controlador desde el que presentar algo encima de `vista`.
    static func presentador(desde vista: UIView) -> UIViewController? {
        var c = vista.window?.rootViewController
        while let otro = c?.presentedViewController { c = otro }
        return c
    }

    /// La hoja de compartir del sistema con `cosas` (ficheros o datos).
    /// `alAcabar` se llama al cerrarla (para borrar lo temporal).
    static func compartir(_ cosas: [Any], desde vista: UIView, alAcabar: (() -> Void)? = nil) {
        guard !cosas.isEmpty, let c = presentador(desde: vista) else { return }
        let hoja = UIActivityViewController(activityItems: cosas, applicationActivities: nil)
        hoja.completionWithItemsHandler = { _, _, _, _ in alAcabar?() }
        if let pop = hoja.popoverPresentationController {
            pop.sourceView = vista
            pop.sourceRect = CGRect(x: vista.bounds.maxX - 120, y: 8, width: 1, height: 1)
        }
        c.present(hoja, animated: true)
    }

    /// **Imprimir** un PDF (un fichero o sus datos) con el dialogo del
    /// sistema. `titulo`: el nombre del trabajo.
    static func imprimir(_ cosa: Any, titulo: String, desde vista: UIView) {
        let impresora = UIPrintInteractionController.shared
        let info = UIPrintInfo.printInfo()
        info.jobName = titulo
        info.outputType = .general
        impresora.printInfo = info
        impresora.printingItem = cosa
        _ = impresora.present(from: CGRect(x: vista.bounds.maxX - 120, y: 8, width: 1, height: 1), in: vista,
                              animated: true, completionHandler: nil)
    }

    /// Borra la carpeta temporal de un fichero hecho con `pdfConTinta`.
    static func olvidar(_ url: URL?) {
        guard let url, url.deletingLastPathComponent().lastPathComponent.hasPrefix("anotado-") else { return }
        try? FileManager.default.removeItem(at: url.deletingLastPathComponent())
    }
}
