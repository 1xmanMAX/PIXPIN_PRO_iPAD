// **La tarjeta de un enlace** en la burbuja de una nota
// (`ventana_chat/tarjetas.rs` de Windows): si la nota lleva una direccion
// web, debajo sale su titulo, el sitio y su imagen, como en el movil. Un
// toque la abre en Safari.
//
// Lo de la pagina lo trae LinkPresentation (`LPMetadataProvider`), una vez
// por direccion: lo traido se guarda aqui mientras la app esta abierta, y lo
// que no se pudo traer (sin red) no se vuelve a pedir hasta otra vez.

import LinkPresentation
import SwiftUI
import UIKit

/// Lo que se ensena de una pagina.
struct DatosDeEnlace: Equatable {
    var titulo: String
    var sitio: String
    var imagen: UIImage?
}

@MainActor
final class TarjetasDeEnlace {
    static let compartidas = TarjetasDeEnlace()

    private var hechas: [URL: DatosDeEnlace?] = [:]
    private var enCurso: [URL: Task<DatosDeEnlace?, Never>] = [:]

    /// **La primera direccion web** de un texto (`http` o `https`).
    nonisolated static func primerEnlace(_ texto: String) -> URL? {
        guard texto.contains("http"),
              let detector = try? NSDataDetector(types: NSTextCheckingResult.CheckingType.link.rawValue) else { return nil }
        let rango = NSRange(texto.startIndex..., in: texto)
        for r in detector.matches(in: texto, options: [], range: rango) {
            if let url = r.url, let esquema = url.scheme?.lowercased(), esquema == "http" || esquema == "https" {
                return url
            }
        }
        return nil
    }

    /// El sitio de una direccion, sin el «www.».
    nonisolated static func sitio(_ url: URL) -> String {
        let h = url.host ?? url.absoluteString
        return h.hasPrefix("www.") ? String(h.dropFirst(4)) : h
    }

    func hecha(_ url: URL) -> DatosDeEnlace?? { hechas[url] }

    func datos(_ url: URL) async -> DatosDeEnlace? {
        if let h = hechas[url] { return h }
        if let t = enCurso[url] { return await t.value }
        let t = Task<DatosDeEnlace?, Never> { @MainActor in
            let proveedor = LPMetadataProvider()
            proveedor.timeout = 10
            guard let m = try? await proveedor.startFetchingMetadata(for: url) else { return nil }
            let titulo = (m.title ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            var imagen: UIImage?
            if let p = m.imageProvider ?? m.iconProvider {
                imagen = await withCheckedContinuation { (c: CheckedContinuation<UIImage?, Never>) in
                    p.loadObject(ofClass: UIImage.self) { obj, _ in c.resume(returning: obj as? UIImage) }
                }
            }
            guard !titulo.isEmpty || imagen != nil else { return nil }
            return DatosDeEnlace(titulo: titulo, sitio: TarjetasDeEnlace.sitio(m.url ?? url), imagen: imagen)
        }
        enCurso[url] = t
        let d = await t.value
        enCurso[url] = nil
        hechas[url] = .some(d)
        return d
    }
}

/// La tarjeta, debajo del texto de la nota.
struct TarjetaEnlace: View {
    let url: URL
    @State private var datos: DatosDeEnlace?
    @Environment(\.escalaChat) private var escala
    @Environment(\.openURL) private var abrir

    var body: some View {
        Group {
            if let datos {
                Button { abrir(url) } label: {
                    HStack(spacing: 10 * escala) {
                        if let img = datos.imagen {
                            Image(uiImage: img).resizable().scaledToFill()
                                .frame(width: 56 * escala, height: 56 * escala)
                                .clipShape(RoundedRectangle(cornerRadius: 8))
                        }
                        VStack(alignment: .leading, spacing: 2) {
                            if !datos.titulo.isEmpty {
                                Text(datos.titulo).font(.system(size: 14 * escala, weight: .semibold)).lineLimit(2)
                            }
                            Text(datos.sitio).font(.system(size: 12 * escala)).opacity(0.7).lineLimit(1)
                        }
                        Spacer(minLength: 0)
                    }
                    .padding(8 * escala)
                    .frame(maxWidth: 340, alignment: .leading)
                    .background(Color.primary.opacity(0.07), in: RoundedRectangle(cornerRadius: 10))
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Abrir \(datos.titulo.isEmpty ? datos.sitio : datos.titulo)")
            }
        }
        .task(id: url) {
            if case let .some(h) = TarjetasDeEnlace.compartidas.hecha(url) {
                datos = h
                return
            }
            datos = await TarjetasDeEnlace.compartidas.datos(url)
        }
    }
}
