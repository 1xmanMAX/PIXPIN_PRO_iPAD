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

    @ObservedObject private var capturador = CapturadorIPad.compartido

    private var tamanoFoto: CGSize { CGSize(width: imagen.width, height: imagen.height) }

    var body: some View {
        VStack(spacing: 0) {
            barraAnotar
                .padding(.top, 8)
            GeometryReader { geo in
                lienzo(en: geo.size)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            barraAcciones
                .padding(.bottom, 10)
        }
        .background(Color(hex: 0x14141A).ignoresSafeArea())
        .environment(\.colorScheme, .dark)
        .foregroundStyle(Paleta.v2.texto)
        .overlay {
            if trabajando { ProgressView().controlSize(.large) }
        }
        .task {
            let base = imagen
            pixelada = await ImagenIPad.fuera { PintorMarcas.pixelar(base) }
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
            Image(decorative: imagen, scale: 1)
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
        }
        .frame(width: vista.width, height: vista.height)
        .overlay(Rectangle().strokeBorder(Paleta.v2.acento, lineWidth: 1))
        .contentShape(Rectangle())
        .gesture(dibujar(factor: f))
        .frame(width: espacio.width, height: espacio.height)
    }

    private func dibujar(factor f: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: 0, coordinateSpace: .local)
            .onChanged { v in
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
            .onEnded { v in
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
            Text("\(imagen.width) × \(imagen.height)")
                .font(.callout.monospacedDigit())
                .foregroundStyle(Paleta.v2.gris)
                .padding(.horizontal, 8)
            separador
            ForEach(HerramientaCaptura.allCases) { h in
                Button { herramienta = h } label: {
                    Image(systemName: h.icono)
                        .font(.system(size: 18, weight: .medium))
                        .frame(width: 42, height: 38)
                        .background(herramienta == h ? Paleta.v2.acento.opacity(0.25) : .clear,
                                    in: RoundedRectangle(cornerRadius: 8))
                        .overlay(RoundedRectangle(cornerRadius: 8)
                            .strokeBorder(herramienta == h ? Paleta.v2.acento : .clear, lineWidth: 1.5))
                }
                .keyboardShortcut(KeyEquivalent(h.tecla), modifiers: .command)
                .help("\(h.nombre) (⌘\(h.tecla))")
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
        let base = imagen, todas = marcas, pix = pixelada
        if todas.isEmpty { return luego(base) }
        trabajando = true
        Task {
            let quemada = await ImagenIPad.fuera { PintorMarcas.quemar(base, marcas: todas, pixelada: pix) }
            trabajando = false
            luego(quemada)
        }
    }

    private func copiar(_ quemada: CGImage) {
        UIPasteboard.general.image = UIImage(cgImage: quemada)
        UINotificationFeedbackGenerator().notificationOccurred(.success)
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
