// Las tareas de todos los chats en el iPad (`VentanaTareas` del Mac, la de
// Windows): la barra comun con el conmutador de las **tres vistas**
// (Windows 5139e5d, 25-oct-2026): **Tarjetas** (cada lista con lo pendiente
// en cuadrados, con su primera foto de fondo, y lo hecho plegado),
// **Kanban** (una columna por lista) y **Horario** (el calendario semanal de
// las clases, `VistaHorarioIPad`, con las tareas con hora como chapitas).
// La vista se guarda con la palabra de Windows (lo que el iPad guardaba
// antes, «Por fecha» o «Por lista», abre en las tarjetas). Con teclado,
// ⌃⌘T, ⌃⌘K y ⌃⌘H cambian de vista, como en el Mac.
//
// Fuera del Horario, el buscador (texto o `#etiqueta`) y, debajo, la caja
// para apuntar en el Inbox de «Mensajes guardados» (o en la lista de una
// asignatura), con fotos.
//
// Con el dedo: tocar la casilla la marca; mantenerla pulsada da el menu:
// corregir, urgencia, etiquetas, recordarla (`⏰`, la hace sonar el vigia
// de `Recordatorios`), moverla a otra lista, ver su chat y borrar (con
// deshacer). Una tarjeta se arrastra a otra columna del Kanban para
// pasarla a esa lista.
//
// Todo lo cambia el nucleo en el mensaje de su chat (`ModeloTareas`, en
// `Tareas.swift`): es lo que viaja a los demas aparatos.

import PhotosUI
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

public struct VistaTareas: View {
    @StateObject private var modelo: ModeloTareas
    /// El horario semanal de la vista Horario.
    @StateObject private var semana: ModeloSemana
    /// La vista elegida, con la palabra de Windows (`tareas-vista.txt`).
    @AppStorage("PixPinTareasVista") private var vistaGuardada = "tarjetas"
    /// En el Horario, si se ve el domingo (el boton de la barra).
    @AppStorage("PixPinHorarioDomingo") private var conDomingo = false
    @State private var nueva = ""
    @State private var imagenes: [URL] = []
    @State private var elegidas: [PhotosPickerItem] = []
    /// Donde se apunta: vacio el Inbox, o una asignatura (Windows, 18-oct).
    @State private var destino = ""
    @State private var asignaturas: [String] = []
    /// La tarea que se esta corrigiendo, la que pide etiqueta o fecha.
    @State private var corrigiendo: TareaConLista?
    @State private var etiquetando: TareaConLista?
    @State private var fechando: TareaConLista?
    /// La tarea que se ensena en las tarjetas (una chapita del Horario).
    @State private var resaltada: String?
    /// El horario como texto (`horario.md`).
    @State private var horarioComoTexto = false
    @ObservedObject private var calendario = CalendarioDelIPad.compartido
    @Environment(\.horizontalSizeClass) private var clase
    @FocusState private var enLaCaja: Bool

    public init(almacen: Almacen) {
        _modelo = StateObject(wrappedValue: ModeloTareas(nucleo: almacen.nucleo))
        _semana = StateObject(wrappedValue: ModeloSemana(nucleo: almacen.nucleo))
    }

    /// (En minusculas: el iPad guardaba «Kanban» con mayuscula.)
    private var vista: VistaDeTareas { vistaTareasDeTexto(texto: vistaGuardada.lowercased()) }

    private func cambiarVista(_ v: VistaDeTareas) {
        if v == .horario, vista != .horario { semana.abrir() }
        if v != .horario, vista == .horario {
            // Pudo nacer una asignatura en el Horario: para apuntar en ella.
            _ = Task<Void, Never> { asignaturas = await ModeloTareas.asignaturasFuera(de: modelo) }
        }
        vistaGuardada = vistaTareasComoTexto(vista: v)
    }

    private var eleccion: Binding<VistaDeTareas> {
        Binding(get: { vista }, set: { cambiarVista($0) })
    }

    /// La vista pedida desde fuera (el mosaico Horario de Funciones, el
    /// pedido `ventana horario`).
    private func irAVista(_ texto: String?) {
        guard let texto else { return }
        VistaTareas.vistaPedida = nil
        cambiarVista(vistaTareasDeTexto(texto: texto))
    }

    public var body: some View {
        VStack(spacing: 0) {
            cabecera
            if vista == .horario {
                VistaHorarioIPad(modelo: semana, tareas: modelo, dias: conDomingo ? 7 : 6,
                                 alPulsarTarea: { t in
                                     // La chapita: esa tarea en las tarjetas.
                                     modelo.busqueda = ""
                                     resaltada = t.id
                                     cambiarVista(.tarjetas)
                                 },
                                 abrirEnTareas: { nombre in
                                     // Sus tarjetas, y lo apuntado va a su lista.
                                     destino = nombre
                                     modelo.busqueda = nombre
                                     cambiarVista(.tarjetas)
                                 })
            } else {
                caja
                if let quedara = ComoQuedaraTarea.vista(nueva) {
                    // Mientras se escribe, como quedara (lo entiende el nucleo).
                    Text(quedara)
                        .font(.system(size: 12.5))
                        .foregroundStyle(Paleta.v2.gris)
                        .lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.leading, 46).padding(.trailing, 16).padding(.bottom, 8)
                        .background(Paleta.v2.fondo)
                        .accessibilityLabel("Quedará así: \(quedara)")
                }
                Paleta.v2.linea.frame(height: 1)
                Group {
                    switch vista {
                    case .kanban: TableroKanban(modelo: modelo, acciones: acciones)
                    default: RejillaTarjetas(modelo: modelo, resaltada: $resaltada, acciones: acciones)
                    }
                }
                .background(Paleta.v2.fondo)
                .refreshable { await modelo.cargar() }
                .overlay {
                    if modelo.cargado, modelo.listas.allSatisfy({ $0.tareas.isEmpty }) {
                        ContentUnavailableView("Sin tareas", systemImage: "checklist",
                                               description: Text("Las listas de tareas de tus chats aparecen aquí."))
                    }
                }
            }
        }
        .ventanaConCabecera()
        .background {
            // ⌃⌘T, ⌃⌘K y ⌃⌘H: las vistas (el Ctrl+T/K/H de Windows).
            ForEach(VistaDeTareas.todas, id: \.self) { v in
                Button(v.rotulo) { cambiarVista(v) }
                    .keyboardShortcut(KeyEquivalent(v.letraAtajo), modifiers: [.control, .command])
                    .hidden()
            }
        }
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
        .onAppear { irAVista(VistaTareas.vistaPedida?.rawValue) }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinAbrirVistaTareas)) { irAVista($0.object as? String) }
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
        .sheet(isPresented: $horarioComoTexto) {
            HojaTextoHorario(inicial: semana.texto() ?? "") { semana.escribir($0) }
        }
    }

    /// La barra comun de Windows (`Cabecera`): el conmutador de vistas y lo
    /// propio de cada una (releer; en el Horario, «Domingo», la nota de voz
    /// de la proxima clase, el calendario del iPad y «Como texto»).
    private var cabecera: some View {
        let compacta = clase == .compact
        return Cabecera("Tareas", subtitulo: subtitulo,
                        buscador: vista == .horario ? nil
                            : BuscadorCabecera($modelo.busqueda, pista: "Buscar: texto o #etiqueta")) {
            SegmentosCabecera(VistaDeTareas.todas.map { ($0, $0.rotulo) }, eleccion: eleccion)
                .help(VistaDeTareas.todas.map(\.pista).joined(separator: "\n"))
            if vista == .horario {
                BotonCabecera(icono: "calendar", rotulo: compacta ? "" : "Domingo",
                              ayuda: conDomingo ? "Quitar el domingo" : "Ver el domingo", activo: conDomingo) {
                    conDomingo.toggle()
                }
                BotonCabecera(icono: "mic", rotulo: "", ayuda: "Nota de voz para la próxima clase") {
                    HorarioIPad.grabarParaLaProxima(nucleo: semana.nucleo)
                }
                BotonCabecera(icono: "calendar.badge.clock", rotulo: "",
                              ayuda: calendario.activo ? "Dejar de avisar de los eventos del calendario del iPad"
                                  : "Avisar también de los eventos del calendario del iPad",
                              activo: calendario.activo) {
                    _ = Task<Void, Never> {
                        await calendario.alternar()
                        semana.releer()
                    }
                }
                BotonCabecera(icono: "pencil", rotulo: compacta ? "" : "Como texto",
                              ayuda: "Editar el horario como texto") { horarioComoTexto = true }
            } else {
                BotonCabecera(icono: "arrow.clockwise", ayuda: "Releer (⌘R)") { Task { await modelo.cargar() } }
                    .keyboardShortcut("r")
            }
        }
    }

    private var subtitulo: String {
        if vista == .horario { return semana.subtitulo }
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
            verEnChat: { Navegacion.compartida.irAlChat(t.lista.chat, mensaje: t.lista.mensaje) },
            pinear: { PinesDeTareasIPad.compartidos.abrir(t, nucleo: modelo.nucleoParaLeer) }
        )
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
    /// «📌 Fijar como pin»: la tarea sola, encima de todo.
    var pinear: () -> Void = {}
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
        if !t.tarea.hecha {
            Button { acciones.pinear() } label: { Label("Fijar como pin", systemImage: "pin") }
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

// MARK: - Tarjetas

/// **Las tarjetas** (tareas-v4 de Windows): cada lista con su encabezado y
/// lo pendiente en cuadrados (una foto de la tarea, de fondo; con varias,
/// los bordes de las de detras), y lo hecho plegado en filas: un cuadrado
/// entero para algo ya tachado pesaba tanto como lo pendiente.
struct RejillaTarjetas: View {
    @ObservedObject var modelo: ModeloTareas
    @Binding var resaltada: String?
    let acciones: (TareaConLista) -> AccionesTarea
    /// Las listas con lo hecho abierto.
    @State private var abiertas: Set<String> = []

    var body: some View {
        ScrollViewReader { lector in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 18) {
                    ForEach(modelo.listasFiltradas, id: \.mensaje) { l in grupo(l) }
                }
                .padding(12)
            }
            .onChange(of: resaltada) { _, id in
                guard let id else { return }
                withAnimation { lector.scrollTo(id, anchor: .center) }
            }
            .onAppear {
                if let id = resaltada {
                    DispatchQueue.main.async { lector.scrollTo(id, anchor: .center) }
                }
            }
        }
    }

    private func grupo(_ l: ListaTareas) -> some View {
        let pendientes = l.tareas.filter { !$0.hecha }
        let hechas = l.tareas.filter(\.hecha)
        let clave = "\(l.chat)/\(l.mensaje)"
        return VStack(alignment: .leading, spacing: 10) {
            CabeceraLista(l: l).font(.callout)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 200, maximum: 260), spacing: 12, alignment: .top)],
                      alignment: .leading, spacing: 12) {
                ForEach(pendientes, id: \.indice) { t in
                    let x = TareaConLista(lista: l, tarea: t)
                    TarjetaTarea(t: x, modelo: modelo, acciones: acciones(x), resaltada: resaltada == x.id)
                        .id(x.id)
                }
            }
            if !hechas.isEmpty {
                DisclosureGroup(isExpanded: Binding(
                    get: { abiertas.contains(clave) || !modelo.busqueda.isEmpty },
                    set: { if $0 { abiertas.insert(clave) } else { abiertas.remove(clave) } })) {
                    ForEach(hechas, id: \.indice) { t in
                        let x = TareaConLista(lista: l, tarea: t)
                        FilaTarea(t: x, conLista: false, listas: modelo.listas, acciones: acciones(x))
                            .padding(.vertical, 2)
                            .id(x.id)
                    }
                } label: {
                    Text("Hechas (\(hechas.count))").font(.caption).foregroundStyle(Paleta.v2.gris)
                }
            }
        }
    }
}

/// **Una tarjeta**: la fila de la tarea en un cuadrado, del alto de lo suyo
/// (entre la mitad del lado y el lado), con su primera foto de fondo.
/// Se arrastra a otra columna del Kanban.
struct TarjetaTarea: View {
    let t: TareaConLista
    @ObservedObject var modelo: ModeloTareas
    let acciones: AccionesTarea
    var resaltada = false
    /// El lado de la tarjeta (`LADO_CUADRADO` de Windows).
    static let lado: CGFloat = 200

    var body: some View {
        let fotos = t.tarea.imagenes
        VStack(alignment: .leading, spacing: 0) {
            FilaTarea(t: t, conLista: false, listas: modelo.listas, acciones: acciones)
            Spacer(minLength: 0)
        }
        .padding(10)
        .frame(maxWidth: .infinity, minHeight: fotos.isEmpty ? Self.lado / 2 : Self.lado,
               maxHeight: Self.lado, alignment: .topLeading)
        .background {
            if let primera = fotos.first {
                FotoDeTarjeta(ruta: primera)
                    .overlay(LinearGradient(colors: [.black.opacity(0.75), .black.opacity(0.25)],
                                            startPoint: .top, endPoint: .bottom))
            } else {
                Paleta.v2.tarjeta
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .overlay {
            RoundedRectangle(cornerRadius: 12)
                .strokeBorder(resaltada ? Paleta.v2.acento : Color.white.opacity(0.08), lineWidth: resaltada ? 2 : 1)
        }
        // Con varias fotos, los bordes de detras: uno con dos, dos con mas.
        .background(alignment: .topTrailing) {
            ForEach(0 ..< min(2, max(0, fotos.count - 1)), id: \.self) { k in
                RoundedRectangle(cornerRadius: 12)
                    .strokeBorder(Color.white.opacity(0.25 - Double(k) * 0.08), lineWidth: 1)
                    .background(Paleta.v2.tarjeta.opacity(0.6), in: RoundedRectangle(cornerRadius: 12))
                    .offset(x: CGFloat(k + 1) * 4, y: -CGFloat(k + 1) * 4)
            }
        }
        .contentShape(.dragPreview, RoundedRectangle(cornerRadius: 12))
        .draggable(t.id) {
            Text(t.tarea.texto).padding(8)
                .background(Paleta.v2.tarjetaEncima, in: RoundedRectangle(cornerRadius: 8))
        }
    }
}

/// La foto de fondo de una tarjeta, leida fuera del hilo principal.
struct FotoDeTarjeta: View {
    let ruta: String
    @State private var imagen: UIImage?

    var body: some View {
        GeometryReader { geo in
            if let imagen {
                Image(uiImage: imagen).resizable().scaledToFill()
                    .frame(width: geo.size.width, height: geo.size.height).clipped()
            } else {
                Paleta.v2.tarjeta
            }
        }
        .task(id: ruta) {
            let ruta = ruta
            // Reducida al tamano de la tarjeta: no hace falta la foto entera.
            let img = await Task.detached(priority: .utility) { () -> UIImage? in
                guard let datos = FileManager.default.contents(atPath: ruta), let u = UIImage(data: datos) else {
                    return nil
                }
                return u.preparingThumbnail(of: CGSize(width: 520, height: 520)) ?? u
            }.value
            imagen = img
        }
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
            // Las mismas tarjetas que la vista Tarjetas (con su foto).
            ForEach(l.tareas.filter { !$0.hecha }, id: \.indice) { t in
                let x = TareaConLista(lista: l, tarea: t)
                TarjetaTarea(t: x, modelo: modelo, acciones: acciones(x))
            }
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

// MARK: - Como quedara

/// **Mientras se escribe una tarea, como quedara** («Urgente · #casa · ⏰
/// 18:00»): lo que guardaria el nucleo (`tareaEntendida`) leido con sus
/// marcas, como lo enseña Windows bajo la caja de apuntar.
enum ComoQuedaraTarea {
    /// `nil` si no hay nada que enseñar (vacio, o se queda tal cual).
    static func vista(_ escrito: String, ahora: Date = Date()) -> String? {
        let t = escrito.trimmingCharacters(in: .whitespaces)
        guard !t.isEmpty else { return nil }
        let d = desfaseLocalMs(ahora)
        let m = marcasDeTarea(texto: tareaEntendida(texto: t, desfaseMs: d), desfaseMs: d)
        return vista(titulo: m.titulo, urgencia: m.urgencia, etiquetas: m.etiquetas,
                     hora: m.horaMs.map { AtajosDeHora.legible(fechaDeLocales($0), ahora: ahora) },
                     escrito: t)
    }

    /// Lo mismo con las marcas ya leidas (lo puro, para las pruebas).
    static func vista(titulo: String, urgencia: String?, etiquetas: [String], hora: String?, escrito: String) -> String? {
        var partes: [String] = []
        if let u = urgencia { partes.append(u == "muy_urgente" ? "Muy urgente" : "Urgente") }
        partes.append(contentsOf: etiquetas.map { "#\($0)" })
        if let hora { partes.append("⏰ \(hora)") }
        // Sin marcas que entender no se dice nada: se ve en la caja.
        guard !partes.isEmpty else { return nil }
        let limpio = titulo.trimmingCharacters(in: .whitespaces)
        return ([limpio.isEmpty ? escrito : limpio] + partes).joined(separator: " · ")
    }
}
