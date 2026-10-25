// **Pronunciar: hablar, oirse y repetir, con una guia delante** (Windows
// `pronunciar.rs`, Mac `VozVentanas/Pronunciar.swift`, movil
// `PronunciarActivity.kt`).
//
// El bucle es corto: **se mantiene pulsado el microfono** (o la barra
// espaciadora del teclado), se habla, se suelta y **se oye al momento**.
// Cada toma pisa la anterior, que era un ensayo; la que convenza se guarda
// en el chat y alli se pasa a texto **en el idioma que se practica**, para
// ver que se entendio.
//
// **La guia** es lo que se tiene delante: un texto escrito aqui, una nota
// `.md`/`.txt`, una imagen o una pagina de un PDF, de Archivos o del chat.
// Las imagenes y los PDF se abren fuera del hilo principal.
//
// «Oír el texto» lee la guia escrita con la voz del iPad en ese idioma. No
// puntua: la transcripcion ya lo dice.
//
// Teclado: Espacio mantenido graba, R la oye otra vez, ⌘G/⌃G la guarda,
// ← → pasan de pagina, + − la letra, Esc cierra (las tomas se tiran).

import AVFoundation
import ImageIO
import PDFKit
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

// MARK: - Lo puro

enum Pronunciar {
    /// El nombre de la ventanita (una a la vez).
    static let nombre = "pronunciar"

    /// Los idiomas que se ofrecen (los del movil): codigo, nombre en su
    /// lengua y la etiqueta del reconocedor y de la voz. El primero es «el
    /// de siempre».
    static let idiomas: [(codigo: String, nombre: String, etiqueta: String)] = [
        ("", "El de siempre", "es-ES"),
        ("es", "Español", "es-ES"),
        ("en", "English", "en-US"),
        ("pt", "Português", "pt-PT"),
        ("fr", "Français", "fr-FR"),
        ("de", "Deutsch", "de-DE"),
        ("it", "Italiano", "it-IT"),
        ("ca", "Català", "ca-ES"),
    ]

    enum ClaseDeGuia: Equatable { case texto, imagen, pdf }

    /// Que clase de guia es un fichero, por su extension. `nil` si no sirve.
    static func clase(de nombre: String) -> ClaseDeGuia? {
        switch (nombre as NSString).pathExtension.lowercased() {
        case "pdf": return .pdf
        case "png", "jpg", "jpeg", "webp", "gif", "bmp", "heic", "tif", "tiff": return .imagen
        case "md", "txt", "markdown": return .texto
        default: return nil
        }
    }

    /// Una guia que se puede traer del chat.
    struct DelChat: Equatable {
        enum Fuente: Equatable {
            case texto(String)
            /// Un adjunto: su ruta en el chat (se resuelve al elegirlo).
            case adjunto(ruta: String)
        }

        var titulo: String
        var fuente: Fuente
    }

    /// **Lo del chat que sirve de guia** (`guias_del_chat`): notas,
    /// imagenes, PDF y `.md`/`.txt`, lo ultimo primero, doce como mucho.
    static func guiasDelChat(_ mensajes: [MensajeChat]) -> [DelChat] {
        var v: [DelChat] = []
        for m in mensajes.reversed() {
            if v.count >= 12 { break }
            guard let ruta = m.ruta else {
                guard let linea = m.texto.split(separator: "\n").first(where: {
                    !$0.trimmingCharacters(in: .whitespaces).isEmpty
                }) else { continue }
                let titulo = String(linea.trimmingCharacters(in: .whitespaces).prefix(60))
                v.append(DelChat(titulo: "✎ \(titulo)", fuente: .texto(m.texto)))
                continue
            }
            let nombre = m.nombre.isEmpty ? (ruta as NSString).lastPathComponent : m.nombre
            let icono: String
            switch clase(de: nombre) {
            case .imagen?: icono = "🖼"
            case .pdf?, .texto?: icono = "📄"
            case nil: continue
            }
            v.append(DelChat(titulo: "\(icono) \(nombre)", fuente: .adjunto(ruta: ruta)))
        }
        return v
    }

    /// El lado mayor de una lamina: de sobra para la pantalla.
    static let ladoDeLamina: CGFloat = 1600
}

/// Una toma que el usuario guardo.
struct TomaPronunciar {
    var nota: NotaGrabada
    /// La etiqueta del idioma que se practicaba (`en-US`); `nil`, el de
    /// siempre.
    var idioma: String?
}

/// Una imagen ya abierta (o una pagina de PDF ya pintada).
struct LaminaGuia: @unchecked Sendable {
    let imagen: CGImage
    let paginas: Int
}

// MARK: - El modelo

@MainActor
final class ModeloPronunciar: ObservableObject {
    enum Guia {
        case ninguna
        case texto(String)
        /// Una imagen o una pagina de PDF; del PDF, su fichero, la pagina y
        /// cuantas tiene.
        case lamina(CGImage?, pdf: (URL, Int, Int)?, cargando: Bool)
    }

    @Published var guia = Guia.ninguna
    @Published var tam: CGFloat = 24
    @Published var idioma = 0
    @Published private(set) var grabando = false
    @Published private(set) var toma: NotaGrabada?
    @Published private(set) var guardada = false
    @Published var escribiendo: String?
    @Published var eligiendoFichero = false
    let aviso = AvisoCorto()

    let delChat: [Pronunciar.DelChat]
    /// Resuelve la ruta de un adjunto del chat.
    let resolver: (String) async -> URL?
    var alGuardar: ((TomaPronunciar) -> Void)?
    var cerrar: (() -> Void)?

    private let mic = MicrofonoSuelto()
    private var abriendo: Task<Void, Never>?
    private var soltado = false
    private var reproductor: AVAudioPlayer?
    private let voz = AVSpeechSynthesizer()
    private let carpeta = Ventanita.carpeta("pronunciar-\(UUID().uuidString)")
    /// El microfono pulsado con el dedo (no con Espacio).
    var conElDedo = false

    init(delChat: [Pronunciar.DelChat], resolver: @escaping (String) async -> URL?) {
        self.delChat = delChat
        self.resolver = resolver
    }

    var llevadoMs: Int64 { mic.llevadoMs }
    var desde: Date? { mic.desde }
    var etiqueta: String { Pronunciar.idiomas[idioma].etiqueta }

    // MARK: La toma

    /// Empieza una toma. Lo que sonaba se calla: si no, se grabaria encima.
    func empezar() {
        guard !grabando, abriendo == nil else { return }
        reproductor?.pause()
        voz.stopSpeaking(at: .immediate)
        soltado = false
        grabando = true
        let fichero = carpeta.appendingPathComponent("toma-\(Ventanita.ahoraMs).m4a")
        abriendo = Task { [weak self] in
            guard let self else { return }
            do {
                try await self.mic.empezar(en: fichero)
                self.abriendo = nil
                // Se solto mientras se abria: era un toque.
                if self.soltado {
                    self.mic.cancelar()
                    self.grabando = false
                    self.aviso.decir("Demasiado corta: mantén pulsado un poco más.", segundos: 3)
                }
            } catch {
                self.abriendo = nil
                self.grabando = false
                self.aviso.decir(MicrofonoSuelto.explicar(error))
            }
        }
    }

    /// Suelta: la toma se cierra y **se oye al momento**. Una muy corta es
    /// un toque sin querer y no pisa la anterior.
    func soltar() {
        conElDedo = false
        guard grabando else { return }
        if abriendo != nil {
            soltado = true
            return
        }
        _ = Task<Void, Never> {
            let nota = await mic.parar()
            grabando = false
            guard let nota else { return }
            guard nota.duracionMs >= FormatoVoz.minimoMs else {
                try? FileManager.default.removeItem(at: nota.url)
                aviso.decir("Demasiado corta: mantén pulsado un poco más.", segundos: 3)
                return
            }
            if let vieja = toma { try? FileManager.default.removeItem(at: vieja.url) }
            toma = nota
            guardada = false
            oir()
        }
    }

    func oir() {
        guard let toma else { return }
        reproductor?.stop()
        reproductor = try? AVAudioPlayer(contentsOf: toma.url)
        reproductor?.play()
    }

    /// Se manda una copia: el chat se queda el fichero y esta toma se tiene
    /// que poder seguir oyendo.
    func guardar() {
        guard let toma, !guardada else { return }
        let copia = carpeta.appendingPathComponent("pronunciar-\(Ventanita.ahoraMs).m4a")
        do {
            try FileManager.default.copyItem(at: toma.url, to: copia)
        } catch {
            aviso.decir("No se pudo guardar", segundos: 3)
            return
        }
        var nota = toma
        nota.url = copia
        alGuardar?(TomaPronunciar(nota: nota, idioma: idioma == 0 ? nil : etiqueta))
        guardada = true
        aviso.decir("Guardado; se pasa a texto para que veas qué se entendió")
    }

    /// Al cerrar: las tomas eran ensayos, fuera.
    func soltarTodo() {
        abriendo?.cancel()
        mic.cancelar()
        reproductor?.stop()
        voz.stopSpeaking(at: .immediate)
        let carpeta = self.carpeta
        // La copia guardada la esta leyendo el chat: se borra un rato despues.
        DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + 120) {
            try? FileManager.default.removeItem(at: carpeta)
        }
    }

    // MARK: La guia

    var hayTexto: Bool { if case .texto = guia { return true } else { return false } }

    func oirTexto() {
        guard case let .texto(t) = guia else { return }
        guard let v = VozAlta.voz(para: etiqueta, preferida: VozAlta.preferida(lengua: etiqueta)) else {
            aviso.decir("Este iPad no tiene voz para ese idioma")
            return
        }
        voz.stopSpeaking(at: .immediate)
        let u = AVSpeechUtterance(string: Telepronter.parrafos(de: t).joined(separator: "\n"))
        u.voice = v
        voz.speak(u)
    }

    func ponerTexto(_ t: String) {
        guia = t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? .ninguna : .texto(t)
    }

    /// Un fichero de Archivos (con su permiso) o del chat.
    func ponerFichero(_ url: URL, conPermiso: Bool = false) {
        switch Pronunciar.clase(de: url.lastPathComponent) {
        case .texto?:
            let abierto = conPermiso && url.startAccessingSecurityScopedResource()
            defer { if abierto { url.stopAccessingSecurityScopedResource() } }
            // Sin exigir UTF-8: una nota vieja puede venir en otra pagina.
            guard let d = try? Data(contentsOf: url) else { return aviso.decir("No se pudo abrir eso", segundos: 3) }
            ponerTexto(String(decoding: d, as: UTF8.self))
        case .imagen?:
            guia = .lamina(nil, pdf: nil, cargando: true)
            cargar(url, pagina: nil, conPermiso: conPermiso)
        case .pdf?:
            guia = .lamina(nil, pdf: (url, 0, 0), cargando: true)
            cargar(url, pagina: 0, conPermiso: conPermiso)
        case nil:
            aviso.decir("No se pudo abrir eso", segundos: 3)
        }
    }

    private var conPermisoPdf = false

    func pagina(_ d: Int) {
        guard case let .lamina(img, pdf?, cargando) = guia, !cargando else { return }
        let nueva = min(max(pdf.1 + d, 0), max(pdf.2 - 1, 0))
        guard nueva != pdf.1 else { return }
        guia = .lamina(img, pdf: (pdf.0, nueva, pdf.2), cargando: true)
        cargar(pdf.0, pagina: nueva, conPermiso: conPermisoPdf)
    }

    /// Abre una imagen o una pagina de PDF fuera del hilo principal.
    private func cargar(_ url: URL, pagina: Int?, conPermiso: Bool) {
        if pagina != nil { conPermisoPdf = conPermiso }
        _ = Task<Void, Never> {
            let l = await Task.detached(priority: .userInitiated) { () -> LaminaGuia? in
                let abierto = conPermiso && url.startAccessingSecurityScopedResource()
                defer { if abierto { url.stopAccessingSecurityScopedResource() } }
                return ModeloPronunciar.lamina(url, pagina: pagina)
            }.value
            guard case let .lamina(_, pdf, _) = guia else { return }
            guard let l else {
                guia = .ninguna
                aviso.decir("No se pudo abrir eso", segundos: 3)
                return
            }
            guia = .lamina(l.imagen, pdf: pdf.map { ($0.0, $0.1, max(l.paginas, 1)) }, cargando: false)
        }
    }

    nonisolated static func lamina(_ url: URL, pagina: Int?) -> LaminaGuia? {
        let lado = Pronunciar.ladoDeLamina
        guard let pagina else {
            guard let fuente = CGImageSourceCreateWithURL(url as CFURL, nil),
                  let img = CGImageSourceCreateThumbnailAtIndex(fuente, 0, [
                      kCGImageSourceCreateThumbnailFromImageAlways: true,
                      kCGImageSourceCreateThumbnailWithTransform: true,
                      kCGImageSourceThumbnailMaxPixelSize: lado,
                  ] as CFDictionary) else { return nil }
            return LaminaGuia(imagen: img, paginas: 0)
        }
        guard let doc = PDFDocument(url: url), let p = doc.page(at: pagina) else { return nil }
        let caja = p.bounds(for: .mediaBox)
        let k = lado / max(caja.width, caja.height, 1)
        let tam = CGSize(width: caja.width * k, height: caja.height * k)
        guard let cg = p.thumbnail(of: tam, for: .mediaBox).cgImage else { return nil }
        return LaminaGuia(imagen: cg, paginas: doc.pageCount)
    }

    /// «Escribir un texto».
    func escribirTexto() {
        if case let .texto(t) = guia { escribiendo = t } else { escribiendo = "" }
    }

    /// Una guia del chat.
    func elegirDelChat(_ d: Pronunciar.DelChat) {
        switch d.fuente {
        case let .texto(t): ponerTexto(t)
        case let .adjunto(ruta):
            _ = Task<Void, Never> {
                guard let u = await resolver(ruta), FileManager.default.fileExists(atPath: u.path) else {
                    return aviso.decir("No se pudo abrir eso", segundos: 3)
                }
                ponerFichero(u)
            }
        }
    }

    // MARK: Teclado

    func tecla(_ k: KeyPress) -> Bool {
        if k.key == .space {
            // Mantener: la tecla repite mientras esta abajo; solo cuenta la
            // primera.
            switch k.phase {
            case .down: empezar()
            case .up: soltar()
            default: break
            }
            return true
        }
        guard k.phase == .down else { return false }
        switch k.key {
        case .escape: cerrar?()
        case .leftArrow: pagina(-1)
        case .rightArrow: pagina(1)
        default:
            switch TeclasVoz.letra(k) {
            case "g" where TeclasVoz.conMando(k): guardar()
            case "r" where !TeclasVoz.conMando(k): oir()
            case "+", "=": tam = min(tam + 2, 44)
            case "-": tam = max(tam - 2, 14)
            default: return false
            }
        }
        return true
    }
}

// MARK: - La vista

struct VistaPronunciar: View {
    @ObservedObject var m: ModeloPronunciar
    @ObservedObject private var aviso: AvisoCorto
    @FocusState private var foco: Bool

    init(m: ModeloPronunciar) {
        self.m = m
        aviso = m.aviso
    }

    var body: some View {
        VStack(spacing: 0) {
            barra
            idiomas
            guia.padding(.horizontal, 16).padding(.top, 12)
            if let t = aviso.texto {
                Text(t).font(.system(size: 14)).foregroundStyle(ColoresVoz.dorado)
                    .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 16).padding(.top, 6)
            }
            pie
        }
        .background(ColoresVoz.fondo.ignoresSafeArea())
        .tecladoDeVentanita($foco) { k in m.tecla(k) }
        .sheet(isPresented: Binding(get: { m.escribiendo != nil }, set: { if !$0 { m.escribiendo = nil } })) {
            HojaTextoGuia(texto: m.escribiendo ?? "") { t in
                if let t { m.ponerTexto(t) }
                m.escribiendo = nil
            }
        }
        .fileImporter(isPresented: $m.eligiendoFichero,
                      allowedContentTypes: [.pdf, .image, .plainText, UTType(filenameExtension: "md") ?? .plainText]) { r in
            if case let .success(u) = r { m.ponerFichero(u, conPermiso: true) }
        }
    }

    private var barra: some View {
        HStack(spacing: 6) {
            BotonVentanita("←", ancho: 44) { m.cerrar?() }.accessibilityLabel("Cerrar")
            Text("Pronunciar").font(.system(size: 17)).foregroundStyle(ColoresVoz.texto).padding(.leading, 8)
            Spacer()
            if m.hayTexto {
                BotonVentanita("A-", ancho: 44) { m.tam = max(m.tam - 2, 14) }
                BotonVentanita("A+", ancho: 44) { m.tam = min(m.tam + 2, 44) }
                BotonVentanita("Oír el texto", ancho: 130) { m.oirTexto() }
            }
            menuGuia(ancho: 110)
        }
        .padding(.horizontal, 10)
        .frame(height: 56)
        .background(ColoresVoz.cristal)
    }

    /// «Guía»: escribir un texto, traer algo de Archivos o del chat.
    private func menuGuia(ancho: CGFloat) -> some View {
        SwiftUI.Menu {
            Button { m.escribirTexto() } label: { Label("Escribir un texto", systemImage: "pencil") }
            Button { m.eligiendoFichero = true } label: {
                Label("Una imagen, un PDF o una nota de Archivos", systemImage: "folder")
            }
            if !m.delChat.isEmpty {
                Section("Del chat") {
                    ForEach(m.delChat.indices, id: \.self) { i in
                        Button(m.delChat[i].titulo) { m.elegirDelChat(m.delChat[i]) }
                    }
                }
            }
        } label: {
            Text("Guía").font(.system(size: 16)).foregroundStyle(ColoresVoz.texto)
                .frame(width: ancho, height: 44)
                .background(ColoresVoz.boton, in: RoundedRectangle(cornerRadius: 8))
        }
    }

    private var idiomas: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Idioma que practicas").font(.system(size: 12)).foregroundStyle(ColoresVoz.apagado)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(Pronunciar.idiomas.indices, id: \.self) { i in
                        Button { m.idioma = i } label: {
                            Text(Pronunciar.idiomas[i].nombre).font(.system(size: 14)).foregroundStyle(ColoresVoz.texto)
                                .padding(.horizontal, 14).frame(height: 34)
                                .background(i == m.idioma ? ColoresVoz.azul : ColoresVoz.cristal, in: Capsule())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 14).padding(.top, 8)
    }

    @ViewBuilder private var guia: some View {
        switch m.guia {
        case .ninguna:
            VStack(spacing: 14) {
                Text("Sin guía: escribe, o trae una imagen, un PDF o una nota")
                    .font(.system(size: 15)).foregroundStyle(ColoresVoz.apagado)
                    .multilineTextAlignment(.center)
                menuGuia(ancho: 160)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case let .texto(t):
            ScrollView {
                Text(t).font(.system(size: m.tam)).foregroundStyle(ColoresVoz.texto)
                    .frame(maxWidth: .infinity, alignment: .leading).textSelection(.enabled)
            }
        case let .lamina(img, pdf, cargando):
            VStack(spacing: 4) {
                ZStack {
                    if let img {
                        Image(decorative: img, scale: 1).resizable().interpolation(.high).scaledToFit()
                    } else if cargando {
                        ProgressView().tint(ColoresVoz.apagado)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                if let pdf {
                    HStack(spacing: 30) {
                        BotonVentanita("‹", ancho: 44) { m.pagina(-1) }.accessibilityLabel("Página anterior")
                        Text("Página \(pdf.1 + 1) de \(max(pdf.2, 1))")
                            .font(.system(size: 13)).foregroundStyle(ColoresVoz.apagado)
                        BotonVentanita("›", ancho: 44) { m.pagina(1) }.accessibilityLabel("Página siguiente")
                    }
                    .frame(height: 48)
                }
            }
        }
    }

    private var pie: some View {
        VStack(spacing: 8) {
            if let toma = m.toma, !m.grabando {
                HStack(spacing: 12) {
                    BotonVentanita("Oír otra vez (R)", color: ColoresVoz.azul, ancho: 200, alto: 44) { m.oir() }
                    BotonVentanita(m.guardada ? "Guardada" : "Guardar en el chat (⌘G)", ancho: 220, alto: 44) { m.guardar() }
                }
                Text("Última toma: \(FormatoVoz.marca(ms: toma.duracionMs))")
                    .font(.system(size: 12)).foregroundStyle(ColoresVoz.apagado)
            } else {
                Spacer().frame(height: 62)
            }
            Text(m.grabando ? "Suelta para oírte"
                 : m.toma != nil ? "Mantén otra vez para repetirla"
                 : "Mantén pulsado el micrófono (o la barra espaciadora) y habla")
                .font(.system(size: 15)).foregroundStyle(m.grabando ? ColoresVoz.rojo : ColoresVoz.apagado)
            if m.grabando, let desde = m.desde {
                TimelineView(.periodic(from: desde, by: 0.1)) { _ in
                    Text(FormatoVoz.marca(ms: m.llevadoMs)).font(.system(size: 20).monospacedDigit())
                        .foregroundStyle(ColoresVoz.rojo)
                }
            }
            microfono
        }
        .padding(.vertical, 12)
        .frame(maxWidth: .infinity)
    }

    private var microfono: some View {
        ZStack {
            Circle().fill(m.grabando ? ColoresVoz.rojo : ColoresVoz.azul)
                .frame(width: m.grabando ? 108 : 92, height: m.grabando ? 108 : 92)
            Image(systemName: "mic.fill").font(.system(size: 36)).foregroundStyle(ColoresVoz.texto)
        }
        .frame(width: 112, height: 112)
        .contentShape(Circle())
        // Mantener pulsado con el dedo: se suelta al levantarlo.
        .gesture(DragGesture(minimumDistance: 0)
            .onChanged { _ in
                if !m.conElDedo {
                    m.conElDedo = true
                    m.empezar()
                }
            }
            .onEnded { _ in m.soltar() })
        .accessibilityLabel("Micrófono: mantén pulsado y habla")
    }
}

/// «Escribir un texto»: la guia escrita a mano.
struct HojaTextoGuia: View {
    @State var texto: String
    let listo: (String?) -> Void

    var body: some View {
        NavigationStack {
            TextEditor(text: $texto)
                .font(.system(size: 17))
                .padding(12)
                .navigationTitle("Escribir un texto")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) { Button("Cancelar") { listo(nil) } }
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Guardar") { listo(texto) }.keyboardShortcut("s", modifiers: .command)
                    }
                }
        }
    }
}

// MARK: - Abrir

extension Pronunciar {
    @MainActor static var abierto: Bool { Ventanita.abierta(nombre) }

    /// **Abre Pronunciar** con lo del chat que sirve de guia. Cada toma que
    /// se guarde llega por `alGuardar` (la ventanita sigue abierta).
    @MainActor
    static func abrir(delChat: [DelChat], resolver: @escaping (String) async -> URL?,
                      alGuardar: @escaping (TomaPronunciar) -> Void) {
        guard !abierto else { return }
        let m = ModeloPronunciar(delChat: delChat, resolver: resolver)
        m.alGuardar = alGuardar
        Ventanita.abrir(nombre, alCerrar: { m.soltarTodo() }) { cerrar in
            VistaPronunciar(m: m).onAppear { m.cerrar = cerrar }
        }
    }
}
