// **Lo que la nota trae ademas del texto** (Windows `crates/pixpin-notas`:
// `panel_comentarios.rs`, `imagenes.rs`, `menu.rs`, las tablas; Mac
// `EditorNotaWeb.swift`), sobre el `UITextView` cuyo texto ES el Markdown:
//
// - **Comentarios al margen** (`Nucleo.abrirComentariosDeNota`, el fichero
//   hermano `anot-<codigo>.comentarios.json` que viaja con la nota): lo
//   comentado se resalta en el texto; el panel de la derecha tiene los
//   hilos, responder, resolver y borrar lo propio. El ancla es una cita con
//   su contexto: si el texto cambia, el nucleo la vuelve a encontrar.
// - **Fotos bajo su renglon**: cada `![…](ruta)` deja debajo el hueco de la
//   foto (el espacio tras el parrafo) y la foto se pone ahi encima del
//   texto, como en Windows. Una foto nueva (la barra o `/imagen`) se copia
//   junto a la nota (`Nucleo.adjuntarANota`) para que viaje con ella.
// - **Tablas editables**: con el cursor en una tabla de barras, «Editar la
//   tabla» la abre como rejilla (celdas, filas, columnas, alineacion) y la
//   vuelve a escribir como el movil (`mdTablaTexto`).
// - **El menu `/`**: una barra al principio del renglon abre los bloques de
//   Windows (`mdBarraComandos`), lo tecleado los filtra y al elegir uno la
//   barra se cambia por el (`mdBarraPoner`).

import PhotosUI
import PixPinCore
import SwiftUI
import UIKit

/// El menu de la barra abierto: donde esta la barra, lo tecleado detras, lo
/// que encaja y donde se ve el cursor (en la vista del texto).
struct MenuBarraNota: Equatable {
    var barra: Int
    var escrito: String
    var comandos: [ComandoBarraMd]
    var cursor: CGRect
}

/// Las fotos de la nota ya leidas (por ruta en este iPad) y sus vistas.
@MainActor
final class FotosDeLaNota {
    /// La ruta del Markdown a la de este iPad (`nil`: no esta).
    var rutas: [String: String?] = [:]
    var imagenes: [String: UIImage] = [:]
    var leyendo: Set<String> = []
    /// Una vista por renglon con foto.
    var vistas: [UInt32: UIImageView] = [:]
    /// El alto que se dejo debajo de cada renglon con foto.
    var huecos: [UInt32: CGFloat] = [:]
}

/// Lo que mide una foto bajo su renglon: a lo ancho de la columna (o al
/// ancho pedido, en pixeles a 96 ppp) y como mucho 480 de alto. Pura.
func tamanoDeFotoEnNota(natural: CGSize, columna: CGFloat, pedido: UInt32?) -> CGSize {
    guard natural.width > 0, natural.height > 0, columna > 0 else { return .zero }
    var ancho = min(columna, natural.width)
    if let p = pedido { ancho = min(CGFloat(p), columna) }
    var alto = ancho * natural.height / natural.width
    if alto > 480 {
        alto = 480
        ancho = alto * natural.width / natural.height
    }
    return CGSize(width: ancho.rounded(), height: alto.rounded())
}

extension ModeloTextoNota {
    // MARK: Reemplazar texto (por `replace`, para que deshacer lo recoja)

    func reemplazar(_ r: NSRange, con nuevo: String, cursor: Int? = nil) {
        guard let v = vista else { return }
        let largo = (v.text as NSString).length
        let r = NSRange(location: min(r.location, largo), length: min(r.length, largo - min(r.location, largo)))
        if let a = v.position(from: v.beginningOfDocument, offset: r.location),
           let b = v.position(from: a, offset: r.length),
           let rango = v.textRange(from: a, to: b) {
            v.replace(rango, withText: nuevo)
        }
        if let cursor { v.selectedRange = NSRange(location: cursor, length: 0) }
        cambio()
        restilar()
    }

    /// Lo que se hace tras restilar: lo comentado y el hueco de las fotos.
    func extrasTrasRestilar(_ s: NSTextStorage) {
        if let c = comentarios {
            hilos = c.hilos(texto: s.string)
            let largo = s.length
            for h in hilos where !h.resuelto {
                guard let a = h.desde, let b = h.hasta, Int(b) <= largo, a < b else { continue }
                s.addAttribute(.backgroundColor, value: UIColor.systemYellow.withAlphaComponent(0.32),
                               range: NSRange(location: Int(a), length: Int(b - a)))
            }
        }
        dejarHuecoParaFotos(s)
        DispatchQueue.main.async { [weak self] in
            MainActor.assumeIsolated { self?.colocarFotos() }
        }
    }

    // MARK: Comentarios

    func abrirComentarios() {
        guard comentarios == nil, let nucleo, let destino else { return }
        _ = Task<Void, Never> {
            let c = await Task.detached(priority: .utility) { try? nucleo.abrirComentariosDeNota(destino: destino) }.value
            guard let c else { return }
            self.comentarios = c
            self.restilar()
        }
    }

    var abiertos: Int { hilos.filter { !$0.resuelto }.count }

    /// Lo elegido en el texto (o la palabra del cursor, si no hay nada).
    var elegido: NSRange { vista?.selectedRange ?? NSRange(location: 0, length: 0) }

    func comentar(_ cuerpo: String) {
        guard let c = comentarios else { return }
        let r = elegido
        if c.nuevo(texto: texto, desde: UInt32(r.location), hasta: UInt32(r.location + r.length), cuerpo: cuerpo) != nil {
            viendoComentarios = true
            restilar()
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        }
    }

    func responder(_ hilo: HiloNota, _ cuerpo: String) {
        guard !cuerpo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        comentarios?.responder(hilo: hilo.id, cuerpo: cuerpo)
        restilar()
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
    }

    func resolver(_ hilo: HiloNota) {
        comentarios?.resolver(hilo: hilo.id, si: !hilo.resuelto)
        restilar()
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
    }

    func borrarComentario(_ id: String) {
        comentarios?.borrar(id: id)
        restilar()
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
    }

    func releerComentarios() {
        comentarios?.releer()
        restilar()
    }

    /// Lo de un hilo, elegido en el texto y a la vista.
    func ir(a h: HiloNota) {
        guard let v = vista, let a = h.desde, let b = h.hasta else { return }
        let r = NSRange(location: Int(a), length: Int(b - a))
        guard NSMaxRange(r) <= (v.text as NSString).length else { return }
        v.selectedRange = r
        v.scrollRangeToVisible(r)
    }

    var yo: String { comentarios?.yo() ?? "" }

    // MARK: El menu de la barra

    /// Si el cursor escribe un comando tras una barra, el menu.
    func mirarBarra() {
        guard markdown, let v = vista, v.selectedRange.length == 0,
              let q = mdBarraConsulta(texto: v.text, cursor: UInt32(v.selectedRange.location)) else {
            if menuBarra != nil { menuBarra = nil }
            return
        }
        let comandos = mdBarraComandos(escrito: q.escrito)
        guard !comandos.isEmpty else {
            menuBarra = nil
            return
        }
        var caret = CGRect(x: 40, y: 40, width: 2, height: 20)
        if let p = v.selectedTextRange?.end {
            caret = v.caretRect(for: p).offsetBy(dx: -v.contentOffset.x, dy: -v.contentOffset.y)
        }
        menuBarra = MenuBarraNota(barra: Int(q.barra), escrito: q.escrito, comandos: comandos, cursor: caret)
    }

    func elegir(_ c: ComandoBarraMd) {
        guard let m = menuBarra, let v = vista else { return }
        menuBarra = nil
        let hasta = v.selectedRange.location
        if c.clave == "imagen" {
            reemplazar(NSRange(location: m.barra, length: hasta - m.barra), con: "", cursor: m.barra)
            pidiendoFoto = true
            return
        }
        let ahora = Int64((Date().timeIntervalSince1970 + Double(TimeZone.current.secondsFromGMT())) * 1000)
        guard let p = mdBarraPoner(texto: v.text, desde: UInt32(m.barra), hasta: UInt32(hasta), clave: c.clave,
                                   ahoraLocalMs: ahora) else { return }
        // Solo cambia lo de la barra: se reemplaza ese trozo.
        let ns = v.text as NSString
        let nuevo = p.texto as NSString
        let cola = ns.length - hasta
        let puesto = nuevo.substring(with: NSRange(location: m.barra, length: nuevo.length - cola - m.barra))
        reemplazar(NSRange(location: m.barra, length: hasta - m.barra), con: puesto, cursor: Int(p.desde))
    }

    // MARK: Tablas

    /// La tabla en la que esta el cursor, si hay una.
    func mirarTabla() {
        guard markdown, let v = vista else { return }
        let i = v.selectedRange.location
        let t = mdTablas(texto: v.text).first { Int($0.desde) <= i && i <= Int($0.hasta) }
        if t != tablaEnCursor { tablaEnCursor = t }
    }

    /// La tabla del cursor a la rejilla; si no hay, una nueva de 3 x 3.
    func editarTabla() {
        mirarTabla()
        if let t = tablaEnCursor {
            editandoTabla = t
            return
        }
        let i = elegido.location
        editandoTabla = TablaMd(desde: UInt32(i), hasta: UInt32(i),
                                filas: Array(repeating: Array(repeating: "", count: 3), count: 3),
                                alineaciones: ["i", "i", "i"])
    }

    /// Lo de la rejilla, al texto.
    func guardarTabla(_ t: TablaMd, filas: [[String]], alineaciones: [String]) {
        var md = mdTablaTexto(filas: filas, alineaciones: alineaciones)
        let ns = texto as NSString
        let r = NSRange(location: Int(t.desde), length: Int(t.hasta - t.desde))
        if r.length == 0 {
            // Una nueva: en su propio parrafo.
            let antes = r.location > 0 ? ns.substring(with: NSRange(location: r.location - 1, length: 1)) : "\n"
            if antes != "\n" { md = "\n" + md }
            md += "\n"
        }
        reemplazar(r, con: md, cursor: r.location + (md as NSString).length)
        tablaEnCursor = nil
    }

    // MARK: Fotos

    /// **Una foto nueva**: se copia junto a la nota y se escribe en su propio
    /// renglon donde esta el cursor.
    func adjuntarFoto(_ datos: Data, nombre: String) {
        guard let nucleo, let destino else { return }
        _ = Task<Void, Never> {
            let ruta = await Task.detached(priority: .userInitiated) {
                try? nucleo.adjuntarANota(destino: destino, nombre: nombre, bytes: datos)
            }.value
            guard let ruta else { return }
            let ns = self.texto as NSString
            let i = min(self.elegido.location, ns.length)
            let antes = i > 0 && ns.substring(with: NSRange(location: i - 1, length: 1)) != "\n" ? "\n" : ""
            let linea = "\(antes)![](\(ruta))\n"
            self.reemplazar(NSRange(location: i, length: 0), con: linea, cursor: i + (linea as NSString).length)
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        }
    }

    private var columnaDeTexto: CGFloat {
        guard let v = vista else { return 0 }
        return v.bounds.width - v.textContainerInset.left - v.textContainerInset.right
            - 2 * v.textContainer.lineFragmentPadding
    }

    /// El hueco bajo cada renglon con foto (el espacio tras su parrafo).
    private func dejarHuecoParaFotos(_ s: NSTextStorage) {
        guard markdown else { return }
        let fotos = mdFotos(texto: s.string)
        let ns = s.string as NSString
        let lineas = Set(fotos.map(\.linea))
        for (l, vista) in self.fotos.vistas where !lineas.contains(l) {
            vista.removeFromSuperview()
            self.fotos.vistas[l] = nil
        }
        self.fotos.huecos = [:]
        let columna = columnaDeTexto
        for f in fotos {
            guard let img = imagen(de: f.ruta) else { continue }
            let tam = tamanoDeFotoEnNota(natural: img.size, columna: columna, pedido: f.ancho)
            guard tam.height > 0, let r = rangoDeLinea(Int(f.linea), en: ns) else { continue }
            let p = NSMutableParagraphStyle()
            p.paragraphSpacing = tam.height + 14
            s.addAttribute(.paragraphStyle, value: p, range: r)
            self.fotos.huecos[f.linea] = tam.height
        }
    }

    private func rangoDeLinea(_ n: Int, en ns: NSString) -> NSRange? {
        var i = 0, linea = 0
        while linea < n {
            let r = ns.range(of: "\n", options: [], range: NSRange(location: i, length: ns.length - i))
            if r.location == NSNotFound { return nil }
            i = r.location + 1
            linea += 1
        }
        let fin = ns.range(of: "\n", options: [], range: NSRange(location: i, length: ns.length - i))
        let largo = (fin.location == NSNotFound ? ns.length : fin.location) - i
        return largo > 0 ? NSRange(location: i, length: largo) : nil
    }

    /// La foto de una ruta del Markdown, si ya esta leida (si no, se lee
    /// fuera del hilo principal y se restila al llegar).
    private func imagen(de ruta: String) -> UIImage? {
        if let i = fotos.imagenes[ruta] { return i }
        guard !fotos.leyendo.contains(ruta), let nucleo, let destino else { return nil }
        if let r = fotos.rutas[ruta], r == nil { return nil }
        fotos.leyendo.insert(ruta)
        _ = Task<Void, Never> {
            let (real, img) = await Task.detached(priority: .utility) { () -> (String?, UIImage?) in
                guard let real = nucleo.rutaEnLaNota(destino: destino, ruta: ruta) else { return (nil, nil) }
                let i = UIImage(contentsOfFile: real)
                return (real, i?.preparingThumbnail(of: FotosDeLaNota.tope(i?.size ?? .zero)) ?? i)
            }.value
            self.fotos.leyendo.remove(ruta)
            self.fotos.rutas[ruta] = .some(real)
            if let img {
                self.fotos.imagenes[ruta] = img
                self.restilar()
            }
        }
        return nil
    }

    /// Pone cada foto bajo su renglon (TextKit 2: el marco del parrafo).
    func colocarFotos() {
        guard markdown, let v = vista, let tlm = v.textLayoutManager else { return }
        let fotos = mdFotos(texto: v.text)
        let columna = columnaDeTexto
        let largo = (v.text as NSString).length
        for f in fotos {
            guard let img = self.fotos.imagenes[f.ruta], self.fotos.huecos[f.linea] != nil,
                  let r = rangoDeLinea(Int(f.linea), en: v.text as NSString), r.location < largo,
                  let loc = tlm.location(tlm.documentRange.location, offsetBy: r.location) else { continue }
            tlm.ensureLayout(for: NSTextRange(location: loc))
            guard let frag = tlm.textLayoutFragment(for: loc) else { continue }
            let marco = frag.layoutFragmentFrame
            let bajo = frag.textLineFragments.last.map { marco.minY + $0.typographicBounds.maxY } ?? marco.maxY
            let tam = tamanoDeFotoEnNota(natural: img.size, columna: columna, pedido: f.ancho)
            let caja = CGRect(x: v.textContainerInset.left + v.textContainer.lineFragmentPadding,
                              y: v.textContainerInset.top + bajo + 7, width: tam.width, height: tam.height)
            let vista = self.fotos.vistas[f.linea] ?? {
                let iv = UIImageView()
                iv.contentMode = .scaleAspectFit
                iv.layer.cornerRadius = 6
                iv.clipsToBounds = true
                iv.isUserInteractionEnabled = false
                v.addSubview(iv)
                self.fotos.vistas[f.linea] = iv
                return iv
            }()
            if vista.image !== img { vista.image = img }
            if vista.frame != caja { vista.frame = caja }
        }
    }
}

extension FotosDeLaNota {
    /// Lo mas grande que hace falta leer de una foto (el doble de la columna
    /// mas ancha de un iPad).
    nonisolated static func tope(_ s: CGSize) -> CGSize {
        guard s.width > 0, s.height > 0 else { return CGSize(width: 1, height: 1) }
        let k = min(1, 2048 / max(s.width, s.height))
        return CGSize(width: s.width * k, height: s.height * k)
    }
}

/// El `UITextView` de la nota: al cambiar de ancho, las fotos se recolocan.
final class TextoNotaUIView: UITextView {
    var alCambiarDeAncho: (() -> Void)?
    private var ancho: CGFloat = 0

    override func layoutSubviews() {
        super.layoutSubviews()
        if abs(bounds.width - ancho) > 0.5 {
            ancho = bounds.width
            alCambiarDeAncho?()
        }
    }
}

// MARK: - Las vistas

/// **El menu de la barra**, junto al cursor.
struct MenuBarraNotaIPad: View {
    let menu: MenuBarraNota
    let elegir: (ComandoBarraMd) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(menu.comandos, id: \.clave) { c in
                Button { elegir(c) } label: {
                    HStack(spacing: 10) {
                        Image(systemName: MenuBarraNotaIPad.icono(c.clave))
                            .frame(width: 22)
                            .foregroundStyle(.secondary)
                        Text(c.nombre)
                        Spacer(minLength: 16)
                        if !c.atajo.isEmpty {
                            Text(c.atajo).font(.system(size: 13).monospaced()).foregroundStyle(.tertiary)
                        }
                    }
                    .frame(height: 34)
                    .padding(.horizontal, 12)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
            Divider()
            Text("Sigue escribiendo para filtrar").font(.caption).foregroundStyle(.secondary)
                .padding(.horizontal, 12).frame(height: 28)
        }
        .frame(width: 260)
        .background(RoundedRectangle(cornerRadius: 10).fill(.regularMaterial))
        .shadow(color: .black.opacity(0.2), radius: 10, y: 3)
    }

    static func icono(_ clave: String) -> String {
        switch clave {
        case "titulo1", "titulo2", "titulo3": "textformat.size"
        case "lista": "list.bullet"
        case "numerada": "list.number"
        case "casillas": "checklist"
        case "cita": "text.quote"
        case "codigo": "chevron.left.forwardslash.chevron.right"
        case "tabla": "tablecells"
        case "imagen": "photo"
        case "fecha": "calendar"
        case "separador": "minus"
        default: "square"
        }
    }
}

/// **El panel de los comentarios** a la derecha del texto.
struct PanelComentariosNotaIPad: View {
    @ObservedObject var modelo: ModeloTextoNota
    @State private var nuevo = ""
    @State private var respuestas: [String: String] = [:]
    @State private var verResueltos = false

    private var abiertos: [HiloNota] { modelo.hilos.filter { !$0.resuelto } }
    private var resueltos: [HiloNota] { modelo.hilos.filter(\.resuelto) }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("Comentarios").font(.headline)
                Spacer()
                Button { modelo.viendoComentarios = false } label: { Image(systemName: "xmark") }
                    .buttonStyle(.borderless)
                    .accessibilityLabel("Cerrar los comentarios")
            }
            .padding(12)
            VStack(alignment: .leading, spacing: 6) {
                TextField("Comentar lo elegido…", text: $nuevo, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(1...4)
                HStack {
                    Text("Elige texto en la nota (o deja el cursor en una palabra).")
                        .font(.caption2).foregroundStyle(.secondary)
                    Spacer()
                    Button("Comentar") {
                        modelo.comentar(nuevo)
                        nuevo = ""
                    }
                    .disabled(nuevo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
            .padding(.horizontal, 12)
            .padding(.bottom, 10)
            Divider()
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 10) {
                    if modelo.hilos.isEmpty {
                        Text("Sin comentarios").foregroundStyle(.secondary).padding(.top, 20)
                    }
                    ForEach(abiertos, id: \.id) { h in tarjeta(h) }
                    if !resueltos.isEmpty {
                        Button(verResueltos ? "Ocultar los resueltos" : "Ver los resueltos (\(resueltos.count))") {
                            verResueltos.toggle()
                        }
                        .font(.callout)
                        if verResueltos { ForEach(resueltos, id: \.id) { h in tarjeta(h) } }
                    }
                }
                .padding(12)
            }
        }
        .frame(width: 320)
        .background(Color(uiColor: .secondarySystemBackground))
        .onAppear { modelo.releerComentarios() }
    }

    private func cuando(_ ms: Int64) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "es")
        f.dateStyle = .short
        f.timeStyle = .short
        return f.string(from: Date(timeIntervalSince1970: Double(ms) / 1000))
    }

    @ViewBuilder private func tarjeta(_ h: HiloNota) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Button { modelo.ir(a: h) } label: {
                Text("«\(h.cita)»").font(.caption).italic().foregroundStyle(.secondary).lineLimit(2)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.plain)
            .disabled(h.desde == nil)
            if h.desde == nil {
                Text("El texto comentado ya no está.").font(.caption2).foregroundStyle(.orange)
            }
            mensaje(autor: h.autor, cuando: h.cuando, texto: h.texto, id: h.id, mio: h.aparato == modelo.yo)
            ForEach(h.respuestas, id: \.id) { r in
                mensaje(autor: r.autor, cuando: r.cuando, texto: r.texto, id: r.id, mio: r.aparato == modelo.yo)
                    .padding(.leading, 12)
            }
            HStack {
                TextField("Responder…", text: Binding(get: { respuestas[h.id] ?? "" },
                                                     set: { respuestas[h.id] = $0 }))
                    .textFieldStyle(.roundedBorder)
                    .font(.callout)
                    .onSubmit {
                        modelo.responder(h, respuestas[h.id] ?? "")
                        respuestas[h.id] = nil
                    }
                Button { modelo.resolver(h) } label: {
                    Image(systemName: h.resuelto ? "arrow.uturn.backward.circle" : "checkmark.circle")
                }
                .buttonStyle(.borderless)
                .help(h.resuelto ? "Volver a abrir" : "Resolver")
                .accessibilityLabel(h.resuelto ? "Volver a abrir" : "Resolver")
            }
        }
        .padding(10)
        .background(RoundedRectangle(cornerRadius: 10).fill(Color(uiColor: .systemBackground)))
        .opacity(h.resuelto ? 0.6 : 1)
    }

    private func mensaje(autor: String, cuando ms: Int64, texto: String, id: String, mio: Bool) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text(autor.isEmpty ? "Alguien" : autor).font(.caption.weight(.semibold))
                Text(cuando(ms)).font(.caption2).foregroundStyle(.secondary)
                Spacer()
                if mio {
                    Button(role: .destructive) { modelo.borrarComentario(id) } label: { Image(systemName: "trash") }
                        .buttonStyle(.borderless)
                        .font(.caption)
                        .accessibilityLabel("Borrar")
                }
            }
            Text(texto).font(.callout).textSelection(.enabled)
        }
    }
}

/// **Una tabla de la nota como rejilla**: escribir en las celdas, poner y
/// quitar filas y columnas y alinear cada columna.
struct EditorTablaNotaIPad: View {
    let tabla: TablaMd
    let guardar: ([[String]], [String]) -> Void
    @Environment(\.dismiss) private var cerrar
    @State private var filas: [[String]]
    @State private var alineaciones: [String]

    init(tabla: TablaMd, guardar: @escaping ([[String]], [String]) -> Void) {
        self.tabla = tabla
        self.guardar = guardar
        let n = max(tabla.filas.map(\.count).max() ?? 1, 1)
        _filas = State(initialValue: tabla.filas.map { f in (0..<n).map { $0 < f.count ? f[$0] : "" } })
        _alineaciones = State(initialValue: (0..<n).map { $0 < tabla.alineaciones.count ? tabla.alineaciones[$0] : "i" })
    }

    private var columnas: Int { alineaciones.count }

    var body: some View {
        NavigationStack {
            ScrollView([.horizontal, .vertical]) {
                Grid(horizontalSpacing: 6, verticalSpacing: 6) {
                    GridRow {
                        Color.clear.frame(width: 28, height: 1)
                        ForEach(0..<columnas, id: \.self) { c in
                            Menu {
                                Button("Izquierda") { alineaciones[c] = "i" }
                                Button("Centro") { alineaciones[c] = "c" }
                                Button("Derecha") { alineaciones[c] = "d" }
                                Divider()
                                Button("Columna antes") { ponerColumna(en: c) }
                                Button("Columna después") { ponerColumna(en: c + 1) }
                                if columnas > 1 {
                                    Button("Quitar la columna", role: .destructive) { quitarColumna(c) }
                                }
                            } label: {
                                Image(systemName: alineaciones[c] == "c" ? "text.aligncenter"
                                      : alineaciones[c] == "d" ? "text.alignright" : "text.alignleft")
                                    .frame(width: 150, height: 28)
                            }
                        }
                    }
                    ForEach(0..<filas.count, id: \.self) { f in
                        GridRow {
                            Menu {
                                Button("Fila antes") { ponerFila(en: f) }
                                Button("Fila después") { ponerFila(en: f + 1) }
                                if filas.count > 2 {
                                    Button("Quitar la fila", role: .destructive) { filas.remove(at: f) }
                                }
                            } label: {
                                Text(f == 0 ? "◆" : "\(f)").font(.caption.monospacedDigit()).frame(width: 28, height: 34)
                            }
                            ForEach(0..<columnas, id: \.self) { c in
                                TextField(f == 0 ? "Cabecera" : "", text: celda(f, c))
                                    .textFieldStyle(.roundedBorder)
                                    .font(f == 0 ? .body.weight(.semibold) : .body)
                                    .multilineTextAlignment(alineaciones[c] == "c" ? .center
                                                            : alineaciones[c] == "d" ? .trailing : .leading)
                                    .frame(width: 150)
                            }
                        }
                    }
                }
                .padding(16)
            }
            .navigationTitle("Tabla")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancelar") { cerrar() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Listo") {
                        guardar(filas, alineaciones)
                        cerrar()
                    }
                }
                ToolbarItemGroup(placement: .bottomBar) {
                    Button { ponerFila(en: filas.count) } label: { Label("Fila", systemImage: "plus") }
                    Button { ponerColumna(en: columnas) } label: { Label("Columna", systemImage: "plus.square") }
                }
            }
        }
    }

    private func celda(_ f: Int, _ c: Int) -> Binding<String> {
        Binding(get: { f < filas.count && c < filas[f].count ? filas[f][c] : "" },
                set: { v in
                    guard f < filas.count, c < filas[f].count else { return }
                    filas[f][c] = v
                })
    }

    private func ponerFila(en i: Int) {
        guard filas.count < 1000 else { return }
        filas.insert(Array(repeating: "", count: columnas), at: min(max(i, 1), filas.count))
    }

    private func ponerColumna(en i: Int) {
        guard columnas < 50 else { return }
        let k = min(max(i, 0), columnas)
        for f in filas.indices { filas[f].insert("", at: min(k, filas[f].count)) }
        alineaciones.insert("i", at: k)
    }

    private func quitarColumna(_ c: Int) {
        guard columnas > 1, c < columnas else { return }
        for f in filas.indices where c < filas[f].count { filas[f].remove(at: c) }
        alineaciones.remove(at: c)
    }
}
