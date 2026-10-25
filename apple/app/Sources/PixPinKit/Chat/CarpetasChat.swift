// **Las carpetas de paginas** («Dia 1, Dia 2…», Windows
// `ventana_chat/carpetas.rs`, 26-oct-2026): un cuaderno con nombre dentro
// del chat que junta lienzos, fotos y paginas sin moverlas, en su orden.
//
// - En el chat, la carpeta es su tarjeta (`BurbujaCarpeta`): el nombre,
//   cuantas paginas lleva y las primeras en pequeno. Un toque la abre.
// - Al abrirla (`VistaCarpeta`, la ventana de Windows): arriba el nombre,
//   «Nueva página» (la plantilla de la ultima otra vez, vacia, con el titulo
//   siguiente) y «Exportar PDF» (la hoja de compartir con sus paginas en
//   orden: PDF, imprimir, guardar en Archivos); debajo, las paginas en
//   rejilla con su miniatura, su numero y su titulo. Se abren con un toque,
//   se cambian de sitio arrastrandolas (o con «Antes» y «Después»), se
//   renombran y se quitan de la carpeta (la pagina sigue en el chat).
// - En el menu de un mensaje, «Añadir a carpeta…» (`MenuEstudio.swift`).
//
// Todo lo que cambia se escribe en el cuaderno por el nucleo
// (`crearCarpeta`, `cambiarCarpeta`, `paginaSiguienteDeCarpeta`), con el
// formato de Windows: viaja igual al PC y al movil.

import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

// MARK: - Lo puro

enum CarpetasChat {
    /// Los textos de Windows (`carpeta-*`).
    static let nombreDeSerie = "Cuaderno"
    static let pagina = "Página"

    /// La palabra de `miniapp` de una carpeta (`"carpeta"`, la del nucleo).
    static let palabra = carpetaMiniapp()

    /// Si un mensaje es una carpeta (`es_carpeta`).
    static func esCarpeta(_ m: MensajeChat) -> Bool {
        m.clase == "MINIAPP" && m.miniapp == palabra
    }

    /// Sus paginas, en orden (vacio si el documento no se entiende).
    static func paginas(_ m: MensajeChat) -> [PaginaDeCarpeta] {
        guard esCarpeta(m) else { return [] }
        return carpetaPaginas(documento: m.texto) ?? []
    }

    /// «1 página», «3 páginas» o «Vacía» (`carpeta-paginas`).
    static func cuantas(_ n: Int) -> String {
        switch n {
        case 0: "Vacía"
        case 1: "1 página"
        default: "\(n) páginas"
        }
    }

    /// Si el mensaje se ofrece para «Añadir a carpeta…» (`se_ofrece`): lo
    /// que puede ser una pagina, no otra carpeta ni un mensaje de sistema.
    static func seOfrece(_ m: MensajeChat) -> Bool {
        !esCarpeta(m) && !m.id.isEmpty && !m.id.hasPrefix("auto-")
    }

    /// Lo que se ve a la vista en el menu (un lienzo, una pagina, una foto);
    /// lo demas, en «Más», como Windows.
    static func vaALaVista(_ m: MensajeChat) -> Bool {
        ["DIBUJO", "PAGINA", "IMAGEN"].contains(m.clase)
    }

    /// **El nombre con el que entra una hoja** (`nombre_de_hoja`): el suyo
    /// si se lo pusieron, sin extension; «Lienzo» o `<id>.excalidraw` no
    /// dicen nada (entonces cuenta la serie o «Página N»).
    static func nombreDeHoja(_ m: MensajeChat) -> String {
        let n = m.nombre.trimmingCharacters(in: .whitespacesAndNewlines)
        if m.clase == "NOTA" {
            let primera = m.texto.split(whereSeparator: \.isNewline).first.map(String.init) ?? ""
            let t = primera.trimmingCharacters(in: CharacterSet(charactersIn: "# ").union(.whitespaces))
            return String(t.prefix(80))
        }
        if n.isEmpty || n == "Lienzo" || n.hasSuffix(".excalidraw") { return "" }
        let sin = (n as NSString).deletingPathExtension
        return sin.isEmpty ? n : sin
    }

    /// El titulo de una pagina: el suyo en la carpeta o, sin el, el nombre
    /// del mensaje.
    static func titulo(_ p: PaginaDeCarpeta, _ m: MensajeChat?) -> String {
        let t = p.titulo.trimmingCharacters(in: .whitespaces)
        if !t.isEmpty { return t }
        guard let m else { return pagina }
        return TextosCompartir.nombreDeFila(nombre: m.nombre, resumen: m.resumen)
    }

    /// Las carpetas de lo cargado del chat, la mas nueva abajo.
    static func delChat(_ mensajes: [MensajeChat]) -> [MensajeChat] {
        mensajes.filter(esCarpeta)
    }

    /// El icono de una pagina sin miniatura (`icono_de`).
    static func icono(_ m: MensajeChat?) -> String {
        switch m?.clase ?? "" {
        case "DIBUJO": "scribble.variable"
        case "PAGINA": "book"
        case "IMAGEN": "photo"
        case "VOZ": "play.fill"
        case "NOTA", "": "square.and.pencil"
        default: "doc"
        }
    }
}

// MARK: - Lo que hace (con el nucleo)

@MainActor
enum AccionesCarpeta {
    /// **Una carpeta nueva al final del chat**, con `primera` dentro si se
    /// da. El nombre, sin repetir («Cuaderno», «Cuaderno 2»…).
    static func crear(en chat: String, con primera: MensajeChat?, almacen: Almacen) async -> MensajeChat? {
        let nucleo = almacen.nucleo
        let (id, hoja) = (primera?.id, primera.map(CarpetasChat.nombreDeHoja) ?? "")
        let r = await Task.detached(priority: .userInitiated) { () -> Result<MensajeChat, Error> in
            Result {
                try nucleo.crearCarpeta(chat: chat, nombre: nil, base: CarpetasChat.nombreDeSerie,
                                        primera: id, nombreDeLaHoja: hoja, porDefecto: CarpetasChat.pagina)
            }
        }.value
        switch r {
        case let .success(m):
            await almacen.alFinal(m, en: chat)
            return m
        case let .failure(e):
            almacen.ponerError("\(e)")
            return nil
        }
    }

    /// **Cambia una carpeta** y pone su tarjeta al dia en el chat.
    static func cambiar(_ carpeta: String, en chat: String, _ cambio: CambioCarpeta,
                        almacen: Almacen) async -> CarpetaCambiada? {
        let nucleo = almacen.nucleo
        let r = await Task.detached(priority: .userInitiated) { () -> Result<CarpetaCambiada, Error> in
            Result { try nucleo.cambiarCarpeta(chat: chat, carpeta: carpeta, cambio: cambio) }
        }.value
        guard case let .success(c) = r else { return nil }
        almacen.sustituir(c.mensaje, en: chat)
        return c
    }

    /// **«Añadir a carpeta»**: mete `m` en `carpeta` y dice como entro.
    static func anadir(_ m: MensajeChat, a carpeta: MensajeChat, en chat: String,
                       almacen: Almacen, avisar: @escaping (String) -> Void) {
        let nombre = carpeta.nombre.isEmpty ? nombreDeSerie : carpeta.nombre
        let cambio = CambioCarpeta.anadir(pagina: m.id, nombreDeLaHoja: CarpetasChat.nombreDeHoja(m),
                                          porDefecto: CarpetasChat.pagina)
        _ = Task<Void, Never> {
            guard let c = await cambiar(carpeta.id, en: chat, cambio, almacen: almacen) else {
                avisar("Esa carpeta ya no está en el chat")
                return
            }
            avisar(c.yaEstaba ? "Ya está en «\(nombre)»" : "Añadida a «\(nombre)» como «\(c.titulo)»")
        }
    }

    private static var nombreDeSerie: String { CarpetasChat.nombreDeSerie }
}

// MARK: - La tarjeta del chat

/// **La carpeta en el chat**: su nombre, cuantas paginas lleva y las
/// primeras en pequeno (`ids_de_la_tarjeta` de Windows).
struct BurbujaCarpeta: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @Environment(\.escalaChat) private var escala

    static func esCarpeta(_ m: MensajeChat) -> Bool { CarpetasChat.esCarpeta(m) }

    var body: some View {
        let paginas = CarpetasChat.paginas(mensaje)
        VStack(alignment: .leading, spacing: 8 * escala) {
            HStack(spacing: 10 * escala) {
                Image(systemName: "book.closed.fill")
                    .font(.system(size: 20 * escala))
                    .foregroundStyle(.white)
                    .frame(width: 40 * escala, height: 40 * escala)
                    .background(Color(hex: 0xB86B1E), in: RoundedRectangle(cornerRadius: 10))
                VStack(alignment: .leading, spacing: 1) {
                    Text(mensaje.nombre.isEmpty ? CarpetasChat.nombreDeSerie : mensaje.nombre)
                        .font(.system(size: 14 * escala, weight: .semibold)).lineLimit(1)
                    Text("Carpeta de páginas · \(CarpetasChat.cuantas(paginas.count))")
                        .font(.caption).opacity(0.7)
                }
            }
            if !paginas.isEmpty {
                HStack(spacing: 6 * escala) {
                    ForEach(Array(paginas.prefix(4).enumerated()), id: \.element.id) { i, p in
                        MiniaturaDePagina(pagina: mensajeDe(p.id), chat: chat, almacen: almacen,
                                          lado: 64 * escala)
                            .overlay(alignment: .bottomLeading) {
                                Text("\(i + 1)")
                                    .font(.system(size: 9 * escala, weight: .bold))
                                    .padding(.horizontal, 4).padding(.vertical, 1)
                                    .background(.black.opacity(0.55), in: Capsule())
                                    .foregroundStyle(.white)
                                    .padding(3)
                            }
                    }
                    if paginas.count > 4 {
                        Text("+\(paginas.count - 4)").font(.caption.weight(.semibold)).opacity(0.7)
                    }
                }
            }
        }
        .frame(maxWidth: 340, alignment: .leading)
    }

    private func mensajeDe(_ id: String) -> MensajeChat? {
        almacen.mensajes[chat]?.first { $0.id == id }
    }
}

/// La miniatura de una pagina: un lienzo dibujado, una foto, o su icono.
struct MiniaturaDePagina: View {
    let pagina: MensajeChat?
    let chat: String
    let almacen: Almacen
    let lado: CGFloat
    @State private var dibujo: CGImage?
    @State private var foto: URL?
    @Environment(\.displayScale) private var escalaPantalla

    var body: some View {
        ZStack {
            Color.white
            if let dibujo {
                Image(decorative: dibujo, scale: max(1, escalaPantalla)).resizable().scaledToFit()
            } else if let foto {
                MiniaturaVista(url: foto, lado: lado).scaledToFill()
            } else {
                Image(systemName: CarpetasChat.icono(pagina))
                    .font(.system(size: lado * 0.32))
                    .foregroundStyle(Color(hex: 0x8A8A8E))
            }
        }
        .frame(width: lado, height: lado * 1.3)
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .overlay { RoundedRectangle(cornerRadius: 6).strokeBorder(.separator.opacity(0.5)) }
        .task(id: "\(pagina?.id ?? "")#\(lado)") { await leer() }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinLienzoGuardado)) { aviso in
            guard let r = aviso.object as? String, r == pagina?.referencia, let m = pagina else { return }
            MiniaturasLienzo.compartidas.olvidar(chat: chat, referencia: r)
            _ = Task<Void, Never> { await leerDibujo(m) }
        }
    }

    private func leer() async {
        guard let m = pagina else { return }
        switch m.clase {
        case "DIBUJO":
            await leerDibujo(m)
        case "IMAGEN":
            foto = await resolverFichero(m, chat, almacen)
        default:
            break
        }
    }

    private func leerDibujo(_ m: MensajeChat) async {
        guard let r = m.referencia, !r.isEmpty else { return }
        let px = CGSize(width: lado * max(1, escalaPantalla), height: lado * 1.3 * max(1, escalaPantalla))
        dibujo = await MiniaturasLienzo.compartidas.imagen(chat: chat, referencia: r, pixeles: px, almacen: almacen)
    }
}

// MARK: - La ventana de la carpeta

/// Lo que se ve de una carpeta abierta, leido del disco.
@MainActor
final class ModeloCarpeta: ObservableObject {
    let chat: String
    let id: String
    let almacen: Almacen
    @Published private(set) var nombre = ""
    @Published private(set) var paginas: [PaginaDeCarpeta] = []
    /// Los mensajes del chat por su id (las paginas apuntan aqui).
    @Published private(set) var mensajes: [String: MensajeChat] = [:]
    @Published private(set) var cargada = false
    @Published private(set) var noEsta = false
    @Published var aviso: String?
    @Published var trabajando = false
    private var turnoAviso = 0

    init(chat: String, id: String, almacen: Almacen) {
        self.chat = chat
        self.id = id
        self.almacen = almacen
    }

    /// Las paginas que siguen en el chat (`vivas`): lo borrado se salta.
    var vivas: [PaginaDeCarpeta] { paginas.filter { mensajes[$0.id] != nil } }

    func recargar() async {
        let todos = await PrepararCompartir.todos(chat, almacen: almacen)
        var porId: [String: MensajeChat] = [:]
        for m in todos { porId[m.id] = m }
        mensajes = porId
        cargada = true
        guard let c = porId[id], CarpetasChat.esCarpeta(c) else {
            noEsta = true
            paginas = []
            return
        }
        noEsta = false
        nombre = c.nombre.isEmpty ? CarpetasChat.nombreDeSerie : c.nombre
        paginas = CarpetasChat.paginas(c)
    }

    func avisar(_ texto: String) {
        aviso = texto
        turnoAviso += 1
        let turno = turnoAviso
        _ = Task<Void, Never> { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 2_600_000_000)
            if self?.turnoAviso == turno { self?.aviso = nil }
        }
    }

    private func aplicar(_ cambio: CambioCarpeta) async {
        guard let c = await AccionesCarpeta.cambiar(id, en: chat, cambio, almacen: almacen) else {
            avisar("No se pudo")
            await recargar()
            return
        }
        paginas = c.paginas
        mensajes[id] = c.mensaje
    }

    func mover(_ pagina: String, a puesto: Int) async {
        await aplicar(.mover(pagina: pagina, hasta: UInt32(max(0, puesto))))
    }

    /// Un puesto antes (`paso` -1) o despues (+1), entre las vivas.
    func correr(_ pagina: String, _ paso: Int) async {
        guard let i = paginas.firstIndex(where: { $0.id == pagina }) else { return }
        let destino = i + paso
        guard destino >= 0, destino < paginas.count else { return }
        await mover(pagina, a: destino)
    }

    func quitar(_ pagina: String) async { await aplicar(.quitar(pagina: pagina)) }

    func renombrar(_ pagina: String, _ titulo: String) async {
        await aplicar(.renombrar(pagina: pagina, titulo: titulo))
    }

    /// **«Nueva página»**: el lienzo nuevo, ya en el chat y en la carpeta.
    func nuevaPagina() async -> MensajeChat? {
        trabajando = true
        defer { trabajando = false }
        let (nucleo, chat, id) = (almacen.nucleo, chat, id)
        let r = await Task.detached(priority: .userInitiated) { () -> Result<PaginaNuevaDeCarpeta, Error> in
            Result { try nucleo.paginaSiguienteDeCarpeta(chat: chat, carpeta: id, porDefecto: CarpetasChat.pagina) }
        }.value
        guard case let .success(n) = r else {
            avisar("No se pudo")
            return nil
        }
        await almacen.alFinal(n.lienzo, en: chat)
        almacen.sustituir(n.carpeta, en: chat)
        mensajes[n.lienzo.id] = n.lienzo
        mensajes[id] = n.carpeta
        paginas = CarpetasChat.paginas(n.carpeta)
        return n.lienzo
    }

    /// Las paginas, en orden, para la hoja de compartir.
    var mensajesEnOrden: [MensajeChat] { vivas.compactMap { mensajes[$0.id] } }
}

/// **La ventana de una carpeta** (`carpetas::abrir` de Windows).
struct VistaCarpeta: View {
    @StateObject private var modelo: ModeloCarpeta
    let cerrar: () -> Void
    @State private var renombrando: PaginaDeCarpeta?
    @State private var titulo = ""
    @State private var arrastrada: String?

    init(chat: String, carpeta: String, almacen: Almacen, cerrar: @escaping () -> Void) {
        _modelo = StateObject(wrappedValue: ModeloCarpeta(chat: chat, id: carpeta, almacen: almacen))
        self.cerrar = cerrar
    }

    /// **Abre la carpeta** encima de todo.
    @MainActor
    static func abrir(chat: String, carpeta: String, almacen: Almacen) {
        ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 900, height: 720)) { cerrar in
            VistaCarpeta(chat: chat, carpeta: carpeta, almacen: almacen, cerrar: cerrar)
        }
    }

    var body: some View {
        NavigationStack {
            contenido
                .navigationTitle(modelo.nombre.isEmpty ? "Carpeta de páginas" : modelo.nombre)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cerrar", action: cerrar).keyboardShortcut(.cancelAction)
                    }
                    ToolbarItemGroup(placement: .primaryAction) {
                        Button { exportar() } label: { Label("Exportar PDF", systemImage: "doc.richtext") }
                            .disabled(modelo.vivas.isEmpty)
                        Button { nuevaPagina() } label: { Label("Nueva página", systemImage: "plus") }
                            .disabled(modelo.noEsta || modelo.trabajando)
                            .keyboardShortcut("n", modifiers: .command)
                    }
                }
                .overlay(alignment: .bottom) {
                    if let aviso = modelo.aviso {
                        AvisoDelChat(texto: aviso).padding(.bottom, 24)
                    }
                }
                .animation(.easeOut(duration: 0.15), value: modelo.aviso)
        }
        .task { await modelo.recargar() }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinTrajoCambios)) { _ in
            _ = Task<Void, Never> { await modelo.recargar() }
        }
        .alert("Renombrar", isPresented: Binding(get: { renombrando != nil }, set: { if !$0 { renombrando = nil } })) {
            TextField("Título", text: $titulo)
            Button("Guardar") {
                if let p = renombrando {
                    let t = titulo
                    _ = Task<Void, Never> { await modelo.renombrar(p.id, t) }
                }
                renombrando = nil
            }
            Button("Cancelar", role: .cancel) { renombrando = nil }
        } message: {
            Text("Escribe el título de la página")
        }
    }

    @ViewBuilder private var contenido: some View {
        if !modelo.cargada {
            ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if modelo.noEsta {
            aviso("Esa carpeta ya no está en el chat")
        } else if modelo.vivas.isEmpty {
            aviso("Esta carpeta está vacía. Pulsa «Nueva página», o en el chat elige «Añadir a carpeta…» en el menú de un lienzo o de una imagen.")
        } else {
            VStack(spacing: 0) {
                HStack {
                    Text(CarpetasChat.cuantas(modelo.vivas.count)).font(.subheadline.weight(.medium))
                    Spacer()
                    Text("Mantén pulsada una página para moverla, renombrarla o quitarla (sigue en el chat)")
                        .font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                .padding(.horizontal, 20).padding(.vertical, 8)
                rejilla
            }
        }
    }

    private func aviso(_ texto: String) -> some View {
        VStack(spacing: 14) {
            Image(systemName: "book.closed").font(.system(size: 40)).foregroundStyle(.secondary)
            Text(texto).multilineTextAlignment(.center).foregroundStyle(.secondary).frame(maxWidth: 420)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(24)
    }

    private var rejilla: some View {
        let vivas = modelo.vivas
        return ScrollView {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 170, maximum: 220), spacing: 18)], spacing: 18) {
                ForEach(Array(vivas.enumerated()), id: \.element.id) { i, p in
                    celda(p, numero: i + 1, total: vivas.count)
                }
            }
            .padding(20)
        }
    }

    private func celda(_ p: PaginaDeCarpeta, numero: Int, total: Int) -> some View {
        let m = modelo.mensajes[p.id]
        return VStack(alignment: .leading, spacing: 6) {
            MiniaturaDePagina(pagina: m, chat: modelo.chat, almacen: modelo.almacen, lado: 170)
                .overlay(alignment: .topLeading) {
                    Text("\(numero)")
                        .font(.system(size: 12, weight: .bold))
                        .padding(.horizontal, 7).padding(.vertical, 2)
                        .background(Paleta.chat.enviar, in: Capsule())
                        .foregroundStyle(.white)
                        .padding(6)
                }
                .opacity(arrastrada == p.id ? 0.4 : 1)
            Text(CarpetasChat.titulo(p, m))
                .font(.system(size: 13, weight: .medium)).lineLimit(1)
        }
        .contentShape(Rectangle())
        .onTapGesture { abrir(m) }
        .contextMenu {
            Button { abrir(m) } label: { Label("Abrir", systemImage: "arrow.up.right.square") }
            Button {
                titulo = p.titulo.isEmpty ? CarpetasChat.titulo(p, m) : p.titulo
                renombrando = p
            } label: { Label("Renombrar", systemImage: "pencil") }
            if numero > 1 {
                Button { _ = Task<Void, Never> { await modelo.correr(p.id, -1) } } label: {
                    Label("Antes", systemImage: "arrow.left")
                }
            }
            if numero < total {
                Button { _ = Task<Void, Never> { await modelo.correr(p.id, 1) } } label: {
                    Label("Después", systemImage: "arrow.right")
                }
            }
            Button(role: .destructive) { _ = Task<Void, Never> { await modelo.quitar(p.id) } } label: {
                Label("Quitar de la carpeta", systemImage: "minus.circle")
            }
        }
        // Arrastrar una pagina encima de otra la lleva a su sitio.
        .onDrag {
            arrastrada = p.id
            return NSItemProvider(object: p.id as NSString)
        }
        .onDrop(of: [UTType.plainText], delegate: SoltarPagina(destino: p.id, modelo: modelo,
                                                                arrastrada: $arrastrada))
    }

    private func abrir(_ m: MensajeChat?) {
        guard let m else { return }
        let (chat, almacen) = (modelo.chat, modelo.almacen)
        let nav = Navegacion.compartida
        switch m.clase {
        case "DIBUJO" where m.referencia != nil:
            cerrar()
            nav.abrir(.lienzo(chat: chat, mensaje: m, foto: nil))
        case "IMAGEN":
            _ = Task<Void, Never> {
                guard let url = await resolverFichero(m, chat, almacen) else { return }
                cerrar()
                nav.abrir(.lienzo(chat: chat, mensaje: m, foto: url))
            }
        default:
            // Lo demas, en el chat, en su sitio (como Windows).
            cerrar()
            nav.irAlChat(chat, mensaje: m.id)
        }
    }

    private func nuevaPagina() {
        _ = Task<Void, Never> {
            guard let lienzo = await modelo.nuevaPagina() else { return }
            cerrar()
            Navegacion.compartida.abrir(.lienzo(chat: modelo.chat, mensaje: lienzo, foto: nil))
        }
    }

    /// **«Exportar PDF»**: la hoja de compartir con las paginas en su orden
    /// (PDF, imprimir, guardar en Archivos), con el nombre de la carpeta.
    private func exportar() {
        let paginas = modelo.mensajesEnOrden
        guard !paginas.isEmpty else {
            modelo.avisar("No se pudo exportar: no hay ninguna página con algo dentro")
            return
        }
        HojaCompartir.abrir(.mensajes(chat: modelo.chat, titulo: modelo.nombre, paginas), almacen: modelo.almacen)
    }
}

/// Soltar una pagina arrastrada encima de otra: la pone en su puesto.
private struct SoltarPagina: DropDelegate {
    let destino: String
    let modelo: ModeloCarpeta
    @Binding var arrastrada: String?

    func performDrop(info: DropInfo) -> Bool {
        let movida = arrastrada
        arrastrada = nil
        guard let movida, movida != destino else { return false }
        let modelo = self.modelo
        let destino = self.destino
        _ = Task<Void, Never> { @MainActor in
            guard let j = modelo.paginas.firstIndex(where: { $0.id == destino }) else { return }
            await modelo.mover(movida, a: j)
        }
        return true
    }

    func dropUpdated(info: DropInfo) -> DropProposal? { DropProposal(operation: .move) }
}
