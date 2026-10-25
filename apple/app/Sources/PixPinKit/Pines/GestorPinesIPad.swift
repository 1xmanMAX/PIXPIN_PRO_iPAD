// El gestor de pines del iPad: los crea, los tiene vivos y los guarda. Es la
// API que usan los demas (la captura, los visores con «Fijar como pin», el
// chat con una mini-app):
//
//     GestorPinesIPad.compartido.pinear(imagen: foto)            // UIImage
//     GestorPinesIPad.compartido.pinear(url: fichero)            // foto, PDF, video
//     GestorPinesIPad.compartido.pinear(pdf: fichero, pagina: 3) // PDF por su pagina
//     GestorPinesIPad.compartido.pinearDesdePortapapeles()
//     GestorPinesIPad.compartido.pinearNota(texto: "")
//     GestorPinesIPad.compartido.pinearMiniapp("tareas")
//     GestorPinesIPad.compartido.pinearMiniapp(chat:mensaje:miniapp:documento:)
//     GestorPinesIPad.compartido.mostrarPanelAbiertos()
//     GestorPinesIPad.compartido.devolverUltimoCerrado()
//
// Un pin de PDF lleva todas sus paginas: se pasan desde su barra (◀ 3 / 12 ▶)
// y una pagina (o todas) se puede sacar como pin propio. Al arrastrar un pin
// salen las guias azules de los demas y al soltar se pega a ellas (el iman,
// `GuiasPin.swift`).
//
// iOS no deja dibujar encima de otras apps, asi que los pines flotan **dentro
// de PixPin**: en la capa de pines (`CapaPines`), encima de las secciones y
// de los visores. Un pin de video puede salir de la app con Picture in
// Picture (`VideoPin.swift`).
//
// **Se guardan y vuelven al arrancar** como en Windows y el Mac
// (`AlmacenPines`, el mismo `almacen/indice.json`): cada pin nuevo deja su
// entrada y lo que se mueve, gira o filtra se apunta juntado (medio segundo).
// Cerrar lo marca cerrado sin borrar nada. Las coordenadas son puntos de la
// capa (la ventana de PixPin), desde arriba a la izquierda.
//
// Rendimiento: las fotos se leen con ImageIO ya reducidas al tamano de la
// pantalla y fuera del hilo principal; el PNG de una captura tambien.

import AVKit
import AVFoundation
import PDFKit
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Lo que lleva un pin.
enum ContenidoPinIPad {
    /// Una foto o una captura (la original, ya reducida a la pantalla) y su
    /// fichero si es una foto abierta.
    case imagen(CGImage, fichero: URL?)
    /// Una nota o una mini-app.
    case herramienta(HerramientaPin)
    /// Un video o un audio (con Picture in Picture).
    case video(AVPlayer, fichero: URL, soloAudio: Bool)
}

/// Un pin abierto en la capa.
@MainActor
final class PinIPad: ObservableObject, Identifiable {
    let id: Int
    /// Su entrada en el almacen (nil si aun no la tiene o no se guarda).
    var idAlmacen: UInt64?
    @Published private(set) var contenido: ContenidoPinIPad
    /// La foto que se ve: la original con los filtros y el giro.
    @Published private(set) var vista: CGImage?
    /// El centro en puntos de la capa.
    @Published var centro: CGPoint
    /// El tamano a escala 1 (ya girado).
    @Published var tamanoBase: CGSize
    @Published var escala: CGFloat = 1
    @Published var opacidad: Double = 1
    /// En una burbuja del borde.
    @Published var minimizado = false
    /// Oculto desde el panel «Pines abiertos».
    @Published var oculto = false
    /// El color de su grupo (0-7), o sin grupo.
    @Published var grupo: Int?
    @Published private(set) var filtros = FiltrosPin()
    @Published private(set) var giro = GiroPin()
    private(set) var cerrado = false
    /// Picture in Picture de un pin de video (lo pone su vista).
    weak var pip: AVPictureInPictureController?
    /// Las paginas de un pin de PDF (nil si no es un PDF).
    @Published private(set) var paginas: PaginasPdfPin?
    /// El reproductor flotante: su fila de audios y por cual va (desde 0).
    @Published var pistas: [URL] = []
    @Published var pistaActual = 0
    /// Se va solo al acabar el ultimo audio (el reproductor flotante).
    var cierraAlAcabar = false
    /// Empieza a sonar en cuanto se ve.
    var sonarAlAbrir = false
    /// El PDF abierto, para pasar de pagina sin volver a leerlo.
    var documentoPdf: PDFDocument?

    init(id: Int, contenido: ContenidoPinIPad, centro: CGPoint, tamanoBase: CGSize) {
        self.id = id
        self.contenido = contenido
        self.centro = centro
        self.tamanoBase = tamanoBase
        if case let .imagen(original, _) = contenido { vista = original }
    }

    /// El tamano que se ve.
    var tamano: CGSize {
        CGSize(width: tamanoBase.width * escala, height: tamanoBase.height * escala)
    }

    var esImagen: Bool {
        if case .imagen = contenido { return true }
        return false
    }

    var esHerramienta: Bool {
        if case .herramienta = contenido { return true }
        return false
    }

    /// La foto de siempre (sin filtros ni giro), si es de foto.
    var imagenOriginal: CGImage? {
        if case let .imagen(original, _) = contenido { return original }
        return nil
    }

    /// El texto de una nota o mini-app (lo que se guarda en su `.txt`).
    var textoHerramienta: String? {
        if case let .herramienta(h) = contenido { return h.texto }
        return nil
    }

    var fichero: URL? {
        switch contenido {
        case let .imagen(_, url): url
        case let .video(_, url, _): url
        case .herramienta: nil
        }
    }

    // MARK: Lo que se le hace

    func ponerFiltros(_ f: FiltrosPin) {
        filtros = f
        rehacerVista()
    }

    /// Gira `pasos` cuartos (1 a la derecha, 3 o -1 a la izquierda).
    func girar(_ pasos: Int) {
        guard pasos % 4 != 0, esImagen else { return }
        giro.girar(pasos)
        if abs(pasos) % 2 == 1 {
            tamanoBase = CGSize(width: tamanoBase.height, height: tamanoBase.width)
        }
        rehacerVista()
    }

    func voltear(horizontal: Bool) {
        if horizontal { giro.volteoH.toggle() } else { giro.volteoV.toggle() }
        rehacerVista()
    }

    /// 0 de Windows: sin filtros ni giro.
    func restaurarVista() {
        if giro.cuartos % 2 == 1 {
            tamanoBase = CGSize(width: tamanoBase.height, height: tamanoBase.width)
        }
        filtros = FiltrosPin()
        giro = GiroPin()
        rehacerVista()
    }

    /// Una foto nueva (anotada) en el mismo pin, con su proporcion.
    func cambiarImagen(_ nueva: CGImage) {
        guard case let .imagen(_, url) = contenido else { return }
        let ancho = tamanoBase.width
        contenido = .imagen(nueva, fichero: url)
        let proporcion = CGFloat(nueva.height) / CGFloat(max(nueva.width, 1))
        filtros = FiltrosPin()
        giro = GiroPin()
        tamanoBase = CGSize(width: ancho, height: (ancho * proporcion).rounded())
        rehacerVista()
    }

    /// Otra pagina del PDF en el mismo pin: mismo ancho, su proporcion, y
    /// el giro y los filtros que tenia.
    func ponerPagina(_ nueva: CGImage, paginas p: PaginasPdfPin) {
        guard case let .imagen(_, url) = contenido else { return }
        paginas = p
        contenido = .imagen(nueva, fichero: url)
        let girada = giro.cuartos % 2 == 1
        let ancho = tamanoBase.width
        let proporcion = girada ? CGFloat(nueva.width) / CGFloat(max(nueva.height, 1))
            : CGFloat(nueva.height) / CGFloat(max(nueva.width, 1))
        tamanoBase = CGSize(width: ancho, height: (ancho * proporcion).rounded())
        rehacerVista()
    }

    /// Lo hace PDF (al nacer o al volver).
    func hacerPdf(_ documento: PDFDocument, paginas p: PaginasPdfPin) {
        documentoPdf = documento
        paginas = p
    }

    /// «Devolver el ultimo pin cerrado»: vuelve a estar vivo.
    func reabrir() {
        cerrado = false
    }

    private func rehacerVista() {
        guard case let .imagen(original, _) = contenido else { return }
        vista = ReglasFiltros.aplicar(original, filtros: filtros, giro: giro)
    }

    func marcarCerrado() {
        cerrado = true
        if case let .video(reproductor, _, _) = contenido { reproductor.pause() }
        pip?.stopPictureInPicture()
    }

    // MARK: Almacen

    func guardado(escalaPantalla: CGFloat) -> PinGuardado {
        var g = ReglasCapa.guardado(centro: centro, tamanoBase: tamanoBase, escala: escala,
                                    escalaPantalla: escalaPantalla, opacidad: opacidad)
        g.giro = giro.cuartos
        g.volteoH = giro.volteoH
        g.volteoV = giro.volteoV
        g.gris = filtros.gris
        g.invertido = filtros.invertido
        g.brillo = filtros.brillo
        return g
    }

    /// Lo que trae uno que vuelve (giro, filtros, opacidad). El sitio ya lo
    /// trae al nacer.
    func aplicar(_ g: PinGuardado) {
        opacidad = Double(ReglasPin.limitarOpacidad(CGFloat(g.opacidad) / 100))
        filtros = FiltrosPin(gris: g.gris, invertido: g.invertido, brillo: g.brillo)
        giro = GiroPin(cuartos: ((g.giro % 4) + 4) % 4, volteoH: g.volteoH, volteoV: g.volteoV)
        rehacerVista()
    }

    // MARK: Panel

    var fila: FilaPin {
        let porciento = Int((opacidad * 100).rounded())
        let conOpacidad: (String) -> String = { porciento < 100 ? "\($0) · opacidad \(porciento) %" : $0 }
        let tipo: TipoFilaPin
        let nombre: String
        var detalle: String
        switch contenido {
        case let .imagen(_, url):
            if let paginas {
                tipo = .pdf
                nombre = url?.lastPathComponent ?? "PDF"
                detalle = conOpacidad("PDF · página \(paginas.rotulo)")
            } else {
                tipo = .foto
                nombre = url?.lastPathComponent ?? "Captura"
                detalle = conOpacidad(url == nil ? "Captura" : "Foto")
            }
        case let .video(_, url, audio):
            tipo = audio ? .audio : .video
            nombre = url.lastPathComponent
            detalle = conOpacidad(audio ? "Audio" : "Vídeo")
        case let .herramienta(h):
            if case .nota = h.clase { tipo = .nota } else { tipo = .herramienta }
            nombre = h.nombre
            detalle = conOpacidad(h.detalle)
        }
        if minimizado { detalle = "Minimizado" }
        return FilaPin(id: id, nombre: nombre, detalle: detalle, tipo: tipo, grupo: grupo, oculto: oculto)
    }
}

@MainActor
public final class GestorPinesIPad: ObservableObject {
    public static let compartido = GestorPinesIPad()

    /// Los abiertos, del de mas abajo al de mas arriba.
    @Published private(set) var pines: [PinIPad] = []
    /// El que ensena su barra (el ultimo tocado).
    @Published var elegido: Int?
    /// El panel «Pines abiertos».
    @Published var panelAbierto = false
    @Published var busqueda = ""
    /// «Cerrar todos» se puede deshacer (30 s).
    @Published private(set) var puedeDeshacer = false
    /// Las guias azules mientras se arrastra un pin.
    @Published private(set) var guias: [GuiaPin] = []
    /// El ultimo cerrado (uno a uno), para devolverlo.
    @Published private(set) var ultimoCerrado: PinIPad?
    private var guardadoUltimoCerrado: PinGuardado?

    /// El tamano de la capa (la ventana): alli nacen y alli se quedan.
    var area: CGSize = CGSize(width: 1024, height: 768) {
        didSet {
            areaConocida = true
            if area != oldValue { meterEnArea() }
        }
    }
    /// Hasta que la capa dice su tamano, los que vuelven no se recolocan.
    private var areaConocida = false

    /// El almacen de la app (chats: «Al chat», notas a «Mensajes guardados»).
    private(set) weak var almacenApp: Almacen?
    private var nucleo: Nucleo? { almacenApp?.nucleo }
    /// Los pines guardados (nil sin carpeta de datos: viven en memoria).
    private var almacen: AlmacenPines?
    private var siguienteId = 1
    private var nacidos = 0
    private var apartados: [PinIPad] = []
    private var guardadosApartados: [Int: PinGuardado] = [:]
    private var olvidarApartados: Task<Void, Never>?
    private var porGuardar: [Int: PinIPad] = [:]
    private var porReescribir: [Int: PinIPad] = [:]
    private var tareaGuardar: Task<Void, Never>?
    private var vigias: [NSObjectProtocol] = []

    init() {
        // Al irse a segundo plano lo pendiente se escribe ya (iOS puede
        // cerrar la app sin avisar despues).
        vigias.append(NotificationCenter.default.addObserver(
            forName: UIApplication.willResignActiveNotification, object: nil, queue: .main
        ) { _ in
            MainActor.assumeIsolated { GestorPinesIPad.compartido.guardarPendientes() }
        })
    }

    /// Cuantos pines hay abiertos.
    public var cuantos: Int { pines.count }

    /// Abre el almacen de la carpeta de datos y devuelve a la capa los pines
    /// que estaban abiertos. Una vez (lo llama la capa al aparecer).
    func arrancar(almacen app: Almacen) {
        guard almacenApp == nil else { return }
        almacenApp = app
        let raiz = URL(fileURLWithPath: app.nucleo.raiz(), isDirectory: true)
        almacen = try? AlmacenPines(raiz: raiz)
        AvisosMini.compartido.vigilar(nucleo: app.nucleo)
        CapturadorIPad.compartido.almacen = app
        restaurar()
    }

    // MARK: Pinear

    /// Pinea una foto o una captura de UIKit (con su orientacion).
    public func pinear(imagen: UIImage) {
        guard let cg = ImagenIPad.derecha(imagen) else { return avisarFalloPin() }
        pinear(imagen: cg)
    }

    /// Pinea una imagen ya en memoria: al almacen va entera (un PNG propio)
    /// y en la capa se ve reducida a la pantalla.
    public func pinear(imagen: CGImage, origen: String = "recorte") {
        let lado = ReglasPin.ladoMiniatura(pantalla: area, escala: ArribaUIKit.escalaPantalla)
        let reducida = Miniatura.reducir(imagen, ladoMaximo: lado)
        abrirImagen(reducida, pixeles: CGSize(width: imagen.width, height: imagen.height), fichero: nil,
                    para: .captura(imagen, origen: origen))
    }

    /// Pinea un fichero: foto (reducida), PDF (su primera pagina) o video y
    /// audio. `false` si no es nada que se pueda pinear.
    @discardableResult
    public func pinear(url: URL) -> Bool {
        guard let tipo = TipoContenido.de(url) else { return false }
        switch tipo {
        case .imagen:
            let lado = ReglasPin.ladoMiniatura(pantalla: area, escala: ArribaUIKit.escalaPantalla)
            Task { [weak self] in
                let imagen = await Miniatura.fueraDelPrincipal { Miniatura.de(url: url, ladoMaximo: lado) }
                guard let self else { return }
                guard let imagen else { return avisarFalloPin() }
                self.abrirImagen(imagen, pixeles: nil, fichero: url, para: .foto(url))
            }
        case .pdf:
            return pinear(pdf: url)
        case .video, .audio:
            let audio = tipo == .audio
            let tam = audio ? CGSize(width: 360, height: 112) : CGSize(width: 480, height: 270)
            abrir(.video(AVPlayer(url: url), fichero: url, soloAudio: audio), tamano: tam, para: .archivo(url))
        }
        return true
    }

    /// El pin del reproductor flotante (uno solo).
    private weak var pinReproductor: PinIPad?

    /// **El reproductor flotante** (pedido «reproducir»,
    /// `reproductor_flotante.rs` de Windows): uno solo; si ya suena, un
    /// pedido nuevo le cambia la fila entera. Varios audios suenan en fila
    /// en el mismo pin, y el pin se va al acabar el ultimo.
    public func reproducir(_ urls: [URL]) {
        let pistas = urls.filter { FileManager.default.fileExists(atPath: $0.path) }
        guard let primera = pistas.first else { return avisarFalloPin() }
        let items = pistas.map { AVPlayerItem(url: $0) }
        if let pin = pinReproductor, !pin.cerrado, case let .video(r, _, true) = pin.contenido,
           let cola = r as? AVQueuePlayer {
            cola.pause()
            cola.removeAllItems()
            for i in items { cola.insert(i, after: nil) }
            pin.pistas = pistas
            pin.pistaActual = 0
            cola.play()
            return
        }
        let cola = AVQueuePlayer(items: items)
        let pin = abrir(.video(cola, fichero: primera, soloAudio: true), tamano: CGSize(width: 360, height: 112),
                        para: .nada)
        pin.pistas = pistas
        pin.cierraAlAcabar = true
        pin.sonarAlAbrir = true
        pinReproductor = pin
    }

    /// Pinea un PDF por su pagina `pagina` (desde 0) con todas las demas
    /// detras: se pasan desde la barra del pin. `false` si no se pudo leer.
    @discardableResult
    public func pinear(pdf url: URL, pagina: Int = 0) -> Bool {
        guard let documento = PDFDocument(url: url), documento.pageCount > 0 else { return false }
        let paginas = PaginasPdfPin(actual: pagina, total: documento.pageCount)
        guard let imagen = Self.pagina(documento, paginas.actual, lado: ladoPin) else { return false }
        let pin = abrirImagen(imagen, pixeles: nil, fichero: url, para: .archivo(url))
        pin.hacerPdf(documento, paginas: paginas)
        guardarPagina(pin)
        refrescar()
        return true
    }

    /// Pinea un PDF ya abierto (el del lector, que puede venir de un Word o
    /// un EPUB convertido) por su pagina `pagina`, con todas las demas. Si
    /// el PDF es un fichero, vuelve al arrancar; si solo esta en memoria, no.
    @discardableResult
    public func pinear(documentoPdf documento: PDFDocument, pagina: Int = 0) -> Bool {
        guard documento.pageCount > 0 else { return false }
        let paginas = PaginasPdfPin(actual: pagina, total: documento.pageCount)
        guard let imagen = Self.pagina(documento, paginas.actual, lado: ladoPin) else { return false }
        let fichero = documento.documentURL.flatMap { url in
            url.isFileURL && TipoContenido.de(url) == .pdf && FileManager.default.fileExists(atPath: url.path) ? url : nil
        }
        let pin = abrirImagen(imagen, pixeles: nil, fichero: fichero, para: fichero.map { ParaAlmacen.archivo($0) } ?? ParaAlmacen.nada)
        pin.hacerPdf(documento, paginas: paginas)
        guardarPagina(pin)
        refrescar()
        return true
    }

    /// Mientras se fotografia lo que se ve, la capa no se pinta.
    @Published private(set) var sinCapa = false

    /// «Fijar como pin» de un visor sin fichero que pinear (un dibujo, un
    /// plano, una pagina web): una foto de lo que se ve en la ventana, sin
    /// los pines de encima.
    public func pinearLoQueSeVe() {
        guard !sinCapa else { return }
        sinCapa = true
        Task { [weak self] in
            // Que se pinte una vez sin la capa.
            try? await Task.sleep(for: .milliseconds(80))
            guard let self else { return }
            let foto = CapturadorIPad.compartido.fotoDeLaApp(despuesDePintar: true)
            self.sinCapa = false
            guard let foto else { return avisarFalloPin() }
            self.pinear(imagen: foto)
        }
    }

    /// Pinea una pagina de un PDF como foto suelta (sin las demas): para un
    /// visor que quiere fijar solo lo que se ve. Se lee a buena resolucion.
    @discardableResult
    public func pinearSoloPagina(pdf url: URL, pagina: Int) -> Bool {
        guard let documento = PDFDocument(url: url), pagina >= 0, pagina < documento.pageCount,
              let imagen = Self.pagina(documento, pagina, lado: 2400) else { return false }
        pinear(imagen: imagen, origen: "recorte")
        return true
    }

    /// Pinea lo que haya en el portapapeles: una imagen, o un texto como
    /// nota. `false` si no habia nada.
    @discardableResult
    public func pinearDesdePortapapeles() -> Bool {
        let tablero = UIPasteboard.general
        if let foto = tablero.image, let cg = ImagenIPad.derecha(foto) {
            pinear(imagen: cg, origen: "portapapeles")
            return true
        }
        if let url = tablero.url, url.isFileURL, pinear(url: url) { return true }
        if let texto = tablero.string, !texto.isEmpty {
            // La palabra magica sola saca su herramienta (`PalabraMagicaPin`).
            switch PalabraMagicaPin.detectar(texto) {
            case let .miniapp(m)?:
                pinearMiniapp(m)
            case .lienzo?:
                if let g = almacenApp?.chats.first(where: \.esGuardados)?.id {
                    Navegacion.compartida.abrir(.lienzo(chat: g, mensaje: nil, foto: nil))
                } else {
                    pinearNota(texto: texto)
                }
            case nil:
                pinearNota(texto: texto)
            }
            return true
        }
        return false
    }

    /// Una nota suelta en Markdown.
    public func pinearNota(texto: String = "") {
        abrir(.herramienta(HerramientaPin(.nota(ModeloNota(texto: texto)))),
              tamano: ReglasCapa.tamanoHerramienta(nil), para: .nota(texto, origen: "nota"))
    }

    /// Una mini-app nueva (`tareas`, `contador`, `ruleta`, `gastos`,
    /// `cronometro`, `temporizador`, `alarma`): se crea en «Mensajes
    /// guardados» (asi se guarda y viaja al movil) y se pinea.
    public func pinearMiniapp(_ miniapp: String, titulo: String = "") {
        let moneda = monedaDelAparato()
        guard let documento = miniDocumentoNuevo(miniapp: miniapp, titulo: titulo, moneda: moneda) else {
            return avisarFalloPin()
        }
        guard let nucleo else {
            abrirMiniapp(DocumentoMini(miniapp: miniapp, documento: documento, nucleo: nil, chat: nil, mensaje: nil))
            return
        }
        Task { [weak self] in
            let creado = await Task.detached(priority: .userInitiated) { () -> (String, MensajeChat)? in
                guard let chat = try? nucleo.chatGuardados(),
                      let m = try? nucleo.crearMiniapp(chat: chat.id, miniapp: miniapp, titulo: titulo, moneda: moneda)
                else { return nil }
                return (chat.id, m)
            }.value
            guard let self else { return }
            guard let creado else { return avisarFalloPin() }
            self.abrirMiniapp(DocumentoMini(miniapp: miniapp, documento: creado.1.texto, nucleo: nucleo,
                                            chat: creado.0, mensaje: creado.1.id))
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        }
    }

    /// Saca a la capa una mini-app que ya esta en un chat (su mensaje
    /// `MINIAPP`): lo que se toque en el pin se guarda en ese mensaje.
    public func pinearMiniapp(chat: String, mensaje: String, miniapp: String, documento: String) {
        abrirMiniapp(DocumentoMini(miniapp: miniapp, documento: documento, nucleo: nucleo,
                                   chat: chat, mensaje: mensaje))
    }

    private func abrirMiniapp(_ doc: DocumentoMini) {
        abrir(.herramienta(HerramientaPin(.mini(doc))), tamano: ReglasCapa.tamanoHerramienta(doc.miniapp),
              para: .nota(doc.documento, origen: AlmacenPines.origenMini(doc.miniapp, chat: doc.chat,
                                                                         mensaje: doc.mensaje)))
        AvisosMini.compartido.avisar(de: doc)
    }

    // MARK: Lo que se hace con un pin

    func pin(_ id: Int) -> PinIPad? {
        pines.first { $0.id == id }
    }

    /// Lo trae delante y ensena su barra.
    func elegir(_ pin: PinIPad) {
        if let i = pines.firstIndex(where: { $0 === pin }), i != pines.count - 1 {
            pines.remove(at: i)
            pines.append(pin)
        }
        elegido = pin.id
    }

    /// Un toque en el pin: delante y con su barra; otro toque, sin barra.
    func tocar(_ pin: PinIPad) {
        if elegido == pin.id, pines.last === pin {
            elegido = nil
        } else {
            elegir(pin)
        }
    }

    /// Desde el panel o una burbuja: a la vista, delante y elegido.
    func traer(_ id: Int) {
        guard let pin = pin(id) else { return }
        pin.oculto = false
        pin.minimizado = false
        pin.centro = ReglasCapa.centroDentro(pin.centro, tamano: pin.tamano, area: area)
        elegir(pin)
        refrescar()
    }

    func minimizar(_ pin: PinIPad) {
        pin.minimizado = true
        if elegido == pin.id { elegido = nil }
        refrescar()
    }

    func alternarOculto(_ id: Int) {
        guard let pin = pin(id) else { return }
        pin.oculto.toggle()
        if pin.oculto, elegido == id { elegido = nil }
        refrescar()
    }

    func ocultarTodos(_ ocultar: Bool) {
        olvidarApartadosYa()
        for p in pines {
            p.oculto = ocultar
            if !ocultar { p.minimizado = false }
        }
        if ocultar { elegido = nil }
        refrescar()
    }

    public func mostrarPanelAbiertos() {
        panelAbierto = true
    }

    /// Lo movio, lo escalo, lo giro... : se apunta (juntado).
    func cambio(_ pin: PinIPad) {
        pin.centro = ReglasCapa.centroDentro(pin.centro, tamano: pin.tamano, area: area)
        programarGuardar(pin)
        refrescar()
    }

    /// Lo de dentro cambio (la nota, la mini-app, la foto anotada).
    func cambioDeContenido(_ pin: PinIPad) {
        porReescribir[pin.id] = pin
        programarEscritura()
        refrescar()
    }

    /// La foto anotada vuelve al pin (y al almacen, como un PNG nuevo).
    func cambiarImagen(de id: Int, por nueva: CGImage) {
        guard let pin = pin(id) else { return }
        let lado = ReglasPin.ladoMiniatura(pantalla: area, escala: ArribaUIKit.escalaPantalla)
        pin.cambiarImagen(Miniatura.reducir(nueva, ladoMaximo: lado))
        guard let almacen, let idAlmacen = pin.idAlmacen else { return }
        Task { [weak self] in
            let png = await ImagenIPad.fuera { ImagenIPad.png(nueva) }
            guard let png else { return }
            try? almacen.reemplazarImagen(idAlmacen, png: png)
            self?.programarGuardar(pin)
        }
    }

    func cerrar(_ pin: PinIPad) {
        // Lo que tuviera por apuntar se apunta antes, para que vuelva igual.
        if porGuardar[pin.id] != nil, let almacen, let id = pin.idAlmacen {
            try? almacen.actualizarPin(id, pin.guardado(escalaPantalla: escalaPantalla))
        }
        pin.marcarCerrado()
        porGuardar[pin.id] = nil
        if let p = porReescribir.removeValue(forKey: pin.id) { reescribir(p) }
        if !apartados.contains(where: { $0 === pin }) {
            guardadoUltimoCerrado = marcarCerrado(pin) ?? pin.guardado(escalaPantalla: escalaPantalla)
            ultimoCerrado = pin
        }
        pines.removeAll { $0 === pin }
        if elegido == pin.id { elegido = nil }
        refrescar()
    }

    /// Si hay un pin cerrado que se pueda devolver.
    public var hayUltimoCerrado: Bool { ultimoCerrado != nil }

    /// «Devolver el ultimo pin cerrado» (`pines devolver_ultimo` de
    /// Windows): vuelve donde estaba, con lo que tenia.
    public func devolverUltimoCerrado() {
        guard let pin = ultimoCerrado else { return avisarFalloPin() }
        ultimoCerrado = nil
        olvidarApartadosYa()
        pin.reabrir()
        pin.oculto = false
        pin.minimizado = false
        pin.centro = ReglasCapa.centroDentro(pin.centro, tamano: pin.tamano, area: area)
        pines.append(pin)
        elegido = pin.id
        if let id = pin.idAlmacen {
            try? almacen?.actualizarPin(id, guardadoUltimoCerrado ?? pin.guardado(escalaPantalla: escalaPantalla))
            _ = try? almacen?.ponerGrupo(id, color: pin.grupo.flatMap(ColorGrupo.porIndice))
        }
        guardadoUltimoCerrado = nil
        refrescar()
    }

    // MARK: Guias e iman

    /// Mientras se arrastra: las guias de donde iria el pin.
    func guiar(_ pin: PinIPad, centro: CGPoint) {
        let marco = ReglasGuias.marco(centro: centro, tamano: pin.tamano)
        let nuevas = ReglasGuias.alinear(marco, otros: marcosDeLosDemas(pin)).guias
        if nuevas != guias { guias = nuevas }
    }

    /// Al soltar: el centro pegado a las guias (el iman), y fuera las guias.
    func iman(_ pin: PinIPad, centro: CGPoint) -> CGPoint {
        if !guias.isEmpty { guias = [] }
        let marco = ReglasGuias.marco(centro: centro, tamano: pin.tamano)
        let ajuste = ReglasGuias.alinear(marco, otros: marcosDeLosDemas(pin))
        return CGPoint(x: ajuste.rect.midX, y: ajuste.rect.midY)
    }

    func quitarGuias() {
        if !guias.isEmpty { guias = [] }
    }

    private func marcosDeLosDemas(_ pin: PinIPad) -> [CGRect] {
        pines.filter { $0 !== pin && !$0.oculto && !$0.minimizado }
            .map { ReglasGuias.marco(centro: $0.centro, tamano: $0.tamano) }
    }

    // MARK: PDF

    /// Pasa `pasos` paginas en un pin de PDF.
    func pasarPagina(_ pin: PinIPad, _ pasos: Int) {
        guard var p = pin.paginas, let documento = pin.documentoPdf, p.pasar(pasos),
              let imagen = Self.pagina(documento, p.actual, lado: ladoPin) else { return }
        pin.ponerPagina(imagen, paginas: p)
        guardarPagina(pin)
        cambio(pin)
    }

    /// «Pinear esta pagina»: la que se ve, como pin de foto propio.
    func pinearPagina(de pin: PinIPad) {
        guard let p = pin.paginas, let documento = pin.documentoPdf,
              let imagen = Self.pagina(documento, p.actual, lado: 2400) else { return avisarFalloPin() }
        pinear(imagen: imagen, origen: "recorte")
    }

    /// Cuantas paginas saca «Pinear todas las paginas» como mucho (cada una
    /// es un pin: mas no se manejan en la capa).
    static let topePaginasSueltas = 24

    /// «Pinear todas las paginas»: cada una en su pin (escalonados).
    func pinearTodasLasPaginas(de pin: PinIPad) {
        guard let p = pin.paginas, let documento = pin.documentoPdf else { return avisarFalloPin() }
        for n in 0 ..< min(p.total, Self.topePaginasSueltas) {
            if let imagen = Self.pagina(documento, n, lado: 1600) { pinear(imagen: imagen, origen: "recorte") }
        }
    }

    /// La pagina de cada pin de PDF se apunta aparte (el indice de Windows
    /// no la tiene): asi vuelve por la misma al arrancar.
    private static let clavePagina = "pines.pdf.pagina."

    private func guardarPagina(_ pin: PinIPad) {
        guard let id = pin.idAlmacen, let p = pin.paginas else { return }
        UserDefaults.standard.set(p.actual, forKey: Self.clavePagina + String(id))
    }

    private func paginaGuardada(_ id: UInt64) -> Int {
        UserDefaults.standard.integer(forKey: Self.clavePagina + String(id))
    }

    private var ladoPin: Int { ReglasPin.ladoMiniatura(pantalla: area, escala: ArribaUIKit.escalaPantalla) }

    // MARK: Arrastrar fuera

    /// Lo que lleva un pin al arrastrarlo a otra app (Split View, Slide
    /// Over): la foto que se ve, el fichero o el texto.
    func proveedorArrastre(_ pin: PinIPad) -> NSItemProvider {
        if let texto = pin.textoHerramienta { return NSItemProvider(object: texto as NSString) }
        if pin.paginas == nil, let vista = pin.vista { return NSItemProvider(object: UIImage(cgImage: vista)) }
        if let url = pin.fichero, let p = NSItemProvider(contentsOf: url) { return p }
        if let vista = pin.vista { return NSItemProvider(object: UIImage(cgImage: vista)) }
        return NSItemProvider()
    }

    /// Cierra todos de forma que se puede deshacer: se apartan y se cierran
    /// de verdad a los 30 s o al hacer otra cosa.
    func cerrarTodosConDeshacer() {
        olvidarApartadosYa()
        guardarPendientes()
        apartados = pines
        for p in apartados {
            if let g = marcarCerrado(p) { guardadosApartados[p.id] = g }
        }
        pines = []
        elegido = nil
        puedeDeshacer = !apartados.isEmpty
        olvidarApartados = Task { [weak self] in
            try? await Task.sleep(for: .seconds(30))
            guard !Task.isCancelled else { return }
            self?.olvidarApartadosYa()
        }
    }

    func deshacerCerrarTodos() {
        olvidarApartados?.cancel()
        olvidarApartados = nil
        let vuelven = apartados
        apartados = []
        pines += vuelven
        for p in vuelven {
            if let id = p.idAlmacen, let g = guardadosApartados[p.id] { try? almacen?.actualizarPin(id, g) }
        }
        guardadosApartados = [:]
        puedeDeshacer = false
    }

    private func olvidarApartadosYa() {
        olvidarApartados?.cancel()
        olvidarApartados = nil
        let lista = apartados
        apartados = []
        guardadosApartados = [:]
        for p in lista { p.marcarCerrado() }
        puedeDeshacer = false
    }

    /// Las filas del panel.
    var filas: [FilaPin] { pines.map(\.fila) }

    /// Avisa a quien pinta el panel y las burbujas.
    private func refrescar() {
        objectWillChange.send()
    }

    // MARK: Lo que sale del pin

    /// Copiar: la foto que se ve (con filtros y giro) o el texto.
    func copiar(_ pin: PinIPad) {
        if let texto = pin.textoHerramienta {
            UIPasteboard.general.string = texto
        } else if let vista = pin.vista {
            UIPasteboard.general.image = UIImage(cgImage: vista)
        } else if let url = pin.fichero {
            UIPasteboard.general.url = url
        }
        UINotificationFeedbackGenerator().notificationOccurred(.success)
    }

    /// Guardar en Fotos (pide el permiso de anadir la primera vez).
    func guardarEnFotos(_ pin: PinIPad) {
        guard let vista = pin.vista else { return avisarFalloPin() }
        UIImageWriteToSavedPhotosAlbum(UIImage(cgImage: vista), nil, nil, nil)
        UINotificationFeedbackGenerator().notificationOccurred(.success)
    }

    /// Al chat: la foto a un proyecto que se elige (con buscador), o la nota
    /// a «Mensajes guardados».
    func alChat(_ pin: PinIPad) {
        guard let almacenApp else { return avisarFalloPin() }
        if let texto = pin.textoHerramienta {
            guard case let .herramienta(h) = pin.contenido, case .nota = h.clase else { return }
            Task {
                await almacenApp.cargarChats()
                guard let chat = almacenApp.chats.first(where: \.esGuardados)?.id else { return avisarFalloPin() }
                let m = await almacenApp.escribir(nota: texto, en: chat)
                if m == nil { avisarFalloPin() } else { UINotificationFeedbackGenerator().notificationOccurred(.success) }
            }
            return
        }
        guard let vista = pin.vista else { return avisarFalloPin() }
        SelectorProyectoIPad.elegir(para: vista, almacen: almacenApp)
    }

    /// Sacar texto (OCR con Vision) y ensenarlo para copiarlo.
    func sacarTextoDe(_ pin: PinIPad) {
        guard let vista = pin.vista else { return avisarFalloPin() }
        VistaTextoSacado.sacar(de: vista)
    }

    /// Anotar: el lienzo con todas sus herramientas sobre la foto del pin
    /// (`Pines/AnotarPinLienzo.swift`); al acabar, la anotada vuelve al
    /// mismo pin. Si el lienzo no se abre, el editor de captura.
    func anotar(_ pin: PinIPad) {
        guard let vista = pin.vista else { return avisarFalloPin() }
        let id = pin.id
        AnotarPinConLienzo.abrir(vista, listo: { [weak self] anotada in
            self?.cambiarImagen(de: id, por: anotada)
        }, sinLienzo: {
            CapturadorIPad.compartido.editar(vista, alAnotarPin: { [weak self] anotada in
                self?.cambiarImagen(de: id, por: anotada)
            })
        })
    }

    // MARK: Por dentro

    private enum ParaAlmacen {
        case captura(CGImage, origen: String)
        case foto(URL)
        case nota(String, origen: String)
        case archivo(URL)
        case entrada(UInt64)
        case nada
    }

    @discardableResult
    private func abrirImagen(_ imagen: CGImage, pixeles: CGSize?, fichero: URL?, para: ParaAlmacen) -> PinIPad {
        let px = pixeles ?? CGSize(width: imagen.width, height: imagen.height)
        let tam = ReglasPin.tamanoInicial(pixeles: px, escala: ArribaUIKit.escalaPantalla, visible: area)
        return abrir(.imagen(imagen, fichero: fichero), tamano: tam, para: para)
    }

    @discardableResult
    private func abrir(_ contenido: ContenidoPinIPad, tamano: CGSize, centro: CGPoint? = nil, escala: CGFloat = 1,
                       para: ParaAlmacen, guardado: PinGuardado? = nil) -> PinIPad {
        olvidarApartadosYa()
        let nace = centro ?? ReglasCapa.centroDeNacer(area: area, n: nacidos)
        if centro == nil { nacidos += 1 }
        let pin = PinIPad(id: siguienteId, contenido: contenido, centro: nace, tamanoBase: tamano)
        siguienteId += 1
        pin.escala = escala
        if let guardado { pin.aplicar(guardado) }
        if areaConocida { pin.centro = ReglasCapa.centroDentro(pin.centro, tamano: pin.tamano, area: area) }
        if case let .herramienta(h) = contenido {
            h.alCambiar = { [weak self, weak pin] in
                guard let self, let pin else { return }
                self.cambioDeContenido(pin)
            }
        }
        pines.append(pin)
        if guardado == nil { elegido = pin.id }
        registrar(pin, para)
        return pin
    }

    private var escalaPantalla: CGFloat { ArribaUIKit.escalaPantalla }

    /// Le da su entrada al pin nuevo.
    private func registrar(_ pin: PinIPad, _ para: ParaAlmacen) {
        if case let .entrada(id) = para {
            pin.idAlmacen = id
            return
        }
        guard let almacen else { return }
        let g = pin.guardado(escalaPantalla: escalaPantalla)
        switch para {
        case let .captura(imagen, origen):
            Task { [weak self] in
                let png = await ImagenIPad.fuera { ImagenIPad.png(imagen) }
                guard let self, let png else { return }
                let abierto = pin.cerrado ? nil : pin.guardado(escalaPantalla: self.escalaPantalla)
                pin.idAlmacen = try? almacen.guardarImagen(png: png, origen: origen, pin: abierto)
                if !pin.cerrado { self.programarGuardar(pin) }
            }
        case let .foto(url):
            pin.idAlmacen = try? almacen.guardarImagen(deFichero: url, origen: AlmacenPines.origenFoto, pin: g)
        case let .nota(texto, origen):
            pin.idAlmacen = try? almacen.guardarNota(texto, origen: origen, pin: g)
        case let .archivo(url):
            pin.idAlmacen = try? almacen.guardarArchivo(url, pin: g)
        case .entrada, .nada:
            break
        }
    }

    @discardableResult
    private func marcarCerrado(_ pin: PinIPad) -> PinGuardado? {
        guard let almacen, let id = pin.idAlmacen, let g = almacen.entrada(id)?.pin else { return nil }
        try? almacen.actualizarPin(id, nil)
        return g
    }

    private func programarGuardar(_ pin: PinIPad) {
        porGuardar[pin.id] = pin
        programarEscritura()
    }

    private func programarEscritura() {
        guard tareaGuardar == nil else { return }
        tareaGuardar = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled else { return }
            self?.guardarPendientes()
        }
    }

    /// Escribe ya lo pendiente.
    func guardarPendientes() {
        tareaGuardar?.cancel()
        tareaGuardar = nil
        let guardar = Array(porGuardar.values)
        let reescribir = Array(porReescribir.values)
        porGuardar = [:]
        porReescribir = [:]
        guard let almacen else { return }
        for pin in guardar where !pin.cerrado {
            guard let id = pin.idAlmacen else { continue }
            try? almacen.actualizarPin(id, pin.guardado(escalaPantalla: escalaPantalla))
            _ = try? almacen.ponerGrupo(id, color: pin.grupo.flatMap(ColorGrupo.porIndice))
        }
        for pin in reescribir { self.reescribir(pin) }
    }

    private func reescribir(_ pin: PinIPad) {
        guard let almacen, let id = pin.idAlmacen, let texto = pin.textoHerramienta else { return }
        try? almacen.reescribirNota(id, texto: texto)
    }

    /// Al cambiar la ventana de tamano, que ninguno se quede fuera.
    private func meterEnArea() {
        for p in pines {
            let dentro = ReglasCapa.centroDentro(p.centro, tamano: p.tamano, area: area)
            if dentro != p.centro { p.centro = dentro }
        }
    }

    // MARK: Restaurar

    /// Devuelve a la capa lo que el almacen da por abierto (al arrancar).
    func restaurar() {
        guard let almacen else { return }
        let yaAbiertos = Set(pines.compactMap(\.idAlmacen))
        for e in almacen.abiertas where !yaAbiertos.contains(e.id) {
            restaurar(e)
        }
    }

    private func restaurar(_ e: EntradaAlmacen) {
        guard let almacen, let g = e.pin else { return }
        let sitio = ReglasCapa.deGuardado(g)
        let grupo = almacen.colorDe(e.id)?.indice
        let poner: (ContenidoPinIPad) -> Void = { [weak self] contenido in
            guard let self else { return }
            let pin = self.abrir(contenido, tamano: sitio.tamanoBase, centro: sitio.centro, escala: sitio.escala,
                                 para: .entrada(e.id), guardado: g)
            pin.grupo = grupo
        }
        let ponerPdf: (CGImage, PDFDocument, PaginasPdfPin, URL) -> Void = { [weak self] imagen, doc, paginas, url in
            guard let self else { return }
            let pin = self.abrir(.imagen(imagen, fichero: url), tamano: sitio.tamanoBase, centro: sitio.centro,
                                 escala: sitio.escala, para: .entrada(e.id), guardado: g)
            pin.grupo = grupo
            pin.hacerPdf(doc, paginas: paginas)
            self.refrescar()
        }
        let lado = ReglasPin.ladoMiniatura(pantalla: area, escala: escalaPantalla)
        switch e.tipo {
        case .imagen:
            let url = almacen.rutaObjeto(e)
            Task {
                guard let imagen = await Miniatura.fueraDelPrincipal({ Miniatura.de(url: url, ladoMaximo: lado) })
                else { return }
                // El giro guardado ya va en el tamano: se pone la foto sin girar.
                poner(.imagen(imagen, fichero: e.origen == AlmacenPines.origenFoto ? url : nil))
            }
        case .nota:
            guard let texto = try? String(contentsOf: almacen.rutaObjeto(e), encoding: .utf8) else { return }
            guard let mini = AlmacenPines.miniDeOrigen(e.origen) else {
                poner(.herramienta(HerramientaPin(.nota(ModeloNota(texto: texto)))))
                return
            }
            // Una mini-app del chat vuelve con lo que diga el chat ahora.
            if let chat = mini.chat, let mensaje = mini.mensaje, let nucleo {
                let actual = (try? nucleo.documentoMiniapp(chat: chat, mensaje: mensaje)) ?? texto
                poner(.herramienta(HerramientaPin(.mini(DocumentoMini(miniapp: mini.cual, documento: actual,
                                                                      nucleo: nucleo, chat: chat, mensaje: mensaje)))))
            } else {
                poner(.herramienta(HerramientaPin(.mini(DocumentoMini(miniapp: mini.cual, documento: texto,
                                                                      nucleo: nil, chat: nil, mensaje: nil)))))
            }
        case .archivo:
            let datos = URL(fileURLWithPath: almacen.dir.deletingLastPathComponent().path, isDirectory: true)
            guard let ruta = e.ruta, let url = ReglasCapa.rutaVigente(ruta, datos: datos) else { return }
            switch TipoContenido.de(url) {
            case .pdf:
                guard let documento = PDFDocument(url: url), documento.pageCount > 0 else { return }
                let paginas = PaginasPdfPin(actual: paginaGuardada(e.id), total: documento.pageCount)
                guard let pagina = Self.pagina(documento, paginas.actual, lado: lado) else { return }
                ponerPdf(pagina, documento, paginas, url)
            case .video, .audio:
                let reproductor = AVPlayer(url: url)
                reproductor.isMuted = true
                poner(.video(reproductor, fichero: url, soloAudio: TipoContenido.de(url) == .audio))
            case .imagen:
                Task {
                    guard let imagen = await Miniatura.fueraDelPrincipal({ Miniatura.de(url: url, ladoMaximo: lado) })
                    else { return }
                    poner(.imagen(imagen, fichero: url))
                }
            case nil:
                return
            }
        }
    }

    /// La pagina `n` (desde 0) de un PDF como foto (PDFKit la pinta a `lado`).
    private static func pagina(_ documento: PDFDocument, _ n: Int, lado: Int) -> CGImage? {
        guard let pagina = documento.page(at: n) else { return nil }
        let caja = pagina.bounds(for: .cropBox).size
        let mayor = max(caja.width, caja.height, 1)
        let f = CGFloat(min(lado, 2400)) / mayor
        let tam = CGSize(width: (caja.width * f).rounded(), height: (caja.height * f).rounded())
        return pagina.thumbnail(of: tam, for: .cropBox).cgImage
    }
}
