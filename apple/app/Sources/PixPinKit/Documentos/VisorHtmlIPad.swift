// **Un HTML** en su visor (el `VisorHtml.swift` del Mac): `WKWebView` con el
// fichero y lo que tenga a su lado (imagenes, estilos), el buscador del
// sistema y el modo noche comun de los visores (la luz invertida con el
// tono conservado, el `invert(1) hue-rotate(180deg)` del movil).
//
// **La tinta encima de la pagina** (Windows `visor_html.rs`, Mac
// `TintaWeb.swift`): el lapiz del riel (o apoyar el Apple Pencil) anota con
// las herramientas del lector. Va en `Nucleo.rutaDeTintaDePagina` (la de un
// adjunto viaja, `anot-<uid>`), en pixeles CSS del documento: quieta, va
// DENTRO de la pagina como un SVG (se desplaza y se acerca con ella sin
// retraso); mientras se arrastra un trazo la pinta una capa encima.
// **Exportar**: la pagina en PDF con lo anotado (`createPDF`), para
// compartir o imprimir.

import PixPinCore
import SwiftUI
import UIKit
import WebKit

@MainActor
final class ControladorHtmlIPad: NSObject, ObservableObject {
    let web: WKWebView
    let modelo = ModeloLectorIPad()
    /// El fichero y el nucleo (sin nucleo, no hay tinta).
    private(set) var url: URL?
    var nucleo: Nucleo?
    @Published var aviso: String?

    private var editor: EditorLienzo?
    private var leido: (version: UInt64, lienzo: LienzoLeido)?
    private var enviado: String?
    private var ultimaHerramienta = "lapiz"
    private let alDia = AlDiaLector()
    private var guardado: DispatchWorkItem?
    /// Mientras se arrastra un trazo (entonces lo pinta `capa`).
    private(set) var pulsando = false
    let capa = CapaTintaWebIPad()
    private(set) lazy var gesto: GestoTintaWebIPad = {
        let g = GestoTintaWebIPad()
        g.controlador = self
        g.cancelsTouchesInView = true
        return g
    }()

    override init() {
        let config = WKWebViewConfiguration()
        config.dataDetectorTypes = [.link]
        web = WKWebView(frame: .zero, configuration: config)
        web.isFindInteractionEnabled = true
        web.allowsBackForwardNavigationGestures = true
        web.isOpaque = false
        web.backgroundColor = .systemBackground
        super.init()
        capa.controlador = self
        web.scrollView.addGestureRecognizer(gesto)
        actualizarGestos()
    }

    func cargar(_ url: URL) {
        self.url = url
        web.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        abrirTinta(crear: false)
        actualizarGestos()
    }

    func buscar() {
        web.findInteraction?.presentFindNavigator(showingReplace: false)
    }

    /// El modo noche: un filtro sobre la pagina entera (las imagenes vuelven
    /// a su color con el mismo filtro otra vez; la tinta tambien).
    func ponerNoche(_ si: Bool) {
        let js = si
            ? """
              (function(){var s=document.getElementById('pixpin-noche');if(!s){s=document.createElement('style');s.id='pixpin-noche';document.documentElement.appendChild(s);}
              s.textContent='html{filter:invert(1) hue-rotate(180deg);background:#fff}img,video,picture,canvas,svg image,#pixpin-tinta{filter:invert(1) hue-rotate(180deg)}';})();
              """
            : "(function(){var s=document.getElementById('pixpin-noche');if(s){s.remove();}})();"
        web.evaluateJavaScript(js, completionHandler: nil)
    }

    /// La pagina termino de cargar: la tinta otra vez dentro.
    func alCargar() {
        enviado = nil
        enviarALaPagina()
    }

    // MARK: La tinta

    var conLapiz: Bool { url != nil && nucleo != nil }

    private func abrirTinta(crear: Bool) {
        guard editor == nil, let url, let nucleo else { return }
        let ruta = nucleo.rutaDeTintaDePagina(documento: url.path)
        guard crear || FileManager.default.fileExists(atPath: ruta),
              let ed = try? EditorLienzo.deFichero(ruta: ruta, crear: true) else { return }
        editor = ed
        aplicarHerramienta(ed)
        enviarALaPagina()
    }

    private var zoom: CGFloat { max(web.scrollView.zoomScale, 0.01) }

    private func lienzo() -> LienzoLeido? {
        guard let ed = editor else { return nil }
        let v = ed.version()
        if leido?.version != v, let l = try? leerLienzo(ed.plano()) { leido = (v, l) }
        return leido?.lienzo
    }

    /// Manda a la pagina lo anotado (un SVG en px CSS), si cambio.
    func enviarALaPagina() {
        let svg = SvgDeTintaIPad.svg(lienzo())
        guard svg != enviado else { return }
        enviado = svg
        let js = "(function(s){var d=document.getElementById('pixpin-tinta');if(!d){d=document.createElement('div');d.id='pixpin-tinta';"
            + "d.style.cssText='position:absolute;left:0;top:0;width:0;height:0;overflow:visible;pointer-events:none;z-index:2147483647';"
            + "document.documentElement.appendChild(d);}d.innerHTML=s;})(" + SvgDeTintaIPad.cadenaJS(svg) + ")"
        web.evaluateJavaScript(js, completionHandler: nil)
    }

    /// La capa de encima, mientras se arrastra: lo mismo que la pagina.
    func pintar(en ctx: CGContext, vista: UIView, sucio: CGRect) {
        guard pulsando, let l = lienzo(), !l.figuras.isEmpty else { return }
        let o = web.scrollView.convert(CGPoint.zero, to: vista)
        let k = zoom
        ctx.saveGState()
        ctx.translateBy(x: o.x, y: o.y)
        ctx.scaleBy(x: k, y: k)
        let visible = CGRect(x: (sucio.minX - o.x) / k, y: (sucio.minY - o.y) / k,
                             width: sucio.width / k, height: sucio.height / k)
        PintorTintaHoja(lienzo: l).pintar(en: ctx, limites: visible)
        ctx.restoreGState()
    }

    private func aplicarHerramienta(_ ed: EditorLienzo) {
        try? ed.herramienta(nombre: modelo.anotando ? modelo.herramienta : "lapiz")
        let c = modelo.color
        ed.tintaColor(r: c.r, g: c.g, b: c.b, a: 1)
    }

    private func actualizarGestos() {
        var tipos: [UITouch.TouchType] = conLapiz ? [.pencil] : []
        if modelo.anotando {
            tipos.append(.indirectPointer)
            if modelo.conDedo { tipos.append(.direct) }
        }
        gesto.allowedTouchTypes = tipos.map { NSNumber(value: $0.rawValue) }
        gesto.isEnabled = conLapiz
        web.scrollView.panGestureRecognizer.minimumNumberOfTouches = modelo.anotando && modelo.conDedo ? 2 : 1
    }

    func aceptar(_ t: UITouch) -> Bool {
        guard conLapiz else { return false }
        switch t.type {
        case .pencil:
            if !modelo.anotando { elegir(ultimaHerramienta) }
            return true
        case .direct: return modelo.anotando && modelo.conDedo
        default: return modelo.anotando
        }
    }

    func trazo(_ fase: FasePuntero, _ toques: [UITouch]) {
        if editor == nil { abrirTinta(crear: true) }
        guard let ed = editor else { return }
        if fase == .pulsar { pulsando = true }
        let k = zoom
        for t in toques {
            let p = t.preciseLocation(in: web.scrollView)
            let presion: Float? = t.type == .pencil && t.maximumPossibleForce > 0
                ? Float(min(max(t.force / t.maximumPossibleForce, 0), 1)) : nil
            _ = ed.puntero(fase: fase, x: Float(p.x / k), y: Float(p.y / k), presion: presion, modificadores: 0,
                           zoom: Float(k))
        }
        capa.setNeedsDisplay()
        if fase == .soltar || fase == .cancelar {
            modelo.puedeDeshacer = ed.puedeDeshacer()
            // Primero la pagina y luego se vacia la capa: sin parpadeo.
            enviarALaPagina()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.06) { [weak self] in
                MainActor.assumeIsolated {
                    self?.pulsando = false
                    self?.capa.setNeedsDisplay()
                }
            }
            programarGuardado()
        }
    }

    private func programarGuardado() {
        guardado?.cancel()
        guard let ed = editor else { return }
        let alDia = self.alDia, ruta = url?.path ?? ""
        let tarea = DispatchWorkItem {
            guard ed.sinGuardar() else { return }
            do {
                if try alDia.guardar(0, ed).escrito {
                    DispatchQueue.main.async {
                        NotificationCenter.default.post(name: .pixpinDocumentoAnotado, object: ruta)
                        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                    }
                }
            } catch {
                NSLog("PixPin: no se pudo guardar la tinta de la página: %@", "\(error)")
            }
        }
        guardado = tarea
        DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + 0.8, execute: tarea)
    }

    func guardarYa() {
        guard guardado != nil else { return }
        guardado?.perform()
        guardado = nil
    }

    // MARK: Exportar

    /// **La pagina en PDF con lo anotado** (la tinta ya va dentro), para
    /// compartirla con el original o imprimirla.
    func exportar(imprimir: Bool) {
        guard let url else { return }
        enviarALaPagina()
        let titulo = url.deletingPathExtension().lastPathComponent
        web.createPDF(configuration: WKPDFConfiguration()) { [weak self] r in
            MainActor.assumeIsolated {
                guard let self else { return }
                guard case let .success(datos) = r else {
                    self.aviso = "No se pudo hacer el PDF de la página."
                    return
                }
                if imprimir {
                    SalidaDocumentoIPad.imprimir(datos, titulo: titulo, desde: self.web)
                    return
                }
                let dir = FileManager.default.temporaryDirectory.appendingPathComponent("web-\(UUID().uuidString)")
                try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
                let pdf = dir.appendingPathComponent(titulo + (self.editor == nil ? "" : " (anotado)") + ".pdf")
                guard (try? datos.write(to: pdf, options: .atomic)) != nil else { return }
                SalidaDocumentoIPad.compartir([url, pdf], desde: self.web) {
                    try? FileManager.default.removeItem(at: dir)
                }
            }
        }
    }
}

extension ControladorHtmlIPad: RielDeTinta {
    var conMarcadores: Bool { false }

    func alternarAnotar() {
        elegir(modelo.anotando ? "leer" : ultimaHerramienta)
    }

    func elegir(_ h: String) {
        guard conLapiz else { return }
        if h == "lapiz" || h == "resaltador" { ultimaHerramienta = h }
        modelo.herramienta = h
        if h == "resaltador", modelo.color == ColorTinta.todos[0] {
            modelo.color = ColorTinta.todos[4]
        } else if h == "lapiz", modelo.color == ColorTinta.todos[4] {
            modelo.color = ColorTinta.todos[0]
        }
        if h == "leer" {
            guardarYa()
        } else if let ed = editor {
            aplicarHerramienta(ed)
        } else {
            abrirTinta(crear: true)
        }
        actualizarGestos()
    }

    func ponerColor(_ c: ColorTinta) {
        modelo.color = c
        if !modelo.anotando || modelo.herramienta == "borrador" { elegir(ultimaHerramienta) }
        if let ed = editor { aplicarHerramienta(ed) }
    }

    func deshacer() {
        guard let ed = editor, ed.deshacer() else { return }
        modelo.puedeDeshacer = ed.puedeDeshacer()
        enviarALaPagina()
        programarGuardado()
    }

    func rehacer() {
        guard let ed = editor, ed.rehacer() else { return }
        modelo.puedeDeshacer = true
        enviarALaPagina()
        programarGuardado()
    }

    func alternarDedo() {
        modelo.conDedo.toggle()
        actualizarGestos()
    }

    func ir(a m: MarcaDocumento) {}
    func quitarMarca(_ m: MarcaDocumento) {}
    func quitarMarcas() {}
    func rotulo(de m: MarcaDocumento) -> String { "" }
}

/// La capa del trazo que se arrastra, encima de la pagina (no recibe toques).
final class CapaTintaWebIPad: UIView {
    weak var controlador: ControladorHtmlIPad?

    override init(frame: CGRect) {
        super.init(frame: frame)
        isUserInteractionEnabled = false
        isOpaque = false
        backgroundColor = .clear
        contentMode = .redraw
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    override func draw(_ rect: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        controlador?.pintar(en: ctx, vista: self, sucio: rect)
    }
}

private struct CapaTintaWebVista: UIViewRepresentable {
    let controlador: ControladorHtmlIPad
    func makeUIView(context: Context) -> CapaTintaWebIPad { controlador.capa }
    func updateUIView(_ v: CapaTintaWebIPad, context: Context) {}
}

/// El gesto del lapiz sobre la pagina: empieza en cuanto se apoya.
final class GestoTintaWebIPad: UIGestureRecognizer {
    weak var controlador: ControladorHtmlIPad?
    private var activo: UITouch?

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        guard activo == nil, let t = touches.first, let c = controlador, c.aceptar(t) else {
            for t in touches where t !== activo { ignore(t, for: event) }
            return
        }
        activo = t
        state = .began
        c.trazo(.pulsar, [t])
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let a = activo, touches.contains(a), let c = controlador else { return }
        c.trazo(.mover, event.coalescedTouches(for: a) ?? [a])
        state = .changed
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let a = activo, touches.contains(a), let c = controlador else { return }
        c.trazo(.soltar, [a])
        activo = nil
        state = .ended
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let a = activo, touches.contains(a), let c = controlador else { return }
        c.trazo(.cancelar, [a])
        activo = nil
        state = .cancelled
    }

    override func reset() {
        super.reset()
        activo = nil
    }
}

/// **La tinta como SVG** en px CSS del documento (`SvgDeTinta` del Mac, sin
/// las fotos), para ponerla dentro de la pagina. Pura: se prueba sola.
enum SvgDeTintaIPad {
    static func svg(_ l: LienzoLeido?) -> String {
        var o = "<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1' style='position:absolute;left:0;top:0;overflow:visible'>"
        for f in l?.figuras ?? [] {
            switch f {
            case let .relleno(p, c):
                o += "<path d='\(d(p))' fill='\(color(c))' fill-opacity='\(c.a)' fill-rule='evenodd'/>"
            case let .tinta(p, c):
                o += "<path d='\(d(p))' fill='\(color(c))' fill-opacity='\(c.a)'/>"
            case let .linea(p, c, grosor, estilo):
                let g = Double(grosor)
                let raya = estilo == 1 ? " stroke-dasharray='\(g * 4) \(g * 3)'"
                    : estilo == 2 ? " stroke-dasharray='0.01 \(g * 2.5)'"
                    : estilo == 3 ? " stroke-dasharray='\(g * 6) \(g * 2.5) 0.01 \(g * 2.5)'" : ""
                o += "<path d='\(d(p))' fill='none' stroke='\(color(c))' stroke-opacity='\(c.a)' stroke-width='\(g)'"
                    + " stroke-linecap='round' stroke-linejoin='round'\(raya)/>"
            case let .texto(t, origen, tam, _, familia, negrita, cursiva, c):
                o += "<text x='\(origen.x)' y='\(origen.y + tam)' font-size='\(tam)' font-family='\(xml(familia))' fill='\(color(c))'"
                    + (negrita ? " font-weight='bold'" : "") + (cursiva ? " font-style='italic'" : "") + ">\(xml(t))</text>"
            case let .rotulo(t, origen, tam, familia, c, _, _, centro, angulo):
                o += "<text x='\(origen.x)' y='\(origen.y + tam)' font-size='\(tam)' font-family='\(xml(familia))' fill='\(color(c))'"
                    + " transform='rotate(\(angulo * 180 / .pi) \(centro.x) \(centro.y))'>\(xml(t))</text>"
            case .imagen, .velo:
                continue
            }
        }
        return o + "</svg>"
    }

    /// El trazado de un `CGPath` como `d` de SVG.
    static func d(_ p: CGPath) -> String {
        var s = ""
        func n(_ v: CGFloat) -> String { String(format: "%.2f", Double(v)) }
        p.applyWithBlock { e in
            let q = e.pointee.points
            switch e.pointee.type {
            case .moveToPoint: s += "M\(n(q[0].x)) \(n(q[0].y))"
            case .addLineToPoint: s += "L\(n(q[0].x)) \(n(q[0].y))"
            case .addQuadCurveToPoint: s += "Q\(n(q[0].x)) \(n(q[0].y)) \(n(q[1].x)) \(n(q[1].y))"
            case .addCurveToPoint:
                s += "C\(n(q[0].x)) \(n(q[0].y)) \(n(q[1].x)) \(n(q[1].y)) \(n(q[2].x)) \(n(q[2].y))"
            case .closeSubpath: s += "Z"
            @unknown default: break
            }
        }
        return s
    }

    static func color(_ c: ColorLienzo) -> String {
        func h(_ v: CGFloat) -> String { String(format: "%02X", Int((min(max(v, 0), 1) * 255).rounded())) }
        return "#" + h(c.r) + h(c.g) + h(c.b)
    }

    static func xml(_ t: String) -> String {
        t.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;").replacingOccurrences(of: "'", with: "&apos;")
    }

    /// Una cadena de JavaScript con el texto dentro, sin romperse.
    static func cadenaJS(_ t: String) -> String {
        let datos = (try? JSONSerialization.data(withJSONObject: [t])) ?? Data("[\"\"]".utf8)
        let s = String(decoding: datos, as: UTF8.self)
        return String(s.dropFirst().dropLast())
    }
}

private struct WebHtmlIPad: UIViewRepresentable {
    let controlador: ControladorHtmlIPad
    let noche: Bool

    func makeCoordinator() -> Delegado { Delegado(controlador: controlador, noche: noche) }

    func makeUIView(context: Context) -> WKWebView {
        controlador.web.navigationDelegate = context.coordinator
        return controlador.web
    }

    func updateUIView(_ v: WKWebView, context: Context) {
        if context.coordinator.noche != noche {
            context.coordinator.noche = noche
            controlador.ponerNoche(noche)
        }
    }

    @MainActor
    final class Delegado: NSObject, WKNavigationDelegate {
        weak var controlador: ControladorHtmlIPad?
        var noche: Bool

        init(controlador: ControladorHtmlIPad, noche: Bool) {
            self.controlador = controlador
            self.noche = noche
        }

        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            if noche { controlador?.ponerNoche(true) }
            controlador?.alCargar()
        }

        /// Los enlaces de fuera se abren en Safari; los del propio fichero, aqui.
        func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction,
                     decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void) {
            if let u = navigationAction.request.url, !u.isFileURL, navigationAction.navigationType == .linkActivated {
                UIApplication.shared.open(u)
                decisionHandler(.cancel)
                return
            }
            decisionHandler(.allow)
        }
    }
}

struct VisorHtmlIPad: View {
    let url: URL
    let titulo: String
    /// Sin nucleo no hay tinta (no se sabe donde guardarla).
    var nucleo: Nucleo?
    @StateObject private var c = ControladorHtmlIPad()
    @ObservedObject private var noche = ModoNoche.compartido

    var body: some View {
        ZStack(alignment: .top) {
            Color(uiColor: .systemBackground).ignoresSafeArea()
            WebHtmlIPad(controlador: c, noche: noche.activo)
                .overlay { CapaTintaWebVista(controlador: c).allowsHitTesting(false) }
                .overlay(alignment: .topTrailing) {
                    if c.conLapiz {
                        RielHojaIPad(c: c, modelo: c.modelo)
                            .padding(.top, 12)
                            .padding(.trailing, 12)
                    }
                }
                .ignoresSafeArea(edges: .bottom)
                .padding(.top, 58)
            BarraVisorIPad(tipo: "HTML", icono: "globe", titulo: titulo, alFijar: {
                if !GestorPinesIPad.compartido.pinear(url: url) { GestorPinesIPad.compartido.pinearLoQueSeVe() }
            }) {
                Button { c.buscar() } label: { Image(systemName: "magnifyingglass") }
                    .keyboardShortcut("f", modifiers: .command)
                    .help("Buscar (⌘F)")
                Button { c.web.reload() } label: { Image(systemName: "arrow.clockwise") }
                    .help("Volver a cargar")
                Menu {
                    Button { c.exportar(imprimir: false) } label: {
                        Label("Exportar en PDF con lo anotado…", systemImage: "doc.richtext")
                    }
                    Button { c.exportar(imprimir: true) } label: { Label("Imprimir…", systemImage: "printer") }
                    ShareLink(item: url) { Label("Compartir el original…", systemImage: "square.and.arrow.up") }
                } label: {
                    Image(systemName: "square.and.arrow.up")
                }
                .help("Exportar")
            }
            if let aviso = c.aviso {
                Text(aviso)
                    .font(.callout)
                    .padding(.horizontal, 14).padding(.vertical, 8)
                    .background(.regularMaterial, in: Capsule())
                    .padding(.top, 70)
                    .onTapGesture { c.aviso = nil }
            }
        }
        .onAppear {
            c.nucleo = nucleo
            c.cargar(url)
        }
        .onDisappear { c.guardarYa() }
    }
}
