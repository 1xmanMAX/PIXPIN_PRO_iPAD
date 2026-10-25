// **El visor de un documento**: elige el que toca por la extension, como el
// doble clic de Windows:
//
// - PDF, Word y PowerPoint (los que pagina `pixpin-office`): el lector con
//   tinta (`LectorPDFIPad`).
// - Excel, OpenDocument y CSV de un chat: la hoja de calculo del nucleo
//   (`HojaCalculoIPad`), que guarda lo editado como tablas del chat.
// - HTML: `VisorHtmlIPad`. Markdown y texto: el editor de las notas, que
//   escribe en su fichero (`VisorMarkdownIPad`).
// - Lo demas (`.doc`, `.ppt`, `.xls`, RTF, EPUB, un Excel suelto...): Vista
//   Rapida (`VistaRapida` de `Raiz/`).
//
// Las firmas publicas (`VisorDocumentoIPad`, `EditorNotaIPad`,
// `HojaCalculoIPad`) las usa `Raiz/RaizIPad.swift`: no se cambian.

import PixPinCore
import SwiftUI

/// Que visor le toca a un documento. Pura: se prueba sola.
public enum VisorDeDocumento: Equatable, Sendable {
    case lector, hoja, html, texto, vistaRapida

    /// Los libros que abre `HojaDeCalculo` (el `.xls` antiguo no se lee).
    public static let libros: Set<String> = ["xlsx", "xlsm", "xltx", "ods", "csv", "tsv"]

    /// `deUnChat`: si viene de un mensaje (la hoja solo abre libros del chat).
    public static func para(_ url: URL, deUnChat: Bool) -> VisorDeDocumento {
        let ext = url.pathExtension.lowercased()
        switch TiposDeDocumento.clase(de: url) {
        case .pdf, .officePaginado: return .lector
        default: break
        }
        if libros.contains(ext) { return deUnChat ? .hoja : .vistaRapida }
        switch ext {
        case "html", "htm", "xhtml": return .html
        case "md", "markdown", "txt", "text": return .texto
        default: return .vistaRapida
        }
    }
}

public struct VisorDocumentoIPad: View {
    let url: URL
    let titulo: String
    let almacen: Almacen
    let chat: String?
    let mensaje: MensajeChat?

    public init(url: URL, titulo: String, almacen: Almacen, chat: String?, mensaje: MensajeChat?) {
        self.url = url
        self.titulo = titulo
        self.almacen = almacen
        self.chat = chat
        self.mensaje = mensaje
    }

    public var body: some View {
        switch VisorDeDocumento.para(url, deUnChat: chat != nil && mensaje != nil) {
        case .lector:
            LectorPDFIPad(url: url, titulo: titulo, nucleo: almacen.nucleo)
        case .hoja:
            if let chat, let mensaje {
                HojaCalculoIPad(almacen: almacen, chat: chat, mensaje: mensaje, respaldo: url)
            } else {
                VistaRapida(url: url)
            }
        case .html:
            VisorHtmlIPad(url: url, titulo: titulo)
        case .texto:
            TextoSueltoIPad(url: url, titulo: titulo)
        case .vistaRapida:
            VistaRapida(url: url)
        }
    }
}

/// Lee un `.md` o un `.txt` (fuera del hilo principal) y lo abre en el
/// editor; si no es texto, Vista Rapida.
private struct TextoSueltoIPad: View {
    let url: URL
    let titulo: String
    @State private var texto: String?
    @State private var fallo = false

    var body: some View {
        Group {
            if let texto {
                VisorMarkdownIPad(url: url, titulo: titulo, texto: texto)
            } else if fallo {
                VistaRapida(url: url)
            } else {
                VStack {
                    BarraVisorIPad(tipo: "Texto", icono: "doc.text", titulo: titulo)
                    Spacer()
                    ProgressView()
                    Spacer()
                }
            }
        }
        .task {
            let url = self.url
            let leido = await Task.detached(priority: .userInitiated) { () -> String? in
                guard let d = try? Data(contentsOf: url) else { return nil }
                return String(data: d, encoding: .utf8) ?? String(data: d, encoding: .isoLatin1)
            }.value
            if let leido { texto = leido } else { fallo = true }
        }
    }
}
