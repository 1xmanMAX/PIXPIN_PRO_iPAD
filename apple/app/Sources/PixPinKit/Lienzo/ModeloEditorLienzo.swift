// El estado del lienzo EDITABLE del iPad que ven SwiftUI (las barras) y la
// vista de UIKit (`VistaEditorLienzoIPad`): el `EditorLienzo` de Rust
// (`pixpin-nucleo/src/editor.rs`, el `Gesto` y la `Escena` de
// `pixpin-motor2d`, los mismos de Windows), la herramienta, el estilo y el
// guardado.
//
// Port del `ModeloEditorLienzo` del Mac (`Lienzo/EditorLienzoView.swift`)
// sin AppKit, con el fondo fijo (`FondoLienzo.swift`) y el papel que sigue
// el modo noche comun de los visores (`ModoNoche`).
//
// Rendimiento (§4bis del plan):
// - guardar va solo, 1 s despues del ultimo cambio, en una cola de fondo
//   (`.utility`): nada de escribir JSON en el hilo principal;
// - al irse la app al fondo se guarda ya, con un rato pedido a iOS.

import Combine
import Foundation
import PixPinCore
import SwiftUI
import UIKit

public extension Notification.Name {
    /// Un lienzo del chat se guardo: `object` es la referencia del dibujo.
    /// La burbuja que lo ensena se vuelve a leer.
    static let pixpinLienzoGuardado = Notification.Name("PixPinLienzoGuardado")
}

@MainActor
public final class ModeloEditorLienzo: ObservableObject {
    public let editor: EditorLienzo
    /// La referencia del dibujo en el chat (para avisar al guardar), si viene
    /// de un chat.
    public let referencia: String?
    /// La foto de cada `id_objeto`, con su fichero.
    public private(set) var fotos: [UInt64: URL]

    @Published public private(set) var herramienta: String
    @Published public private(set) var puedeDeshacer = false
    @Published public var error: String?
    /// Lo que ensena la barra de estilo (lo elegido, o el «actual»).
    @Published public private(set) var estilo: EstiloLienzo
    /// Hay algo elegido: el panel de estilo sale aunque la herramienta no
    /// dibuje (para cambiarle el color a lo elegido).
    @Published public private(set) var hayElegidos = false
    /// Se esta escribiendo un texto: las teclas son del texto y no atajos.
    @Published public private(set) var escribiendo = false
    /// El grupo de la barra con su desplegable abierto, si hay uno.
    @Published public var desplegado: GrupoHerramientaLienzo?
    /// El boton de imagen pidio elegir una foto (lo atiende SwiftUI con el
    /// selector de Fotos).
    @Published public var pidiendoImagen = false
    /// El panel de estilo escondido a mano (apretar el Apple Pencil).
    @Published public var panelEscondido = false
    /// **La zona va al chat** («Zona al chat» del grupo de elegir): lo que
    /// se recorte con la zona no se queda como copia en el lienzo, va al chat
    /// del lienzo como una foto (`zona_al_chat.rs` de Windows).
    @Published public var zonaAlChat = false
    /// **«Dibujar con el dedo»** (el de Notas): el dedo pinta aunque se use
    /// el Apple Pencil, y la vista se mueve con dos dedos. Apagado (de
    /// fabrica), desde que llega el Pencil el dedo mueve la vista y la palma
    /// no pinta. Se recuerda en este iPad (es del aparato, no del dibujo).
    @Published public var dedoPintaSiempre: Bool = ModeloEditorLienzo.dedoRecordado {
        didSet {
            UserDefaults.standard.set(dedoPintaSiempre, forKey: ModeloEditorLienzo.claveDedo)
            vista?.ajustarArrastre()
        }
    }
    nonisolated static let claveDedo = "pixpin.lienzo.dedoPinta"
    nonisolated static var dedoRecordado: Bool { UserDefaults.standard.bool(forKey: claveDedo) }
    /// El papel es oscuro: la barra va con el tema oscuro de Excalidraw,
    /// como en Windows (`tema_barra`).
    public let papelOscuro: Bool
    /// **Se ve de noche**: el papel es oscuro o el modo noche comun de los
    /// visores esta puesto (la barra va con el tema oscuro).
    @Published public private(set) var verOscuro: Bool
    /// El fondo fijo (una foto, una pagina de PDF), si lo hay.
    public let fondo: FondoLienzo?
    /// El chat del lienzo, si es de un chat (el `.pixpin` de compartir).
    public let chat: String?
    /// El nucleo, para la biblioteca de figuras (`nil` en un lienzo suelto
    /// de las pruebas).
    public let nucleo: Nucleo?
    /// El nombre del lienzo, para los ficheros que se comparten.
    public let titulo: String
    /// Lo que ensenan dentro los mosaicos y las lupas (`TapadoLienzo.swift`).
    private(set) lazy var tapados: PintorDeTapados = {
        let t = PintorDeTapados(editor: editor) { [weak self] id in self?.foto(id) }
        t.fondo = fondo
        return t
    }()

    /// La vista que pinta (la pone ella al nacer).
    weak var vista: VistaEditorLienzoIPad?
    /// Lo que hace el boton de cerrar de la barra del lienzo. Sin el,
    /// cierra el visor.
    public var alCerrar: (() -> Void)?

    /// Guardar, uno detras de otro y fuera del hilo principal.
    private let colaGuardar = DispatchQueue(label: "pixpin.lienzo.guardar", qos: .utility)
    private var guardadoPendiente: DispatchWorkItem?
    private var cambiosVistos: UInt64
    /// La herramienta de antes de la goma (el doble toque del Pencil vuelve).
    private var anterior: String?

    public init(editor: EditorLienzo, referencia: String? = nil, fotos: [UInt64: URL] = [:],
                nucleo: Nucleo? = nil, titulo: String = "Lienzo", fondo: FondoLienzo? = nil,
                chat: String? = nil) {
        self.editor = editor
        self.referencia = referencia
        self.fotos = fotos
        self.nucleo = nucleo
        self.titulo = titulo
        self.fondo = fondo
        self.chat = chat
        // El papel del motor mide lo que el fondo: entra en lo exportado.
        if let fondo { editor.ponerPapel(ancho: Float(fondo.tamano.width), alto: Float(fondo.tamano.height), png: nil) }
        herramienta = editor.herramientaActual()
        cambiosVistos = editor.cambios()
        puedeDeshacer = editor.puedeDeshacer()
        estilo = editor.estilo()
        let papel = (try? leerLienzo(editor.planoDeVista(x: 0, y: 0, zoom: 1, anchoPx: 1, altoPx: 1)))?.fondo
        let oscuro = papel.map { 0.299 * $0.r + 0.587 * $0.g + 0.114 * $0.b < 0.5 } ?? false
        papelOscuro = oscuro
        verOscuro = oscuro || ModoNoche.compartido.activo
        noche = ModoNoche.compartido.$activo.dropFirst().sink { [weak self] activo in
            guard let self else { return }
            let ver = self.papelOscuro || activo
            if ver != self.verOscuro { self.verOscuro = ver }
            self.vista?.cambioDeNoche()
        }
    }

    /// Lo que mira el modo noche comun.
    private var noche: AnyCancellable?

    /// **El papel que se ensena de noche** (`PAPEL` de `pixpin_render::oscuro`,
    /// el gris de la barra de los pines): con el modo noche puesto, un papel
    /// claro se ve de este gris y la tinta se adapta para leerse, como en
    /// los lectores. Solo se ve: lo guardado y lo exportado siguen con su
    /// papel. Con un fondo fijo (una pagina, una foto) no: la tinta adaptada
    /// no se leeria encima de la hoja blanca.
    static let papelDeNoche = ColorLienzo(r: 0x1F / 255, g: 0x1F / 255, b: 0x22 / 255, a: 1)

    /// El papel con el que se pinta ahora la vista, si no es el suyo.
    var papelDeVista: ColorLienzo? {
        ModoNoche.compartido.activo && !papelOscuro && fondo == nil ? Self.papelDeNoche : nil
    }

    /// **Para quien usa esta barra con otros editores** (la tinta de un
    /// documento): lo elegido, el estilo y deshacer van tambien a ellos.
    public var alElegir: ((String) -> Void)?
    public var alCambiarEstilo: ((CambioDeEstilo) -> Void)?
    public var alDeshacer: (() -> Void)?
    public var alRehacer: (() -> Void)?

    public func elegir(_ nombre: String) {
        // Otra herramienta: la zona vuelve a ser la copia en el lienzo.
        zonaAlChat = false
        alElegir?(nombre)
        do {
            let antes = herramienta
            try editor.herramienta(nombre: nombre)
            herramienta = editor.herramientaActual()
            if herramienta != antes, herramienta != "borrador" { anterior = nil }
            estilo = editor.estilo()
            hayElegidos = editor.cuantosElegidos() > 0
            escribiendo = editor.escribiendo()
            vista?.trasCambio()
        } catch {
            self.error = "\(error)"
        }
    }

    /// **El doble toque del Apple Pencil** (`switchEraser`): de lo que se
    /// este usando a la goma y de la goma a lo de antes.
    public func alternarGoma() {
        if herramienta == "borrador" {
            elegir(anterior ?? "lapiz")
            anterior = nil
        } else {
            let antes = herramienta
            elegir("borrador")
            anterior = antes
        }
    }

    /// El doble toque con «la herramienta anterior» (`switchPrevious`).
    private var ultimaDistinta: String?
    public func alternarAnterior() {
        let ahora = herramienta
        elegir(ultimaDistinta ?? (ahora == "lapiz" ? "borrador" : "lapiz"))
        ultimaDistinta = ahora
    }

    /// Un mando de la barra de estilo: queda para lo proximo y, si hay algo
    /// elegido, se le aplica (un paso de deshacer).
    public func cambiarEstilo(_ cambio: CambioDeEstilo) {
        alCambiarEstilo?(cambio)
        _ = editor.cambiarEstilo(cambio: cambio)
        estilo = editor.estilo()
        vista?.trasCambio()
    }

    /// Tras algo que pudo cambiar lo elegido o la herramienta desde dentro
    /// (soltar, pegar): la barra vuelve a mirar al editor.
    func refrescarBarra() {
        let e = editor.estilo()
        if e != estilo { estilo = e }
        let elegidos = editor.cuantosElegidos() > 0
        if elegidos != hayElegidos { hayElegidos = elegidos }
        let h = editor.herramientaActual()
        if h != herramienta { herramienta = h }
        let t = editor.escribiendo()
        if t != escribiendo { escribiendo = t }
    }

    /// Apunta la foto de una imagen recien pegada.
    func anadirFoto(_ id: UInt64, _ url: URL) {
        fotos[id] = url
    }

    /// Las fotos ya leidas de este lienzo, suyas: la cache compartida la
    /// vacia el sistema cuando quiere.
    private var fotosLeidas: [UInt64: CGImage] = [:]
    private var leyendo: Set<UInt64> = []
    private var fotosReleidas = false

    /// El bitmap de una foto del lienzo; si aun no esta, se lee y se repinta.
    func foto(_ id: UInt64) -> CGImage? {
        if let img = fotosLeidas[id] { return img }
        if fotos[id] == nil, !fotosReleidas {
            // Una imagen que no se apunto al abrir (llegada al sincronizar):
            // se vuelven a pedir sus rutas.
            fotosReleidas = true
            for f in editor.imagenes() where fotos[f.idObjeto] == nil { fotos[f.idObjeto] = URL(fileURLWithPath: f.ruta) }
        }
        guard let url = fotos[id] else { return nil }
        if let img = Miniaturas.compartidas.enCache(url, lado: 2048) {
            fotosLeidas[id] = img
            return img
        }
        if !leyendo.contains(id) {
            leyendo.insert(id)
            Task { [weak self] in
                let img = await Miniaturas.compartidas.cargar(url, lado: 2048)
                guard let self else { return }
                self.leyendo.remove(id)
                if let img {
                    self.fotosLeidas[id] = img
                    self.vista?.repintarFotos()
                }
            }
        }
        return nil
    }

    /// Las imagenes del pintor: las fotos y lo de dentro de los mosaicos y
    /// las lupas, a `escala` pixeles por unidad.
    func imagenes(escala: CGFloat) -> (UInt64) -> CGImage? {
        let version = editor.version()
        return { [weak self] id in
            guard let self else { return nil }
            // Primero las fotos: el id de una foto puede tener los bits altos
            // puestos, como un mosaico o una lupa.
            if self.fotos[id] != nil { return self.foto(id) }
            if PintorDeTapados.esTapado(id) {
                return self.tapados.imagen(id, version: version, escala: escala)
            }
            return self.foto(id)
        }
    }

    public func deshacer() {
        if let alDeshacer { return alDeshacer() }
        _ = editor.deshacer()
        vista?.trasCambio()
    }

    public func rehacer() {
        if let alRehacer { return alRehacer() }
        _ = editor.rehacer()
        vista?.trasCambio()
    }

    public func borrarSeleccion() {
        if editor.borrarSeleccion() { vista?.trasCambio() }
    }

    public func seleccionarTodo() {
        editor.seleccionarTodo()
        vista?.trasCambio()
    }

    /// Lo llama la vista tras cada cosa que puede cambiar el documento: si
    /// cambio, se programa el guardado (1 s despues del ULTIMO cambio).
    func revisarCambios() {
        let cambios = editor.cambios()
        let puede = editor.puedeDeshacer()
        if puede != puedeDeshacer { puedeDeshacer = puede }
        guard cambios != cambiosVistos else { return }
        cambiosVistos = cambios
        programarGuardado(dentroDe: 1.0)
    }

    private func programarGuardado(dentroDe segundos: Double) {
        guardadoPendiente?.cancel()
        let tarea = DispatchWorkItem { [editor, referencia] in
            ModeloEditorLienzo.guardarYa(editor, referencia: referencia)
        }
        guardadoPendiente = tarea
        colaGuardar.asyncAfter(deadline: .now() + segundos, execute: tarea)
    }

    /// Guarda ahora (al cerrar el visor o al irse la app al fondo): termina
    /// el texto que se estaba escribiendo y escribe en la cola de fondo, con
    /// un rato pedido a iOS por si la app se suspende a medias. El editor
    /// vive lo que dure la tarea aunque el visor ya no este.
    public func guardarAhora(terminarTexto: Bool = true) {
        guardadoPendiente?.cancel()
        guardadoPendiente = nil
        if terminarTexto, editor.escribiendo() {
            _ = editor.terminarTexto()
            refrescarBarra()
        }
        let editor = self.editor
        let referencia = self.referencia
        let rato = UIApplication.shared.beginBackgroundTask(withName: "PixPin: guardar el lienzo")
        colaGuardar.async {
            ModeloEditorLienzo.guardarYa(editor, referencia: referencia)
            DispatchQueue.main.async {
                if rato != .invalid { UIApplication.shared.endBackgroundTask(rato) }
            }
        }
    }

    /// Se cierra el visor: el asistente ya no dibuja aqui y se guarda.
    public func cerrar() {
        editor.cerrado()
        guardarAhora()
    }

    /// En la cola de fondo: escribe y, si escribio, avisa en el principal.
    nonisolated static func guardarYa(_ editor: EditorLienzo, referencia: String?) {
        let escrito: Bool
        do {
            escrito = try editor.guardar()
        } catch {
            NSLog("PixPin: no se pudo guardar el lienzo: %@", "\(error)")
            return
        }
        guard escrito else { return }
        DispatchQueue.main.async {
            NotificationCenter.default.post(name: .pixpinLienzoGuardado, object: referencia)
            // La sincronizacion sola cuenta sus 30 s de calma desde aqui.
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        }
    }

    /// El boton de cerrar de la isla: lo que diga quien lo hospeda o cerrar
    /// el visor (se guarda al desaparecer).
    func cerrarVisor() {
        if let alCerrar { return alCerrar() }
        Navegacion.compartida.cerrarVisor()
    }
}
