// El lienzo EDITABLE del iPad: una `UIView` (Y hacia abajo, como el mundo
// del lienzo) que manda el Apple Pencil, el dedo y el teclado al
// `EditorLienzo` de Rust y pinta lo que le devuelve con el lector y el
// pintor de siempre (`LectorLienzo.swift`, `PintorLienzo.swift`).
//
// Port de `VistaEditorLienzoNS` del Mac, con el tacto:
// - **Apple Pencil**: pinta siempre, con su presion (`force`) y todas sus
//   muestras (`coalescedTouches`); lo que iOS predice (`predictedTouches`)
//   se dibuja encima un fotograma, sin mandarlo al motor, para que la tinta
//   no vaya detras de la punta. La inclinacion (`altitudeAngle`) aclara el
//   grafito tumbado (el motor no tiene inclinacion: va en la presion).
// - **La palma y el dedo no pintan cuando hay Pencil** (desde que se usa el
//   Pencil, o si el sistema lo pide con «Solo dibujar con el Apple
//   Pencil»): el dedo mueve la vista y dos dedos la acercan. Sin Pencil, el
//   dedo pinta y se mueve con dos dedos.
// - Doble toque del Pencil (`UIPencilInteraction`): goma ⇄ lapiz (o lo que
//   el usuario tenga puesto en Ajustes); apretarlo esconde el panel.
// - Pasar el Pencil por encima (iPad con hover) o el puntero del trackpad:
//   un puntito donde caeria la tinta (el circulo de la goma).
// - Deshacer con dos dedos, rehacer con tres (como Procreate y Notas), y los
//   botones de la barra (⌘Z, ⇧⌘Z).
// - El texto: el teclado del iPad (`UIKeyInput`) mientras se escribe.
//
// Rendimiento (§4bis del plan): una llamada al puente por muestra; pintar
// pide a Rust SOLO lo que cae en pantalla (`planoDeVista`), una vez por
// fotograma y solo si cambio la version, la camara o el tamano.

import PixPinCore
import UIKit
import UniformTypeIdentifiers

/// La vista que edita. Camara: `origen` es el punto del mundo en la esquina
/// de arriba a la izquierda y `zoom` los puntos de pantalla por unidad.
@MainActor
public final class VistaEditorLienzoIPad: UIView, UIKeyInput, UIPencilInteractionDelegate,
    UIGestureRecognizerDelegate, UIDropInteractionDelegate {
    let modelo: ModeloEditorLienzo
    private var editor: EditorLienzo { modelo.editor }

    public private(set) var origen = CGPoint.zero
    public private(set) var zoom: CGFloat = 1
    static let zoomMinimo: CGFloat = 0.05
    static let zoomMaximo: CGFloat = 30

    /// Lo ultimo pedido a Rust y con que se pidio: si nada cambio, se pinta
    /// esto sin cruzar el puente.
    private var leido: LienzoLeido?
    private var clave: (version: UInt64, x: CGFloat, y: CGFloat, zoom: CGFloat, w: CGFloat, h: CGFloat)?
    private var versionVista: UInt64 = 0
    /// Los colores ya adaptados al papel oscuro.
    private var coloresOscuros: [ColorLienzo: ColorLienzo] = [:]
    private var encajada = false

    // El toque que pinta y como.
    private var toque: UITouch?
    private var toqueEsPencil = false
    /// Ya se uso el Pencil: desde ahora el dedo no pinta (la palma tampoco).
    private(set) var pencilVisto = false
    /// Lo que iOS cree que viene (en la vista), para pintarlo un fotograma.
    private var prediccion: [CGPoint] = []
    private var ultimoEnVista: CGPoint?
    /// Donde esta el Pencil (o el puntero) sin apoyar, en la vista.
    private var flotando: CGPoint?

    /// El gesto de pararse: una sola espera de una vez.
    let reloj = RelojFormaRapida(temblorPx: CGFloat(formaRapidaTemblorPx()))
    /// La raya de calibrar mientras se arrastra, en el mundo.
    private var rayaDeCalibrar: (desde: CGPoint, hasta: CGPoint)?
    /// Se esta soltando una imagen encima.
    private var soltandoImagen = false

    private let pellizco = UIPinchGestureRecognizer()
    private let arrastre = UIPanGestureRecognizer()
    private let dosDedos = UITapGestureRecognizer()
    private let tresDedos = UITapGestureRecognizer()
    private let encima = UIHoverGestureRecognizer()

    /// El dedo pinta: no hay Pencil a la vista y el sistema no pide solo Pencil.
    var dedoPinta: Bool { !(pencilVisto || UIPencilInteraction.prefersPencilOnlyDrawing) }

    public init(modelo: ModeloEditorLienzo) {
        self.modelo = modelo
        super.init(frame: CGRect(x: 0, y: 0, width: 800, height: 600))
        modelo.vista = self
        isMultipleTouchEnabled = true
        isOpaque = true
        contentMode = .redraw
        clipsToBounds = true
        backgroundColor = .white
        reloj.alPararse = { [weak self] in
            // Vence en la cola principal (`RelojFormaRapida.conDispatch`).
            MainActor.assumeIsolated { self?.alPararse() }
        }
        montarGestos()
        let lapiz = UIPencilInteraction()
        lapiz.delegate = self
        addInteraction(lapiz)
        addInteraction(UIDropInteraction(delegate: self))
        // Las fotos del lienzo: se cargan reducidas y se repinta una vez.
        let fotos = modelo.fotos
        if !fotos.isEmpty {
            Task { [weak self] in
                var alguna = false
                for url in Set(fotos.values) {
                    if await Miniaturas.compartidas.cargar(url, lado: 2048) != nil { alguna = true }
                }
                if alguna { self?.repintarFotos() }
            }
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    private func montarGestos() {
        let directo = [NSNumber(value: UITouch.TouchType.direct.rawValue)]
        pellizco.addTarget(self, action: #selector(alPellizcar(_:)))
        arrastre.addTarget(self, action: #selector(alArrastrar(_:)))
        dosDedos.addTarget(self, action: #selector(alTocarConDos(_:)))
        tresDedos.addTarget(self, action: #selector(alTocarConTres(_:)))
        encima.addTarget(self, action: #selector(alFlotar(_:)))
        dosDedos.numberOfTouchesRequired = 2
        tresDedos.numberOfTouchesRequired = 3
        arrastre.maximumNumberOfTouches = 2
        // Dos dedos en el trackpad (o la rueda): mover la vista.
        arrastre.allowedScrollTypesMask = .all
        for g in [dosDedos, tresDedos] as [UIGestureRecognizer] {
            g.allowedTouchTypes = directo
        }
        // Mover y acercar tambien con el trackpad (sus dos dedos llegan
        // como desplazamiento y pellizco, no como toques; el clic, que si es
        // un toque, pinta: ver `shouldReceive`).
        let conPuntero = directo + [NSNumber(value: UITouch.TouchType.indirectPointer.rawValue)]
        pellizco.allowedTouchTypes = conPuntero
        arrastre.allowedTouchTypes = conPuntero
        for g in [pellizco, arrastre, dosDedos, tresDedos] as [UIGestureRecognizer] {
            g.delegate = self
            // El dedo que pinta no espera a que los gestos se decidan.
            g.delaysTouchesEnded = false
            addGestureRecognizer(g)
        }
        addGestureRecognizer(encima)
        ajustarArrastre()
    }

    /// Sin Pencil el dedo pinta y se mueve con dos; con Pencil, con uno.
    private func ajustarArrastre() {
        arrastre.minimumNumberOfTouches = dedoPinta ? 2 : 1
    }

    /// Llego una foto que faltaba: se pinta otra vez.
    func repintarFotos() {
        clave = nil
        setNeedsDisplay()
    }

    override public func layoutSubviews() {
        super.layoutSubviews()
        if !encajada, bounds.width > 10, bounds.height > 10 {
            encajada = true
            encajar()
        }
        // Otro tamano (girar el iPad, Split View): se vuelve a pedir lo que
        // se ve (la clave lleva el tamano).
        setNeedsDisplay()
    }

    /// El centro de lo que se ve, en el mundo (`centro_de_la_vista`): ahi
    /// entran las figuras.
    var centroMundo: CGPoint {
        CGPoint(x: origen.x + bounds.width / zoom / 2, y: origen.y + bounds.height / zoom / 2)
    }

    /// Lo que se ve, en unidades del mundo.
    var visibleMundo: CGSize { CGSize(width: bounds.width / zoom, height: bounds.height / zoom) }

    /// Lo dibujado entero en la vista (o el origen al 100 % si esta vacio).
    /// `margenArriba`: lo que tapan las barras flotantes.
    public func encajar() {
        guard let caja = editor.caja() else {
            origen = CGPoint(x: 0, y: -VistaEditorLienzoIPad.margenArriba)
            zoom = 1
            setNeedsDisplay()
            return
        }
        let margen: CGFloat = 40
        let arriba = VistaEditorLienzoIPad.margenArriba
        let ancho = CGFloat(caja.x1 - caja.x0), alto = CGFloat(caja.y1 - caja.y0)
        let z = min((bounds.width - margen * 2) / max(ancho, 1),
                    (bounds.height - margen * 2 - arriba) / max(alto, 1), 1)
        zoom = max(VistaEditorLienzoIPad.zoomMinimo, z)
        origen = CGPoint(x: CGFloat(caja.x0) - (bounds.width / zoom - ancho) / 2,
                         y: CGFloat(caja.y0) - ((bounds.height - arriba) / zoom - alto) / 2 - arriba / zoom)
        setNeedsDisplay()
    }

    /// Lo que tapan arriba la barra del visor y la de herramientas.
    static let margenArriba: CGFloat = 110

    // MARK: Pintar

    override public func draw(_ rect: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        let version = editor.version()
        let w = bounds.width, h = bounds.height
        if let c = clave, c.version == version, c.x == origen.x, c.y == origen.y,
           c.zoom == zoom, c.w == w, c.h == h {
            // Nada cambio: lo de antes.
        } else {
            let plano = editor.planoDeVista(x: Float(origen.x), y: Float(origen.y), zoom: Float(zoom),
                                            anchoPx: Float(w), altoPx: Float(h))
            leido = try? leerLienzo(plano)
            clave = (version, origen.x, origen.y, zoom, w, h)
        }
        versionVista = version
        ctx.setFillColor(leido?.fondo.cg ?? CGColor(gray: 1, alpha: 1))
        ctx.fill(rect)
        guard var leido else { return }
        // Papel oscuro: la tinta adaptada para que se lea (el negro de la
        // letra pasa a claro), como `contraste::adaptar` de Windows.
        let p = leido.fondo
        if 0.299 * p.r + 0.587 * p.g + 0.114 * p.b < 0.5 {
            leido = leido.sobrePapel(p, cache: &coloresOscuros)
        }
        ctx.saveGState()
        ctx.scaleBy(x: zoom, y: zoom)
        ctx.translateBy(x: -origen.x, y: -origen.y)
        let visible = CGRect(x: origen.x, y: origen.y, width: w / zoom, height: h / zoom)
        let escala = zoom * traitCollection.displayScale
        PintorLienzo(lienzo: leido, imagen: modelo.imagenes(escala: escala)).pintar(en: ctx, limites: visible)
        if let raya = rayaDeCalibrar {
            // La raya de calibrar: a trazos, del grosor de un punto de pantalla.
            ctx.setStrokeColor(CGColor(red: 0.20, green: 0.45, blue: 0.90, alpha: 0.95))
            ctx.setLineWidth(1.5 / zoom)
            ctx.setLineDash(phase: 0, lengths: [6 / zoom, 4 / zoom])
            ctx.move(to: raya.desde)
            ctx.addLine(to: raya.hasta)
            ctx.strokePath()
        }
        ctx.restoreGState()
        pintarPrediccion(ctx)
        pintarFlotando(ctx)
        if soltandoImagen {
            ctx.setStrokeColor(tintColor.cgColor)
            ctx.setLineWidth(3)
            ctx.stroke(bounds.insetBy(dx: 1.5, dy: 1.5))
        }
    }

    /// El grosor aproximado de la tinta del lapiz, en el mundo (solo para
    /// la prediccion y el puntito: lo de verdad lo pinta el motor).
    private var grosorDeTinta: CGFloat {
        switch modelo.estilo.grosor {
        case .fino: return 2
        case .medio: return 3.5
        case .grueso: return 6
        }
    }

    private var colorDeTinta: CGColor {
        let c = modelo.estilo.trazo
        return CGColor(srgbRed: CGFloat(c.r), green: CGFloat(c.g), blue: CGFloat(c.b), alpha: CGFloat(c.a))
    }

    /// Lo que iOS predice del trazo a mano, de lo ultimo a la punta.
    private func pintarPrediccion(_ ctx: CGContext) {
        guard toque != nil, modelo.herramienta == "lapiz", let desde = ultimoEnVista, !prediccion.isEmpty else { return }
        ctx.saveGState()
        ctx.setStrokeColor(colorDeTinta)
        ctx.setLineWidth(grosorDeTinta * zoom)
        ctx.setLineCap(.round)
        ctx.setLineJoin(.round)
        ctx.move(to: desde)
        for p in prediccion { ctx.addLine(to: p) }
        ctx.strokePath()
        ctx.restoreGState()
    }

    /// El puntito (o el circulo de la goma) donde caeria el Pencil.
    private func pintarFlotando(_ ctx: CGContext) {
        guard toque == nil, let p = flotando else { return }
        ctx.saveGState()
        if modelo.herramienta == "borrador" {
            let r: CGFloat = 10
            let c = CGRect(x: p.x - r, y: p.y - r, width: r * 2, height: r * 2)
            ctx.setStrokeColor(UIColor.white.cgColor)
            ctx.setLineWidth(3)
            ctx.strokeEllipse(in: c)
            ctx.setStrokeColor(UIColor.darkGray.cgColor)
            ctx.setLineWidth(1)
            ctx.strokeEllipse(in: c)
        } else if !HerramientaLienzo.sinEstilo.contains(modelo.herramienta) {
            let r = max(2, grosorDeTinta * zoom / 2)
            let c = CGRect(x: p.x - r, y: p.y - r, width: r * 2, height: r * 2)
            ctx.setFillColor(UIColor.white.withAlphaComponent(0.8).cgColor)
            ctx.fillEllipse(in: c.insetBy(dx: -1.5, dy: -1.5))
            ctx.setFillColor(colorDeTinta)
            ctx.fillEllipse(in: c)
        }
        ctx.restoreGState()
    }

    /// **La estela del laser se apaga sola**: mientras el motor diga que se
    /// anima, se repinta a cada fotograma aunque la version no cambie.
    private var reloj60: CADisplayLink?

    private func seguirAnimando() {
        guard reloj60 == nil, editor.animando() else { return }
        let enlace = CADisplayLink(target: self, selector: #selector(fotograma(_:)))
        enlace.add(to: .main, forMode: .common)
        reloj60 = enlace
    }

    @objc private func fotograma(_ enlace: CADisplayLink) {
        clave = nil
        setNeedsDisplay()
        if !editor.animando() {
            enlace.invalidate()
            reloj60 = nil
        }
    }

    override public func removeFromSuperview() {
        reloj60?.invalidate()
        reloj60 = nil
        super.removeFromSuperview()
    }

    /// Tras algo que pudo cambiar el dibujo: repinta si cambio la version y
    /// avisa al modelo (guardar, la barra).
    func trasCambio(version: UInt64? = nil) {
        let v = version ?? editor.version()
        if v != versionVista {
            setNeedsDisplay()
        }
        // Lo elegido o el texto pudieron cambiar: la barra lo ensena.
        modelo.refrescarBarra()
        modelo.revisarCambios()
        ajustarTeclado()
    }

    /// El teclado sale mientras se escribe un texto y se va al terminar.
    private func ajustarTeclado() {
        let escribe = editor.escribiendo()
        if escribe, !isFirstResponder {
            becomeFirstResponder()
        } else if !escribe, isFirstResponder {
            resignFirstResponder()
        }
    }

    /// La espera del gesto de pararse vencio con el Pencil aun apoyado.
    private func alPararse() {
        if editor.formaRapida() { setNeedsDisplay() }
    }

    /// Tras soltar con una herramienta de medir: el cajetin que pida; con
    /// la zona, su foto.
    private func atenderPeticion() {
        guard ["escalar", "cota", "zona"].contains(modelo.herramienta),
              let peticion = editor.tomarPeticion() else { return }
        let editor = self.editor
        switch peticion {
        case let .zona(x0, y0, x1, y1):
            sacarZona(CGRect(x: CGFloat(x0), y: CGFloat(y0), width: CGFloat(x1 - x0), height: CGFloat(y1 - y0)))
        case let .calibrar(largoPx):
            CajetinesLienzo.calibrar(unidades: editor.unidadesDeMedida(), desde: self) { [weak self] valor, unidad in
                if editor.calibrar(largoPx: largoPx, valor: valor, unidad: unidad) { self?.trasCambio() }
            }
        case let .dictarCota(id, largo, grados, unidad):
            CajetinesLienzo.dictarCota(largo: largo, grados: grados, unidad: unidad, desde: self) { [weak self] l, g in
                if editor.dictarCota(id: id, largo: l, grados: g) { self?.trasCambio() }
            }
        }
    }

    /// **La foto de una zona** (F8): lo que hay dentro de `caja`, a la
    /// escala de la foto de Windows, recortada en redondo y con su filo;
    /// entra encima, corrida y elegida.
    private func sacarZona(_ caja: CGRect) {
        let k = min(4, max(1, 1600 / max(caja.width, caja.height, 1)))
        let plano = editor.planoDeVista(x: Float(caja.minX), y: Float(caja.minY), zoom: Float(k),
                                        anchoPx: Float(caja.width * k), altoPx: Float(caja.height * k))
        guard let img = PintorDeHojas.imagen(plano, caja: caja, escala: k, transparente: false,
                                             tapados: modelo.tapados, version: editor.version(),
                                             fotos: { [weak self] in self?.modelo.foto($0) }),
              let redonda = PintorDeHojas.enRedondo(img, papelDeNoche: modelo.papelOscuro),
              let png = PintorDeHojas.png(redonda),
              let foto = try? editor.ponerCopiaDeZona(png: png, x0: Float(caja.minX), y0: Float(caja.minY),
                                                      x1: Float(caja.maxX), y1: Float(caja.maxY), zoom: Float(zoom))
        else {
            UINotificationFeedbackGenerator().notificationOccurred(.error)
            return
        }
        let url = URL(fileURLWithPath: foto.ruta)
        modelo.anadirFoto(foto.idObjeto, url)
        trasCambio()
        Task { [weak self] in
            if await Miniaturas.compartidas.cargar(url, lado: 2048) != nil { self?.repintarFotos() }
        }
    }

    // MARK: Pencil y dedos

    private func alMundo(_ p: CGPoint) -> CGPoint {
        CGPoint(x: origen.x + p.x / zoom, y: origen.y + p.y / zoom)
    }

    /// Donde esta el toque en la vista: el Pencil con su precision fina.
    private func enVista(_ t: UITouch) -> CGPoint {
        t.type == .pencil ? t.preciseLocation(in: self) : t.location(in: self)
    }

    private func modificadores(_ evento: UIEvent?) -> UInt32 {
        var m: UInt32 = 0
        guard let f = evento?.modifierFlags else { return 0 }
        if f.contains(.shift) { m |= 1 }
        if f.contains(.alternate) { m |= 2 }
        return m
    }

    /// La presion del Pencil (0...1; 1 de `force` es un toque normal, que
    /// queda en la mitad). El grafito tumbado pinta mas claro. El dedo y el
    /// raton: ninguna (el motor hace la suya).
    private func presion(_ t: UITouch) -> Float? {
        guard t.type == .pencil else { return nil }
        var p = t.force / 2
        if modelo.herramienta == "grafito" {
            p *= 0.55 + 0.45 * sin(t.altitudeAngle)
        }
        return Float(min(1, max(0.02, p)))
    }

    private func mandar(_ fase: FasePuntero, _ t: UITouch, _ evento: UIEvent?) -> UInt64 {
        let p = alMundo(enVista(t))
        return editor.puntero(fase: fase, x: Float(p.x), y: Float(p.y), presion: presion(t),
                              modificadores: modificadores(evento), zoom: Float(zoom))
    }

    private func empezar(_ t: UITouch, _ evento: UIEvent?, pencil: Bool) {
        toque = t
        toqueEsPencil = pencil
        prediccion = []
        flotando = nil
        if modelo.desplegado != nil { modelo.desplegado = nil }
        // Un toque fuera del texto lo termina (lo hace el motor al pulsar).
        let v = mandar(.pulsar, t, evento)
        trasCambio(version: v)
        let p = enVista(t)
        ultimoEnVista = p
        // El gesto de pararse, solo trazando a mano (una pregunta por trazo).
        if HerramientaLienzo.aMano.contains(modelo.herramienta) {
            reloj.pulsar(en: p, esperaMs: editor.esperaFormaMs())
        } else {
            reloj.soltar()
        }
        if modelo.herramienta == "escalar" {
            let m = alMundo(p)
            rayaDeCalibrar = (m, m)
        }
        seguirAnimando()
    }

    /// Se deja lo que iba a medias (un segundo dedo, el sistema se llevo el
    /// toque, llego el Pencil mientras pintaba el dedo).
    private func cancelarTrazo() {
        guard toque != nil else { return }
        toque = nil
        prediccion = []
        reloj.soltar()
        rayaDeCalibrar = nil
        let v = editor.puntero(fase: .cancelar, x: 0, y: 0, presion: nil, modificadores: 0, zoom: Float(zoom))
        trasCambio(version: v)
        setNeedsDisplay()
    }

    override public func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        for t in touches {
            switch t.type {
            case .pencil:
                if !pencilVisto {
                    pencilVisto = true
                    ajustarArrastre()
                }
                // El Pencil manda: lo que pintara el dedo se deja, y la vista
                // deja de moverse (la palma apoyada antes que la punta).
                if toque != nil, !toqueEsPencil { cancelarTrazo() }
                pararGestosDeDedo()
                if toque == nil { empezar(t, event, pencil: true) }
            case .indirectPointer:
                // El clic del raton o del trackpad pinta como el Pencil.
                if toque == nil { empezar(t, event, pencil: true) }
            default:
                if let actual = toque, !toqueEsPencil, actual !== t {
                    // Un segundo dedo: era mover o acercar, no pintar.
                    cancelarTrazo()
                } else if toque == nil, dedoPinta,
                          (event?.allTouches?.filter { $0.type == .direct && $0.phase != .ended }.count ?? 1) <= 1 {
                    empezar(t, event, pencil: false)
                }
            }
        }
    }

    override public func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = toque, touches.contains(t) else { return }
        let muestras = event?.coalescedTouches(for: t) ?? [t]
        var v = versionVista
        for m in muestras { v = mandar(.mover, m, event) }
        let p = enVista(t)
        ultimoEnVista = p
        prediccion = (event?.predictedTouches(for: t) ?? []).map { enVista($0) }
        reloj.mover(a: p)
        if let raya = rayaDeCalibrar {
            rayaDeCalibrar = (raya.desde, alMundo(p))
        }
        if v != versionVista || !prediccion.isEmpty || rayaDeCalibrar != nil { setNeedsDisplay() }
        seguirAnimando()
    }

    override public func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = toque, touches.contains(t) else { return }
        reloj.soltar()
        let v = mandar(.soltar, t, event)
        toque = nil
        prediccion = []
        ultimoEnVista = nil
        if rayaDeCalibrar != nil {
            rayaDeCalibrar = nil
            setNeedsDisplay()
        }
        trasCambio(version: v)
        atenderPeticion()
        seguirAnimando()
    }

    override public func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let t = toque, touches.contains(t) else { return }
        cancelarTrazo()
    }

    /// Los gestos de dedo que estuvieran moviendo la vista se paran.
    private func pararGestosDeDedo() {
        for g in [arrastre, pellizco] as [UIGestureRecognizer] where g.state == .began || g.state == .changed {
            g.isEnabled = false
            g.isEnabled = true
        }
    }

    // MARK: Mover y acercar la vista

    private func desplazar(dx: CGFloat, dy: CGFloat) {
        origen.x -= dx / zoom
        origen.y -= dy / zoom
        setNeedsDisplay()
    }

    /// Acerca `factor` veces dejando quieto el punto de la vista `foco`.
    private func acercar(_ factor: CGFloat, en foco: CGPoint) {
        let nuevo = max(VistaEditorLienzoIPad.zoomMinimo, min(VistaEditorLienzoIPad.zoomMaximo, zoom * factor))
        guard nuevo != zoom, nuevo.isFinite else { return }
        let mundo = alMundo(foco)
        zoom = nuevo
        origen = CGPoint(x: mundo.x - foco.x / zoom, y: mundo.y - foco.y / zoom)
        setNeedsDisplay()
    }

    @objc private func alPellizcar(_ g: UIPinchGestureRecognizer) {
        guard g.state == .began || g.state == .changed else { return }
        if toque != nil, !toqueEsPencil { cancelarTrazo() }
        acercar(g.scale, en: g.location(in: self))
        g.scale = 1
    }

    @objc private func alArrastrar(_ g: UIPanGestureRecognizer) {
        guard g.state == .began || g.state == .changed else { return }
        if toque != nil, !toqueEsPencil { cancelarTrazo() }
        let d = g.translation(in: self)
        desplazar(dx: d.x, dy: d.y)
        g.setTranslation(.zero, in: self)
    }

    @objc private func alTocarConDos(_ g: UITapGestureRecognizer) {
        guard g.state == .ended else { return }
        modelo.deshacer()
    }

    @objc private func alTocarConTres(_ g: UITapGestureRecognizer) {
        guard g.state == .ended else { return }
        modelo.rehacer()
    }

    @objc private func alFlotar(_ g: UIHoverGestureRecognizer) {
        switch g.state {
        case .began, .changed:
            flotando = g.location(in: self)
        default:
            flotando = nil
        }
        setNeedsDisplay()
    }

    public func gestureRecognizerShouldBegin(_ g: UIGestureRecognizer) -> Bool {
        // Con el Pencil apoyado, la palma no mueve la vista.
        if g === encima { return true }
        return !(toque != nil && toqueEsPencil)
    }

    public func gestureRecognizer(_ g: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        // El clic del trackpad o del raton pinta; nunca mueve la vista.
        touch.type != .indirectPointer
    }

    public func gestureRecognizer(_ g: UIGestureRecognizer,
                                  shouldRecognizeSimultaneouslyWith otro: UIGestureRecognizer) -> Bool {
        // Pellizcar y mover a la vez, con los mismos dos dedos.
        (g === pellizco && otro === arrastre) || (g === arrastre && otro === pellizco)
    }

    // MARK: Doble toque y apretar el Pencil

    public func pencilInteraction(_ interaction: UIPencilInteraction,
                                  didReceiveTap tap: UIPencilInteraction.Tap) {
        switch UIPencilInteraction.preferredTapAction {
        case .ignore: return
        case .switchPrevious: modelo.alternarAnterior()
        default: modelo.alternarGoma()
        }
    }

    public func pencilInteraction(_ interaction: UIPencilInteraction,
                                  didReceiveSqueeze squeeze: UIPencilInteraction.Squeeze) {
        guard squeeze.phase == .ended, UIPencilInteraction.preferredSqueezeAction != .ignore else { return }
        modelo.panelEscondido.toggle()
    }

    // MARK: Teclado (el texto del lienzo)

    override public var canBecomeFirstResponder: Bool { editor.escribiendo() }

    public var hasText: Bool { true }

    public func insertText(_ text: String) {
        guard editor.escribiendo() else { return }
        if text == "\n" {
            if editor.tecla(nombre: "entrar") { trasCambio() }
        } else if editor.teclear(texto: text) {
            trasCambio()
        }
    }

    public func deleteBackward() {
        if editor.tecla(nombre: "retroceso") { trasCambio() }
    }

    public var autocorrectionType: UITextAutocorrectionType = .no
    public var spellCheckingType: UITextSpellCheckingType = .no
    public var smartQuotesType: UITextSmartQuotesType = .no
    public var smartDashesType: UITextSmartDashesType = .no

    /// Las flechas, Inicio, Fin, Suprimir y Esc del teclado fisico.
    override public func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        var usadas = false
        if editor.escribiendo() {
            for p in presses {
                guard let k = p.key else { continue }
                let nombre: String?
                switch k.keyCode {
                case .keyboardLeftArrow: nombre = "izquierda"
                case .keyboardRightArrow: nombre = "derecha"
                case .keyboardHome: nombre = "inicio"
                case .keyboardEnd: nombre = "fin"
                case .keyboardDeleteForward: nombre = "suprimir"
                case .keyboardEscape: nombre = "escape"
                default: nombre = nil
                }
                if let nombre {
                    usadas = true
                    if editor.tecla(nombre: nombre) { trasCambio() }
                }
            }
        }
        if !usadas { super.pressesBegan(presses, with: event) }
    }

    /// Termina el texto que se estaba escribiendo (otra herramienta, cerrar).
    func terminarTexto() {
        if editor.terminarTexto() { trasCambio() }
        ajustarTeclado()
    }

    // MARK: Pegar y soltar imagenes

    /// ⌘V: una imagen del portapapeles, en el centro de lo que se ve.
    func pegarDelPortapapeles() {
        guard !editor.escribiendo() else { return }
        let tablero = UIPasteboard.general
        if let datos = tablero.data(forPasteboardType: UTType.png.identifier) {
            pegar(.datos(datos), en: centroMundo)
        } else if let img = tablero.image, let datos = img.pngData() {
            pegar(.datos(datos), en: centroMundo)
        } else if let url = tablero.url, url.isFileURL {
            pegar(.fichero(url), en: centroMundo)
        } else {
            UINotificationFeedbackGenerator().notificationOccurred(.error)
        }
    }

    /// El boton de imagen de la barra (una foto de Fotos), en el centro.
    func meterImagen(_ datos: Data) {
        pegar(.datos(datos), en: centroMundo)
    }

    /// Convierte a PNG fuera del hilo principal y lo pega centrado en
    /// `centro` (mundo); despues lee su miniatura y repinta.
    private func pegar(_ fuente: ImagenPegadaLienzo.Origen, en centro: CGPoint) {
        let editor = self.editor
        let ancho = Float(bounds.width / zoom), alto = Float(bounds.height / zoom)
        Task { [weak self] in
            let hecho = await Task.detached(priority: .userInitiated) { () -> ImagenDelLienzo? in
                guard let png = ImagenPegadaLienzo.png(de: fuente) else { return nil }
                return try? editor.pegarImagen(png: png, centroX: Float(centro.x), centroY: Float(centro.y),
                                               anchoVista: ancho, altoVista: alto)
            }.value
            guard let self else { return }
            guard let foto = hecho else {
                UINotificationFeedbackGenerator().notificationOccurred(.error)
                return
            }
            let url = URL(fileURLWithPath: foto.ruta)
            self.modelo.anadirFoto(foto.idObjeto, url)
            self.trasCambio()
            if await Miniaturas.compartidas.cargar(url, lado: 2048) != nil { self.repintarFotos() }
        }
    }

    public func dropInteraction(_ interaction: UIDropInteraction, canHandle session: UIDropSession) -> Bool {
        session.hasItemsConforming(toTypeIdentifiers: [UTType.image.identifier])
    }

    public func dropInteraction(_ interaction: UIDropInteraction,
                                sessionDidUpdate session: UIDropSession) -> UIDropProposal {
        if !soltandoImagen {
            soltandoImagen = true
            setNeedsDisplay()
        }
        return UIDropProposal(operation: .copy)
    }

    public func dropInteraction(_ interaction: UIDropInteraction, sessionDidExit session: UIDropSession) {
        soltandoImagen = false
        setNeedsDisplay()
    }

    public func dropInteraction(_ interaction: UIDropInteraction, sessionDidEnd session: UIDropSession) {
        soltandoImagen = false
        setNeedsDisplay()
    }

    public func dropInteraction(_ interaction: UIDropInteraction, performDrop session: UIDropSession) {
        soltandoImagen = false
        setNeedsDisplay()
        var centro = alMundo(session.location(in: self))
        for item in session.items {
            let proveedor = item.itemProvider
            guard proveedor.hasItemConformingToTypeIdentifier(UTType.image.identifier) else { continue }
            let aqui = centro
            _ = proveedor.loadDataRepresentation(forTypeIdentifier: UTType.image.identifier) { datos, _ in
                guard let datos else { return }
                Task { @MainActor [weak self] in self?.pegar(.datos(datos), en: aqui) }
            }
            // Varias a la vez: una un poco corrida de la otra.
            centro.x += 24 / zoom
            centro.y += 24 / zoom
        }
    }
}
