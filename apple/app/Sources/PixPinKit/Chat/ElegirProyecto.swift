// **Adjuntar un proyecto entero o una pagina de un proyecto** (la hoja «+»
// de Windows: `AdjProyecto` y `AdjPagina`, `ventana_chat/paginas.rs`):
//
// - «Proyecto entero» mete la puerta a otro proyecto (`PROYECTO`): no copia
//   nada, pulsarla lo abre (`Nucleo.adjuntarProyecto`);
// - «Página de un proyecto» deja elegir el proyecto y despues una de sus
//   hojas (las paginas de su PDF y sus lienzos, `paginasDeProyecto`). Si el
//   nucleo pide la pagina ya pintada (una hoja del PDF de otro proyecto,
//   `paginaPorPintar`), se pinta aqui con PDFKit y va con ella
//   (`adjuntarPagina`).

import PDFKit
import PixPinCore
import SwiftUI
import UIKit

extension Almacen {
    /// Mete en `chat` la puerta al proyecto `proyecto`.
    @discardableResult
    func adjuntar(proyecto: String, en chat: String) async -> MensajeChat? {
        let nucleo = self.nucleo
        let r = await Task.detached(priority: .userInitiated) { () -> Result<MensajeChat, Error> in
            Result { try nucleo.adjuntarProyecto(chat: chat, proyecto: proyecto) }
        }.value
        switch r {
        case let .success(m):
            anadir([m], en: chat)
            return m
        case let .failure(e):
            ponerError("No se pudo adjuntar el proyecto: \(e)")
            return nil
        }
    }

    /// Mete en `chat` la hoja `pagina` del proyecto `desde`.
    @discardableResult
    func adjuntar(pagina: PaginaElegible, de desde: String, en chat: String) async -> MensajeChat? {
        let nucleo = self.nucleo
        let hoja = pagina.hoja
        let r = await Task.detached(priority: .userInitiated) { () -> Result<MensajeChat, Error> in
            var png: Data?
            if let pintar = nucleo.paginaPorPintar(hacia: chat, desde: desde, hoja: hoja) {
                png = ElegirProyecto.pintar(pdf: URL(fileURLWithPath: pintar.pdf), pagina: Int(pintar.pagina))
            }
            return Result { try nucleo.adjuntarPagina(hacia: chat, desde: desde, hoja: hoja, png: png) }
        }.value
        switch r {
        case let .success(m):
            anadir([m], en: chat)
            return m
        case let .failure(e):
            ponerError("No se pudo adjuntar la página: \(e)")
            return nil
        }
    }
}

enum ElegirProyecto {
    /// La hoja `pagina` de un PDF en PNG, a 1600 de ancho sobre blanco.
    nonisolated static func pintar(pdf: URL, pagina: Int, ancho: CGFloat = 1600) -> Data? {
        guard let doc = PDFDocument(url: pdf), let p = doc.page(at: pagina) else { return nil }
        var caja = p.bounds(for: .cropBox)
        if abs(p.rotation) % 180 == 90 { caja = CGRect(x: 0, y: 0, width: caja.height, height: caja.width) }
        guard caja.width > 0, caja.height > 0 else { return nil }
        let k = ancho / caja.width
        let w = Int(ancho.rounded()), h = max(1, Int((caja.height * k).rounded()))
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        ctx.setFillColor(CGColor(gray: 1, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
        ctx.scaleBy(x: k, y: k)
        p.draw(with: .cropBox, to: ctx)
        guard let img = ctx.makeImage() else { return nil }
        return UIImage(cgImage: img).pngData()
    }

    /// El rotulo de una hoja que se ofrece: «Página 3» o «Lienzo 2».
    nonisolated static func rotulo(_ p: PaginaElegible) -> String {
        "Página \(p.numero)"
    }
}

/// **Elegir un proyecto** (y, si se pide, una de sus hojas), en una hoja.
struct HojaElegirProyecto: View {
    let almacen: Almacen
    /// El chat donde se adjunta (no se ofrece a si mismo).
    let chat: String
    /// Para elegir una pagina (si no, el proyecto entero).
    let conPaginas: Bool
    let alElegirProyecto: (FichaChat) -> Void
    let alElegirPagina: (FichaChat, PaginaElegible) -> Void
    @Environment(\.dismiss) private var cerrar
    @State private var filtro = ""

    private var proyectos: [FichaChat] {
        let f = filtro.trimmingCharacters(in: .whitespaces)
        return almacen.chats.filter { c in
            c.id != chat && !c.esAsistente && (f.isEmpty || c.nombre.localizedStandardContains(f))
        }
    }

    var body: some View {
        NavigationStack {
            List(proyectos, id: \.id) { p in
                if conPaginas {
                    NavigationLink {
                        PaginasDelProyecto(almacen: almacen, proyecto: p) { pagina in
                            alElegirPagina(p, pagina)
                            cerrar()
                        }
                    } label: { fila(p) }
                } else {
                    Button {
                        alElegirProyecto(p)
                        cerrar()
                    } label: { fila(p) }
                    .buttonStyle(.plain)
                }
            }
            .overlay {
                if proyectos.isEmpty { Text("No hay otros proyectos").foregroundStyle(.secondary) }
            }
            .searchable(text: $filtro, prompt: "Buscar")
            .navigationTitle(conPaginas ? "Página de un proyecto" : "Proyecto entero")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancelar") { cerrar() } }
            }
        }
    }

    private func fila(_ p: FichaChat) -> some View {
        HStack(spacing: 12) {
            ZStack {
                Circle().fill(Paleta.avatar(p.id))
                Text(Paleta.iniciales(p.nombre)).font(.system(size: 14)).foregroundStyle(.white)
            }
            .frame(width: 36, height: 36)
            VStack(alignment: .leading, spacing: 2) {
                Text(p.nombre).font(.system(size: 15, weight: .medium))
                Text(p.hojas == 1 ? "1 hoja" : "\(p.hojas) hojas").font(.caption).foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
        }
        .contentShape(Rectangle())
    }
}

/// Las hojas de un proyecto que se pueden adjuntar.
private struct PaginasDelProyecto: View {
    let almacen: Almacen
    let proyecto: FichaChat
    let elegir: (PaginaElegible) -> Void
    @State private var paginas: [PaginaElegible]?

    var body: some View {
        Group {
            if let paginas {
                if paginas.isEmpty {
                    Text("Este proyecto no tiene páginas").foregroundStyle(.secondary)
                } else {
                    List(paginas, id: \.hoja) { p in
                        Button { elegir(p) } label: {
                            Label(ElegirProyecto.rotulo(p), systemImage: p.esLienzo ? "scribble.variable" : "doc.richtext")
                        }
                    }
                }
            } else {
                ProgressView()
            }
        }
        .navigationTitle(proyecto.nombre)
        .task {
            let (nucleo, id) = (almacen.nucleo, proyecto.id)
            paginas = await Task.detached(priority: .userInitiated) { nucleo.paginasDeProyecto(proyecto: id) }.value
        }
    }
}
