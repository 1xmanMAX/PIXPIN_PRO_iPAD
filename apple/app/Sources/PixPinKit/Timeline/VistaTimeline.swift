// El timeline en el iPad (`VentanaTimeline` del Mac, `timeline/ventana.rs`
// de Windows): la barra comun con el buscador y las pestanas «Hoy»,
// «Momentos», «Calendario» y «Lecciones», y «Exportar». A la izquierda los
// dias (por paginas, el mas nuevo arriba); tocar uno abre sus momentos (por
// paginas) encima de la pestana. En «Hoy», abajo, la caja de apuntar: texto,
// fotos (de Fotos o la camara) y el microfono para dictar.
//
// Tocar un momento abre su detalle (las fotos enteras, su audio y sus
// botones); mantenerlo pulsado da el menu: mandar a «Momentos», hacerlo
// leccion, corregir, compartir y borrar (con deshacer).
//
// Lo que se lee y se escribe es `timeline/momentos.jsonl`, el de Windows,
// por el nucleo (`ModeloTimeline`, en `Timeline.swift`, del Mac tal cual).

import PhotosUI
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

public struct VistaTimeline: View {
    @StateObject private var modelo: ModeloTimeline
    @StateObject private var dictado = DictadoTimeline()
    @State private var detalle: DetalleMomento?
    @State private var exportado: FicheroExportado?
    @State private var exportando = false
    @Environment(\.horizontalSizeClass) private var clase

    public init(almacen: Almacen) {
        _modelo = StateObject(wrappedValue: ModeloTimeline(nucleo: almacen.nucleo))
    }

    public var body: some View {
        VStack(spacing: 0) {
            Cabecera("Timeline", subtitulo: subtitulo,
                     buscador: BuscadorCabecera($modelo.consulta,
                                                pista: modelo.pestana == .lecciones ? "Buscar en las lecciones" : "Buscar en el timeline")) {
                SegmentosCabecera(ModeloTimeline.Pestana.allCases.map { ($0, $0.rotulo) },
                                  eleccion: Binding(get: { modelo.pestana },
                                                    set: { p in Task { await modelo.cambiar(a: p) } }))
                if modelo.pestana != .lecciones {
                    BotonCabecera(icono: "square.and.arrow.up", ayuda: "Exportar lo que se ve") { exportar() }
                        .disabled(exportando)
                }
            }
            HStack(spacing: 0) {
                if clase != .compact, modelo.pestana != .lecciones {
                    ColumnaDias(modelo: modelo)
                        .frame(width: 230)
                    Paleta.v2.linea.frame(width: 1)
                }
                contenido
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .ventanaConCabecera()
        .overlay(alignment: .bottom) { aviso }
        .onChange(of: modelo.consulta) { _, _ in modelo.consultaCambiada() }
        .onChange(of: modelo.filtro) { _, _ in _ = Task<Void, Never> { await modelo.recargar() } }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinTrajoCambios)) { _ in
            Task { await modelo.recargar() }
        }
        .task {
            ArranqueOrganizar.apuntar()
            await modelo.recargar()
        }
        .onDisappear { dictado.cancelar() }
        .sheet(item: $detalle) { d in
            VistaDetalleMomento(modelo: modelo, lista: d.lista, indice: d.indice, foto: d.foto) { m in
                detalle = nil
                Task { await modelo.corregir(m) }
            }
        }
        .sheet(item: $exportado) { f in
            HojaExportado(fichero: f)
                .presentationDetents([.medium])
        }
        .background {
            // ⌘1…4, como en Windows y el Mac.
            ZStack {
                ForEach(ModeloTimeline.Pestana.allCases) { p in
                    Button("") { Task { await modelo.cambiar(a: p) } }
                        .keyboardShortcut(KeyEquivalent(Character(String(p.rawValue))))
                }
                Button("") { Task { await modelo.deshacerBorrado() } }
                    .keyboardShortcut("z")
                    .disabled(modelo.deshacer == nil)
            }
            .opacity(0)
            .allowsHitTesting(false)
        }
    }

    private var subtitulo: String {
        let n = modelo.totalDias
        return n == 1 ? "1 día con algo" : "\(n) días con algo"
    }

    @ViewBuilder
    private var contenido: some View {
        switch modelo.modo {
        case .buscar:
            ResultadosTimeline(modelo: modelo, abrir: abrir)
        case let .dia(iso):
            VStack(spacing: 0) {
                HStack {
                    Button { Task { await modelo.volverDelDia() } } label: {
                        Label("Volver", systemImage: "chevron.left")
                    }
                    .foregroundStyle(Paleta.v2.acento)
                    Spacer()
                    Text(ModeloTimeline.rotulo(iso: iso).capitalizedPrimera)
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(Paleta.v2.texto)
                    Spacer()
                    Text(modelo.totalMomentos == 1 ? "1 momento" : "\(modelo.totalMomentos) momentos")
                        .font(.system(size: 12)).foregroundStyle(Paleta.v2.gris)
                }
                .padding(.horizontal, 16).padding(.vertical, 10)
                Paleta.v2.linea.frame(height: 1)
                LineaDeMomentos(modelo: modelo, abrir: abrir)
            }
        case .pestana(.hoy):
            VStack(spacing: 0) {
                LineaDeMomentos(modelo: modelo, abrir: abrir)
                CajaTimeline(modelo: modelo, dictado: dictado)
            }
        case .pestana(.momentos):
            ArchivoTimeline(modelo: modelo, abrir: abrir)
        case .pestana(.estado):
            CalendarioTimeline(modelo: modelo)
        case .pestana(.lecciones):
            CuerpoLecciones(nucleo: modelo.nucleo, texto: modelo.consulta)
        }
    }

    @ViewBuilder
    private var aviso: some View {
        if let t = modelo.aviso {
            AvisoAbajo(texto: t, deshacer: modelo.deshacer != nil ? deshacerBorrado : nil)
                .padding(.bottom, modelo.modo == .pestana(.hoy) ? 70 : 0)
        } else if let e = modelo.error {
            AvisoAbajo(texto: e, esError: true)
        }
    }

    private func deshacerBorrado() {
        Task { await modelo.deshacerBorrado() }
    }

    private func abrir(_ m: MomentoTimeline, _ foto: Int) {
        let lista = modelo.visibles
        let i = lista.firstIndex { $0.id == m.id }
        detalle = DetalleMomento(lista: i == nil ? [m] : lista, indice: i ?? 0, foto: foto)
    }

    /// «Exportar»: la pagina HTML de lo que se ve, a un fichero para
    /// compartir.
    private func exportar() {
        exportando = true
        Task {
            defer { exportando = false }
            do {
                let p = try await modelo.paginaParaExportar()
                let nombre = p.nombre.isEmpty ? "timeline.html" : p.nombre
                let url = FileManager.default.temporaryDirectory.appendingPathComponent(nombre)
                try Data(p.html.utf8).write(to: url, options: .atomic)
                exportado = FicheroExportado(url: url, cuantos: p.cuantos)
            } catch {
                modelo.avisar("No se pudo exportar")
            }
        }
    }
}

/// Lo que abre el detalle: la lista que se ve, el momento y su foto.
struct DetalleMomento: Identifiable {
    let id = UUID()
    let lista: [MomentoTimeline]
    let indice: Int
    let foto: Int
}

/// La pagina exportada, lista para compartir.
struct FicheroExportado: Identifiable {
    let url: URL
    let cuantos: UInt32
    var id: String { url.path }
}

struct HojaExportado: View {
    let fichero: FicheroExportado
    @Environment(\.dismiss) private var cerrar

    var body: some View {
        VStack(spacing: 18) {
            Image(systemName: "doc.richtext").font(.system(size: 40)).foregroundStyle(Paleta.v2.acento)
            Text(fichero.url.lastPathComponent).font(.headline)
            Text(fichero.cuantos == 1 ? "1 momento" : "\(fichero.cuantos) momentos")
                .foregroundStyle(.secondary)
            HStack(spacing: 12) {
                Button {
                    cerrar()
                    Navegacion.compartida.abrir(.vistaRapida(url: fichero.url))
                } label: { Label("Ver", systemImage: "eye") }
                    .buttonStyle(.bordered)
                ShareLink(item: fichero.url) { Label("Compartir", systemImage: "square.and.arrow.up") }
                    .buttonStyle(.borderedProminent)
            }
        }
        .padding(24)
    }
}

// MARK: - Los dias

/// La columna de los dias (`dias` de Windows): todos, o solo los que tienen
/// algo en «Momentos»; por paginas.
struct ColumnaDias: View {
    @ObservedObject var modelo: ModeloTimeline

    var body: some View {
        VStack(spacing: 0) {
            Picker("Ver", selection: $modelo.filtro) {
                ForEach(ModeloTimeline.Filtro.allCases) { f in Text(f.rawValue).tag(f) }
            }
            .pickerStyle(.segmented)
            .padding(10)
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    fila(titulo: "Últimas 24 horas", detalle: nil, elegido: modelo.eleccion == .hoy) {
                        Task {
                            if modelo.pestana != .hoy { await modelo.cambiar(a: .hoy) } else { await modelo.elegir(.hoy) }
                        }
                    }
                    ForEach(modelo.dias, id: \.iso) { d in
                        fila(titulo: ModeloTimeline.rotulo(d).capitalizedPrimera,
                             detalle: "\(d.cuantos)",
                             elegido: modelo.eleccion == .dia(d.iso)) {
                            modelo.consulta = ""
                            Task { await modelo.elegir(.dia(d.iso)) }
                        }
                    }
                    if modelo.quedanDias {
                        ProgressView().frame(maxWidth: .infinity).padding()
                            .task { await modelo.cargarMasDias() }
                    }
                }
                .padding(.horizontal, 8)
                .padding(.bottom, 12)
            }
        }
        .background(Paleta.v2.fondo)
    }

    private func fila(titulo: String, detalle: String?, elegido: Bool, accion: @escaping () -> Void) -> some View {
        Button(action: accion) {
            HStack {
                Text(titulo).font(.system(size: 13.5)).lineLimit(1)
                    .foregroundStyle(elegido ? Color.white : Paleta.v2.texto)
                Spacer(minLength: 4)
                if let detalle {
                    Text(detalle).font(.system(size: 12).monospacedDigit())
                        .foregroundStyle(elegido ? Color.white.opacity(0.8) : Paleta.v2.gris)
                }
            }
            .padding(.horizontal, 10)
            .frame(minHeight: 40)
            .background(elegido ? Paleta.v2.azulLleno : .clear, in: RoundedRectangle(cornerRadius: 8))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

// MARK: - La linea

/// Los momentos de lo elegido («Hoy» o un dia), como una linea de tiempo
/// vertical. En «Hoy» se queda pegada al final (lo nuevo), como un chat.
struct LineaDeMomentos: View {
    @ObservedObject var modelo: ModeloTimeline
    let abrir: (MomentoTimeline, Int) -> Void

    var body: some View {
        ScrollViewReader { lector in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(modelo.momentos, id: \.id) { m in
                        TarjetaMomento(momento: m, modelo: modelo, abrir: abrir).id(m.id)
                    }
                    if modelo.quedanMomentos {
                        ProgressView().frame(maxWidth: .infinity).padding()
                            .task { await modelo.cargarMasMomentos() }
                    }
                }
                .padding(16)
            }
            .scrollDismissesKeyboard(.interactively)
            .onChange(of: modelo.momentos.last?.id) { _, ultimo in
                guard modelo.eleccion == .hoy, let ultimo else { return }
                withAnimation { lector.scrollTo(ultimo, anchor: .bottom) }
            }
            .onAppear {
                if modelo.eleccion == .hoy, let u = modelo.momentos.last?.id { lector.scrollTo(u, anchor: .bottom) }
            }
        }
        .overlay {
            if modelo.momentos.isEmpty, !modelo.quedanMomentos {
                Text(modelo.eleccion == .hoy
                    ? "Aún no hay nada en las últimas 24 horas.\nEscribe abajo lo que está pasando, o díctalo con el micrófono."
                    : "Ese día no tiene nada apuntado.")
                    .multilineTextAlignment(.center)
                    .foregroundStyle(Paleta.v2.gris)
                    .padding()
            }
        }
    }
}

/// Lo encontrado en todo el timeline, por dias (lo mas nuevo arriba).
struct ResultadosTimeline: View {
    @ObservedObject var modelo: ModeloTimeline
    let abrir: (MomentoTimeline, Int) -> Void

    private var porDias: [(String, [MomentoTimeline])] {
        var out: [(String, [MomentoTimeline])] = []
        for m in modelo.encontrados {
            let d = ModeloTimeline.iso(de: m.cuando)
            if out.last?.0 == d { out[out.count - 1].1.append(m) } else { out.append((d, [m])) }
        }
        return out
    }

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0, pinnedViews: .sectionHeaders) {
                ForEach(porDias, id: \.0) { dia in
                    Section {
                        ForEach(dia.1, id: \.id) { m in
                            TarjetaMomento(momento: m, modelo: modelo, abrir: abrir)
                        }
                    } header: {
                        Button {
                            modelo.consulta = ""
                            Task { await modelo.elegir(.dia(dia.0)) }
                        } label: {
                            Text(ModeloTimeline.rotulo(iso: dia.0).capitalizedPrimera)
                                .font(.system(size: 13, weight: .semibold))
                                .foregroundStyle(Paleta.v2.gris)
                                .padding(.vertical, 8)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(Paleta.v2.fondo)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .padding(16)
        }
        .scrollDismissesKeyboard(.immediately)
        .overlay {
            if modelo.encontrados.isEmpty {
                Text("No hay ningún momento con eso.").foregroundStyle(Paleta.v2.gris)
            }
        }
    }
}

/// Un momento: la hora y un punto en la linea, y su tarjeta con el titulo,
/// lo contado, sus fotos, su audio y sus marcas. Tocarla abre el detalle.
struct TarjetaMomento: View {
    let momento: MomentoTimeline
    @ObservedObject var modelo: ModeloTimeline
    let abrir: (MomentoTimeline, Int) -> Void
    @ObservedObject private var reproductor = ReproductorVoz.compartido

    var body: some View {
        HStack(alignment: .top, spacing: 0) {
            Text(momento.hora)
                .font(.system(size: 12).monospacedDigit())
                .foregroundStyle(Paleta.v2.gris)
                .frame(width: 48, alignment: .trailing)
                .padding(.top, 14)
            VStack(spacing: 0) {
                Rectangle().fill(Color(hex: 0x3A3A3D)).frame(width: 2, height: 16)
                Circle()
                    .fill(momento.enMomentos ? Paleta.v2.naranja : Paleta.v2.acento)
                    .frame(width: 10, height: 10)
                Rectangle().fill(Color(hex: 0x3A3A3D)).frame(width: 2).frame(maxHeight: .infinity)
            }
            .frame(width: 36)
            tarjeta
                .padding(.bottom, 12)
        }
    }

    private var tarjeta: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Text(momento.titulo.isEmpty ? "Sin título" : momento.titulo)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(Paleta.v2.texto)
                if momento.leccion != nil {
                    Image(systemName: "lightbulb.fill").foregroundStyle(Paleta.v2.amarillo)
                        .accessibilityLabel("Lección aprendida")
                }
                if momento.enMomentos {
                    Image(systemName: "bookmark.fill").foregroundStyle(Paleta.v2.naranja)
                        .accessibilityLabel("En Momentos")
                }
                Spacer(minLength: 0)
            }
            if !momento.descripcion.isEmpty {
                Text(momento.descripcion)
                    .font(.system(size: 14))
                    .foregroundStyle(Paleta.v2.cuerpo)
            }
            if !momento.fotos.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    LazyHStack(spacing: 6) {
                        ForEach(Array(momento.fotos.enumerated()), id: \.offset) { i, ruta in
                            MiniaturaCaptura(url: URL(fileURLWithPath: ruta), esVideo: false)
                                .frame(width: 140, height: 105)
                                .clipShape(RoundedRectangle(cornerRadius: 8))
                                .onTapGesture { abrir(momento, i) }
                        }
                    }
                }
            }
            if let audio = momento.audio {
                Button {
                    ReproductorVoz.compartido.alternar(momento.id, url: URL(fileURLWithPath: audio),
                                                       titulo: momento.titulo)
                } label: {
                    Label(TarjetaMomento.duracion(momento.duracionMs),
                          systemImage: reproductor.sonando && reproductor.actual == momento.id
                              ? "stop.circle.fill" : "play.circle.fill")
                        .font(.system(size: 14))
                        .padding(.horizontal, 12).frame(height: 34)
                        .background(Paleta.v2.caja, in: Capsule())
                }
                .buttonStyle(.plain)
                .foregroundStyle(Paleta.v2.texto)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Paleta.v2.tarjeta, in: RoundedRectangle(cornerRadius: 14))
        .overlay {
            if modelo.corrigiendo?.id == momento.id {
                RoundedRectangle(cornerRadius: 14).strokeBorder(Paleta.v2.acento, lineWidth: 2)
            }
        }
        .contentShape(RoundedRectangle(cornerRadius: 14))
        .onTapGesture { abrir(momento, 0) }
        .contextMenu { MenuMomento(momento: momento, modelo: modelo, abrir: abrir) }
        .hoverEffect(.highlight)
    }

    /// «0:42» de los ms de un audio.
    static func duracion(_ ms: Int64) -> String {
        let s = Int(max(0, ms) / 1000)
        return String(format: "%d:%02d", s / 60, s % 60)
    }
}

/// Lo que se puede hacer con un momento (su menu).
struct MenuMomento: View {
    let momento: MomentoTimeline
    @ObservedObject var modelo: ModeloTimeline
    let abrir: (MomentoTimeline, Int) -> Void

    var body: some View {
        Button { abrir(momento, 0) } label: { Label("Ver", systemImage: "eye") }
        Button { Task { await modelo.alternarMomentos(momento) } } label: {
            Label(momento.enMomentos ? "Quitar de Momentos" : "Mandar a Momentos",
                  systemImage: momento.enMomentos ? "bookmark.slash" : "bookmark")
        }
        Button { Task { await modelo.alternarLeccion(momento) } } label: {
            Label(momento.leccion != nil ? "Ya no es lección" : "Hacer lección", systemImage: "lightbulb")
        }
        Button { Task { await modelo.corregir(momento) } } label: { Label("Corregir", systemImage: "pencil") }
        Button { UIPasteboard.general.string = MenuMomento.texto(momento) } label: {
            Label("Copiar el texto", systemImage: "doc.on.doc")
        }
        let fotos = momento.fotos.map { URL(fileURLWithPath: $0) }
        if fotos.isEmpty {
            ShareLink(item: MenuMomento.texto(momento)) { Label("Compartir…", systemImage: "square.and.arrow.up") }
        } else {
            ShareLink(items: fotos) { Label("Compartir las fotos…", systemImage: "square.and.arrow.up") }
        }
        Divider()
        Button(role: .destructive) { Task { await modelo.borrar(momento) } } label: {
            Label("Borrar", systemImage: "trash")
        }
    }

    static func texto(_ m: MomentoTimeline) -> String {
        let cabeza = "\(m.hora) \(m.titulo)"
        return m.descripcion.isEmpty ? cabeza : "\(cabeza)\n\(m.descripcion)"
    }
}

// MARK: - La caja de apuntar

/// Abajo en «Hoy», como en un chat: el texto (↩ apunta), las fotos
/// elegidas (de Fotos), el microfono y el boton de apuntar.
struct CajaTimeline: View {
    @ObservedObject var modelo: ModeloTimeline
    @ObservedObject var dictado: DictadoTimeline
    @State private var elegidas: [PhotosPickerItem] = []
    @FocusState private var foco: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let r = dictado.rotulo {
                Text(r).font(.system(size: 12.5)).foregroundStyle(Paleta.v2.gris)
            }
            if modelo.corrigiendo != nil {
                HStack {
                    Label("Corrigiendo", systemImage: "pencil").font(.system(size: 12.5))
                        .foregroundStyle(Paleta.v2.acento)
                    Spacer()
                    Button("Cancelar") { modelo.vaciarCaja() }.font(.system(size: 12.5))
                }
            }
            if !modelo.pegadas.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(Array(modelo.pegadas.enumerated()), id: \.offset) { i, f in
                            MiniaturaCaptura(url: URL(fileURLWithPath: f.ruta), esVideo: false)
                                .frame(width: 64, height: 64)
                                .clipShape(RoundedRectangle(cornerRadius: 8))
                                .overlay(alignment: .topTrailing) {
                                    Button { modelo.quitarPegada(i) } label: {
                                        Image(systemName: "xmark.circle.fill").foregroundStyle(.white, .black.opacity(0.6))
                                    }
                                    .padding(2)
                                }
                        }
                    }
                }
            }
            HStack(alignment: .bottom, spacing: 8) {
                PhotosPicker(selection: $elegidas, matching: .images) {
                    Image(systemName: "photo.on.rectangle")
                        .font(.system(size: 18))
                        .frame(width: 40, height: 40)
                }
                .foregroundStyle(Paleta.v2.gris)
                .accessibilityLabel("Añadir fotos")
                TextField("¿Qué está pasando?", text: $modelo.texto, axis: .vertical)
                    .lineLimit(1 ... 6)
                    .textFieldStyle(.plain)
                    .font(.system(size: 15))
                    .foregroundStyle(Paleta.v2.texto)
                    .padding(.horizontal, 12).padding(.vertical, 10)
                    .background(Paleta.v2.tarjeta, in: RoundedRectangle(cornerRadius: 18))
                    .focused($foco)
                    .onSubmit(enviar)
                Button { Task { await dictado.pulsar(modelo: modelo) } } label: {
                    Image(systemName: dictado.grabando ? "stop.circle.fill" : "mic")
                        .font(.system(size: 18))
                        .foregroundStyle(dictado.grabando ? Paleta.v2.rojo : Paleta.v2.gris)
                        .frame(width: 40, height: 40)
                }
                .buttonStyle(.plain)
                .keyboardShortcut("m", modifiers: [.command, .shift])
                .accessibilityLabel(dictado.grabando ? "Terminar de dictar" : "Dictar")
                Button(action: enviar) {
                    Image(systemName: "arrow.up.circle.fill")
                        .font(.system(size: 30))
                        .foregroundStyle(Paleta.v2.acento)
                }
                .buttonStyle(.plain)
                .disabled(modelo.texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    && modelo.pegadas.isEmpty)
                .accessibilityLabel(modelo.corrigiendo == nil ? "Apuntar" : "Guardar")
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(Paleta.v2.fondo)
        .overlay(alignment: .top) { Paleta.v2.linea.frame(height: 1) }
        .onChange(of: elegidas) { _, items in
            guard !items.isEmpty else { return }
            elegidas = []
            Task { await traer(items) }
        }
    }

    private func enviar() {
        Task {
            if await modelo.enviar(audio: dictado.tomarPendiente()) { foco = true }
        }
    }

    /// Las fotos de Fotos, a temporales que el nucleo se lleva al apuntar.
    private func traer(_ items: [PhotosPickerItem]) async {
        for item in items {
            guard let datos = try? await item.loadTransferable(type: Data.self) else { continue }
            let ext = item.supportedContentTypes.first?.preferredFilenameExtension ?? "jpg"
            let url = FileManager.default.temporaryDirectory
                .appendingPathComponent("pixpin-timeline-\(UUID().uuidString).\(ext)")
            if (try? datos.write(to: url)) != nil {
                modelo.anadirFoto(url, temporal: true)
            }
        }
    }
}

// MARK: - Momentos y Calendario

/// El archivo: cada dia, su numero grande con el mes y el dia de la semana,
/// y en fila lo que paso. Tocar uno abre su detalle; el numero, el dia.
struct ArchivoTimeline: View {
    @ObservedObject var modelo: ModeloTimeline
    let abrir: (MomentoTimeline, Int) -> Void

    private let columnas = [GridItem(.adaptive(minimum: 120, maximum: 160), spacing: 8)]

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 18) {
                ForEach(Array(modelo.archivo.enumerated()), id: \.element.dia.iso) { i, d in
                    if i == 0 || modelo.archivo[i - 1].dia.anio != d.dia.anio {
                        Text(String(d.dia.anio)).font(.system(size: 26, weight: .bold)).foregroundStyle(Paleta.v2.texto)
                    }
                    HStack(alignment: .top, spacing: 16) {
                        Button {
                            Task { await modelo.elegir(.dia(d.dia.iso)) }
                        } label: {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(String(d.dia.dia)).font(.system(size: 28, weight: .bold))
                                Text("\(d.nombreMes) · \(d.diaCorto)").font(.system(size: 11))
                                    .foregroundStyle(Paleta.v2.gris)
                            }
                            .foregroundStyle(Paleta.v2.texto)
                            .frame(width: 84, alignment: .leading)
                        }
                        .buttonStyle(.plain)
                        LazyVGrid(columns: columnas, alignment: .leading, spacing: 8) {
                            ForEach(d.momentos, id: \.id) { m in
                                MiniMomento(momento: m)
                                    .onTapGesture { abrir(m, 0) }
                                    .contextMenu { MenuMomento(momento: m, modelo: modelo, abrir: abrir) }
                            }
                        }
                    }
                }
                if modelo.quedaArchivo {
                    ProgressView().frame(maxWidth: .infinity).task { await modelo.cargarMasArchivo() }
                }
            }
            .padding(20)
        }
        .overlay {
            if modelo.archivo.isEmpty {
                Text("Aquí van los momentos que elijas. En «Hoy», mantén pulsada una tarjeta y elige «Mandar a Momentos».")
                    .multilineTextAlignment(.center).foregroundStyle(Paleta.v2.gris).frame(maxWidth: 380)
            }
        }
    }
}

/// Un momento en pequeno: su primera foto, o su titulo sobre un color que
/// sale de su id (siempre el mismo).
struct MiniMomento: View {
    let momento: MomentoTimeline

    var body: some View {
        ZStack(alignment: .bottomLeading) {
            if let f = momento.fotos.first {
                MiniaturaCaptura(url: URL(fileURLWithPath: f), esVideo: false)
            } else {
                Paleta.avatar(momento.id).opacity(0.55)
                Text(momento.titulo).font(.system(size: 12.5, weight: .medium)).foregroundStyle(.white)
                    .lineLimit(4).padding(8)
            }
            if momento.audio != nil {
                Image(systemName: "waveform").font(.system(size: 11)).foregroundStyle(.white)
                    .padding(5).background(.black.opacity(0.5), in: Circle()).padding(5)
            }
        }
        .frame(width: 120, height: 120)
        .clipShape(RoundedRectangle(cornerRadius: 10))
        .accessibilityLabel(momento.titulo)
    }
}

/// Una tarjeta por mes: su nombre y lo que mas se repitio, y sus dias; los
/// que tienen algo, en color. Tocar uno abre el dia; mantenerlo pulsado lo
/// elige para exportar varios juntos.
struct CalendarioTimeline: View {
    @ObservedObject var modelo: ModeloTimeline

    private let columnas = Array(repeating: GridItem(.flexible(), spacing: 6), count: 7)

    var body: some View {
        ScrollView {
            LazyVStack(spacing: 14) {
                ForEach(modelo.meses, id: \.self) { mes in
                    VStack(alignment: .leading, spacing: 10) {
                        HStack(alignment: .firstTextBaseline) {
                            Text(String(mes.mes)).font(.system(size: 30, weight: .bold))
                            Text("\(mes.nombre) \(String(mes.anio))").font(.system(size: 15, weight: .semibold))
                            Spacer()
                            if let mas = mes.loMas {
                                Text("Lo más: \(mas) (\(mes.loMasVeces))").font(.system(size: 12))
                                    .foregroundStyle(Paleta.v2.gris).lineLimit(1)
                            }
                        }
                        .foregroundStyle(Paleta.v2.texto)
                        LazyVGrid(columns: columnas, spacing: 6) {
                            ForEach(mes.dias, id: \.iso) { d in dia(d) }
                        }
                    }
                    .padding(14)
                    .background(Paleta.v2.tarjeta, in: RoundedRectangle(cornerRadius: 14))
                }
            }
            .padding(20)
            .padding(.bottom, modelo.elegidos.isEmpty ? 0 : 60)
        }
        .overlay(alignment: .bottom) {
            if !modelo.elegidos.isEmpty {
                HStack(spacing: 12) {
                    Text(modelo.elegidos.count == 1 ? "1 día elegido para exportar" : "\(modelo.elegidos.count) días elegidos para exportar")
                        .foregroundStyle(Paleta.v2.texto)
                    BotonCabecera(icono: "xmark", ayuda: "Quitar la elección", tenue: true) { modelo.elegidos = [] }
                }
                .padding(10)
                .background(Paleta.v2.tarjetaEncima, in: Capsule())
                .padding(.bottom, 16)
            }
        }
        .overlay {
            if modelo.meses.isEmpty {
                Text("Aún no hay meses con algo apuntado.").foregroundStyle(Paleta.v2.gris)
            }
        }
    }

    private func dia(_ d: DiaEstado) -> some View {
        let elegido = modelo.elegidos.contains(d.iso)
        return Text(String(d.dia))
            .font(.system(size: 13, weight: d.cuantos > 0 ? .semibold : .regular).monospacedDigit())
            .foregroundStyle(d.cuantos > 0 ? Color.white : Paleta.v2.gris)
            .frame(maxWidth: .infinity, minHeight: 36)
            .background(d.cuantos > 0 ? Paleta.v2.acento.opacity(min(1, 0.35 + Double(d.cuantos) * 0.12)) : Paleta.v2.caja,
                        in: Circle())
            .overlay { if elegido { Circle().strokeBorder(Paleta.v2.naranja, lineWidth: 2) } }
            .contentShape(Circle())
            .onTapGesture {
                guard d.cuantos > 0 else { return }
                Task { await modelo.elegir(.dia(d.iso)) }
            }
            .onLongPressGesture {
                if elegido { modelo.elegidos.remove(d.iso) } else { modelo.elegidos.insert(d.iso) }
            }
            .accessibilityLabel("\(d.dia): \(d.cuantos) momentos")
    }
}

// MARK: - El detalle

/// El detalle de un momento: la foto entera sobre negro (se pasa con el
/// dedo), lo contado y sus botones; las flechas del teclado pasan de foto y
/// de momento, como en el Mac.
struct VistaDetalleMomento: View {
    @ObservedObject var modelo: ModeloTimeline
    let lista: [MomentoTimeline]
    @State var indice: Int
    @State var foto: Int
    let corregir: (MomentoTimeline) -> Void
    @Environment(\.dismiss) private var cerrar
    @ObservedObject private var reproductor = ReproductorVoz.compartido

    init(modelo: ModeloTimeline, lista: [MomentoTimeline], indice: Int, foto: Int,
         corregir: @escaping (MomentoTimeline) -> Void) {
        self.modelo = modelo
        self.lista = lista
        _indice = State(initialValue: indice)
        _foto = State(initialValue: foto)
        self.corregir = corregir
    }

    private var momento: MomentoTimeline? { lista.indices.contains(indice) ? lista[indice] : nil }

    var body: some View {
        NavigationStack {
            if let m = momento {
                VStack(spacing: 0) {
                    if !m.fotos.isEmpty {
                        TabView(selection: $foto) {
                            ForEach(Array(m.fotos.enumerated()), id: \.offset) { i, ruta in
                                FotoEntera(url: URL(fileURLWithPath: ruta))
                                    .tag(i)
                                    .onTapGesture { Navegacion.compartida.abrir(.vistaRapida(url: URL(fileURLWithPath: ruta))) }
                            }
                        }
                        .tabViewStyle(.page(indexDisplayMode: m.fotos.count > 1 ? .always : .never))
                        .background(Color.black)
                        .frame(maxHeight: .infinity)
                    }
                    ScrollView {
                        VStack(alignment: .leading, spacing: 10) {
                            Text("\(ModeloTimeline.rotulo(iso: ModeloTimeline.iso(de: m.cuando)).capitalizedPrimera) · \(m.hora)")
                                .font(.system(size: 12.5)).foregroundStyle(Paleta.v2.gris)
                            Text(m.titulo.isEmpty ? "Sin título" : m.titulo)
                                .font(.system(size: 20, weight: .semibold)).foregroundStyle(Paleta.v2.texto)
                                .textSelection(.enabled)
                            if !m.descripcion.isEmpty {
                                Text(m.descripcion).font(.system(size: 15)).foregroundStyle(Paleta.v2.cuerpo)
                                    .textSelection(.enabled)
                            }
                            if let audio = m.audio {
                                Button {
                                    ReproductorVoz.compartido.alternar(m.id, url: URL(fileURLWithPath: audio), titulo: m.titulo)
                                } label: {
                                    Label(TarjetaMomento.duracion(m.duracionMs),
                                          systemImage: reproductor.sonando && reproductor.actual == m.id
                                              ? "stop.circle.fill" : "play.circle.fill")
                                }
                                .buttonStyle(.bordered)
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(20)
                    }
                    .frame(maxHeight: m.fotos.isEmpty ? .infinity : 220)
                }
                .background(Paleta.v2.fondo)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cerrar") { cerrar() }
                    }
                    ToolbarItemGroup(placement: .primaryAction) {
                        Button { Task { await modelo.alternarMomentos(m) } } label: {
                            Image(systemName: m.enMomentos ? "bookmark.fill" : "bookmark")
                        }
                        .accessibilityLabel(m.enMomentos ? "Quitar de Momentos" : "Mandar a Momentos")
                        Button { Task { await modelo.alternarLeccion(m) } } label: {
                            Image(systemName: m.leccion != nil ? "lightbulb.fill" : "lightbulb")
                        }
                        .accessibilityLabel("Hacer lección")
                        Button { corregir(m) } label: { Image(systemName: "pencil") }
                            .accessibilityLabel("Corregir")
                        if m.fotos.isEmpty {
                            ShareLink(item: MenuMomento.texto(m))
                        } else {
                            ShareLink(items: m.fotos.map { URL(fileURLWithPath: $0) })
                        }
                        Button(role: .destructive) {
                            cerrar()
                            Task { await modelo.borrar(m) }
                        } label: { Image(systemName: "trash") }
                            .accessibilityLabel("Borrar")
                    }
                }
                .background {
                    ZStack {
                        Button("") { pasarMomento(-1) }.keyboardShortcut(.upArrow, modifiers: [])
                        Button("") { pasarMomento(1) }.keyboardShortcut(.downArrow, modifiers: [])
                        Button("") { if foto > 0 { foto -= 1 } }.keyboardShortcut(.leftArrow, modifiers: [])
                        Button("") { if foto < m.fotos.count - 1 { foto += 1 } }
                            .keyboardShortcut(.rightArrow, modifiers: [])
                    }
                    .opacity(0)
                    .allowsHitTesting(false)
                }
            } else {
                ContentUnavailableView("Ya no está", systemImage: "clock")
            }
        }
        .environment(\.colorScheme, .dark)
    }

    private func pasarMomento(_ paso: Int) {
        let n = indice + paso
        guard lista.indices.contains(n) else { return }
        indice = n
        foto = 0
    }
}

/// Una foto entera, ajustada (ImageIO a un tamano grande, de la cache).
struct FotoEntera: View {
    let url: URL
    @State private var imagen: CGImage?

    var body: some View {
        ZStack {
            Color.black
            if let imagen {
                Image(decorative: imagen, scale: 1).resizable().scaledToFit()
            } else {
                ProgressView()
            }
        }
        .task(id: url) {
            imagen = await Miniaturas.compartidas.cargar(url, lado: 2400)
        }
    }
}
