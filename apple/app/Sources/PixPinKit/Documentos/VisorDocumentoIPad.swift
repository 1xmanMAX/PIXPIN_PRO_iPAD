// **El visor de un documento**: elige el que toca por la extension, como el
// doble clic de Windows:
//
// - PDF y PowerPoint (los que pagina `pixpin-office`): el lector con tinta
//   (`LectorPDFIPad`); el PowerPoint, ademas, se presenta.
// - Word y EPUB: la lectura seguida con su tinta (`VisorReflujoIPad`), como
//   el Mac y Windows; un Word se puede ver tambien paginado (el lector).
// - Un CSV o un TSV suelto (fuera de un chat): la tabla de solo ver
//   (`TablaSueltaIPad`).
// - Excel, OpenDocument y CSV de un chat: la hoja de calculo del nucleo
//   (`HojaCalculoIPad`), que guarda lo editado como tablas del chat. Un
//   Excel u OpenDocument suelto, la misma hoja (`LibroSueltoIPad`, con
//   `Nucleo.abrirLibro`): se edita, pero el archivo no cambia (lo editado
//   sale como una copia en Excel).
// - HTML: `VisorHtmlIPad`. Markdown y texto: el editor de las notas, que
//   escribe en su fichero (`VisorMarkdownIPad`).
// - Lo demas (`.doc`, `.ppt`, `.xls`, RTF...): Vista Rapida (`VistaRapida`
//   de `Raiz/`).
//
// Las firmas publicas (`VisorDocumentoIPad`, `EditorNotaIPad`,
// `HojaCalculoIPad`) las usa `Raiz/RaizIPad.swift`: no se cambian.

import PixPinCore
import SwiftUI

/// Que visor le toca a un documento. Pura: se prueba sola.
public enum VisorDeDocumento: Equatable, Sendable {
    case lector, reflujo, hoja, tablaSuelta, html, texto, vistaRapida

    /// Los que se leen seguidos (`VisoresReflujo.seLee` del Mac).
    public static let seguidos: Set<String> = ["docx", "docm", "epub"]

    /// Los libros que abre `HojaDeCalculo` (el `.xls` antiguo no se lee).
    public static let libros: Set<String> = ["xlsx", "xlsm", "xltx", "ods", "csv", "tsv"]

    /// `deUnChat`: si viene de un mensaje (la hoja solo abre libros del chat).
    public static func para(_ url: URL, deUnChat: Bool) -> VisorDeDocumento {
        let ext = url.pathExtension.lowercased()
        if seguidos.contains(ext) { return .reflujo }
        switch TiposDeDocumento.clase(de: url) {
        case .pdf, .officePaginado: return .lector
        default: break
        }
        if libros.contains(ext) {
            if deUnChat { return .hoja }
            return ext == "csv" || ext == "tsv" ? .tablaSuelta : .hoja
        }
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
        case .reflujo:
            ReflujoOPaginadoIPad(url: url, titulo: titulo, nucleo: almacen.nucleo)
        case .tablaSuelta:
            TablaSueltaIPad(url: url, titulo: titulo)
        case .hoja:
            if let chat, let mensaje {
                HojaCalculoIPad(almacen: almacen, chat: chat, mensaje: mensaje, respaldo: url)
            } else {
                LibroSueltoIPad(url: url, titulo: titulo, nucleo: almacen.nucleo)
            }
        case .html:
            VisorHtmlIPad(url: url, titulo: titulo, nucleo: almacen.nucleo)
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

/// Un Word o un libro: seguido; un Word, tambien paginado como Office (el
/// lector con tinta por hojas), como «Leer» y «Leer y anotar» del Mac.
private struct ReflujoOPaginadoIPad: View {
    let url: URL
    let titulo: String
    let nucleo: Nucleo
    @State private var paginado = false

    private var esWord: Bool { TiposDeDocumento.clase(de: url) == .officePaginado }

    var body: some View {
        if paginado || (esWord && !seAbreEnElVisor(nombre: url.lastPathComponent)) {
            LectorPDFIPad(url: url, titulo: titulo, nucleo: nucleo)
        } else {
            VisorReflujoIPad(url: url, titulo: titulo, nucleo: nucleo,
                             alPaginar: esWord ? { paginado = true } : nil)
        }
    }
}

/// **Un libro suelto** (un Excel u OpenDocument de la app Archivos, fuera de
/// un chat): la misma hoja que los del chat, con su tinta, sus marcadores y
/// sus graficos. Se edita, pero el archivo no cambia (como el visor de hojas
/// de Windows); lo editado sale con «Compartir una copia en Excel». Si no se
/// lee (un `.xls` renombrado, uno roto), Vista Rapida.
struct LibroSueltoIPad: View {
    let url: URL
    let titulo: String
    let nucleo: Nucleo
    @State private var modelo: ModeloHojaIPad?
    @State private var fallo = false

    var body: some View {
        Group {
            if let modelo {
                VistaHojaIPad(modelo: modelo, titulo: titulo, nucleo: nucleo, fichero: url)
            } else if fallo {
                VistaRapida(url: url)
            } else {
                VStack(spacing: 0) {
                    BarraVisorIPad(tipo: "Hoja de cálculo", icono: "tablecells", titulo: titulo)
                    Spacer()
                    ProgressView()
                    Spacer()
                }
            }
        }
        .task {
            guard modelo == nil else { return }
            let nucleo = self.nucleo, ruta = url.path
            let hecho = await Task.detached(priority: .userInitiated) { () -> HojaDeCalculo? in
                try? nucleo.abrirLibro(ruta: ruta)
            }.value
            if let hecho { modelo = ModeloHojaIPad(hoja: hecho) } else { fallo = true }
        }
    }
}
