// **Sacar el dibujo** en el iPad: compartir e imprimir, como el grupo
// `sacar` de la barra de Windows. Cada marco es una hoja (`hojasParaExportar`
// del nucleo, lo mismo que Windows y el Mac); cada hoja sale como PNG con
// su papel, pintada con el pintor de siempre (`PintorDeHojas`).
//
// Compartir va por la hoja del sistema (`UIActivityViewController`: Mensajes,
// Mail, AirDrop, Guardar en Archivos...); imprimir, por la del sistema
// (`UIPrintInteractionController`).

import PixPinCore
import UIKit

extension ModeloEditorLienzo {
    /// Las hojas del dibujo ya pintadas (a 2 pixeles por unidad, como la
    /// exportacion de Windows). Vacio si no hay nada.
    func hojasPintadas() -> [(nombre: String, imagen: CGImage)] {
        _ = editor.terminarTexto()
        let version = editor.version()
        return editor.hojasParaExportar(que: .todo).compactMap { h in
            let caja = CGRect(x: CGFloat(h.x0), y: CGFloat(h.y0), width: CGFloat(h.x1 - h.x0),
                              height: CGFloat(h.y1 - h.y0))
            guard caja.width > 0, caja.height > 0,
                  let img = PintorDeHojas.imagen(h.plano, caja: caja, escala: 2, transparente: false,
                                                 tapados: tapados, version: version,
                                                 fotos: { [weak self] in self?.foto($0) })
            else { return nil }
            return (h.nombre, img)
        }
    }

    /// Un nombre de fichero sin lo que no deja el sistema.
    private func nombreDeFichero(_ nombre: String, _ i: Int, de total: Int) -> String {
        var base = (nombre.isEmpty ? titulo : nombre)
            .components(separatedBy: CharacterSet(charactersIn: "/\\:?*\"<>|")).joined(separator: "-")
        if base.isEmpty { base = "Lienzo" }
        if total > 1, nombre.isEmpty { base += " \(i + 1)" }
        return base + ".png"
    }

    /// **Compartir…**: las hojas como PNG con su nombre.
    func compartir() {
        desplegado = nil
        guard let vista else { return }
        let hojas = hojasPintadas()
        guard !hojas.isEmpty else {
            CajetinesLienzo.avisar("Compartir", "No hay nada dibujado que compartir.", desde: vista)
            return
        }
        let carpeta = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-compartir-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        var urls: [URL] = []
        for (i, h) in hojas.enumerated() {
            guard let png = PintorDeHojas.png(h.imagen) else { continue }
            let url = carpeta.appendingPathComponent(nombreDeFichero(h.nombre, i, de: hojas.count))
            if (try? png.write(to: url)) != nil { urls.append(url) }
        }
        guard !urls.isEmpty else { return }
        let hoja = UIActivityViewController(activityItems: urls, applicationActivities: nil)
        hoja.completionWithItemsHandler = { _, _, _, _ in
            try? FileManager.default.removeItem(at: carpeta)
        }
        if let pop = hoja.popoverPresentationController {
            pop.sourceView = vista
            pop.sourceRect = CGRect(x: vista.bounds.midX, y: 60, width: 1, height: 1)
        }
        CajetinesLienzo.controlador(desde: vista)?.present(hoja, animated: true)
    }

    /// **Imprimir…**: una hoja de papel por hoja del dibujo.
    func imprimir() {
        desplegado = nil
        guard let vista else { return }
        let hojas = hojasPintadas()
        guard !hojas.isEmpty else {
            CajetinesLienzo.avisar("Imprimir", "No hay nada dibujado que imprimir.", desde: vista)
            return
        }
        let impresora = UIPrintInteractionController.shared
        let info = UIPrintInfo.printInfo()
        info.jobName = titulo
        info.outputType = .general
        impresora.printInfo = info
        impresora.printingItems = hojas.map { UIImage(cgImage: $0.imagen) }
        _ = impresora.present(from: CGRect(x: vista.bounds.midX, y: 60, width: 1, height: 1), in: vista,
                              animated: true, completionHandler: nil)
    }
}
