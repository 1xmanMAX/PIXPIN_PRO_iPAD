// **La ventana de las figuras** (Windows `ventana_editor/figuras/ventana.rs`
// y `ventana_chat/figuras.rs`, 22 al 25-oct-2026; del Mac,
// `Lienzo/VentanaFiguras.swift` y `Chat/FigurasDelChat.swift`): las de serie
// (plantillas de pagina y simbolos por categorias) y las propias, en una hoja
// grande.
//
// - **A la izquierda**, las categorias con cuantas tiene cada una («Todas»
//   arriba, «Mis figuras» al final). En un iPad estrecho, una fila de
//   pastillas arriba.
// - **En medio**, las miniaturas por categorias, y el buscador arriba (sin
//   mayusculas ni tildes).
// - **A la derecha**, la elegida en grande: si es una plantilla con numeros,
//   con su intervalo («del 1 al 40, de 10 en 10») para insertarla ya
//   alargada; las propias, con renombrar; y quitar (dos toques).
//
// Se abre de tres maneras, como en el Mac:
// - desde la cabecera del chat, **para mirar** (sin lienzo donde insertar);
// - desde el clip de un chat («Figura»), **para adjuntar**: la elegida entra
//   en el chat como un lienzo con ella dentro (en el del asistente, la caja
//   empieza con «Mejora la figura …»);
// - desde el boton Figuras del lienzo, **para insertar** en medio de lo que
//   se ve (con su intervalo, alargada).
//
// Lo que se busca, se filtra, se alarga y se guarda es de Rust
// (`galeria_figuras.rs`, `GaleriaFiguras`); las miniaturas son el bufer
// plano de siempre, pintado con `MapaLienzo`.

import PixPinCore
import SwiftUI
import UIKit

// MARK: - Lo puro

enum FigurasChat {
    /// «3 líneas · 2 formas · 4 textos» (`resumen_de_piezas` de Windows).
    static func resumen(_ d: DetalleDeFigura) -> String {
        var v: [String] = []
        if d.lineas > 0 { v.append("\(d.lineas) líneas") }
        if d.formas > 0 { v.append("\(d.formas) formas") }
        if d.textos > 0 { v.append("\(d.textos) textos") }
        if d.trazos > 0 { v.append("\(d.trazos) trazos a mano") }
        if d.otras > 0 { v.append("\(d.otras) otras") }
        return v.isEmpty ? "\(d.total)" : v.joined(separator: " · ")
    }

    /// Los campos del intervalo de dos en dos; el «de … en …», solo.
    static func filas(_ campos: [CampoDeIntervalo]) -> [[CampoDeIntervalo]] {
        var filas: [[CampoDeIntervalo]] = []
        var i = 0
        while i < campos.count {
            let par = i + 1 < campos.count && !campos[i].aLoAncho && !campos[i + 1].aLoAncho
            filas.append(par ? [campos[i], campos[i + 1]] : [campos[i]])
            i += par ? 2 : 1
        }
        return filas
    }

    /// La rejilla por categorias, en el orden en que llegan.
    static func grupos(_ lista: [FichaDeFigura]) -> [GrupoVisibleFiguras] {
        var v: [GrupoVisibleFiguras] = []
        for f in lista {
            if v.last?.nombre == f.categoria {
                v[v.count - 1].fichas.append(f)
            } else {
                v.append(GrupoVisibleFiguras(nombre: f.categoria, fichas: [f]))
            }
        }
        return v
    }

    /// **«Guardar en mis figuras»** un lienzo del chat: lo que se ve en el,
    /// con el nombre del mensaje (sin el, «Figura de <chat>»), como el Mac
    /// (`ChatAbierto.guardarEnFiguras`).
    @MainActor
    static func guardar(lienzo m: MensajeChat, c: ChatAbierto) {
        guard let dibujo = m.referencia, !dibujo.isEmpty else { return }
        let (nucleo, chat) = (c.almacen.nucleo, c.chat)
        let nombre = m.nombre
        let nombreDelChat = c.almacen.chats.first { $0.id == chat }?.nombre ?? ""
        _ = Task<Void, Never> {
            let r = await Task.detached(priority: .userInitiated) { () -> Result<String?, Error> in
                Result {
                    try nucleo.guardarLienzoEnFiguras(chat: chat, dibujo: dibujo, nombre: nombre,
                                                      nombreDelChat: nombreDelChat)
                }
            }.value
            switch r {
            case let .success(guardada?): c.avisar("Guardada en tus figuras como «\(guardada)»")
            case .success(nil): c.avisar("Ese lienzo está vacío: no hay nada que guardar")
            case .failure: c.avisar("No se pudo: mira el registro de PixPin")
            }
        }
    }

    /// **«Figura»** del clip: abre la galeria eligiendo para este chat. La
    /// elegida entra como un lienzo con ella dentro; en el chat del
    /// asistente, `empezar` pone «Mejora la figura …» en la caja.
    @MainActor
    static func elegirParaAdjuntar(chat: String, almacen: Almacen, avisar: @escaping (String) -> Void,
                                   empezar: ((String) -> Void)?) {
        let ficha = almacen.chats.first { $0.id == chat }
        let destino = ChatDeLaFigura(chat: chat, nombre: ficha?.nombre ?? "") { f in
            adjuntar(f.id, nombre: f.corto, chat: chat, almacen: almacen, avisar: avisar,
                     empezar: ficha?.esAsistente == true ? empezar : nil)
        }
        VistaFiguras.abrir(almacen: almacen, paraElChat: destino)
    }

    /// La figura `id`, a un lienzo nuevo del chat con su nombre.
    @MainActor
    static func adjuntar(_ id: String, nombre: String, chat: String, almacen: Almacen,
                         avisar: @escaping (String) -> Void, empezar: ((String) -> Void)?) {
        let nucleo = almacen.nucleo
        _ = Task<Void, Never> {
            let r = await Task.detached(priority: .userInitiated) { () -> Result<MensajeChat, Error> in
                Result { try nucleo.adjuntarFigura(chat: chat, id: id) }
            }.value
            switch r {
            case let .success(m):
                await almacen.alFinal(m, en: chat)
                // Se trae ese chat (`ir_a` de Windows) y, si mientras se
                // elegia se paso a otra seccion, se vuelve a el (Windows
                // bd8ac36, `FigurasDelChat.ventanasDelChat` del Mac): si no,
                // la figura entraria sin que se viera.
                Navegacion.compartida.saltoAMensaje = nil
                Navegacion.compartida.irAlChat(chat)
                empezar?("Mejora la figura «\(nombre)»: ")
            case .failure:
                avisar("No se pudo: mira el registro de PixPin")
            }
        }
    }
}

/// El chat al que se adjunta lo que se elija (la galeria abierta desde su
/// clip).
struct ChatDeLaFigura {
    let chat: String
    /// Su nombre, para el boton («Adjuntar a «Casa Lima»»).
    let nombre: String
    let adjuntar: @MainActor (FichaDeFigura) -> Void
}

/// Un grupo de la rejilla: el titulo de su categoria y sus figuras.
struct GrupoVisibleFiguras: Identifiable {
    let nombre: String
    var fichas: [FichaDeFigura]
    var id: String { nombre }
}

// MARK: - El modelo

@MainActor
final class ModeloFiguras: ObservableObject {
    let galeria: GaleriaFiguras
    /// El lienzo donde se inserta; sin el, se mira, se renombra y se quita.
    weak var destino: ModeloEditorLienzo?
    /// El chat al que se adjunta (abierta desde su clip).
    let paraElChat: ChatDeLaFigura?
    @Published private(set) var aviso: String?
    private var avisos = 0

    @Published var consulta = "" { didSet { if consulta != oldValue { rehacer() } } }
    /// `nil`: todas.
    @Published var categoria: String? { didSet { if categoria != oldValue { rehacer() } } }
    @Published private(set) var categorias: [CategoriaDeFiguras] = []
    @Published private(set) var lista: [FichaDeFigura] = []
    @Published private(set) var cuantas: UInt32 = 0
    @Published private(set) var elegida: FichaDeFigura?
    @Published private(set) var campos: [CampoDeIntervalo] = []
    @Published var valores: [String: String] = [:] { didSet { rehacerPrevia() } }
    @Published private(set) var previa: PreviaDeIntervalo?
    @Published private(set) var detalle: DetalleDeFigura?
    @Published var nombre = ""
    @Published var confirmarQuitar = false
    /// En un iPad estrecho, la elegida se ve en una hoja aparte.
    @Published var verPrevia = false

    init(galeria: GaleriaFiguras, destino: ModeloEditorLienzo?, paraElChat: ChatDeLaFigura?) {
        self.galeria = galeria
        self.destino = destino
        self.paraElChat = paraElChat
        rehacer()
    }

    var grupos: [GrupoVisibleFiguras] { FigurasChat.grupos(lista) }

    /// Con titulos de categoria: sin filtrar ni buscar.
    var conTitulos: Bool {
        categoria == nil && consulta.trimmingCharacters(in: .whitespaces).isEmpty
    }

    var puedeInsertar: Bool { destino?.vista != nil }

    func decir(_ texto: String) {
        aviso = texto
        avisos += 1
        let mio = avisos
        _ = Task<Void, Never> { [weak self] in
            try? await Task.sleep(nanoseconds: 1_800_000_000)
            guard let self, self.avisos == mio else { return }
            self.aviso = nil
        }
    }

    /// **Adjunta la elegida** al chat, si se abrio para eso, y lo dice un
    /// momento (la hoja se queda abierta por si se quiere otra).
    func adjuntar() {
        guard let d = paraElChat, let e = elegida else { return }
        d.adjuntar(e)
        decir("Adjuntada a «\(d.nombre)» ✓")
    }

    /// Vuelve a leer la biblioteca (llego de otro aparato, se guardo una).
    func releer() {
        try? galeria.releer()
        DibujosDeFiguras.compartidos.olvidar()
        rehacer()
    }

    func rehacer() {
        categorias = galeria.categorias()
        if let c = categoria, !categorias.contains(where: { $0.nombre == c }) { categoria = nil }
        cuantas = galeria.cuantas()
        lista = galeria.visibles(categoria: categoria, consulta: consulta)
        if let e = elegida, let ahora = lista.first(where: { $0.id == e.id }) {
            elegida = ahora
        } else {
            elegir(lista.first)
        }
    }

    func elegir(_ f: FichaDeFigura?) {
        guard f?.id != elegida?.id else { return }
        elegida = f
        confirmarQuitar = false
        campos = f.map { galeria.campos(id: $0.id) } ?? []
        detalle = f.flatMap { galeria.detalle(id: $0.id) }
        nombre = f?.nombre ?? ""
        valores = [:]
        previa = nil
    }

    private var valoresPedidos: [ValorDeCampo] {
        valores.map { ValorDeCampo(clave: $0.key, texto: $0.value) }
    }

    private func rehacerPrevia() {
        guard let e = elegida, e.alargable else { previa = nil; return }
        previa = galeria.previa(id: e.id, valores: valoresPedidos)
    }

    /// **Insertar** la elegida en medio de lo que se ve del lienzo (con su
    /// intervalo, alargada). `true` si entro (la hoja se cierra).
    func insertar() -> Bool {
        guard let e = elegida, let destino, let vista = destino.vista else { return false }
        let c = vista.centroMundo
        do {
            if try galeria.insertar(id: e.id, valores: valoresPedidos, editor: destino.editor,
                                    centroX: Float(c.x), centroY: Float(c.y)) {
                vista.trasCambio()
                destino.refrescarBarra()
                return true
            }
        } catch {
            previa = galeria.previa(id: e.id, valores: valoresPedidos)
                ?? PreviaDeIntervalo(texto: "\(error)", error: true)
        }
        return false
    }

    func renombrar() {
        guard let e = elegida else { return }
        if (try? galeria.renombrar(id: e.id, nombre: nombre)) == true { rehacer() }
    }

    /// Quitar: el primer toque pide confirmarlo, el segundo la quita.
    func quitar() {
        guard let e = elegida else { return }
        guard confirmarQuitar else { confirmarQuitar = true; return }
        confirmarQuitar = false
        if (try? galeria.quitar(id: e.id)) == true {
            elegida = nil
            rehacer()
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

    func imagen(_ id: String, pixeles: CGSize, galeria: GaleriaFiguras) async -> CGImage? {
        let w = max(1, Int(pixeles.width.rounded())), h = max(1, Int(pixeles.height.rounded()))
        let k = "\(id)|\(w)x\(h)" as NSString
        if let c = cache.object(forKey: k) { return c.imagen }
        let hecha = await Task.detached(priority: .utility) { () -> CGImage? in
            guard let plano = galeria.plano(id: id), let leido = try? leerLienzo(plano) else { return nil }
            return MapaLienzo.imagen(leido, ancho: w, alto: h, margen: CGFloat(min(w, h)) * 0.06) { _ in nil }
        }.value
        if let hecha { cache.setObject(Caja(hecha), forKey: k, cost: hecha.bytesPerRow * hecha.height) }
        return hecha
    }
}

/// Una figura dibujada en su hueco, sobre el papel claro de las plantillas.
struct DibujoDeFigura: View {
    let id: String
    let galeria: GaleriaFiguras
    let tamano: CGSize
    @State private var imagen: CGImage?
    @Environment(\.displayScale) private var escalaPantalla

    var body: some View {
        ZStack {
            Color(hex: 0xF2F2F5)
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
            imagen = await DibujosDeFiguras.compartidos.imagen(id, pixeles: px, galeria: galeria)
        }
    }
}

// MARK: - La hoja

/// **La galeria de las figuras** (`figuras::abrir` de Windows).
struct VistaFiguras: View {
    @StateObject private var modelo: ModeloFiguras
    let cerrar: () -> Void
    @Environment(\.horizontalSizeClass) private var clase

    init(modelo: ModeloFiguras, cerrar: @escaping () -> Void) {
        _modelo = StateObject(wrappedValue: modelo)
        self.cerrar = cerrar
    }

    /// **Abre la galeria para mirar** (la cabecera del chat).
    @MainActor
    static func abrir(almacen: Almacen, chat: String?) {
        abrir(nucleo: almacen.nucleo, destino: nil, paraElChat: nil)
    }

    /// **Abre la galeria para adjuntar al chat** (el clip → «Figura»).
    @MainActor
    static func abrir(almacen: Almacen, paraElChat: ChatDeLaFigura) {
        abrir(nucleo: almacen.nucleo, destino: nil, paraElChat: paraElChat)
    }

    /// **Abre la galeria para insertar en el lienzo** (su boton Figuras).
    @MainActor
    static func abrir(nucleo: Nucleo, destino: ModeloEditorLienzo?, paraElChat: ChatDeLaFigura?) {
        guard let galeria = try? nucleo.galeriaDeFiguras() else { return }
        let modelo = ModeloFiguras(galeria: galeria, destino: destino, paraElChat: paraElChat)
        ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 1100, height: 780)) { cerrar in
            VistaFiguras(modelo: modelo, cerrar: cerrar)
        }
    }

    private var ancho: Bool { clase != .compact }

    private var subtitulo: String {
        modelo.paraElChat.map { "Elige una figura para «\($0.nombre)»" } ?? "\(modelo.cuantas) figuras y plantillas"
    }

    var body: some View {
        NavigationStack {
            Group {
                if modelo.cuantas == 0 {
                    vacio("Todavía no hay figuras. En un lienzo, elige algo y «Guardar en figuras».")
                } else {
                    HStack(spacing: 0) {
                        if ancho {
                            lateral.frame(width: 236)
                            Divider()
                        }
                        VStack(spacing: 0) {
                            if !ancho { pastillas }
                            rejilla
                        }
                        if ancho {
                            Divider()
                            ScrollView { panel.padding(20) }.frame(width: 340)
                        }
                    }
                }
            }
            .navigationTitle("Figuras")
            .navigationBarTitleDisplayMode(.inline)
            .searchable(text: $modelo.consulta, placement: .toolbar, prompt: "Buscar figuras")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cerrar", action: cerrar).keyboardShortcut(.cancelAction)
                }
                ToolbarItem(placement: .principal) {
                    VStack(spacing: 0) {
                        Text("Figuras").font(.headline)
                        Text(subtitulo).font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
            .sheet(isPresented: Binding(get: { !ancho && modelo.verPrevia && modelo.elegida != nil },
                                        set: { if !$0 { modelo.verPrevia = false } })) {
                NavigationStack {
                    ScrollView { panel.padding(20) }
                        .toolbar {
                            ToolbarItem(placement: .cancellationAction) {
                                Button("Cerrar") { modelo.verPrevia = false }
                            }
                        }
                }
                .presentationDetents([.medium, .large])
            }
            .overlay(alignment: .bottom) {
                if let aviso = modelo.aviso { AvisoDelChat(texto: aviso).padding(.bottom, 24) }
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinTrajoCambios)) { _ in
            modelo.releer()
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

    // MARK: Las categorias

    private var lateral: some View {
        List {
            fila(nil, "Todas", modelo.cuantas)
            ForEach(modelo.categorias, id: \.nombre) { c in
                fila(c.nombre, c.nombre, c.cuenta)
            }
            // Lo demas que traia el menu del lienzo, cuando se inserta en uno.
            if let destino = modelo.destino {
                Section("Insertar") {
                    Button("Gráfica de una función…") { herramienta(destino) { $0.pedirGrafica() } }
                    Button("Tabla en blanco") { herramienta(destino) { $0.pedirTablaEnBlanco() } }
                    Button("Pegar tabla (de Excel)") { herramienta(destino) { $0.pegarTabla() } }
                    Button("Plano de funciones") { herramienta(destino) { $0.estamparInstrumento(2) } }
                    Button("Recta de funciones") { herramienta(destino) { $0.estamparInstrumento(1) } }
                    Button("Espacio xyz") { herramienta(destino) { $0.estamparInstrumento(3) } }
                }
            }
        }
        .listStyle(.sidebar)
    }

    /// Lo demas del menu: se cierra la hoja y lo hace el lienzo.
    private func herramienta(_ destino: ModeloEditorLienzo, _ hacer: @escaping (ModeloEditorLienzo) -> Void) {
        cerrar()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { hacer(destino) }
    }

    private func fila(_ c: String?, _ rotulo: String, _ n: UInt32) -> some View {
        Button { modelo.categoria = c } label: {
            HStack {
                Text(rotulo).lineLimit(1)
                Spacer()
                Text("\(n)").foregroundStyle(.secondary).monospacedDigit()
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .listRowBackground(modelo.categoria == c ? Paleta.chat.enviar.opacity(0.18) : Color.clear)
    }

    private var pastillas: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                pastilla(nil, "Todas")
                ForEach(modelo.categorias, id: \.nombre) { c in pastilla(c.nombre, c.nombre) }
            }
            .padding(.horizontal, 12).padding(.vertical, 8)
        }
    }

    private func pastilla(_ c: String?, _ rotulo: String) -> some View {
        Button { modelo.categoria = c } label: {
            Text(rotulo).font(.subheadline)
                .padding(.horizontal, 12).padding(.vertical, 6)
                .background(modelo.categoria == c ? Paleta.chat.enviar : Color.secondary.opacity(0.15), in: Capsule())
                .foregroundStyle(modelo.categoria == c ? Color.white : Color.primary)
        }
        .buttonStyle(.plain)
    }

    // MARK: La rejilla

    private var rejilla: some View {
        let grupos = modelo.grupos
        return ScrollView {
            if grupos.isEmpty {
                Text("No hay ninguna figura que lo tenga").foregroundStyle(.secondary).padding(40)
            }
            LazyVStack(alignment: .leading, spacing: 18, pinnedViews: [.sectionHeaders]) {
                ForEach(grupos) { g in
                    Section {
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 160, maximum: 190), spacing: 12)], spacing: 12) {
                            ForEach(g.fichas, id: \.id) { f in celda(f) }
                        }
                    } header: {
                        if modelo.conTitulos {
                            HStack {
                                Text(g.nombre).font(.headline)
                                Text("\(g.fichas.count)").foregroundStyle(.secondary)
                                Spacer()
                            }
                            .padding(.vertical, 6)
                            .background(.background)
                        }
                    }
                }
            }
            .padding(16)
        }
    }

    private func celda(_ f: FichaDeFigura) -> some View {
        let elegida = modelo.elegida?.id == f.id
        return VStack(spacing: 6) {
            DibujoDeFigura(id: f.id, galeria: modelo.galeria, tamano: CGSize(width: 160, height: 120))
                .overlay {
                    if elegida {
                        RoundedRectangle(cornerRadius: 8).strokeBorder(Paleta.chat.enviar, lineWidth: 3)
                    }
                }
            Text(f.corto).font(.caption).lineLimit(2).multilineTextAlignment(.center)
                .frame(width: 160)
        }
        .contentShape(Rectangle())
        .onTapGesture(count: 2) {
            modelo.elegir(f)
            accionPrincipal()
        }
        .onTapGesture {
            modelo.elegir(f)
            modelo.verPrevia = true
        }
    }

    /// El doble toque: insertar en el lienzo o adjuntar al chat.
    private func accionPrincipal() {
        if modelo.destino != nil {
            if modelo.insertar() { cerrar() }
        } else if modelo.paraElChat != nil {
            modelo.adjuntar()
        }
    }

    // MARK: La elegida

    @ViewBuilder private var panel: some View {
        if let f = modelo.elegida {
            VStack(alignment: .leading, spacing: 0) {
                DibujoDeFigura(id: f.id, galeria: modelo.galeria, tamano: CGSize(width: 300, height: 240))
                    .padding(.bottom, 12)
                if let d = modelo.paraElChat {
                    Button { modelo.adjuntar() } label: {
                        Label("Adjuntar a «\(d.nombre)»", systemImage: "paperclip")
                            .frame(maxWidth: .infinity, minHeight: 40)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(Paleta.chat.enviar)
                    .padding(.bottom, 4)
                    Text("O doble toque en la figura.").font(.caption).foregroundStyle(.secondary)
                        .padding(.bottom, 12)
                }
                Text(f.corto).font(.title3.weight(.semibold)).lineLimit(2)
                Text("\(f.categoria)  ·  \(f.piezas) piezas")
                    .font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                    .padding(.top, 4).padding(.bottom, 18)
                if !modelo.campos.isEmpty {
                    rotulo("Intervalo")
                    ForEach(Array(FigurasChat.filas(modelo.campos).enumerated()), id: \.offset) { _, fila in
                        HStack(spacing: 8) {
                            ForEach(fila, id: \.clave) { c in campo(c) }
                        }
                        .padding(.bottom, 8)
                    }
                    if let p = modelo.previa {
                        Text(p.error ? p.texto : "→ \(p.texto)")
                            .font(.callout)
                            .foregroundStyle(p.error ? Color.red : Paleta.chat.enviar)
                            .fixedSize(horizontal: false, vertical: true)
                            .padding(.bottom, 12)
                    }
                }
                if !f.deSerie {
                    rotulo("Nombre")
                    HStack(spacing: 8) {
                        TextField("", text: $modelo.nombre)
                            .textFieldStyle(.roundedBorder)
                            .onSubmit { modelo.renombrar() }
                        Button("Renombrar") { modelo.renombrar() }
                            .disabled(modelo.nombre.trimmingCharacters(in: .whitespaces).isEmpty || modelo.nombre == f.nombre)
                    }
                    .padding(.bottom, 16)
                }
                if modelo.destino == nil, let d = modelo.detalle {
                    rotulo("Mide")
                    Text("\(Int(f.ancho.rounded())) × \(Int(f.alto.rounded()))").font(.callout).padding(.bottom, 12)
                    rotulo("Piezas")
                    Text(FigurasChat.resumen(d)).font(.callout).padding(.bottom, 12)
                    if !d.letras.isEmpty {
                        rotulo("Textos que lleva")
                        Text(d.letras.prefix(12).joined(separator: " · "))
                            .font(.callout)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                VStack(spacing: 8) {
                    if modelo.destino != nil {
                        Button {
                            if modelo.insertar() { cerrar() }
                        } label: {
                            Label("Insertar", systemImage: "plus").frame(maxWidth: .infinity, minHeight: 40)
                        }
                        .buttonStyle(.borderedProminent)
                        .tint(Paleta.chat.enviar)
                        .keyboardShortcut(.defaultAction)
                        .disabled(!modelo.puedeInsertar)
                    }
                    Button(role: .destructive) { modelo.quitar() } label: {
                        Label(modelo.confirmarQuitar ? "Pulsa otra vez para quitarla" : "Quitar", systemImage: "trash")
                            .frame(maxWidth: .infinity, minHeight: 40)
                    }
                    .buttonStyle(.bordered)
                }
                .padding(.top, 12)
            }
        } else {
            vacio("Elige una figura para verla en grande")
        }
    }

    private func rotulo(_ s: String) -> some View {
        Text(s.uppercased())
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(.secondary)
            .padding(.bottom, 6)
    }

    /// Un campo del intervalo: su rotulo encima y la caja con la pista de
    /// lo que trae la de serie.
    private func campo(_ c: CampoDeIntervalo) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(c.rotulo).font(.caption).lineLimit(1)
            TextField(c.pista, text: Binding(get: { modelo.valores[c.clave] ?? "" },
                                             set: { modelo.valores[c.clave] = $0 }))
                .textFieldStyle(.roundedBorder)
                .keyboardType(.numbersAndPunctuation)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
