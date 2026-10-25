// **El lector de documentos del iPad**: un PDF (o un Word o un PowerPoint
// paginados como Office) con tinta del Apple Pencil, resaltador, goma,
// marcadores, espacios para anotar a cada lado y la voz. Es el
// `LectorDocumento.swift` del Mac (y `lector_pdf.rs` de Windows) con UIKit:
//
// - Las hojas las pinta **PDFKit** (`PDFView`): desplazar, pellizcar,
//   buscar (el buscador del sistema, `UIFindInteraction`), miniaturas
//   (`PDFThumbnailView`) y elegir texto son suyos. Un Word o un PowerPoint se
//   maquetan en Rust (`DocumentoOffice`) y cada hoja se pinta en un PDF de una
//   pagina (`PintorOffice`), que se va anadiendo mientras se maqueta el resto.
// - La tinta de cada hoja es un `EditorLienzo` de Rust abierto con
//   `Nucleo.anotarHoja`, en las unidades de la hoja (1400 de ancho), y se
//   guarda donde la guardan Windows, Android y el Mac (`anot-<uid>-p<n>` y su
//   marco) para que viaje. Nada de PencilKit: lo que se guarda es del nucleo.
// - **La tinta no tiembla**: cada hoja lleva encima su vista de tinta
//   (`PDFPageOverlayViewProvider`, iOS 16), que se mueve y se acerca con ella.
// - **El Apple Pencil anota siempre**: apoyarlo empieza a anotar con la
//   ultima herramienta; el dedo desplaza (o pinta, con «Dibujar con el dedo»,
//   y entonces se desplaza con dos dedos). El doble toque del lapiz cambia a
//   la goma y vuelta (lo que diga Ajustes del iPad).
//
// Rendimiento (§4bis del plan), como el Mac:
// - solo se abren las tintas de las hojas que se ven y tienen fichero
//   (`hojasConTinta`) o la que se empieza a anotar, con tope;
// - lo que se pinta de cada una se pide a Rust una vez por cambio (`plano`);
//   al trazar solo se repinta el trocito que cambia;
// - guardar va solo, 1 s despues del ultimo trazo, en una cola `.utility`;
// - lo que cambia en el disco con el lector abierto se relee o se junta
//   (`AlDiaLector`).

import Combine
import PDFKit
import PixPinCore
import SwiftUI
import UIKit
import UIKit.UIGestureRecognizerSubclass

public extension Notification.Name {
    /// Se guardo lo anotado de un documento: `object` es su ruta.
    static let pixpinDocumentoAnotado = Notification.Name("PixPinDocumentoAnotado")
}

/// Los colores de la tinta del lector (la paleta de Excalidraw, como el
/// movil, Windows y el Mac).
public struct ColorTinta: Identifiable, Equatable, Sendable {
    public let id: String
    public let r: Float, g: Float, b: Float

    public static let todos: [ColorTinta] = [
        .init(id: "negro", r: 0.118, g: 0.118, b: 0.118),
        .init(id: "rojo", r: 0.878, g: 0.192, b: 0.192),
        .init(id: "azul", r: 0.098, g: 0.443, b: 0.761),
        .init(id: "verde", r: 0.184, g: 0.620, b: 0.267),
        .init(id: "amarillo", r: 1.0, g: 0.831, b: 0.231),
    ]

    var color: Color { Color(red: Double(r), green: Double(g), blue: Double(b)) }
}

/// Lo que ve la barra y el riel del lector.
@MainActor
final class ModeloLectorIPad: ObservableObject {
    /// `leer`, `lapiz`, `resaltador` o `borrador`.
    @Published var herramienta = "leer"
    @Published var color = ColorTinta.todos[0]
    @Published var espacios: UInt8 = 0
    @Published var marcas: [MarcaDocumento] = []
    @Published var pagina = 0
    @Published var paginas = 0
    @Published var maquetando = false
    @Published var aviso: String?
    @Published var nombre = ""
    /// Eligiendo el emoticono de un marcador nuevo.
    @Published var poniendoMarca = false
    @Published var miniaturas = false
    /// Pantalla completa: sin barra ni riel (un toque arriba los devuelve).
    @Published var completa = false
    /// El dedo tambien pinta (y se desplaza con dos dedos).
    @Published var conDedo = false
    @Published var pidiendoPagina = false
    @Published var paginaPedida = ""
    @Published var puedeDeshacer = false
    /// La hoja del indice abierta.
    @Published var viendoIndice = false
    /// Escribiendo el nombre nuevo (F2).
    @Published var renombrando = false
    @Published var nombreNuevo = ""
    /// Un aviso que se va solo («Ahora se llama…»).
    @Published var nota: String?

    var anotando: Bool { herramienta != "leer" }

    /// El nombre con la hoja que se ve: «Tema 3 · 4/20» (como Windows).
    var titulo: String {
        paginas > 1 ? "\(nombre) · \(pagina + 1)/\(paginas)" : nombre
    }
}

// MARK: - La vista de cada hoja

/// **La tinta de una hoja**, encima de ella (la da PDFKit con
/// `PDFPageOverlayViewProvider`). No recibe toques: los recoge el gesto del
/// lector. Su capa se alarga a los lados con los espacios para anotar.
final class HojaTintaIPad: UIView {
    weak var controlador: ControladorLectorIPad?
    let indice: Int
    let capa = CapaHojaTintaIPad()

    init(controlador: ControladorLectorIPad, indice: Int) {
        self.controlador = controlador
        self.indice = indice
        super.init(frame: .zero)
        isUserInteractionEnabled = false
        backgroundColor = .clear
        isOpaque = false
        clipsToBounds = false
        capa.hoja = self
        addSubview(capa)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    override func layoutSubviews() {
        super.layoutSubviews()
        colocar()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        colocar()
    }

    /// La capa: la hoja y sus espacios para anotar.
    func colocar() {
        let bits = controlador?.modelo.espacios ?? 0
        let m = bounds.width * 0.75
        let izq: CGFloat = bits & 1 != 0 ? m : 0
        let der: CGFloat = bits & 2 != 0 ? m : 0
        let marco = CGRect(x: -izq, y: 0, width: bounds.width + izq + der, height: bounds.height)
        if capa.frame != marco {
            capa.frame = marco
            capa.setNeedsDisplay()
        }
        ajustarNitidez()
    }

    /// Al acercar, la capa se pinta con mas puntos (con tope: una capa
    /// enorme se come la memoria).
    func ajustarNitidez() {
        guard window != nil, bounds.width > 0 else { return }
        let enPantalla = convert(bounds, to: nil).width
        let k = max(enPantalla / bounds.width, 1)
        let deseada = traitCollection.displayScale * k
        let tope = max(4096 / max(capa.bounds.width, capa.bounds.height, 1), 1)
        let nueva = min(deseada, tope)
        if abs(capa.contentScaleFactor - nueva) > 0.05 {
            capa.contentScaleFactor = nueva
            capa.setNeedsDisplay()
        }
    }

    func repintar() { capa.setNeedsDisplay() }

    /// Repinta solo `rect` (en las coordenadas de `vista`).
    func repintar(_ rect: CGRect, desde vista: UIView) {
        guard !rect.isNull else { return repintar() }
        capa.setNeedsDisplay(capa.convert(rect, from: vista))
    }
}

final class CapaHojaTintaIPad: UIView {
    weak var hoja: HojaTintaIPad?

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
        guard let ctx = UIGraphicsGetCurrentContext(), let h = hoja, let c = h.controlador else { return }
        // La hoja dentro de la capa (la capa empieza en el espacio izquierdo).
        let pagina = CGRect(x: -frame.minX, y: 0, width: h.bounds.width, height: h.bounds.height)
        c.pintar(h.indice, en: ctx, pagina: pagina, sucio: rect)
    }
}

// MARK: - El gesto de la tinta

/// Recoge el Apple Pencil (y el dedo o el raton si toca) para la tinta.
/// Empieza en cuanto se apoya: el desplazamiento de PDFKit no se lo lleva.
final class GestoTintaIPad: UIGestureRecognizer {
    weak var controlador: ControladorLectorIPad?
    private var activo: UITouch?

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        guard activo == nil, let t = touches.first, let c = controlador, c.aceptar(t) else {
            for t in touches where t !== activo { ignore(t, for: event) }
            return
        }
        activo = t
        state = .began
        c.trazo(.pulsar, [t], vista: view)
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let a = activo, touches.contains(a) else { return }
        let todos = event.coalescedTouches(for: a) ?? [a]
        controlador?.trazo(.mover, todos, vista: view)
        state = .changed
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let a = activo, touches.contains(a) else { return }
        controlador?.trazo(.soltar, [a], vista: view)
        activo = nil
        state = .ended
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let a = activo, touches.contains(a) else { return }
        controlador?.trazo(.cancelar, [a], vista: view)
        activo = nil
        state = .cancelled
    }

    override func reset() {
        super.reset()
        activo = nil
    }
}

// MARK: - El controlador

/// **Un documento abierto en el lector.**
@MainActor
final class ControladorLectorIPad: NSObject, ObservableObject {
    /// Cambia si se renombra un documento suelto (se mueve con sus hermanos).
    private(set) var url: URL
    let nucleo: Nucleo
    let modelo = ModeloLectorIPad()
    let voz = VozDocumentoIPad()
    let vistaPDF = PDFView()
    let miniaturas = PDFThumbnailView()
    private let gesto = GestoTintaIPad(target: nil, action: nil)
    /// La pagina que sube sola.
    let avance = AvanceLectorIPad()
    private lazy var reloj = RelojAvanceIPad(avance: avance) { [weak self] in
        self.flatMap { ControladorLectorIPad.desplazador(en: $0.vistaPDF) }
    }
    /// Los marcos numerados para imprimir.
    let marcos: MarcosDoc
    private let goma = GomaDeMarcoIPad()
    private var inicioMarco: CGPoint?
    private var tareaNota: DispatchWorkItem?

    /// Las vistas de tinta de las hojas que PDFKit tiene puestas.
    private var hojas: [Int: HojaTintaIPad] = [:]
    /// La tinta de cada hoja abierta, y lo ultimo leido de ella.
    private var editores: [Int: EditorLienzo] = [:]
    private var leidos: [Int: (version: UInt64, lienzo: LienzoLeido)] = [:]
    /// La tinta adaptada al papel oscuro de cada hoja.
    private var adaptadas: [Int: (version: UInt64, lienzo: LienzoLeido)] = [:]
    private var coloresAdaptados: [ColorLienzo: ColorLienzo] = [:]
    private var cargando: Set<Int> = []
    /// Las hojas con fichero de tinta (las demas no se abren hasta anotarlas).
    private var conTinta: Set<Int> = []
    /// Por orden de uso, para soltar las mas viejas.
    private var usadas: [Int] = []
    private static let topeDeHojas = 16
    private var releyendo: Set<Int> = []

    /// La hoja del trazo en curso y la ultima anotada (deshacer).
    private var hojaDelTrazo: Int?
    private var ultimaAnotada: Int?
    private var ultimoPunto: CGPoint?
    /// Con que se empieza al apoyar el lapiz, y la de antes de la goma.
    private var ultimaHerramienta = "lapiz"

    private let alDia = AlDiaLector()
    private let colaGuardar = DispatchQueue(label: "pixpin.lector.guardar", qos: .utility)
    private var guardadoPendiente: DispatchWorkItem?
    private var lecturaPendiente: DispatchWorkItem?
    private var tareaOffice: Task<Void, Never>?
    private var vigias: [NSObjectProtocol] = []
    private var suscripciones: Set<AnyCancellable> = []
    private var mirarDisco: Timer?
    private var abierto = false
    private var cerrado = false
    /// Adonde va lo anotado (lo dice el nucleo al abrir).
    private var donde: DondeAnota = .juntoAlDocumento

    init(url: URL, nucleo: Nucleo, titulo: String) {
        self.url = url
        self.nucleo = nucleo
        marcos = MarcosDoc(documento: url, nucleo: nucleo)
        super.init()
        modelo.nombre = titulo
        montar()
        DocumentosDelIPad.activar()
    }

    private func montar() {
        vistaPDF.displayMode = .singlePageContinuous
        vistaPDF.displayDirection = .vertical
        vistaPDF.displaysPageBreaks = true
        vistaPDF.autoScales = true
        vistaPDF.pageOverlayViewProvider = self
        vistaPDF.isFindInteractionEnabled = true
        ponerFondo()
        gesto.controlador = self
        gesto.cancelsTouchesInView = true
        gesto.delaysTouchesBegan = false
        gesto.delegate = self
        vistaPDF.addGestureRecognizer(gesto)
        let lapiz = UIPencilInteraction()
        lapiz.delegate = self
        vistaPDF.addInteraction(lapiz)
        miniaturas.pdfView = vistaPDF
        miniaturas.layoutMode = .horizontal
        miniaturas.thumbnailSize = CGSize(width: 44, height: 60)
        miniaturas.backgroundColor = .secondarySystemBackground
        voz.vistaPDF = vistaPDF
        voz.alParar = { [weak self] pagina, altura in self?.ponerMarcaDeVoz(pagina: pagina, altura: altura) }
        goma.frame = vistaPDF.bounds
        goma.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        vistaPDF.addSubview(goma)
        reloj.quieta = { [weak self] in
            guard let self else { return true }
            return self.modelo.anotando || self.marcos.activo
        }
        // Los marcos se pintan en la capa de cada hoja.
        marcos.$lista.dropFirst().receive(on: RunLoop.main).sink { [weak self] _ in
            MainActor.assumeIsolated { self?.repintarHojas() }
        }.store(in: &suscripciones)
        marcos.$activo.dropFirst().receive(on: RunLoop.main).sink { [weak self] _ in
            MainActor.assumeIsolated { self?.actualizarGestos() }
        }.store(in: &suscripciones)
        actualizarGestos()
        let centro = NotificationCenter.default
        // Aligerado en su sitio mientras se lee: se vuelve a abrir.
        vigias.append(centro.addObserver(forName: .pixpinPdfAligerado, object: nil, queue: .main) { [weak self] n in
            let ruta = n.object as? String
            MainActor.assumeIsolated {
                if let self, ruta == self.url.path { self.recargar() }
            }
        })
        for nombre in [Notification.Name.PDFViewPageChanged, .PDFViewScaleChanged, .PDFViewVisiblePagesChanged] {
            vigias.append(centro.addObserver(forName: nombre, object: vistaPDF, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.alMoverse() }
            })
        }
        // Lo anotado por otro en este documento (el asistente, la
        // sincronizacion): se mira el disco ya.
        vigias.append(centro.addObserver(forName: .pixpinDocumentoAnotado, object: nil, queue: .main) { [weak self] n in
            let ruta = n.object as? String
            MainActor.assumeIsolated {
                if let self, ruta == self.url.path { self.mirarElDisco() }
            }
        })
        // Al irse la app al fondo se guarda ya (iOS puede cerrarla luego).
        vigias.append(centro.addObserver(forName: UIApplication.willResignActiveNotification, object: nil,
                                         queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, !self.cerrado else { return }
                self.guardarLectura()
                if self.guardadoPendiente != nil { self.programarGuardado(espera: 0) }
            }
        })
    }

    // MARK: Abrir y cerrar

    /// Lee lo recordado y pone el documento. Un Word o un PowerPoint se
    /// maquetan en segundo plano, hoja a hoja.
    func abrir() {
        guard !abierto else { return }
        abierto = true
        let ruta = url.path
        let nucleo = self.nucleo
        Task {
            let (lectura, nombre) = await Task.detached(priority: .userInitiated) {
                (nucleo.lecturaDeDocumento(documento: ruta), nucleo.nombreDeDocumento(documento: ruta))
            }.value
            guard !self.cerrado else { return }
            self.modelo.espacios = lectura.espacios
            self.modelo.marcas = lectura.marcas
            self.donde = lectura.donde
            if !nombre.isEmpty { self.modelo.nombre = nombre }
            switch TiposDeDocumento.clase(de: self.url) {
            case .officePaginado:
                self.abrirOffice(lectura)
            default:
                if let doc = ControladorLectorIPad.documento(url: self.url), doc.pageCount > 0 {
                    self.poner(doc, lectura: lectura)
                    return
                }
                // Roto: el de un proyecto se rehace desde su copia limpia.
                let roto = self.url
                let sano = await Task.detached(priority: .userInitiated) { PdfSano.reparar(roto) }.value
                guard !self.cerrado, let sano, let doc = ControladorLectorIPad.documento(url: sano),
                      doc.pageCount > 0 else {
                    self.modelo.aviso = "No se puede abrir este PDF."
                    return
                }
                self.poner(doc, lectura: lectura)
                self.avisar(sano == roto ? "El PDF estaba roto: se rehízo desde su copia limpia."
                                         : "El PDF está roto: se enseña su copia limpia.")
            }
        }
    }

    /// Vuelve a abrir el PDF en el mismo sitio (cambio en su sitio al
    /// aligerarse). Un Word o un PowerPoint no cambian asi.
    private func recargar() {
        guard !cerrado, TiposDeDocumento.clase(de: url) == .pdf, vistaPDF.document != nil,
              let nuevo = ControladorLectorIPad.documento(url: url), nuevo.pageCount > 0 else { return }
        if let viejo = vistaPDF.document { TonosDeDocumentos.compartidos.olvidar(viejo) }
        TonosDeDocumentos.compartidos.poner(tono, a: nuevo)
        let i = modelo.pagina
        let auto = vistaPDF.autoScales, escala = vistaPDF.scaleFactor
        hojas = [:]
        vistaPDF.document = nuevo
        vistaPDF.autoScales = auto
        if !auto { vistaPDF.scaleFactor = escala }
        if let p = nuevo.page(at: min(i, nuevo.pageCount - 1)) { vistaPDF.go(to: p) }
        modelo.paginas = nuevo.pageCount
        ponerMargenes()
        actualizarGestos()
        avance.palabras = nil
        if let r = ColaAligerar.compartida.resultado[url.path] { avisar(r) }
    }

    /// Un aviso arriba que se va solo a los 2,5 s.
    func avisar(_ texto: String) {
        modelo.nota = texto
        tareaNota?.cancel()
        let t = DispatchWorkItem { [weak self] in MainActor.assumeIsolated { self?.modelo.nota = nil } }
        tareaNota = t
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.5, execute: t)
    }

    private func repintarHojas() {
        for h in hojas.values { h.repintar() }
    }

    /// Un PDF con hojas del lector (las que saben pintarse en oscuro).
    static func documento(url: URL) -> PDFDocument? {
        let doc = PDFDocument(url: url)
        doc?.delegate = DelegadoHojas.compartido
        return doc
    }

    private func poner(_ doc: PDFDocument, lectura: LecturaDocumento) {
        TonosDeDocumentos.compartidos.poner(tono, a: doc)
        vistaPDF.document = doc
        modelo.paginas = doc.pageCount
        ponerMargenes()
        actualizarGestos()
        let ruta = url.path
        let n = UInt32(doc.pageCount)
        let nucleo = self.nucleo
        Task {
            let hojas = await Task.detached(priority: .userInitiated) {
                nucleo.hojasConTinta(documento: ruta, paginas: n)
            }.value
            self.conTinta = Set(hojas.map { Int($0) })
            self.cargarVisibles()
        }
        // Por donde se iba y a que aumento, con la vista ya colocada.
        DispatchQueue.main.async { [weak self] in
            MainActor.assumeIsolated { self?.irA(lectura) }
        }
    }

    private func irA(_ lectura: LecturaDocumento) {
        guard let doc = vistaPDF.document else { return }
        vistaPDF.layoutIfNeeded()
        if lectura.zoom.isFinite, abs(lectura.zoom - 1) > 0.01 {
            let ajuste = vistaPDF.scaleFactorForSizeToFit
            if ajuste > 0 {
                vistaPDF.autoScales = false
                vistaPDF.scaleFactor = ajuste * CGFloat(lectura.zoom)
            }
        }
        let i = Int(lectura.pagina.rounded(.down))
        guard i >= 0, i < doc.pageCount, let p = doc.page(at: i), lectura.pagina > 0.001 else { return }
        let b = p.bounds(for: vistaPDF.displayBox)
        let f = CGFloat(lectura.pagina - lectura.pagina.rounded(.down))
        vistaPDF.go(to: CGRect(x: b.minX, y: b.maxY - f * b.height - 1, width: 1, height: 1), on: p)
    }

    private func abrirOffice(_ lectura: LecturaDocumento) {
        modelo.maquetando = true
        let ruta = url.path
        let nucleo = self.nucleo
        tareaOffice = Task { [weak self] in
            let abierto = await Task.detached(priority: .userInitiated) { () -> DocumentoOffice? in
                try? DocumentoOffice.abrir(ruta: ruta, medidor: MedidorCoreText())
            }.value
            guard let office = abierto else {
                self?.modelo.maquetando = false
                self?.modelo.aviso = "No se puede abrir este documento."
                return
            }
            let doc = PDFDocument()
            var n: UInt32 = 0
            while !Task.isCancelled {
                let k = n
                let pagina = await Task.detached(priority: .userInitiated) { () -> Data? in
                    guard let p = office.pagina(n: k),
                          let pintor = try? PintorOffice(pagina: p, imagen: { office.imagen(id: $0) })
                    else { return nil }
                    return pintor.pdf()
                }.value
                let suelta = pagina.flatMap { PDFDocument(data: $0) }
                suelta?.delegate = DelegadoHojas.compartido
                guard let self, !self.cerrado, let hoja = suelta?.page(at: 0) else { break }
                doc.insert(hoja, at: doc.pageCount)
                if n == 0 {
                    self.poner(doc, lectura: lectura)
                } else {
                    self.modelo.paginas = doc.pageCount
                    if Int(lectura.pagina) == Int(n) { self.vistaPDF.go(to: hoja) }
                }
                n += 1
            }
            self?.modelo.maquetando = false
            if let self, doc.pageCount > 0 {
                let total = UInt32(doc.pageCount)
                let hojas = await Task.detached(priority: .utility) {
                    nucleo.hojasConTinta(documento: ruta, paginas: total)
                }.value
                self.conTinta = Set(hojas.map { Int($0) })
                self.cargarVisibles()
            } else if let self, !self.cerrado {
                self.modelo.aviso = "Este documento no tiene hojas."
            }
        }
    }

    /// El visor se cierra: se guarda todo lo pendiente.
    func cerrar() {
        guard !cerrado else { return }
        cerrado = true
        voz.parar()
        reloj.parar()
        avance.activo = false
        suscripciones = []
        tareaOffice?.cancel()
        guardadoPendiente?.cancel()
        lecturaPendiente?.cancel()
        guardarLectura()
        let pares = editores.map { ($0.key, $0.value) }
        let ruta = url.path, alDia = self.alDia
        colaGuardar.async { _ = ControladorLectorIPad.guardarTinta(pares, ruta: ruta, alDia: alDia) }
        for v in vigias { NotificationCenter.default.removeObserver(v) }
        vigias = []
        mirarDisco?.invalidate()
        mirarDisco = nil
        if let doc = vistaPDF.document { TonosDeDocumentos.compartidos.olvidar(doc) }
    }

    // MARK: Hojas y geometria

    /// La hoja en la vista del PDF (Y hacia abajo).
    private func geometria(_ pagina: PDFPage) -> GeometriaHoja {
        GeometriaHoja(rect: vistaPDF.convert(pagina.bounds(for: vistaPDF.displayBox), from: pagina).standardized)
    }

    /// El alto de una hoja en unidades de la tinta (tal como se ve, girada).
    func alto(_ pagina: PDFPage) -> Float {
        let b = pagina.bounds(for: vistaPDF.displayBox)
        let girada = pagina.rotation % 180 != 0
        return GeometriaHoja.altoDeHoja(ancho: girada ? b.height : b.width, alto: girada ? b.width : b.height)
    }

    private func indice(_ p: PDFPage) -> Int {
        vistaPDF.document?.index(for: p) ?? 0
    }

    private func alMoverse() {
        if let p = vistaPDF.currentPage {
            let i = indice(p)
            if i != modelo.pagina {
                modelo.pagina = i
                programarLectura()
            }
        }
        cargarVisibles()
        for h in hojas.values { h.ajustarNitidez() }
    }

    /// Abre (fuera del hilo principal) la tinta de las hojas que se ven y
    /// tienen fichero.
    private func cargarVisibles() {
        for p in vistaPDF.visiblePages {
            let i = indice(p)
            guard conTinta.contains(i), editores[i] == nil, !cargando.contains(i) else { continue }
            cargando.insert(i)
            let ruta = url.path, h = alto(p), nucleo = self.nucleo
            Task {
                let ed = await Task.detached(priority: .userInitiated) {
                    try? nucleo.anotarHoja(documento: ruta, pagina: UInt32(i), alto: h)
                }.value
                self.cargando.remove(i)
                if let ed, self.editores[i] == nil, !self.cerrado {
                    self.guardarEditor(ed, en: i)
                    self.hojas[i]?.repintar()
                }
            }
        }
    }

    private func guardarEditor(_ ed: EditorLienzo, en i: Int) {
        editores[i] = ed
        aplicarHerramienta(ed)
        usar(i)
        empezarAMirarElDisco()
    }

    /// Apunta que la hoja `i` se uso; si hay demasiadas abiertas, se guarda y
    /// se suelta la mas vieja que no se ve.
    private func usar(_ i: Int) {
        usadas.removeAll { $0 == i }
        usadas.append(i)
        let visibles = Set(vistaPDF.visiblePages.map(indice))
        while usadas.count > ControladorLectorIPad.topeDeHojas,
              let vieja = usadas.first(where: { !visibles.contains($0) && $0 != hojaDelTrazo }) {
            usadas.removeAll { $0 == vieja }
            if let ed = editores.removeValue(forKey: vieja) {
                let alDia = self.alDia
                colaGuardar.async { _ = try? alDia.guardar(vieja, ed) }
            }
            adaptadas[vieja] = nil
            leidos[vieja] = nil
        }
    }

    /// La tinta de la hoja `i`, abriendola ya si hace falta (al empezar a
    /// anotar en ella).
    private func editor(_ i: Int, pagina: PDFPage) -> EditorLienzo? {
        if let ed = editores[i] { return ed }
        do {
            let ed = try nucleo.anotarHoja(documento: url.path, pagina: UInt32(i), alto: alto(pagina))
            guardarEditor(ed, en: i)
            return ed
        } catch {
            modelo.aviso = "No se puede anotar aquí: \(error)"
            return nil
        }
    }

    // MARK: Pintar

    /// El tono de las hojas: el modo noche comun de los visores.
    var tono: TonoHojas { TonoHojas(negativo: false, oscuro: ModoNoche.compartido.activo) }

    /// Pinta la hoja `i` en su capa: el papel de los espacios y la tinta.
    /// `pagina`: la hoja dentro de la capa.
    func pintar(_ i: Int, en ctx: CGContext, pagina: CGRect, sucio: CGRect) {
        let g = GeometriaHoja(rect: pagina)
        let papel = tono.papel
        let (izq, der) = g.espacios(modelo.espacios)
        ctx.setFillColor(papel.map { $0.cg.copy(alpha: 0.92) ?? $0.cg } ?? UIColor(white: 1, alpha: 0.92).cgColor)
        for r in [izq, der].compactMap({ $0 }) where r.intersects(sucio) {
            ctx.fill(r)
        }
        defer { pintarMarcos(i, en: ctx, pagina: pagina, sucio: sucio) }
        guard let ed = editores[i] else { return }
        let v = ed.version()
        if leidos[i]?.version != v, let l = try? leerLienzo(ed.plano()) {
            leidos[i] = (v, l)
        }
        guard var l = leidos[i]?.lienzo, !l.figuras.isEmpty else { return }
        // Sobre papel oscuro la tinta se adapta, como en Windows.
        if let papel {
            if let a = adaptadas[i], a.version == v {
                l = a.lienzo
            } else {
                l = l.sobrePapel(papel, cache: &coloresAdaptados)
                adaptadas[i] = (v, l)
            }
        }
        ctx.saveGState()
        ctx.concatenate(g.transformacion)
        let k = max(g.escala, 0.0001)
        let visible = CGRect(x: (sucio.minX - g.rect.minX) / k, y: (sucio.minY - g.rect.minY) / k,
                             width: sucio.width / k, height: sucio.height / k)
        PintorTintaHoja(lienzo: l).pintar(en: ctx, limites: visible)
        ctx.restoreGState()
    }

    private func ponerFondo() {
        vistaPDF.backgroundColor = ModoNoche.compartido.activo
            ? UIColor(red: 0.055, green: 0.055, blue: 0.063, alpha: 1)
            : UIColor(red: 0.11, green: 0.11, blue: 0.118, alpha: 1)
    }

    /// **El modo noche** (luna/sol de la barra): PDFKit guarda lo ya pintado
    /// de cada hoja, asi que se le vuelve a dar el documento en el mismo
    /// sitio y al mismo aumento.
    func aplicarNoche() {
        ponerFondo()
        adaptadas = [:]
        coloresAdaptados = [:]
        guard let doc = vistaPDF.document else { return }
        TonosDeDocumentos.compartidos.poner(tono, a: doc)
        let destino = vistaPDF.currentDestination
        let auto = vistaPDF.autoScales, escala = vistaPDF.scaleFactor
        hojas = [:]
        vistaPDF.document = nil
        vistaPDF.document = doc
        vistaPDF.autoScales = auto
        if !auto { vistaPDF.scaleFactor = escala }
        if let destino { vistaPDF.go(to: destino) }
        ponerMargenes()
        actualizarGestos()
    }

    // MARK: Anotar

    private func aplicarHerramienta(_ ed: EditorLienzo) {
        try? ed.herramienta(nombre: modelo.anotando ? modelo.herramienta : "lapiz")
        let c = modelo.color
        ed.tintaColor(r: c.r, g: c.g, b: c.b, a: 1)
    }

    /// **El lapiz del riel**: anotar con la ultima herramienta, o volver a leer.
    func alternarAnotar() {
        elegir(modelo.anotando ? "leer" : ultimaHerramienta)
    }

    func elegir(_ h: String) {
        if h == "lapiz" || h == "resaltador" { ultimaHerramienta = h }
        modelo.herramienta = h
        if h == "resaltador", modelo.color == ColorTinta.todos[0] {
            modelo.color = ColorTinta.todos[4]
        } else if h == "lapiz", modelo.color == ColorTinta.todos[4] {
            modelo.color = ColorTinta.todos[0]
        }
        for ed in editores.values { aplicarHerramienta(ed) }
        actualizarGestos()
    }

    func ponerColor(_ c: ColorTinta) {
        modelo.color = c
        if !modelo.anotando || modelo.herramienta == "borrador" { modelo.herramienta = ultimaHerramienta }
        for ed in editores.values { aplicarHerramienta(ed) }
        actualizarGestos()
    }

    func alternarDedo() {
        modelo.conDedo.toggle()
        actualizarGestos()
    }

    /// Quien se lleva cada toque: el lapiz siempre pinta; el dedo desplaza
    /// (o pinta con «Dibujar con el dedo», y se desplaza con dos).
    private func actualizarGestos() {
        var tipos: [UITouch.TouchType] = [.pencil]
        if marcos.activo {
            // Poniendo marcos: el lapiz, el dedo y el raton dibujan el marco
            // (se desplaza con dos dedos).
            tipos += [.direct, .indirectPointer]
        } else if modelo.anotando {
            tipos.append(.indirectPointer)
            if modelo.conDedo { tipos.append(.direct) }
        }
        gesto.allowedTouchTypes = tipos.map { NSNumber(value: $0.rawValue) }
        if let desplazar = ControladorLectorIPad.desplazador(en: vistaPDF) {
            desplazar.panGestureRecognizer.allowedTouchTypes = [
                NSNumber(value: UITouch.TouchType.direct.rawValue),
                NSNumber(value: UITouch.TouchType.indirect.rawValue),
                NSNumber(value: UITouch.TouchType.indirectPointer.rawValue),
            ]
            desplazar.panGestureRecognizer.minimumNumberOfTouches =
                marcos.activo || (modelo.anotando && modelo.conDedo) ? 2 : 1
        }
    }

    /// La vista que desplaza las hojas dentro de PDFKit.
    private static func desplazador(en v: UIView) -> UIScrollView? {
        for s in v.subviews {
            if let d = s as? UIScrollView { return d }
            if let d = desplazador(en: s) { return d }
        }
        return nil
    }

    /// Si un toque es para la tinta. Apoyar el lapiz leyendo empieza a
    /// anotar con la ultima herramienta.
    func aceptar(_ t: UITouch) -> Bool {
        if marcos.activo { return true }
        switch t.type {
        case .pencil:
            if !modelo.anotando { elegir(ultimaHerramienta) }
            return true
        case .direct:
            return modelo.anotando && modelo.conDedo
        default:
            return modelo.anotando
        }
    }

    func trazo(_ fase: FasePuntero, _ toques: [UITouch], vista: UIView?) {
        guard let primero = toques.first else { return }
        if marcos.activo || inicioMarco != nil {
            trazoDeMarco(fase, primero)
            return
        }
        if fase == .pulsar {
            let p = primero.location(in: vistaPDF)
            guard let pagina = vistaPDF.page(for: p, nearest: true) else { return }
            hojaDelTrazo = indice(pagina)
            ultimoPunto = nil
            modelo.poniendoMarca = false
        }
        guard let i = hojaDelTrazo, let pagina = vistaPDF.document?.page(at: i),
              let ed = editor(i, pagina: pagina) else { return }
        let g = geometria(pagina)
        var sucio = ultimoPunto.map { CGRect(origin: $0, size: .zero) } ?? .null
        for t in toques {
            let p = t.preciseLocation(in: vistaPDF)
            let q = g.aHoja(p)
            let presion: Float? = t.type == .pencil && t.maximumPossibleForce > 0
                ? Float(min(max(t.force / t.maximumPossibleForce, 0), 1)) : nil
            _ = ed.puntero(fase: fase, x: Float(q.x), y: Float(q.y), presion: presion, modificadores: 0,
                           zoom: Float(g.escala))
            sucio = sucio.union(CGRect(origin: p, size: .zero))
            ultimoPunto = p
        }
        if fase == .soltar || fase == .cancelar {
            conTinta.insert(i)
            ultimaAnotada = i
            hojaDelTrazo = nil
            ultimoPunto = nil
            usar(i)
            programarGuardado()
            modelo.puedeDeshacer = true
            hojas[i]?.repintar()
        } else if modelo.herramienta == "lapiz" || modelo.herramienta == "resaltador" {
            // Solo el trocito nuevo (con margen para el grosor).
            let margen = max(40, 60 * g.escala)
            hojas[i]?.repintar(sucio.insetBy(dx: -margen, dy: -margen), desde: vistaPDF)
        } else {
            hojas[i]?.repintar()
        }
    }

    func deshacer() {
        guard let i = ultimaAnotada, let ed = editores[i] else { return }
        if ed.deshacer() {
            hojas[i]?.repintar()
            programarGuardado()
        }
        modelo.puedeDeshacer = ed.puedeDeshacer()
    }

    func rehacer() {
        guard let i = ultimaAnotada, let ed = editores[i] else { return }
        if ed.rehacer() {
            hojas[i]?.repintar()
            programarGuardado()
            modelo.puedeDeshacer = true
        }
    }

    // MARK: Guardar

    private func programarGuardado(espera: TimeInterval = 1.0) {
        guardadoPendiente?.cancel()
        let pares = editores.map { ($0.key, $0.value) }
        let ruta = url.path, alDia = self.alDia
        let tarea = DispatchWorkItem { [weak self] in
            let juntas = ControladorLectorIPad.guardarTinta(pares, ruta: ruta, alDia: alDia)
            guard !juntas.isEmpty else { return }
            DispatchQueue.main.async { MainActor.assumeIsolated { self?.releer(juntas) } }
        }
        guardadoPendiente = tarea
        colaGuardar.asyncAfter(deadline: .now() + espera, execute: tarea)
    }

    /// Guarda la tinta con cambios (en `colaGuardar`). Devuelve las hojas
    /// que se juntaron con lo que cambio en el disco: hay que releerlas.
    private nonisolated static func guardarTinta(_ editores: [(Int, EditorLienzo)], ruta: String,
                                                 alDia: AlDiaLector) -> [Int] {
        var alguno = false
        var juntas: [Int] = []
        for (i, ed) in editores where ed.sinGuardar() {
            do {
                let r = try alDia.guardar(i, ed)
                alguno = r.escrito || alguno
                if r.junto { juntas.append(i) }
            } catch {
                NSLog("PixPin: no se pudo guardar la tinta: %@", "\(error)")
            }
        }
        if alguno {
            DispatchQueue.main.async {
                NotificationCenter.default.post(name: .pixpinDocumentoAnotado, object: ruta)
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            }
        }
        return juntas
    }

    /// Por donde se va: la hoja de arriba de lo que se ve y en que fraccion
    /// de ella (`2.25`, un cuarto de la tercera).
    private func sitioDeLaVista() -> Double {
        let arriba = CGPoint(x: vistaPDF.bounds.midX, y: vistaPDF.bounds.minY + 60)
        guard let p = vistaPDF.page(for: arriba, nearest: true) else { return Double(modelo.pagina) }
        let g = geometria(p)
        let f = (arriba.y - g.rect.minY) / max(g.rect.height, 1)
        return Double(indice(p)) + Double(min(max(f, 0), 0.999))
    }

    /// El alto de cada hoja en unidades de la tinta (lo pide el marco).
    private func altos() -> [Float] {
        guard let doc = vistaPDF.document else { return [] }
        return (0..<doc.pageCount).compactMap { doc.page(at: $0).map(alto) }
    }

    private func programarLectura() {
        lecturaPendiente?.cancel()
        let tarea = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated { self?.guardarLectura() }
        }
        lecturaPendiente = tarea
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5, execute: tarea)
    }

    /// Lo recordado (aumento, por donde iba, espacios y marcadores): el
    /// `.pixpin-lectura` de Windows, fuera del hilo principal.
    private func guardarLectura() {
        guard let doc = vistaPDF.document, doc.pageCount > 0 else { return }
        let ajuste = vistaPDF.scaleFactorForSizeToFit
        let zoom = vistaPDF.autoScales || ajuste <= 0 ? 1 : Float(vistaPDF.scaleFactor / ajuste)
        let lectura = LecturaDocumento(zoom: zoom, pagina: sitioDeLaVista(), espacios: modelo.espacios,
                                       marcas: modelo.marcas, donde: donde)
        let altos = self.altos()
        let ruta = url.path, nucleo = self.nucleo
        colaGuardar.async {
            try? nucleo.guardarLecturaDeDocumento(documento: ruta, lectura: lectura, altos: altos)
        }
    }

    // MARK: Lo que cambia en el disco

    private func empezarAMirarElDisco() {
        guard mirarDisco == nil, !cerrado else { return }
        let r = Timer(timeInterval: 1.5, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.mirarElDisco() }
        }
        RunLoop.main.add(r, forMode: .common)
        mirarDisco = r
    }

    /// Mira (en la cola de guardar) si la tinta de alguna hoja abierta
    /// cambio en el disco; las que no tienen nada a medias se releen.
    private func mirarElDisco() {
        guard !editores.isEmpty else { return }
        let abiertas = editores.mapValues { $0.ruta() }
        let alDia = self.alDia
        colaGuardar.async { [weak self] in
            let cambiadas = alDia.cambiadas(abiertas)
            guard !cambiadas.isEmpty else { return }
            DispatchQueue.main.async { MainActor.assumeIsolated { self?.releer(cambiadas) } }
        }
    }

    /// Relee la tinta de las hojas `hojas` si aqui no hay nada a medias.
    private func releer(_ lista: [Int]) {
        for i in lista {
            guard let viejo = editores[i], !viejo.sinGuardar(), hojaDelTrazo != i, !releyendo.contains(i),
                  let pagina = vistaPDF.document?.page(at: i) else { continue }
            releyendo.insert(i)
            let ruta = url.path, h = alto(pagina), nucleo = self.nucleo, rutaTinta = viejo.ruta()
            let cola = colaGuardar
            Task {
                let (visto, ed) = await withCheckedContinuation { cont in
                    cola.async {
                        let v = AlDiaLector.ver(rutaTinta)
                        cont.resume(returning: (v, try? nucleo.anotarHoja(documento: ruta, pagina: UInt32(i), alto: h)))
                    }
                }
                self.releyendo.remove(i)
                guard let ed, self.editores[i] === viejo, !viejo.sinGuardar(), self.hojaDelTrazo != i else { return }
                self.editores[i] = ed
                self.aplicarHerramienta(ed)
                self.leidos[i] = nil
                self.adaptadas[i] = nil
                let alDia = self.alDia
                cola.async { alDia.apuntar(i, visto) }
                self.hojas[i]?.repintar()
            }
        }
    }

    // MARK: Espacios y marcadores

    private func ponerMargenes() {
        guard let p = vistaPDF.document?.page(at: 0) else { return }
        let ancho = p.bounds(for: vistaPDF.displayBox).width * 0.75
        let izq: CGFloat = modelo.espacios & 1 != 0 ? ancho : 4
        let der: CGFloat = modelo.espacios & 2 != 0 ? ancho : 4
        vistaPDF.pageBreakMargins = UIEdgeInsets(top: 4, left: izq, bottom: 4, right: der)
        vistaPDF.layoutDocumentView()
        for h in hojas.values {
            h.setNeedsLayout()
            h.repintar()
        }
    }

    /// Un espacio para anotar a un lado (1 izquierda, 2 derecha).
    func alternarEspacio(_ lado: UInt8) {
        // Antes de cambiarlos, Rust apunta el marco de la tinta de antes con
        // los espacios de ahora (`fijar_lo_viejo`): se guarda ya.
        guardarLectura()
        modelo.espacios = (modelo.espacios ^ lado) & 3
        ponerMargenes()
        guardarLectura()
    }

    func ponerMarca(emoji: String) {
        guard let p = vistaPDF.currentPage else { return }
        let g = geometria(p)
        let centro = CGPoint(x: g.rect.midX, y: max(vistaPDF.bounds.minY + 80, g.rect.minY))
        let (x, y) = g.sitio(de: centro, pagina: indice(p))
        modelo.marcas = conMarca(modelo.marcas, x: x, y: y, emoji: emoji,
                                 ahora: Int64(Date().timeIntervalSince1970 * 1000))
        modelo.poniendoMarca = false
        guardarLectura()
    }

    /// El verde de la voz: donde se dejo de escuchar (uno por documento).
    private func ponerMarcaDeVoz(pagina: Int, altura: Double) {
        let sin = modelo.marcas.filter { $0.emoji != VozAlta.emojiDeVoz }
        modelo.marcas = conMarca(sin, x: 0.5, y: Double(pagina) + min(max(altura, 0), 0.999),
                                 emoji: VozAlta.emojiDeVoz, ahora: Int64(Date().timeIntervalSince1970 * 1000))
        guardarLectura()
    }

    func ir(a m: MarcaDocumento) {
        guard let doc = vistaPDF.document, let p = doc.page(at: Int(m.y.rounded(.down))) else { return }
        let b = p.bounds(for: vistaPDF.displayBox)
        let f = CGFloat(m.y - m.y.rounded(.down))
        vistaPDF.go(to: CGRect(x: b.minX, y: b.maxY - f * b.height - 40, width: 1, height: 40), on: p)
    }

    func quitarMarca(_ m: MarcaDocumento) {
        modelo.marcas.removeAll { $0.id == m.id }
        guardarLectura()
    }

    func quitarMarcas() {
        modelo.marcas = []
        guardarLectura()
    }

    // MARK: Ir, buscar, voz

    func irAPagina(_ texto: String) {
        guard let n = Int(texto.trimmingCharacters(in: .whitespaces)), let doc = vistaPDF.document,
              let p = doc.page(at: max(0, min(n - 1, doc.pageCount - 1))) else { return }
        vistaPDF.go(to: p)
    }

    func buscar() {
        let buscador: UIFindInteraction? = vistaPDF.findInteraction
        buscador?.presentFindNavigator(showingReplace: false)
    }

    func escuchar() {
        voz.alternar(doc: vistaPDF.document, pagina: modelo.pagina)
    }
}

// MARK: - Marcos, desplazar sola, indice, compartir, imprimir y el nombre

extension ControladorLectorIPad {
    private var hojasDelLector: HojasDelLector? {
        vistaPDF.document.map { HojasDelLector($0, caja: vistaPDF.displayBox) }
    }

    /// Los marcos de la hoja `i`, encima de su tinta.
    fileprivate func pintarMarcos(_ i: Int, en ctx: CGContext, pagina: CGRect, sucio: CGRect) {
        guard !marcos.lista.isEmpty, let h = hojasDelLector else { return }
        let vista = hojas[i]
        let k = vista.map { v in v.bounds.width > 0 ? v.convert(v.bounds, to: nil).width / v.bounds.width : 1 } ?? 1
        for (n, m) in marcos.lista.enumerated() {
            guard let r = h.rect(m, enHoja: i, vista: pagina), r.insetBy(dx: -24, dy: -24).intersects(sucio) else {
                continue
            }
            PintarMarcos.pintar(r, numero: n + 1, en: ctx, k: k)
        }
    }

    /// El marco que se arrastra (en la vista del PDF) y, al soltar, el
    /// marco en las unidades del lector.
    fileprivate func trazoDeMarco(_ fase: FasePuntero, _ t: UITouch) {
        let p = t.location(in: vistaPDF)
        switch fase {
        case .pulsar:
            inicioMarco = p
            goma.rect = nil
        case .mover:
            guard let a = inicioMarco else { return }
            goma.rect = CGRect(x: min(a.x, p.x), y: min(a.y, p.y), width: abs(a.x - p.x), height: abs(a.y - p.y))
        case .soltar:
            defer {
                inicioMarco = nil
                goma.rect = nil
            }
            // Menos de 8 puntos de ancho o de alto es un toque.
            guard let a = inicioMarco, abs(a.x - p.x) > 8, abs(a.y - p.y) > 8, let h = hojasDelLector,
                  let ua = unidades(a, h), let ub = unidades(p, h) else { return }
            marcos.poner(CGRect(x: min(ua.x, ub.x), y: min(ua.y, ub.y), width: abs(ua.x - ub.x),
                                height: abs(ua.y - ub.y)), parte: 0)
        default:
            inicioMarco = nil
            goma.rect = nil
        }
    }

    private func unidades(_ p: CGPoint, _ h: HojasDelLector) -> CGPoint? {
        guard let doc = vistaPDF.document, let hoja = vistaPDF.page(for: p, nearest: true) else { return nil }
        return h.unidades(vistaPDF.convert(p, to: hoja), hoja: doc.index(for: hoja),
                          bounds: hoja.bounds(for: vistaPDF.displayBox))
    }

    /// El boton del marco del riel (o la I): poner marcos numerados.
    func alternarMarcos() {
        if modelo.anotando { elegir("leer") }
        marcos.activo.toggle()
        modelo.poniendoMarca = false
        actualizarGestos()
    }

    /// Una ficha: ir al marco `n`.
    func irAMarco(_ n: Int) {
        guard n >= 0, n < marcos.lista.count, let doc = vistaPDF.document, let h = hojasDelLector else { return }
        let m = marcos.lista[n]
        let i = h.hoja(de: CGFloat(m.caja[1]))
        guard let p = doc.page(at: i) else { return }
        let b = p.bounds(for: vistaPDF.displayBox)
        let a = h.punto(CGPoint(x: m.caja[0], y: m.caja[1]), bounds: b, hoja: i)
        let z = h.punto(CGPoint(x: m.caja[2], y: m.caja[3]), bounds: b, hoja: i)
        vistaPDF.go(to: CGRect(x: min(a.x, z.x), y: min(a.y, z.y), width: abs(a.x - z.x), height: abs(a.y - z.y)), on: p)
    }

    // MARK: Desplazar sola

    /// D: pone o quita la barra de la pagina que sube sola.
    func alternarAvance() {
        if avance.activo {
            avance.activo = false
            reloj.parar()
            return
        }
        guard let doc = vistaPDF.document, doc.pageCount > 0 else { return }
        avance.ppm = nucleo.palabrasPorMinuto()
        avance.enMarcha = true
        avance.antes = nil
        avance.activo = true
        avance.quedan = "Contando las palabras…"
        if avance.palabras == nil { contarPalabrasDelDocumento(doc) }
        reloj.empezar()
    }

    /// Las palabras del documento, fuera del hilo principal. Un escaneo
    /// cuenta 250 por hoja, lo de una pagina corriente.
    private func contarPalabrasDelDocumento(_ doc: PDFDocument) {
        let hojas = doc.pageCount
        let deOffice = TiposDeDocumento.clase(de: url) == .officePaginado
        let texto = deOffice ? (doc.string ?? "") : nil
        let url = self.url
        Task {
            let n = await Task.detached(priority: .utility) { () -> Int in
                let t = texto ?? PDFDocument(url: url)?.string ?? ""
                return Int(contarPalabras(texto: t))
            }.value
            self.avance.palabras = n > 0 ? n : hojas * 250
            self.reloj.reponerQuedan()
        }
    }

    func cambiarVelocidad(_ sentido: Int32) {
        avance.ppm = otraVelocidadDeLeer(ppm: avance.ppm, sentido: sentido)
        reloj.reponerQuedan()
        let ppm = avance.ppm, nucleo = self.nucleo
        colaGuardar.async { try? nucleo.guardarPalabrasPorMinuto(ppm: ppm) }
    }

    // MARK: Indice

    var indice: [EntradaIndice] { EntradaIndice.de(vistaPDF.document) }

    func ir(a e: EntradaIndice) {
        guard let p = vistaPDF.document?.page(at: e.destino) else { return }
        vistaPDF.go(to: p)
    }

    // MARK: Compartir e imprimir

    private var hayTinta: Bool { !conTinta.isEmpty || !editores.isEmpty }

    /// **El PDF con la tinta quemada** (en una carpeta temporal).
    func pdfConTinta() -> URL? {
        guard let doc = vistaPDF.document else { return nil }
        let (nucleo, ruta) = (self.nucleo, url.path)
        return SalidaDocumentoIPad.pdfConTinta(doc, nombre: url.deletingPathExtension().lastPathComponent) { i, p in
            let ed = self.editores[i] ?? (self.conTinta.contains(i)
                ? try? nucleo.anotarHoja(documento: ruta, pagina: UInt32(i), alto: self.alto(p)) : nil)
            return ed.flatMap { try? leerLienzo($0.plano()) }
        }
    }

    /// ⇧⌘S: el original y, si tiene tinta, «<nombre> (anotado).pdf».
    func compartir() {
        var cosas: [Any] = [url]
        let anotado = hayTinta ? pdfConTinta() : nil
        if let anotado { cosas.append(anotado) }
        SalidaDocumentoIPad.compartir(cosas, desde: vistaPDF) { SalidaDocumentoIPad.olvidar(anotado) }
    }

    /// ⌘P: con lo anotado encima. Con marcos, se imprimen ellos en grupo
    /// (una hoja por marco), como Windows.
    func imprimir() {
        guard let doc = vistaPDF.document else { return }
        let titulo = url.deletingPathExtension().lastPathComponent
        let anotado = hayTinta ? pdfConTinta() : nil
        defer { SalidaDocumentoIPad.olvidar(anotado) }
        if !marcos.lista.isEmpty {
            let fuente = anotado.flatMap { PDFDocument(url: $0) } ?? doc
            guard let d = PintarMarcos.pdf(marcos.lista, documento: fuente) else { return }
            SalidaDocumentoIPad.imprimir(d, titulo: titulo, desde: vistaPDF)
            return
        }
        if let anotado, let d = try? Data(contentsOf: anotado) {
            SalidaDocumentoIPad.imprimir(d, titulo: titulo, desde: vistaPDF)
        } else if TiposDeDocumento.clase(de: url) == .pdf {
            SalidaDocumentoIPad.imprimir(url, titulo: titulo, desde: vistaPDF)
        } else if let d = doc.dataRepresentation() {
            SalidaDocumentoIPad.imprimir(d, titulo: titulo, desde: vistaPDF)
        }
    }

    /// «Aligerar el PDF» a mano: va delante, con el nivel de los ajustes.
    func aligerar() {
        avisar("Aligerando…")
        ColaAligerar.compartida.aMano(url, nucleo: nucleo)
        let ruta = url.path
        Task { [weak self] in
            // Lo que no cambio el fichero tambien se dice.
            while ColaAligerar.compartida.pendiente(URL(fileURLWithPath: ruta)) {
                try? await Task.sleep(nanoseconds: 300_000_000)
            }
            guard let self, let r = ColaAligerar.compartida.resultado[ruta] else { return }
            self.avisar(r)
        }
    }

    // MARK: Cambiar el nombre (F2)

    func empezarRenombrar() {
        modelo.nombreNuevo = (modelo.nombre as NSString).deletingPathExtension
        modelo.renombrando = true
    }

    /// La tinta se guarda antes: un documento suelto se mueve con sus
    /// hermanos y la tinta abierta va con el.
    func confirmarNombre(_ escrito: String) {
        modelo.renombrando = false
        guardadoPendiente?.cancel()
        let pares = editores.map { ($0.key, $0.value) }
        let ruta = url.path, alDia = self.alDia, nucleo = self.nucleo
        let cola = colaGuardar
        Task {
            let r: DocumentoRenombrado? = await withCheckedContinuation { cont in
                cola.async {
                    _ = ControladorLectorIPad.guardarTinta(pares, ruta: ruta, alDia: alDia)
                    cont.resume(returning: try? nucleo.renombrarDocumento(documento: ruta, escrito: escrito))
                }
            }
            self.renombrado(r)
        }
    }

    private func renombrado(_ r: DocumentoRenombrado?) {
        let sinExtension = { (n: String) in (n as NSString).deletingPathExtension }
        switch r {
        case .sinCambios:
            return
        case let .enElChat(nombre):
            modelo.nombre = nombre
            avisar("Ahora se llama «\(sinExtension(nombre))»")
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        case let .enDisco(nombre, ruta):
            url = URL(fileURLWithPath: ruta)
            modelo.nombre = nombre
            // La tinta abierta era de la ruta de antes: se vuelve a abrir.
            editores = [:]
            leidos = [:]
            adaptadas = [:]
            usadas = []
            let alDia = self.alDia
            colaGuardar.async { alDia.olvidar() }
            marcos.mudar(a: url, nucleo: nucleo)
            if let n = vistaPDF.document?.pageCount {
                let nucleo = self.nucleo
                Task {
                    let hojas = await Task.detached { nucleo.hojasConTinta(documento: ruta, paginas: UInt32(n)) }.value
                    self.conTinta = Set(hojas.map { Int($0) })
                    self.cargarVisibles()
                    self.repintarHojas()
                }
            }
            avisar("Ahora se llama «\(sinExtension(nombre))»")
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        case let .yaExiste(nombre):
            avisar("Ya hay un archivo que se llama «\(nombre)»")
        case .noSePuede, nil:
            avisar("No se pudo cambiar el nombre")
        }
    }
}

// MARK: - PDFKit y el Apple Pencil

extension ControladorLectorIPad: @preconcurrency PDFPageOverlayViewProvider {
    func pdfView(_ view: PDFView, overlayViewFor page: PDFPage) -> UIView? {
        let i = indice(page)
        if let h = hojas[i] { return h }
        let h = HojaTintaIPad(controlador: self, indice: i)
        hojas[i] = h
        return h
    }

    func pdfView(_ pdfView: PDFView, willEndDisplayingOverlayView overlayView: UIView, for page: PDFPage) {
        guard let h = overlayView as? HojaTintaIPad, hojas[h.indice] === h else { return }
        hojas[h.indice] = nil
    }
}

extension ControladorLectorIPad: UIPencilInteractionDelegate {
    /// El doble toque (o apretar) del lapiz: la goma y vuelta, o lo que
    /// pida Ajustes del iPad.
    func pencilInteraction(_ interaction: UIPencilInteraction, didReceiveTap tap: UIPencilInteraction.Tap) {
        switch UIPencilInteraction.preferredTapAction {
        case .ignore:
            return
        default:
            elegir(modelo.herramienta == "borrador" ? ultimaHerramienta : "borrador")
        }
    }
}

extension ControladorLectorIPad: UIGestureRecognizerDelegate {
    /// La tinta no se comparte con el desplazamiento de PDFKit.
    func gestureRecognizer(_ g: UIGestureRecognizer,
                           shouldRecognizeSimultaneouslyWith otro: UIGestureRecognizer) -> Bool {
        false
    }
}

// MARK: - SwiftUI

/// El `PDFView` del controlador.
struct VistaPDFIPad: UIViewRepresentable {
    let controlador: ControladorLectorIPad

    func makeUIView(context: Context) -> PDFView { controlador.vistaPDF }
    func updateUIView(_ v: PDFView, context: Context) {}
}

/// La tira de miniaturas (abajo).
struct MiniaturasPDFIPad: UIViewRepresentable {
    let controlador: ControladorLectorIPad

    func makeUIView(context: Context) -> PDFThumbnailView { controlador.miniaturas }
    func updateUIView(_ v: PDFThumbnailView, context: Context) {}
}

/// **El lector entero**: el documento, la barra comun arriba y el riel a la
/// derecha, como el pin del lector de Windows.
struct LectorPDFIPad: View {
    @StateObject private var c: ControladorLectorIPad

    init(url: URL, titulo: String, nucleo: Nucleo) {
        _c = StateObject(wrappedValue: ControladorLectorIPad(url: url, nucleo: nucleo, titulo: titulo))
    }

    var body: some View {
        CuerpoLectorIPad(c: c, modelo: c.modelo, voz: c.voz, marcos: c.marcos, avance: c.avance)
    }
}

private struct CuerpoLectorIPad: View {
    let c: ControladorLectorIPad
    @ObservedObject var modelo: ModeloLectorIPad
    @ObservedObject var voz: VozDocumentoIPad
    @ObservedObject var marcos: MarcosDoc
    @ObservedObject var avance: AvanceLectorIPad
    @ObservedObject private var noche = ModoNoche.compartido

    private var icono: String {
        switch c.url.pathExtension.lowercased() {
        case "pptx", "pptm", "potx", "potm", "ppsx", "ppsm": "rectangle.on.rectangle"
        case "pdf": "doc.richtext"
        default: "doc.text"
        }
    }

    private var tipo: String {
        switch c.url.pathExtension.lowercased() {
        case "pptx", "pptm", "potx", "potm", "ppsx", "ppsm": "PowerPoint"
        case "pdf": "PDF"
        default: "Word"
        }
    }

    var body: some View {
        ZStack(alignment: .top) {
            Color(uiColor: .systemBackground).ignoresSafeArea()
            VistaPDFIPad(controlador: c)
                .ignoresSafeArea(edges: .bottom)
                .padding(.top, modelo.completa ? 0 : 58)
            if modelo.completa {
                HStack {
                    Spacer()
                    Button { modelo.completa = false } label: {
                        Image(systemName: "arrow.down.right.and.arrow.up.left")
                            .padding(10)
                            .background(.regularMaterial, in: Circle())
                    }
                    .help("Salir de pantalla completa")
                    .padding(.trailing, 14)
                    .padding(.top, 8)
                }
            } else {
                barra
                HStack(alignment: .top) {
                    Spacer()
                    RielLectorIPad(c: c, modelo: modelo, marcos: marcos)
                        .padding(.trailing, 10)
                }
                .padding(.top, 70)
            }
            if let nota = modelo.nota {
                Text(nota)
                    .font(.system(size: 14))
                    .foregroundStyle(Color(red: 0.96, green: 0.96, blue: 0.97))
                    .padding(.horizontal, 14)
                    .frame(height: 36)
                    .background(Color(red: 0.118, green: 0.118, blue: 0.125, opacity: 0.95),
                                in: RoundedRectangle(cornerRadius: 12))
                    .padding(.top, modelo.completa ? 12 : 66)
                    .transition(.opacity)
                    .allowsHitTesting(false)
            }
            VStack(spacing: 8) {
                Spacer()
                BarraAvanceIPad(avance: c.avance, cambiar: { c.cambiarVelocidad($0) },
                                cerrar: { c.alternarAvance() })
                if modelo.poniendoMarca {
                    TiraEmojisMarca { e in c.ponerMarca(emoji: e) } cerrar: { modelo.poniendoMarca = false }
                }
                if voz.vivo {
                    BarraVozIPad(voz: voz)
                }
                if let aviso = modelo.aviso {
                    Text(aviso).font(.callout).padding(.horizontal, 12).padding(.vertical, 6)
                        .background(Paleta.v2.rojo.opacity(0.25), in: Capsule())
                        .onTapGesture { modelo.aviso = nil }
                } else if modelo.maquetando {
                    Label("Maquetando…", systemImage: "hourglass").font(.caption)
                        .padding(.horizontal, 12).padding(.vertical, 6)
                        .background(.regularMaterial, in: Capsule())
                }
                if modelo.miniaturas, !modelo.completa {
                    MiniaturasPDFIPad(controlador: c)
                        .frame(height: 76)
                        .clipShape(RoundedRectangle(cornerRadius: 10))
                        .padding(.horizontal, 12)
                }
            }
            .padding(.bottom, 12)
            VStack {
                Spacer()
                HStack {
                    FichasDeMarcos(marcos: marcos, ir: { c.irAMarco($0) }, imprimir: { c.imprimir() })
                    Spacer()
                }
            }
            .padding(.leading, 14)
            .padding(.bottom, modelo.miniaturas && !modelo.completa ? 100 : 14)
        }
        .statusBarHidden(modelo.completa)
        .sheet(isPresented: $modelo.viendoIndice) {
            HojaIndiceIPad(entradas: c.indice, actual: modelo.pagina, ir: { c.ir(a: $0) })
        }
        .alert("Cambiar el nombre", isPresented: $modelo.renombrando) {
            TextField("Nombre", text: $modelo.nombreNuevo)
            Button("Guardar") { c.confirmarNombre(modelo.nombreNuevo) }
            Button("Cancelar", role: .cancel) {}
        } message: {
            Text("La extensión no cambia.")
        }
        .onAppear { c.abrir() }
        .onDisappear { c.cerrar() }
        .onChange(of: noche.activo) { _, _ in c.aplicarNoche() }
        .alert("Ir a la página", isPresented: $modelo.pidiendoPagina) {
            TextField("Página (1–\(max(modelo.paginas, 1)))", text: $modelo.paginaPedida)
                .keyboardType(.numberPad)
            Button("Ir") { c.irAPagina(modelo.paginaPedida) }
            Button("Cancelar", role: .cancel) {}
        }
    }

    private var barra: some View {
        BarraVisorIPad(tipo: tipo, icono: icono, titulo: modelo.titulo) {
            Button { c.buscar() } label: { Image(systemName: "magnifyingglass") }
                .keyboardShortcut("f", modifiers: .command)
                .help("Buscar (⌘F)")
            Button { modelo.miniaturas.toggle() } label: {
                Image(systemName: modelo.miniaturas ? "square.grid.2x2.fill" : "square.grid.2x2")
            }
            .help("Miniaturas")
            Button {
                modelo.paginaPedida = "\(modelo.pagina + 1)"
                modelo.pidiendoPagina = true
            } label: { Image(systemName: "number") }
                .keyboardShortcut("g", modifiers: .command)
                .help("Ir a la página")
            Button { c.escuchar() } label: {
                Image(systemName: voz.leyendo ? "speaker.wave.2.fill" : "speaker.wave.2")
            }
            .help("Leer en voz alta (L)")
            Button { modelo.viendoIndice = true } label: { Image(systemName: "list.bullet.indent") }
                .help("Índice")
            Button { c.alternarAvance() } label: {
                Image(systemName: "chevron.down.2")
                    .foregroundStyle(avance.activo ? Paleta.v2.acento : Color.primary)
            }
            .help("Desplazar sola (D)")
            Button { c.compartir() } label: { Image(systemName: "square.and.arrow.up") }
                .keyboardShortcut("s", modifiers: [.command, .shift])
                .help("Compartir con lo anotado (⇧⌘S)")
            Menu {
                Button { c.imprimir() } label: { Label("Imprimir… (⌘P)", systemImage: "printer") }
                Button { c.alternarMarcos() } label: {
                    Label(marcos.activo ? "Dejar de poner marcos" : "Marcos para imprimir", systemImage: "rectangle.dashed")
                }
                if !marcos.lista.isEmpty {
                    Button(role: .destructive) { marcos.borrar() } label: {
                        Label("Quitar todos los marcos", systemImage: "trash")
                    }
                }
                Divider()
                Button { c.empezarRenombrar() } label: { Label("Cambiar el nombre…", systemImage: "pencil") }
                if TiposDeDocumento.clase(de: c.url) == .pdf {
                    Button { c.aligerar() } label: { Label("Aligerar el PDF", systemImage: "arrow.down.right.and.arrow.up.left.circle") }
                }
            } label: {
                Image(systemName: "ellipsis.circle")
            }
            .help("Más")
            .background {
                // Los atajos del menu (dentro de un `Menu` no valen hasta abrirlo).
                Group {
                    Button("") { c.imprimir() }.keyboardShortcut("p", modifiers: .command)
                    Button("") { c.empezarRenombrar() }.keyboardShortcut("r", modifiers: [.command, .shift])
                }
                .opacity(0)
                .accessibilityHidden(true)
            }
            Button { modelo.completa = true } label: {
                Image(systemName: "arrow.up.left.and.arrow.down.right")
            }
            .help("Pantalla completa")
        }
    }
}

/// Los mandos de la voz, abajo: pausa, trozo anterior y siguiente,
/// velocidad y parar.
private struct BarraVozIPad: View {
    @ObservedObject var voz: VozDocumentoIPad

    var body: some View {
        HStack(spacing: 14) {
            Button { voz.anterior() } label: { Image(systemName: "backward.fill") }
            Button { voz.leyendo ? voz.pausar() : voz.seguir() } label: {
                Image(systemName: voz.leyendo ? "pause.fill" : "play.fill")
            }
            Button { voz.siguiente() } label: { Image(systemName: "forward.fill") }
            Button { voz.otraVelocidad() } label: {
                Text(VozAlta.rotulo(voz.velocidad)).font(.callout.monospacedDigit())
            }
            if !voz.nombreVoz.isEmpty {
                Text(voz.nombreVoz).font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
            Button { voz.parar() } label: { Image(systemName: "xmark") }
        }
        .buttonStyle(.borderless)
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(.regularMaterial, in: Capsule())
    }
}

/// Elegir el emoticono de un marcador nuevo (los de Windows).
struct TiraEmojisMarca: View {
    let elegir: (String) -> Void
    let cerrar: () -> Void

    var body: some View {
        VStack(spacing: 4) {
            Text("Elige un marcador").font(.caption).foregroundStyle(Color(white: 0.78))
            HStack(spacing: 2) {
                ForEach(Array(emojisDeMarcas().enumerated()), id: \.offset) { _, e in
                    Button { elegir(e) } label: {
                        Text(e).font(.system(size: 24)).frame(width: 40, height: 40).contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
                Button(action: cerrar) {
                    Image(systemName: "xmark").frame(width: 32, height: 40)
                }
                .buttonStyle(.plain)
                .foregroundStyle(Color(white: 0.8))
            }
        }
        .padding(8)
        .background(Paleta.v2.fondo.opacity(0.95), in: RoundedRectangle(cornerRadius: 16))
        .environment(\.colorScheme, .dark)
    }
}
