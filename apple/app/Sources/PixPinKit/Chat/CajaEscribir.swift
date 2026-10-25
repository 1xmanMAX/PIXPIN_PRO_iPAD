// **La caja de escribir del chat**: la «isla» flotante de Windows (y del
// movil), `pintar_redaccion` de `ventana_chat.rs` con las medidas de
// `pixpin-ui/src/chat.rs`, traida del Mac (`Chat/CajaEscribir.swift`) con
// UIKit:
//
// - la isla: radio 22 y un filete de 1 que la despega del papel;
// - dentro, el campo en pastilla, con el clip a su derecha (se retira al
//   escribir, como en el movil); el clip abre la hoja de adjuntar
//   (`HojaAdjuntarChat`);
// - fuera del campo, a la derecha, un sitio con dos caras: el microfono, o
//   enviar si hay algo escrito. Grabando, el campo deja su sitio al
//   cronometro.
//
// Teclado, como Windows: Intro envia (tambien la tecla «Enviar» del teclado
// de la pantalla), ⇧Intro (o ⌥Intro) salta de linea, ⌘Intro envia y Esc
// suelta lo seleccionado o, si no lo hay, limpia lo escrito; con la caja
// vacia, Esc deja de responder. ⌘V con una foto en el portapapeles la
// adjunta (la caja solo sabe pegar texto).

import PhotosUI
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

// MARK: - Colores

extension Paleta {
    /// `CLARO` y `OSCURO` de `ventana_chat.rs`: lo de la caja de escribir,
    /// las burbujas y las hojas que salen del chat.
    public enum chat {
        /// El fondo de la cabecera y de las hojas (`cabecera`).
        public static let cabecera = Color(claro: 0xFFFFFF, oscuro: 0x17212B)
        public static let isla = Color(claro: 0xFFFFFF, oscuro: 0x14192D)
        /// El filete de la isla: tinta al 10 % de dia; de noche, opaco.
        public static let islaBorde = Color(uiColor: UIColor { rasgos in
            rasgos.userInterfaceStyle == .dark
                ? UIColor(red: 0x3A / 255, green: 0x40 / 255, blue: 0x60 / 255, alpha: 1)
                : UIColor(red: 0x10 / 255, green: 0x1B / 255, blue: 0x24 / 255, alpha: 0.10)
        })
        /// La pastilla donde se escribe y lo apagado de dentro.
        public static let campo = Color(claro: 0xECEEF8, oscuro: 0x242B55)
        public static let campoApagado = Color(claro: 0x5D5F71, oscuro: 0xB4B9D7)
        /// Enviar y lo que se pulsa en el chat.
        public static let enviar = Color(claro: 0x40A7E3, oscuro: 0x5288C1)
        /// Lo elegido con el teclado (`AZUL_ELEGIDO`).
        public static let azulElegido = Color(hex: 0x0060DF)
        /// Grabando (`#E5534B`).
        public static let grabando = Color(hex: 0xE5534B)
        /// Las burbujas (`burbuja_mia` y `burbuja_otra`, la del asistente),
        /// su texto y su hora.
        public static let burbujaMia = Color(claro: 0xEFFFDE, oscuro: 0x3E618A)
        public static let burbujaOtra = Color(claro: 0xFFFFFF, oscuro: 0x243447)
        public static let textoMio = Color(claro: 0x101B24, oscuro: 0xFAFAFA)
        public static let horaMia = Color(claro: 0x3F7A30, oscuro: 0xA8CCE8)
        /// La pildora que separa los dias (`separador_dia`) y su texto.
        public static let separadorDia = Color(claro: 0xE1E3F3, oscuro: 0x242B55)
        public static let textoSeparador = Color(claro: 0x42465E, oscuro: 0xC5C9EA)
        /// Las pastillas de la cabecera (`pildora`) y su texto.
        public static let pildora = Color(claro: 0xFFFFFF, oscuro: 0x161B31)
        public static let pildoraTexto = Color(claro: 0x1B1B21, oscuro: 0xE8EAF6)
        public static let pildoraSub = Color(claro: 0x5D5F71, oscuro: 0xB4B9D7)
        /// El disco de la carpeta del titulo y su icono.
        public static let carpeta = Color(claro: 0x7A5900, oscuro: 0xFFD179)
        public static let carpetaIcono = Color(claro: 0xFFFFFF, oscuro: 0x2B1D00)
        /// El boton redondo dorado y su icono (`circulo`, `circulo_icono`).
        public static let circulo = Color(claro: 0xFFDEA6, oscuro: 0x493A11)
        public static let circuloIcono = Color(claro: 0x3E2D00, oscuro: 0xFEE8B7)
    }
}

// MARK: - Medidas (las de `pixpin-ui/src/chat.rs`, al 100 %)

enum MedidasIsla {
    static let margenX: CGFloat = 7
    static let arriba: CGFloat = 6
    static let abajo: CGFloat = 9
    static let rellenoX: CGFloat = 6
    static let rellenoY: CGFloat = 4
    static let radio: CGFloat = 22
    /// Lo alto del campo con una linea, y el lado del clip.
    static let fila: CGFloat = 44
    /// El lado del boton de la voz (o enviar).
    static let boton: CGFloat = fila + 4
    static let textoX: CGFloat = 14
    static let textoArriba: CGFloat = 11
    static let textoAbajo: CGFloat = 11
    /// La letra de lo que se escribe.
    static let letra: CGFloat = 16
    /// Lo mas que crece la caja antes de desplazarse, en lineas.
    static let lineas = 8
}

extension Array {
    /// El elemento `i`, o `nil` si no esta.
    subscript(safe i: Int) -> Element? { indices.contains(i) ? self[i] : nil }
}

// MARK: - La isla

struct IslaDeEscribir: View {
    @Binding var borrador: String
    let chat: String
    let almacen: Almacen
    /// Sube para pedir el foco (al abrir el chat, tras enviar...).
    var foco: Int
    let enviar: () -> Void
    /// Esc con la caja vacia: `true` si alguien lo uso.
    let escaparVacio: () -> Bool
    /// ⌘V con algo que no es texto: `true` si se adjunto.
    let pegarAdjuntos: (UITextView) -> Bool
    let adjuntar: (AccionAdjuntar) -> Void
    @Environment(\.escalaChat) private var f
    @ObservedObject private var grabadora = GrabadoraVoz.compartida
    @State private var hoja = false

    private var hayTexto: Bool { !borrador.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    private var grabandoAqui: Bool { grabadora.grabando(en: chat) }

    var body: some View {
        HStack(alignment: .bottom, spacing: MedidasIsla.rellenoX * f) {
            campo
            botonDeVoz
        }
        .padding(.horizontal, MedidasIsla.rellenoX * f)
        .padding(.vertical, MedidasIsla.rellenoY * f)
        .background {
            RoundedRectangle(cornerRadius: MedidasIsla.radio * f)
                .fill(Paleta.chat.isla)
                .overlay(RoundedRectangle(cornerRadius: MedidasIsla.radio * f)
                    .strokeBorder(Paleta.chat.islaBorde, lineWidth: 1))
        }
        .padding(.horizontal, MedidasIsla.margenX * f)
        .padding(.top, MedidasIsla.arriba * f)
        .padding(.bottom, MedidasIsla.abajo * f)
    }

    /// La pastilla: lo escrito (o el cronometro, grabando) y el clip.
    private var campo: some View {
        HStack(alignment: .bottom, spacing: 0) {
            if grabandoAqui, case let .grabando(_, desde) = grabadora.estado {
                TimelineView(.periodic(from: desde, by: 1)) { contexto in
                    HStack(spacing: 8) {
                        Circle().fill(Paleta.chat.grabando).frame(width: 8 * f, height: 8 * f)
                        Text(FormatoVoz.marca(ms: Int64(contexto.date.timeIntervalSince(desde) * 1000)))
                            .font(.system(size: MedidasIsla.letra * f).monospacedDigit())
                            .foregroundStyle(Paleta.chat.grabando)
                        Text("Grabando… pulsa ■ para mandarla")
                            .font(.system(size: 13 * f))
                            .foregroundStyle(Paleta.chat.campoApagado)
                            .lineLimit(1)
                    }
                }
                .padding(.leading, MedidasIsla.textoX * f)
                .frame(maxWidth: .infinity, minHeight: MedidasIsla.fila * f, alignment: .leading)
            } else {
                CampoDeEscribir(texto: $borrador, tamano: MedidasIsla.letra * f,
                                lineas: MedidasIsla.lineas, foco: foco,
                                enviar: enviar, escaparVacio: escaparVacio, pegarAdjuntos: pegarAdjuntos)
                    .overlay(alignment: .topLeading) {
                        if borrador.isEmpty {
                            Text("Escribe algo…")
                                .font(.system(size: MedidasIsla.letra * f))
                                .foregroundStyle(Paleta.chat.campoApagado)
                                .allowsHitTesting(false)
                        }
                    }
                    .padding(.leading, MedidasIsla.textoX * f)
                    .padding(.trailing, hayTexto ? MedidasIsla.textoX * f : 0)
                    .padding(.top, MedidasIsla.textoArriba * f)
                    .padding(.bottom, MedidasIsla.textoAbajo * f)
                    .frame(minHeight: MedidasIsla.fila * f)
                if !hayTexto { clip }
            }
        }
        .background(Paleta.chat.campo,
                    in: RoundedRectangle(cornerRadius: min(MedidasIsla.radio * f, MedidasIsla.fila * f / 2)))
    }

    private var clip: some View {
        Button { hoja.toggle() } label: {
            Image(systemName: "paperclip")
                .font(.system(size: 19 * f, weight: .medium))
                .foregroundStyle(Paleta.chat.campoApagado)
                .frame(width: MedidasIsla.fila * f, height: MedidasIsla.fila * f)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Adjuntar")
        .popover(isPresented: $hoja, arrowEdge: .bottom) {
            HojaAdjuntarChat(esAsistente: almacen.chats.first(where: { $0.id == chat })?.esAsistente == true) { accion in
                hoja = false
                // Una vuelta despues: que el globo ya se haya ido (un
                // selector de fotos o de archivos no sale encima de otro).
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { adjuntar(accion) }
            }
        }
    }

    /// El microfono, enviar o parar: el mismo sitio con tres caras.
    private var botonDeVoz: some View {
        Button(action: pulsarVoz) {
            Group {
                if grabadora.estado == .guardando || grabadora.estado == .transcribiendo, !hayTexto {
                    ProgressView()
                } else {
                    Image(systemName: grabandoAqui ? "stop.fill" : (hayTexto ? "paperplane.fill" : "mic"))
                        .font(.system(size: 20 * f, weight: .medium))
                        .foregroundStyle(grabandoAqui ? Paleta.chat.grabando
                                         : (hayTexto ? Paleta.chat.enviar : Paleta.chat.campoApagado))
                }
            }
            .frame(width: MedidasIsla.boton * f, height: MedidasIsla.boton * f)
            .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .disabled(!hayTexto && vozOcupadaEnOtro)
        .accessibilityLabel(grabandoAqui ? "Parar y mandar la nota de voz"
                            : (hayTexto ? "Enviar" : "Grabar una nota de voz"))
    }

    private func pulsarVoz() {
        if hayTexto, !grabandoAqui {
            enviar()
        } else {
            grabadora.alternar(en: chat, almacen: almacen)
        }
    }

    /// Grabando en otro chat, o guardando: el microfono espera.
    private var vozOcupadaEnOtro: Bool {
        switch grabadora.estado {
        case .quieta: false
        case let .grabando(c, _): c != chat
        case .guardando, .transcribiendo: true
        }
    }
}

// MARK: - La hoja del clip

/// Lo que se puede adjuntar desde el clip (la hoja de Windows con lo que el
/// iPad tiene: la «Captura» de Windows aqui es la camara).
enum AccionAdjuntar: Hashable {
    case foto, archivo, lienzo, nota, tareas, tabla, camara
    /// Lo que llega de «Pegar» (el boton del sistema, que no pide permiso).
    case pegar([NSItemProvider])
    /// «Proyecto entero» (la puerta `PROYECTO`) y «Página de un proyecto».
    case proyecto, pagina
    /// Una mini-app viva (contador, ruleta, gastos, relojes): nace en el
    /// chat y se abre en un pin.
    case mini(String)
    /// Una ventanita de la voz (conversación, teleprónter, pronunciar,
    /// grabar clase): `VozEnElChat.abrir`.
    case voz(HerramientaDeVoz)
    /// Una figura de la biblioteca, elegida en su galeria, a un lienzo del
    /// chat (en el del asistente, para que la mejore), y pedirle una nueva
    /// al asistente (`para_el_asistente`, como el Mac).
    case figura, pedirFigura
    /// Una carpeta de paginas (26-oct-2026): un cuaderno con nombre.
    case carpeta

    static func == (a: AccionAdjuntar, b: AccionAdjuntar) -> Bool {
        switch (a, b) {
        case (.foto, .foto), (.archivo, .archivo), (.lienzo, .lienzo), (.nota, .nota),
             (.tareas, .tareas), (.tabla, .tabla), (.camara, .camara), (.proyecto, .proyecto),
             (.pagina, .pagina), (.figura, .figura), (.pedirFigura, .pedirFigura), (.carpeta, .carpeta): true
        case let (.pegar(x), .pegar(y)): x == y
        case let (.mini(x), .mini(y)): x == y
        case let (.voz(x), .voz(y)): x == y
        default: false
        }
    }

    func hash(into h: inout Hasher) {
        switch self {
        case .foto: h.combine(0)
        case .archivo: h.combine(1)
        case .lienzo: h.combine(2)
        case .nota: h.combine(3)
        case .tareas: h.combine(4)
        case .tabla: h.combine(5)
        case .camara: h.combine(6)
        case let .pegar(p): h.combine(7); h.combine(p.count)
        case .proyecto: h.combine(8)
        case .pagina: h.combine(9)
        case let .mini(m): h.combine(10); h.combine(m)
        case let .voz(v): h.combine(11); h.combine(v)
        case .figura: h.combine(12)
        case .pedirFigura: h.combine(13)
        case .carpeta: h.combine(14)
        }
    }
}

/// Las baldosas de la hoja de adjuntar, con los colores de Windows.
struct HojaAdjuntarChat: View {
    /// En el chat del asistente, primero lo de las figuras (la hoja
    /// `delAsistente` del Mac).
    var esAsistente = false
    let elegir: (AccionAdjuntar) -> Void

    private static let baldosas: [(AccionAdjuntar, String, String, UInt32)] = [
        (.foto, "Foto o vídeo", "photo", 0x0A6CD6),
        (.archivo, "Archivo", "doc.text.fill", 0x636366),
        (.lienzo, "Lienzo", "pencil.and.scribble", 0xC77700),
        (.nota, "Nota", "list.bullet", 0x248A3D),
        (.tareas, "Lista de tareas", "checklist", 0x1E7A4A),
        (.tabla, "Tabla", "tablecells.fill", 0x4B49C8),
        (.camara, "Cámara", "camera.fill", 0xC93C35),
    ]

    /// Lo que se usa menos, en chips debajo (como Windows): el proyecto, sus
    /// paginas y las mini-apps vivas.
    private static let chips: [(AccionAdjuntar, String, String)] = [
        (.proyecto, "Proyecto entero", "folder"),
        (.pagina, "Página de un proyecto", "doc.richtext"),
        // Una figura de la biblioteca (Windows 1d50a7c): abre la galeria de
        // las figuras para elegirla viendola, con su boton «Adjuntar».
        (.figura, "Figura", "paintpalette"),
        (.carpeta, "Carpeta de páginas", "book"),
        (.mini("contador"), "Contador", "plus.forwardslash.minus"),
        (.mini("ruleta"), "Ruleta", "circle.dashed"),
        (.mini("gastos"), "Gastos", "eurosign.circle"),
        (.mini("cronometro"), "Cronómetro", "stopwatch"),
        (.mini("temporizador"), "Temporizador", "timer"),
        (.mini("alarma"), "Alarma", "alarm"),
        (.voz(.conversacion), HerramientaDeVoz.conversacion.rotulo, HerramientaDeVoz.conversacion.simbolo),
        (.voz(.telepronter), HerramientaDeVoz.telepronter.rotulo, HerramientaDeVoz.telepronter.simbolo),
        (.voz(.pronunciar), HerramientaDeVoz.pronunciar.rotulo, HerramientaDeVoz.pronunciar.simbolo),
        (.voz(.clase), HerramientaDeVoz.clase.rotulo, HerramientaDeVoz.clase.simbolo),
    ]

    /// Los chips de esta hoja: en la del asistente, primero «Figura» y
    /// «Pedir una figura nueva».
    private var chips: [(AccionAdjuntar, String, String)] {
        guard esAsistente else { return Self.chips }
        let figuras: [(AccionAdjuntar, String, String)] = [
            (.figura, "Figura", "paintpalette"),
            (.pedirFigura, "Pedir una figura nueva", "wand.and.stars"),
        ]
        return figuras + Self.chips.filter { $0.0 != .figura }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            LazyVGrid(columns: Array(repeating: GridItem(.fixed(92), spacing: 8), count: 4), spacing: 12) {
                ForEach(Self.baldosas.indices, id: \.self) { i in
                    let (accion, rotulo, simbolo, color) = Self.baldosas[i]
                    Button { elegir(accion) } label: {
                        VStack(spacing: 8) {
                            Image(systemName: simbolo)
                                .font(.system(size: 20, weight: .semibold))
                                .foregroundStyle(.white)
                                .frame(width: 48, height: 48)
                                .background(Color(hex: color), in: RoundedRectangle(cornerRadius: 12))
                            Text(rotulo)
                                .font(.system(size: 12))
                                .foregroundStyle(.primary)
                                .lineLimit(2)
                                .multilineTextAlignment(.center)
                        }
                        .frame(width: 92, height: 92)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .disabled(accion == .camara && !UIImagePickerController.isSourceTypeAvailable(.camera))
                }
            }
            Divider()
            Text("Más").font(.system(size: 12, weight: .medium)).foregroundStyle(.secondary)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 120), spacing: 6, alignment: .leading)],
                      alignment: .leading, spacing: 6) {
                ForEach(chips.indices, id: \.self) { i in
                    let (accion, rotulo, simbolo) = chips[i]
                    Button { elegir(accion) } label: {
                        Label(rotulo, systemImage: simbolo)
                            .font(.system(size: 13))
                            .lineLimit(1)
                            .padding(.horizontal, 10)
                            .frame(height: 32)
                            .background(Color.primary.opacity(0.07), in: Capsule())
                            .contentShape(Capsule())
                    }
                    .buttonStyle(.plain)
                }
            }
            Divider()
            HStack {
                Text("Pegar lo copiado").font(.system(size: 13)).foregroundStyle(.secondary)
                Spacer()
                PasteButton(supportedContentTypes: EntradaDeFicheros.tipos) { proveedores in
                    DispatchQueue.main.async { elegir(.pegar(proveedores)) }
                }
                .labelStyle(.titleAndIcon)
                .buttonBorderShape(.capsule)
            }
        }
        .padding(16)
        .frame(width: 420)
        .presentationCompactAdaptation(.sheet)
    }
}

// MARK: - El campo de varias lineas

/// Lo escrito en la isla: un `UITextView` (un `TextField` de SwiftUI no deja
/// distinguir Intro de ⇧Intro). Crece con lo escrito hasta `lineas` y
/// despues se desplaza.
struct CampoDeEscribir: UIViewRepresentable {
    @Binding var texto: String
    var tamano: CGFloat
    var lineas: Int
    var foco: Int
    let enviar: () -> Void
    let escaparVacio: () -> Bool
    let pegarAdjuntos: (UITextView) -> Bool

    func makeCoordinator() -> Coordinador { Coordinador(self) }

    func makeUIView(context: Context) -> TextoDeLaIsla {
        let tv = TextoDeLaIsla()
        tv.backgroundColor = .clear
        tv.font = .systemFont(ofSize: tamano)
        tv.textColor = .label
        tv.textContainerInset = .zero
        tv.textContainer.lineFragmentPadding = 0
        tv.isScrollEnabled = false
        tv.returnKeyType = .send
        tv.smartQuotesType = .no
        tv.smartDashesType = .no
        tv.text = texto
        tv.delegate = context.coordinator
        tv.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        tv.setContentHuggingPriority(.defaultLow, for: .horizontal)
        let c = context.coordinator
        tv.alEnviar = { [weak c] in c?.padre.enviar() }
        tv.alEscaparVacio = { [weak c] in _ = c?.padre.escaparVacio() }
        tv.alPegar = { [weak c, weak tv] in
            guard let tv else { return false }
            return c?.padre.pegarAdjuntos(tv) ?? false
        }
        tv.alCambiar = { [weak c, weak tv] in
            guard let tv else { return }
            c?.padre.texto = tv.text
        }
        c.focoVisto = foco
        return tv
    }

    func updateUIView(_ tv: TextoDeLaIsla, context: Context) {
        let c = context.coordinator
        c.padre = self
        if tv.text != texto { tv.text = texto }
        if tv.font?.pointSize != tamano { tv.font = .systemFont(ofSize: tamano) }
        if c.focoVisto != foco {
            c.focoVisto = foco
            DispatchQueue.main.async { tv.becomeFirstResponder() }
        }
    }

    /// Crece con lo escrito: lo alto de sus lineas con el ancho que le dan.
    func sizeThatFits(_ propuesta: ProposedViewSize, uiView tv: TextoDeLaIsla, context: Context) -> CGSize? {
        let ancho = max(propuesta.width ?? 300, 1)
        let letra = tv.font ?? .systemFont(ofSize: tamano)
        let medido = tv.sizeThatFits(CGSize(width: ancho, height: .greatestFiniteMagnitude)).height
        let linea = letra.lineHeight
        let tope = linea * CGFloat(lineas)
        let desplazar = medido > tope + 0.5
        if tv.isScrollEnabled != desplazar {
            DispatchQueue.main.async { tv.isScrollEnabled = desplazar }
        }
        return CGSize(width: ancho, height: ceil(min(max(medido, linea), tope)))
    }

    @MainActor
    final class Coordinador: NSObject, UITextViewDelegate {
        var padre: CampoDeEscribir
        var focoVisto = 0

        init(_ padre: CampoDeEscribir) { self.padre = padre }

        func textViewDidChange(_ tv: UITextView) {
            padre.texto = tv.text
        }

        func textView(_ tv: UITextView, shouldChangeTextIn range: NSRange, replacementText t: String) -> Bool {
            guard t == "\n" else { return true }
            // Escribiendo con marcas (japones, acentos a medias): es suyo.
            if tv.markedTextRange != nil { return true }
            if let isla = tv as? TextoDeLaIsla, isla.saltoPedido {
                isla.saltoPedido = false
                return true
            }
            padre.enviar()
            return false
        }
    }
}

/// El texto de la isla: las teclas de Windows con el teclado del iPad y el
/// ⌘V de fotos y ficheros.
final class TextoDeLaIsla: UITextView {
    var alEnviar: (() -> Void)?
    var alEscaparVacio: (() -> Void)?
    var alPegar: (() -> Bool)?
    var alCambiar: (() -> Void)?
    /// ⇧Intro pidio un salto de linea (el delegado lo deja pasar).
    var saltoPedido = false

    override var keyCommands: [UIKeyCommand]? {
        let salto = UIKeyCommand(input: "\r", modifierFlags: .shift, action: #selector(saltoDeLinea))
        let saltoOpcion = UIKeyCommand(input: "\r", modifierFlags: .alternate, action: #selector(saltoDeLinea))
        let enviar = UIKeyCommand(title: "Enviar", action: #selector(enviarAhora), input: "\r", modifierFlags: .command)
        let esc = UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(escapar))
        for k in [salto, saltoOpcion, enviar, esc] { k.wantsPriorityOverSystemBehavior = true }
        return [salto, saltoOpcion, enviar, esc]
    }

    @objc private func saltoDeLinea() {
        saltoPedido = true
        insertText("\n")
        saltoPedido = false
    }

    @objc private func enviarAhora() { alEnviar?() }

    /// Con algo seleccionado, Esc solo lo suelta; con algo escrito, lo
    /// limpia; vacia, se lo queda quien este detras (la respuesta).
    @objc private func escapar() {
        if markedTextRange != nil { return }
        let r = selectedRange
        if r.length > 0 {
            selectedRange = NSRange(location: r.location + r.length, length: 0)
        } else if !text.isEmpty {
            text = ""
            alCambiar?()
        } else {
            alEscaparVacio?()
        }
    }

    override func paste(_ sender: Any?) {
        if let alPegar, alPegar() { return }
        super.paste(sender)
    }

    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        if action == #selector(paste(_:)), EntradaDeFicheros.hayAdjuntosParaPegar() { return true }
        return super.canPerformAction(action, withSender: sender)
    }
}

// MARK: - Una caja de una linea (buscadores)

/// Las flechas de una caja de una linea.
enum FlechaDeCampo: Equatable { case arriba, abajo, izquierda, derecha }

/// Las teclas de una caja de una linea que puede querer quien la usa.
enum TeclaDeCampo: Equatable {
    case flecha(FlechaDeCampo)
    case intro, introConMayusculas, escape
}

/// Una caja de una linea sin marco (los buscadores del chat): da a quien la
/// usa las flechas, Intro y Esc antes que a ella misma.
struct CampoDeUnaLinea: UIViewRepresentable {
    @Binding var texto: String
    var invitacion: String
    var tamano: CGFloat = 13
    /// Sube para pedir el foco; al montarse lo pide siempre (si no es -1).
    var foco: Int = 0
    /// `true` si la tecla era suya.
    var tecla: (TeclaDeCampo) -> Bool = { _ in false }

    init(texto: Binding<String>, invitacion: String, tamano: CGFloat = 13, foco: Bool = true,
         tecla: @escaping (TeclaDeCampo) -> Bool = { _ in false }) {
        _texto = texto
        self.invitacion = invitacion
        self.tamano = tamano
        self.foco = foco ? 0 : -1
        self.tecla = tecla
    }

    init(texto: Binding<String>, invitacion: String, tamano: CGFloat = 13, pedirFoco: Int,
         tecla: @escaping (TeclaDeCampo) -> Bool = { _ in false }) {
        _texto = texto
        self.invitacion = invitacion
        self.tamano = tamano
        foco = pedirFoco
        self.tecla = tecla
    }

    func makeCoordinator() -> Coordinador { Coordinador(self) }

    func makeUIView(context: Context) -> CampoDeUnaLineaUI {
        let campo = CampoDeUnaLineaUI()
        campo.borderStyle = .none
        campo.font = .systemFont(ofSize: tamano)
        campo.placeholder = invitacion
        campo.text = texto
        campo.returnKeyType = .search
        campo.clearButtonMode = .never
        campo.autocorrectionType = .no
        campo.delegate = context.coordinator
        campo.setContentHuggingPriority(.defaultLow, for: .horizontal)
        campo.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        let c = context.coordinator
        campo.addTarget(c, action: #selector(Coordinador.cambio(_:)), for: .editingChanged)
        campo.alTecla = { [weak c] t in c?.padre.tecla(t) ?? false }
        c.focoVisto = foco
        if foco >= 0 {
            DispatchQueue.main.async { campo.becomeFirstResponder() }
        }
        return campo
    }

    func updateUIView(_ campo: CampoDeUnaLineaUI, context: Context) {
        context.coordinator.padre = self
        if campo.text != texto { campo.text = texto }
        if campo.font?.pointSize != tamano { campo.font = .systemFont(ofSize: tamano) }
        if context.coordinator.focoVisto != foco {
            context.coordinator.focoVisto = foco
            DispatchQueue.main.async { campo.becomeFirstResponder() }
        }
    }

    @MainActor
    final class Coordinador: NSObject, UITextFieldDelegate {
        var padre: CampoDeUnaLinea
        var focoVisto = 0

        init(_ padre: CampoDeUnaLinea) { self.padre = padre }

        @objc func cambio(_ campo: UITextField) {
            padre.texto = campo.text ?? ""
        }

        func textFieldShouldReturn(_ campo: UITextField) -> Bool {
            _ = padre.tecla(.intro)
            return false
        }
    }
}

/// El `UITextField` de `CampoDeUnaLinea`: las flechas, ⇧Intro y Esc del
/// teclado del iPad van primero a quien lo usa.
final class CampoDeUnaLineaUI: UITextField {
    var alTecla: ((TeclaDeCampo) -> Bool)?

    override var keyCommands: [UIKeyCommand]? {
        let teclas: [UIKeyCommand] = [
            UIKeyCommand(input: UIKeyCommand.inputUpArrow, modifierFlags: [], action: #selector(arriba)),
            UIKeyCommand(input: UIKeyCommand.inputDownArrow, modifierFlags: [], action: #selector(abajo)),
            UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(escape)),
            UIKeyCommand(input: "\r", modifierFlags: .shift, action: #selector(introMayusculas)),
        ]
        for k in teclas { k.wantsPriorityOverSystemBehavior = true }
        return teclas
    }

    @objc private func arriba() { _ = alTecla?(.flecha(.arriba)) }
    @objc private func abajo() { _ = alTecla?(.flecha(.abajo)) }
    @objc private func escape() { _ = alTecla?(.escape) }
    @objc private func introMayusculas() { _ = alTecla?(.introConMayusculas) }
}
