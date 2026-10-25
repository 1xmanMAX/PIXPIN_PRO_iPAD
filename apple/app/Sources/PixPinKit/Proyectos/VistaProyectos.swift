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

import PhotosUI
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
    /// El proyecto al que se le elige la imagen de logo.
    @State private var logoPara: FichaChat?
    @State private var fotoLogo: [PhotosPickerItem] = []
    @ObservedObject private var logos = LogosProyecto.compartidos

    public init(almacen: Almacen) { self.almacen = almacen }

    public var body: some View {
        HStack(spacing: 0) {
            if !estrecho || nav.chatAbierto == nil {
                ListaProyectos(almacen: almacen, elegido: $nav.chatAbierto) { chat in
                    // Compartir con formatos: sus hojas en PDF, HTML, PNG...,
                    // los originales y el proyecto entero como `.pixpin`.
                    Button { HojaCompartir.abrir(.proyectos([chat]), almacen: almacen) } label: {
                        Label("Compartir…", systemImage: "square.and.arrow.up")
                    }
                    if !chat.esAsistente {
                        Button { logoPara = chat } label: {
                            Label("Poner imagen de logo…", systemImage: "photo.circle")
                        }
                        if logos.tiene(chat.id) {
                            Button {
                                Task { await logos.quitar(chat.id, nucleo: almacen.nucleo) }
                            } label: { Label("Quitar imagen de logo", systemImage: "circle.slash") }
                        }
                    }
                    if !chat.esGuardados, !chat.esAsistente {
                        Button(role: .destructive) { aBorrar = chat } label: {
                            Label("Borrar proyecto", systemImage: "trash")
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
        .photosPicker(isPresented: Binding(get: { logoPara != nil }, set: { if !$0, fotoLogo.isEmpty { logoPara = nil } }),
                      selection: $fotoLogo, maxSelectionCount: 1, matching: .images)
        .onChange(of: fotoLogo) { _, elegidas in
            guard let item = elegidas.first, let chat = logoPara else { return }
            fotoLogo = []
            logoPara = nil
            let (logos, nucleo, almacen) = (self.logos, almacen.nucleo, self.almacen)
            Task {
                guard let datos = try? await item.loadTransferable(type: Data.self),
                      await logos.poner(chat.id, datos: datos, nucleo: nucleo) else {
                    almacen.ponerError("No se pudo poner esa imagen de logo")
                    return
                }
            }
        }
        .task {
            CompartirDelIPad.activar()
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
    /// «Elegir varios»: los marcados (para compartirlos o borrarlos juntos).
    @State private var marcando = false
    @State private var marcados: Set<String> = []
    @State private var preguntarBorrar = false
    @State private var verBorrados = false
    @ObservedObject private var logos = LogosProyecto.compartidos

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
                Menu {
                    Button { empezarAMarcar(nil) } label: { Label("Elegir varios", systemImage: "checkmark.circle") }
                    Button { verBorrados = true } label: { Label("Proyectos borrados…", systemImage: "trash.slash") }
                } label: {
                    Image(systemName: "ellipsis")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(Paleta.chat.circuloIcono)
                        .frame(width: 38, height: 38)
                        .background(Paleta.chat.circulo, in: Circle())
                        .contentShape(Circle())
                }
                .accessibilityLabel("Más")
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
            if !marcando {
                BotonProyectoNuevo { creando = true }
                    .padding(16)
            }
        }
        .safeAreaInset(edge: .bottom) {
            if marcando { barraDeMarcados }
        }
        .confirmationDialog(marcados.count == 1 ? "¿Borrar 1 proyecto?" : "¿Borrar \(marcados.count) proyectos?",
                            isPresented: $preguntarBorrar, titleVisibility: .visible) {
            Button("Borrar los marcados", role: .destructive, action: borrarMarcados)
            Button("Cancelar", role: .cancel) {}
        } message: {
            Text("Se borran también en tus otros aparatos la próxima vez que sincronices. "
                + "Queda una copia en la papelera de PixPin por si te arrepientes.")
        }
        .sheet(isPresented: $verBorrados) {
            NavigationStack {
                VistaCopias(nucleo: almacen.nucleo) { [almacen] in Task { await almacen.cargarChats() } }
                    .navigationTitle("Proyectos borrados")
                    .navigationBarTitleDisplayMode(.inline)
            }
        }
        .alert("Proyecto nuevo", isPresented: $creando) {
            TextField("Nombre", text: $nuevo)
            Button("Cancelar", role: .cancel) { nuevo = "" }
            Button("Crear", action: crear)
                .disabled(nuevo.trimmingCharacters(in: .whitespaces).isEmpty)
        }
    }

    private func fila(_ chat: FichaChat) -> some View {
        let marcado = marcados.contains(chat.id)
        return HStack(spacing: 0) {
            if marcando {
                Image(systemName: marcado ? "checkmark.circle.fill" : "circle")
                    .font(.system(size: 22))
                    .foregroundStyle(marcado ? Paleta.proyectos.filaElegida : Paleta.proyectos.apagado)
                    .padding(.leading, 10)
                    .opacity(sePuedeMarcar(chat) ? 1 : 0.3)
            }
            FilaProyecto(chat: chat, elegida: !marcando && chat.id == elegido, logo: logos.logo(chat.id))
        }
        .id(chat.id)
        .contentShape(Rectangle())
        .onTapGesture {
            if marcando {
                guard sePuedeMarcar(chat) else { return }
                if marcado { marcados.remove(chat.id) } else { marcados.insert(chat.id) }
            } else {
                elegido = chat.id
            }
        }
        .onAppear { logos.mirar(chat.id, nucleo: almacen.nucleo) }
        .contextMenu {
            Button { elegido = chat.id } label: { Label("Abrir", systemImage: "bubble.left") }
            menu(chat)
            if sePuedeMarcar(chat) {
                Button { empezarAMarcar(chat.id) } label: { Label("Elegir varios", systemImage: "checkmark.circle") }
            }
        }
    }

    /// «Mensajes guardados» y el asistente no se borran ni se marcan.
    private func sePuedeMarcar(_ c: FichaChat) -> Bool { !c.esGuardados && !c.esAsistente }

    private func empezarAMarcar(_ id: String?) {
        withAnimation(.easeOut(duration: 0.15)) {
            marcando = true
            marcados = id.map { [$0] } ?? []
        }
    }

    private func dejarDeMarcar() {
        withAnimation(.easeOut(duration: 0.15)) {
            marcando = false
            marcados = []
        }
    }

    /// Los marcados, en el orden de la lista.
    private var fichasMarcadas: [FichaChat] { almacen.chats.filter { marcados.contains($0.id) } }

    /// La barra de lo marcado, abajo: cuantos, Compartir y Borrar.
    private var barraDeMarcados: some View {
        HStack(spacing: 14) {
            Text(marcados.count == 1 ? "1 marcado" : "\(marcados.count) marcados")
                .font(.system(size: 15, weight: .medium))
                .foregroundStyle(Paleta.proyectos.texto)
            Spacer()
            Button {
                HojaCompartir.abrir(.proyectos(fichasMarcadas), almacen: almacen)
            } label: { Label("Compartir", systemImage: "square.and.arrow.up") }
                .disabled(marcados.isEmpty)
            Button(role: .destructive) { preguntarBorrar = true } label: {
                Label("Borrar los marcados", systemImage: "trash")
            }
            .foregroundStyle(.red)
            .disabled(marcados.isEmpty)
            Button("Cancelar", action: dejarDeMarcar)
                .keyboardShortcut(.cancelAction)
        }
        .labelStyle(.iconOnly)
        .font(.system(size: 17))
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(Paleta.proyectos.lista)
        .overlay(alignment: .top) { Paleta.proyectos.separador.frame(height: 1) }
    }

    /// «Borrar los marcados» (`proyecto-borrar-varios`): uno tras otro, y si
    /// el abierto estaba entre ellos, se cierra.
    private func borrarMarcados() {
        let ids = fichasMarcadas.filter(sePuedeMarcar).map(\.id)
        if let e = elegido, ids.contains(e) { elegido = nil }
        dejarDeMarcar()
        let almacen = self.almacen
        Task {
            for id in ids { await almacen.borrarProyecto(id) }
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
    /// La imagen de logo, si tiene (`LogoProyecto.swift`).
    var logo: UIImage?

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
            AvatarProyecto(chat: chat, logo: logo, lado: 46)
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
