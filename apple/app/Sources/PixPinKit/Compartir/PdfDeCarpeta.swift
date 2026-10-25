// **Una carpeta de paginas como PDF** (`compartir/carpeta.rs` de Windows,
// 26-oct-2026; del Mac, `Compartir/PdfDeCarpeta.swift`; el usuario: «al
// exportar, sale como un PDF: una pagina por elemento, y de cada lienzo solo
// lo que va sobre la plantilla o sobre el marco; si no hay ninguno, todo lo
// dibujado»).
//
// No es otro exportador: son las piezas de compartir de siempre
// (`PrepararCompartir`) con la regla del cuaderno para los lienzos —sus
// hojas salen de `Nucleo.hojasDeCarpeta` (`exportar::hojas_de_cuaderno`, el
// mismo motor que Windows)— y una sola pagina por foto. El PDF lo escribe el
// mismo pintor (`PreparadoCompartir.pdf`) y se le pone el titulo de cada
// pagina («Día 1», «Día 2») en el indice de marcadores, como Windows.
//
// En el iPad, «Exportar PDF» lo deja en un fichero temporal con el nombre de
// la carpeta y abre la hoja de compartir del sistema (Archivos, AirDrop,
// imprimir), en vez del dialogo de guardar del Mac.

import PDFKit
import PixPinCore
import UIKit

/// Lo comun de las carpetas que piden la hoja de compartir y el PDF.
@MainActor
enum CarpetasDelChat {
    /// Si el mensaje es una carpeta de paginas.
    nonisolated static func esCarpeta(_ m: MensajeChat) -> Bool {
        m.clase == "MINIAPP" && m.miniapp == "carpeta"
    }

    /// La carpeta con sus paginas, leida fuera del hilo principal.
    static func leer(_ id: String, chat: String, almacen: Almacen) async -> CarpetaDePaginas? {
        let nucleo = almacen.nucleo
        return await Task.detached(priority: .userInitiated) {
            (try? nucleo.carpetaDePaginas(chat: chat, id: id)) ?? nil
        }.value
    }
}

@MainActor
enum PdfDeCarpeta {
    /// Lo que se dice si no sale nada (`carpeta-no-exporto`).
    static let noSeExporto = "No se pudo exportar: no hay ninguna página con algo dentro"

    /// Lo preparado de una carpeta y, de cada pagina de la carpeta, en que
    /// pieza empieza y con que titulo (el indice del PDF).
    struct Preparada {
        var preparado: PreparadoCompartir
        var indice: [(pieza: Int, titulo: String)]
    }

    /// **Las piezas de una carpeta**, en su orden.
    static func preparar(_ carpeta: CarpetaDePaginas, chat: String, almacen: Almacen) async -> Preparada {
        let p = PreparadoCompartir(titulo: carpeta.nombre)
        var indice: [(pieza: Int, titulo: String)] = []
        for pagina in carpeta.paginas {
            let antes = p.piezas.count
            await anadirPagina(p, pagina, chat: chat, almacen: almacen)
            if p.piezas.count > antes { indice.append((antes, pagina.titulo)) }
        }
        return Preparada(preparado: p, indice: indice)
    }

    /// Las piezas de una pagina: un lienzo, por su plantilla; una foto, una
    /// sola pagina; lo demas, como al compartirlo entre otros mensajes.
    static func anadirPagina(_ p: PreparadoCompartir, _ pagina: PaginaDeCarpeta, chat: String,
                             almacen: Almacen) async {
        let m = pagina.mensaje
        switch m.clase {
        case "DIBUJO", "PAGINA":
            guard let referencia = m.referencia, !referencia.isEmpty else { return }
            await anadirLienzo(p, chat: chat, referencia: referencia, clave: m.id, titulo: pagina.titulo,
                               detalle: m.clase == "PAGINA" ? "Página del PDF" : "Lienzo", almacen: almacen)
        case "IMAGEN":
            guard let url = await PrepararCompartir.fichero(m, chat: chat, almacen: almacen),
                  let f = PiezasCompartir.foto(url, clave: m.id, nombre: pagina.titulo, detalle: "Foto") else { return }
            p.anadir(f)
        default:
            let url = await PrepararCompartir.fichero(m, chat: chat, almacen: almacen)
            await PrepararCompartir.anadirMensaje(p, m, chat: chat, fichero: url, solo: false, almacen: almacen)
        }
    }

    /// Las hojas de un lienzo con la regla del cuaderno: la de su plantilla,
    /// si no cada marco, si no todo.
    static func anadirLienzo(_ p: PreparadoCompartir, chat: String, referencia: String, clave: String,
                             titulo: String, detalle: String, almacen: Almacen) async {
        let nucleo = almacen.nucleo
        let leido = await Task.detached(priority: .userInitiated) { () -> ([HojaDeDibujo], [UInt64: URL])? in
            guard let hojas = try? nucleo.hojasDeCarpeta(chat: chat, dibujo: referencia), !hojas.isEmpty else { return nil }
            var fotos: [UInt64: URL] = [:]
            if let lienzo = try? nucleo.abrirLienzo(chat: chat, dibujo: referencia) {
                for f in (try? nucleo.imagenesDelLienzo(chat: chat, lienzo: lienzo)) ?? [] {
                    fotos[f.idObjeto] = URL(fileURLWithPath: f.ruta)
                }
            }
            return (hojas, fotos)
        }.value
        guard let (hojas, rutas) = leido else { return }
        let fotos = CacheDeFotos(rutas)
        for (k, h) in hojas.enumerated() {
            let pagina = k == 0
                ? PaginaCompartir(clave: clave, nombre: titulo, detalle: detalle)
                : PaginaCompartir(clave: "\(clave)|\(k)", nombre: "\(titulo) · \(k + 1)", detalle: "", nivel: 1)
            p.anadir(PiezasCompartir.dibujo(h, pagina: pagina, fotos: fotos))
        }
    }

    /// **El PDF de la carpeta**, con los titulos en el indice. `nil` si no
    /// sale ninguna pagina («no hay ninguna página con algo dentro»).
    static func pdf(_ c: Preparada) -> Data? {
        guard !c.preparado.piezas.isEmpty, let datos = PreparadoCompartir.pdf(c.preparado.piezas) else { return nil }
        return conIndice(datos, c.indice)
    }

    /// El PDF con un marcador por pagina de la carpeta, que lleva a su
    /// primera hoja (`de_piezas_con_indice` de Windows).
    static func conIndice(_ datos: Data, _ indice: [(pieza: Int, titulo: String)]) -> Data {
        guard !indice.isEmpty, let doc = PDFDocument(data: datos) else { return datos }
        let raiz = PDFOutline()
        for (hoja, titulo) in indice {
            guard let pagina = doc.page(at: hoja) else { continue }
            let marcador = PDFOutline()
            marcador.label = titulo
            let caja = pagina.bounds(for: .mediaBox)
            marcador.destination = PDFDestination(page: pagina, at: CGPoint(x: caja.minX, y: caja.maxY))
            raiz.insertChild(marcador, at: raiz.numberOfChildren)
        }
        doc.outlineRoot = raiz
        return doc.dataRepresentation() ?? datos
    }

    /// **«Exportar PDF»**: el PDF de la carpeta en un fichero temporal con su
    /// nombre (`nombreDelPdfDeCarpeta`), para la hoja de compartir. `nil` si
    /// no sale ninguna pagina.
    static func exportar(_ carpeta: CarpetaDePaginas, chat: String, almacen: Almacen) async -> URL? {
        guard !carpeta.paginas.isEmpty else { return nil }
        let preparada = await preparar(carpeta, chat: chat, almacen: almacen)
        guard let datos = pdf(preparada) else { return nil }
        let carpetaTmp = FileManager.default.temporaryDirectory
            .appendingPathComponent("carpeta-\(UUID().uuidString)", isDirectory: true)
        do {
            try FileManager.default.createDirectory(at: carpetaTmp, withIntermediateDirectories: true)
            let destino = carpetaTmp.appendingPathComponent(nombreDelPdfDeCarpeta(titulo: carpeta.nombre))
            try datos.write(to: destino, options: .atomic)
            return destino
        } catch {
            return nil
        }
    }
}
