// Las tareas de todos los chats en el iPad (`VentanaTareas` del Mac, la de
// Windows): la barra comun con el buscador (texto o `#etiqueta`) y las
// vistas «Por fecha», «Por lista» y «Kanban»; debajo, la caja para apuntar
// en el Inbox de «Mensajes guardados» (o en la lista de una asignatura), con
// fotos.
//
// Con el dedo: tocar la casilla la marca; deslizar a la derecha la marca y
// a la izquierda la borra (con deshacer); mantenerla pulsada da el menu:
// corregir, urgencia, etiquetas, recordarla (`⏰`, la hace sonar el vigia
// de `Recordatorios`), moverla a otra lista, ver su chat y borrar. En el
// Kanban se arrastra una tarjeta a otra columna para pasarla a esa lista.
//
// Todo lo cambia el nucleo en el mensaje de su chat (`ModeloTareas`, en
// `Tareas.swift`): es lo que viaja a los demas aparatos.

import PhotosUI
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

public struct VistaTareas: View {
    public enum Vista: String, CaseIterable, Identifiable {
        case porFecha = "Por fecha"
        case porLista = "Por lista"
        /// Cada lista una columna; arrastrar una tarjeta la pasa a otra
        /// lista (Windows, 17-oct).
        case kanban = "Kanban"
        public var id: String { rawValue }
    }

    @StateObject private var modelo: ModeloTareas
    @AppStorage("PixPinTareasVista") private var vista: Vista = .porFecha
    @State private var nueva = ""
    @State private var verHechas = false
    @State private var imagenes: [URL] = []
    @State private var elegidas: [PhotosPickerItem] = []
    /// Donde se apunta: vacio el Inbox, o una asignatura (Windows, 18-oct).
    @State private var destino = ""
    @State private var asignaturas: [String] = []
    /// La tarea que se esta corrigiendo, la que pide etiqueta o fecha.
    @State private var corrigiendo: TareaConLista?
    @State private var etiquetando: TareaConLista?
    @State private var fechando: TareaConLista?
    @FocusState private var enLaCaja: Bool

    public init(almacen: Almacen) {
        _modelo = StateObject(wrappedValue: ModeloTareas(nucleo: almacen.nucleo))
    }

    public var body: some View {
        VStack(spacing: 0) {
            Cabecera("Tareas", subtitulo: subtitulo,
                     buscador: BuscadorCabecera($modelo.busqueda, pista: "Buscar: texto o #etiqueta")) {
                SegmentosCabecera(Vista.allCases.map { ($0, $0.rawValue) }, eleccion: $vista)
                BotonCabecera(icono: "arrow.clockwise", ayuda: "Releer (⌘R)") { Task { await modelo.cargar() } }
                    .keyboardShortcut("r")
            }
            caja
            Paleta.v2.linea.frame(height: 1)
            Group {
                switch vista {
                case .porFecha: List { porFecha }.listStyle(.insetGrouped)
                case .porLista: List { porLista }.listStyle(.insetGrouped)
                case .kanban: TableroKanban(modelo: modelo, acciones: acciones)
                }
            }
            .scrollContentBackground(.hidden)
            .background(Paleta.v2.fondo)
            .refreshable { await modelo.cargar() }
            .overlay {
                if modelo.cargado, modelo.listas.allSatisfy({ $0.tareas.isEmpty }) {
                    ContentUnavailableView("Sin tareas", systemImage: "checklist",
                                           description: Text("Las listas de tareas de tus chats aparecen aquí."))
                }
            }
        }
        .ventanaConCabecera()
        .overlay(alignment: .bottom) {
            if let t = modelo.borrada {
                AvisoAbajo(texto: "Borrada: \(t.tarea.texto)") { Task { await modelo.deshacer() } }
            } else if let e = modelo.error {
                AvisoAbajo(texto: e, esError: true)
            }
        }
        .task {
            ArranqueOrganizar.apuntar()
            await modelo.cargar()
            asignaturas = await ModeloTareas.asignaturasFuera(de: modelo)
        }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinTrajoCambios)) { _ in
            Task { await modelo.cargar() }
        }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinTareasCambiadas)) { _ in
            Task { await modelo.cargar() }
        }
        .onChange(of: elegidas) { _, items in
            guard !items.isEmpty else { return }
            elegidas = []
            Task { await traer(items) }
        }
        .sheet(item: $corrigiendo) { t in
            HojaTexto(titulo: "Corregir la tarea", pista: "Tarea", inicial: t.tarea.texto,
                      boton: "Guardar") { nuevo in Task { await modelo.editar(t, a: nuevo) } }
        }
        .sheet(item: $etiquetando) { t in
            HojaTexto(titulo: "Añadir etiqueta", pista: "#etiqueta", inicial: "", boton: "Añadir") { e in
                Task { await modelo.editar(t, a: ModeloTareas.conEtiqueta(t.tarea.texto, e)) }
            }
        }
        .sheet(item: $fechando) { t in
            HojaFecha(inicial: t.tarea.recuerda.map(fechaDeLocales) ?? Date().addingTimeInterval(3600)) { cuando in
                Task { await modelo.recordar(t, en: cuando) }
            }
        }
    }

    private var subtitulo: String {
        let pendientes = modelo.listas.reduce(0) { $0 + Int($1.pendientes) }
        let listas = modelo.listas.count == 1 ? "1 lista" : "\(modelo.listas.count) listas"
        return "\(listas) · \(pendientes) pendientes"
    }

    /// La caja de apuntar (`urgente`, `#etiqueta`, `recordar a las 5`…: lo
    /// entiende el nucleo como en Windows).
    private var caja: some View {
        HStack(spacing: 10) {
            Image(systemName: "plus.circle.fill").foregroundStyle(Paleta.v2.acento).font(.system(size: 20))
            TextField("", text: $nueva,
                      prompt: Text("Apuntar en el Inbox (urgente, #etiqueta, recordar a las 5)").foregroundStyle(Paleta.v2.gris))
                .textFieldStyle(.plain)
                .foregroundStyle(Paleta.v2.texto)
                .submitLabel(.done)
                .focused($enLaCaja)
                .onSubmit(apuntar)
            if !imagenes.isEmpty {
                Button { imagenes = [] } label: {
                    Label("\(imagenes.count)", systemImage: "photo")
                }
                .buttonStyle(.plain)
                .foregroundStyle(Paleta.v2.cuerpo)
                .accessibilityLabel("\(imagenes.count) imágenes; tocar para quitarlas")
            }
            if !asignaturas.isEmpty {
                Picker("Dónde", selection: $destino) {
                    Text("Inbox").tag("")
                    ForEach(asignaturas, id: \.self) { Text($0).tag($0) }
                }
                .labelsHidden()
                .frame(maxWidth: 170)
            }
            PhotosPicker(selection: $elegidas, matching: .images) {
                Image(systemName: "photo.badge.plus").font(.system(size: 17))
            }
            .foregroundStyle(Paleta.v2.gris)
            .accessibilityLabel("Añadir imágenes a la tarea")
            BotonCabecera(rotulo: "Apuntar", activo: true, accion: apuntar)
                .disabled(nueva.trimmingCharacters(in: .whitespaces).isEmpty && imagenes.isEmpty)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(Paleta.v2.fondo)
    }

    private func acciones(_ t: TareaConLista) -> AccionesTarea {
        AccionesTarea(
            marcar: { h in Task { await modelo.marcar(t, hecha: h) } },
            recordar: { c in Task { await modelo.recordar(t, en: c) } },
            elegirHora: { fechando = t },
            corregir: { corrigiendo = t },
            urgencia: { u in Task { await modelo.editar(t, a: ModeloTareas.conUrgencia(t.tarea.texto, u)) } },
            etiquetar: { etiquetando = t },
            quitarEtiqueta: { e in Task { await modelo.editar(t, a: ModeloTareas.sinEtiqueta(t.tarea.texto, e)) } },
            borrar: { Task { await modelo.borrar(t) } },
            mover: { l in Task { await modelo.mover(t, a: l) } },
            verEnChat: { Navegacion.compartida.irAlChat(t.lista.chat, mensaje: t.lista.mensaje) }
        )
    }

    private func fila(_ t: TareaConLista, conLista: Bool) -> some View {
        FilaTarea(t: t, conLista: conLista, listas: modelo.listas, acciones: acciones(t))
            .listRowBackground(Paleta.v2.tarjeta)
    }

    @ViewBuilder
    private var porFecha: some View {
        ForEach(modelo.pendientesPorGrupo) { g in
            Section {
                ForEach(g.tareas) { t in fila(t, conLista: true) }
            } header: {
                Text("\(g.grupo.titulo) · \(g.tareas.count)").foregroundStyle(Paleta.v2.gris)
            }
        }
        let hechas = modelo.hechas
        if !hechas.isEmpty {
            Section {
                DisclosureGroup(isExpanded: $verHechas) {
                    ForEach(hechas) { t in fila(t, conLista: true) }
                } label: {
                    Text("Hechas (\(hechas.count))").foregroundStyle(Paleta.v2.texto)
                }
                .listRowBackground(Paleta.v2.tarjeta)
            }
        }
    }

    @ViewBuilder
    private var porLista: some View {
        ForEach(modelo.listasFiltradas, id: \.mensaje) { l in
            Section {
                ForEach(l.tareas, id: \.indice) { t in fila(TareaConLista(lista: l, tarea: t), conLista: false) }
            } header: {
                CabeceraLista(l: l)
            }
        }
    }

    private func apuntar() {
        let texto = nueva
        let fotos = imagenes
        guard !texto.trimmingCharacters(in: .whitespaces).isEmpty || !fotos.isEmpty else { return }
        nueva = ""
        imagenes = []
        let donde = destino
        Task {
            await modelo.apuntar(texto, imagenes: fotos, asignatura: donde)
            enLaCaja = true
        }
    }

    /// Las fotos elegidas, a temporales que el nucleo se lleva al apuntar.
    private func traer(_ items: [PhotosPickerItem]) async {
        for item in items {
            guard let datos = try? await item.loadTransferable(type: Data.self) else { continue }
            let ext = item.supportedContentTypes.first?.preferredFilenameExtension ?? "jpg"
            let url = FileManager.default.temporaryDirectory
                .appendingPathComponent("pixpin-tarea-\(UUID().uuidString).\(ext)")
            if (try? datos.write(to: url)) != nil { imagenes.append(url) }
        }
    }
}

// MARK: - Cambiar el texto (urgencia y etiquetas)

/// La urgencia de una tarea, con la marca de Windows (`🔼` urgente, `⏫` muy
/// urgente: las que pone el nucleo al entender «urgente»).
public enum UrgenciaTarea: String, CaseIterable, Identifiable {
    case normal, urgente, muyUrgente = "muy_urgente"
    public var id: String { rawValue }

    public var titulo: String {
        switch self {
        case .normal: "Normal"
        case .urgente: "Urgente"
        case .muyUrgente: "Muy urgente"
        }
    }

    var marca: String {
        switch self {
        case .normal: ""
        case .urgente: "🔼"
        case .muyUrgente: "⏫"
        }
    }
}

extension ModeloTareas {
    /// Las marcas de prioridad que entiende el formato de las tareas.
    nonisolated static let marcasDeUrgencia = ["⏫", "🔼", "🔺", "🔽", "⏬"]

    /// `texto` con otra urgencia: fuera la marca que tuviera y, si no es
    /// normal, la nueva al final (el nucleo la entiende al corregir).
    nonisolated static func conUrgencia(_ texto: String, _ u: UrgenciaTarea) -> String {
        var t = texto
        for m in marcasDeUrgencia { t = t.replacingOccurrences(of: m, with: "") }
        t = t.split(separator: " ", omittingEmptySubsequences: true).joined(separator: " ")
        return u == .normal ? t : "\(t) \(u.marca)"
    }

    /// `texto` con una `#etiqueta` mas (sin repetirla).
    nonisolated static func conEtiqueta(_ texto: String, _ etiqueta: String) -> String {
        let limpia = etiqueta.trimmingCharacters(in: .whitespaces)
            .replacingOccurrences(of: "#", with: "")
            .replacingOccurrences(of: " ", with: "_")
        guard !limpia.isEmpty else { return texto }
        let palabras = texto.split(separator: " ").map(String.init)
        guard !palabras.contains("#\(limpia)") else { return texto }
        return "\(texto) #\(limpia)"
    }

    /// `texto` sin esa `#etiqueta`.
    nonisolated static func sinEtiqueta(_ texto: String, _ etiqueta: String) -> String {
        texto.split(separator: " ").map(String.init).filter { $0 != "#\(etiqueta)" }.joined(separator: " ")
    }

    /// Las asignaturas del horario, leidas fuera del hilo principal.
    static func asignaturasFuera(de modelo: ModeloTareas) async -> [String] {
        let nucleo = modelo.nucleoParaLeer
        let d = desfaseLocalMs()
        return await Task.detached(priority: .utility) { nucleo.nombresDeAsignaturas(desfaseMs: d) }.value
    }
}

// MARK: - La fila

/// El titulo de una lista, su chat y cuantas van hechas.
struct CabeceraLista: View {
    let l: ListaTareas

    var body: some View {
        HStack {
            Text(l.titulo.isEmpty ? "Tareas" : l.titulo).bold().foregroundStyle(Paleta.v2.texto)
            Text(l.guardados ? "Mensajes guardados" : l.nombreChat).foregroundStyle(Paleta.v2.gris).lineLimit(1)
            Spacer()
            Text("\(l.tareas.count - Int(l.pendientes)) de \(l.tareas.count)").monospacedDigit()
                .foregroundStyle(Paleta.v2.gris)
        }
        .textCase(nil)
    }
}

/// Lo que se puede hacer con una tarea desde su fila o su tarjeta.
struct AccionesTarea {
    var marcar: (Bool) -> Void = { _ in }
    /// Poner la hora de recordarla (`nil` la quita).
    var recordar: (Date?) -> Void = { _ in }
    /// Elegir la fecha y la hora a mano.
    var elegirHora: () -> Void = {}
    var corregir: () -> Void = {}
    var urgencia: (UrgenciaTarea) -> Void = { _ in }
    var etiquetar: () -> Void = {}
    var quitarEtiqueta: (String) -> Void = { _ in }
    var borrar: () -> Void = {}
    var mover: (ListaTareas) -> Void = { _ in }
    var verEnChat: () -> Void = {}
}

/// Una tarea: su casilla, el texto con sus chapitas (prioridad y
/// #etiquetas), su hora de recordar, cuantos dias lleva y, si se pide, su
/// lista y su chat.
struct FilaTarea: View {
    let t: TareaConLista
    let conLista: Bool
    var listas: [ListaTareas] = []
    var acciones = AccionesTarea()

    private var marcas: MarcasTarea { marcasDeTarea(texto: t.tarea.texto, desfaseMs: desfaseLocalMs()) }

    var body: some View {
        let m = marcas
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Button { acciones.marcar(!t.tarea.hecha) } label: {
                Image(systemName: t.tarea.hecha ? "checkmark.circle.fill" : "circle")
                    .font(.system(size: 22))
                    .foregroundStyle(t.tarea.hecha ? Paleta.v2.verde : Paleta.v2.gris)
                    .frame(width: 30, height: 30)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(t.tarea.hecha ? "Desmarcar" : "Marcar como hecha")
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 4) {
                    ForEach(m.chapitas, id: \.self) { Text($0) }
                    Text(m.vista.isEmpty ? "(sin texto)" : m.vista)
                        .strikethrough(t.tarea.hecha)
                        .foregroundStyle(t.tarea.hecha ? Paleta.v2.gris : Paleta.v2.texto)
                }
                .font(.system(size: 15))
                let chapitas = m.urgencia != nil || !m.etiquetas.isEmpty || t.tarea.recuerda != nil
                if chapitas {
                    HStack(spacing: 4) {
                        if let u = m.urgencia {
                            ChapitaTarea(texto: (m.marcaUrgencia ?? "") + (u == "muy_urgente" ? " muy urgente" : " urgente"),
                                         color: u == "muy_urgente" ? Paleta.v2.rojo : Paleta.v2.naranja)
                        }
                        ForEach(m.etiquetas, id: \.self) { ChapitaTarea(texto: "#\($0)", color: Paleta.v2.acento) }
                        if let r = t.tarea.recuerda {
                            ChapitaTarea(texto: "⏰ " + AtajosDeHora.legible(fechaDeLocales(r), ahora: Date()),
                                         color: Paleta.v2.amarillo)
                        }
                    }
                }
                if conLista {
                    Text("\(t.lista.titulo.isEmpty ? "Tareas" : t.lista.titulo) · \(t.lista.guardados ? "Mensajes guardados" : t.lista.nombreChat)")
                        .font(.caption).foregroundStyle(Paleta.v2.gris)
                }
            }
            Spacer(minLength: 4)
            if !t.tarea.imagenes.isEmpty {
                Image(systemName: "photo").foregroundStyle(Paleta.v2.gris)
                    .accessibilityLabel("\(t.tarea.imagenes.count) imágenes")
            }
            if let d = t.tarea.dias, !t.tarea.hecha {
                Text(d == 0 ? "hoy" : "\(d) d")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(d > 7 ? Paleta.v2.naranja : Paleta.v2.gris)
                    .accessibilityLabel("Creada el \(t.tarea.creada ?? "")")
            }
        }
        .contentShape(Rectangle())
        .swipeActions(edge: .leading) {
            Button { acciones.marcar(!t.tarea.hecha) } label: {
                Label(t.tarea.hecha ? "Pendiente" : "Hecha", systemImage: t.tarea.hecha ? "arrow.uturn.backward" : "checkmark")
            }
            .tint(Paleta.v2.verde)
        }
        .swipeActions(edge: .trailing) {
            Button(role: .destructive) { acciones.borrar() } label: { Label("Borrar", systemImage: "trash") }
        }
        .contextMenu { menu(m) }
    }

    @ViewBuilder
    private func menu(_ m: MarcasTarea) -> some View {
        Button { acciones.corregir() } label: { Label("Corregir", systemImage: "pencil") }
        Button { acciones.marcar(!t.tarea.hecha) } label: {
            Label(t.tarea.hecha ? "Marcar pendiente" : "Marcar hecha", systemImage: "checkmark.circle")
        }
        Menu {
            ForEach(UrgenciaTarea.allCases) { u in
                Button(u.titulo) { acciones.urgencia(u) }
            }
        } label: { Label("Urgencia", systemImage: "exclamationmark.triangle") }
        Menu {
            Button("Añadir etiqueta…") { acciones.etiquetar() }
            ForEach(m.etiquetas, id: \.self) { e in
                Button("Quitar #\(e)", role: .destructive) { acciones.quitarEtiqueta(e) }
            }
        } label: { Label("Etiquetas", systemImage: "number") }
        if !t.tarea.hecha {
            Menu {
                ForEach(AtajosDeHora.atajos(ahora: Date()), id: \.titulo) { a in
                    Button(a.titulo) { acciones.recordar(a.cuando) }
                }
                Button("Elegir fecha y hora…") { acciones.elegirHora() }
            } label: { Label("Recordármelo", systemImage: "alarm") }
        }
        if t.tarea.recuerda != nil {
            Button { acciones.recordar(nil) } label: { Label("Quitar el recordatorio", systemImage: "alarm.waves.left.and.right") }
        }
        let otras = listas.filter { $0.mensaje != t.lista.mensaje || $0.chat != t.lista.chat }
        if !otras.isEmpty {
            Menu {
                ForEach(otras, id: \.mensaje) { l in
                    Button("\(l.titulo.isEmpty ? "Tareas" : l.titulo) · \(l.guardados ? "Mensajes guardados" : l.nombreChat)") {
                        acciones.mover(l)
                    }
                }
            } label: { Label("Mover a…", systemImage: "arrow.right.square") }
        }
        Button { acciones.verEnChat() } label: { Label("Ver en el chat", systemImage: "bubble.left.and.bubble.right") }
        Button { UIPasteboard.general.string = m.titulo } label: { Label("Copiar", systemImage: "doc.on.doc") }
        Divider()
        Button(role: .destructive) { acciones.borrar() } label: { Label("Borrar", systemImage: "trash") }
    }
}

/// Una chapita de una tarea (su urgencia, una etiqueta, su hora).
struct ChapitaTarea: View {
    let texto: String
    let color: Color

    var body: some View {
        Text(texto)
            .font(.caption)
            .padding(.horizontal, 6).padding(.vertical, 1)
            .background(color.opacity(0.18), in: Capsule())
            .foregroundStyle(color)
    }
}

// MARK: - Kanban

/// **El tablero Kanban** (Windows, 17-oct): cada lista una columna con lo
/// pendiente arriba y lo hecho plegado al pie. Arrastrar una tarjeta a otra
/// columna la pasa a esa lista. Lo que no cabe a lo ancho baja a otra
/// banda: nunca hay que desplazarse de lado.
struct TableroKanban: View {
    @ObservedObject var modelo: ModeloTareas
    let acciones: (TareaConLista) -> AccionesTarea

    var body: some View {
        ScrollView {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 260, maximum: 380), spacing: 12, alignment: .top)],
                      alignment: .leading, spacing: 12) {
                ForEach(modelo.listasFiltradas, id: \.mensaje) { l in
                    ColumnaKanban(l: l, modelo: modelo, acciones: acciones)
                }
            }
            .padding(12)
        }
    }
}

struct ColumnaKanban: View {
    let l: ListaTareas
    @ObservedObject var modelo: ModeloTareas
    let acciones: (TareaConLista) -> AccionesTarea
    @State private var encima = false
    @State private var verHechas = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            CabeceraLista(l: l).font(.callout)
            ForEach(l.tareas.filter { !$0.hecha }, id: \.indice) { t in tarjeta(t) }
            let hechas = l.tareas.filter(\.hecha)
            if !hechas.isEmpty {
                DisclosureGroup(isExpanded: $verHechas) {
                    ForEach(hechas, id: \.indice) { t in tarjeta(t) }
                } label: {
                    Text("Hechas (\(hechas.count))").font(.caption).foregroundStyle(Paleta.v2.gris)
                }
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, minHeight: 120, alignment: .topLeading)
        .background(encima ? Paleta.v2.cajaElegida : Paleta.v2.tarjeta, in: RoundedRectangle(cornerRadius: 12))
        .overlay {
            if encima { RoundedRectangle(cornerRadius: 12).strokeBorder(Paleta.v2.acento, lineWidth: 2) }
        }
        .dropDestination(for: String.self) { ids, _ in
            var alguna = false
            for id in ids {
                if let t = modelo.tarea(id: id) {
                    alguna = true
                    Task { await modelo.mover(t, a: l) }
                }
            }
            return alguna
        } isTargeted: { encima = $0 }
    }

    private func tarjeta(_ t: Tarea) -> some View {
        let x = TareaConLista(lista: l, tarea: t)
        return FilaTarea(t: x, conLista: false, listas: modelo.listas, acciones: acciones(x))
            .padding(8)
            .background(Paleta.v2.tarjetaEncima, in: RoundedRectangle(cornerRadius: 8))
            .draggable(x.id) {
                Text(t.texto).padding(8).background(Paleta.v2.tarjetaEncima, in: RoundedRectangle(cornerRadius: 8))
            }
    }
}

// MARK: - Hojas

/// Una hoja con un campo de texto (corregir, poner una etiqueta).
struct HojaTexto: View {
    let titulo: String
    let pista: String
    let inicial: String
    let boton: String
    let hecho: (String) -> Void
    @State private var texto = ""
    @FocusState private var foco: Bool
    @Environment(\.dismiss) private var cerrar

    var body: some View {
        NavigationStack {
            Form {
                TextField(pista, text: $texto, axis: .vertical)
                    .lineLimit(1 ... 5)
                    .focused($foco)
                    .onSubmit(guardar)
            }
            .navigationTitle(titulo)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancelar") { cerrar() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button(boton, action: guardar)
                        .disabled(texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
        }
        .presentationDetents([.medium])
        .onAppear {
            texto = inicial
            foco = true
        }
    }

    private func guardar() {
        let t = texto.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return }
        hecho(t)
        cerrar()
    }
}

/// Elegir la fecha y la hora de recordar una tarea.
struct HojaFecha: View {
    let inicial: Date
    let hecho: (Date) -> Void
    @State private var cuando = Date()
    @Environment(\.dismiss) private var cerrar

    var body: some View {
        NavigationStack {
            Form {
                DatePicker("Cuándo", selection: $cuando, in: Date()..., displayedComponents: [.date, .hourAndMinute])
                    .datePickerStyle(.graphical)
            }
            .navigationTitle("Recordármelo")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancelar") { cerrar() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Poner") {
                        hecho(cuando)
                        cerrar()
                    }
                }
            }
        }
        .onAppear { cuando = max(inicial, Date().addingTimeInterval(60)) }
    }
}
