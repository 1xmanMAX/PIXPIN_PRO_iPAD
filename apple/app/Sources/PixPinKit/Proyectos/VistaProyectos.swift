// **La seccion Proyectos**: la lista de chats a la izquierda y el chat a la
// derecha, como la ventana de Proyectos de Windows (`ventana_chat.rs`, el
// chat de estilo Telegram) y el Mac (`Vistas.swift`, `ListaProyectos.swift`).
//
// - La lista: los botones redondos dorados (Galeria, Timeline, Tareas,
//   Sincronizar), el buscador en capsula, las filas de 62 con el avatar de
//   46 y sus iniciales (`Paleta.avatar`, `Paleta.iniciales`), la ultima linea
//   y los sin leer; abajo, los archivados, y el boton redondo de proyecto
//   nuevo (54) abajo a la derecha.
// - El chat, sobre su papel (`Chat/VistaChat.swift`).
// - Estrecho (el iPad en vertical con la barra lateral, Split View, Slide
//   Over): o la lista o el chat, con la flecha para volver, como el movil.
//
// El chat abierto es `Navegacion.compartida.chatAbierto`: Buscar, el
// timeline o un enlace lo cambian y aqui se ve (y `saltoAMensaje` lo
// atiende el chat).

import PixPinCore
import SwiftUI

public struct VistaProyectos: View {
    @ObservedObject var almacen: Almacen
    @ObservedObject private var nav = Navegacion.compartida
    /// El ancho de la lista (se recuerda; 260 a 540, como Windows).
    @AppStorage("proyectos.anchoLista") private var anchoLista: Double = 340
    @State private var anchoAlEmpezar: Double?
    /// No caben lista y chat: se ve como el movil.
    @State private var estrecho = false
    /// El proyecto que se va a borrar, mientras se pregunta.
    @State private var aBorrar: FichaChat?

    public init(almacen: Almacen) { self.almacen = almacen }

    public var body: some View {
        HStack(spacing: 0) {
            if !estrecho || nav.chatAbierto == nil {
                ListaProyectos(almacen: almacen, elegido: $nav.chatAbierto) { chat in
                    if !chat.esGuardados, !chat.esAsistente {
                        Button(role: .destructive) { aBorrar = chat } label: {
                            Label("Borrar", systemImage: "trash")
                        }
                    }
                }
                .frame(width: estrecho ? nil : anchoLista)
                .frame(maxWidth: estrecho ? .infinity : nil)
            }
            if !estrecho {
                Paleta.proyectos.separador
                    .frame(width: 1)
                    .overlay { asaLista }
            }
            if !estrecho || nav.chatAbierto != nil {
                ZStack {
                    PapelChat().ignoresSafeArea()
                    if let chat = nav.chatAbierto, almacen.chats.isEmpty || almacen.chats.contains(where: { $0.id == chat }) {
                        VistaChat(almacen: almacen, chat: chat,
                                  alVolver: estrecho ? { nav.chatAbierto = nil } : nil)
                            .id(chat)
                    } else {
                        PildoraDelPapel(texto: "Elige un proyecto")
                    }
                }
                .frame(maxWidth: .infinity)
            }
        }
        .onGeometryChange(for: Bool.self) { $0.size.width < anchoLista + 400 } action: { estrecho = $0 }
        // La franja de arriba de Windows: fina, del color de la barra, con
        // el boton de la barra lateral del sistema.
        .navigationTitle(nav.chatAbierto.flatMap { id in almacen.chats.first { $0.id == id }?.nombre } ?? "Proyectos")
        .navigationBarTitleDisplayMode(.inline)
        .toolbarBackground(Paleta.proyectos.barra, for: .navigationBar)
        .toolbarBackground(.visible, for: .navigationBar)
        // Como `confirmar_destructivo` de Windows: el borrado viaja.
        .confirmationDialog(aBorrar.map { "¿Borrar «\($0.nombre)»?" } ?? "",
                            isPresented: Binding(get: { aBorrar != nil }, set: { if !$0 { aBorrar = nil } }),
                            titleVisibility: .visible, presenting: aBorrar) { chat in
            Button("Borrar", role: .destructive) {
                if nav.chatAbierto == chat.id { nav.chatAbierto = nil }
                Task { await almacen.borrarProyecto(chat.id) }
            }
            Button("Cancelar", role: .cancel) {}
        } message: { _ in
            Text("Se borra también en tus otros aparatos la próxima vez que sincronices. "
                + "Queda una copia en la papelera de PixPin por si te arrepientes.")
        }
        .task {
            await almacen.cargarChats()
            // El chat «Asistente», fijo detras de «Mensajes guardados».
            await almacen.asegurarAsistente()
        }
        .onChange(of: nav.chatAbierto) { _, visible in almacen.aligerar(visible: visible) }
        .onDisappear { almacen.nadaAlFrente() }
        .overlay(alignment: .bottom) {
            if let error = almacen.error {
                Text(error).font(.caption).padding(8)
                    .background(.red.opacity(0.15), in: RoundedRectangle(cornerRadius: 8))
                    .padding()
                    .onTapGesture { almacen.ponerError(nil) }
            }
        }
    }

    /// El borde de la lista: se arrastra para estirarla (260 a 540, como
    /// `LISTA_MINIMA` y `LISTA_MAXIMA` de Windows).
    private var asaLista: some View {
        Color.clear
            .frame(width: 14)
            .contentShape(Rectangle())
            .hoverEffect(.highlight)
            .gesture(DragGesture(coordinateSpace: .global)
                .onChanged { g in
                    if anchoAlEmpezar == nil { anchoAlEmpezar = anchoLista }
                    anchoLista = min(max((anchoAlEmpezar ?? anchoLista) + g.translation.width, 260), 540)
                }
                .onEnded { _ in anchoAlEmpezar = nil })
    }
}

// MARK: - La lista

/// La columna de los proyectos.
struct ListaProyectos<Menu: View>: View {
    @ObservedObject var almacen: Almacen
    @Binding var elegido: String?
    /// Lo que va en el menu de cada fila (lo pone la seccion).
    @ViewBuilder let menu: (FichaChat) -> Menu
    @State private var filtro = ""
    @State private var creando = false
    @State private var nuevo = ""
    @State private var verArchivados = false

    private func casa(_ c: FichaChat) -> Bool {
        let f = filtro.trimmingCharacters(in: .whitespaces)
        guard !f.isEmpty else { return true }
        return c.nombre.localizedStandardContains(f) || c.resumen.localizedStandardContains(f)
    }

    private var visibles: [FichaChat] { almacen.chats.filter { !$0.archivado && casa($0) } }
    private var archivados: [FichaChat] { almacen.chats.filter { $0.archivado && casa($0) } }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                CapsulaBuscar(texto: $filtro)
                // Como la cabecera de Windows: galeria, timeline, tareas y
                // sincronizar a mano.
                BotonRedondoLista(simbolo: "photo.on.rectangle", ayuda: "Galería") {
                    Navegacion.compartida.seccion = .galeria
                }
                BotonRedondoLista(simbolo: "clock.arrow.circlepath", ayuda: "Timeline") {
                    Navegacion.compartida.seccion = .timeline
                }
                BotonRedondoLista(simbolo: "checklist", ayuda: "Tareas") {
                    Navegacion.compartida.seccion = .tareas
                }
                BotonRedondoLista(simbolo: "arrow.triangle.2.circlepath", ayuda: "Sincronizar") {
                    Navegacion.compartida.seccion = .aparatos
                }
            }
            .padding(.horizontal, 7)
            .padding(.vertical, 7)
            Paleta.proyectos.separador.frame(height: 1)
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(visibles, id: \.id) { chat in fila(chat) }
                    if !archivados.isEmpty {
                        Button {
                            withAnimation(.easeOut(duration: 0.15)) { verArchivados.toggle() }
                        } label: {
                            HStack(spacing: 12) {
                                Image(systemName: "archivebox")
                                    .font(.system(size: 18))
                                    .foregroundStyle(.white)
                                    .frame(width: 46, height: 46)
                                    .background(Paleta.proyectos.apagado, in: Circle())
                                Text("Archivados")
                                    .font(.system(size: 15, weight: .semibold))
                                    .foregroundStyle(Paleta.proyectos.texto)
                                Spacer()
                                Text("\(archivados.count)")
                                    .font(.system(size: 13))
                                    .foregroundStyle(Paleta.proyectos.apagado)
                                Image(systemName: verArchivados ? "chevron.up" : "chevron.down")
                                    .foregroundStyle(Paleta.proyectos.apagado)
                            }
                            .padding(.horizontal, 10)
                            .frame(height: 62)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        if verArchivados {
                            ForEach(archivados, id: \.id) { chat in fila(chat) }
                        }
                    }
                    if visibles.isEmpty, archivados.isEmpty, !filtro.isEmpty {
                        Text("Ningún proyecto con «\(filtro)»")
                            .font(.system(size: 14))
                            .foregroundStyle(Paleta.proyectos.apagado)
                            .padding(.top, 24)
                    }
                    // Que el ultimo no quede debajo del boton redondo.
                    Color.clear.frame(height: 80)
                }
            }
            .refreshable { await almacen.cargarChats() }
        }
        .background(Paleta.proyectos.lista.ignoresSafeArea())
        .overlay(alignment: .bottomTrailing) {
            BotonProyectoNuevo { creando = true }
                .padding(16)
        }
        .alert("Proyecto nuevo", isPresented: $creando) {
            TextField("Nombre", text: $nuevo)
            Button("Cancelar", role: .cancel) { nuevo = "" }
            Button("Crear", action: crear)
                .disabled(nuevo.trimmingCharacters(in: .whitespaces).isEmpty)
        }
    }

    private func fila(_ chat: FichaChat) -> some View {
        FilaProyecto(chat: chat, elegida: chat.id == elegido)
            .id(chat.id)
            .onTapGesture { elegido = chat.id }
            .contextMenu {
                Button { elegido = chat.id } label: { Label("Abrir", systemImage: "bubble.left") }
                menu(chat)
            }
    }

    private func crear() {
        let nombre = nuevo.trimmingCharacters(in: .whitespaces)
        nuevo = ""
        guard !nombre.isEmpty else { return }
        let almacen = self.almacen
        Task {
            let antes = Set(almacen.chats.map(\.id))
            await almacen.crearChat(nombre: nombre)
            // El nuevo, abierto (como Windows al crearlo).
            if let f = almacen.chats.first(where: { !antes.contains($0.id) }) { elegido = f.id }
        }
    }
}

/// El buscador de la lista: una capsula del color del buscador de Windows.
struct CapsulaBuscar: View {
    @Binding var texto: String

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 14))
                .foregroundStyle(Paleta.proyectos.apagado)
            TextField("", text: $texto, prompt: Text("Buscar").foregroundStyle(Paleta.proyectos.apagado))
                .textFieldStyle(.plain)
                .font(.system(size: 15))
                .foregroundStyle(Paleta.proyectos.texto)
                .autocorrectionDisabled()
                .submitLabel(.search)
            if !texto.isEmpty {
                Button { texto = "" } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(Paleta.proyectos.apagado)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Vaciar")
            }
        }
        .padding(.horizontal, 12)
        .frame(height: 38)
        .background(Paleta.proyectos.buscador, in: Capsule())
    }
}

/// Los botones redondos de junto al buscador: el disco dorado del movil
/// (`circulo` y `circulo_icono` de Windows).
struct BotonRedondoLista: View {
    let simbolo: String
    let ayuda: String
    let accion: () -> Void

    var body: some View {
        Button(action: accion) {
            Simbolo(nombre: simbolo, lado: 18)
                .foregroundStyle(Paleta.chat.circuloIcono)
                .frame(width: 38, height: 38)
                .background(Paleta.chat.circulo, in: Circle())
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .hoverEffect(.lift)
        .accessibilityLabel(ayuda)
    }
}

/// El boton redondo de proyecto nuevo, abajo a la derecha de la lista.
struct BotonProyectoNuevo: View {
    let accion: () -> Void

    var body: some View {
        Button(action: accion) {
            Image(systemName: "plus")
                .font(.system(size: 26, weight: .light))
                .foregroundStyle(.white)
                .frame(width: 56, height: 56)
                .background(Paleta.proyectos.contador, in: Circle())
                .shadow(color: .black.opacity(0.2), radius: 3, y: 1)
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .hoverEffect(.lift)
        .accessibilityLabel("Proyecto nuevo")
        .keyboardShortcut("n", modifiers: .command)
    }
}

/// Una fila de la lista: el avatar con sus iniciales, el nombre y debajo
/// la ultima linea (o cuantas hojas tiene); elegida, en azul con el texto
/// blanco.
struct FilaProyecto: View {
    let chat: FichaChat
    let elegida: Bool

    /// La segunda linea: la ultima del chat, sin la sintaxis de la nota
    /// (`# `, `**`, `- [ ]`) ni caracteres de control; si no hay, las hojas.
    static func debajo(_ chat: FichaChat) -> String {
        let r = String(textoConFormato(chat.resumen).characters)
            .replacingOccurrences(of: "\n", with: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if !r.isEmpty { return r }
        return chat.hojas == 1 ? "1 hoja" : "\(chat.hojas) hojas"
    }

    var body: some View {
        HStack(spacing: 12) {
            ZStack {
                Circle().fill(Paleta.avatar(chat.id))
                Text(Paleta.iniciales(chat.nombre))
                    .font(.system(size: 18, weight: .regular))
                    .foregroundStyle(.white)
            }
            .frame(width: 46, height: 46)
            VStack(alignment: .leading, spacing: 5) {
                Text(chat.nombre)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(elegida ? Color.white : Paleta.proyectos.texto)
                Text(Self.debajo(chat))
                    .font(.system(size: 14))
                    .foregroundStyle(elegida ? Color.white.opacity(0.85) : Paleta.proyectos.apagado)
            }
            .lineLimit(1)
            Spacer(minLength: 0)
            if chat.sinLeer > 0 {
                Text("\(chat.sinLeer)")
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(elegida ? Paleta.proyectos.filaElegida : .white)
                    .padding(.horizontal, 6)
                    .frame(minWidth: 21, minHeight: 21)
                    .background(elegida ? Color.white : Paleta.proyectos.contador, in: Capsule())
            }
        }
        .padding(.horizontal, 10)
        .frame(height: 62)
        .background(elegida ? Paleta.proyectos.filaElegida : .clear)
        .contentShape(Rectangle())
        .hoverEffect(.highlight)
    }
}
