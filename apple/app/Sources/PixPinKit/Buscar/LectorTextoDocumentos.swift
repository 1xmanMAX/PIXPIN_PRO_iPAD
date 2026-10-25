// **Lee el texto de los documentos de los chats en segundo plano**, para
// que «Buscar en todo» encuentre lo que pone dentro (D8). Como
// `LectorTextoGaleria` con el OCR de las capturas: el nucleo
// (`documentos_sin_texto`) lee el solo lo que sabe (Word, PowerPoint,
// Excel, OpenDocument, EPUB, HTML, Markdown y texto) y devuelve los PDF,
// que se leen aqui con PDFKit y se apuntan con `poner_texto_de_documento`.
// Lo leido queda en `cache/textos/`: la segunda vez no se relee nada.

import Foundation
import PDFKit
import PixPinCore

@MainActor
final class LectorTextoDocumentos {
    private let nucleo: Nucleo
    private let alLeer: () -> Void
    private var tarea: Task<Void, Never>?

    /// Lo mas que se lee de un PDF, en paginas (un catalogo de mil paginas
    /// no se busca entero).
    nonisolated static let topeDePaginas = 400

    init(nucleo: Nucleo, alLeer: @escaping () -> Void) {
        self.nucleo = nucleo
        self.alLeer = alLeer
    }

    func despertar() {
        guard tarea == nil else { return }
        tarea = Task { [weak self] in
            await self?.vuelta()
            self?.tarea = nil
        }
    }

    func parar() {
        tarea?.cancel()
        tarea = nil
    }

    private func vuelta() async {
        let nucleo = self.nucleo
        // Los ya intentados: si uno no se puede apuntar, no se vuelve a el.
        var intentados: Set<String> = []
        while !Task.isCancelled {
            guard let p = try? await Task.detached(priority: .utility, operation: {
                try nucleo.documentosSinTexto(cuantos: 8)
            }).value else { return }
            let pdfs = p.pdfs.filter { !intentados.contains($0.ruta) }
            if pdfs.isEmpty, p.leidos == 0 { return }
            for d in pdfs {
                if Task.isCancelled { return }
                intentados.insert(d.ruta)
                let url = URL(fileURLWithPath: d.ruta)
                let texto = await Task.detached(priority: .utility) { LectorTextoDocumentos.leer(url) }.value
                try? await Task.detached(priority: .utility) {
                    try nucleo.ponerTextoDeDocumento(ruta: d.ruta, texto: texto ?? "")
                }.value
                try? await Task.sleep(for: .milliseconds(40))
            }
            alLeer()
        }
    }

    /// El texto de un PDF, pagina a pagina. `nil` si no se pudo abrir (se
    /// apunta vacio: no se reintenta en cada vuelta).
    nonisolated static func leer(_ url: URL) -> String? {
        guard let doc = PDFDocument(url: url) else { return nil }
        var partes: [String] = []
        for i in 0 ..< min(doc.pageCount, topeDePaginas) {
            if let t = doc.page(at: i)?.string, !t.isEmpty { partes.append(t) }
        }
        return partes.joined(separator: "\n")
    }
}
