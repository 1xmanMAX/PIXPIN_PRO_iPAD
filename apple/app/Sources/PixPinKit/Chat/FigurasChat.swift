// **La ventana de las figuras** (Windows `ventana_chat/figuras.rs`, 24 y
// 25-oct-2026: «que sea como una galeria donde yo pueda ver las figuras asi
// realizadas agrupadas … intuitivo y muy rapido de ver», «y un
// previsualizador de las figuras y de todo lo que tiene»).
//
// - **A la izquierda, los grupos** con su cuenta: «Todas», «Mis figuras» y
//   las categorias (la categoria va en el nombre, «Diario · Gratitud»). En
//   un iPad estrecho, una fila de pastillas arriba.
// - **En el centro, cada grupo con su titulo** y sus figuras en rejilla,
//   dibujadas de verdad, con su nombre. Arriba, el buscador (sin
//   mayusculas ni tildes, en el nombre, el grupo y los textos).
// - **A la derecha, el previsualizador** de la elegida: en grande, con su
//   grupo, lo que mide, de que piezas esta hecha y los textos que lleva;
//   «Insertar» (un lienzo nuevo en el chat con la figura dentro) y, en las
//   del usuario, «Borrar la figura» (dos toques).
//
// Las figuras son las de la biblioteca de siempre (`figuras.json`, que
// viaja con la sincronizacion): las de serie de Windows llegan por ahi.
// Tambien aqui: «Guardar en mis figuras» de un lienzo del chat.

import PixPinCore
import SwiftUI
import UIKit

// MARK: - Lo puro

/// Un grupo de la galeria: su categoria (vacia, «Mis figuras») y sus
/// figuras en el orden de la biblioteca.
struct GrupoDeFiguras: Identifiable {
    let grupo: String
    var figuras: [FichaDeFigura]
    var id: String { grupo }
}

enum FigurasChat {
    /// El grupo de lo que no lleva categoria: lo del usuario.
    static let misFiguras = ""
    /// El grupo de «Todas» en la lista de la izquierda.
    static let todas = "\u{1}todas"

    /// Sin mayusculas ni tildes (`plegar` de Windows).
    static func plegar(_ s: String) -> String {
        s.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: Locale(identifier: "es"))
    }

    /// Si la figura casa con lo buscado: en su nombre, su grupo o sus textos.
    static func casa(_ f: FichaDeFigura, _ consulta: String) -> Bool {
        let q = plegar(consulta.trimmingCharacters(in: .whitespaces))
        guard !q.isEmpty else { return true }
        return plegar(f.nombre).contains(q) || plegar(f.grupo).contains(q)
            || f.letras.contains { plegar($0).contains(q) }
    }

    /// El orden de los grupos: lo del usuario, las categorias de serie en su
    /// orden y las inventadas detras.
    static func orden(_ g: String, categorias: [String]) -> Int {
        if g == misFiguras { return 0 }
        if let i = categorias.firstIndex(of: g) { return i + 1 }
        return categorias.count + 1
    }

    /// Las figuras por grupos, en su orden, conservando el de la biblioteca
    /// dentro de cada uno.
    static func porGrupos(_ figuras: [FichaDeFigura], categorias: [String]) -> [GrupoDeFiguras] {
        var v: [GrupoDeFiguras] = []
        for f in figuras {
            if let i = v.firstIndex(where: { $0.grupo == f.grupo }) {
                v[i].figuras.append(f)
            } else {
                v.append(GrupoDeFiguras(grupo: f.grupo, figuras: [f]))
            }
        }
        return v.enumerated()
            .sorted { a, b in
                let (oa, ob) = (orden(a.element.grupo, categorias: categorias), orden(b.element.grupo, categorias: categorias))
                return oa != ob ? oa < ob : a.offset < b.offset
            }
            .map(\.element)
    }

    static func rotulo(_ g: String) -> String { g == misFiguras ? "Mis figuras" : g }

    /// «3 líneas · 2 formas · 1 texto» (`resumen_de_piezas`).
    static func resumen(_ p: PiezasDeFigura) -> String {
        var v: [String] = []
        if p.lineas > 0 { v.append("\(p.lineas) líneas") }
        if p.formas > 0 { v.append("\(p.formas) formas") }
        if p.textos > 0 { v.append("\(p.textos) textos") }
        if p.trazos > 0 { v.append("\(p.trazos) trazos a mano") }
        if p.otras > 0 { v.append("\(p.otras) otras") }
        return v.isEmpty ? "\(p.total) piezas" : v.joined(separator: " · ")
    }

    /// El nombre con que se guarda un lienzo del chat: el suyo o, si la
    /// fila se llama como su fichero, «Figura de <chat>».
    static func nombreParaGuardar(_ m: MensajeChat, chat: String) -> String {
        let n = m.nombre.trimmingCharacters(in: .whitespacesAndNewlines)
        if n.isEmpty || n.hasSuffix(".excalidraw") { return "Figura de \(chat)" }
        return n
    }

    /// **«Guardar en mis figuras»** de un lienzo del chat.
    @MainActor
    static func guardar(lienzo m: MensajeChat, c: ChatAbierto) {
        guard let referencia = m.referencia else { return }
        let proyecto = c.almacen.chats.first { $0.id == c.chat }?.nombre ?? "PixPin"
        let nombre = nombreParaGuardar(m, chat: proyecto)
        let (nucleo, chat) = (c.almacen.nucleo, c.chat)
        _ = Task<Void, Never> {
            let r = await Task.detached(priority: .userInitiated) { () -> Bool? in
                try? nucleo.guardarLienzoComoFigura(chat: chat, dibujo: referencia, nombre: nombre)
            }.value
            switch r {
            case true?: c.avisar("Guardada en tus figuras como «\(nombre)»")
            case false?: c.avisar("Ese lienzo está vacío: no hay nada que guardar")
            case nil: c.avisar("No se pudo")
            }
        }
    }
}

// MARK: - Los dibujos

/// Las figuras pintadas, por id y tamano (una figura no cambia: se pinta
/// una vez).
@MainActor
final class DibujosDeFiguras {
    static let compartidos = DibujosDeFiguras()
    private let cache = NSCache<NSString, Caja>()
    private final class Caja {
        let imagen: CGImage
        init(_ imagen: CGImage) { self.imagen = imagen }
    }

    init() { cache.totalCostLimit = 48 << 20 }

    func olvidar() { cache.removeAllObjects() }

    func imagen(_ id: String, pixeles: CGSize, nucleo: Nucleo) async -> CGImage? {
        let w = max(1, Int(pixeles.width.rounded())), h = max(1, Int(pixeles.height.rounded()))
        let k = "\(id)|\(w)x\(h)" as NSString
        if let c = cache.object(forKey: k) { return c.imagen }
        let hecha = await Task.detached(priority: .utility) { () -> CGImage? in
            guard let lienzo = (try? nucleo.lienzoDeFigura(id: id)) ?? nil,
                  let leido = try? leerLienzo(lienzo.plano()) else { return nil }
            return MapaLienzo.imagen(leido, ancho: w, alto: h, margen: CGFloat(min(w, h)) * 0.06) { _ in nil }
        }.value
        if let hecha { cache.setObject(Caja(hecha), forKey: k, cost: hecha.bytesPerRow * hecha.height) }
        return hecha
    }
}

/// Una figura dibujada en su hueco.
struct DibujoDeFigura: View {
    let id: String
    let nucleo: Nucleo
    let tamano: CGSize
    @State private var imagen: CGImage?
    @Environment(\.displayScale) private var escalaPantalla

    var body: some View {
        ZStack {
            Color.white
            if let imagen {
                Image(decorative: imagen, scale: max(1, escalaPantalla)).resizable().scaledToFit()
            } else {
                ProgressView()
            }
        }
        .frame(width: tamano.width, height: tamano.height)
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay { RoundedRectangle(cornerRadius: 8).strokeBorder(.separator.opacity(0.5)) }
        .task(id: "\(id)#\(tamano.width)") {
            let px = CGSize(width: tamano.width * max(1, escalaPantalla), height: tamano.height * max(1, escalaPantalla))
            imagen = await DibujosDeFiguras.compartidos.imagen(id, pixeles: px, nucleo: nucleo)
        }
    }
}

// MARK: - La ventana

@MainActor
final class ModeloGaleriaFiguras: ObservableObject {
    let almacen: Almacen
    let chat: String?
    @Published private(set) var figuras: [FichaDeFigura] = []
    @Published private(set) var cargada = false
    @Published var grupo = FigurasChat.todas
    @Published var buscado = ""
    @Published var elegida: String?
    /// El primer toque de «Borrar la figura» (el segundo borra).
    @Published var borrando: String?
    @Published var aviso: String?
    let categorias = figurasCategorias()

    init(almacen: Almacen, chat: String?) {
        self.almacen = almacen
        self.chat = chat
    }

    func leer() async {
        let nucleo = almacen.nucleo
        figuras = await Task.detached(priority: .userInitiated) {
            (try? nucleo.galeriaDeFiguras()) ?? []
        }.value
        cargada = true
        if let e = elegida, !figuras.contains(where: { $0.id == e }) { elegida = nil }
    }

    /// Los grupos con su cuenta (para la columna de la izquierda).
    var grupos: [GrupoDeFiguras] {
        FigurasChat.porGrupos(figuras, categorias: categorias)
    }

    /// Lo que se ve: el grupo elegido (o todos) con lo buscado.
    var secciones: [GrupoDeFiguras] {
        FigurasChat.porGrupos(figuras.filter { FigurasChat.casa($0, buscado) }, categorias: categorias)
            .filter { grupo == FigurasChat.todas || $0.grupo == grupo }
    }

    var ficha: FichaDeFigura? { figuras.first { $0.id == elegida } }

    func borrar(_ f: FichaDeFigura) {
        guard !f.deSerie else { return }
        guard borrando == f.id else {
            borrando = f.id
            return
        }
        borrando = nil
        let nucleo = almacen.nucleo
        let id = f.id
        _ = Task<Void, Never> {
            _ = await Task.detached(priority: .userInitiated) { try? nucleo.quitarFigura(id: id) }.value
            await leer()
        }
    }

    /// **«Insertar»**: un lienzo nuevo al final del chat con la figura.
    func insertar(_ f: FichaDeFigura) async -> MensajeChat? {
        guard let chat else { return nil }
        let nucleo = almacen.nucleo
        let id = f.id
        let r = await Task.detached(priority: .userInitiated) { () -> Result<MensajeChat, Error> in
            Result { try nucleo.adjuntarFigura(chat: chat, id: id) }
        }.value
        guard case let .success(m) = r else {
            aviso = "No se pudo"
            return nil
        }
        await almacen.alFinal(m, en: chat)
        return m
    }
}

/// **La galeria de las figuras** (`figuras::abrir` de Windows).
struct VistaFiguras: View {
    @StateObject private var modelo: ModeloGaleriaFiguras
    let cerrar: () -> Void
    @Environment(\.horizontalSizeClass) private var clase

    init(almacen: Almacen, chat: String?, cerrar: @escaping () -> Void) {
        _modelo = StateObject(wrappedValue: ModeloGaleriaFiguras(almacen: almacen, chat: chat))
        self.cerrar = cerrar
    }

    /// **Abre la galeria** encima de todo. Con `chat`, «Insertar» lo pone
    /// en ese chat.
    @MainActor
    static func abrir(almacen: Almacen, chat: String?) {
        ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 1080, height: 760)) { cerrar in
            VistaFiguras(almacen: almacen, chat: chat, cerrar: cerrar)
        }
    }

    private var ancho: Bool { clase != .compact }

    var body: some View {
        NavigationStack {
            Group {
                if !modelo.cargada {
                    ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if modelo.figuras.isEmpty {
                    vacio("Todavía no hay figuras. En un lienzo, elige algo y «Guardar como figura».")
                } else {
                    HStack(spacing: 0) {
                        if ancho {
                            columnaDeGrupos.frame(width: 210)
                            Divider()
                        }
                        VStack(spacing: 0) {
                            if !ancho { pastillasDeGrupos }
                            rejilla
                        }
                        if ancho {
                            Divider()
                            previa.frame(width: 340)
                        }
                    }
                }
            }
            .navigationTitle("Figuras")
            .navigationBarTitleDisplayMode(.inline)
            .searchable(text: $modelo.buscado, placement: .toolbar, prompt: "Buscar figuras")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cerrar", action: cerrar).keyboardShortcut(.cancelAction)
                }
            }
            .sheet(isPresented: Binding(get: { !ancho && modelo.elegida != nil },
                                        set: { if !$0 { modelo.elegida = nil } })) {
                NavigationStack {
                    previa
                        .toolbar {
                            ToolbarItem(placement: .cancellationAction) {
                                Button("Cerrar") { modelo.elegida = nil }
                            }
                        }
                }
                .presentationDetents([.medium, .large])
            }
            .overlay(alignment: .bottom) {
                if let aviso = modelo.aviso { AvisoDelChat(texto: aviso).padding(.bottom, 24) }
            }
        }
        .task { await modelo.leer() }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinTrajoCambios)) { _ in
            DibujosDeFiguras.compartidos.olvidar()
            _ = Task<Void, Never> { await modelo.leer() }
        }
    }

    private func vacio(_ texto: String) -> some View {
        VStack(spacing: 14) {
            Image(systemName: "square.on.square").font(.system(size: 40)).foregroundStyle(.secondary)
            Text(texto).multilineTextAlignment(.center).foregroundStyle(.secondary).frame(maxWidth: 420)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(24)
    }

    // MARK: Los grupos

    private var columnaDeGrupos: some View {
        List {
            filaDeGrupo(FigurasChat.todas, "Todas", modelo.figuras.count)
            ForEach(modelo.grupos) { g in
                filaDeGrupo(g.grupo, FigurasChat.rotulo(g.grupo), g.figuras.count)
            }
        }
        .listStyle(.sidebar)
    }

    private func filaDeGrupo(_ g: String, _ rotulo: String, _ n: Int) -> some View {
        Button { modelo.grupo = g } label: {
            HStack {
                Text(rotulo).lineLimit(1)
                Spacer()
                Text("\(n)").foregroundStyle(.secondary).monospacedDigit()
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .listRowBackground(modelo.grupo == g ? Paleta.chat.enviar.opacity(0.18) : Color.clear)
    }

    private var pastillasDeGrupos: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                pastilla(FigurasChat.todas, "Todas")
                ForEach(modelo.grupos) { g in pastilla(g.grupo, FigurasChat.rotulo(g.grupo)) }
            }
            .padding(.horizontal, 12).padding(.vertical, 8)
        }
    }

    private func pastilla(_ g: String, _ rotulo: String) -> some View {
        Button { modelo.grupo = g } label: {
            Text(rotulo).font(.subheadline)
                .padding(.horizontal, 12).padding(.vertical, 6)
                .background(modelo.grupo == g ? Paleta.chat.enviar : Color.secondary.opacity(0.15), in: Capsule())
                .foregroundStyle(modelo.grupo == g ? Color.white : Color.primary)
        }
        .buttonStyle(.plain)
    }

    // MARK: La rejilla

    private var rejilla: some View {
        let secciones = modelo.secciones
        return ScrollView {
            if secciones.isEmpty {
                Text("No hay ninguna figura que lo tenga").foregroundStyle(.secondary).padding(40)
            }
            LazyVStack(alignment: .leading, spacing: 18, pinnedViews: [.sectionHeaders]) {
                ForEach(secciones) { s in
                    Section {
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 140, maximum: 170), spacing: 12)], spacing: 12) {
                            ForEach(s.figuras, id: \.id) { f in celda(f) }
                        }
                    } header: {
                        HStack {
                            Text(FigurasChat.rotulo(s.grupo)).font(.headline)
                            Text("\(s.figuras.count)").foregroundStyle(.secondary)
                            Spacer()
                        }
                        .padding(.vertical, 6)
                        .background(.background)
                    }
                }
            }
            .padding(16)
        }
    }

    private func celda(_ f: FichaDeFigura) -> some View {
        let elegida = modelo.elegida == f.id
        return VStack(spacing: 6) {
            DibujoDeFigura(id: f.id, nucleo: modelo.almacen.nucleo, tamano: CGSize(width: 140, height: 104))
                .overlay {
                    if elegida {
                        RoundedRectangle(cornerRadius: 8).strokeBorder(Paleta.chat.enviar, lineWidth: 3)
                    }
                }
            Text(f.nombre).font(.caption).lineLimit(2).multilineTextAlignment(.center)
                .frame(width: 140)
        }
        .contentShape(Rectangle())
        .onTapGesture { modelo.elegida = f.id }
        .contextMenu {
            if modelo.chat != nil {
                Button { insertar(f) } label: { Label("Insertar", systemImage: "plus.square.on.square") }
            }
            if !f.deSerie {
                Button(role: .destructive) {
                    modelo.borrando = f.id
                    modelo.borrar(f)
                } label: { Label("Borrar la figura", systemImage: "trash") }
            }
        }
    }

    // MARK: El previsualizador

    @ViewBuilder private var previa: some View {
        if let f = modelo.ficha {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    DibujoDeFigura(id: f.id, nucleo: modelo.almacen.nucleo, tamano: CGSize(width: 300, height: 260))
                    Text(f.nombre).font(.title3.weight(.semibold))
                    Text("\(FigurasChat.rotulo(f.grupo)) · \(f.deSerie ? "de serie" : "tuya")")
                        .font(.subheadline).foregroundStyle(.secondary)
                    LabeledContent("Mide", value: "\(Int(f.ancho.rounded())) × \(Int(f.alto.rounded()))")
                    LabeledContent("Piezas", value: FigurasChat.resumen(f.piezas))
                    if !f.letras.isEmpty {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Textos que lleva").font(.subheadline.weight(.medium))
                            ForEach(Array(f.letras.prefix(30).enumerated()), id: \.offset) { _, l in
                                Text("· " + l).font(.callout).foregroundStyle(.secondary).lineLimit(2)
                            }
                        }
                    }
                    HStack {
                        if modelo.chat != nil {
                            Button { insertar(f) } label: { Label("Insertar", systemImage: "plus.square.on.square") }
                                .buttonStyle(.borderedProminent)
                                .tint(Paleta.chat.enviar)
                        }
                        if !f.deSerie {
                            Button(role: .destructive) { modelo.borrar(f) } label: {
                                Label(modelo.borrando == f.id ? "Pulsa otra vez para borrar" : "Borrar la figura",
                                      systemImage: "trash")
                            }
                            .buttonStyle(.bordered)
                        }
                    }
                    .padding(.top, 6)
                }
                .padding(20)
            }
        } else {
            vacio("Elige una figura para verla en grande")
        }
    }

    private func insertar(_ f: FichaDeFigura) {
        _ = Task<Void, Never> {
            guard let m = await modelo.insertar(f), let chat = modelo.chat else { return }
            cerrar()
            Navegacion.compartida.abrir(.lienzo(chat: chat, mensaje: m, foto: nil))
        }
    }
}
