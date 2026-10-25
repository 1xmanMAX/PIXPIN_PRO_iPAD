// **El menu de un mensaje del chat**, el de Windows (`menu_de_mensaje` de
// `ventana_chat.rs`, la v2 de los menus) con su orden, traido del Mac
// (`Chat/MenuMensaje.swift`) al iPad: sale al **mantener pulsada** una
// burbuja (o con el clic secundario del trackpad).
//
// - encima, la fila de etiquetas (⭐ ✅ ⏳ 💡 💰 📍): pulsar la puesta la
//   quita;
// - lo diario: Responder, Copiar, Pinear, Añadir o editar la descripcion,
//   Abrir en el editor de notas y lo propio de cada burbuja (Pasar a texto
//   en una nota de voz, Abrir...);
// - mandarlo a otro sitio y acordarse: Reenviar (a que proyecto), Compartir…
//   (la hoja del sistema) y Recordarmelo (o Quitar el recordatorio);
// - «Mas»: Cambiar el nombre, Fijar arriba, Guardar esto (lo del buzon),
//   Ver comentarios y Elegir varios;
// - Borrar, en rojo y aparte, preguntando antes.
//
// Lo que se escribe en el cuaderno pasa por el nucleo
// (`Nucleo.cambiarMensaje`, `reenviarMensajes`, `borrarMensajes`): los mismos
// campos que Windows y el movil (`fijado`, `emoji`, `texto`, `nombre`,
// `recuerdaEn`, `enBuzon`, `respondeA`), para que viajen igual.

import PixPinCore
import SwiftUI
import UIKit
import UserNotifications

/// Lo que otras partes de la app ponen para el chat sin depender de el (si
/// no lo ponen, la entrada no sale).
@MainActor
public enum GanchosChat {
    /// «Pinear» un fichero del chat (lo pone la capa de pines).
    public static var pinearFichero: ((URL, String) -> Void)?
    /// «Pinear» una lista de tareas viva (chat, mensaje, documento).
    public static var pinearTareas: ((String, String, String) -> Void)?
    /// «Hacer lección» con un mensaje (chat, mensaje).
    public static var hacerLeccion: ((String, MensajeChat, Almacen) -> Void)?
}

/// Lo comun del menu que no es una vista.
enum MenuMensaje {
    /// Las etiquetas del movil y de Windows, en su orden
    /// (`chats::ETIQUETAS` del nucleo).
    static let etiquetas = ["\u{2B50}", "\u{2705}", "\u{23F3}", "\u{1F4A1}", "\u{1F4B0}", "\u{1F4CD}"]

    /// Si lo escribio el asistente (su codigo acaba en `·IA`, como Windows).
    static func esDelAsistente(_ m: MensajeChat) -> Bool {
        m.codigoChat?.hasSuffix("·IA") ?? false
    }
}

/// La pregunta antes de borrar, con los textos de Windows (`chat-borrar`,
/// `chat-borrar-aviso`).
enum ConfirmarBorrar {
    static let aviso = "Se quitará de la conversación. Un archivo adjunto se borra con él. ¿Seguir?"

    /// El titulo con cuantos son, si es mas de uno.
    static func titulo(cuantos: Int) -> String {
        cuantos > 1 ? "Borrar \(cuantos) mensajes" : "Borrar"
    }
}

// MARK: - Lo elegido

/// **Los mensajes elegidos de un chat** y como cambian (puro, sin vistas:
/// se prueba solo).
struct EleccionChat: Equatable {
    private(set) var ids: Set<String> = []
    /// El ultimo tocado: desde el sale el tramo.
    private(set) var ancla: String?

    var vacia: Bool { ids.isEmpty }

    /// Un toque con alguno elegido (o «Elegir varios»): lo pone o lo quita.
    mutating func alternar(_ id: String) {
        if ids.remove(id) == nil { ids.insert(id) }
        ancla = id
    }

    /// ⇧-toque (con teclado): todo lo de entre el ancla y `id` (en el orden
    /// de `orden`). Sin ancla que se vea, solo `id`.
    mutating func tramo(hasta id: String, en orden: [String]) {
        guard let ancla, let a = orden.firstIndex(of: ancla), let b = orden.firstIndex(of: id) else {
            alternar(id)
            return
        }
        ids.formUnion(orden[min(a, b)...max(a, b)])
        self.ancla = id
    }

    /// Los elegidos en el orden del chat (los que ya no estan, fuera).
    func enOrden(_ orden: [String]) -> [String] {
        orden.filter { ids.contains($0) }
    }

    /// Olvida los que ya no estan cargados (borrados, otra pagina).
    mutating func quedarse(con presentes: Set<String>) {
        ids.formIntersection(presentes)
        if let a = ancla, !presentes.contains(a) { ancla = nil }
    }

    mutating func vaciar() {
        ids = []
        ancla = nil
    }
}

/// Lo que se esta escribiendo en la hoja de una linea: la descripcion, el
/// nombre o la hora de avisar (la barra de Windows encima de la caja).
enum EscrituraMensaje: Identifiable, Equatable {
    case describir(MensajeChat)
    case renombrar(MensajeChat)
    case hora(MensajeChat)

    var id: String {
        switch self {
        case let .describir(m): "d" + m.id
        case let .renombrar(m): "r" + m.id
        case let .hora(m): "h" + m.id
        }
    }
}

// MARK: - El estado del chat abierto

/// **El estado del chat abierto que toca el menu**: a quien se responde,
/// la hoja de escribir, el aviso de abajo y las acciones que escriben.
@MainActor
final class ChatAbierto: ObservableObject {
    let chat: String
    let almacen: Almacen
    /// El mensaje al que se contesta (la barra encima de la caja).
    @Published var respondiendo: String?
    @Published var escritura: EscrituraMensaje?
    /// Lo que se dice un momento abajo («Copiado», «Reenviado»...).
    @Published private(set) var aviso: String?
    /// Los mensajes elegidos («Elegir varios»).
    @Published var eleccion = EleccionChat()
    /// Lo que se va a compartir (abre la hoja del sistema).
    @Published var compartiendo: CosasACompartir?
    /// Los que se van a borrar, mientras se pregunta.
    @Published var aBorrar: [String]?
    private var turnoAviso = 0

    /// Saltar a un mensaje cargado (lo pone `VistaChat`).
    var irA: (String) -> Void = { _ in }

    init(chat: String, almacen: Almacen) {
        self.chat = chat
        self.almacen = almacen
    }

    var cargados: [MensajeChat] { almacen.mensajes[chat] ?? [] }

    /// Los elegidos en el orden del chat.
    func elegidos() -> [String] { eleccion.enOrden(cargados.map(\.id)) }

    func avisar(_ texto: String) {
        aviso = texto
        turnoAviso += 1
        let turno = turnoAviso
        Task {
            try? await Task.sleep(nanoseconds: 2_200_000_000)
            if turno == turnoAviso { aviso = nil }
        }
    }

    /// Los ids sobre los que actua una entrada: si el mensaje esta entre
    /// los elegidos, todos los elegidos (como en Windows); si no, el.
    func sobre(_ m: MensajeChat) -> [String] {
        let todos = elegidos()
        return todos.contains(m.id) ? todos : [m.id]
    }

    /// El fichero del mensaje en este iPad, si ya se resolvio y esta.
    func fichero(_ m: MensajeChat) -> URL? {
        guard let ruta = m.ruta, !ruta.isEmpty,
              let url = almacen.urlResuelta(deRuta: ruta, en: chat),
              FileManager.default.fileExists(atPath: url.path) else { return nil }
        return url
    }

    // MARK: Lo diario

    func responder(_ m: MensajeChat) {
        respondiendo = m.id
    }

    /// Copia el texto (sin espacios de los lados), como `Accion::Copiar`;
    /// una foto sola, la foto.
    func copiar(_ ids: [String]) {
        let mensajes = cargados.filter { ids.contains($0.id) }
        let texto = ChatAbierto.textoParaCopiar(mensajes)
        if texto.isEmpty, mensajes.count == 1, mensajes[0].clase == "IMAGEN",
           let url = fichero(mensajes[0]), let foto = UIImage(contentsOfFile: url.path) {
            UIPasteboard.general.image = foto
            avisar("Copiado")
            return
        }
        guard !texto.isEmpty else { return }
        UIPasteboard.general.string = texto
        avisar("Copiado")
        if ids.count > 1 { eleccion.vaciar() }
    }

    /// El texto de unos mensajes para el portapapeles: el de cada uno sin
    /// espacios a los lados, sin las mini-apps (su texto es un JSON) y
    /// separados por una linea en blanco (`copiar_marcados` de Windows).
    nonisolated static func textoParaCopiar(_ mensajes: [MensajeChat]) -> String {
        mensajes
            .filter { $0.clase != "MINIAPP" }
            .map { $0.texto.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .joined(separator: "\n\n")
    }

    /// Si «Copiar» sale para este mensaje.
    nonisolated static func sePuedeCopiar(_ m: MensajeChat) -> Bool {
        if m.clase == "IMAGEN" { return true }
        return m.clase != "MINIAPP" && !m.texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    /// Una lista de tareas: no tiene fichero, pero se pinea viva.
    nonisolated static func esListaDeTareas(_ m: MensajeChat) -> Bool {
        m.clase == "MINIAPP" && m.miniapp == "tareas"
    }

    /// Si «Pinear» sale (hace falta quien sepa pinear).
    func sePuedePinear(_ m: MensajeChat) -> Bool {
        if ChatAbierto.esListaDeTareas(m) { return GanchosChat.pinearTareas != nil }
        return m.ruta?.isEmpty == false && GanchosChat.pinearFichero != nil
    }

    /// Sacarlo a la capa de pines: el fichero en un pin; una lista de
    /// tareas, la lista viva.
    func pinear(_ m: MensajeChat) {
        if ChatAbierto.esListaDeTareas(m), let pinear = GanchosChat.pinearTareas {
            pinear(chat, m.id, m.texto)
            avisar("Pineado")
            return
        }
        guard let url = fichero(m) else {
            avisar("Este mensaje no tiene un archivo en este iPad")
            return
        }
        GanchosChat.pinearFichero?(url, m.nombre.isEmpty ? url.lastPathComponent : m.nombre)
        avisar("Pineado")
    }

    // MARK: Lo que cambia el mensaje

    func cambiar(_ ids: [String], _ cambio: CambioMensaje, aviso: String? = nil) {
        let almacen = self.almacen
        let chat = self.chat
        Task {
            var bien = true
            for id in ids where await almacen.cambiar(mensaje: id, en: chat, cambio) == nil {
                bien = false
            }
            if let aviso, bien { avisar(aviso) }
        }
    }

    /// Una etiqueta de la fila: la que ya estaba puesta, otra vez, se quita.
    func etiquetar(_ m: MensajeChat, _ emoji: String) {
        cambiar([m.id], .etiqueta(emoji: m.emoji == emoji ? nil : emoji))
    }

    /// Fija lo elegido; si ya estaba todo fijado, lo suelta (`fijar_marcados`).
    func fijar(_ ids: [String]) {
        let mensajes = cargados.filter { ids.contains($0.id) }
        let fijar = mensajes.contains { !$0.fijado }
        let cambiar = mensajes.filter { $0.fijado != fijar }.map(\.id)
        self.cambiar(cambiar, .fijar(fijado: fijar))
        if ids.count > 1 { eleccion.vaciar() }
    }

    func recordar(_ m: MensajeChat, en cuando: Date) {
        let ms = Int64((cuando.timeIntervalSince1970 * 1000).rounded())
        let r = m.resumen.trimmingCharacters(in: .whitespacesAndNewlines)
        let texto = r.isEmpty ? (m.nombre.isEmpty ? "Recordatorio" : m.nombre) : r
        let almacen = self.almacen
        let chat = self.chat
        Task {
            guard await almacen.cambiar(mensaje: m.id, en: chat, .recordar(cuando: ms)) != nil else { return }
            ChatAbierto.programarAviso(chat: chat, id: m.id, texto: texto, cuando: cuando)
            avisar("Te aviso a las \(AtajosDeHora.legible(cuando, ahora: Date()))")
        }
    }

    func olvidar(_ m: MensajeChat) {
        let almacen = self.almacen
        let chat = self.chat
        Task {
            guard await almacen.cambiar(mensaje: m.id, en: chat, .recordar(cuando: nil)) != nil else { return }
            UNUserNotificationCenter.current().removePendingNotificationRequests(
                withIdentifiers: [AvisosRecordatorio.identificador(chat: chat, id: m.id)])
            avisar("Recordatorio quitado")
        }
    }

    /// El aviso del sistema a su hora (el mismo id que usa el vigia de
    /// recordatorios, asi no se duplica).
    static func programarAviso(chat: String, id: String, texto: String, cuando: Date) {
        guard cuando > Date() else { return }
        let centro = UNUserNotificationCenter.current()
        centro.requestAuthorization(options: [.alert, .sound]) { _, _ in }
        let contenido = UNMutableNotificationContent()
        contenido.title = "Recordatorio"
        contenido.body = texto
        contenido.sound = .default
        contenido.userInfo = [AvisosRecordatorio.claveChat: chat, AvisosRecordatorio.claveId: id]
        let partes = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute, .second], from: cuando)
        let peticion = UNNotificationRequest(identifier: AvisosRecordatorio.identificador(chat: chat, id: id),
                                             content: contenido,
                                             trigger: UNCalendarNotificationTrigger(dateMatching: partes, repeats: false))
        centro.add(peticion) { _ in }
    }

    /// Lo escrito en la hoja: descripcion o nombre.
    func guardar(_ e: EscrituraMensaje, _ texto: String) {
        switch e {
        case let .describir(m):
            cambiar([m.id], .descripcion(texto: texto))
        case let .renombrar(m):
            let limpio = texto.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !limpio.isEmpty else { return }
            // Un audio se escribe sin su «.m4a» y el nucleo se la vuelve a
            // poner (`con_su_extension`).
            cambiar([m.id], .nombre(nombre: limpio))
        case .hora:
            break
        }
    }

    /// Lo que la hoja de cambiar el nombre trae escrito: el nombre, sin el
    /// «.m4a» de un audio; un lienzo sin nombre, «Lienzo».
    nonisolated static func nombreParaEscribir(_ m: MensajeChat) -> String {
        let n = m.nombre.trimmingCharacters(in: .whitespaces)
        if m.clase != "DIBUJO", !n.isEmpty {
            return n.hasSuffix(".m4a") ? String(n.dropLast(4)) : n
        }
        return n.isEmpty ? (m.clase == "DIBUJO" ? "Lienzo" : m.resumen) : n
    }

    // MARK: Mandarlo a otro sitio

    /// Los demas proyectos: todos menos este («Mensajes guardados» tambien).
    var otrosChats: [FichaChat] { almacen.chats.filter { $0.id != chat && !$0.archivado } }

    func reenviar(_ ids: [String], a destino: String) {
        let almacen = self.almacen
        let chat = self.chat
        Task {
            let n = await almacen.reenviar(mensajes: ids, de: chat, a: destino)
            if n > 0 {
                eleccion.vaciar()
                avisar("Reenviado")
            }
        }
    }

    /// «Compartir…»: la hoja del sistema con el fichero de cada mensaje (la
    /// foto, el PDF, la voz) o su texto (`compartir_mensajes` de Windows).
    func compartir(_ ids: [String]) {
        let mensajes = cargados.filter { ids.contains($0.id) }
        guard !mensajes.isEmpty else { return }
        let almacen = self.almacen
        let chat = self.chat
        Task {
            var cosas: [Any] = []
            for m in mensajes {
                if let u = await resolverFichero(m, chat, almacen), FileManager.default.fileExists(atPath: u.path) {
                    cosas.append(u)
                } else {
                    let t = m.texto.trimmingCharacters(in: .whitespacesAndNewlines)
                    if !t.isEmpty, m.clase != "MINIAPP" { cosas.append(t) }
                }
            }
            guard !cosas.isEmpty else {
                avisar("No hay nada que compartir en este iPad")
                return
            }
            compartiendo = CosasACompartir(cosas: cosas)
        }
    }

    /// «Ver comentarios»: los que le responden.
    func respuestas(a m: MensajeChat) -> [MensajeChat] {
        cargados.filter { $0.respondeA == m.id }
    }

    /// Pide confirmacion para borrar (la pregunta sale en `VistaChat`).
    func pedirBorrar(_ ids: [String]) {
        guard !ids.isEmpty else { return }
        aBorrar = ids
    }
}

// MARK: - Las entradas

/// Las entradas del menu de un mensaje. `propias` son las de la burbuja
/// (Abrir, Pasar a texto...): van con lo diario.
struct MenuDeMensaje<Propias: View>: View {
    let m: MensajeChat
    @ObservedObject var c: ChatAbierto
    let propias: Propias

    init(m: MensajeChat, c: ChatAbierto, @ViewBuilder propias: () -> Propias) {
        self.m = m
        self.c = c
        self.propias = propias()
    }

    private var marcado: Bool { c.eleccion.ids.contains(m.id) }

    var body: some View {
        // ── La fila de etiquetas ──
        ControlGroup {
            ForEach(MenuMensaje.etiquetas, id: \.self) { e in
                Button { c.etiquetar(m, e) } label: {
                    Text(m.emoji == e ? "\(e)✓" : e)
                }
            }
        }
        .controlGroupStyle(.compactMenu)

        // ── Lo diario ──
        Section {
            Button { c.responder(m) } label: { Label("Responder", systemImage: "arrowshape.turn.up.left") }
            if ChatAbierto.sePuedeCopiar(m) {
                Button { c.copiar([m.id]) } label: { Label("Copiar", systemImage: "doc.on.doc") }
            }
            if c.sePuedePinear(m) {
                Button { c.pinear(m) } label: { Label("Pinear", systemImage: "pin") }
            }
            if m.sePuedeDescribir {
                Button { c.escritura = .describir(m) } label: {
                    Label(m.texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        ? "Añadir descripción" : "Editar descripción", systemImage: "captions.bubble")
                }
            }
            if m.clase == "NOTA" {
                Button {
                    Navegacion.compartida.abrir(.nota(chat: c.chat, mensaje: m))
                } label: { Label("Abrir en el editor de notas", systemImage: "square.and.pencil") }
            }
            propias
        }

        // ── Mandarlo a otro sitio y acordarse ──
        Section {
            Menu {
                ForEach(c.otrosChats, id: \.id) { f in
                    Button { c.reenviar(c.sobre(m), a: f.id) } label: {
                        Label(f.nombre, systemImage: f.esGuardados ? "bookmark" : "folder")
                    }
                }
            } label: {
                Label("Reenviar", systemImage: "arrowshape.turn.up.right")
            }
            .disabled(c.otrosChats.isEmpty)
            Button { c.compartir(c.sobre(m)) } label: { Label("Compartir…", systemImage: "square.and.arrow.up") }
            if m.recuerdaEn != nil {
                Button { c.olvidar(m) } label: { Label("Quitar el recordatorio", systemImage: "alarm") }
            } else {
                Menu {
                    ForEach(AtajosDeHora.atajos(ahora: Date()), id: \.titulo) { h in
                        Button { c.recordar(m, en: h.cuando) } label: { Label(h.titulo, systemImage: "alarm") }
                    }
                    Button { c.escritura = .hora(m) } label: { Label("Elegir la hora…", systemImage: "pencil") }
                } label: {
                    Label("Recordármelo", systemImage: "alarm")
                }
            }
            if !m.esLeccion, m.clase != "ARCHIVO", let leccion = GanchosChat.hacerLeccion {
                Button { leccion(c.chat, m, c.almacen) } label: { Label("Hacer lección", systemImage: "lightbulb") }
            }
        }

        // ── «Más»: lo que se usa poco ──
        Section {
            Menu {
                if m.sePuedeRenombrar {
                    Button { c.escritura = .renombrar(m) } label: { Label("Cambiar el nombre", systemImage: "pencil") }
                }
                Button { c.fijar([m.id]) } label: {
                    Label(m.fijado ? "Quitar de arriba" : "Fijar arriba", systemImage: m.fijado ? "flag.slash" : "flag")
                }
                if m.enBuzon {
                    Button { c.cambiar([m.id], .rescatar, aviso: "Guardado") } label: {
                        Label("Guardar esto", systemImage: "bookmark")
                    }
                }
                let respuestas = c.respuestas(a: m)
                if !respuestas.isEmpty {
                    Menu {
                        ForEach(respuestas, id: \.id) { r in
                            Button { c.irA(r.id) } label: {
                                Label(String(r.resumen.prefix(80)), systemImage: "arrowshape.turn.up.left")
                            }
                        }
                    } label: {
                        Label("Ver comentarios", systemImage: "bubble.left.and.bubble.right")
                    }
                }
                Divider()
                Button { c.eleccion.alternar(m.id) } label: {
                    Label(marcado ? "Quitar de los elegidos" : "Elegir varios", systemImage: "checkmark.circle")
                }
            } label: {
                Label("Más", systemImage: "ellipsis.circle")
            }
        }

        // ── Borrar, en rojo y aparte ──
        Section {
            Button(role: .destructive) { c.pedirBorrar(c.sobre(m)) } label: { Label("Borrar", systemImage: "trash") }
        }
    }
}

// MARK: - La hoja de escribir una linea

/// Escribir la descripcion o el nombre (una linea, Intro guarda), o elegir
/// la hora de avisar (la barra de Windows).
struct HojaEscritura: View {
    let escritura: EscrituraMensaje
    let guardar: (String) -> Void
    let recordar: (MensajeChat, Date) -> Void
    @Environment(\.dismiss) private var cerrar
    @State private var texto = ""
    @State private var hora = Date().addingTimeInterval(3600)
    @FocusState private var enfocado: Bool

    private var titulo: String {
        switch escritura {
        case let .describir(m):
            m.texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "Añadir descripción" : "Editar descripción"
        case .renombrar: "Cambiar el nombre"
        case .hora: "Avisar a las"
        }
    }

    private var vale: Bool {
        switch escritura {
        case .renombrar: !texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        case .hora: hora > Date()
        case .describir: true
        }
    }

    var body: some View {
        NavigationStack {
            Form {
                switch escritura {
                case .hora:
                    DatePicker("Hora", selection: $hora, in: Date()..., displayedComponents: [.date, .hourAndMinute])
                        .datePickerStyle(.graphical)
                case .describir, .renombrar:
                    TextField(escritura.esRenombrar ? "Nombre" : "Descripción", text: $texto, axis: .vertical)
                        .focused($enfocado)
                        .onSubmit(hecho)
                        .submitLabel(.done)
                }
            }
            .navigationTitle(titulo)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancelar") { cerrar() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Guardar", action: hecho).disabled(!vale)
                }
            }
        }
        .presentationDetents(escritura.esHora ? [.large] : [.medium])
        .onAppear {
            switch escritura {
            case let .describir(m): texto = m.texto.trimmingCharacters(in: .whitespacesAndNewlines)
            case let .renombrar(m): texto = ChatAbierto.nombreParaEscribir(m)
            case .hora: texto = ""
            }
            enfocado = true
        }
    }

    private func hecho() {
        guard vale else { return }
        if case let .hora(m) = escritura {
            recordar(m, hora)
        } else {
            guardar(texto)
        }
        cerrar()
    }
}

private extension EscrituraMensaje {
    var esHora: Bool {
        if case .hora = self { return true }
        return false
    }

    var esRenombrar: Bool {
        if case .renombrar = self { return true }
        return false
    }
}

// MARK: - Las piezas de la burbuja y del chat

/// A quien responde un mensaje, encima de su burbuja (la cita de Windows):
/// una raya de color y su resumen. Un toque lleva a el.
struct CitaDeMensaje: View {
    let citado: MensajeChat?
    let irA: () -> Void

    var body: some View {
        HStack(spacing: 6) {
            RoundedRectangle(cornerRadius: 1.5).fill(Paleta.chat.enviar).frame(width: 3)
            Text(citado.map { $0.resumen.isEmpty ? $0.nombre : $0.resumen } ?? "Mensaje que ya no está")
                .font(.caption).foregroundStyle(.secondary).lineLimit(2)
        }
        .padding(.vertical, 2)
        .frame(maxWidth: 320, alignment: .leading)
        .contentShape(Rectangle())
        .onTapGesture { if citado != nil { irA() } }
        .accessibilityLabel("Ir al mensaje")
    }
}

/// Las marcas de la burbuja junto a la hora: fijado, recordatorio y su
/// etiqueta.
struct MarcasDeMensaje: View {
    let mensaje: MensajeChat

    var body: some View {
        HStack(spacing: 4) {
            if mensaje.fijado {
                Image(systemName: "flag.fill").accessibilityLabel("Fijado arriba")
            }
            if let ms = mensaje.recuerdaEn {
                let cuando = Date(timeIntervalSince1970: TimeInterval(ms) / 1000)
                Label(AtajosDeHora.legible(cuando, ahora: Date()), systemImage: "alarm")
                    .labelStyle(.titleAndIcon)
            }
            if let e = mensaje.emoji, !e.isEmpty {
                Text(e)
            }
        }
        .font(.caption2)
    }
}

/// La barra de encima de la caja mientras se responde: a quien, y su aspa
/// (Esc tambien la quita).
struct BarraDeRespuesta: View {
    let citado: MensajeChat
    let cancelar: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "arrowshape.turn.up.left").foregroundStyle(Paleta.chat.enviar)
            RoundedRectangle(cornerRadius: 1.5).fill(Paleta.chat.enviar).frame(width: 3, height: 26)
            VStack(alignment: .leading, spacing: 1) {
                Text("Respondiendo").font(.caption.weight(.medium)).foregroundStyle(Paleta.chat.enviar)
                Text(citado.resumen.isEmpty ? citado.nombre : citado.resumen)
                    .font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer()
            Button(action: cancelar) { Image(systemName: "xmark.circle.fill").font(.title3) }
                .buttonStyle(.borderless)
                .foregroundStyle(.secondary)
                .accessibilityLabel("Dejar de responder")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(Paleta.chat.isla, in: RoundedRectangle(cornerRadius: 16))
        .padding(.horizontal, MedidasIsla.margenX)
        .padding(.top, 6)
    }
}

/// «Mensaje fijado», arriba del chat: el ultimo fijado de lo cargado. Un
/// toque lleva a el.
struct BarraDeFijado: View {
    let fijado: MensajeChat
    let irA: () -> Void

    var body: some View {
        Button(action: irA) {
            HStack(spacing: 8) {
                RoundedRectangle(cornerRadius: 1.5).fill(Paleta.chat.enviar).frame(width: 3, height: 28)
                VStack(alignment: .leading, spacing: 1) {
                    Text("Mensaje fijado").font(.caption.weight(.medium)).foregroundStyle(Paleta.chat.enviar)
                    Text(fijado.resumen.isEmpty ? fijado.nombre : fijado.resumen)
                        .font(.caption).foregroundStyle(Paleta.proyectos.texto).lineLimit(1)
                }
                Spacer()
                Image(systemName: "flag.fill").foregroundStyle(Paleta.chat.enviar)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(Paleta.chat.cabecera)
    }
}

/// El aviso de un momento abajo del chat.
struct AvisoDelChat: View {
    let texto: String

    var body: some View {
        Text(texto)
            .font(.callout)
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(.regularMaterial, in: Capsule())
            .shadow(radius: 2)
            .padding(.bottom, 8)
            .transition(.opacity)
            .allowsHitTesting(false)
    }
}

/// La barra de los elegidos, encima de la caja de escribir: lo que hace la
/// de Windows con lo marcado (Copiar, Compartir, Fijar, Reenviar y Borrar).
struct BarraDeElegidos: View {
    @ObservedObject var c: ChatAbierto

    var body: some View {
        let ids = c.elegidos()
        HStack(spacing: 14) {
            Text(ids.count == 1 ? "1 elegido" : "\(ids.count) elegidos")
                .font(.callout.weight(.medium))
            Spacer()
            Button { c.copiar(ids) } label: { Label("Copiar", systemImage: "doc.on.doc") }
            Button { c.compartir(ids) } label: { Label("Compartir", systemImage: "square.and.arrow.up") }
            Button { c.fijar(ids) } label: { Label("Fijar", systemImage: "flag") }
            Menu {
                ForEach(c.otrosChats, id: \.id) { f in
                    Button(f.nombre) { c.reenviar(ids, a: f.id) }
                }
            } label: {
                Label("Reenviar", systemImage: "arrowshape.turn.up.right")
            }
            Button(role: .destructive) { c.pedirBorrar(ids) } label: {
                Label("Borrar", systemImage: "trash")
            }
            .foregroundStyle(.red)
            Button("Cancelar") { c.eleccion.vaciar() }
                .keyboardShortcut(.cancelAction)
        }
        .labelStyle(.iconOnly)
        .font(.system(size: 17))
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .background(Paleta.chat.isla, in: RoundedRectangle(cornerRadius: 16))
        .padding(.horizontal, MedidasIsla.margenX)
        .padding(.top, 6)
    }
}

// MARK: - En el almacen

extension Almacen {
    /// Escribe una nota que contesta a `respondeA` (Responder) y la anade al
    /// final de lo cargado, como `escribir(nota:en:)`.
    @discardableResult
    func escribir(nota: String, en chat: String, respondeA: String?) async -> MensajeChat? {
        guard let respondeA, !respondeA.isEmpty else { return await escribir(nota: nota, en: chat) }
        let texto = nota.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !texto.isEmpty else { return nil }
        let nucleo = self.nucleo
        do {
            let m = try await Task.detached(priority: .userInitiated) {
                try nucleo.escribirNota(chat: chat, texto: texto, respondeA: respondeA)
            }.value
            anadir([m], en: chat)
            return m
        } catch {
            ponerError("\(error)")
            return nil
        }
    }

    /// Cambia un mensaje desde su menu (`Nucleo.cambiarMensaje`) y lo pone
    /// al dia en lo cargado. `nil` si no salio o ya no esta.
    @discardableResult
    func cambiar(mensaje id: String, en chat: String, _ cambio: CambioMensaje) async -> MensajeChat? {
        let nucleo = self.nucleo
        let hecho = await Task.detached(priority: .userInitiated) { () -> Result<MensajeChat?, Error> in
            Result { try nucleo.cambiarMensaje(chat: chat, id: id, cambio: cambio) }
        }.value
        switch hecho {
        case let .success(m?):
            sustituir(m, en: chat)
            return m
        case .success(nil):
            return nil
        case let .failure(e):
            ponerError("No se pudo: \(e)")
            return nil
        }
    }

    /// Reenvia mensajes a otro chat (`Nucleo.reenviarMensajes`): cuantos.
    func reenviar(mensajes ids: [String], de chat: String, a destino: String) async -> UInt32 {
        let nucleo = self.nucleo
        let hecho = await Task.detached(priority: .userInitiated) { () -> Result<UInt32, Error> in
            Result { try nucleo.reenviarMensajes(chat: chat, ids: ids, destino: destino) }
        }.value
        switch hecho {
        case let .success(n):
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            await cargarChats()
            return n
        case let .failure(e):
            ponerError("No se pudo reenviar: \(e)")
            return 0
        }
    }

    /// Un mensaje recien creado, al final de lo cargado (o, si se veia una
    /// pagina del medio, el chat otra vez desde el final).
    func alFinal(_ m: MensajeChat, en chat: String) async {
        anadir([m], en: chat)
    }
}
