// **Fusionar páginas del PDF del proyecto en un lienzo nuevo**
// (`fusionar_paginas.rs` de Windows, `FusionarPaginas.kt` del movil): «la
// 4, la 5 y la 6 en un mismo lienzo, separadas a una distancia razonable»,
// como dos planos juntos en la mesa para dibujar encima.
//
// Las cuentas y lo que se escribe son del nucleo (`anchosParaFusionar`,
// `Nucleo.fusionarPaginas`: un marco por pagina con su rotulo, la pagina de
// imagen con candado, el mensaje `DIBUJO` y su hoja en el proyecto). Aqui se
// pintan las paginas (Core Graphics, fuera del hilo principal) y se eligen.
//
// Se pide desde las figuras del lienzo de un chat cuyo proyecto tiene PDF
// («Fusionar páginas del PDF…»); otras zonas pueden presentar
// `HojaFusionarPaginas` o llamar a `FusionarPaginasIPad.fusionar`.

import PixPinCore
import SwiftUI
import UIKit

public enum FusionarPaginasIPad {
    /// La calidad del JPEG de cada pagina (la del movil).
    static let calidad: CGFloat = 0.88

    /// Lo que mide cada pagina (en puntos, ya girada).
    static func medidas(_ doc: CGPDFDocument, _ paginas: [Int]) -> [MedidaDePagina] {
        paginas.map { i in
            guard let p = doc.page(at: i + 1) else { return MedidaDePagina(ancho: 595, alto: 842) }
            let r = p.getBoxRect(.cropBox)
            let girada = p.rotationAngle % 180 != 0
            return MedidaDePagina(ancho: Double(girada ? r.height : r.width), alto: Double(girada ? r.width : r.height))
        }
    }

    /// **Pinta las paginas** (desde 0) a los anchos del nucleo, en JPEG.
    /// Bloqueante: fuera del hilo principal.
    public static func pintar(pdf: URL, paginas: [Int]) -> [PaginaParaFusionar] {
        guard let doc = CGPDFDocument(pdf as CFURL) else { return [] }
        let validas = paginas.filter { $0 >= 0 && $0 < doc.numberOfPages }
        let tamanos = medidas(doc, validas)
        let anchos = anchosParaFusionar(medidas: tamanos)
        var hechas: [PaginaParaFusionar] = []
        for (k, i) in validas.enumerated() {
            guard let p = doc.page(at: i + 1), k < anchos.count else { continue }
            let m = tamanos[k]
            let ancho = Int(anchos[k])
            let alto = max(1, Int((Double(ancho) * m.alto / max(m.ancho, 1)).rounded()))
            guard let ctx = CGContext(data: nil, width: ancho, height: alto, bitsPerComponent: 8, bytesPerRow: 0,
                                      space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
                                      bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) else { continue }
            let caja = CGRect(x: 0, y: 0, width: ancho, height: alto)
            ctx.setFillColor(CGColor(gray: 1, alpha: 1))
            ctx.fill(caja)
            ctx.interpolationQuality = .high
            // La pagina (con su giro) a su tamano en puntos, y luego a los
            // pixeles pedidos (`getDrawingTransform` no agranda).
            ctx.saveGState()
            let escala = CGFloat(ancho) / CGFloat(max(m.ancho, 1))
            ctx.scaleBy(x: escala, y: escala)
            let papel = CGRect(x: 0, y: 0, width: CGFloat(m.ancho), height: CGFloat(m.alto))
            ctx.concatenate(p.getDrawingTransform(.cropBox, rect: papel, rotate: 0, preserveAspectRatio: true))
            ctx.drawPDFPage(p)
            ctx.restoreGState()
            guard let img = ctx.makeImage(),
                  let jpg = UIImage(cgImage: img).jpegData(compressionQuality: calidad) else { continue }
            hechas.append(PaginaParaFusionar(pagina: UInt32(i), jpg: jpg, ancho: UInt32(ancho), alto: UInt32(alto)))
        }
        return hechas
    }

    /// **Junta `paginas` (desde 0) del PDF del proyecto `chat`** en un lienzo
    /// nuevo y devuelve su mensaje, para abrirlo. `nil` si no salio.
    public static func fusionar(nucleo: Nucleo, chat: String, paginas: [Int]) async -> MensajeChat? {
        await Task.detached(priority: .userInitiated) { () -> MensajeChat? in
            guard let ruta = nucleo.pdfDelProyecto(chat: chat) else { return nil }
            let hechas = pintar(pdf: URL(fileURLWithPath: ruta), paginas: paginas)
            guard !hechas.isEmpty else { return nil }
            return try? nucleo.fusionarPaginas(chat: chat, paginas: hechas)
        }.value
    }

    /// Cuantas paginas tiene el PDF del proyecto (0 si no tiene).
    public static func cuantasPaginas(nucleo: Nucleo, chat: String) -> Int {
        guard let ruta = nucleo.pdfDelProyecto(chat: chat),
              let doc = CGPDFDocument(URL(fileURLWithPath: ruta) as CFURL) else { return 0 }
        return doc.numberOfPages
    }
}

/// **Elegir las paginas que se fusionan** (de 2 a 12) y fusionarlas.
public struct HojaFusionarPaginas: View {
    let nucleo: Nucleo
    let chat: String
    /// El lienzo nuevo, para abrirlo.
    let hecho: (MensajeChat) -> Void

    @Environment(\.dismiss) private var cerrar
    @State private var total = 0
    @State private var elegidas: Set<Int> = []
    @State private var trabajando = false
    @State private var fallo: String?

    public init(nucleo: Nucleo, chat: String, hecho: @escaping (MensajeChat) -> Void) {
        self.nucleo = nucleo
        self.chat = chat
        self.hecho = hecho
    }

    private var tope: Int { Int(topeDePaginasParaFusionar()) }

    public var body: some View {
        NavigationStack {
            List {
                Section {
                    ForEach(0 ..< total, id: \.self) { i in
                        Toggle("Página \(i + 1)", isOn: Binding(
                            get: { elegidas.contains(i) },
                            set: { si in
                                if si { if elegidas.count < tope { elegidas.insert(i) } } else { elegidas.remove(i) }
                            }))
                    }
                } footer: {
                    Text("Juntas en un lienzo nuevo, separadas, cada una en su marco. De 2 a \(tope) páginas.")
                }
                if let fallo {
                    Section { Label(fallo, systemImage: "exclamationmark.triangle").foregroundStyle(.red) }
                }
            }
            .navigationTitle(elegidas.count >= 2
                ? nombreDePaginasFusionadas(paginas: elegidas.sorted().map { UInt32($0) })
                : "Fusionar páginas")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancelar") { cerrar() } }
                ToolbarItem(placement: .confirmationAction) {
                    if trabajando {
                        ProgressView()
                    } else {
                        Button("Fusionar en un lienzo", action: fusionar).disabled(elegidas.count < 2)
                    }
                }
            }
        }
        .task {
            let nucleo = self.nucleo, chat = self.chat
            total = await Task.detached(priority: .userInitiated) {
                FusionarPaginasIPad.cuantasPaginas(nucleo: nucleo, chat: chat)
            }.value
        }
    }

    private func fusionar() {
        trabajando = true
        fallo = nil
        let paginas = elegidas.sorted()
        _ = Task<Void, Never> {
            let m = await FusionarPaginasIPad.fusionar(nucleo: nucleo, chat: chat, paginas: paginas)
            trabajando = false
            guard let m else {
                fallo = "No se pudieron fusionar las páginas."
                return
            }
            NotificationCenter.default.post(name: .pixpinTrajoCambios, object: nil)
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            cerrar()
            hecho(m)
        }
    }
}

extension ModeloEditorLienzo {
    /// Si el proyecto de este lienzo tiene PDF (para ofrecer fusionar).
    var proyectoConPdf: Bool {
        guard let chat, let nucleo else { return false }
        return nucleo.pdfDelProyecto(chat: chat) != nil
    }

    /// **«Fusionar páginas del PDF…»**: elegirlas y abrir el lienzo nuevo
    /// (se guarda antes lo de ahora).
    func pedirFusionarPaginas() {
        guard let chat, let nucleo, let vista else { return }
        let abrir = alSeguirEnlace
        let hoja = UIHostingController(rootView: HojaFusionarPaginas(nucleo: nucleo, chat: chat) { [weak self] m in
            self?.guardarAhora(terminarTexto: true)
            abrir?(m)
        })
        hoja.modalPresentationStyle = .formSheet
        CajetinesLienzo.presentar(hoja, desde: vista)
    }
}
