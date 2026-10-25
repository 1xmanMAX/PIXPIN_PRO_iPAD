// **La lectura seguida de Word y libros, con su tinta** (EPUB y Word): es
// `visor.rs` de Windows (el `VisorHtmlActivity` del movil) y el
// `VisorReflujo.swift` del Mac, con UIKit y el Apple Pencil. Una columna de
// texto que se adapta, con la letra que uno quiere, y se anota encima sin
// que nada tiemble.
//
// - **La maqueta es la del movil, Windows y el Mac** (`DocumentoReflujo` del
//   nucleo): las mismas letras (Noto Serif y Roboto, incrustadas en el
//   nucleo y abiertas aqui con Core Text) y los mismos cortes de renglon.
//   Core Text solo dice lo que avanza cada letra (`MedidorCoreTextReflujo`);
//   los renglones los parte Rust y aqui se pintan por donde se partieron.
//   Con la columna fijada (`anot-<uid>.maqueta`) los aparatos parten igual y
//   la tinta cae en la misma palabra.
// - **La tinta** es un `EditorLienzo` de Rust (`Nucleo.anotarReflujo`) en las
//   unidades del lector (el cero en el borde de la columna, la Y hacia abajo)
//   y se guarda donde la guardan Windows y Android (`anot-<uid>`), asi
//   viaja. La primera vez que se anota se fijan la columna y la letra.
// - Lo de los lectores: buscar, el indice, marcadores, escuchar (`LeerEnVoz`,
//   con el verde), la pagina que sube sola, el modo noche comun, compartir
//   e imprimir con lo anotado.
//
// Rendimiento: abrir y colocar van fuera del hilo principal; colocar se
// repite solo si cambian la columna o la letra. Se pinta solo lo que se ve
// (una capa del tamano de la pantalla que se repinta al desplazar): los
// bloques estan en orden de altura y se busca el primero que asoma. Los
// renglones de cada bloque se guardan ya hechos (`CTLine`) hasta que cambie
// la maqueta o el tono, y las imagenes se descodifican al verse.

import Combine
import CoreText
import ImageIO
import PixPinCore
import SwiftUI
import UIKit

// MARK: - Las letras del movil

/// **Las letras del lector** con Core Text: las del movil incrustadas en el
/// nucleo (`letrasDeLectura`), abiertas en memoria una vez, y Courier New
/// del sistema. Mide y pinta el mismo sitio. Con cerrojo: se usa desde el
/// hilo que coloca y desde el que pinta.
final class LetrasDeLecturaIPad: @unchecked Sendable {
    static let compartidas = LetrasDeLecturaIPad()

    private struct Clave: Hashable {
        let familia: String
        let negrita: Bool
        let cursiva: Bool
    }

    private struct ConTam: Hashable {
        let clave: Clave
        let tam: CGFloat
    }

    private let cerrojo = NSLock()
    private var descriptores: [Clave: CTFontDescriptor]?
    private var letras: [ConTam: CTFont] = [:]

    /// Un peso de CSS pinta con la negrita de 600 para arriba.
    static func esNegrita(_ peso: UInt16) -> Bool { peso >= 600 }

    private func cargarSinCerrojo() -> [Clave: CTFontDescriptor] {
        if let d = descriptores { return d }
        var d: [Clave: CTFontDescriptor] = [:]
        for l in letrasDeLectura() {
            guard let lista = CTFontManagerCreateFontDescriptorsFromData(l.datos as CFData) as? [CTFontDescriptor],
                  let primero = lista.first else { continue }
            d[Clave(familia: l.familia, negrita: l.negrita, cursiva: l.cursiva)] = primero
        }
        descriptores = d
        return d
    }

    func letra(familia: String, negrita: Bool, cursiva: Bool, tam: CGFloat) -> CTFont {
        let k = Clave(familia: familia, negrita: negrita, cursiva: cursiva)
        cerrojo.lock()
        defer { cerrojo.unlock() }
        if let f = letras[ConTam(clave: k, tam: tam)] { return f }
        let d = cargarSinCerrojo()
        let f: CTFont
        if let desc = d[k] ?? d[Clave(familia: familia, negrita: negrita, cursiva: false)]
            ?? d[Clave(familia: familia, negrita: false, cursiva: false)] {
            f = CTFontCreateWithFontDescriptor(desc, tam, nil)
        } else {
            var base = CTFontCreateWithName((familia == "Courier New" ? "CourierNewPSMT" : familia) as CFString, tam, nil)
            var rasgos: CTFontSymbolicTraits = []
            if negrita { rasgos.insert(.traitBold) }
            if cursiva { rasgos.insert(.traitItalic) }
            if !rasgos.isEmpty, let con = CTFontCreateCopyWithSymbolicTraits(base, tam, nil, rasgos, rasgos) {
                base = con
            }
            f = base
        }
        letras[ConTam(clave: k, tam: tam)] = f
        return f
    }

    /// **El parrafo con su letra y sus tramos**, listo para Core Text.
    func atribuido(_ texto: String, familia: String, peso: UInt16, tam: CGFloat, tramos: [TramoReflujo],
                   color: CGColor? = nil) -> NSAttributedString {
        let s = NSMutableAttributedString(string: texto)
        let largo = (texto as NSString).length
        let negrita = LetrasDeLecturaIPad.esNegrita(peso)
        var base: [NSAttributedString.Key: Any] = [
            NSAttributedString.Key(kCTFontAttributeName as String): letra(familia: familia, negrita: negrita,
                                                                           cursiva: false, tam: tam),
        ]
        if let color { base[NSAttributedString.Key(kCTForegroundColorAttributeName as String)] = color }
        s.setAttributes(base, range: NSRange(location: 0, length: largo))
        for t in tramos {
            let r = NSRange(location: Int(t.inicio), length: Int(t.longitud))
            guard r.location < largo else { continue }
            let rr = NSRange(location: r.location, length: min(r.length, largo - r.location))
            let f = letra(familia: t.mono ? "Courier New" : familia, negrita: t.negrita || negrita,
                          cursiva: t.cursiva, tam: tam)
            s.addAttribute(NSAttributedString.Key(kCTFontAttributeName as String), value: f, range: rr)
        }
        return s
    }

    /// Lo que avanza cada unidad UTF-16 (el racimo entero en su primera).
    func avances(_ texto: String, familia: String, peso: UInt16, tam: CGFloat, tramos: [TramoReflujo]) -> [Float] {
        let largo = (texto as NSString).length
        var v = [Float](repeating: 0, count: largo)
        guard largo > 0 else { return v }
        let linea = CTLineCreateWithAttributedString(atribuido(texto, familia: familia, peso: peso, tam: tam,
                                                               tramos: tramos))
        guard let corridas = CTLineGetGlyphRuns(linea) as? [CTRun] else { return v }
        for run in corridas {
            let n = CTRunGetGlyphCount(run)
            guard n > 0 else { continue }
            var indices = [CFIndex](repeating: 0, count: n)
            var avances = [CGSize](repeating: .zero, count: n)
            CTRunGetStringIndices(run, CFRange(location: 0, length: n), &indices)
            CTRunGetAdvances(run, CFRange(location: 0, length: n), &avances)
            for k in 0 ..< n where indices[k] >= 0 && indices[k] < largo {
                v[indices[k]] += Float(avances[k].width)
            }
        }
        return v
    }
}

/// **El medidor del nucleo**: Core Text con las letras del movil.
final class MedidorCoreTextReflujo: MedidorReflujo, @unchecked Sendable {
    func avances(texto: String, familia: String, peso: UInt16, tam: Float, tramos: [TramoReflujo]) -> [Float] {
        LetrasDeLecturaIPad.compartidas.avances(texto, familia: familia, peso: peso, tam: CGFloat(tam), tramos: tramos)
    }
}

// MARK: - El tono de la pagina

/// Los colores de la pagina: en claro, papel blanco; en oscuro, el papel
/// oscuro de las hojas con los colores del modo noche de Windows.
struct ColoresReflujo: Equatable {
    var papel: CGColor
    var texto: CGColor
    var apagado: CGColor
    var raya: CGColor
    var celda: CGColor
    var cristal: CGColor
    var oscuro: Bool

    static func de(_ t: TonoHojas) -> ColoresReflujo {
        let rgb = { (r: Int, g: Int, b: Int) in
            CGColor(srgbRed: CGFloat(r) / 255, green: CGFloat(g) / 255, blue: CGFloat(b) / 255, alpha: 1)
        }
        if t.oscuro != t.negativo {
            return ColoresReflujo(papel: TonoHojas.papelOscuro.cg,
                                  texto: rgb(0xE4, 0xE2, 0xE6), apagado: rgb(0x9A, 0x9A, 0xA2),
                                  raya: rgb(0x3A, 0x3A, 0x42), celda: rgb(0x4A, 0x4A, 0x52),
                                  cristal: rgb(0x26, 0x26, 0x2C), oscuro: true)
        }
        return ColoresReflujo(papel: rgb(0xFF, 0xFF, 0xFF), texto: rgb(0x1E, 0x1E, 0x22),
                              apagado: rgb(0x6A, 0x6A, 0x72), raya: rgb(0xCF, 0xCF, 0xD6),
                              celda: rgb(0xA8, 0xA8, 0xB0), cristal: rgb(0xF1, 0xF1, 0xF4), oscuro: false)
    }

    func de(_ t: TinteReflujo) -> CGColor {
        switch t {
        case .texto: texto
        case .apagado: apagado
        case .raya: raya
        }
    }
}

// MARK: - Lo que ve la barra

@MainActor
final class ModeloReflujoIPad: ObservableObject {
    @Published var nombre = ""
    /// `leer`, `lapiz`, `resaltador` o `borrador`.
    @Published var herramienta = "leer"
    @Published var color = ColorTinta.todos[0]
    @Published var conDedo = false
    @Published var tamano: UInt32 = 100
    @Published var tipo: UInt8 = 0
    @Published var grosor: UInt8 = 1
    /// La columna fijada (hay tinta): la letra no se puede cambiar.
    @Published var fijada = false
    @Published var indice: [EntradaIndiceReflujo] = []
    @Published var colocando = true
    @Published var marcas: [MarcadorReflujo] = []
    @Published var poniendoMarca = false
    @Published var viendoIndice = false
    @Published var aviso: String?
    @Published var nota: String?
    /// Buscar: la caja puesta, lo escrito, cuantas y la actual.
    @Published var buscando = false
    @Published var consulta = ""
    @Published var encontradas = 0
    @Published var actual: Int?

    var anotando: Bool { herramienta != "leer" }

    static let tipos = ["Serif", "Sans", "Ancho fijo", "A mano"]
    static let grosores = ["Fina", "Normal", "Gruesa", "Negra"]
}

// MARK: - La pagina

/// **Lo que se ve de la pagina**: una capa del tamano de la vista que se
/// desplaza, encima de ella, que pinta lo que cae dentro (las letras y la
/// tinta en unidades del lector). No recibe toques.
final class PaginaReflujoIPad: UIView {
    weak var visor: ControladorReflujoIPad?

    override init(frame: CGRect) {
        super.init(frame: frame)
        isUserInteractionEnabled = false
        isOpaque = true
        contentMode = .redraw
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    override func draw(_ sucio: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        visor?.pintarVista(en: ctx, sucio: sucio)
    }
}

/// La vista que se desplaza: coloca la capa de pintar en lo que se ve cada
/// vez que se mueve.
final class DesplazarReflujoIPad: UIScrollView {
    var alColocar: (() -> Void)?

    override func layoutSubviews() {
        super.layoutSubviews()
        alColocar?()
    }
}

/// Recoge el Apple Pencil (y el dedo o el raton si toca) para la tinta.
final class GestoTintaReflujoIPad: UIGestureRecognizer {
    var aceptar: ((UITouch) -> Bool)?
    var trazo: ((FasePuntero, [UITouch]) -> Void)?
    private var activo: UITouch?

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        guard activo == nil, let t = touches.first, aceptar?(t) == true else {
            for t in touches where t !== activo { ignore(t, for: event) }
            return
        }
        activo = t
        state = .began
        trazo?(.pulsar, [t])
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let a = activo, touches.contains(a) else { return }
        trazo?(.mover, event.coalescedTouches(for: a) ?? [a])
        state = .changed
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let a = activo, touches.contains(a) else { return }
        trazo?(.soltar, [a])
        activo = nil
        state = .ended
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let a = activo, touches.contains(a) else { return }
        trazo?(.cancelar, [a])
        activo = nil
        state = .cancelled
    }

    override func reset() {
        super.reset()
        activo = nil
    }
}

// MARK: - El visor

/// **Un Word o un libro abierto en la lectura seguida.**
@MainActor
final class ControladorReflujoIPad: NSObject, ObservableObject {
    let url: URL
    let nucleo: Nucleo
    let modelo = ModeloReflujoIPad()
    let avance = AvanceLectorIPad()
    let desplazar = DesplazarReflujoIPad()
    private let contenido = UIView()
    let pagina = PaginaReflujoIPad()
    private let gesto = GestoTintaReflujoIPad(target: nil, action: nil)
    private lazy var reloj = RelojAvanceIPad(avance: avance) { [weak self] in self?.desplazar }

    private(set) var documento: DocumentoReflujo?
    private(set) var maqueta: MaquetaReflujo?
    private(set) var lectura: LecturaReflujo?
    private var colocadoCon: (Float, UInt32, UInt8, UInt8)?
    private var colocandoAhora: (Float, UInt32, UInt8, UInt8)?
    private var lineas: [Int: [(CTLine, CGFloat)]] = [:]
    private var imagenes: [UInt32: CGImage] = [:]
    private var descodificando: Set<UInt32> = []
    private var colores = ColoresReflujo.de(TonoHojas())
    /// De donde a donde va la pagina a lo ancho (unidades).
    private var izq: CGFloat = 0
    private var anchoPagina: CGFloat = 1
    private var altoPagina: CGFloat = 1

    private var editor: EditorLienzo?
    private var leido: (version: UInt64, lienzo: LienzoLeido)?
    private var adaptada: (version: UInt64, lienzo: LienzoLeido)?
    private var coloresAdaptados: [ColorLienzo: ColorLienzo] = [:]
    private var trazando = false
    private var ultimaHerramienta = "lapiz"
    private let alDia = AlDiaLector()
    private var mirarDisco: Timer?

    private var coincidencias: [CoincidenciaReflujo] = []
    /// Escuchar: el colocado que suena y la palabra.
    private(set) var sonando: Int? { didSet { pagina.setNeedsDisplay() } }
    private var palabra: NSRange? { didSet { pagina.setNeedsDisplay() } }
    private var enganchada = false
    private var engancharAlColocar = true

    private let colaGuardar = DispatchQueue(label: "pixpin.reflujo.guardar", qos: .utility)
    private var guardadoPendiente: DispatchWorkItem?
    private var lecturaPendiente: DispatchWorkItem?
    private var recolocar: DispatchWorkItem?
    private var tareaNota: DispatchWorkItem?
    private var vigias: [NSObjectProtocol] = []
    private var anchoVisto: CGFloat = 0
    private var abierto = false
    private var cerrado = false

    init(url: URL, nucleo: Nucleo, titulo: String) {
        self.url = url
        self.nucleo = nucleo
        super.init()
        modelo.nombre = titulo
        montar()
        DocumentosDelIPad.activar()
    }

    private func montar() {
        desplazar.delegate = self
        desplazar.minimumZoomScale = 0.3
        desplazar.maximumZoomScale = CGFloat(aumentoMaximoDeLectura())
        desplazar.alwaysBounceVertical = true
        desplazar.contentInsetAdjustmentBehavior = .never
        desplazar.addSubview(contenido)
        pagina.visor = self
        desplazar.addSubview(pagina)
        desplazar.alColocar = { [weak self] in self?.alColocarLaVista() }
        gesto.cancelsTouchesInView = true
        gesto.delaysTouchesBegan = false
        gesto.aceptar = { [weak self] t in self?.aceptar(t) ?? false }
        gesto.trazo = { [weak self] fase, toques in self?.trazo(fase, toques) }
        desplazar.addGestureRecognizer(gesto)
        let lapiz = UIPencilInteraction()
        lapiz.delegate = self
        desplazar.addInteraction(lapiz)
        reloj.quieta = { [weak self] in self?.modelo.anotando ?? true }
        actualizarGestos()
        let centro = NotificationCenter.default
        vigias.append(centro.addObserver(forName: .pixpinDocumentoAnotado, object: nil, queue: .main) { [weak self] n in
            let ruta = n.object as? String
            MainActor.assumeIsolated {
                if let self, ruta == self.url.path { self.mirarElDisco() }
            }
        })
        vigias.append(centro.addObserver(forName: UIApplication.willResignActiveNotification, object: nil,
                                         queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, !self.cerrado else { return }
                self.guardarLectura()
                if self.guardadoPendiente != nil { self.programarGuardado(espera: 0) }
            }
        })
    }

    // MARK: Abrir y colocar

    func abrir() {
        guard !abierto else { return }
        abierto = true
        aplicarTono()
        let ruta = url.path, nucleo = self.nucleo
        Task {
            let (doc, lectura, nombre) = await Task.detached(priority: .userInitiated) {
                (try? DocumentoReflujo.abrir(ruta: ruta), nucleo.lecturaDeReflujo(documento: ruta),
                 nucleo.nombreDeDocumento(documento: ruta))
            }.value
            guard !self.cerrado else { return }
            if !nombre.isEmpty { self.modelo.nombre = nombre }
            guard let doc else {
                self.modelo.colocando = false
                self.modelo.aviso = "No se puede abrir este documento."
                return
            }
            self.documento = doc
            self.modelo.indice = doc.indice()
            self.avance.palabras = Int(doc.palabras())
            self.ponerLectura(lectura)
            if lectura.zoom.isFinite, lectura.zoom > 0 { self.desplazar.zoomScale = CGFloat(lectura.zoom) }
            self.colocarSiHaceFalta(irA: lectura.sitio)
            if lectura.columna > 0 { self.abrirTinta(siHay: true) }
        }
    }

    private func ponerLectura(_ l: LecturaReflujo) {
        lectura = l
        modelo.tamano = l.tamano
        modelo.tipo = l.tipo
        modelo.grosor = l.grosor
        modelo.fijada = l.columna > 0
        modelo.marcas = l.marcadores
    }

    /// La columna que toca: la fijada si hay tinta; si no, la que cabe.
    var columna: Float {
        if let l = lectura, l.columna > 0 { return Float(l.columna) }
        return columnaDeLecturaQueCabe(ancho: Float(max(desplazar.bounds.width, 320)))
    }

    private func colocarSiHaceFalta(irA: Float? = nil) {
        guard let doc = documento, let l = lectura else { return }
        let clave = (columna, l.tamano, l.tipo, l.grosor)
        if let c = colocadoCon, c == clave, irA == nil { return }
        if let c = colocandoAhora, c == clave { return }
        colocandoAhora = clave
        modelo.colocando = true
        let donde = irA ?? fraccion()
        Task {
            let m = await Task.detached(priority: .userInitiated) {
                doc.colocar(columna: clave.0, tamano: clave.1, tipo: clave.2, grosor: clave.3,
                            medidor: MedidorCoreTextReflujo())
            }.value
            guard !self.cerrado, let c = self.colocandoAhora, c == clave else { return }
            self.colocandoAhora = nil
            self.colocadoCon = clave
            self.maqueta = m
            self.lineas = [:]
            self.coincidencias = []
            self.modelo.colocando = false
            self.ponerLimites()
            self.irA(fraccion: donde)
            if self.modelo.buscando, !self.modelo.consulta.isEmpty { self.buscar(self.modelo.consulta) }
            self.alColocarLaVoz()
        }
    }

    /// **De donde a donde va el documento a lo ancho** (`visor::limites`):
    /// sin tinta, la columna; con tinta o anotando, sus dos espacios.
    private func limites() -> (CGFloat, CGFloat) {
        guard let m = maqueta, let l = lectura else { return (0, CGFloat(columna)) }
        let c = CGFloat(m.columna)
        let tablas = m.colocados.compactMap { $0.caja.map { CGFloat($0.x + $0.ancho) } }.max() ?? c
        var (a, b) = (CGFloat(0), c)
        if l.columna > 0 || modelo.anotando {
            let lados = ladosDeLaColumna(lectura: l, columna: m.columna)
            a = -CGFloat(lados.first ?? 0)
            b = c + CGFloat(lados.last ?? 0)
        }
        return (a, max(b, tablas))
    }

    /// La pagina a la medida de lo colocado y el aumento dentro de sus topes.
    private func ponerLimites() {
        guard let m = maqueta else { return }
        let (a, b) = limites()
        let renglon = CGFloat(16 * Float(lectura?.tamano ?? 100) / 100 * 1.45)
        let antes = fraccion()
        izq = a
        anchoPagina = max(b - a, 1)
        altoPagina = CGFloat(m.alto) + renglon * 4
        let z = desplazar.zoomScale
        contenido.bounds = CGRect(x: 0, y: 0, width: anchoPagina, height: altoPagina)
        contenido.transform = CGAffineTransform(scaleX: z, y: z)
        contenido.center = CGPoint(x: anchoPagina * z / 2, y: altoPagina * z / 2)
        desplazar.contentSize = CGSize(width: anchoPagina * z, height: altoPagina * z)
        let minimo = CGFloat(aumentoMinimoDeLectura(anchoDoc: Float(anchoPagina), vista: Float(desplazar.bounds.width)))
        desplazar.minimumZoomScale = max(min(minimo, 1), 0.1)
        centrar()
        irA(fraccion: antes)
        pagina.setNeedsDisplay()
    }

    /// Con la pagina mas estrecha que la vista, en medio.
    private func centrar() {
        let ancho = desplazar.contentSize.width
        let lado = max((desplazar.bounds.width - ancho) / 2, 0)
        let arriba: CGFloat = 70
        let abajo: CGFloat = 90
        let nuevo = UIEdgeInsets(top: arriba, left: lado, bottom: abajo, right: lado)
        if desplazar.contentInset != nuevo { desplazar.contentInset = nuevo }
    }

    /// La vista se movio o cambio de tamano: la capa de pintar en lo que se
    /// ve y, sin tinta, la columna sigue al ancho.
    private func alColocarLaVista() {
        let visto = desplazar.bounds
        if pagina.frame != visto {
            pagina.frame = visto
        }
        pagina.setNeedsDisplay()
        if abs(visto.width - anchoVisto) > 1 {
            anchoVisto = visto.width
            centrar()
            guard lectura?.columna == 0 else { return }
            recolocar?.cancel()
            let t = DispatchWorkItem { [weak self] in MainActor.assumeIsolated { self?.colocarSiHaceFalta() } }
            recolocar = t
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.25, execute: t)
        }
    }

    // MARK: Donde se esta

    private var altoDoc: CGFloat { CGFloat(maqueta?.alto ?? 0) }

    /// Lo de arriba de lo que se ve, en unidades.
    private var arriba: CGFloat {
        (desplazar.contentOffset.y + desplazar.contentInset.top) / max(desplazar.zoomScale, 0.0001)
    }

    private var altoVisto: CGFloat {
        (desplazar.bounds.height - desplazar.contentInset.top) / max(desplazar.zoomScale, 0.0001)
    }

    func fraccion() -> Float {
        guard altoDoc > 0 else { return lectura?.sitio ?? 0 }
        return Float(min(max(arriba / altoDoc, 0), 1))
    }

    func irA(y: CGFloat) {
        let z = desplazar.zoomScale
        let tope = max(desplazar.contentSize.height + desplazar.contentInset.bottom - desplazar.bounds.height,
                       -desplazar.contentInset.top)
        let o = min(max(y * z - desplazar.contentInset.top, -desplazar.contentInset.top), tope)
        desplazar.contentOffset = CGPoint(x: desplazar.contentOffset.x, y: o)
    }

    func irA(fraccion f: Float) { irA(y: CGFloat(f) * altoDoc) }

    func ir(a m: MarcadorReflujo) { irA(fraccion: m.fraccion) }

    func ir(aBloque b: UInt32) {
        guard let m = maqueta, let c = m.colocados.first(where: { ($0.bloque ?? 0) >= b && $0.bloque != nil }) else {
            return
        }
        irA(y: CGFloat(c.y) - 12)
    }

    /// El indice para la hoja del indice (el destino es el bloque).
    var entradasDelIndice: [EntradaIndice] {
        modelo.indice.enumerated().map { i, e in
            EntradaIndice(id: i, titulo: e.titulo, nivel: max(Int(e.nivel) - 1, 0), destino: Int(e.bloque))
        }
    }

    /// El bloque que se ve arriba (para marcar el capitulo en el indice).
    var bloqueDeArriba: Int {
        guard let m = maqueta else { return 0 }
        let y = arriba
        return Int(m.colocados.last(where: { CGFloat($0.y) <= y + 4 && $0.bloque != nil })?.bloque ?? 0)
    }

    // MARK: Pintar

    /// Pinta lo que se ve en la capa (sus coordenadas: las de la pantalla).
    func pintarVista(en ctx: CGContext, sucio: CGRect) {
        ctx.setFillColor(colores.papel)
        ctx.fill(sucio)
        // De la capa al contenido: la capa esta donde se ve (su origen es el
        // desplazamiento) y el contenido empieza en el borde izquierdo.
        let z = max(desplazar.zoomScale, 0.0001)
        let o = pagina.frame.origin
        ctx.saveGState()
        ctx.translateBy(x: -o.x, y: -o.y)
        ctx.scaleBy(x: z, y: z)
        ctx.translateBy(x: -izq, y: 0)
        let enUnidades = CGRect(x: (sucio.minX + o.x) / z + izq, y: (sucio.minY + o.y) / z,
                                width: sucio.width / z, height: sucio.height / z)
        pintar(en: ctx, sucio: enUnidades, linea: 1 / z, conTinta: true, marcar: true)
        ctx.restoreGState()
    }

    /// Pinta, en unidades del lector, lo que cae en `sucio`: el papel, cada
    /// bloque, lo encontrado, lo que suena y la tinta.
    private func pintar(en ctx: CGContext, sucio: CGRect, linea: CGFloat, conTinta: Bool, marcar: Bool) {
        guard let m = maqueta else { return }
        let cs = m.colocados
        var i = 0, hasta = cs.count
        while i < hasta {
            let mitad = (i + hasta) / 2
            let c = cs[mitad]
            let fondo = max(CGFloat(c.y + c.alto), c.caja.map { CGFloat($0.y + $0.alto) } ?? 0)
            if fondo < sucio.minY - 400 { i = mitad + 1 } else { hasta = mitad }
        }
        if marcar, let s = sonando, s < cs.count {
            let c = cs[s]
            let r = CGRect(x: CGFloat(c.sangria) - 4, y: CGFloat(c.y) - 2, width: CGFloat(c.ancho) + 8,
                           height: CGFloat(c.alto) + 4)
            ctx.setFillColor(UIColor(red: 1, green: 0.78, blue: 0.2, alpha: 0.22).cgColor)
            ctx.fill(r)
            if let p = palabra {
                ctx.setFillColor(UIColor(red: 1, green: 0.7, blue: 0.1, alpha: 0.45).cgColor)
                for q in cajas(de: s, p) { ctx.fill(q) }
            }
        }
        if marcar, modelo.buscando, !coincidencias.isEmpty {
            for (k, e) in coincidencias.enumerated() {
                let c = Int(e.colocado)
                guard c < cs.count, CGFloat(cs[c].y) < sucio.maxY + 40,
                      CGFloat(cs[c].y + cs[c].alto) > sucio.minY - 40 else { continue }
                ctx.setFillColor(k == modelo.actual
                    ? UIColor(red: 1, green: 0.6, blue: 0, alpha: 0.55).cgColor
                    : UIColor(red: 1, green: 0.9, blue: 0.2, alpha: 0.45).cgColor)
                for q in cajas(de: c, NSRange(location: Int(e.inicio), length: Int(e.largo))) {
                    ctx.addPath(CGPath(roundedRect: q, cornerWidth: 2, cornerHeight: 2, transform: nil))
                    ctx.fillPath()
                }
            }
        }
        var k = i
        while k < cs.count {
            let c = cs[k]
            if CGFloat(c.y) > sucio.maxY + 40, c.caja.map({ CGFloat($0.y) > sucio.maxY }) ?? true { break }
            pintarColocado(k, c, en: ctx, linea: linea)
            k += 1
        }
        if conTinta { pintarTinta(en: ctx, sucio: sucio) }
    }

    private func pintarColocado(_ k: Int, _ c: ColocadoReflujo, en ctx: CGContext, linea: CGFloat) {
        if let caja = c.caja {
            let r = CGRect(x: CGFloat(caja.x), y: CGFloat(caja.y), width: CGFloat(caja.ancho), height: CGFloat(caja.alto))
            if caja.relleno {
                ctx.setFillColor(colores.cristal)
                ctx.fill(r)
            }
            ctx.setFillColor(colores.celda)
            ctx.fill(CGRect(x: r.minX, y: r.minY, width: linea, height: r.height))
            ctx.fill(CGRect(x: r.maxX - linea, y: r.minY, width: linea, height: r.height))
            if caja.arriba { ctx.fill(CGRect(x: r.minX, y: r.minY, width: r.width, height: linea)) }
            if caja.abajo { ctx.fill(CGRect(x: r.minX, y: r.maxY - linea, width: r.width, height: linea)) }
        }
        switch c.clase {
        case .regla, .capitulo:
            ctx.setFillColor(colores.raya)
            ctx.fill(CGRect(x: CGFloat(c.sangria), y: CGFloat(c.y), width: CGFloat(c.ancho), height: CGFloat(c.alto)))
            return
        default: break
        }
        let hueco = CGRect(x: CGFloat(c.sangria), y: CGFloat(c.y), width: CGFloat(c.ancho), height: CGFloat(c.alto))
        if c.recuadro {
            if let n = c.imagen {
                if let img = imagen(n) {
                    ctx.saveGState()
                    ctx.translateBy(x: hueco.minX, y: hueco.maxY)
                    ctx.scaleBy(x: 1, y: -1)
                    ctx.interpolationQuality = .high
                    ctx.draw(img, in: CGRect(origin: .zero, size: hueco.size))
                    ctx.restoreGState()
                } else {
                    ctx.setFillColor(colores.cristal)
                    ctx.fill(hueco)
                }
                return
            }
            ctx.setStrokeColor(colores.apagado)
            ctx.setLineWidth(linea)
            ctx.stroke(hueco.insetBy(dx: linea / 2, dy: linea / 2))
        }
        guard !c.texto.isEmpty else { return }
        let renglon = CGFloat(c.tam * c.interlineado)
        let f = LetrasDeLecturaIPad.compartidas.letra(familia: c.familia, negrita: LetrasDeLecturaIPad.esNegrita(c.peso),
                                                       cursiva: false, tam: CGFloat(c.tam))
        let asc = CTFontGetAscent(f), desc = CTFontGetDescent(f)
        for (n, (l, x)) in lineas(de: k, c).enumerated() {
            let y0 = CGFloat(c.y) + CGFloat(n) * renglon
            // El medio interlineado de CSS arriba y abajo de la letra.
            let base = y0 + (renglon - (asc + desc)) / 2 + asc
            ctx.saveGState()
            ctx.textMatrix = .identity
            ctx.translateBy(x: x, y: base)
            ctx.scaleBy(x: 1, y: -1)
            ctx.textPosition = .zero
            CTLineDraw(l, ctx)
            ctx.restoreGState()
        }
    }

    /// Los renglones de un colocado, partidos donde los partio Rust.
    private func lineas(de k: Int, _ c: ColocadoReflujo) -> [(CTLine, CGFloat)] {
        if let l = lineas[k] { return l }
        let todo = LetrasDeLecturaIPad.compartidas.atribuido(c.texto, familia: c.familia, peso: c.peso,
                                                              tam: CGFloat(c.tam), tramos: c.tramos,
                                                              color: colores.de(c.tinte))
        let largo = todo.length
        var pares: [(Int, Int)] = stride(from: 0, to: c.renglones.count - 1, by: 2).map {
            (Int(c.renglones[$0]), Int(c.renglones[$0 + 1]))
        }
        if pares.isEmpty { pares = [(0, largo)] }
        let salida: [(CTLine, CGFloat)] = pares.map { a, b in
            let a = min(a, largo), b = min(max(b, a), largo)
            let l = CTLineCreateWithAttributedString(todo.attributedSubstring(from: NSRange(location: a, length: b - a)))
            let w = CGFloat(CTLineGetTypographicBounds(l, nil, nil, nil))
            let x: CGFloat
            switch c.alineacion {
            case .izquierda: x = CGFloat(c.sangria)
            case .centro: x = CGFloat(c.sangria) + (CGFloat(c.ancho) - w) / 2
            case .derecha: x = CGFloat(c.sangria) + CGFloat(c.ancho) - w
            }
            return (l, x)
        }
        lineas[k] = salida
        return salida
    }

    /// Las cajas de unas letras de un colocado, en unidades del documento.
    private func cajas(de k: Int, _ r: NSRange) -> [CGRect] {
        guard let m = maqueta, k < m.colocados.count else { return [] }
        let c = m.colocados[k]
        let renglon = CGFloat(c.tam * c.interlineado)
        var pares: [(Int, Int)] = stride(from: 0, to: c.renglones.count - 1, by: 2).map {
            (Int(c.renglones[$0]), Int(c.renglones[$0 + 1]))
        }
        if pares.isEmpty { pares = [(0, (c.texto as NSString).length)] }
        var v: [CGRect] = []
        for (n, ((a, b), (l, x))) in zip(pares, lineas(de: k, c)).enumerated() {
            let i = max(r.location, a), f = min(NSMaxRange(r), b)
            guard f > i else { continue }
            let x0 = CTLineGetOffsetForStringIndex(l, i - a, nil)
            let x1 = CTLineGetOffsetForStringIndex(l, f - a, nil)
            v.append(CGRect(x: x + x0, y: CGFloat(c.y) + CGFloat(n) * renglon, width: max(x1 - x0, 2), height: renglon))
        }
        return v
    }

    private func imagen(_ n: UInt32) -> CGImage? {
        if let i = imagenes[n] { return i }
        guard !descodificando.contains(n), let doc = documento else { return nil }
        descodificando.insert(n)
        Task {
            let img = await Task.detached(priority: .utility) { () -> CGImage? in
                guard let datos = doc.imagen(n: n),
                      let fuente = CGImageSourceCreateWithData(datos as CFData, nil) else { return nil }
                return CGImageSourceCreateImageAtIndex(fuente, 0, nil)
            }.value
            if let img { self.imagenes[n] = img }
            self.pagina.setNeedsDisplay()
        }
        return nil
    }

    private func pintarTinta(en ctx: CGContext, sucio: CGRect) {
        guard let ed = editor else { return }
        let v = ed.version()
        if leido?.version != v, let l = try? leerLienzo(ed.plano()) { leido = (v, l) }
        guard var l = leido?.lienzo, !l.figuras.isEmpty else { return }
        if colores.oscuro {
            if let a = adaptada, a.version == v {
                l = a.lienzo
            } else {
                l = l.sobrePapel(TonoHojas.papelOscuro, cache: &coloresAdaptados)
                adaptada = (v, l)
            }
        }
        PintorTintaHoja(lienzo: l).pintar(en: ctx, limites: sucio)
    }

    // MARK: Tono

    func aplicarTono() {
        colores = ColoresReflujo.de(TonoHojas(oscuro: ModoNoche.compartido.activo))
        desplazar.backgroundColor = UIColor(cgColor: colores.papel)
        lineas = [:]
        adaptada = nil
        coloresAdaptados = [:]
        pagina.setNeedsDisplay()
    }

    // MARK: La letra

    /// Cambia algo de la letra, **si se puede**: con tinta encima la letra
    /// esta fijada (lo anotado quedaria sobre otras palabras).
    private func cambiarLetra(_ cambio: (inout LecturaReflujo) -> Void) {
        guard var l = lectura else { return }
        if l.columna > 0 {
            avisar("La letra está fijada: hay tinta encima y el texto no puede moverse de su sitio")
            return
        }
        cambio(&l)
        ponerLectura(l)
        guardarLectura()
        colocarSiHaceFalta()
    }

    func cambiarTamano(_ mas: Bool) {
        cambiarLetra { $0.tamano = tamanoDeLetraVecino(actual: $0.tamano, mas: mas) }
    }

    func ponerTipo(_ t: UInt8) { cambiarLetra { $0.tipo = t } }
    func ponerGrosor(_ g: UInt8) { cambiarLetra { $0.grosor = g } }

    /// **Un paso mas o menos de espacio a un lado** para anotar.
    func espacio(izquierda: Bool, mas: Bool) {
        guard let l = lectura else { return }
        guard let nueva = otroEspacioDeLado(lectura: l, columnaMedida: columna, izquierda: izquierda, mas: mas) else {
            avisar(mas ? "No cabe más espacio a ese lado" : "Ya no hay espacio a ese lado")
            return
        }
        let fijaAhora = l.columna == 0
        ponerLectura(nueva)
        if fijaAhora { guardarLecturaYa() } else { guardarLectura() }
        ponerLimites()
    }

    // MARK: Anotar

    private func abrirTinta(siHay: Bool) {
        guard editor == nil else { return }
        let ruta = url.path, nucleo = self.nucleo
        Task {
            let ed = await Task.detached(priority: .userInitiated) { () -> EditorLienzo? in
                if siHay, !nucleo.hayTintaDeReflujo(documento: ruta) { return nil }
                return try? nucleo.anotarReflujo(documento: ruta)
            }.value
            guard let ed, self.editor == nil, !self.cerrado else { return }
            self.ponerEditor(ed)
        }
    }

    private func ponerEditor(_ ed: EditorLienzo) {
        editor = ed
        leido = nil
        adaptada = nil
        aplicarHerramienta(ed)
        empezarAMirarElDisco()
        pagina.setNeedsDisplay()
    }

    private func aplicarHerramienta(_ ed: EditorLienzo) {
        try? ed.herramienta(nombre: modelo.anotando ? modelo.herramienta : "lapiz")
        let c = modelo.color
        ed.tintaColor(r: c.r, g: c.g, b: c.b, a: 1)
    }

    func alternarAnotar() {
        elegir(modelo.anotando ? "leer" : ultimaHerramienta)
    }

    /// **Elegir una herramienta**. La primera vez que se anota se fija la
    /// columna (y con ella la letra).
    func elegir(_ h: String) {
        if h != "leer", var l = lectura, l.columna == 0, maqueta != nil {
            l.columna = UInt32(columna.rounded())
            ponerLectura(l)
            guardarLecturaYa()
        }
        if h == "lapiz" || h == "resaltador" { ultimaHerramienta = h }
        modelo.herramienta = h
        if h == "resaltador", modelo.color == ColorTinta.todos[0] {
            modelo.color = ColorTinta.todos[4]
        } else if h == "lapiz", modelo.color == ColorTinta.todos[4] {
            modelo.color = ColorTinta.todos[0]
        }
        if h != "leer" {
            modelo.poniendoMarca = false
            if let ed = editor {
                aplicarHerramienta(ed)
            } else {
                do {
                    ponerEditor(try nucleo.anotarReflujo(documento: url.path))
                } catch {
                    modelo.aviso = "No se puede anotar aquí: \(error)"
                }
            }
        } else {
            programarGuardado()
        }
        ponerLimites()
        actualizarGestos()
    }

    func ponerColor(_ c: ColorTinta) {
        modelo.color = c
        if !modelo.anotando || modelo.herramienta == "borrador" {
            elegir(ultimaHerramienta)
        } else if let ed = editor {
            aplicarHerramienta(ed)
        }
    }

    func alternarDedo() {
        modelo.conDedo.toggle()
        actualizarGestos()
    }

    private func actualizarGestos() {
        var tipos: [UITouch.TouchType] = [.pencil]
        if modelo.anotando {
            tipos.append(.indirectPointer)
            if modelo.conDedo { tipos.append(.direct) }
        }
        gesto.allowedTouchTypes = tipos.map { NSNumber(value: $0.rawValue) }
        desplazar.panGestureRecognizer.minimumNumberOfTouches = modelo.anotando && modelo.conDedo ? 2 : 1
    }

    private func aceptar(_ t: UITouch) -> Bool {
        guard maqueta != nil else { return false }
        switch t.type {
        case .pencil:
            if !modelo.anotando { elegir(ultimaHerramienta) }
            return editor != nil
        case .direct:
            return modelo.anotando && modelo.conDedo && editor != nil
        default:
            return modelo.anotando && editor != nil
        }
    }

    private func trazo(_ fase: FasePuntero, _ toques: [UITouch]) {
        guard let ed = editor else { return }
        trazando = fase == .pulsar || fase == .mover
        for t in toques {
            let p = t.preciseLocation(in: contenido)
            let presion: Float? = t.type == .pencil && t.maximumPossibleForce > 0
                ? Float(min(max(t.force / t.maximumPossibleForce, 0), 1)) : nil
            _ = ed.puntero(fase: fase, x: Float(p.x + izq), y: Float(p.y), presion: presion, modificadores: 0,
                           zoom: Float(desplazar.zoomScale))
        }
        pagina.setNeedsDisplay()
        if fase == .soltar || fase == .cancelar {
            trazando = false
            programarGuardado()
        }
    }

    func deshacer() {
        if editor?.deshacer() == true {
            pagina.setNeedsDisplay()
            programarGuardado()
        }
    }

    func rehacer() {
        if editor?.rehacer() == true {
            pagina.setNeedsDisplay()
            programarGuardado()
        }
    }

    // MARK: Guardar

    private func programarGuardado(espera: TimeInterval = 1.0) {
        guardadoPendiente?.cancel()
        guard let ed = editor else { return }
        let ruta = url.path, alDia = self.alDia
        let tarea = DispatchWorkItem { [weak self] in
            let junto = ControladorReflujoIPad.guardarTinta(ed, ruta: ruta, alDia: alDia)
            guard junto else { return }
            DispatchQueue.main.async { MainActor.assumeIsolated { self?.releer() } }
        }
        guardadoPendiente = tarea
        colaGuardar.asyncAfter(deadline: .now() + espera, execute: tarea)
    }

    private nonisolated static func guardarTinta(_ ed: EditorLienzo, ruta: String, alDia: AlDiaLector) -> Bool {
        guard ed.sinGuardar() else { return false }
        do {
            let r = try alDia.guardar(0, ed)
            if r.escrito {
                DispatchQueue.main.async {
                    NotificationCenter.default.post(name: .pixpinDocumentoAnotado, object: ruta)
                    NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                }
            }
            return r.junto
        } catch {
            NSLog("PixPin: no se pudo guardar la tinta del visor: %@", "\(error)")
            return false
        }
    }

    private func lecturaDeAhora() -> LecturaReflujo? {
        guard var l = lectura else { return nil }
        if maqueta != nil { l.sitio = fraccion() }
        l.zoom = Float(desplazar.zoomScale)
        return l
    }

    func guardarLectura() {
        guard let l = lecturaDeAhora() else { return }
        lectura = l
        let ruta = url.path, nucleo = self.nucleo
        colaGuardar.async { try? nucleo.guardarLecturaDeReflujo(documento: ruta, lectura: l) }
    }

    /// Lo mismo, ya (antes de abrir la tinta, que necesita la columna).
    private func guardarLecturaYa() {
        guard let l = lecturaDeAhora() else { return }
        lectura = l
        let ruta = url.path, nucleo = self.nucleo
        colaGuardar.sync { _ = try? nucleo.guardarLecturaDeReflujo(documento: ruta, lectura: l) }
    }

    private func programarLectura() {
        lecturaPendiente?.cancel()
        let t = DispatchWorkItem { [weak self] in MainActor.assumeIsolated { self?.guardarLectura() } }
        lecturaPendiente = t
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5, execute: t)
    }

    func cerrar() {
        guard !cerrado else { return }
        cerrado = true
        reloj.parar()
        avance.activo = false
        LeerEnVoz.compartida.desenganchar(self)
        guardadoPendiente?.cancel()
        lecturaPendiente?.cancel()
        recolocar?.cancel()
        guardarLectura()
        if let ed = editor {
            let ruta = url.path, alDia = self.alDia
            colaGuardar.async { _ = ControladorReflujoIPad.guardarTinta(ed, ruta: ruta, alDia: alDia) }
        }
        for v in vigias { NotificationCenter.default.removeObserver(v) }
        vigias = []
        mirarDisco?.invalidate()
        mirarDisco = nil
    }

    // MARK: Al dia con el disco

    private func empezarAMirarElDisco() {
        guard mirarDisco == nil, !cerrado else { return }
        let r = Timer(timeInterval: 1.5, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.mirarElDisco() }
        }
        RunLoop.main.add(r, forMode: .common)
        mirarDisco = r
    }

    private func mirarElDisco() {
        guard let ed = editor else { return }
        let abiertas = [0: ed.ruta()]
        let alDia = self.alDia
        colaGuardar.async { [weak self] in
            guard !alDia.cambiadas(abiertas).isEmpty else { return }
            DispatchQueue.main.async { MainActor.assumeIsolated { self?.releer() } }
        }
    }

    private func releer() {
        guard let viejo = editor, !viejo.sinGuardar(), !trazando else { return }
        let ruta = url.path, nucleo = self.nucleo, rutaTinta = viejo.ruta(), cola = colaGuardar
        Task {
            let (visto, ed) = await withCheckedContinuation { cont in
                cola.async {
                    let v = AlDiaLector.ver(rutaTinta)
                    cont.resume(returning: (v, try? nucleo.anotarReflujo(documento: ruta)))
                }
            }
            guard let ed, self.editor === viejo, !viejo.sinGuardar(), !self.trazando else { return }
            self.ponerEditor(ed)
            let alDia = self.alDia
            cola.async { alDia.apuntar(0, visto) }
        }
    }

    // MARK: Marcadores

    func ponerMarca(emoji: String) {
        guard var l = lectura else { return }
        l.marcadores = conMarcadorDeLectura(marcadores: l.marcadores, fraccion: fraccion(), emoji: emoji,
                                            ahora: UInt64(Date().timeIntervalSince1970 * 1000))
        modelo.poniendoMarca = false
        ponerLectura(l)
        guardarLectura()
    }

    func quitarMarca(_ m: MarcadorReflujo) {
        guard var l = lectura else { return }
        l.marcadores.removeAll { $0.id == m.id }
        ponerLectura(l)
        guardarLectura()
    }

    func quitarMarcas() {
        guard var l = lectura else { return }
        l.marcadores = []
        ponerLectura(l)
        guardarLectura()
    }

    private func ponerVerde(_ v: VozReflujo?) {
        guard var l = lectura else { return }
        l.voz = v
        ponerLectura(l)
        guardarLectura()
    }

    // MARK: Buscar

    func buscar(_ q: String) {
        guard let doc = documento else { return }
        let consulta = q.trimmingCharacters(in: .whitespaces)
        coincidencias = consulta.isEmpty ? [] : doc.buscar(consulta: consulta)
        modelo.encontradas = coincidencias.count
        let cs = maqueta?.colocados ?? []
        let y = arriba
        modelo.actual = coincidencias.isEmpty ? nil : (coincidencias.firstIndex { e in
            Int(e.colocado) < cs.count && CGFloat(cs[Int(e.colocado)].y + cs[Int(e.colocado)].alto) >= y
        } ?? 0)
        mostrarBuscada()
    }

    func siguiente(_ paso: Int) {
        guard !coincidencias.isEmpty else { return }
        let n = coincidencias.count
        modelo.actual = ((modelo.actual ?? -paso) + paso + n) % n
        mostrarBuscada()
    }

    private func mostrarBuscada() {
        pagina.setNeedsDisplay()
        guard let i = modelo.actual, i < coincidencias.count, let m = maqueta else { return }
        let c = Int(coincidencias[i].colocado)
        guard c < m.colocados.count else { return }
        let y = CGFloat(m.colocados[c].y)
        if y < arriba + 20 || y > arriba + altoVisto * 0.8 { irA(y: max(y - altoVisto * 0.3, 0)) }
    }

    func dejarDeBuscar() {
        modelo.buscando = false
        coincidencias = []
        modelo.encontradas = 0
        modelo.actual = nil
        pagina.setNeedsDisplay()
    }

    // MARK: Desplazar sola

    func alternarAvance() {
        if avance.activo {
            avance.activo = false
            reloj.parar()
            return
        }
        guard maqueta != nil else { return }
        if LeerEnVoz.compartida.suena(url.path) { LeerEnVoz.compartida.soltar() }
        avance.ppm = nucleo.palabrasPorMinuto()
        avance.enMarcha = true
        avance.antes = nil
        avance.activo = true
        if (avance.palabras ?? 0) <= 0 { avance.palabras = max(Int(documento?.palabras() ?? 0), 250) }
        reloj.empezar()
    }

    func cambiarVelocidad(_ sentido: Int32) {
        avance.ppm = otraVelocidadDeLeer(ppm: avance.ppm, sentido: sentido)
        reloj.reponerQuedan()
        let ppm = avance.ppm, nucleo = self.nucleo
        colaGuardar.async { try? nucleo.guardarPalabrasPorMinuto(ppm: ppm) }
    }

    // MARK: Escuchar

    private var voz: LeerEnVoz { .compartida }

    var escuchando: Bool { enganchada && voz.suena(url.path) }

    private func fracciones(_ m: MaquetaReflujo) -> [Float] {
        let alto = max(m.alto, 1)
        return m.paraLeer.map { Int($0) < m.colocados.count ? min(max(m.colocados[Int($0)].y / alto, 0), 1) : 0 }
    }

    /// **Escuchar**: con la voz puesta, play/pausa; si no, se empieza por el
    /// verde o, si no hay, por lo que asoma arriba.
    func escuchar() {
        if escuchando {
            voz.alternar()
            return
        }
        guard let m = maqueta else { return }
        let idx = m.paraLeer.map { Int($0) }.filter { $0 < m.colocados.count }
        guard !idx.isEmpty else {
            avisar("Este documento no tiene texto que leer")
            return
        }
        if avance.activo { alternarAvance() }
        let fr = fracciones(m)
        let y = arriba
        let porArriba = idx.firstIndex { CGFloat(m.colocados[$0].y + m.colocados[$0].alto) > y + 4 } ?? 0
        let desde = lectura?.voz.flatMap { parrafoDelVerde(fracciones: fr, verde: $0) }.map { Int($0) } ?? porArriba
        let parrafos = idx.map { m.colocados[$0].texto }
        let muestra = String(parrafos.dropFirst(desde).prefix(40).joined(separator: " ").prefix(4000))
        let idioma = VozAlta.idiomaParaLeer(muestra: muestra, lang: "", delEquipo: VozAlta.idiomaDeLaApp)
        let titulo = documento.map { $0.titulo() }.flatMap { $0.isEmpty ? nil : $0 }
            ?? (modelo.nombre as NSString).deletingPathExtension
        let ruta = url.path, nucleo = self.nucleo
        let cola = DispatchQueue(label: "pixpin.voz.verde.reflujo", qos: .utility)
        let doc = LeerEnVoz.Documento(clave: ruta, titulo: titulo, parrafos: parrafos, idioma: idioma) { i in
            let verde = i.map { VozReflujo(parrafo: UInt32($0), fraccion: $0 < fr.count ? fr[$0] : 0) }
            cola.async {
                var l = nucleo.lecturaDeReflujo(documento: ruta)
                l.voz = verde
                try? nucleo.guardarLecturaDeReflujo(documento: ruta, lectura: l)
            }
        }
        guard voz.arrancar(doc, lector: self) else {
            avisar("El iPad no tiene voz para «\(VozAlta.partes(idioma).lengua)»: Ajustes › Accesibilidad › Contenido leído")
            return
        }
        enganchada = true
        voz.leer(desde: min(desde, parrafos.count - 1))
        modelo.poniendoMarca = false
        objectWillChange.send()
    }

    /// Ya hay algo colocado: si se estaba escuchando este documento con el
    /// visor cerrado, se engancha.
    private func alColocarLaVoz() {
        guard engancharAlColocar, let m = maqueta else { return }
        engancharAlColocar = false
        guard voz.suena(url.path), !enganchada else { return }
        enganchada = voz.enganchar(self, clave: url.path, cuantos: m.paraLeer.count)
    }

    // MARK: Compartir e imprimir

    /// **El documento en hojas de papel**, con lo anotado: cada hoja corta
    /// entre dos bloques (sin partir renglones), en claro.
    func pdf() -> URL? {
        guard let m = maqueta else { return nil }
        let (a, b) = limites()
        let ancho = max(b - a, 1)
        let margen = ancho * 0.06
        let papel = CGRect(x: 0, y: 0, width: 595, height: 842)
        let k = papel.width / (ancho + 2 * margen)
        let util = (papel.height / k) - 2 * margen
        // Los cortes: donde empieza cada hoja.
        var cortes: [CGFloat] = [0]
        var desde: CGFloat = 0
        let fin = CGFloat(m.alto)
        while desde + util < fin {
            var corte = desde + util
            if let c = m.colocados.first(where: { CGFloat($0.y) < corte && CGFloat($0.y + $0.alto) > corte }),
               CGFloat(c.y) > desde + util * 0.5 {
                corte = CGFloat(c.y)
            }
            cortes.append(corte)
            desde = corte
        }
        let guardados = (colores, lineas, adaptada)
        colores = ColoresReflujo.de(TonoHojas())
        lineas = [:]
        adaptada = nil
        defer {
            (colores, lineas, adaptada) = guardados
            pagina.setNeedsDisplay()
        }
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("anotado-\(UUID().uuidString)")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let nombre = (modelo.nombre as NSString).deletingPathExtension
        let salida = dir.appendingPathComponent("\(nombre.isEmpty ? "Documento" : nombre).pdf")
        let pintor = UIGraphicsPDFRenderer(bounds: papel)
        do {
            try pintor.writePDF(to: salida) { c in
                for (n, y0) in cortes.enumerated() {
                    c.beginPage()
                    let y1 = n + 1 < cortes.count ? cortes[n + 1] : fin + 40
                    let ctx = c.cgContext
                    ctx.saveGState()
                    ctx.scaleBy(x: k, y: k)
                    ctx.translateBy(x: margen - a, y: margen - y0)
                    let zona = CGRect(x: a, y: y0, width: ancho, height: y1 - y0)
                    ctx.clip(to: zona.insetBy(dx: -margen, dy: 0))
                    pintar(en: ctx, sucio: zona, linea: 1 / k, conTinta: true, marcar: false)
                    ctx.restoreGState()
                }
            }
        } catch {
            return nil
        }
        return salida
    }

    func compartir() {
        var cosas: [Any] = [url]
        let hecho = pdf()
        if let hecho { cosas.append(hecho) }
        SalidaDocumentoIPad.compartir(cosas, desde: desplazar) { SalidaDocumentoIPad.olvidar(hecho) }
    }

    func imprimir() {
        guard let hecho = pdf(), let d = try? Data(contentsOf: hecho) else { return }
        SalidaDocumentoIPad.olvidar(hecho)
        SalidaDocumentoIPad.imprimir(d, titulo: (modelo.nombre as NSString).deletingPathExtension, desde: desplazar)
    }

    func avisar(_ texto: String) {
        modelo.nota = texto
        tareaNota?.cancel()
        let t = DispatchWorkItem { [weak self] in MainActor.assumeIsolated { self?.modelo.nota = nil } }
        tareaNota = t
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.5, execute: t)
    }
}

extension ControladorReflujoIPad: UIScrollViewDelegate {
    func viewForZooming(in scrollView: UIScrollView) -> UIView? { contenido }

    func scrollViewDidZoom(_ scrollView: UIScrollView) {
        centrar()
        pagina.setNeedsDisplay()
        programarLectura()
    }

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        programarLectura()
    }
}

extension ControladorReflujoIPad: UIPencilInteractionDelegate {
    func pencilInteraction(_ interaction: UIPencilInteraction, didReceiveTap tap: UIPencilInteraction.Tap) {
        guard UIPencilInteraction.preferredTapAction != .ignore else { return }
        elegir(modelo.herramienta == "borrador" ? ultimaHerramienta : "borrador")
    }
}

extension ControladorReflujoIPad: LectorEnganchado {
    func vozSuena(_ i: Int, primero: Bool) {
        guard let m = maqueta, i >= 0, i < m.paraLeer.count else { return }
        let k = Int(m.paraLeer[i])
        guard k < m.colocados.count else { return }
        sonando = k
        palabra = nil
        let c = m.colocados[k]
        let y = CGFloat(c.y), abajo = CGFloat(c.y + c.alto)
        if !modelo.anotando, y < arriba + altoVisto * 0.08 || abajo > arriba + altoVisto * 0.85 {
            irA(y: max(y - altoVisto * 0.25, 0))
        }
        ponerVerde(VozReflujo(parrafo: UInt32(i), fraccion: min(max(c.y / max(m.alto, 1), 0), 1)))
    }

    func vozPalabra(_ r: NSRange?) { palabra = r }

    func vozAcabo() {
        sonando = nil
        palabra = nil
        ponerVerde(nil)
    }

    func vozFuera() {
        enganchada = false
        sonando = nil
        palabra = nil
    }
}

// MARK: - SwiftUI

struct VistaDesplazarReflujo: UIViewRepresentable {
    let c: ControladorReflujoIPad

    func makeUIView(context: Context) -> DesplazarReflujoIPad { c.desplazar }
    func updateUIView(_ v: DesplazarReflujoIPad, context: Context) {}
}

/// **La lectura seguida entera**: el texto, la barra comun arriba y el riel
/// de la tinta a la derecha. `alPaginar`: un Word se puede ver tambien
/// paginado como Office (el lector con tinta por hojas).
struct VisorReflujoIPad: View {
    @StateObject private var c: ControladorReflujoIPad
    var alPaginar: (() -> Void)?

    init(url: URL, titulo: String, nucleo: Nucleo, alPaginar: (() -> Void)? = nil) {
        _c = StateObject(wrappedValue: ControladorReflujoIPad(url: url, nucleo: nucleo, titulo: titulo))
        self.alPaginar = alPaginar
    }

    var body: some View {
        CuerpoReflujoIPad(c: c, modelo: c.modelo, avance: c.avance, voz: LeerEnVoz.compartida, alPaginar: alPaginar)
    }
}

private struct CuerpoReflujoIPad: View {
    let c: ControladorReflujoIPad
    @ObservedObject var modelo: ModeloReflujoIPad
    @ObservedObject var avance: AvanceLectorIPad
    @ObservedObject var voz: LeerEnVoz
    var alPaginar: (() -> Void)?
    @ObservedObject private var noche = ModoNoche.compartido
    @FocusState private var enBuscar: Bool

    private var esLibro: Bool { c.url.pathExtension.lowercased() == "epub" }

    var body: some View {
        ZStack(alignment: .top) {
            Color(uiColor: .systemBackground).ignoresSafeArea()
            VistaDesplazarReflujo(c: c).ignoresSafeArea(edges: .bottom)
            VStack(spacing: 8) {
                BarraVisorIPad(tipo: esLibro ? "Libro" : "Word", icono: esLibro ? "book" : "doc.text",
                               titulo: modelo.nombre) { botones }
                if modelo.buscando { cajaBuscar }
                if let nota = modelo.nota {
                    Text(nota).font(.system(size: 14))
                        .foregroundStyle(Color(red: 0.96, green: 0.96, blue: 0.97))
                        .padding(.horizontal, 14).frame(height: 36)
                        .background(Color(red: 0.118, green: 0.118, blue: 0.125, opacity: 0.95),
                                    in: RoundedRectangle(cornerRadius: 12))
                        .allowsHitTesting(false)
                }
            }
            HStack(alignment: .top) {
                Spacer()
                riel.padding(.trailing, 10)
            }
            .padding(.top, 70 + (modelo.buscando ? 52 : 0))
            VStack(spacing: 8) {
                Spacer()
                BarraAvanceIPad(avance: avance, cambiar: { c.cambiarVelocidad($0) }, cerrar: { c.alternarAvance() })
                BarraEscuchar(voz: voz, clave: { c.url.path })
                if modelo.poniendoMarca {
                    TiraEmojisMarca { e in c.ponerMarca(emoji: e) } cerrar: { modelo.poniendoMarca = false }
                }
                if let aviso = modelo.aviso {
                    Text(aviso).font(.callout).padding(.horizontal, 12).padding(.vertical, 6)
                        .background(Paleta.v2.rojo.opacity(0.25), in: Capsule())
                        .onTapGesture { modelo.aviso = nil }
                } else if modelo.colocando {
                    Label("Colocando…", systemImage: "hourglass").font(.caption)
                        .padding(.horizontal, 12).padding(.vertical, 6)
                        .background(.regularMaterial, in: Capsule())
                }
            }
            .padding(.bottom, 12)
        }
        .onAppear { c.abrir() }
        .onDisappear { c.cerrar() }
        .onChange(of: noche.activo) { _, _ in c.aplicarTono() }
        .sheet(isPresented: $modelo.viendoIndice) {
            HojaIndiceIPad(entradas: c.entradasDelIndice, actual: c.bloqueDeArriba, rotulo: nil,
                           ir: { c.ir(aBloque: UInt32($0.destino)) })
        }
    }

    @ViewBuilder private var botones: some View {
        Button {
            modelo.buscando.toggle()
            if modelo.buscando { enBuscar = true } else { c.dejarDeBuscar() }
        } label: { Image(systemName: "magnifyingglass") }
            .keyboardShortcut("f", modifiers: .command)
            .help("Buscar (⌘F)")
        if !modelo.indice.isEmpty {
            Button { modelo.viendoIndice = true } label: { Image(systemName: "list.bullet.indent") }
                .help("Índice")
        }
        Menu {
            Button("Letra más grande") { c.cambiarTamano(true) }
            Button("Letra más pequeña") { c.cambiarTamano(false) }
            Section("Tipo") {
                ForEach(0 ..< ModeloReflujoIPad.tipos.count, id: \.self) { i in
                    Toggle(ModeloReflujoIPad.tipos[i], isOn: Binding(get: { Int(modelo.tipo) == i },
                                                                    set: { _ in c.ponerTipo(UInt8(i)) }))
                }
            }
            Section("Grosor") {
                ForEach(0 ..< ModeloReflujoIPad.grosores.count, id: \.self) { i in
                    Toggle(ModeloReflujoIPad.grosores[i], isOn: Binding(get: { Int(modelo.grosor) == i },
                                                                       set: { _ in c.ponerGrosor(UInt8(i)) }))
                }
            }
        } label: {
            Image(systemName: modelo.fijada ? "textformat.size.larger" : "textformat.size")
        }
        .help(modelo.fijada ? "La letra está fijada: hay tinta encima" : "La letra · \(modelo.tamano) %")
        Button { c.escuchar() } label: {
            Image(systemName: voz.leyendo && c.escuchando ? "speaker.wave.2.fill" : "speaker.wave.2")
        }
        .help("Leer en voz alta (L)")
        Button { c.alternarAvance() } label: {
            Image(systemName: "chevron.down.2").foregroundStyle(avance.activo ? Paleta.v2.acento : Color.primary)
        }
        .help("Desplazar sola (D)")
        Button { c.compartir() } label: { Image(systemName: "square.and.arrow.up") }
            .keyboardShortcut("s", modifiers: [.command, .shift])
            .help("Compartir con lo anotado (⇧⌘S)")
        Menu {
            Button { c.imprimir() } label: { Label("Imprimir… (⌘P)", systemImage: "printer") }
            if let alPaginar {
                Button(action: alPaginar) { Label("Ver paginado (como Word)", systemImage: "doc.richtext") }
            }
            Divider()
            Button("Más espacio a la izquierda") { c.espacio(izquierda: true, mas: true) }
            Button("Menos espacio a la izquierda") { c.espacio(izquierda: true, mas: false) }
            Button("Más espacio a la derecha") { c.espacio(izquierda: false, mas: true) }
            Button("Menos espacio a la derecha") { c.espacio(izquierda: false, mas: false) }
            if !modelo.marcas.isEmpty {
                Divider()
                Button(role: .destructive) { c.quitarMarcas() } label: {
                    Label("Quitar los marcadores", systemImage: "bookmark.slash")
                }
            }
        } label: { Image(systemName: "ellipsis.circle") }
            .help("Más")
            .background {
                Button("") { c.imprimir() }.keyboardShortcut("p", modifiers: .command).opacity(0)
                    .accessibilityHidden(true)
            }
    }

    private var cajaBuscar: some View {
        HStack(spacing: 10) {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField("Buscar en el documento", text: $modelo.consulta)
                .textFieldStyle(.plain)
                .focused($enBuscar)
                .submitLabel(.search)
                .onSubmit { c.siguiente(1) }
                .onChange(of: modelo.consulta) { _, q in c.buscar(q) }
            if !modelo.consulta.isEmpty {
                Text(modelo.encontradas == 0 ? "Sin resultados" : "\((modelo.actual ?? 0) + 1) de \(modelo.encontradas)")
                    .font(.caption.monospacedDigit()).foregroundStyle(.secondary)
            }
            Button { c.siguiente(-1) } label: { Image(systemName: "chevron.up") }
                .keyboardShortcut("g", modifiers: [.command, .shift])
            Button { c.siguiente(1) } label: { Image(systemName: "chevron.down") }
                .keyboardShortcut("g", modifiers: .command)
            Button { c.dejarDeBuscar() } label: { Image(systemName: "xmark.circle.fill") }
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 12)
        .frame(height: 40)
        .frame(maxWidth: 520)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))
        .padding(.horizontal, 16)
    }

    private var riel: some View {
        VStack(spacing: 10) {
            VStack(spacing: 2) {
                BotonRielIPad(simbolo: "bookmark", ayuda: "Poner un marcador", encendido: modelo.poniendoMarca) {
                    modelo.poniendoMarca.toggle()
                }
                BotonRielIPad(simbolo: "pencil.tip", ayuda: modelo.anotando ? "Dejar de anotar" : "Anotar",
                              encendido: modelo.anotando) { c.alternarAnotar() }
            }
            .padding(3)
            .background(RoundedRectangle(cornerRadius: 9).fill(.regularMaterial))
            .shadow(color: .black.opacity(0.16), radius: 6, y: 2)
            if modelo.anotando {
                VStack(spacing: 2) {
                    BotonRielIPad(simbolo: "pencil", ayuda: "Lápiz", encendido: modelo.herramienta == "lapiz") {
                        c.elegir("lapiz")
                    }
                    BotonRielIPad(simbolo: "highlighter", ayuda: "Resaltador",
                                  encendido: modelo.herramienta == "resaltador") { c.elegir("resaltador") }
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
                    BotonRielIPad(simbolo: "arrow.uturn.backward", ayuda: "Deshacer (⌘Z)") { c.deshacer() }
                        .keyboardShortcut("z", modifiers: .command)
                    BotonRielIPad(simbolo: "arrow.uturn.forward", ayuda: "Rehacer (⇧⌘Z)") { c.rehacer() }
                        .keyboardShortcut("z", modifiers: [.command, .shift])
                    BotonRielIPad(simbolo: "hand.point.up.left", ayuda: "Dibujar con el dedo",
                                  encendido: modelo.conDedo) { c.alternarDedo() }
                }
                .padding(3)
                .background(RoundedRectangle(cornerRadius: 9).fill(.regularMaterial))
                .shadow(color: .black.opacity(0.16), radius: 6, y: 2)
            }
            if !modelo.marcas.isEmpty {
                ScrollView(.vertical, showsIndicators: false) {
                    VStack(spacing: 2) {
                        ForEach(modelo.marcas, id: \.id) { m in
                            Button { c.ir(a: m) } label: {
                                VStack(spacing: 0) {
                                    Text(m.emoji).font(.system(size: 18))
                                    Text("\(Int((m.fraccion * 100).rounded())) %").font(.system(size: 9).monospacedDigit())
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
                .background(RoundedRectangle(cornerRadius: 9).fill(.regularMaterial))
                .shadow(color: .black.opacity(0.16), radius: 6, y: 2)
            }
        }
        .fixedSize()
    }
}
