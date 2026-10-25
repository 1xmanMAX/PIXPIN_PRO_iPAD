// **Buscar dentro del chat** (la lupa de la cabecera de Windows, ⌘F como
// su Ctrl+F; `busqueda` de `Abierto` en `ventana_chat.rs`):
//
// - ⌘F abre la caja arriba del chat; con la caja abierta y vacia se ve la
//   conversacion entera;
// - al escribir, solo quedan las burbujas que casan (el texto o lo dicho en
//   una voz —su resumen— y el nombre del fichero; sin mayusculas ni
//   tildes), con lo buscado resaltado en las notas, y encima de la caja de
//   escribir la barra «N resultados · Ver en contexto»;
// - bajo la caja, los chips de las etiquetas que de verdad hay puestas
//   filtran por su emoji (pulsado otra vez, se quita);
// - ↑ y ↓ (o ⇧⌘G y ⌘G) van de un resultado a otro, «3 de 12»; Intro o
//   «Ver en contexto» quitan el filtro y llevan al elegido en su sitio;
// - Esc apaga la lupa, antes que nada mas.
//
// El Mac carga el chat por paginas (§4bis.2 del plan): para buscar en todo
// el chat se lee entero una vez, fuera del hilo principal, al abrir la lupa,
// y se filtra en memoria al escribir. Al cerrarla se suelta.

import PixPinCore
import SwiftUI

/// Si un mensaje casa con lo buscado (`casa_la_busqueda` de Windows): en su
/// texto (el resumen, que en una voz es lo dicho) o en el nombre del
/// fichero, sin mirar mayusculas ni tildes. En blanco casa con todo: con la
/// caja abierta y vacia se ven todas.
func casaLaBusqueda(texto: String, nombre: String, aguja: String) -> Bool {
    let a = aguja.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !a.isEmpty else { return true }
    let opciones: String.CompareOptions = [.caseInsensitive, .diacriticInsensitive]
    return texto.range(of: a, options: opciones) != nil || nombre.range(of: a, options: opciones) != nil
}

/// Las etiquetas puestas (`emojis_usados`): las conocidas en su orden y,
/// detras, las que lleguen de otro sitio, en el orden en que salen.
func etiquetasUsadas(_ puestas: [String?]) -> [String] {
    var vistas: [String] = []
    for e in puestas {
        let e = (e ?? "").trimmingCharacters(in: .whitespaces)
        if !e.isEmpty, !vistas.contains(e) { vistas.append(e) }
    }
    var v = MenuMensaje.etiquetas.filter { vistas.contains($0) }
    for e in vistas where !v.contains(e) { v.append(e) }
    return v
}

/// Un mensaje encontrado y su posicion en el chat (0 el mas antiguo).
struct HalladoEnChat: Identifiable {
    let mensaje: MensajeChat
    let posicion: UInt64
    var id: String { mensaje.id }
}

@MainActor
final class BuscadorDelChat: ObservableObject {
    let chat: String
    private let nucleo: Nucleo
    @Published private(set) var abierto = false
    @Published var texto = "" {
        didSet { if texto != oldValue { filtrar() } }
    }
    /// El chip puesto, si hay uno.
    @Published var etiqueta: String? {
        didSet { if etiqueta != oldValue { filtrar() } }
    }
    @Published private(set) var hallados: [HalladoEnChat] = []
    /// El resultado elegido con ↑ ↓ (indice en `hallados`).
    @Published var elegido: Int?
    @Published private(set) var cargando = false
    @Published private(set) var chips: [String] = []
    /// Sube para devolver el foco a la caja (⌘F con la lupa ya abierta).
    @Published private(set) var focoPedido = 0
    /// El chat entero, mientras la lupa esta abierta (posicion = indice).
    private var todos: [MensajeChat] = []
    private var turno = 0
    /// La lectura del chat en curso (las pruebas la esperan).
    private(set) var carga: Task<Void, Never>?

    /// Lo que se lee de cada vez al cargar el chat entero.
    nonisolated static let tamPagina: UInt32 = 1000

    init(chat: String, nucleo: Nucleo) {
        self.chat = chat
        self.nucleo = nucleo
    }

    /// Hay algo que filtra: lo escrito o un chip.
    var filtrando: Bool {
        abierto && (!texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || etiqueta != nil)
    }

    /// «3 de 12», «Nada con eso» o nada (sin filtro).
    var cuenta: String? {
        guard filtrando, !cargando else { return nil }
        guard !hallados.isEmpty else { return "Nada con eso" }
        return "\((elegido ?? hallados.count - 1) + 1) de \(hallados.count)"
    }

    /// La barra de encima de la isla: sin nada escrito no hay recuento
    /// (decir «312 resultados» no informa de nada), y con cero ya esta el
    /// «Nada con eso».
    var resultados: String? {
        guard filtrando, !hallados.isEmpty else { return nil }
        return hallados.count == 1 ? "1 resultado" : "\(hallados.count) resultados"
    }

    /// Abre la lupa (o, abierta, le devuelve el foco) y lee el chat.
    func abrir() {
        focoPedido += 1
        guard !abierto else { return }
        abierto = true
        carga = Task { await cargar() }
    }

    /// Apaga la lupa: se va lo escrito, el chip y lo leido.
    func cerrar() {
        turno += 1
        abierto = false
        texto = ""
        etiqueta = nil
        hallados = []
        elegido = nil
        todos = []
        chips = []
        cargando = false
    }

    /// Lee el chat entero, por paginas y fuera del hilo principal.
    private func cargar() async {
        turno += 1
        let mio = turno
        cargando = true
        let nucleo = self.nucleo
        let chat = self.chat
        let leidos = await Task.detached(priority: .userInitiated) { () -> [MensajeChat] in
            var todos: [MensajeChat] = []
            var desde: UInt64 = 0
            while true {
                guard let p = try? nucleo.mensajes(chat: chat, desde: desde, cuantos: BuscadorDelChat.tamPagina)
                else { break }
                todos.append(contentsOf: p.mensajes)
                desde = p.desde + UInt64(p.mensajes.count)
                if p.mensajes.isEmpty || desde >= p.total { break }
            }
            return todos
        }.value
        guard mio == turno, abierto else { return }
        todos = leidos
        chips = etiquetasUsadas(leidos.map(\.emoji))
        cargando = false
        filtrar()
    }

    private func filtrar() {
        guard filtrando, !cargando else {
            hallados = []
            elegido = nil
            return
        }
        hallados = BuscadorDelChat.filtrar(todos, aguja: texto, etiqueta: etiqueta)
        // El mas nuevo, que es lo que queda abajo, a la vista.
        elegido = hallados.isEmpty ? nil : hallados.count - 1
    }

    /// Los que pasan los dos filtros: lo escrito y el chip (`se_ve`).
    nonisolated static func filtrar(_ todos: [MensajeChat], aguja: String, etiqueta: String?) -> [HalladoEnChat] {
        todos.enumerated().compactMap { i, m in
            let texto = m.resumen.isEmpty ? m.texto : m.resumen
            guard casaLaBusqueda(texto: texto, nombre: m.nombre, aguja: aguja) else { return nil }
            if let etiqueta, m.emoji != etiqueta { return nil }
            return HalladoEnChat(mensaje: m, posicion: UInt64(i))
        }
    }

    /// Pasa al resultado de al lado (+1 el siguiente, mas nuevo); da la
    /// vuelta en los extremos, como ⌘G.
    func mover(_ paso: Int) {
        guard !hallados.isEmpty else { return }
        let n = hallados.count
        elegido = (((elegido ?? n - 1) + paso) % n + n) % n
    }

    /// El salto al elegido en su sitio, y la lupa apagada («Ver en
    /// contexto»): quien buscaba no se queda sin saber donde estaba.
    func enContexto() -> SaltoAMensaje? {
        guard let h = hallados[safe: elegido ?? 0] else { return nil }
        let salto = SaltoAMensaje(chat: chat, mensaje: h.mensaje.id, posicion: h.posicion)
        cerrar()
        return salto
    }

    /// El chip pulsado: lo pone, o lo quita si ya estaba.
    func alternar(_ e: String) {
        etiqueta = etiqueta == e ? nil : e
    }
}

// MARK: - Las vistas

/// La caja de la lupa, arriba del chat, con su recuento, ↑ ↓ y ✕; debajo,
/// los chips de las etiquetas.
struct BarraBuscarEnChat: View {
    @ObservedObject var buscador: BuscadorDelChat
    let verEnContexto: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(Paleta.proyectos.apagado)
                CampoDeUnaLinea(texto: $buscador.texto, invitacion: "Buscar aquí…", tamano: 13,
                                pedirFoco: buscador.focoPedido) { t in tecla(t) }
                if buscador.cargando {
                    ProgressView().controlSize(.small)
                } else if let cuenta = buscador.cuenta {
                    Text(cuenta)
                        .font(.system(size: 12).monospacedDigit())
                        .foregroundStyle(Paleta.proyectos.apagado)
                        .fixedSize()
                }
                boton("chevron.up", "Anterior (↑ o ⇧⌘G)") { buscador.mover(-1) }
                    .disabled(buscador.hallados.count < 2)
                boton("chevron.down", "Siguiente (↓ o ⌘G)") { buscador.mover(1) }
                    .disabled(buscador.hallados.count < 2)
                boton("xmark", "Cerrar la búsqueda (Esc)") { buscador.cerrar() }
            }
            .padding(.horizontal, 12)
            .frame(height: 34)
            .background(Paleta.proyectos.buscador, in: Capsule())
            if !buscador.chips.isEmpty {
                HStack(spacing: 6) {
                    ForEach(buscador.chips, id: \.self) { e in
                        Button { buscador.alternar(e) } label: {
                            Text(e)
                                .font(.system(size: 14))
                                .padding(.horizontal, 10)
                                .frame(height: 28)
                                .background(buscador.etiqueta == e ? Paleta.chat.enviar.opacity(0.35)
                                                                   : Paleta.proyectos.texto.opacity(0.08),
                                            in: Capsule())
                                .contentShape(Capsule())
                        }
                        .buttonStyle(.plain)
                        .help(buscador.etiqueta == e ? "Quitar el filtro" : "Solo los que llevan \(e)")
                    }
                }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(Paleta.chat.cabecera)
    }

    private func boton(_ simbolo: String, _ ayuda: String, _ hacer: @escaping () -> Void) -> some View {
        Button(action: hacer) {
            Image(systemName: simbolo)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(Paleta.proyectos.apagado)
                .frame(width: 22, height: 22)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(ayuda)
    }

    private func tecla(_ t: TeclaDeCampo) -> Bool {
        switch t {
        case .flecha(.arriba): buscador.mover(-1)
        case .flecha(.abajo): buscador.mover(1)
        case .intro: verEnContexto()
        case .introConMayusculas: buscador.mover(-1)
        case .escape: buscador.cerrar()
        case .flecha: return false
        }
        return true
    }
}

/// «N resultados · Ver en contexto», encima de la caja de escribir.
struct BarraDeResultados: View {
    let rotulo: String
    let verEnContexto: () -> Void

    var body: some View {
        HStack {
            Text(rotulo)
                .font(.system(size: 12))
                .foregroundStyle(Paleta.proyectos.texto)
            Spacer()
            Button("Ver en contexto", action: verEnContexto)
                .buttonStyle(.plain)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Paleta.chat.enviar)
                .help("Quitar el filtro e ir al elegido en su sitio (Intro)")
        }
        .padding(.horizontal, 14)
        .frame(height: 34)
        .background(Paleta.chat.campo, in: RoundedRectangle(cornerRadius: 16))
        .padding(.horizontal, MedidasIsla.margenX)
        .padding(.top, 6)
    }
}

private struct ClaveAgujaDelChat: EnvironmentKey {
    static let defaultValue = ""
}

extension EnvironmentValues {
    /// Lo que se busca en el chat (para resaltarlo en las burbujas).
    var agujaDelChat: String {
        get { self[ClaveAgujaDelChat.self] }
        set { self[ClaveAgujaDelChat.self] = newValue }
    }
}

/// `texto` con cada aparicion de `aguja` resaltada (sin mayusculas ni
/// tildes), como `resaltado::coincidencias` de Windows.
func textoResaltado(_ crudo: String, aguja: String) -> AttributedString {
    var a = textoConFormato(crudo)
    let texto = String(a.characters)
    conEnlaces(&a)
    let q = aguja.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !q.isEmpty else { return a }
    var desde = texto.startIndex
    while desde < texto.endIndex,
          let r = texto.range(of: q, options: [.caseInsensitive, .diacriticInsensitive], range: desde..<texto.endIndex) {
        if let ar = Range(r, in: a) {
            a[ar].backgroundColor = Color.yellow.opacity(0.55)
            a[ar].foregroundColor = Color.black
        }
        desde = r.upperBound
    }
    return a
}

/// **Una nota con su formato** (como en Windows y el movil): sin los
/// caracteres de control que dejo el teclado (un DEL de ⌥⌫), los titulos en
/// negrita, las vinetas como «•» y lo de dentro de la linea en Markdown
/// (**negrita**, *cursiva*, `codigo`, [enlaces](…)).
func textoConFormato(_ crudo: String) -> AttributedString {
    let limpio = String(String.UnicodeScalarView(crudo.unicodeScalars.filter {
        !(CharacterSet.controlCharacters.contains($0) && $0 != "\n" && $0 != "\t")
    }))
    var lineas: [String] = []
    for l in limpio.components(separatedBy: "\n") {
        let t = l.trimmingCharacters(in: .whitespaces)
        if let r = t.range(of: #"^#{1,6}\s+"#, options: .regularExpression) {
            let titulo = String(t[r.upperBound...])
            lineas.append(titulo.isEmpty ? "" : "**\(titulo)**")
        } else if let r = t.range(of: #"^[-*+]\s+\[[ xX]\]\s*"#, options: .regularExpression) {
            let hecha = t[r].contains("x") || t[r].contains("X")
            lineas.append((hecha ? "☑︎ ~~" : "☐ ") + t[r.upperBound...] + (hecha ? "~~" : ""))
        } else if let r = t.range(of: #"^[-*+]\s+"#, options: .regularExpression) {
            let sangria = String(l.prefix { $0 == " " }.prefix(6))
            lineas.append(sangria + "• " + t[r.upperBound...])
        } else {
            lineas.append(l)
        }
    }
    let md = lineas.joined(separator: "\n")
    let op = AttributedString.MarkdownParsingOptions(interpretedSyntax: .inlineOnlyPreservingWhitespace,
                                                     failurePolicy: .returnPartiallyParsedIfPossible)
    return (try? AttributedString(markdown: md, options: op)) ?? AttributedString(limpio)
}

/// **Los enlaces pulsables** de una burbuja (Windows, 17-oct): `http`,
/// `https` y `www.` subrayados y en color de enlace; un clic los abre.
func conEnlaces(_ a: inout AttributedString) {
    let texto = String(a.characters)
    guard texto.contains("http") || texto.contains("www."),
          let detector = try? NSDataDetector(types: NSTextCheckingResult.CheckingType.link.rawValue)
    else { return }
    let ns = texto as NSString
    for m in detector.matches(in: texto, range: NSRange(location: 0, length: ns.length)) {
        guard let url = m.url, ["http", "https"].contains(url.scheme?.lowercased() ?? ""),
              let r = Range(m.range, in: texto), let ar = Range(r, in: a) else { continue }
        // Solo lo escrito como enlace (no un correo ni «ejemplo.com» suelto).
        let escrito = ns.substring(with: m.range).lowercased()
        guard escrito.hasPrefix("http") || escrito.hasPrefix("www.") else { continue }
        a[ar].link = url
        a[ar].underlineStyle = .single
    }
}
