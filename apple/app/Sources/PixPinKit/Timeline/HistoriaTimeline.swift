// **La historia de un momento** (Windows `timeline/ventana/historia.rs` y
// `disposicion::historia`, 5-oct-2026): como las de Instagram, que es lo
// que pidio el usuario.
//
// - Con foto, la foto **llena** la pantalla (recortada al centro) bajo un
//   velo que la oscurece arriba y abajo; sin foto, el degradado estable del
//   momento (`EstiloHistoria.degradado`, el de `pixpin_timeline::estilo`).
// - El titulo grande y la descripcion, **centrados en el medio**, con
//   sombra para que se lean sobre cualquier foto.
// - Barritas arriba con varias fotos; tocar el tercio izquierdo o derecho
//   (o ←/→) pasa de foto; deslizar arriba o abajo (o ↑/↓), de momento.
// - Arriba a la derecha: compartir como imagen, pinear la foto que se ve,
//   hacerlo leccion y cerrar; abajo, la chapita (en Momentos, leccion) y la
//   nota de voz en grande.
//
// Lo mismo, sin botones, es **la tarjeta que se comparte**
// (`TarjetaHistoria`): 1080 × 1350, como un post. Y aqui tambien el PDF del
// timeline (`PdfDelTimeline`): la misma pagina que se exporta en HTML,
// impresa en A4 (Windows la imprime con Edge).

import PixPinCore
import SwiftUI
import UIKit
import WebKit

// MARK: - Lo puro

enum EstiloHistoria {
    /// Los degradados de un momento sin foto, `(arriba a la izquierda,
    /// abajo a la derecha)` en `0xRRGGBB` (`estilo::DEGRADADOS`).
    static let degradados: [(UInt32, UInt32)] = [
        (0x4E2A84, 0xC2457A), // morado a frambuesa
        (0x1D4E89, 0x2A9D8F), // azul a verde agua
        (0xB4462F, 0xE9A23B), // teja a ambar
        (0x2B2D6E, 0x6A4BC4), // indigo a violeta
        (0x0F5E5A, 0x7FB069), // pino a hoja
        (0x7A1F3D, 0xE76F51), // vino a coral
        (0x23395B, 0x8E7DBE), // noche a lavanda
        (0x5C3D2E, 0xC08552), // cafe a caramelo
    ]

    /// El degradado de un momento, por su id (siempre el mismo: FNV-1a de
    /// sus bytes, como `estilo::degradado_de`, para que la app, la imagen
    /// compartida y la pagina exportada lo ensenen igual).
    static func degradado(id: String) -> (UInt32, UInt32) {
        var h: UInt32 = 0x811C_9DC5
        for b in id.utf8 {
            h ^= UInt32(b)
            h = h &* 0x0100_0193
        }
        return degradados[Int(h % UInt32(degradados.count))]
    }

    /// El tamano de la tarjeta que se comparte (4:5, un post).
    static let tarjeta = CGSize(width: 1080, height: 1350)

    /// El tamano de letra del titulo segun lo largo que sea (una frase
    /// corta, grande; un parrafo, mas pequeno).
    static func letraDelTitulo(_ titulo: String, base: CGFloat) -> CGFloat {
        switch titulo.count {
        case ..<40: base
        case ..<90: base * 0.8
        default: base * 0.65
        }
    }

    /// La fecha de arriba: «lunes 5 de octubre · 18:40».
    static func fecha(_ m: MomentoTimeline) -> String {
        "\(ModeloTimeline.rotulo(iso: ModeloTimeline.iso(de: m.cuando)).capitalizedPrimera) · \(m.hora)"
    }

    /// El nombre del PNG compartido (sin lo que no vale en un fichero).
    static func nombreDeImagen(_ m: MomentoTimeline) -> String {
        let base = m.titulo.isEmpty ? "momento" : String(m.titulo.prefix(40))
        let limpio = base.map { "/\\:?*\"<>|\n".contains($0) ? "-" : $0 }
        return String(limpio).trimmingCharacters(in: .whitespaces) + ".png"
    }
}

// MARK: - El fondo y el texto (comunes a la historia y a la tarjeta)

/// La foto llenando o el degradado del momento, con su velo.
struct FondoHistoria: View {
    let momento: MomentoTimeline
    let imagen: CGImage?

    var body: some View {
        ZStack {
            let d = EstiloHistoria.degradado(id: momento.id)
            LinearGradient(colors: [Color(hex: d.0), Color(hex: d.1)], startPoint: .topLeading, endPoint: .bottomTrailing)
            if let imagen {
                GeometryReader { geo in
                    Image(decorative: imagen, scale: 1).resizable().scaledToFill()
                        .frame(width: geo.size.width, height: geo.size.height).clipped()
                }
                // El velo: oscuro arriba y abajo, y un poco en medio para
                // que el texto se lea.
                LinearGradient(stops: [.init(color: .black.opacity(0.55), location: 0),
                                       .init(color: .black.opacity(0.25), location: 0.3),
                                       .init(color: .black.opacity(0.35), location: 0.6),
                                       .init(color: .black.opacity(0.7), location: 1)],
                               startPoint: .top, endPoint: .bottom)
            }
        }
    }
}

/// El titulo y la descripcion en el medio, con sombra.
struct TextoHistoria: View {
    let momento: MomentoTimeline
    var escala: CGFloat = 1

    var body: some View {
        VStack(spacing: 14 * escala) {
            Text(momento.titulo.isEmpty ? "Sin título" : momento.titulo)
                .font(.system(size: EstiloHistoria.letraDelTitulo(momento.titulo, base: 34 * escala), weight: .bold))
            if !momento.descripcion.isEmpty {
                Text(momento.descripcion)
                    .font(.system(size: 18 * escala))
                    .lineLimit(12)
            }
        }
        .multilineTextAlignment(.center)
        .foregroundStyle(.white)
        .shadow(color: .black.opacity(0.6), radius: 6 * escala, y: 2 * escala)
        .frame(maxWidth: 680 * escala)
        .padding(.horizontal, 32 * escala)
    }
}

// MARK: - La historia

struct VistaHistoria: View {
    @ObservedObject var modelo: ModeloTimeline
    let lista: [MomentoTimeline]
    @State var indice: Int
    @State var foto: Int
    let corregir: (MomentoTimeline) -> Void
    @Environment(\.dismiss) private var cerrar
    @ObservedObject private var reproductor = ReproductorVoz.compartido
    @State private var imagen: CGImage?
    @State private var haciendo = false
    @State private var dicho: String?

    init(modelo: ModeloTimeline, lista: [MomentoTimeline], indice: Int, foto: Int,
         corregir: @escaping (MomentoTimeline) -> Void) {
        self.modelo = modelo
        self.lista = lista
        _indice = State(initialValue: indice)
        _foto = State(initialValue: foto)
        self.corregir = corregir
    }

    private var momento: MomentoTimeline? { lista.indices.contains(indice) ? lista[indice] : nil }

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            if let m = momento {
                historia(m)
            } else {
                ContentUnavailableView("Ya no está", systemImage: "clock")
            }
        }
        .environment(\.colorScheme, .dark)
        .statusBarHidden()
        .task(id: "\(momento?.id ?? "")/\(foto)") { await cargarFoto() }
        .background {
            ZStack {
                Button("") { pasarMomento(-1) }.keyboardShortcut(.upArrow, modifiers: [])
                Button("") { pasarMomento(1) }.keyboardShortcut(.downArrow, modifiers: [])
                Button("") { pasarFoto(-1) }.keyboardShortcut(.leftArrow, modifiers: [])
                Button("") { pasarFoto(1) }.keyboardShortcut(.rightArrow, modifiers: [])
                Button("") { cerrar() }.keyboardShortcut(.cancelAction)
            }
            .opacity(0)
            .allowsHitTesting(false)
        }
    }

    private func historia(_ m: MomentoTimeline) -> some View {
        GeometryReader { geo in
            ZStack {
                FondoHistoria(momento: m, imagen: m.fotos.isEmpty ? nil : imagen)
                    .ignoresSafeArea()
                TextoHistoria(momento: m)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .allowsHitTesting(false)
                // Los tercios de pasar (debajo de los botones).
                HStack(spacing: 0) {
                    Color.clear.contentShape(Rectangle()).onTapGesture { pasarFoto(-1) }
                        .frame(width: geo.size.width / 3)
                    Spacer(minLength: 0)
                    Color.clear.contentShape(Rectangle()).onTapGesture { pasarFoto(1) }
                        .frame(width: geo.size.width / 3)
                }
                .padding(.vertical, 80)
                VStack(spacing: 0) {
                    arriba(m)
                    Spacer()
                    abajo(m)
                }
            }
            .gesture(
                DragGesture(minimumDistance: 30)
                    .onEnded { g in
                        guard abs(g.translation.height) > abs(g.translation.width), abs(g.translation.height) > 80 else { return }
                        pasarMomento(g.translation.height < 0 ? 1 : -1)
                    }
            )
        }
    }

    private func arriba(_ m: MomentoTimeline) -> some View {
        VStack(spacing: 10) {
            if m.fotos.count > 1 {
                HStack(spacing: 4) {
                    ForEach(0 ..< m.fotos.count, id: \.self) { k in
                        Capsule().fill(Color.white.opacity(k == foto ? 0.95 : 0.35)).frame(height: 3)
                    }
                }
                .padding(.horizontal, 12)
            }
            HStack(spacing: 8) {
                Text(EstiloHistoria.fecha(m))
                    .font(.system(size: 14, weight: .medium)).foregroundStyle(.white.opacity(0.9))
                    .shadow(color: .black.opacity(0.5), radius: 3)
                Spacer()
                redondo(haciendo ? "hourglass" : "square.and.arrow.up", "Compartir como imagen") { compartir(m) }
                    .disabled(haciendo)
                if foto < m.fotos.count {
                    redondo("pin", "Pinear la foto") {
                        if GestorPinesIPad.compartido.pinear(url: URL(fileURLWithPath: m.fotos[foto])) {
                            cerrar()
                        }
                    }
                }
                redondo(m.leccion != nil ? "lightbulb.fill" : "lightbulb",
                        m.leccion != nil ? "Ya no es lección" : "Hacer lección") {
                    _ = Task<Void, Never> { await modelo.alternarLeccion(m) }
                }
                Menu {
                    Button { _ = Task<Void, Never> { await modelo.alternarMomentos(m) } } label: {
                        Label(m.enMomentos ? "Quitar de Momentos" : "Mandar a Momentos",
                              systemImage: m.enMomentos ? "bookmark.slash" : "bookmark")
                    }
                    Button { corregir(m) } label: { Label("Corregir", systemImage: "pencil") }
                    Button { UIPasteboard.general.string = MenuMomento.texto(m) } label: {
                        Label("Copiar el texto", systemImage: "doc.on.doc")
                    }
                    if !m.fotos.isEmpty {
                        ShareLink(items: m.fotos.map { URL(fileURLWithPath: $0) }) {
                            Label("Compartir las fotos…", systemImage: "photo.on.rectangle")
                        }
                    }
                    Divider()
                    Button(role: .destructive) {
                        cerrar()
                        _ = Task<Void, Never> { await modelo.borrar(m) }
                    } label: { Label("Borrar", systemImage: "trash") }
                } label: {
                    Image(systemName: "ellipsis").font(.system(size: 17, weight: .semibold)).foregroundStyle(.white)
                        .frame(width: 40, height: 40).background(.black.opacity(0.35), in: Circle())
                }
                .accessibilityLabel("Más")
                redondo("xmark", "Cerrar") { cerrar() }
            }
            .padding(.horizontal, 16)
        }
        .padding(.top, 10)
    }

    private func abajo(_ m: MomentoTimeline) -> some View {
        VStack(spacing: 14) {
            if let d = dicho {
                Text(d).font(.system(size: 14)).foregroundStyle(.white)
                    .padding(.horizontal, 14).padding(.vertical, 8)
                    .background(.black.opacity(0.5), in: Capsule())
                    .task(id: d) {
                        try? await Task.sleep(for: .seconds(2))
                        dicho = nil
                    }
            }
            HStack(spacing: 8) {
                if m.enMomentos { chapa("🔖 En Momentos") }
                if m.leccion != nil { chapa("💡 Lección") }
            }
            if let audio = m.audio {
                Button {
                    ReproductorVoz.compartido.alternar(m.id, url: URL(fileURLWithPath: audio), titulo: m.titulo)
                } label: {
                    HStack(spacing: 12) {
                        Image(systemName: reproductor.sonando && reproductor.actual == m.id
                            ? "stop.circle.fill" : "play.circle.fill")
                            .font(.system(size: 34))
                        Text(TarjetaMomento.duracion(m.duracionMs)).font(.system(size: 17).monospacedDigit())
                        Spacer(minLength: 0)
                        Image(systemName: "waveform").font(.system(size: 22)).opacity(0.7)
                    }
                    .foregroundStyle(.white)
                    .padding(.horizontal, 14)
                    .frame(width: 300, height: 56)
                    .background(.black.opacity(0.45), in: Capsule())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Nota de voz")
            }
        }
        .padding(.bottom, 34)
    }

    private func chapa(_ t: String) -> some View {
        Text(t).font(.system(size: 13)).foregroundStyle(.white)
            .padding(.horizontal, 10).padding(.vertical, 5)
            .background(.white.opacity(0.18), in: Capsule())
    }

    private func redondo(_ icono: String, _ rotulo: String, accion: @escaping () -> Void) -> some View {
        Button(action: accion) {
            Image(systemName: icono).font(.system(size: 16, weight: .semibold)).foregroundStyle(.white)
                .frame(width: 40, height: 40).background(.black.opacity(0.35), in: Circle())
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(rotulo)
    }

    private func cargarFoto() async {
        guard let m = momento, m.fotos.indices.contains(foto) else {
            imagen = nil
            return
        }
        imagen = await Miniaturas.compartidas.cargar(URL(fileURLWithPath: m.fotos[foto]), lado: 2400)
    }

    private func pasarFoto(_ paso: Int) {
        guard let m = momento else { return }
        let n = foto + paso
        if m.fotos.indices.contains(n) {
            foto = n
        } else {
            // Al acabar las fotos, el momento de al lado (como Instagram).
            pasarMomento(paso)
        }
    }

    private func pasarMomento(_ paso: Int) {
        let n = indice + paso
        guard lista.indices.contains(n) else { return }
        indice = n
        foto = paso < 0 ? max(0, lista[n].fotos.count - 1) : 0
    }

    private func compartir(_ m: MomentoTimeline) {
        haciendo = true
        Task {
            defer { haciendo = false }
            guard let url = await TarjetaHistoria.png(m) else {
                dicho = "No se pudo hacer la imagen"
                return
            }
            SalidasDeLaHoja.compartir([url], desde: nil) { _ in }
        }
    }
}

// MARK: - La tarjeta que se comparte

/// La historia sin botones, en 1080 × 1350, con la primera foto de fondo.
struct TarjetaHistoria: View {
    let momento: MomentoTimeline
    let imagen: CGImage?

    var body: some View {
        let t = EstiloHistoria.tarjeta
        ZStack {
            FondoHistoria(momento: momento, imagen: imagen)
            TextoHistoria(momento: momento, escala: 2)
            VStack {
                Text(EstiloHistoria.fecha(momento))
                    .font(.system(size: 30, weight: .medium)).foregroundStyle(.white.opacity(0.9))
                    .shadow(color: .black.opacity(0.5), radius: 4)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Spacer()
                HStack {
                    if let audio = momento.audio, !audio.isEmpty {
                        Label(TarjetaMomento.duracion(momento.duracionMs), systemImage: "waveform")
                            .font(.system(size: 26)).foregroundStyle(.white.opacity(0.85))
                    }
                    Spacer()
                    Text("PixPin · Timeline").font(.system(size: 24, weight: .medium)).foregroundStyle(.white.opacity(0.7))
                }
            }
            .padding(56)
        }
        .frame(width: t.width, height: t.height)
        .clipped()
        .environment(\.colorScheme, .dark)
    }

    /// La tarjeta de un momento hecha PNG en un temporal (para la hoja de
    /// compartir). `nil` si no se pudo.
    @MainActor
    static func png(_ m: MomentoTimeline) async -> URL? {
        var foto: CGImage?
        if let ruta = m.fotos.first {
            foto = await Miniaturas.compartidas.cargar(URL(fileURLWithPath: ruta), lado: 1600)
        }
        let r = ImageRenderer(content: TarjetaHistoria(momento: m, imagen: foto))
        r.scale = 1
        r.proposedSize = ProposedViewSize(EstiloHistoria.tarjeta)
        guard let img = r.uiImage, let datos = img.pngData() else { return nil }
        let carpeta = FileManager.default.temporaryDirectory.appendingPathComponent("pixpin-timeline", isDirectory: true)
        try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        let url = carpeta.appendingPathComponent(EstiloHistoria.nombreDeImagen(m))
        return (try? datos.write(to: url, options: .atomic)) != nil ? url : nil
    }
}

// MARK: - El PDF

/// **El timeline en PDF**: la pagina que se exporta (HTML con todo dentro)
/// cargada en un `WKWebView` fuera de la pantalla e impresa en A4 con el
/// formateador de impresion de iOS (lo que Windows hace con Edge).
@MainActor
final class PdfDelTimeline: NSObject, WKNavigationDelegate {
    private var web: WKWebView?
    private var listo: CheckedContinuation<Void, Error>?
    private static var vivos: [PdfDelTimeline] = []

    /// Un A4 en puntos y su margen.
    static let papel = CGRect(x: 0, y: 0, width: 595.2, height: 841.8)
    static let margen: CGFloat = 28

    /// El PDF de `html`.
    static func hacer(html: String) async throws -> Data {
        let p = PdfDelTimeline()
        vivos.append(p)
        defer { vivos.removeAll { $0 === p } }
        return try await p.imprimir(html)
    }

    private func imprimir(_ html: String) async throws -> Data {
        let w = WKWebView(frame: CGRect(x: 0, y: 0, width: Self.papel.width, height: Self.papel.height))
        w.navigationDelegate = self
        web = w
        try await withCheckedThrowingContinuation { (c: CheckedContinuation<Void, Error>) in
            listo = c
            w.loadHTMLString(html, baseURL: nil)
        }
        // Las fotos de dentro (en `data:`) acaban de pintarse.
        try? await Task.sleep(for: .milliseconds(400))
        let r = UIPrintPageRenderer()
        r.addPrintFormatter(w.viewPrintFormatter(), startingAtPageAt: 0)
        r.setValue(NSValue(cgRect: Self.papel), forKey: "paperRect")
        r.setValue(NSValue(cgRect: Self.papel.insetBy(dx: Self.margen, dy: Self.margen)), forKey: "printableRect")
        let datos = NSMutableData()
        UIGraphicsBeginPDFContextToData(datos, Self.papel, nil)
        let paginas = max(1, r.numberOfPages)
        r.prepare(forDrawingPages: NSRange(location: 0, length: paginas))
        for i in 0 ..< paginas {
            UIGraphicsBeginPDFPage()
            r.drawPage(at: i, in: UIGraphicsGetPDFContextBounds())
        }
        UIGraphicsEndPDFContext()
        web = nil
        return datos as Data
    }

    nonisolated func webView(_: WKWebView, didFinish _: WKNavigation!) {
        Task { @MainActor in
            self.listo?.resume()
            self.listo = nil
        }
    }

    nonisolated func webView(_: WKWebView, didFail _: WKNavigation!, withError error: Error) {
        Task { @MainActor in
            self.listo?.resume(throwing: error)
            self.listo = nil
        }
    }

    nonisolated func webView(_: WKWebView, didFailProvisionalNavigation _: WKNavigation!, withError error: Error) {
        Task { @MainActor in
            self.listo?.resume(throwing: error)
            self.listo = nil
        }
    }
}
