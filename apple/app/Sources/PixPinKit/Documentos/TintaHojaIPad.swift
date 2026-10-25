// **La tinta de la hoja de calculo y su riel** (Windows
// `visor_hoja/riel.rs` y `visor_hoja/capa.rs`, Mac `RielHoja.swift`): el
// mismo riel de los PDF a la derecha, con el marcador y el lapiz.
//
// - **El lapiz**: anotar encima de las celdas con el Apple Pencil (o con el
//   dedo, «Dibujar con el dedo»), con las herramientas del lector. La tinta
//   de cada hoja va en `anot-<uid>-h<i>` (`Nucleo.rutaDeTintaDeHoja`), la
//   que viaja con el libro del chat, en pixeles de Excel al 100 %.
// - **Las unidades** (`visor_hoja/capa.rs`): la tinta va en la rejilla de
//   Excel del libro (`rejillaDelLibro`, anchos y altos de verdad) y cada
//   punto se lleva a **la misma fraccion de la misma celda** de la rejilla
//   de aqui (filas de 30 puntos, columnas de 96): lo anotado cae en su celda
//   en Windows, el Mac y el iPad aunque las celdas midan distinto.
// - **El marcador**: en la celda activa; `x` la columna e `y` la hoja por
//   10 000 000 mas la fila, en la linea `marcas` del libro (la del PDF).
//
// La rejilla de SwiftUI no deja meter un gesto propio: un marcador invisible
// (`MarcaRejillaHoja`) en el fondo del contenido encuentra el `UIScrollView`
// de debajo; el gesto del lapiz va en el y la capa de la tinta, encima de la
// rejilla, sigue su desplazamiento.

import Combine
import PixPinCore
import SwiftUI
import UIKit

// MARK: - Las cuentas (puras, se prueban solas)

/// **De la rejilla de aqui a la de la tinta y al reves**, celda a celda.
public struct RejillaDeTintaHoja: Equatable, Sendable {
    /// Los bordes de las columnas en la vista (en el contenido de la
    /// rejilla), desde el de la columna 0.
    public var vistaX: [CGFloat]
    /// Donde empieza la fila 0 en la vista y lo que mide cada fila.
    public var vistaY0: CGFloat
    public var altoFila: CGFloat
    /// Los bordes de la tinta (pixeles de Excel) y lo que mide lo que va mas
    /// alla de ellos.
    public var tintaX: [CGFloat]
    public var tintaY: [CGFloat]
    public var anchoTinta: CGFloat
    public var altoTinta: CGFloat

    public init(vistaX: [CGFloat], vistaY0: CGFloat, altoFila: CGFloat,
                tintaX: [CGFloat], tintaY: [CGFloat], anchoTinta: CGFloat, altoTinta: CGFloat) {
        self.vistaX = vistaX.isEmpty ? [0] : vistaX
        self.vistaY0 = vistaY0
        self.altoFila = max(altoFila, 1)
        self.tintaX = tintaX.isEmpty ? [0] : tintaX
        self.tintaY = tintaY.isEmpty ? [0] : tintaY
        self.anchoTinta = max(anchoTinta, 1)
        self.altoTinta = max(altoTinta, 1)
    }

    /// El borde `i` de una lista, alargada con `paso` (mas alla o antes).
    static func borde(_ v: [CGFloat], _ i: Int, paso: CGFloat) -> CGFloat {
        if i < 0 { return v[0] + CGFloat(i) * paso }
        if i < v.count { return v[i] }
        return v[v.count - 1] + CGFloat(i - v.count + 1) * paso
    }

    /// En que tramo cae `x` (con `x` antes del primero, tramos negativos).
    static func tramo(_ v: [CGFloat], _ x: CGFloat, paso: CGFloat) -> Int {
        if x < v[0] { return Int(((x - v[0]) / paso).rounded(.down)) }
        if x >= v[v.count - 1] { return v.count - 1 + Int(((x - v[v.count - 1]) / paso).rounded(.down)) }
        var lo = 0, hi = v.count - 1
        while hi - lo > 1 {
            let m = (lo + hi) / 2
            if v[m] <= x { lo = m } else { hi = m }
        }
        return lo
    }

    private var anchoVista: CGFloat {
        vistaX.count > 1 ? max(vistaX[vistaX.count - 1] - vistaX[vistaX.count - 2], 1) : 96
    }

    private static func llevar(_ x: CGFloat, de a: [CGFloat], pasoA: CGFloat, a b: [CGFloat], pasoB: CGFloat) -> CGFloat {
        let i = tramo(a, x, paso: pasoA)
        let a0 = borde(a, i, paso: pasoA), a1 = borde(a, i + 1, paso: pasoA)
        let b0 = borde(b, i, paso: pasoB), b1 = borde(b, i + 1, paso: pasoB)
        let f = a1 > a0 ? (x - a0) / (a1 - a0) : 0
        return b0 + f * (b1 - b0)
    }

    private var filasVista: [CGFloat] { [vistaY0] }

    /// De un punto del contenido de la rejilla a la tinta.
    public func aTinta(_ p: CGPoint) -> CGPoint {
        let x = RejillaDeTintaHoja.llevar(p.x, de: vistaX, pasoA: anchoVista, a: tintaX, pasoB: anchoTinta)
        let y = RejillaDeTintaHoja.llevar(p.y, de: filasVista, pasoA: altoFila, a: tintaY, pasoB: altoTinta)
        return CGPoint(x: x, y: y)
    }

    /// De la tinta al contenido de la rejilla.
    public func aVista(_ q: CGPoint) -> CGPoint {
        let x = RejillaDeTintaHoja.llevar(q.x, de: tintaX, pasoA: anchoTinta, a: vistaX, pasoB: anchoVista)
        let y = RejillaDeTintaHoja.llevar(q.y, de: tintaY, pasoA: altoTinta, a: filasVista, pasoB: altoFila)
        return CGPoint(x: x, y: y)
    }

    /// Cuantos puntos de la vista mide una unidad de la tinta cerca de `q`
    /// (para el grosor del trazo y el tamano de los textos).
    public func escala(en q: CGPoint) -> CGFloat {
        let a = aVista(q), b = aVista(CGPoint(x: q.x + 10, y: q.y + 10))
        return max(((b.x - a.x) + (b.y - a.y)) / 20, 0.01)
    }

    /// La caja de un grafico (o de lo que sea) dada en celdas mas pixeles
    /// de Excel, en la vista.
    public func caja(columna: Int, fila: Int, dx: CGFloat, dy: CGFloat,
                     hasta: (columna: Int, fila: Int, dx: CGFloat, dy: CGFloat)?,
                     ancho: CGFloat, alto: CGFloat) -> CGRect {
        let tx = RejillaDeTintaHoja.borde(tintaX, columna, paso: anchoTinta) + dx
        let ty = RejillaDeTintaHoja.borde(tintaY, fila, paso: altoTinta) + dy
        let fin: CGPoint
        if let h = hasta {
            fin = CGPoint(x: RejillaDeTintaHoja.borde(tintaX, h.columna, paso: anchoTinta) + h.dx,
                          y: RejillaDeTintaHoja.borde(tintaY, h.fila, paso: altoTinta) + h.dy)
        } else {
            fin = CGPoint(x: tx + ancho, y: ty + alto)
        }
        let a = aVista(CGPoint(x: tx, y: ty)), b = aVista(fin)
        return CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(b.x - a.x), height: abs(b.y - a.y))
    }
}

/// Un trazado llevado punto a punto por `f`.
func caminoLlevado(_ p: CGPath, _ f: (CGPoint) -> CGPoint) -> CGPath {
    let m = CGMutablePath()
    withoutActuallyEscaping(f) { f in
        p.applyWithBlock { el in
            let e = el.pointee
            switch e.type {
            case .moveToPoint: m.move(to: f(e.points[0]))
            case .addLineToPoint: m.addLine(to: f(e.points[0]))
            case .addQuadCurveToPoint: m.addQuadCurve(to: f(e.points[1]), control: f(e.points[0]))
            case .addCurveToPoint:
                m.addCurve(to: f(e.points[2]), control1: f(e.points[0]), control2: f(e.points[1]))
            case .closeSubpath: m.closeSubpath()
            @unknown default: break
            }
        }
    }
    return m
}

/// El lienzo de la tinta (en unidades de Excel) llevado a la rejilla de aqui.
func lienzoEnLaRejilla(_ l: LienzoLeido, _ r: RejillaDeTintaHoja) -> LienzoLeido {
    let f: (CGPoint) -> CGPoint = { r.aVista($0) }
    let figuras: [FiguraLienzo] = l.figuras.map { fig in
        switch fig {
        case let .relleno(p, c): return .relleno(caminoLlevado(p, f), c)
        case let .tinta(p, c): return .tinta(caminoLlevado(p, f), c)
        case let .linea(p, c, grosor, estilo):
            let k = r.escala(en: p.boundingBoxOfPath.origin)
            return .linea(caminoLlevado(p, f), c, grosor: grosor * k, estilo: estilo)
        case let .velo(p, c): return .velo(hueco: caminoLlevado(p, f), c)
        case let .texto(t, o, tam, anchoMax, familia, negrita, cursiva, c):
            let k = r.escala(en: o)
            return .texto(t, origen: f(o), tam: tam * k, anchoMax: anchoMax * k, familia: familia,
                          negrita: negrita, cursiva: cursiva, c)
        case let .rotulo(t, o, tam, familia, c, halo, grosorHalo, centro, angulo):
            let k = r.escala(en: o)
            return .rotulo(t, origen: f(o), tam: tam * k, familia: familia, c, halo: halo,
                           grosorHalo: grosorHalo * k, centro: f(centro), angulo: angulo)
        case let .imagen(id, caja, opacidad, angulo, recorte, natural):
            let a = f(caja.origin), b = f(CGPoint(x: caja.maxX, y: caja.maxY))
            return .imagen(id: id, caja: CGRect(x: a.x, y: a.y, width: b.x - a.x, height: b.y - a.y),
                           opacidad: opacidad, angulo: angulo, recorte: recorte, natural: natural)
        }
    }
    return LienzoLeido(fondo: l.fondo, caja: nil, figuras: figuras)
}

/// La marca de un marcador en la celda (`riel::sitio_de_marca`).
public func sitioDeMarcaDeHoja(hoja: Int, fila: Int, columna: Int) -> (x: Double, y: Double) {
    (Double(columna), Double(hoja) * ControladorTintaHoja.porHoja + Double(fila))
}

/// De que hoja, fila y columna es un marcador (`riel::celda_de_marca`).
public func celdaDeMarcaDeHoja(_ m: MarcaDocumento) -> (hoja: Int, fila: Int, columna: Int) {
    let y = max(m.y, 0).rounded()
    let h = (y / ControladorTintaHoja.porHoja).rounded(.down)
    return (Int(h), Int(max(y - h * ControladorTintaHoja.porHoja, 0)), Int(max(m.x, 0).rounded()))
}

/// La pestana `pestana` (en `posicion` de `total`) entre las hojas del libro:
/// por su nombre, o por «archivo · hoja» (como las nombra el nucleo al leer
/// un libro de varias), o la misma posicion si no se encuentra.
public func indiceEnElLibro(pestana: String, posicion: Int, hojasDelLibro: [String]) -> Int? {
    if let i = hojasDelLibro.firstIndex(of: pestana) { return i }
    if let i = hojasDelLibro.firstIndex(where: { pestana.hasSuffix(" · " + $0) }) { return i }
    return posicion < hojasDelLibro.count ? posicion : nil
}

// MARK: - El controlador

/// **La tinta, los marcadores y los graficos de la hoja que se ve.**
@MainActor
final class ControladorTintaHoja: ObservableObject {
    static let porHoja: Double = 10_000_000

    let modelo = ModeloLectorIPad()
    /// El libro (con fichero): sin el, ni tinta ni marcadores.
    let url: URL?
    let nucleo: Nucleo
    weak var hoja: ModeloHojaIPad?
    @Published private(set) var graficos: [GraficoHoja] = []
    /// La rejilla de la tinta de la hoja que se ve (al llegar, se puede
    /// anotar y se pintan la tinta y los graficos).
    @Published private(set) var rejilla: RejillaDeTintaHoja?

    private var editor: EditorLienzo?
    private var leido: (version: UInt64, lienzo: LienzoLeido)?
    private var nombresDelLibro: [String] = []
    private var tintaDe: (hoja: Int, rejilla: RejillaTinta?)?
    private var indiceHoja = 0
    private var ultimaHerramienta = "lapiz"
    private let alDia = AlDiaLector()
    private let colaGuardar = DispatchQueue(label: "pixpin.hoja.tinta", qos: .utility)
    private var guardado: DispatchWorkItem?

    let capa = CapaTintaHojaIPad()
    private(set) lazy var gesto: GestoTintaHojaIPad = {
        let g = GestoTintaHojaIPad()
        g.controlador = self
        g.cancelsTouchesInView = true
        g.delaysTouchesBegan = false
        return g
    }()
    weak var marcador: UIView?
    private weak var desplazador: UIScrollView?
    private var mirando: NSKeyValueObservation?

    init(url: URL?, nucleo: Nucleo) {
        self.url = url
        self.nucleo = nucleo
        capa.controlador = self
        if let url {
            modelo.marcas = nucleo.lecturaDeDocumento(documento: url.path).marcas
            let ruta = url.path
            Task { [weak self] in
                let (nombres, graficos) = await Task.detached(priority: .utility) {
                    (hojasDelLibro(ruta: ruta), graficosDelLibro(ruta: ruta))
                }.value
                guard let self else { return }
                self.nombresDelLibro = nombres
                self.todosLosGraficos = graficos
                self.cambioDeHoja()
            }
        }
        actualizarGestos()
    }

    private var todosLosGraficos: [GraficoHoja] = []

    var conLapiz: Bool { url != nil }

    // MARK: La rejilla de debajo

    /// El marcador del contenido de la rejilla ya esta en una ventana: se
    /// busca su `UIScrollView` y se le pone el gesto del lapiz.
    func enganchar(_ marca: UIView) {
        marcador = marca
        var v: UIView? = marca.superview
        while let x = v, !(x is UIScrollView) { v = x.superview }
        guard let s = v as? UIScrollView, s !== desplazador else { return }
        desplazador?.removeGestureRecognizer(gesto)
        desplazador = s
        s.addGestureRecognizer(gesto)
        mirando = s.observe(\.contentOffset, options: [.new]) { [weak self] _, _ in
            DispatchQueue.main.async { MainActor.assumeIsolated { self?.capa.setNeedsDisplay() } }
        }
        actualizarGestos()
        capa.setNeedsDisplay()
    }

    /// Desplaza la rejilla para que se vea la celda (arriba a la izquierda).
    func verCelda(fila: Int, columna: Int) {
        guard let s = desplazador, let h = hoja else { return }
        let x = h.izquierda(columna) - ModeloHojaIPad.anchoNumeros
        let y = CGFloat(fila) * ModeloHojaIPad.altoFila
        let maxX = max(s.contentSize.width - s.bounds.width, 0)
        let maxY = max(s.contentSize.height - s.bounds.height, 0)
        s.setContentOffset(CGPoint(x: min(max(x, 0), maxX), y: min(max(y, 0), maxY)), animated: true)
    }

    // MARK: Hojas

    /// Otra hoja delante (o los anchos cambiaron): su tinta, su rejilla y
    /// sus graficos.
    func cambioDeHoja() {
        guard let h = hoja else { return }
        let i = h.activa
        if i != indiceHoja || editor == nil {
            guardarYa()
            indiceHoja = i
            editor = nil
            leido = nil
            abrirTinta(crear: modelo.anotando)
        }
        let pestana = i < h.nombres.count ? h.nombres[i] : ""
        let enLibro = indiceEnElLibro(pestana: pestana, posicion: i, hojasDelLibro: nombresDelLibro)
        graficos = todosLosGraficos.filter { g in
            enLibro.map { Int(g.orden) == $0 } ?? false
        }
        cargarRejilla(enLibro)
    }

    /// La rejilla de Excel de la hoja (fuera del hilo principal la primera
    /// vez) y la de aqui con los anchos de ahora.
    func cargarRejilla(_ enLibro: Int?) {
        guard let h = hoja else { return }
        let columnas = h.columnas, filas = h.filas
        if let t = tintaDe, t.hoja == indiceHoja {
            ponerRejilla(t.rejilla, columnas: columnas)
            return
        }
        guard let url, let enLibro else {
            tintaDe = (hoja: indiceHoja, rejilla: nil)
            ponerRejilla(nil, columnas: columnas)
            return
        }
        let ruta = url.path, esta = indiceHoja
        Task { [weak self] in
            let r = await Task.detached(priority: .utility) {
                rejillaDelLibro(ruta: ruta, hoja: UInt32(enLibro), columnas: UInt32(max(columnas, 64)),
                                filas: UInt32(max(filas, 400)))
            }.value
            guard let self, self.indiceHoja == esta else { return }
            self.tintaDe = (hoja: esta, rejilla: r)
            self.ponerRejilla(r, columnas: self.hoja?.columnas ?? columnas)
        }
    }

    private func ponerRejilla(_ t: RejillaTinta?, columnas: Int) {
        guard let h = hoja else { return }
        var vx: [CGFloat] = [ModeloHojaIPad.anchoNumeros]
        vx.reserveCapacity(columnas + 1)
        for c in 0..<columnas { vx.append(vx[c] + h.ancho(c)) }
        // Sin libro de Excel: la rejilla de una tabla del chat en Windows
        // (`tabla_xlsx`: 88 x 22, con los anchos puestos a mano).
        let tx: [CGFloat], ty: [CGFloat], aw: CGFloat, ah: CGFloat
        if let t {
            tx = t.xs.map { CGFloat($0) }
            ty = t.ys.map { CGFloat($0) }
            aw = CGFloat(t.anchoDefecto)
            ah = CGFloat(t.altoDefecto)
        } else {
            var x: [CGFloat] = [0]
            for c in 0..<columnas { x.append(x[c] + h.anchoPuesto(c).map { CGFloat($0) } ?? 88) }
            tx = x
            ty = [0]
            aw = 88
            ah = 22
        }
        rejilla = RejillaDeTintaHoja(vistaX: vx, vistaY0: 26, altoFila: ModeloHojaIPad.altoFila,
                                     tintaX: tx, tintaY: ty, anchoTinta: aw, altoTinta: ah)
        leido = nil
        capa.setNeedsDisplay()
    }

    /// Cambio un ancho de columna: la rejilla de aqui es otra.
    func cambioDeAnchos() {
        guard let h = hoja else { return }
        ponerRejilla(tintaDe?.hoja == indiceHoja ? tintaDe?.rejilla : nil, columnas: h.columnas)
    }

    private func abrirTinta(crear: Bool) {
        guard let url else { return }
        let ruta = nucleo.rutaDeTintaDeHoja(libro: url.path, hoja: UInt32(indiceHoja))
        guard crear || FileManager.default.fileExists(atPath: ruta),
              let ed = try? EditorLienzo.deFichero(ruta: ruta, crear: true) else { return }
        editor = ed
        aplicarHerramienta(ed)
        capa.setNeedsDisplay()
    }

    // MARK: Pintar

    func pintar(en ctx: CGContext, vista: UIView, sucio: CGRect) {
        guard let ed = editor, let r = rejilla, let marca = marcador else { return }
        let v = ed.version()
        if leido?.version != v, let l = try? leerLienzo(ed.plano()) {
            leido = (v, lienzoEnLaRejilla(l, r))
        }
        guard let l = leido?.lienzo, !l.figuras.isEmpty else { return }
        let o = marca.convert(CGPoint.zero, to: vista)
        ctx.saveGState()
        // Sin pisar las letras de las columnas (fijas arriba).
        ctx.clip(to: CGRect(x: 0, y: 26, width: vista.bounds.width, height: max(vista.bounds.height - 26, 0)))
        ctx.translateBy(x: o.x, y: o.y)
        PintorTintaHoja(lienzo: l).pintar(en: ctx, limites: sucio.offsetBy(dx: -o.x, dy: -o.y))
        ctx.restoreGState()
    }

    // MARK: El lapiz

    private func aplicarHerramienta(_ ed: EditorLienzo) {
        try? ed.herramienta(nombre: modelo.anotando ? modelo.herramienta : "lapiz")
        let c = modelo.color
        ed.tintaColor(r: c.r, g: c.g, b: c.b, a: 1)
    }

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
        } else {
            modelo.poniendoMarca = false
            if let ed = editor { aplicarHerramienta(ed) } else { abrirTinta(crear: true) }
        }
        actualizarGestos()
    }

    func ponerColor(_ c: ColorTinta) {
        modelo.color = c
        if !modelo.anotando || modelo.herramienta == "borrador" { elegir(ultimaHerramienta) }
        if let ed = editor { aplicarHerramienta(ed) }
    }

    func alternarDedo() {
        modelo.conDedo.toggle()
        actualizarGestos()
    }

    /// El lapiz siempre puede anotar (apoyarlo empieza); el dedo desplaza,
    /// o pinta con «Dibujar con el dedo» (y se desplaza con dos).
    private func actualizarGestos() {
        var tipos: [UITouch.TouchType] = conLapiz ? [.pencil] : []
        if modelo.anotando {
            tipos.append(.indirectPointer)
            if modelo.conDedo { tipos.append(.direct) }
        }
        gesto.allowedTouchTypes = tipos.map { NSNumber(value: $0.rawValue) }
        gesto.isEnabled = conLapiz
        desplazador?.panGestureRecognizer.minimumNumberOfTouches = modelo.anotando && modelo.conDedo ? 2 : 1
    }

    func aceptar(_ t: UITouch) -> Bool {
        guard conLapiz, rejilla != nil else { return false }
        switch t.type {
        case .pencil:
            if !modelo.anotando { elegir(ultimaHerramienta) }
            return true
        case .direct: return modelo.anotando && modelo.conDedo
        default: return modelo.anotando
        }
    }

    func trazo(_ fase: FasePuntero, _ toques: [UITouch]) {
        guard let marca = marcador, let r = rejilla else { return }
        if editor == nil { abrirTinta(crear: true) }
        guard let ed = editor else { return }
        for t in toques {
            let q = r.aTinta(t.preciseLocation(in: marca))
            let presion: Float? = t.type == .pencil && t.maximumPossibleForce > 0
                ? Float(min(max(t.force / t.maximumPossibleForce, 0), 1)) : nil
            _ = ed.puntero(fase: fase, x: Float(q.x), y: Float(q.y), presion: presion, modificadores: 0,
                           zoom: Float(r.escala(en: q)))
        }
        capa.setNeedsDisplay()
        if fase == .soltar || fase == .cancelar {
            modelo.puedeDeshacer = ed.puedeDeshacer()
            programarGuardado()
        }
    }

    func deshacer() {
        guard let ed = editor, ed.deshacer() else { return }
        modelo.puedeDeshacer = ed.puedeDeshacer()
        capa.setNeedsDisplay()
        programarGuardado()
    }

    func rehacer() {
        guard let ed = editor, ed.rehacer() else { return }
        modelo.puedeDeshacer = true
        capa.setNeedsDisplay()
        programarGuardado()
    }

    // MARK: Guardar

    private func programarGuardado() {
        guardado?.cancel()
        guard let ed = editor else { return }
        let alDia = self.alDia, i = indiceHoja
        let tarea = DispatchWorkItem {
            ControladorTintaHoja.guardar(ed, i, alDia)
        }
        guardado = tarea
        colaGuardar.asyncAfter(deadline: .now() + 0.8, execute: tarea)
    }

    func guardarYa() {
        guardado?.cancel()
        guard let ed = editor else { return }
        let alDia = self.alDia, i = indiceHoja
        colaGuardar.async { ControladorTintaHoja.guardar(ed, i, alDia) }
    }

    private nonisolated static func guardar(_ ed: EditorLienzo, _ i: Int, _ alDia: AlDiaLector) {
        guard ed.sinGuardar() else { return }
        do {
            if try alDia.guardar(i, ed).escrito {
                DispatchQueue.main.async {
                    NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                }
            }
        } catch {
            NSLog("PixPin: no se pudo guardar la tinta de la hoja: %@", "\(error)")
        }
    }

    // MARK: Marcadores

    func ponerMarca(_ emoji: String) {
        modelo.poniendoMarca = false
        guard let url, let h = hoja else { return }
        let s = sitioDeMarcaDeHoja(hoja: h.activa, fila: h.fila, columna: h.columna)
        modelo.marcas = conMarca(modelo.marcas, x: s.x, y: s.y, emoji: emoji,
                                 ahora: Int64(Date().timeIntervalSince1970 * 1000))
        guardarMarcas(url)
    }

    func ir(a m: MarcaDocumento) {
        guard let h = hoja else { return }
        let c = celdaDeMarcaDeHoja(m)
        if c.hoja != h.activa, c.hoja < h.nombres.count { h.irAHoja(c.hoja) }
        h.elegir(c.fila, c.columna)
        verCelda(fila: c.fila, columna: c.columna)
    }

    func quitarMarca(_ m: MarcaDocumento) {
        modelo.marcas.removeAll { $0.id == m.id }
        if let url { guardarMarcas(url) }
    }

    func quitarMarcas() {
        modelo.marcas = []
        if let url { guardarMarcas(url) }
    }

    private func guardarMarcas(_ url: URL) {
        var l = nucleo.lecturaDeDocumento(documento: url.path)
        l.marcas = modelo.marcas
        try? nucleo.guardarLecturaDeDocumento(documento: url.path, lectura: l, altos: [])
    }
}

// MARK: - Las vistas de UIKit

/// La capa de la tinta, encima de la rejilla (del tamano de lo que se ve):
/// no recibe toques, los recoge el gesto puesto en el desplazador.
final class CapaTintaHojaIPad: UIView {
    weak var controlador: ControladorTintaHoja?

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

/// El gesto del lapiz en la hoja: empieza en cuanto se apoya.
final class GestoTintaHojaIPad: UIGestureRecognizer {
    weak var controlador: ControladorTintaHoja?
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
        let todos = event.coalescedTouches(for: a) ?? [a]
        c.trazo(.mover, todos)
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

/// La capa de la tinta en SwiftUI.
struct CapaTintaHojaVista: UIViewRepresentable {
    let controlador: ControladorTintaHoja
    func makeUIView(context: Context) -> CapaTintaHojaIPad { controlador.capa }
    func updateUIView(_ v: CapaTintaHojaIPad, context: Context) { v.setNeedsDisplay() }
}

/// El marcador invisible del contenido de la rejilla (ver arriba).
struct MarcaRejillaHoja: UIViewRepresentable {
    let controlador: ControladorTintaHoja

    final class Marca: UIView {
        var alEntrar: ((UIView) -> Void)?
        override func didMoveToWindow() {
            super.didMoveToWindow()
            if window != nil { alEntrar?(self) }
        }
    }

    func makeUIView(context: Context) -> Marca {
        let m = Marca()
        m.isUserInteractionEnabled = false
        m.backgroundColor = .clear
        let c = controlador
        m.alEntrar = { v in MainActor.assumeIsolated { c.enganchar(v) } }
        return m
    }

    func updateUIView(_ v: Marca, context: Context) {}
}

// MARK: - El riel

/// Lo que el riel de la tinta pide a quien lo monta (la hoja de calculo, la
/// pagina web).
@MainActor
protocol RielDeTinta: AnyObject {
    /// Si hay donde guardar la tinta (sin fichero, no hay lapiz).
    var conLapiz: Bool { get }
    /// Si se ponen marcadores.
    var conMarcadores: Bool { get }
    func alternarAnotar()
    func elegir(_ h: String)
    func ponerColor(_ c: ColorTinta)
    func deshacer()
    func rehacer()
    func alternarDedo()
    func ir(a m: MarcaDocumento)
    func quitarMarca(_ m: MarcaDocumento)
    func quitarMarcas()
    /// Lo que se escribe bajo el emoticono de un marcador.
    func rotulo(de m: MarcaDocumento) -> String
}

extension ControladorTintaHoja: RielDeTinta {
    var conMarcadores: Bool { url != nil }

    func rotulo(de m: MarcaDocumento) -> String {
        let celda = celdaDeMarcaDeHoja(m)
        return "\(letrasDeColumna(celda.columna))\(celda.fila + 1)"
    }
}

/// **El riel de la tinta** (el de los PDF, `RielLectorIPad`, para la hoja de
/// calculo y la pagina web): el marcador y el lapiz (con sus herramientas
/// al anotar) y los marcadores puestos.
struct RielHojaIPad: View {
    let c: any RielDeTinta
    @ObservedObject var modelo: ModeloLectorIPad

    var body: some View {
        VStack(alignment: .trailing, spacing: 10) {
            VStack(spacing: 2) {
                if c.conMarcadores {
                    BotonRielIPad(simbolo: "bookmark", ayuda: "Poner un marcador (⌘M)", encendido: modelo.poniendoMarca) {
                        modelo.poniendoMarca.toggle()
                    }
                    .keyboardShortcut("m", modifiers: .command)
                }
                if c.conLapiz {
                    BotonRielIPad(simbolo: "pencil.tip", ayuda: modelo.anotando ? "Dejar de anotar" : "Anotar",
                                  encendido: modelo.anotando) {
                        c.alternarAnotar()
                    }
                }
            }
            .modifier(FondoRielHoja())
            if modelo.anotando { herramientas }
            if c.conMarcadores, !modelo.marcas.isEmpty { marcadores }
        }
        .fixedSize()
    }

    private var herramientas: some View {
        VStack(spacing: 2) {
            BotonRielIPad(simbolo: "pencil", ayuda: "Lápiz", encendido: modelo.herramienta == "lapiz") {
                c.elegir("lapiz")
            }
            BotonRielIPad(simbolo: "highlighter", ayuda: "Resaltador", encendido: modelo.herramienta == "resaltador") {
                c.elegir("resaltador")
            }
            BotonRielIPad(simbolo: "eraser", ayuda: "Goma", encendido: modelo.herramienta == "borrador") {
                c.elegir("borrador")
            }
            Divider().frame(width: 28)
            ForEach(ColorTinta.todos) { col in
                Button { c.ponerColor(col) } label: {
                    Circle().fill(col.color)
                        .frame(width: 22, height: 22)
                        .overlay(Circle().strokeBorder(Color.primary.opacity(0.25), lineWidth: 1))
                        .overlay(Circle().strokeBorder(Paleta.v2.acento, lineWidth: modelo.color == col ? 3 : 0)
                            .padding(-3))
                        .frame(width: 36, height: 32)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(col.id.capitalized)
            }
            Divider().frame(width: 28)
            BotonRielIPad(simbolo: "arrow.uturn.backward", ayuda: "Deshacer la tinta") { c.deshacer() }
            BotonRielIPad(simbolo: "arrow.uturn.forward", ayuda: "Rehacer la tinta") { c.rehacer() }
            Divider().frame(width: 28)
            BotonRielIPad(simbolo: "hand.point.up.left", ayuda: "Dibujar con el dedo", encendido: modelo.conDedo) {
                c.alternarDedo()
            }
        }
        .modifier(FondoRielHoja())
    }

    private var marcadores: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(spacing: 2) {
                ForEach(modelo.marcas, id: \.id) { m in
                    Button { c.ir(a: m) } label: {
                        VStack(spacing: 0) {
                            Text(m.emoji).font(.system(size: 18))
                            Text(c.rotulo(de: m))
                                .font(.system(size: 9).monospacedDigit())
                                .foregroundStyle(.secondary)
                        }
                        .frame(width: 36, height: 38)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .contextMenu {
                        Button("Ir") { c.ir(a: m) }
                        Button("Quitar este marcador", role: .destructive) { c.quitarMarca(m) }
                        Button("Quitar todos", role: .destructive) { c.quitarMarcas() }
                    }
                }
            }
            .padding(3)
        }
        .frame(maxHeight: 260)
        .modifier(FondoRielHoja())
    }
}

private struct FondoRielHoja: ViewModifier {
    func body(content: Content) -> some View {
        content
            .padding(3)
            .background(RoundedRectangle(cornerRadius: 9).fill(.regularMaterial))
            .shadow(color: .black.opacity(0.16), radius: 6, y: 2)
    }
}

/// La tira de emoticonos de un marcador nuevo (con el marcador del riel
/// pulsado).
struct TiraDeMarcaDeHoja: View {
    let tinta: ControladorTintaHoja
    @ObservedObject var modelo: ModeloLectorIPad

    var body: some View {
        if modelo.poniendoMarca {
            TiraEmojisMarca { e in tinta.ponerMarca(e) } cerrar: { modelo.poniendoMarca = false }
        }
    }
}
