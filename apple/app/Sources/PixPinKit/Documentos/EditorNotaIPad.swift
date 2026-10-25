// **El editor de notas Markdown** del iPad (la nota `NOTA` del chat; en
// Windows, `pixpin-notas`; en el Mac, `EditorNota.swift`): un `UITextView`
// cuyo texto ES el Markdown, letra a letra. El formato se pone encima
// mientras se escribe (titulos, negrita, cursiva, listas, casillas, codigo,
// citas, enlaces) con lo que dice `pixpin_docs::md_vivo` (Rust, `mdTramos`),
// y las formulas `$…$` se componen con `pixpin_docs::formulas`
// (`formulaToques`): `x^2` se ve x² fuera del renglon donde se escribe.
//
// Las marcas (`#`, `**`, `[`, los dolares) se esconden fuera del renglon del
// cursor; en el suyo se ven, para poder borrarlas sin adivinar. Intro sigue
// las listas (`mdContinuar`) y un toque en una casilla la marca.
//
// Guardar es leer el texto y ya: 1 s despues de la ultima tecla, fuera del
// hilo principal, con `Nucleo.editarNota` (la linea del cuaderno y, si es una
// nota del movil, su hoja `nota`), como Windows y el Mac. El mismo editor
// abre un `.md` suelto (`VisorMarkdownIPad`), que se guarda en su fichero.
//
// Rendimiento: analizar es una pasada por el texto en Rust y se pide tras una
// pausa de 120 ms al escribir, no por cada tecla.

import Combine
import PhotosUI
import PixPinCore
import SwiftUI
import UIKit

/// Los estilos de una nota, aplicados sobre un `NSTextStorage`. Sin
/// ventanas: se prueba solo.
@MainActor
public enum EstiloDeNotaIPad {
    public static let tamBase: CGFloat = 17

    public static var letraBase: UIFont { UIFont.systemFont(ofSize: tamBase) }

    static func tamTitulo(_ nivel: UInt8) -> CGFloat {
        switch nivel {
        case 1: return tamBase * 1.7
        case 2: return tamBase * 1.4
        case 3: return tamBase * 1.2
        default: return tamBase * 1.05
        }
    }

    private static func conRasgo(_ s: NSTextStorage, _ r: NSRange, _ rasgo: UIFontDescriptor.SymbolicTraits) {
        s.enumerateAttribute(.font, in: r) { valor, sub, _ in
            let f = (valor as? UIFont) ?? letraBase
            let rasgos = f.fontDescriptor.symbolicTraits.union(rasgo)
            if let d = f.fontDescriptor.withSymbolicTraits(rasgos) {
                s.addAttribute(.font, value: UIFont(descriptor: d, size: f.pointSize), range: sub)
            }
        }
    }

    private static func monoespaciada(_ s: NSTextStorage, _ r: NSRange) {
        s.enumerateAttribute(.font, in: r) { valor, sub, _ in
            let tam = (valor as? UIFont)?.pointSize ?? tamBase
            s.addAttribute(.font, value: UIFont.monospacedSystemFont(ofSize: tam * 0.95, weight: .regular), range: sub)
        }
    }

    /// Una marca escondida: diminuta y transparente.
    private static func esconder(_ s: NSTextStorage, _ r: NSRange) {
        s.addAttributes([.font: UIFont.systemFont(ofSize: 0.1), .foregroundColor: UIColor.clear], range: r)
    }

    /// **Pone el formato** de `tramos` sobre todo el texto. `lineaDelCursor`:
    /// en ese renglon las marcas se ven (atenuadas).
    public static func aplicar(_ s: NSTextStorage, tramos: [TramoMd], lineaDelCursor: UInt32?) {
        let todo = NSRange(location: 0, length: s.length)
        guard todo.length > 0 else { return }
        s.beginEditing()
        defer { s.endEditing() }
        s.setAttributes([.font: letraBase, .foregroundColor: UIColor.label], range: todo)
        func rango(_ t: TramoMd) -> NSRange? {
            let a = Int(t.desde), b = min(Int(t.hasta), todo.length)
            return a < b ? NSRange(location: a, length: b - a) : nil
        }
        // Primero lo que cambia el tamano (titulos), luego los rasgos.
        for t in tramos {
            guard let r = rango(t) else { continue }
            switch t.estilo {
            case let .titulo(nivel):
                s.addAttribute(.font, value: UIFont.boldSystemFont(ofSize: tamTitulo(nivel)), range: r)
            case .cabecera:
                conRasgo(s, r, .traitBold)
            default:
                break
            }
        }
        for t in tramos {
            guard let r = rango(t) else { continue }
            switch t.estilo {
            case .negrita: conRasgo(s, r, .traitBold)
            case .cursiva: conRasgo(s, r, .traitItalic)
            case .tachado:
                s.addAttribute(.strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: r)
            case .codigo:
                monoespaciada(s, r)
                s.addAttribute(.backgroundColor, value: UIColor.quaternarySystemFill, range: r)
            case .bloqueCodigo:
                monoespaciada(s, r)
                s.addAttribute(.backgroundColor, value: UIColor.quaternarySystemFill.withAlphaComponent(0.5), range: r)
            case .cita:
                s.addAttribute(.foregroundColor, value: UIColor.secondaryLabel, range: r)
                let p = NSMutableParagraphStyle()
                p.headIndent = 16
                p.firstLineHeadIndent = 16
                s.addAttribute(.paragraphStyle, value: p, range: r)
            case .enlace:
                s.addAttribute(.foregroundColor, value: UIColor.link, range: r)
                s.addAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, range: r)
                if let u = t.url, let url = URL(string: u) { s.addAttribute(.link, value: url, range: r) }
            case .casilla:
                monoespaciada(s, r)
                s.addAttribute(.foregroundColor, value: UIColor.tintColor, range: r)
            case .hecha:
                s.addAttribute(.strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: r)
                s.addAttribute(.foregroundColor, value: UIColor.secondaryLabel, range: r)
            case .numero:
                s.addAttribute(.foregroundColor, value: UIColor.secondaryLabel, range: r)
            case .regla, .imagen:
                s.addAttribute(.foregroundColor, value: UIColor.tertiaryLabel, range: r)
            case .formula:
                formula(s, r, fueraDelCursor: t.linea != lineaDelCursor)
            default:
                break
            }
        }
        // Las marcas, al final: encima de todo lo demas.
        for t in tramos where t.estilo == .marca {
            guard let r = rango(t) else { continue }
            if t.linea == lineaDelCursor {
                s.addAttribute(.foregroundColor, value: UIColor.tertiaryLabel, range: r)
            } else {
                esconder(s, r)
            }
        }
    }

    /// Una formula entre dolares: en cursiva de imprenta y, fuera del
    /// renglon del cursor, compuesta (`formulaToques`): exponentes arriba,
    /// subindices abajo y sus marcas y dolares escondidos.
    private static func formula(_ s: NSTextStorage, _ r: NSRange, fueraDelCursor: Bool) {
        var d = UIFont.systemFont(ofSize: tamBase).fontDescriptor
        d = d.withDesign(.serif) ?? d
        d = d.withSymbolicTraits(.traitItalic) ?? d
        let letra = UIFont(descriptor: d, size: tamBase)
        s.addAttribute(.font, value: letra, range: r)
        guard fueraDelCursor, r.length > 0 else { return }
        var dentroR = r
        let texto = (s.string as NSString).substring(with: r)
        if r.length >= 2, texto.hasPrefix("$"), texto.hasSuffix("$") {
            esconder(s, NSRange(location: r.location, length: 1))
            esconder(s, NSRange(location: r.location + r.length - 1, length: 1))
            dentroR = NSRange(location: r.location + 1, length: r.length - 2)
        }
        let dentro = (s.string as NSString).substring(with: dentroR)
        let pequena = UIFont(descriptor: letra.fontDescriptor, size: tamBase * 0.7)
        for t in formulaToques(formula: dentro) {
            let a = dentroR.location + Int(t.desde)
            let largo = Int(t.hasta) - Int(t.desde)
            guard largo > 0, a + largo <= dentroR.location + dentroR.length else { continue }
            let sub = NSRange(location: a, length: largo)
            switch t.toque {
            case .arriba:
                s.addAttributes([.font: pequena, .baselineOffset: tamBase * 0.38], range: sub)
            case .abajo:
                s.addAttributes([.font: pequena, .baselineOffset: -tamBase * 0.18], range: sub)
            case .oculto:
                esconder(s, sub)
            }
        }
    }

    /// El renglon (desde 0) de una posicion UTF-16 del texto.
    public nonisolated static func linea(de pos: Int, en texto: String) -> UInt32 {
        let ns = texto as NSString
        var n: UInt32 = 0
        var i = 0
        let fin = min(pos, ns.length)
        while i < fin {
            let r = ns.rangeOfCharacter(from: .newlines, options: [], range: NSRange(location: i, length: fin - i))
            if r.location == NSNotFound { break }
            n += 1
            i = r.location + r.length
        }
        return n
    }

    /// El titulo de una nota (su primer renglon con algo), o «Nota».
    public nonisolated static func titulo(_ texto: String) -> String {
        let t = mdTitulo(texto: texto)
        return t.isEmpty ? "Nota" : t
    }

    /// **La marca de Markdown de un boton** alrededor de `elegido` (o en
    /// el renglon, para los de renglon): devuelve el texto nuevo y lo que
    /// queda elegido. Pura: se prueba sola.
    public nonisolated static func marcar(_ texto: String, elegido: NSRange, marca: MarcaNota) -> (String, NSRange) {
        let ns = texto as NSString
        let r = NSRange(location: min(elegido.location, ns.length),
                        length: min(elegido.length, ns.length - min(elegido.location, ns.length)))
        switch marca {
        case .negrita, .cursiva, .tachado, .codigo, .formula:
            let m = marca.envoltura
            let dentro = ns.substring(with: r)
            let nuevo = ns.replacingCharacters(in: r, with: m + dentro + m)
            return (nuevo, NSRange(location: r.location + (m as NSString).length, length: r.length))
        case .titulo, .lista, .casilla, .cita:
            let renglon = ns.lineRange(for: NSRange(location: r.location, length: 0))
            let prefijo = marca.prefijo
            let actual = ns.substring(with: renglon)
            if actual.hasPrefix(prefijo) {
                let nuevo = ns.replacingCharacters(in: NSRange(location: renglon.location,
                                                               length: (prefijo as NSString).length), with: "")
                return (nuevo, NSRange(location: max(renglon.location, r.location - (prefijo as NSString).length), length: r.length))
            }
            let nuevo = ns.replacingCharacters(in: NSRange(location: renglon.location, length: 0), with: prefijo)
            return (nuevo, NSRange(location: r.location + (prefijo as NSString).length, length: r.length))
        }
    }
}

/// Lo que ponen los botones de la barra de la nota.
public enum MarcaNota: CaseIterable, Sendable {
    case titulo, negrita, cursiva, tachado, lista, casilla, cita, codigo, formula

    var envoltura: String {
        switch self {
        case .negrita: "**"
        case .cursiva: "*"
        case .tachado: "~~"
        case .codigo: "`"
        case .formula: "$"
        default: ""
        }
    }

    var prefijo: String {
        switch self {
        case .titulo: "# "
        case .lista: "- "
        case .casilla: "- [ ] "
        case .cita: "> "
        default: ""
        }
    }

    var icono: String {
        switch self {
        case .titulo: "textformat.size"
        case .negrita: "bold"
        case .cursiva: "italic"
        case .tachado: "strikethrough"
        case .lista: "list.bullet"
        case .casilla: "checklist"
        case .cita: "text.quote"
        case .codigo: "chevron.left.forwardslash.chevron.right"
        case .formula: "function"
        }
    }

    var ayuda: String {
        switch self {
        case .titulo: "Título"
        case .negrita: "Negrita (⌘B)"
        case .cursiva: "Cursiva (⌘I)"
        case .tachado: "Tachado"
        case .lista: "Lista"
        case .casilla: "Casilla"
        case .cita: "Cita"
        case .codigo: "Código"
        case .formula: "Fórmula ($…$)"
        }
    }
}

// MARK: - El texto

/// El estado de un texto Markdown abierto: lo que se escribe, cuando se
/// restila y cuando se guarda (`guardar` lo pone quien lo abre).
@MainActor
final class ModeloTextoNota: ObservableObject {
    @Published var titulo: String
    /// Sin formato (un `.txt`): ni tramos ni casillas.
    let markdown: Bool
    /// Escribe el texto donde viva (fuera del hilo principal lo hace el).
    var guardar: (@MainActor (String) -> Void)?
    weak var vista: UITextView?
    /// Donde vive la nota (sus comentarios y sus fotos, `NotaMasIPad.swift`)
    /// y el nucleo; sin ellos, ni comentarios ni fotos nuevas.
    var destino: DestinoNota?
    var nucleo: Nucleo?
    var comentarios: ComentariosNota?
    @Published var hilos: [HiloNota] = []
    @Published var viendoComentarios = false
    @Published var menuBarra: MenuBarraNota?
    @Published var tablaEnCursor: TablaMd?
    @Published var editandoTabla: TablaMd?
    @Published var pidiendoFoto = false
    let fotos = FotosDeLaNota()
    private var guardado: String
    private var restilo: DispatchWorkItem?
    private var pendiente: DispatchWorkItem?
    private var lineaDelCursor: UInt32?

    init(texto: String, markdown: Bool = true) {
        guardado = texto
        self.markdown = markdown
        titulo = markdown ? EstiloDeNotaIPad.titulo(texto) : ""
        textoInicial = texto
    }

    let textoInicial: String

    var texto: String { vista?.text ?? guardado }

    func restilar() {
        guard markdown, let v = vista, v.markedTextRange == nil else { return }
        let s = v.textStorage
        let tramos = mdTramos(texto: s.string)
        let seleccion = v.selectedRange
        EstiloDeNotaIPad.aplicar(s, tramos: tramos, lineaDelCursor: lineaDelCursor)
        extrasTrasRestilar(s)
        v.selectedRange = seleccion
        v.typingAttributes = [.font: EstiloDeNotaIPad.letraBase, .foregroundColor: UIColor.label]
    }

    func cambio() {
        programarRestilo()
        programarGuardado()
        mirarBarra()
    }

    fileprivate func cambioDeSeleccion() {
        mirarBarra()
        mirarTabla()
        guard let v = vista, markdown else { return }
        let l = EstiloDeNotaIPad.linea(de: v.selectedRange.location, en: v.text)
        if l != lineaDelCursor {
            lineaDelCursor = l
            programarRestilo()
        }
    }

    private func programarRestilo() {
        restilo?.cancel()
        let tarea = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated { self?.restilar() }
        }
        restilo = tarea
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.12, execute: tarea)
    }

    private func programarGuardado() {
        pendiente?.cancel()
        let tarea = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated { self?.guardarYa() }
        }
        pendiente = tarea
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0, execute: tarea)
    }

    /// **Buscar** en el texto (el buscador del sistema, ⌘F).
    func buscar() {
        vista?.findInteraction?.presentFindNavigator(showingReplace: false)
    }

    /// De que nota es la voz (`LeerEnVoz.suena`).
    var claveDeVoz: String { "nota\u{0}\(ObjectIdentifier(self).hashValue)" }

    /// **Escuchar** desde el renglon del cursor; si ya suena, pausa o sigue.
    /// `false` si no hay nada que leer o el iPad no tiene voz.
    @discardableResult
    func escuchar(clave: String) -> Bool {
        let voz = LeerEnVoz.compartida
        if voz.suena(clave) {
            voz.alternar()
            return true
        }
        let desde = vista?.selectedRange.location ?? 0
        return VozDeLaNotaIPad.escuchar(texto: texto, desde: desde, clave: clave,
                                        titulo: titulo.isEmpty ? "Nota" : titulo, vista: vista)
    }

    /// Escribe si cambio (al cerrar, ya).
    func guardarYa() {
        pendiente?.cancel()
        let actual = texto
        guard actual != guardado else { return }
        guardado = actual
        if markdown { titulo = EstiloDeNotaIPad.titulo(actual) }
        guardar?(actual)
    }

    /// Un boton de la barra: pone (o quita) su marca en lo elegido.
    func marcar(_ m: MarcaNota) {
        guard let v = vista else { return }
        let (nuevo, sel) = EstiloDeNotaIPad.marcar(v.text, elegido: v.selectedRange, marca: m)
        let todo = NSRange(location: 0, length: (v.text as NSString).length)
        if let inicio = v.position(from: v.beginningOfDocument, offset: 0),
           let fin = v.position(from: inicio, offset: todo.length),
           let rango = v.textRange(from: inicio, to: fin) {
            // Por `replace` para que deshacer lo recoja.
            v.replace(rango, withText: nuevo)
        } else {
            v.text = nuevo
        }
        v.selectedRange = sel
        cambio()
        restilar()
    }

    /// Un toque en una casilla (`[ ]`/`[x]`) la marca o desmarca.
    fileprivate func toqueEnCasilla(_ i: Int) -> Bool {
        guard markdown, let v = vista else { return false }
        let s = v.text ?? ""
        guard let t = mdTramos(texto: s).first(where: {
            if case .casilla = $0.estilo { return Int($0.desde) <= i && i < Int($0.hasta) }
            return false
        }) else { return false }
        let pos = Int(t.desde) + 1
        let ns = s as NSString
        guard pos < ns.length, let a = v.position(from: v.beginningOfDocument, offset: pos),
              let b = v.position(from: a, offset: 1), let r = v.textRange(from: a, to: b) else { return false }
        let ahora = ns.substring(with: NSRange(location: pos, length: 1))
        v.replace(r, withText: ahora == " " ? "x" : " ")
        cambio()
        return true
    }

    /// Intro: sigue la lista (o la corta si el renglon solo tenia la marca).
    fileprivate func intro(_ v: UITextView) -> Bool {
        guard markdown else { return true }
        let ns = v.text as NSString
        let cursor = v.selectedRange.location
        let renglon = ns.lineRange(for: NSRange(location: cursor, length: 0))
        var sinSalto = ns.substring(with: renglon)
        while sinSalto.hasSuffix("\n") || sinSalto.hasSuffix("\r") { sinSalto.removeLast() }
        switch mdContinuar(renglon: sinSalto) {
        case .nada:
            return true
        case let .con(prefijo):
            v.insertText("\n" + prefijo)
            cambio()
            return false
        case .cortar:
            guard let a = v.position(from: v.beginningOfDocument, offset: renglon.location),
                  let b = v.position(from: a, offset: (sinSalto as NSString).length),
                  let r = v.textRange(from: a, to: b) else { return true }
            v.replace(r, withText: "")
            cambio()
            return false
        }
    }
}

/// El `UITextView` de la nota.
struct TextoNotaIPad: UIViewRepresentable {
    let modelo: ModeloTextoNota

    func makeCoordinator() -> Coordinador { Coordinador(modelo: modelo) }

    func makeUIView(context: Context) -> UITextView {
        let v = TextoNotaUIView()
        let m = modelo
        v.alCambiarDeAncho = { MainActor.assumeIsolated { m.restilar() } }
        v.font = modelo.markdown ? EstiloDeNotaIPad.letraBase : UIFont.monospacedSystemFont(ofSize: 15, weight: .regular)
        v.textColor = .label
        v.backgroundColor = .systemBackground
        v.textContainerInset = UIEdgeInsets(top: 20, left: 24, bottom: 40, right: 24)
        v.autocorrectionType = .default
        v.smartQuotesType = .no
        v.smartDashesType = .no
        v.alwaysBounceVertical = true
        v.keyboardDismissMode = .interactive
        // Buscar (y reemplazar) del sistema: ⌘F, ⌘G.
        v.isFindInteractionEnabled = true
        v.text = modelo.textoInicial
        v.delegate = context.coordinator
        let toque = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinador.toque(_:)))
        toque.delegate = context.coordinator
        v.addGestureRecognizer(toque)
        modelo.vista = v
        modelo.restilar()
        return v
    }

    func updateUIView(_ v: UITextView, context: Context) {}

    @MainActor
    final class Coordinador: NSObject, UITextViewDelegate, UIGestureRecognizerDelegate {
        let modelo: ModeloTextoNota

        init(modelo: ModeloTextoNota) { self.modelo = modelo }

        func textViewDidChange(_ textView: UITextView) { modelo.cambio() }

        func textViewDidChangeSelection(_ textView: UITextView) { modelo.cambioDeSeleccion() }

        func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
            guard text == "\n", range.length == 0 else { return true }
            return modelo.intro(textView)
        }

        @objc func toque(_ g: UITapGestureRecognizer) {
            guard let v = g.view as? UITextView, g.state == .ended else { return }
            let p = g.location(in: v)
            guard let pos = v.closestPosition(to: p) else { return }
            let i = v.offset(from: v.beginningOfDocument, to: pos)
            _ = modelo.toqueEnCasilla(i)
        }

        func gestureRecognizer(_ g: UIGestureRecognizer,
                               shouldRecognizeSimultaneouslyWith otro: UIGestureRecognizer) -> Bool {
            true
        }
    }
}

/// La barra de Markdown (encima del texto): los botones de `MarcaNota`, la
/// tabla y la foto.
struct BarraMarkdownIPad: View {
    @ObservedObject var modelo: ModeloTextoNota

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 4) {
                ForEach(MarcaNota.allCases, id: \.self) { m in
                    Button { modelo.marcar(m) } label: {
                        Image(systemName: m.icono).frame(width: 36, height: 32)
                    }
                    .buttonStyle(.borderless)
                    .help(m.ayuda)
                    .accessibilityLabel(m.ayuda)
                    .modifier(AtajoDeMarca(marca: m))
                }
                Divider().frame(height: 22)
                Button { modelo.editarTabla() } label: {
                    Image(systemName: modelo.tablaEnCursor == nil ? "tablecells.badge.ellipsis" : "tablecells")
                        .frame(width: 36, height: 32)
                        .foregroundStyle(modelo.tablaEnCursor == nil ? Color.primary : Paleta.v2.acento)
                }
                .buttonStyle(.borderless)
                .help(modelo.tablaEnCursor == nil ? "Insertar una tabla" : "Editar la tabla")
                .accessibilityLabel(modelo.tablaEnCursor == nil ? "Insertar una tabla" : "Editar la tabla")
                if modelo.destino != nil {
                    Button { modelo.pidiendoFoto = true } label: {
                        Image(systemName: "photo").frame(width: 36, height: 32)
                    }
                    .buttonStyle(.borderless)
                    .help("Poner una foto")
                    .accessibilityLabel("Poner una foto")
                }
            }
            .padding(.horizontal, 12)
        }
        .frame(height: 40)
    }
}

private struct AtajoDeMarca: ViewModifier {
    let marca: MarcaNota

    func body(content: Content) -> some View {
        switch marca {
        case .negrita: content.keyboardShortcut("b", modifiers: .command)
        case .cursiva: content.keyboardShortcut("i", modifiers: .command)
        default: content
        }
    }
}

/// Lo comun del editor de una nota y del de un `.md` suelto.
struct EditorTextoIPad: View {
    let tipo: String
    let icono: String
    @ObservedObject var modelo: ModeloTextoNota
    var tituloFijo: String?
    /// El fichero que se ve (un `.md` suelto): «Fijar como pin» lo fija; sin
    /// el (la nota del chat), se fija lo que se ve.
    var fichero: URL?
    let alCerrar: () -> Void
    @ObservedObject private var noche = ModoNoche.compartido
    @ObservedObject private var voz = LeerEnVoz.compartida
    @EnvironmentObject private var nav: Navegacion
    @State private var sinVoz = false
    @State private var fotoElegida: PhotosPickerItem?

    private var claveVoz: String { fichero?.path ?? modelo.claveDeVoz }

    var body: some View {
        VStack(spacing: 0) {
            BarraVisorIPad(tipo: tipo, icono: icono, titulo: tituloFijo ?? modelo.titulo,
                           alFijar: {
                               if let u = fichero, GestorPinesIPad.compartido.pinear(url: u) { return }
                               GestorPinesIPad.compartido.pinearLoQueSeVe()
                           },
                           alCerrar: {
                               modelo.guardarYa()
                               alCerrar()
                               nav.cerrarVisor()
                           }) {
                Button { modelo.buscar() } label: { Image(systemName: "magnifyingglass") }
                    .help("Buscar (⌘F)")
                Button {
                    if !modelo.escuchar(clave: claveVoz) { sinVoz = true }
                } label: {
                    Image(systemName: voz.suena(claveVoz) && voz.leyendo ? "speaker.wave.2.fill" : "speaker.wave.2")
                }
                .help("Escuchar desde el cursor")
                if modelo.comentarios != nil {
                    Button { modelo.viendoComentarios.toggle() } label: {
                        Image(systemName: modelo.viendoComentarios ? "text.bubble.fill" : "text.bubble")
                            .overlay(alignment: .topTrailing) {
                                if modelo.abiertos > 0 {
                                    Text("\(modelo.abiertos)").font(.system(size: 9, weight: .bold))
                                        .foregroundStyle(.white)
                                        .padding(.horizontal, 3)
                                        .background(Capsule().fill(Paleta.v2.rojo))
                                        .offset(x: 8, y: -6)
                                }
                            }
                    }
                    .help("Comentarios")
                    .accessibilityLabel("Comentarios, \(modelo.abiertos) abiertos")
                }
                ShareLink(item: modelo.texto) { Image(systemName: "square.and.arrow.up") }
                    .help("Compartir el texto")
            }
            if modelo.markdown {
                BarraMarkdownIPad(modelo: modelo)
                Divider()
            }
            HStack(spacing: 0) {
                TextoNotaIPad(modelo: modelo)
                    .ignoresSafeArea(.container, edges: .bottom)
                    .overlay(alignment: .bottom) {
                        BarraEscuchar(voz: voz, clave: { claveVoz }).padding(.bottom, 16)
                    }
                    .overlay(alignment: .topLeading) {
                        if let m = modelo.menuBarra {
                            GeometryReader { g in
                                let alto = CGFloat(m.comandos.count) * 34 + 30
                                let abajo = m.cursor.maxY + 6 + alto <= g.size.height
                                MenuBarraNotaIPad(menu: m) { modelo.elegir($0) }
                                    .offset(x: min(max(m.cursor.minX, 8), max(g.size.width - 270, 8)),
                                            y: abajo ? m.cursor.maxY + 6 : max(m.cursor.minY - 6 - alto, 8))
                            }
                        }
                    }
                if modelo.viendoComentarios, modelo.comentarios != nil {
                    Divider()
                    PanelComentariosNotaIPad(modelo: modelo)
                }
            }
        }
        .sheet(isPresented: Binding(get: { modelo.editandoTabla != nil },
                                    set: { if !$0 { modelo.editandoTabla = nil } })) {
            if let t = modelo.editandoTabla {
                EditorTablaNotaIPad(tabla: t) { filas, alineaciones in
                    modelo.guardarTabla(t, filas: filas, alineaciones: alineaciones)
                }
            }
        }
        .photosPicker(isPresented: $modelo.pidiendoFoto, selection: $fotoElegida, matching: .images)
        .onChange(of: fotoElegida) { _, item in
            guard let item else { return }
            fotoElegida = nil
            _ = Task<Void, Never> {
                guard let datos = try? await item.loadTransferable(type: Data.self) else { return }
                let jpg = await Task.detached(priority: .userInitiated) {
                    UIImage(data: datos)?.jpegData(compressionQuality: 0.85)
                }.value
                if let jpg { modelo.adjuntarFoto(jpg, nombre: "foto.jpg") }
            }
        }
        .alert("No se puede escuchar", isPresented: $sinVoz) {
            Button("Aceptar", role: .cancel) {}
        } message: {
            Text("La nota no tiene texto que leer o el iPad no tiene voz para su idioma (Ajustes › Accesibilidad › Contenido leído).")
        }
        .background(Color(uiColor: .systemBackground).ignoresSafeArea())
        .modifier(NocheDelVisor(activo: noche.activo))
        .onDisappear {
            modelo.guardarYa()
            alCerrar()
        }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.willResignActiveNotification)) { _ in
            modelo.guardarYa()
        }
    }
}

/// El modo noche de los visores: fuerza el oscuro; si no, manda el sistema.
struct NocheDelVisor: ViewModifier {
    let activo: Bool

    func body(content: Content) -> some View {
        if activo { content.environment(\.colorScheme, .dark) } else { content }
    }
}

// MARK: - La nota del chat

/// **La nota `NOTA` del chat** en su editor de Markdown.
public struct EditorNotaIPad: View {
    let almacen: Almacen
    let chat: String
    let mensaje: MensajeChat
    @StateObject private var modelo: ModeloTextoNota

    public init(almacen: Almacen, chat: String, mensaje: MensajeChat) {
        self.almacen = almacen
        self.chat = chat
        self.mensaje = mensaje
        // Con el texto ya guardado si el chat lo tiene mas nuevo.
        let actual = almacen.mensajes[chat]?.first { $0.id == mensaje.id } ?? mensaje
        _modelo = StateObject(wrappedValue: ModeloTextoNota(texto: actual.texto))
    }

    public var body: some View {
        EditorTextoIPad(tipo: "Nota", icono: "note.text", modelo: modelo, alCerrar: {})
            .onAppear { conectar() }
    }

    private func conectar() {
        let nucleo = almacen.nucleo, chat = self.chat, id = mensaje.id
        let almacen = self.almacen
        modelo.nucleo = nucleo
        modelo.destino = .mensaje(chat: chat, mensaje: id)
        modelo.abrirComentarios()
        modelo.guardar = { texto in
            Task {
                let hecho = await Task.detached(priority: .utility) { () -> Result<MensajeChat, Error> in
                    Result { try nucleo.editarNota(chat: chat, mensaje: id, texto: texto) }
                }.value
                switch hecho {
                case let .success(m): almacen.sustituir(m, en: chat)
                case let .failure(e): almacen.ponerError("No se pudo guardar la nota: \(e)")
                }
            }
        }
    }
}

// MARK: - Un Markdown o un texto suelto

/// **Un `.md` (o un `.txt`) suelto**: se lee y se escribe en su fichero,
/// con el mismo editor que las notas.
struct VisorMarkdownIPad: View {
    let url: URL
    let titulo: String
    /// Para los comentarios y las fotos de un `.md` (a su lado).
    var nucleo: Nucleo?
    @StateObject private var modelo: ModeloTextoNota
    @State private var error: String?

    init(url: URL, titulo: String, texto: String, nucleo: Nucleo? = nil) {
        self.url = url
        self.titulo = titulo
        self.nucleo = nucleo
        let ext = url.pathExtension.lowercased()
        _modelo = StateObject(wrappedValue: ModeloTextoNota(texto: texto, markdown: ext == "md" || ext == "markdown"))
    }

    var body: some View {
        EditorTextoIPad(tipo: modelo.markdown ? "Markdown" : "Texto",
                        icono: modelo.markdown ? "doc.plaintext" : "doc.text",
                        modelo: modelo, tituloFijo: titulo, fichero: url, alCerrar: {})
            .overlay(alignment: .bottom) {
                if let error {
                    Text(error).font(.callout).padding(8)
                        .background(Paleta.v2.rojo.opacity(0.25), in: Capsule())
                        .padding(.bottom, 20)
                        .onTapGesture { self.error = nil }
                }
            }
            .onAppear {
                let url = self.url
                if modelo.markdown, let nucleo {
                    modelo.nucleo = nucleo
                    modelo.destino = .fichero(ruta: url.path)
                    modelo.abrirComentarios()
                }
                modelo.guardar = { texto in
                    Task {
                        let fallo = await Task.detached(priority: .utility) { () -> String? in
                            do {
                                try Data(texto.utf8).write(to: url, options: .atomic)
                                return nil
                            } catch {
                                return "No se pudo guardar: \(error.localizedDescription)"
                            }
                        }.value
                        self.error = fallo
                        if fallo == nil { NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil) }
                    }
                }
            }
    }
}
