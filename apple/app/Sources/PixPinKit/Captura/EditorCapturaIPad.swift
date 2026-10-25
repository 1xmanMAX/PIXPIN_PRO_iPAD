// El editor de captura del iPad, a pantalla completa (el de Windows tras
// elegir la zona, `captura-despues.png`): arriba el tamano y la barra de
// anotar (lapiz, flecha, rectangulo, texto y mosaico; cinco colores, tres
// grosores, deshacer y rehacer), en medio la foto y abajo lo que se hace con
// ella: **Copiar, Pinear, Al chat, Texto, Guardar** y Descartar.
//
// Se dibuja con el dedo o con el Apple Pencil. Todo lo anotado se **quema en
// la imagen** al elegir que hacer con ella (`PintorMarcas.quemar`), fuera del
// hilo principal.
//
// Con el teclado del iPad, los atajos de Windows con ⌘ (para no quitarle
// teclas a una caja de texto): ⌘1-⌘5 herramientas, ⌘Z y
// ⇧⌘Z, Intro copia, ⌘P pinea, ⌘M al chat, ⌘T texto, ⌘S guarda, Esc descarta.
//
// **Recortar** (lo que en Windows es elegir la zona, aqui despues de la
// foto): arrastrar marca la zona (o la mueve si se empieza dentro), con
// proporciones 16:9 · 4:3 · 1:1, las medidas exactas en An y Al y
// «Repetir la ultima zona» (`R`); Intro recorta, Esc vuelve. Lo anotado se
// recorta con la foto. **Lupa**: con el cuentagotas, o con el Apple Pencil
// por encima sin tocar (o el trackpad), sale la lupa con el color en HEX o
// RGB; `C` lo copia. **Copiar** apila la captura (`PilaCapturasIPad`).

import SwiftUI
import UIKit

/// Para que se abre el editor.
enum ModoEditorCaptura {
    /// Una captura nueva (foto, escaner, Fotos, pegar, la app).
    case captura
    /// Anotar la foto de un pin: «Listo» la devuelve al pin.
    case anotarPin((CGImage) -> Void)

    var esPin: Bool {
        if case .anotarPin = self { return true }
        return false
    }
}

struct EditorCapturaIPad: View {
    let imagen: CGImage
    let modo: ModoEditorCaptura
    let cerrar: () -> Void

    @State private var herramienta: HerramientaCaptura = .flecha
    @State private var color: UInt32 = ColoresCaptura.todos[0]
    @State private var grosor: CGFloat = GrosoresCaptura.todos[1]
    @State private var marcas: [MarcaCaptura] = []
    @State private var deshechas: [MarcaCaptura] = []
    @State private var enCurso: MarcaCaptura?
    @State private var pixelada: CGImage?
    /// El sitio (en la foto) donde se esta escribiendo un texto.
    @State private var sitioTexto: CGPoint?
    @State private var textoNuevo = ""
    @State private var pidiendoTexto = false
    @State private var trabajando = false
    /// El factor de la vista cuando se toco para escribir (para el grosor).
    @State private var factorTexto: CGFloat = 1
    @State private var alChat: ImagenSuelta?
    @State private var textoDe: ImagenSuelta?

    /// La foto ya recortada (nil: la entera).
    @State private var recortada: CGImage?
    /// Lo que se quito por arriba y por la izquierda (para volver a la entera).
    @State private var origenRecorte: CGPoint = .zero
    /// Cuantas veces se recorto (para rehacer el mosaico).
    @State private var recortes = 0
    @State private var modoEdicion: ModoEdicionCaptura = .anotar
    /// La zona que se va a recortar, en pixeles de la foto.
    @State private var zona: CGRect?
    @State private var proporcion: ProporcionCaptura = .libre
    /// La zona al empezar a moverla (si el dedo empezo dentro).
    @State private var zonaAlEmpezar: CGRect?
    /// Si el arrastre de ahora mueve la zona (nil: aun no se sabe).
    @State private var moviendoZona: Bool?
    @State private var anchoTexto = ""
    @State private var altoTexto = ""
    /// Donde mira la lupa (pixeles de la foto) y si va en HEX.
    @State private var puntoLupa: CGPoint?
    @State private var lupaEnHex = true

    @ObservedObject private var capturador = CapturadorIPad.compartido

    /// La foto con la que se trabaja (la recortada, si se recorto).
    private var foto: CGImage { recortada ?? imagen }
    private var tamanoFoto: CGSize { CGSize(width: foto.width, height: foto.height) }
    private var limitesFoto: CGRect { CGRect(origin: .zero, size: tamanoFoto) }

    var body: some View {
        VStack(spacing: 0) {
            barraAnotar
                .padding(.top, 8)
            GeometryReader { geo in
                lienzo(en: geo.size)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            Group {
                if modoEdicion == .recortar { barraRecorte } else { barraAcciones }
            }
            .padding(.bottom, 10)
        }
        .background(Color(hex: 0x14141A).ignoresSafeArea())
        .environment(\.colorScheme, .dark)
        .foregroundStyle(Paleta.v2.texto)
        .overlay {
            if trabajando { ProgressView().controlSize(.large) }
        }
        .task(id: recortes) {
            let base = foto
            pixelada = nil
            pixelada = await ImagenIPad.fuera { PintorMarcas.pixelar(base) }
        }
        .onChange(of: zona) { _, nueva in
            guard let z = nueva.map({ RecorteCaptura.enPixeles($0, ancho: foto.width, alto: foto.height) }),
                  RecorteCaptura.valida(z) else { return }
            anchoTexto = "\(Int(z.width))"
            altoTexto = "\(Int(z.height))"
        }
        .background {
            // `C` copia el color de la lupa.
            Button("") { copiarColor() }
                .keyboardShortcut("c", modifiers: [])
                .disabled(puntoLupa == nil || pidiendoTexto)
                .opacity(0)
        }
        .alert("Texto", isPresented: $pidiendoTexto) {
            TextField("Escribe aquí", text: $textoNuevo)
            Button("Cancelar", role: .cancel) { sitioTexto = nil }
            Button("Poner") { ponerTexto() }
        }
        .sheet(item: $alChat) { suelta in
            if let almacen = capturador.almacen {
                VistaSelectorProyecto(almacen: almacen) { chat, comentario in
                    alChat = nil
                    guard let chat else { return }
                    let quemada = suelta.imagen
                    Task {
                        await SelectorProyectoIPad.mandar(quemada, a: chat, comentario: comentario, almacen: almacen)
                    }
                    cerrar()
                }
            }
        }
        .sheet(item: $textoDe) { suelta in
            VistaTextoSacado(imagen: suelta.imagen) { textoDe = nil }
        }
    }

    // MARK: La foto y el dibujo

    private func lienzo(en espacio: CGSize) -> some View {
        let f = min(espacio.width / max(tamanoFoto.width, 1), espacio.height / max(tamanoFoto.height, 1), 4)
        let vista = CGSize(width: tamanoFoto.width * f, height: tamanoFoto.height * f)
        let todas = marcas + (enCurso.map { [$0] } ?? [])
        let pix = pixelada
        let tam = tamanoFoto
        return ZStack {
            Image(decorative: foto, scale: 1)
                .resizable()
                .interpolation(.high)
            Canvas { ctx, _ in
                ctx.scaleBy(x: f, y: f)
                ctx.withCGContext { cg in
                    PintorMarcas.pintar(todas, en: cg, tamano: tam, pixelada: pix)
                }
            }
            .allowsHitTesting(false)
            if let sitioTexto {
                Image(systemName: "character.cursor.ibeam")
                    .foregroundStyle(ColoresCaptura.colorSwiftUI(color))
                    .position(x: sitioTexto.x * f, y: sitioTexto.y * f)
            }
            if modoEdicion == .recortar {
                velo(zona: zona, factor: f, vista: vista)
            }
        }
        .frame(width: vista.width, height: vista.height)
        .overlay(Rectangle().strokeBorder(Paleta.v2.acento, lineWidth: 1))
        .contentShape(Rectangle())
        .gesture(gestoLienzo(factor: f))
        .onContinuousHover(coordinateSpace: .local) { fase in
            // El Apple Pencil por encima (o el trackpad): la lupa.
            switch fase {
            case let .active(p):
                // Al anotar no: el Pencil por encima ya ensena donde pinta.
                guard modoEdicion != .anotar else { return }
                puntoLupa = CGPoint(x: p.x / f, y: p.y / f)
            case .ended:
                if modoEdicion != .lupa { puntoLupa = nil }
            }
        }
        .overlay(alignment: .topLeading) {
            if let p = puntoLupa {
                lupa(en: p, factor: f, vista: vista)
            }
        }
        .frame(width: espacio.width, height: espacio.height)
    }

    /// La lupa, al lado del punto (arriba a la derecha; si no cabe, al otro
    /// lado) para que el dedo no la tape.
    private func lupa(en p: CGPoint, factor f: CGFloat, vista: CGSize) -> some View {
        let lado = VistaLupaCaptura.lado
        let px = p.x * f, py = p.y * f
        var x = px + 30
        if x + lado > vista.width + 40 { x = px - 30 - lado }
        var y = py - 30 - lado - 50
        if y < -40 { y = py + 30 }
        return VistaLupaCaptura(imagen: foto, punto: p, enHex: $lupaEnHex) { copiarColor() }
            .frame(width: lado + 60)
            .offset(x: x - 30, y: y)
            .allowsHitTesting(modoEdicion == .lupa)
    }

    /// Lo de fuera de la zona, oscurecido, y la zona con su borde azul y sus
    /// medidas.
    @ViewBuilder
    private func velo(zona: CGRect?, factor f: CGFloat, vista: CGSize) -> some View {
        if let z = zona.map({ RecorteCaptura.enPixeles($0, ancho: foto.width, alto: foto.height) }), RecorteCaptura.valida(z) {
            let r = CGRect(x: z.minX * f, y: z.minY * f, width: z.width * f, height: z.height * f)
            Path { camino in
                camino.addRect(CGRect(origin: .zero, size: vista))
                camino.addRect(r)
            }
            .fill(Color.black.opacity(0.55), style: FillStyle(eoFill: true))
            .allowsHitTesting(false)
            Rectangle()
                .strokeBorder(Paleta.v2.acento, lineWidth: 2)
                .frame(width: r.width, height: r.height)
                .position(x: r.midX, y: r.midY)
                .allowsHitTesting(false)
            Text("\(Int(z.width)) × \(Int(z.height))")
                .font(.caption.monospacedDigit().weight(.semibold))
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(Paleta.v2.acento, in: RoundedRectangle(cornerRadius: 4))
                .foregroundStyle(.white)
                .position(x: min(max(r.minX + 44, 44), vista.width - 44), y: max(r.minY - 14, 12))
                .allowsHitTesting(false)
        } else {
            Color.black.opacity(0.35).allowsHitTesting(false)
        }
    }

    private func gestoLienzo(factor f: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: 0, coordinateSpace: .local)
            .onChanged { v in
                switch modoEdicion {
                case .anotar: dibujarCambio(v, factor: f)
                case .recortar: recortarCambio(v, factor: f)
                case .lupa: puntoLupa = CGPoint(x: v.location.x / f, y: v.location.y / f)
                }
            }
            .onEnded { v in
                switch modoEdicion {
                case .anotar: dibujarFin(v, factor: f)
                case .recortar:
                    zonaAlEmpezar = nil
                    moviendoZona = nil
                    puntoLupa = nil
                case .lupa: puntoLupa = CGPoint(x: v.location.x / f, y: v.location.y / f)
                }
            }
    }

    // MARK: Recortar

    private func recortarCambio(_ v: DragGesture.Value, factor f: CGFloat) {
        let inicio = CGPoint(x: v.startLocation.x / f, y: v.startLocation.y / f)
        let p = CGPoint(x: min(max(v.location.x / f, 0), tamanoFoto.width),
                        y: min(max(v.location.y / f, 0), tamanoFoto.height))
        puntoLupa = p
        if moviendoZona == nil {
            // Se decide al empezar: dentro de la zona la mueve; fuera, otra.
            if let z = zona, RecorteCaptura.valida(z), z.contains(inicio) {
                moviendoZona = true
                zonaAlEmpezar = z
            } else {
                moviendoZona = false
            }
        }
        if moviendoZona == true, let base = zonaAlEmpezar {
            // Empezo dentro: se mueve entera.
            zona = RecorteCaptura.deslizar(base, dx: v.translation.width / f, dy: v.translation.height / f,
                                           dentroDe: limitesFoto)
            return
        }
        let r = RecorteCaptura.atado(desde: inicio, hasta: p, proporcion: proporcion)
        zona = RecorteCaptura.dentro(r, de: limitesFoto)
    }

    private func entrarARecortar() {
        puntoLupa = nil
        enCurso = nil
        modoEdicion = .recortar
        if zona == nil, proporcion != .libre {
            zona = RecorteCaptura.ajustar(nil, a: proporcion, limites: limitesFoto)
        }
    }

    private func salirDeRecortar() {
        modoEdicion = .anotar
        zona = nil
        zonaAlEmpezar = nil
        moviendoZona = nil
        puntoLupa = nil
    }

    private func elegirProporcion(_ p: ProporcionCaptura) {
        proporcion = p
        if p != .libre { zona = RecorteCaptura.ajustar(zona, a: p, limites: limitesFoto) }
    }

    private func ponerMedidas() {
        guard let a = RecorteCaptura.numero(anchoTexto), let b = RecorteCaptura.numero(altoTexto) else {
            return avisarFalloPin()
        }
        proporcion = .libre
        zona = RecorteCaptura.zona(ancho: a, alto: b, seleccion: zona, limites: limitesFoto)
    }

    private func repetirUltimaZona() {
        guard let r = UltimaZonaCaptura().leer(ancho: foto.width, alto: foto.height) else { return avisarFalloPin() }
        proporcion = .libre
        zona = r
    }

    /// Recorta la foto (y lo anotado) a la zona.
    private func recortar() {
        guard let z = zona.map({ RecorteCaptura.enPixeles($0, ancho: foto.width, alto: foto.height) }),
              RecorteCaptura.valida(z), z.size != tamanoFoto,
              let nueva = foto.cropping(to: z) else {
            salirDeRecortar()
            return
        }
        UltimaZonaCaptura().guardar(z)
        marcas = RecorteCaptura.moverMarcas(marcas, recorte: z)
        deshechas = RecorteCaptura.moverMarcas(deshechas, recorte: z)
        origenRecorte = CGPoint(x: origenRecorte.x + z.minX, y: origenRecorte.y + z.minY)
        recortada = nueva
        recortes += 1
        salirDeRecortar()
    }

    /// Vuelve a la foto entera (lo anotado vuelve a su sitio).
    private func volverAEntera() {
        guard recortada != nil else { return }
        let atras = CGRect(x: -origenRecorte.x, y: -origenRecorte.y, width: 1, height: 1)
        marcas = RecorteCaptura.moverMarcas(marcas, recorte: atras)
        deshechas = RecorteCaptura.moverMarcas(deshechas, recorte: atras)
        origenRecorte = .zero
        recortada = nil
        recortes += 1
        zona = nil
    }

    // MARK: Lupa

    private func copiarColor() {
        guard let p = puntoLupa, let px = RecorteCaptura.pixel(en: p, ancho: foto.width, alto: foto.height),
              let c = ColorPixel.en(foto, x: px.x, y: px.y) else { return }
        UIPasteboard.general.string = lupaEnHex ? c.hex : c.rgb
        UINotificationFeedbackGenerator().notificationOccurred(.success)
    }

    // MARK: Anotar

    private func dibujarCambio(_ v: DragGesture.Value, factor f: CGFloat) {
                let p = CGPoint(x: v.location.x / f, y: v.location.y / f)
                guard herramienta != .texto else { return }
                if var m = enCurso {
                    if herramienta == .lapiz {
                        m.puntos.append(p)
                    } else {
                        m.puntos = [m.puntos.first ?? p, p]
                    }
                    enCurso = m
                } else {
                    let inicio = CGPoint(x: v.startLocation.x / f, y: v.startLocation.y / f)
                    enCurso = MarcaCaptura(herramienta: herramienta, color: color, grosor: grosor / f,
                                           puntos: [inicio, p])
                }
    }

    private func dibujarFin(_ v: DragGesture.Value, factor f: CGFloat) {
                let p = CGPoint(x: v.location.x / f, y: v.location.y / f)
                if herramienta == .texto {
                    sitioTexto = p
                    factorTexto = f
                    textoNuevo = ""
                    pidiendoTexto = true
                    return
                }
                if let m = enCurso, m.tieneCuerpo {
                    marcas.append(m)
                    deshechas = []
                }
                enCurso = nil
    }

    private func ponerTexto() {
        defer { sitioTexto = nil }
        guard let sitio = sitioTexto else { return }
        // El grosor en la foto: el de la pantalla pasado a pixeles de la foto.
        let f = max(factorTexto, 0.01)
        let m = MarcaCaptura(herramienta: .texto, color: color, grosor: grosor / f, puntos: [sitio], texto: textoNuevo)
        guard m.tieneCuerpo else { return }
        marcas.append(m)
        deshechas = []
    }

    // MARK: Barras

    private var barraAnotar: some View {
        HStack(spacing: 6) {
            Text("\(foto.width) × \(foto.height)")
                .font(.callout.monospacedDigit())
                .foregroundStyle(Paleta.v2.gris)
                .padding(.horizontal, 8)
            separador
            ForEach(HerramientaCaptura.allCases) { h in
                let activa = modoEdicion == .anotar && herramienta == h
                Button {
                    if modoEdicion == .recortar { salirDeRecortar() }
                    modoEdicion = .anotar
                    puntoLupa = nil
                    herramienta = h
                } label: {
                    Image(systemName: h.icono)
                        .font(.system(size: 18, weight: .medium))
                        .frame(width: 42, height: 38)
                        .background(activa ? Paleta.v2.acento.opacity(0.25) : .clear,
                                    in: RoundedRectangle(cornerRadius: 8))
                        .overlay(RoundedRectangle(cornerRadius: 8)
                            .strokeBorder(activa ? Paleta.v2.acento : .clear, lineWidth: 1.5))
                }
                .keyboardShortcut(KeyEquivalent(h.tecla), modifiers: .command)
                .help("\(h.nombre) (⌘\(h.tecla))")
            }
            separador
            botonModo(.recortar, "crop", "Recortar (⌘6)", tecla: "6") { entrarARecortar() }
            botonModo(.lupa, "eyedropper", "Lupa: el color en HEX / RGB (⌘7)", tecla: "7") {
                if modoEdicion == .recortar { salirDeRecortar() }
                modoEdicion = .lupa
                puntoLupa = CGPoint(x: tamanoFoto.width / 2, y: tamanoFoto.height / 2)
            }
            separador
            ForEach(ColoresCaptura.todos, id: \.self) { c in
                Button { color = c } label: {
                    Circle()
                        .fill(ColoresCaptura.colorSwiftUI(c))
                        .frame(width: 26, height: 26)
                        .overlay(Circle().strokeBorder(Color.white.opacity(color == c ? 1 : 0.25), lineWidth: color == c ? 3 : 1))
                        .frame(width: 34, height: 38)
                }
            }
            separador
            ForEach(GrosoresCaptura.todos, id: \.self) { g in
                Button { grosor = g } label: {
                    Circle()
                        .fill(Paleta.v2.texto)
                        .frame(width: g + 4, height: g + 4)
                        .frame(width: 38, height: 38)
                        .background(grosor == g ? Paleta.v2.acento.opacity(0.25) : .clear, in: RoundedRectangle(cornerRadius: 8))
                }
                .help("Grosor")
            }
            separador
            Button { deshacer() } label: {
                Image(systemName: "arrow.uturn.backward").frame(width: 38, height: 38)
            }
            .keyboardShortcut("z", modifiers: .command)
            .disabled(marcas.isEmpty)
            .help("Deshacer (⌘Z)")
            Button { rehacer() } label: {
                Image(systemName: "arrow.uturn.forward").frame(width: 38, height: 38)
            }
            .keyboardShortcut("z", modifiers: [.command, .shift])
            .disabled(deshechas.isEmpty)
            .help("Rehacer (⇧⌘Z)")
        }
        .buttonStyle(.plain)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(Paleta.v2.fondo, in: RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(Paleta.v2.linea))
    }

    private var barraAcciones: some View {
        HStack(spacing: 8) {
            if case let .anotarPin(devolver) = modo {
                accion("Listo", "checkmark", lleno: true) {
                    hacer { quemada in
                        devolver(quemada)
                        cerrar()
                    }
                }
                .keyboardShortcut(.defaultAction)
                accion("Copiar", "doc.on.doc") { hacer(copiar) }
                    .keyboardShortcut("c", modifiers: .command)
            } else {
                accion("Copiar", "doc.on.doc", lleno: true) { hacer(copiar) }
                    .keyboardShortcut(.defaultAction)
                accion("Pinear", "pin") {
                    hacer { quemada in
                        cerrar()
                        GestorPinesIPad.compartido.pinear(imagen: quemada)
                    }
                }
                .keyboardShortcut("p", modifiers: .command)
            }
            accion("Al chat", "bubble.left") { hacer { alChat = ImagenSuelta($0) } }
                .keyboardShortcut("m", modifiers: .command)
                .disabled(capturador.almacen == nil)
            accion("Texto", "textformat") { hacer { textoDe = ImagenSuelta($0) } }
                .keyboardShortcut("t", modifiers: .command)
            if !modo.esPin {
                Menu {
                    Button { hacer(guardarEnGaleria) } label: {
                        Label("En la galería de capturas", systemImage: "photo.on.rectangle")
                    }
                    Button { hacer(guardarEnFotos) } label: { Label("En Fotos", systemImage: "photo") }
                } label: {
                    Label("Guardar", systemImage: "square.and.arrow.down")
                        .padding(.horizontal, 14)
                        .frame(height: 44)
                        .background(Paleta.v2.caja, in: RoundedRectangle(cornerRadius: 10))
                } primaryAction: {
                    hacer(guardarEnGaleria)
                }
                .keyboardShortcut("s", modifiers: .command)
            }
            Rectangle().fill(Paleta.v2.linea).frame(width: 1, height: 28)
            Button("Descartar") { cerrar() }
                .keyboardShortcut(.cancelAction)
                .foregroundStyle(Paleta.v2.gris)
                .padding(.horizontal, 10)
        }
        .buttonStyle(.plain)
        .font(.body.weight(.medium))
        .padding(8)
        .background(Paleta.v2.fondo, in: RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(Paleta.v2.linea))
        .disabled(trabajando)
    }

    private var separador: some View {
        Rectangle().fill(Paleta.v2.linea).frame(width: 1, height: 26).padding(.horizontal, 2)
    }

    /// Recortar y la lupa, al lado de las herramientas de anotar.
    private func botonModo(_ m: ModoEdicionCaptura, _ icono: String, _ ayuda: String, tecla: Character,
                           _ entrar: @escaping () -> Void) -> some View {
        let activo = modoEdicion == m
        return Button {
            if activo {
                if m == .recortar { salirDeRecortar() } else { modoEdicion = .anotar; puntoLupa = nil }
            } else {
                entrar()
            }
        } label: {
            Image(systemName: icono)
                .font(.system(size: 18, weight: .medium))
                .frame(width: 42, height: 38)
                .background(activo ? Paleta.v2.acento.opacity(0.25) : .clear, in: RoundedRectangle(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(activo ? Paleta.v2.acento : .clear, lineWidth: 1.5))
        }
        .keyboardShortcut(KeyEquivalent(tecla), modifiers: .command)
        .help(ayuda)
        .accessibilityLabel(ayuda)
    }

    /// La barra de recortar (la de abajo de la captura de Windows): Libre,
    /// 16:9, 4:3, 1:1, An × Al, «Repetir la ultima zona», la foto entera,
    /// Cancelar y Recortar.
    private var barraRecorte: some View {
        HStack(spacing: 8) {
            ForEach(ProporcionCaptura.allCases) { p in
                Button { elegirProporcion(p) } label: {
                    Text(p.rotulo)
                        .font(.callout.weight(.medium))
                        .padding(.horizontal, 12)
                        .frame(height: 40)
                        .background(proporcion == p ? Paleta.v2.acento.opacity(0.25) : Paleta.v2.caja,
                                    in: RoundedRectangle(cornerRadius: 9))
                        .overlay(RoundedRectangle(cornerRadius: 9)
                            .strokeBorder(proporcion == p ? Paleta.v2.acento : .clear, lineWidth: 1.5))
                }
            }
            Rectangle().fill(Paleta.v2.linea).frame(width: 1, height: 28)
            HStack(spacing: 4) {
                Text("An").foregroundStyle(Paleta.v2.gris)
                TextField("", text: $anchoTexto)
                    .keyboardType(.numberPad)
                    .frame(width: 64)
                    .onSubmit { ponerMedidas() }
                Text("×").foregroundStyle(Paleta.v2.gris)
                Text("Al").foregroundStyle(Paleta.v2.gris)
                TextField("", text: $altoTexto)
                    .keyboardType(.numberPad)
                    .frame(width: 64)
                    .onSubmit { ponerMedidas() }
                Button { ponerMedidas() } label: {
                    Image(systemName: "checkmark").frame(width: 34, height: 34)
                }
                .help("Poner estas medidas")
            }
            .font(.callout.monospacedDigit())
            .padding(.horizontal, 8)
            .frame(height: 40)
            .background(Paleta.v2.caja, in: RoundedRectangle(cornerRadius: 9))
            Button { repetirUltimaZona() } label: {
                Label("Repetir la última zona", systemImage: "arrow.counterclockwise")
                    .padding(.horizontal, 12)
                    .frame(height: 40)
                    .background(Paleta.v2.caja, in: RoundedRectangle(cornerRadius: 9))
            }
            .keyboardShortcut("r", modifiers: [])
            .help("Repetir la última zona (R)")
            if recortada != nil {
                Button { volverAEntera() } label: {
                    Image(systemName: "arrow.up.left.and.arrow.down.right").frame(width: 40, height: 40)
                        .background(Paleta.v2.caja, in: RoundedRectangle(cornerRadius: 9))
                }
                .help("Volver a la foto entera")
            }
            Rectangle().fill(Paleta.v2.linea).frame(width: 1, height: 28)
            Button("Cancelar") { salirDeRecortar() }
                .keyboardShortcut(.cancelAction)
                .foregroundStyle(Paleta.v2.gris)
                .padding(.horizontal, 6)
            accion("Recortar", "crop", lleno: true) { recortar() }
                .keyboardShortcut(.defaultAction)
                .disabled(!RecorteCaptura.valida(zona))
        }
        .buttonStyle(.plain)
        .font(.body.weight(.medium))
        .padding(8)
        .background(Paleta.v2.fondo, in: RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(Paleta.v2.linea))
    }

    private func accion(_ titulo: String, _ icono: String, lleno: Bool = false,
                        _ hacer: @escaping () -> Void) -> some View {
        Button(action: hacer) {
            Label(titulo, systemImage: icono)
                .padding(.horizontal, 14)
                .frame(height: 44)
                .background(lleno ? Paleta.v2.azulLleno : Paleta.v2.caja, in: RoundedRectangle(cornerRadius: 10))
        }
    }

    // MARK: Hacer

    private func deshacer() {
        guard let m = marcas.popLast() else { return }
        deshechas.append(m)
    }

    private func rehacer() {
        guard let m = deshechas.popLast() else { return }
        marcas.append(m)
    }

    /// Quema lo anotado (fuera del hilo principal) y hace `luego` con ello.
    private func hacer(_ luego: @escaping (CGImage) -> Void) {
        guard !trabajando else { return }
        let base = foto, todas = marcas, pix = pixelada
        if todas.isEmpty { return luego(base) }
        trabajando = true
        Task {
            let quemada = await ImagenIPad.fuera { PintorMarcas.quemar(base, marcas: todas, pixelada: pix) }
            trabajando = false
            luego(quemada)
        }
    }

    /// Copiar apila (la pila de capturas de Windows); al anotar un pin, solo
    /// copia.
    private func copiar(_ quemada: CGImage) {
        if modo.esPin {
            UIPasteboard.general.image = UIImage(cgImage: quemada)
            UINotificationFeedbackGenerator().notificationOccurred(.success)
        } else {
            PilaCapturasIPad.compartida.copiar(quemada)
        }
        cerrar()
    }

    private func guardarEnGaleria(_ quemada: CGImage) {
        Task {
            let hecha = await CapturadorIPad.compartido.guardarEnGaleria(quemada)
            if hecha == nil { avisarFalloPin() } else { UINotificationFeedbackGenerator().notificationOccurred(.success) }
        }
        cerrar()
    }

    private func guardarEnFotos(_ quemada: CGImage) {
        UIImageWriteToSavedPhotosAlbum(UIImage(cgImage: quemada), nil, nil, nil)
        UINotificationFeedbackGenerator().notificationOccurred(.success)
        cerrar()
    }
}

/// Que hace el dedo encima de la foto.
enum ModoEdicionCaptura: Equatable {
    case anotar, recortar, lupa
}

/// Una imagen con identidad, para `.sheet(item:)`.
struct ImagenSuelta: Identifiable {
    let id = UUID()
    let imagen: CGImage
    init(_ imagen: CGImage) { self.imagen = imagen }
}

extension ColoresCaptura {
    static func colorSwiftUI(_ hex: UInt32) -> Color {
        Color(hex: hex)
    }
}
