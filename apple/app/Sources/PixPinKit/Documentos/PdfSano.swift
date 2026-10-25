// **El PDF de un proyecto, aunque este roto** (Windows,
// `pdf_del_proyecto.rs`; el movil v0.81, «la portada repara el PDF roto»).
//
// Cada proyecto con PDF guarda dos copias: el documento (`pdfOrigen`) y una
// **copia limpia** (`pdfLimpio`), las dos junto a su `proyecto.json`
// (`archivos/doc-<t>.pdf` y `archivos/limpio-<t>.pdf`). Si el documento llega
// hecho de trozos (dos escritores a la vez en el movil, un corte a mitad de
// un pegado), no se abre. Lo que se hace es lo de Windows: la primera vez se
// **rehace desde la copia limpia** (se escribe al lado y luego se cambia el
// nombre, de un tiron) y se abre el propio documento; si ya se rehizo o no
// se puede escribir, se abre la copia limpia. Lo anotado no va dentro del
// PDF (va en los lienzos de sus hojas): copiar la limpia encima no pierde
// nada.

import Foundation
import PDFKit

enum PdfSano {
    /// Los que ya se rehicieron en esta sesion: si vuelven a fallar se abre
    /// la copia, sin rehacer otra vez.
    nonisolated(unsafe) private static var rehechos: Set<String> = []
    private static let cerrojo = NSLock()

    /// **La copia limpia de un documento**: la del proyecto cuyo `pdfOrigen`
    /// es este fichero (`<proyecto>/archivos/doc.pdf` con
    /// `<proyecto>/proyecto.json`). `nil` si no es de un proyecto o no hay.
    static func copiaLimpia(de documento: URL) -> URL? {
        let carpeta = documento.deletingLastPathComponent().deletingLastPathComponent()
        let ficha = carpeta.appendingPathComponent("proyecto.json")
        guard let d = try? Data(contentsOf: ficha) else { return nil }
        return copiaLimpia(de: documento, carpeta: carpeta, proyecto: d)
    }

    /// Lo puro de `copiaLimpia`: con el `proyecto.json` ya leido.
    static func copiaLimpia(de documento: URL, carpeta: URL, proyecto: Data) -> URL? {
        guard let o = try? JSONSerialization.jsonObject(with: proyecto) as? [String: Any],
              let origen = o["pdfOrigen"] as? String, let limpio = o["pdfLimpio"] as? String,
              !origen.isEmpty, !limpio.isEmpty else { return nil }
        let delOrigen = carpeta.appendingPathComponent(origen).standardizedFileURL
        guard delOrigen.resolvingSymlinksInPath().path == documento.standardizedFileURL.resolvingSymlinksInPath().path
        else { return nil }
        let copia = carpeta.appendingPathComponent(limpio).standardizedFileURL
        guard copia.path != delOrigen.path, FileManager.default.fileExists(atPath: copia.path) else { return nil }
        return copia
    }

    /// **Que abrir cuando el documento no se deja**: el propio documento ya
    /// rehecho desde su copia limpia (la primera vez) o la copia limpia.
    /// `nil` sin copia o con la copia rota tambien. Fuera del hilo principal.
    static func reparar(_ documento: URL) -> URL? {
        guard let limpio = copiaLimpia(de: documento), let bytes = try? Data(contentsOf: limpio),
              let doc = PDFDocument(data: bytes), doc.pageCount > 0 else { return nil }
        cerrojo.lock()
        let ya = rehechos.contains(documento.path)
        rehechos.insert(documento.path)
        cerrojo.unlock()
        if ya { return limpio }
        // Al lado y luego el nombre: el documento no puede quedarse a medias.
        let temporal = documento.appendingPathExtension("sano")
        do {
            try bytes.write(to: temporal)
            _ = try FileManager.default.replaceItemAt(documento, withItemAt: temporal)
            NSLog("PixPin: documento del proyecto roto, rehecho desde su copia limpia: %@", documento.path)
            return documento
        } catch {
            try? FileManager.default.removeItem(at: temporal)
            return limpio
        }
    }

    /// Si un PDF se abre con al menos una hoja.
    static func seAbre(_ url: URL) -> Bool {
        (PDFDocument(url: url)?.pageCount ?? 0) > 0
    }
}
