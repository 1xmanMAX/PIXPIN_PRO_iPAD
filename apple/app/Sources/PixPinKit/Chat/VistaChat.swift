// **El chat de un proyecto**, como en Windows y el Mac (`Vistas.swift`):
// el papel de fondo, la cabecera en pastillas (el proyecto y la lupa), la
// conversacion por paginas y la isla de escribir abajo.
//
// La conversacion es una `LazyVStack`: solo existen las filas que se ven, y
// la pagina anterior se pide al llegar arriba (§4bis.2 del plan); tras un
// salto (desde Buscar, el timeline, una cita) se carga solo la pagina del
// mensaje y lo de despues al bajar.
//
// En el iPad: un toque abre (la foto y el dibujo en el lienzo, la nota en su
// editor, la tabla en la hoja, un archivo en su visor), mantener pulsado
// saca el menu del mensaje (`MenuMensaje.swift`); se adjunta desde el clip
// (Fotos, Archivos, la camara, «Pegar»), soltando encima o con ⌘V.

import PhotosUI
import PixPinCore
import GameController
import SwiftUI
import UIKit
import UniformTypeIdentifiers

public struct VistaChat: View {
    @ObservedObject var almacen: Almacen
    let chat: String
    /// Estrecho (como el movil): la flecha de volver a la lista.
    var alVolver: (() -> Void)?
    @ObservedObject private var nav = Navegacion.compartida
    /// Lo que toca el menu de los mensajes: a quien se responde, lo
    /// elegido, la hoja de escribir y el aviso.
    @StateObject private var menu: ChatAbierto
    /// La lupa del chat (`Chat/BuscarEnChat.swift`).
    @StateObject private var buscador: BuscadorDelChat
    /// Compartido, el lado de cada burbuja (`Chat/ConversacionCompartida.swift`).
    @StateObject private var compartida: ConversacionDelChat
    @ObservedObject private var chapas = ChapasDeCompartidos.compartidas
    /// La escala del chat, en por ciento (`Chat/EscalaChat.swift`).
    @AppStorage(EscalaChat.clave) private var escala = EscalaChat.porDefecto
    @State private var borrador = ""
    /// Algo arrastrado encima del chat (para marcarlo).
    @State private var soltando = false
    /// El mensaje marcado tras un salto, y al que desplazarse.
    @State private var destacado: String?
    @State private var irA: String?
    /// Sube al llegar la pagina del salto: entonces se desplaza.
    @State private var saltos = 0
    /// Se esta viendo una pagina del medio: no se baja solo al final.
    @State private var enSalto = false
    /// Sube para devolver el foco a la caja de escribir.
    @State private var focoCaja = 0
    @State private var eligiendoFotos = false
    @State private var fotos: [PhotosPickerItem] = []
    @State private var importando = false
    @State private var camara = false
    /// Las fotos elegidas, en el cuadro de enviar con su descripcion.
    @State private var porEnviar: LoteDeFotos?
    /// Elegir un proyecto o una pagina de un proyecto para adjuntar.
    @State private var eligiendoProyecto: Bool?
    /// ⇧ pulsada en el teclado del iPad: un toque marca el tramo.
    /// El panel Información del chat.
    @State private var verInfo = false

    public init(almacen: Almacen, chat: String, alVolver: (() -> Void)? = nil) {
        self.almacen = almacen
        self.chat = chat
        self.alVolver = alVolver
        _menu = StateObject(wrappedValue: ChatAbierto(chat: chat, almacen: almacen))
        _buscador = StateObject(wrappedValue: BuscadorDelChat(chat: chat, nucleo: almacen.nucleo))
        _compartida = StateObject(wrappedValue: ConversacionDelChat(chat: chat, nucleo: almacen.nucleo))
    }

    /// La escala como factor.
    private var f: CGFloat { CGFloat(EscalaChat.valida(escala)) / 100 }

    private var ficha: FichaChat? { almacen.chats.first { $0.id == chat } }

    private var cargados: [MensajeChat] { almacen.mensajes[chat] ?? [] }

    /// El ultimo fijado de lo cargado («Mensaje fijado», arriba).
    private var fijado: MensajeChat? { cargados.last(where: \.fijado) }

    /// Aquel al que se responde, si sigue cargado.
    private var respondido: MensajeChat? {
        guard let id = menu.respondiendo else { return nil }
        return cargados.first { $0.id == id }
    }

    /// Lo que se ve: la conversacion cargada o, buscando, lo que casa.
    private var filas: [MensajeChat] {
        buscador.filtrando ? buscador.hallados.map(\.mensaje) : cargados
    }

    /// El resultado elegido con ↑ ↓, buscando.
    private var elegidoEnBusqueda: String? {
        guard buscador.filtrando, let i = buscador.elegido else { return nil }
        return buscador.hallados[safe: i]?.id
    }

    private var idsCargados: [String] { cargados.map(\.id) }

    /// La cabecera (aparte: junta en `body` es demasiado para el compilador).
    private var cabecera: some View {
        let biblioteca: (() -> Void)? = ficha?.esGuardados == true ? { [almacen] in BibliotecaAudio.abrir(almacen: almacen) } : nil
        return CabeceraChat(ficha: ficha, personas: chapas.personas[chat], buscando: buscador.abierto, alVolver: alVolver,
                     buscar: { buscador.abrir() }, elegir: { empezarAElegir() },
                     escala: $escala, avisar: { menu.avisar($0) },
                     biblioteca: biblioteca,
                     informacion: { verInfo = true },
                     nuevaCarpeta: { [almacen, chat, menu] in
                         // Como el Mac: nace con el nombre listo para
                         // escribirlo («Cuaderno» no dice nada).
                         _ = Task<Void, Never> { @MainActor in
                             guard let c = await AccionesCarpeta.crear(en: chat, con: nil, almacen: almacen) else { return }
                             menu.escritura = .renombrar(c)
                         }
                     },
                     figuras: { [almacen, chat] in VistaFiguras.abrir(almacen: almacen, chat: chat) })
    }

    public var body: some View {
        VStack(spacing: 0) {
            cabecera
            if ficha?.esAsistente == true {
                CabeceraDelAsistente(almacen: almacen, chat: chat)
                    .background(Paleta.chat.cabecera)
            }
            if buscador.abierto {
                BarraBuscarEnChat(buscador: buscador, verEnContexto: verEnContexto)
            }
            if let fijado, !buscador.filtrando {
                BarraDeFijado(fijado: fijado) { ir(a: fijado.id) }
            }
            conversacion
            abajo
        }
        .background(PapelChat().ignoresSafeArea())
        .task {
            // Si se vino por un salto, la carga es la suya.
            if atenderSalto() || enSalto { return }
            await almacen.abrir(chat: chat)
        }
        .onReceive(nav.$saltoAMensaje) { _ in _ = atenderSalto() }
        // Compartido, de quien es cada burbuja: al cambiar lo que se ve o
        // quien esta dentro (Windows b89c058).
        .task(id: filas.map(\.id)) { compartida.releer(filas) }
        .onChange(of: chapas.personas[chat]) { _, _ in compartida.releer(filas) }
        // Lo que trae otro aparato al sincronizar: se relee (si no se esta
        // en un salto, que se perderia).
        .onReceive(NotificationCenter.default.publisher(for: .pixpinTrajoCambios)) { _ in
            guard !enSalto else { return }
            Task { await almacen.abrir(chat: chat) }
        }
        // Lo elegido que ya no esta cargado (borrado, otra pagina) se olvida.
        .onChange(of: almacen.mensajes[chat]?.count) { _, _ in
            menu.eleccion.quedarse(con: Set(idsCargados))
        }
        .onAppear {
            menu.irA = { id in ir(a: id) }
            CompartirDelIPad.activar()
        }
        // Describir, cambiar el nombre o la hora de avisar.
        .sheet(item: $menu.escritura) { e in
            HojaEscritura(escritura: e, guardar: { menu.guardar(e, $0) },
                          recordar: { m, cuando in menu.recordar(m, en: cuando) })
        }
        .sheet(item: $menu.compartiendo) { c in
            ActividadCompartir(cosas: c.cosas).ignoresSafeArea()
        }
        .confirmationDialog(ConfirmarBorrar.titulo(cuantos: menu.aBorrar?.count ?? 1),
                            isPresented: Binding(get: { menu.aBorrar != nil },
                                                 set: { if !$0 { menu.aBorrar = nil } }),
                            titleVisibility: .visible, presenting: menu.aBorrar) { ids in
            Button("Borrar", role: .destructive) { borrar(ids) }
            Button("Cancelar", role: .cancel) {}
        } message: { _ in
            Text(ConfirmarBorrar.aviso)
        }
        .overlay(alignment: .bottom) {
            if let aviso = menu.aviso { AvisoDelChat(texto: aviso).padding(.bottom, 70 * f) }
        }
        .animation(.easeOut(duration: 0.15), value: menu.aviso)
        // Soltar ficheros o fotos en el chat los adjunta (se copian).
        .onDrop(of: EntradaDeFicheros.tipos, isTargeted: $soltando) { proveedores in
            // Una burbuja de PixPin soltada aqui mismo no se adjunta otra vez.
            if ArrastrarBurbuja.esDeUnaBurbuja(proveedores) { return false }
            let (chat, almacen) = (self.chat, self.almacen)
            Task { @MainActor in await EntradaDeFicheros.soltar(proveedores, en: chat, almacen: almacen) }
            return true
        }
        .overlay {
            if soltando {
                RoundedRectangle(cornerRadius: 12)
                    .strokeBorder(Paleta.chat.enviar, style: StrokeStyle(lineWidth: 3, dash: [8, 5]))
                    .padding(4)
                    .allowsHitTesting(false)
            }
        }
        .photosPicker(isPresented: $eligiendoFotos, selection: $fotos, maxSelectionCount: 30,
                      matching: .any(of: [.images, .videos]))
        .onChange(of: fotos) { _, nuevas in
            guard !nuevas.isEmpty else { return }
            fotos = []
            adjuntarFotos(nuevas)
        }
        .fileImporter(isPresented: $importando, allowedContentTypes: [.item], allowsMultipleSelection: true) { r in
            guard case let .success(urls) = r, !urls.isEmpty else { return }
            adjuntarDeArchivos(urls)
        }
        .sheet(item: $porEnviar) { lote in
            HojaEnviarFotos(lote: lote, proyecto: ficha?.nombre ?? "") { fotos, descripcion in
                let (chat, almacen) = (self.chat, self.almacen)
                Task { await almacen.enviar(fotos: fotos, descripcion: descripcion, en: chat) }
            }
        }
        .sheet(isPresented: Binding(get: { eligiendoProyecto != nil }, set: { if !$0 { eligiendoProyecto = nil } })) {
            HojaElegirProyecto(almacen: almacen, chat: chat, conPaginas: eligiendoProyecto == true,
                               alElegirProyecto: { p in
                                   let (chat, almacen) = (self.chat, self.almacen)
                                   Task { await almacen.adjuntar(proyecto: p.id, en: chat) }
                               },
                               alElegirPagina: { p, pagina in
                                   let (chat, almacen) = (self.chat, self.almacen)
                                   Task { await almacen.adjuntar(pagina: pagina, de: p.id, en: chat) }
                               })
        }
        .fullScreenCover(isPresented: $camara) {
            CamaraChat(alHacer: { foto in
                guard let datos = foto.jpegData(compressionQuality: 0.9) else { return }
                let (chat, almacen) = (self.chat, self.almacen)
                Task { await almacen.adjuntar(datos: datos, formato: "jpg", en: chat) }
            }, alAcabar: { camara = false })
            .ignoresSafeArea()
        }
        .background { atajos }
        .sheet(isPresented: $verInfo) {
            VistaInfoChat(almacen: almacen, chat: chat, titulo: ficha?.nombre ?? "")
        }
    }

    // MARK: La conversacion

    private var conversacion: some View {
        ScrollViewReader { lector in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 6 * f) {
                    if !buscador.filtrando, almacen.quedanAnteriores(chat: chat) {
                        ProgressView().frame(maxWidth: .infinity)
                            .task { await almacen.cargarAnteriores(chat: chat) }
                    }
                    let lista = filas
                    ForEach(Array(lista.enumerated()), id: \.element.id) { i, m in
                        if !buscador.filtrando, SeparadorDeDia.cambia(lista[safe: i - 1], m) {
                            SeparadorDeDia(cuando: m.cuando)
                        }
                        fila(m).id(m.id)
                    }
                    if !buscador.filtrando, almacen.quedanPosteriores(chat: chat) {
                        ProgressView().frame(maxWidth: .infinity)
                            .task { await almacen.cargarPosteriores(chat: chat) }
                    }
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
            }
            .defaultScrollAnchor(.bottom)
            .scrollDismissesKeyboard(.interactively)
            .overlay {
                if buscador.filtrando, !buscador.cargando, buscador.hallados.isEmpty {
                    PildoraDelPapel(texto: "Nada con eso")
                } else if !buscador.filtrando, cargados.isEmpty, almacen.totales[chat] == 0 {
                    PildoraDelPapel(texto: "Aún no hay nada aquí")
                }
            }
            .onChange(of: almacen.mensajes[chat]?.last?.id) { _, ultimo in
                if let ultimo, !enSalto, !buscador.filtrando {
                    withAnimation(.easeOut(duration: 0.2)) { lector.scrollTo(ultimo, anchor: .bottom) }
                }
            }
            .onChange(of: saltos) { _, _ in
                guard let id = irA else { return }
                // Una vuelta despues: que la pila perezosa ya tenga la fila.
                DispatchQueue.main.async { withAnimation { lector.scrollTo(id, anchor: .center) } }
            }
            // Buscando, ↑ ↓ llevan al resultado elegido.
            .onChange(of: elegidoEnBusqueda) { _, id in
                guard let id else { return }
                DispatchQueue.main.async { lector.scrollTo(id, anchor: .center) }
            }
        }
        .environment(\.font, .system(size: 15 * f))
        .environment(\.escalaChat, f)
        .environment(\.agujaDelChat, buscador.filtrando ? buscador.texto : "")
    }

    /// Lo de abajo: la barra de lo elegido, de los resultados o de la
    /// respuesta, lo que suena y la isla.
    @ViewBuilder private var abajo: some View {
        if !menu.eleccion.vacia {
            BarraDeElegidos(c: menu)
        } else if let resultados = buscador.resultados {
            // Buscando, el recuento manda sobre la respuesta, que sigue
            // colgada y vuelve al cerrar la lupa (Windows).
            BarraDeResultados(rotulo: resultados, verEnContexto: verEnContexto)
        } else if let respondido {
            BarraDeRespuesta(citado: respondido) { menu.respondiendo = nil }
        }
        // Lo que suena, encima de la caja de escribir (D3).
        BarraReproductor().padding(.horizontal, MedidasIsla.margenX).padding(.top, 4)
        // Lo que se esta escuchando en voz alta («Escuchar» de un mensaje).
        BarraEscucharSuelta().padding(.horizontal, MedidasIsla.margenX).padding(.top, 4)
        IslaDeEscribir(borrador: $borrador, chat: chat, almacen: almacen, foco: focoCaja,
                       enviar: enviar, escaparVacio: escaparVacio,
                       pegarAdjuntos: { [menu] campo in
                           // Con texto escrito, una foto va dentro de lo
                           // escrito: sale una sola nota (`Chat/NotaRica.swift`).
                           EntradaDeFicheros.pegarEnLaNota(en: chat, almacen: almacen, campo: campo,
                                                           avisar: { menu.avisar($0) })
                               || EntradaDeFicheros.pegar(en: chat, almacen: almacen)
                       },
                       adjuntar: adjuntar)
            .environment(\.escalaChat, f)
    }

    /// Una burbuja con lo suyo: la marca del salto o del resultado elegido,
    /// lo elegido, el toque y el menu.
    private func fila(_ m: MensajeChat) -> some View {
        let elegida = menu.eleccion.ids.contains(m.id)
        let marcada = m.id == destacado || m.id == elegidoEnBusqueda
        // Compartido: lo mio a la derecha, lo de los demas a la izquierda
        // (`Chat/ConversacionCompartida.swift`); un chat normal, como siempre.
        let lado = compartida.lados[m.id]
        let mia = lado?.mio == true
        return HStack(alignment: .center, spacing: 8) {
            if !menu.eleccion.vacia {
                Image(systemName: elegida ? "checkmark.circle.fill" : "circle")
                    .font(.system(size: 22))
                    .foregroundStyle(elegida ? Paleta.chat.enviar : Color.white.opacity(0.8))
                    .transition(.move(edge: .leading).combined(with: .opacity))
            }
            if mia { Spacer(minLength: EnSuLado.margenContrario(f)) }
            BurbujaChat(mensaje: m, chat: chat, almacen: almacen,
                        citado: citado(por: m), irACita: { ir(a: m.respondeA ?? "") })
                .overlay {
                    if marcada {
                        RoundedRectangle(cornerRadius: 17 * f, style: .continuous)
                            .strokeBorder(Paleta.chat.enviar, lineWidth: 2)
                    }
                }
                .contextMenu {
                    MenuDeMensaje(m: m, c: menu) { propias(m) }
                }
                // Llevarla a otra app (Split View, Slide Over): su fichero,
                // la foto con lo anotado, el lienzo en PNG o el texto.
                .onDrag { ArrastrarBurbuja.proveedor(m, chat: chat, almacen: almacen) }
                .modifier(EnSuLado(lado: lado, f: f))
                .frame(maxWidth: 560 * f, alignment: mia ? .trailing : .leading)
            if !mia { Spacer(minLength: lado == nil ? 0 : EnSuLado.margenContrario(f)) }
        }
        .padding(.vertical, 1)
        .background {
            if elegida {
                Paleta.chat.enviar.opacity(0.16).padding(.horizontal, -12)
            } else if marcada {
                Paleta.proyectos.filaElegida.opacity(0.35).padding(.horizontal, -12)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { toque(m) }
    }

    /// Lo propio de cada burbuja en su menu (va con lo diario).
    @ViewBuilder private func propias(_ m: MensajeChat) -> some View {
        switch m.clase {
        case "IMAGEN":
            Button { abrir(m) } label: { Label("Anotar encima", systemImage: "pencil.tip.crop.circle") }
            Button { vistaRapida(m) } label: { Label("Vista rápida", systemImage: "eye") }
        case "ARCHIVO":
            Button { abrir(m) } label: { Label("Abrir", systemImage: "arrow.up.right.square") }
            Button { vistaRapida(m) } label: { Label("Vista rápida", systemImage: "eye") }
        case "VOZ":
            Button { transcribir(m) } label: {
                Label(m.transcripcion?.isEmpty == false ? "Volver a pasar a texto" : "Pasar a texto",
                      systemImage: "captions.bubble")
            }
            // Una clase grabada: «Estudiar la clase» lo pone el menu comun
            // (`MenuDeMensaje`), tambien en sus capturas y su video.
        case "DIBUJO":
            Button { abrir(m) } label: { Label("Abrir en lienzo", systemImage: "pencil.and.scribble") }
        case "TABLA":
            Button { abrir(m) } label: { Label("Abrir la hoja", systemImage: "tablecells") }
        case "MINIAPP" where BurbujaTablaChat.esTabla(m):
            Button { abrir(m) } label: { Label("Abrir la hoja", systemImage: "tablecells") }
        case "MINIAPP" where BurbujaTareas.esTareas(m):
            Button { abrir(m) } label: { Label("Abrir en Tareas", systemImage: "checklist") }
        case "PAGINA":
            Button { abrir(m) } label: { Label("Dibujar encima", systemImage: "pencil.tip.crop.circle") }
        case "PROYECTO":
            Button { abrir(m) } label: { Label("Abrir el proyecto", systemImage: "folder") }
        case "CROQUIS":
            Button { abrir(m) } label: { Label("Ver en 3D", systemImage: "cube") }
        default:
            EmptyView()
        }
        if VentanasDeVisor.sePuede, ChatAbierto.abreVisor(m) {
            Button { abrir(m, enOtraVentana: true) } label: {
                Label("Abrir en otra ventana", systemImage: "macwindow.on.rectangle")
            }
        }
    }

    /// Aquel al que responde `m`, si esta cargado.
    private func citado(por m: MensajeChat) -> MensajeChat? {
        guard let id = m.respondeA, !id.isEmpty else { return nil }
        return cargados.first { $0.id == id }
    }

    // MARK: Tocar y abrir

    /// Un toque: con alguno elegido, lo alterna (Windows); si no, abre.
    private func toque(_ m: MensajeChat) {
        // ⇧ + toque (teclado del iPad): todo lo de entre el ultimo tocado y
        // este, como Mayus+clic en Windows (`marcar.rs`).
        if mayusPulsada() {
            withAnimation(.easeOut(duration: 0.15)) { menu.eleccion.tramo(hasta: m.id, en: idsCargados) }
            return
        }
        if !menu.eleccion.vacia {
            withAnimation(.easeOut(duration: 0.15)) { menu.eleccion.alternar(m.id) }
        } else {
            abrir(m)
        }
    }

    private func empezarAElegir() {
        guard let ultimo = cargados.last else { return }
        withAnimation(.easeOut(duration: 0.15)) { menu.eleccion.alternar(ultimo.id) }
    }

    /// **Abre un mensaje** con su visor, como el clic de Windows: la foto en
    /// su lienzo, el dibujo en el editor, la nota en el suyo, la tabla en la
    /// hoja y un archivo segun su tipo.
    private func abrir(_ m: MensajeChat, enOtraVentana: Bool = false) {
        let (chat, almacen) = (self.chat, self.almacen)
        // «Abrir en otra ventana»: el visor va a una ventana suya (Stage Manager).
        let nav = enOtraVentana ? VentanasDeVisor.navegacionAparte() : self.nav
        switch m.clase {
        case "IMAGEN":
            Task {
                guard let url = await resolverFichero(m, chat, almacen) else { return }
                nav.abrir(.lienzo(chat: chat, mensaje: m, foto: url))
            }
        case "DIBUJO":
            nav.abrir(.lienzo(chat: chat, mensaje: m, foto: nil))
        case "NOTA":
            nav.abrir(.nota(chat: chat, mensaje: m))
        case "TABLA":
            nav.abrir(.hoja(chat: chat, mensaje: m))
        case "MINIAPP" where BurbujaTablaChat.esTabla(m):
            nav.abrir(.hoja(chat: chat, mensaje: m))
        case "MINIAPP" where BurbujaTareas.esTareas(m):
            nav.seccion = .tareas
        case "MINIAPP" where BurbujaCarpeta.esCarpeta(m):
            VistaCarpeta.abrir(chat: chat, carpeta: m.id, almacen: almacen)
        case "ARCHIVO":
            Task {
                guard let url = await resolverFichero(m, chat, almacen),
                      FileManager.default.fileExists(atPath: url.path) else {
                    menu.avisar("Este archivo no está en este iPad")
                    return
                }
                nav.abrirFichero(url, titulo: m.nombre.isEmpty ? nil : m.nombre, chat: chat, mensaje: m)
            }
        case "VOZ":
            Task {
                guard let url = await resolverFichero(m, chat, almacen) else { return }
                ReproductorVoz.compartido.alternar(m.id, url: url, titulo: BurbujaVoz.titulo(de: m))
            }
        case "PAGINA":
            abrirPagina(m, nav: nav)
        case "PROYECTO":
            if let f = BurbujaProyecto.ficha(m, en: almacen) {
                nav.irAlChat(f.id)
            } else {
                menu.avisar("Este proyecto no está en este iPad")
            }
        case "CROQUIS":
            abrirCroquis(m, nav: nav)
        default:
            break
        }
    }

    /// Como Windows al pulsar la hoja: en el lienzo, con la pagina de fondo,
    /// estrenando su dibujo si no lo tiene. Sin hoja en el proyecto, el
    /// documento en el lector.
    private func abrirPagina(_ m: MensajeChat, nav otra: Navegacion? = nil) {
        let (nucleo, chat, nav) = (almacen.nucleo, chat, otra ?? self.nav)
        let id = m.id
        Task {
            let dibujo = await Task.detached(priority: .userInitiated) {
                try? nucleo.estrenarDibujoDePagina(chat: chat, id: id)
            }.value ?? nil
            if let dibujo {
                nav.abrir(.lienzo(chat: chat, mensaje: mensajeDeLienzo(dibujo, como: m), foto: nil))
                return
            }
            let hoja = await Task.detached(priority: .userInitiated) {
                try? nucleo.hojaDePagina(chat: chat, id: id)
            }.value ?? nil
            if let pdf = hoja?.pdf {
                nav.abrirFichero(URL(fileURLWithPath: pdf), titulo: m.nombre.isEmpty ? nil : m.nombre,
                                 chat: chat, mensaje: m)
            } else {
                menu.avisar("Esta página no está en este iPad")
            }
        }
    }

    /// Un croquis 3D en el visor 3D (lo que el nucleo dice que es su fichero).
    private func abrirCroquis(_ m: MensajeChat, nav otra: Navegacion? = nil) {
        guard let r = m.referencia, !r.isEmpty else { return }
        let (nucleo, chat, nav) = (almacen.nucleo, chat, otra ?? self.nav)
        Task {
            let ruta = await Task.detached(priority: .userInitiated) {
                nucleo.croquisDelChat(chat: chat, referencia: r)
            }.value
            guard let ruta else {
                menu.avisar("Este croquis aún no ha llegado a este iPad")
                return
            }
            CroquisDelChatIPad.apuntar(ruta: ruta, chat: chat, referencia: r)
            nav.abrir(.modelo3d(url: URL(fileURLWithPath: ruta), titulo: BurbujaCroquis.titulo(m)))
        }
    }

    private func vistaRapida(_ m: MensajeChat) {
        let (chat, almacen, nav) = (self.chat, self.almacen, self.nav)
        Task {
            guard let url = await resolverFichero(m, chat, almacen),
                  FileManager.default.fileExists(atPath: url.path) else {
                menu.avisar("Este archivo no está en este iPad")
                return
            }
            nav.abrir(.vistaRapida(url: url))
        }
    }

    private func transcribir(_ m: MensajeChat) {
        let (chat, almacen) = (self.chat, self.almacen)
        menu.avisar("Pasando a texto…")
        Task { await almacen.transcribir(m, en: chat) }
    }

    // MARK: Saltos

    /// Lleva a un mensaje cargado y lo marca (la cita, el fijado, el hilo).
    private func ir(a id: String) {
        guard !id.isEmpty, idsCargados.contains(id) else { return }
        marcar(id)
        irA = id
        saltos += 1
    }

    /// Marca un momento el mensaje (como el resalte de Windows).
    private func marcar(_ id: String?) {
        destacado = id
        guard let id else { return }
        Task {
            try? await Task.sleep(nanoseconds: 2_500_000_000)
            if destacado == id { withAnimation { destacado = nil } }
        }
    }

    /// «Ver en contexto»: apaga la lupa y salta al resultado elegido en su
    /// sitio (cargando su pagina si no esta).
    private func verEnContexto() {
        guard let salto = buscador.enContexto() else { return }
        if let id = salto.mensaje, idsCargados.contains(id) {
            ir(a: id)
        } else {
            saltar(a: salto.mensaje, posicion: salto.posicion)
        }
        focoCaja += 1
    }

    /// Toma el salto pedido (`Navegacion.saltoAMensaje`) si es de este chat:
    /// carga su pagina, marca el mensaje y se desplaza hasta el.
    private func atenderSalto() -> Bool {
        guard let s = nav.saltoAMensaje, s.chat == chat else { return false }
        let id = s.mensaje
        nav.saltoAMensaje = nil
        if idsCargados.contains(id) {
            ir(a: id)
            return true
        }
        let (nucleo, chat) = (almacen.nucleo, chat)
        enSalto = true
        marcar(id)
        irA = id
        Task {
            let posicion = await Task.detached(priority: .userInitiated) {
                try? nucleo.posicionDeMensaje(chat: chat, mensaje: id)
            }.value ?? nil
            if let posicion {
                await almacen.abrir(chat: chat, en: posicion)
            } else {
                enSalto = false
                await almacen.abrir(chat: chat)
            }
            saltos += 1
        }
        return true
    }

    private func saltar(a id: String?, posicion: UInt64) {
        enSalto = true
        marcar(id)
        irA = id
        let (almacen, chat) = (self.almacen, self.chat)
        Task {
            await almacen.abrir(chat: chat, en: posicion)
            saltos += 1
        }
    }

    // MARK: Escribir y adjuntar

    /// Esc con la caja vacia: suelta lo elegido, la lupa y, si no, la
    /// respuesta.
    private func escaparVacio() -> Bool {
        if !menu.eleccion.vacia {
            menu.eleccion.vaciar()
            return true
        }
        if buscador.abierto {
            buscador.cerrar()
            return true
        }
        if menu.respondiendo != nil {
            menu.respondiendo = nil
            return true
        }
        return false
    }

    private func enviar() {
        let texto = borrador
        guard !texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        enSalto = false
        destacado = nil
        borrador = ""
        // Contestando a un mensaje (Responder): la nota lleva `respondeA`.
        let respondeA = menu.respondiendo
        menu.respondiendo = nil
        let (almacen, chat) = (self.almacen, self.chat)
        Task {
            // Si era para el asistente (su chat, o `@asistente`), contesta.
            if let m = await almacen.escribir(nota: texto, en: chat, respondeA: respondeA) {
                GanchosAlmacen.alEnviar?([m], chat, almacen)
            }
        }
    }

    /// Pregunta hecha: borra (`Almacen.borrar`).
    private func borrar(_ ids: [String]) {
        guard !ids.isEmpty else { return }
        if let d = destacado, ids.contains(d) { destacado = nil }
        if let r = menu.respondiendo, ids.contains(r) { menu.respondiendo = nil }
        let (almacen, chat, menu) = (self.almacen, self.chat, self.menu)
        Task {
            await almacen.borrar(mensajes: ids, en: chat)
            menu.eleccion.quedarse(con: Set((almacen.mensajes[chat] ?? []).map(\.id)))
        }
    }

    /// Una entrada de la hoja del clip.
    private func adjuntar(_ accion: AccionAdjuntar) {
        enSalto = false
        let (almacen, chat, nav) = (self.almacen, self.chat, self.nav)
        switch accion {
        case .foto:
            eligiendoFotos = true
        case .archivo:
            importando = true
        case .camara:
            camara = true
        case .lienzo:
            Task {
                guard let m = await almacen.crearLienzo(en: chat) else { return }
                nav.abrir(.lienzo(chat: chat, mensaje: m, foto: nil))
            }
        case .nota:
            // Windows abre el editor con una nota por nacer; aqui la nota
            // nace ya, con su titulo, y se abre en el editor de notas.
            Task {
                guard let m = await almacen.escribir(nota: "# Nota nueva", en: chat) else { return }
                nav.abrir(.nota(chat: chat, mensaje: m))
            }
        case .tareas:
            crear { nucleo in
                try nucleo.crearMiniapp(chat: chat, miniapp: "tareas", titulo: "Lista de tareas",
                                        moneda: Locale.current.currency?.identifier ?? "EUR")
            } despues: { _ in }
        case .tabla:
            crear { nucleo in try nucleo.crearTabla(chat: chat, nombre: "Tabla") } despues: { m in
                nav.abrir(.hoja(chat: chat, mensaje: m))
            }
        case .proyecto:
            eligiendoProyecto = false
        case .pagina:
            eligiendoProyecto = true
        case let .mini(miniapp):
            let titulo = DocumentoMini.nombre(miniapp)
            crear { nucleo in
                try nucleo.crearMiniapp(chat: chat, miniapp: miniapp, titulo: titulo,
                                        moneda: Locale.current.currency?.identifier ?? "EUR")
            } despues: { m in
                // Viva en un pin, como el panel flotante de Windows.
                GestorPinesIPad.compartido.pinearMiniapp(chat: chat, mensaje: m.id, miniapp: miniapp, documento: m.texto)
            }
        case let .pegar(proveedores):
            Task { await EntradaDeFicheros.soltar(proveedores, en: chat, almacen: almacen) }
        case let .voz(herramienta):
            VozEnElChat.abrir(herramienta, chat: chat, almacen: almacen, borrador: borrador)
        case .figura:
            FigurasChat.elegirParaAdjuntar(chat: chat, almacen: almacen, avisar: { [menu] in menu.avisar($0) },
                                           empezar: empezarCaja)
        case .pedirFigura:
            empezarCaja("Hazme una figura de ")
        case .carpeta:
            // Nace con el nombre listo para escribirlo, como el Mac.
            let menu = self.menu
            Task {
                guard let c = await AccionesCarpeta.crear(en: chat, con: nil, almacen: almacen) else { return }
                menu.escritura = .renombrar(c)
            }
        }
    }

    /// Lo que empieza la caja (las figuras del asistente): solo si estaba
    /// vacia, y con el foco en ella.
    private func empezarCaja(_ texto: String) {
        if borrador.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { borrador = texto }
        focoCaja += 1
    }

    /// Crea un mensaje con el nucleo (fuera del hilo principal), lo pone al
    /// final y hace `despues` con el.
    private func crear(_ hacer: @escaping @Sendable (Nucleo) throws -> MensajeChat,
                       despues: @escaping (MensajeChat) -> Void) {
        let (almacen, chat) = (self.almacen, self.chat)
        let nucleo = almacen.nucleo
        Task {
            let r = await Task.detached(priority: .userInitiated) { () -> Result<MensajeChat, Error> in
                Result { try hacer(nucleo) }
            }.value
            switch r {
            case let .success(m):
                await almacen.alFinal(m, en: chat)
                despues(m)
            case let .failure(e):
                almacen.ponerError("\(e)")
            }
        }
    }

    /// Lo elegido en Fotos: los bytes tal cual (HEIC pasa a PNG, como
    /// Windows y Android lo entienden), uno a uno.
    private func adjuntarFotos(_ elegidas: [PhotosPickerItem]) {
        // Antes de meterlas, el cuadro con sus miniaturas y la descripcion
        // (`envio.rs` de Windows): se reordenan, se quitan o se anaden.
        Task {
            let fotos = await OrdenDeEnvio.leer(elegidas)
            guard !fotos.isEmpty else { return }
            porEnviar = LoteDeFotos(fotos: fotos)
        }
    }

    /// Lo elegido en Archivos (con su permiso de acceso mientras se copia).
    private func adjuntarDeArchivos(_ urls: [URL]) {
        let (almacen, chat) = (self.almacen, self.chat)
        Task {
            let accedidos = urls.map { ($0, $0.startAccessingSecurityScopedResource()) }
            await almacen.adjuntar(urls: urls, en: chat)
            for (u, a) in accedidos where a { u.stopAccessingSecurityScopedResource() }
        }
    }

    // MARK: El teclado del iPad

    /// ⌘F (la lupa del chat, como el Ctrl+F de Windows) y la escala (⌘+,
    /// ⌘−, ⌘0), con el teclado del iPad.
    private var atajos: some View {
        Group {
            Button("Buscar en el chat") { buscador.abrir() }
                .keyboardShortcut("f", modifiers: .command)
            Button("Más grande") { cambiarEscala("+") }
                .keyboardShortcut("+", modifiers: .command)
            Button("Más grande") { cambiarEscala("+") }
                .keyboardShortcut("=", modifiers: .command)
            Button("Más pequeño") { cambiarEscala("-") }
                .keyboardShortcut("-", modifiers: .command)
            Button("Tamaño normal") { cambiarEscala("0") }
                .keyboardShortcut("0", modifiers: .command)
            if buscador.abierto || !menu.eleccion.vacia {
                Button("Cerrar") { _ = escaparVacio() }
                    .keyboardShortcut(.escape, modifiers: [])
            }
        }
        .opacity(0)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    private func cambiarEscala(_ tecla: String) {
        guard let pedida = EscalaChat.pedida(tecla: tecla, actual: escala) else { return }
        escala = pedida
        menu.avisar(EscalaChat.aviso(pedida))
    }
}

// MARK: - La cabecera

/// La cabecera del chat, como Windows: sobre el papel, a la izquierda la
/// flecha de volver (estrecho), en medio la pastilla del proyecto (el disco
/// de la carpeta, el nombre y cuantas hojas) y a la derecha la de la lupa y
/// el menu.
struct CabeceraChat: View {
    let ficha: FichaChat?
    /// Compartido: cuantas personas (la chapa verde y «Compartido · N
    /// personas» delante, lo primero que hay que saber antes de escribir).
    var personas: UInt32?
    let buscando: Bool
    var alVolver: (() -> Void)?
    let buscar: () -> Void
    let elegir: () -> Void
    @Binding var escala: Int
    let avisar: (String) -> Void
    /// «Biblioteca de audio» (sale en «Mensajes guardados», como Windows).
    var biblioteca: (() -> Void)?
    /// El panel Información (Todo, Fotos, Archivos, Voz...).
    var informacion: (() -> Void)?
    /// «Carpeta de páginas» nueva y la galería de «Figuras» (Windows, 24 y
    /// 26-oct; `Chat/CarpetasChat.swift`, `Chat/FigurasChat.swift`).
    var nuevaCarpeta: (() -> Void)?
    var figuras: (() -> Void)?

    var body: some View {
        HStack(spacing: 10) {
            if let alVolver {
                Button(action: alVolver) {
                    Image(systemName: "arrow.left")
                        .font(.system(size: 17, weight: .medium))
                        .foregroundStyle(Paleta.chat.pildoraTexto)
                        .frame(width: 44, height: 44)
                        .background(Paleta.chat.pildora, in: Circle())
                        .overlay(Circle().strokeBorder(Paleta.chat.islaBorde, lineWidth: 1))
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Volver a los proyectos")
            }
            Spacer(minLength: 0)
            HStack(spacing: 8) {
                Image(systemName: ficha?.esAsistente == true ? "sparkles"
                      : ficha?.esGuardados == true ? "bookmark.fill" : "folder.fill")
                    .font(.system(size: 13))
                    .foregroundStyle(Paleta.chat.carpetaIcono)
                    .frame(width: 30, height: 30)
                    .background(Paleta.chat.carpeta, in: Circle())
                    .overlay(alignment: .bottomTrailing) {
                        if personas != nil { ChapaCompartido(avatar: 30, aro: Paleta.chat.pildora) }
                    }
                VStack(alignment: .leading, spacing: 0) {
                    Text(ficha?.nombre ?? "")
                        .font(.system(size: 15, weight: .medium))
                        .foregroundStyle(Paleta.chat.pildoraTexto)
                    Text(subtitulo)
                        .font(.system(size: 11))
                        .foregroundStyle(Paleta.chat.pildoraSub)
                }
                .lineLimit(1)
            }
            .padding(.leading, 6)
            .padding(.trailing, 14)
            .frame(height: 44)
            .background(Paleta.chat.pildora, in: Capsule())
            .overlay(Capsule().strokeBorder(Paleta.chat.islaBorde, lineWidth: 1))
            // Tocar el nombre abre la ficha del chat (como el Mac y Telegram:
            // proyectos, fotos, videos, archivos y enlaces).
            .contentShape(Capsule())
            .onTapGesture { informacion?() }
            .hoverEffect(.highlight)
            .accessibilityAddTraits(informacion != nil ? .isButton : [])
            .accessibilityHint(informacion != nil ? "Proyectos, fotos, vídeos, archivos y enlaces del chat" : "")
            Spacer(minLength: 0)
            HStack(spacing: 2) {
                Button(action: buscar) {
                    Image(systemName: "magnifyingglass").frame(width: 40, height: 40).contentShape(Rectangle())
                }
                .accessibilityLabel("Buscar en el chat (⌘F)")
                Menu {
                    Button(action: buscar) { Label("Buscar en el chat", systemImage: "magnifyingglass") }
                    Button(action: elegir) { Label("Elegir varios", systemImage: "checkmark.circle") }
                    if let informacion {
                        Button(action: informacion) { Label("Información", systemImage: "info.circle") }
                    }
                    if let biblioteca {
                        Button(action: biblioteca) { Label("Biblioteca de audio", systemImage: "music.note.list") }
                    }
                    if let nuevaCarpeta {
                        Button(action: nuevaCarpeta) { Label("Carpeta de páginas", systemImage: "book") }
                    }
                    if let figuras {
                        Button(action: figuras) { Label("Figuras", systemImage: "square.on.square") }
                    }
                    Section("Tamaño del chat") {
                        Button { cambiar("+") } label: { Label("Más grande (⌘+)", systemImage: "plus.magnifyingglass") }
                        Button { cambiar("-") } label: { Label("Más pequeño (⌘−)", systemImage: "minus.magnifyingglass") }
                        Button { cambiar("0") } label: { Label("Tamaño normal (⌘0)", systemImage: "1.magnifyingglass") }
                    }
                } label: {
                    Image(systemName: "ellipsis").frame(width: 40, height: 40).contentShape(Rectangle())
                }
                .accessibilityLabel("Más")
            }
            .buttonStyle(.plain)
            .font(.system(size: 17, weight: .medium))
            .foregroundStyle(Paleta.chat.pildoraTexto)
            .padding(.horizontal, 4)
            .frame(height: 44)
            .background(Paleta.chat.pildora, in: Capsule())
            .overlay(Capsule().strokeBorder(Paleta.chat.islaBorde, lineWidth: 1))
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
    }

    private var subtitulo: String {
        guard let ficha else { return "" }
        if ficha.esAsistente { return "Asistente" }
        let hojas = ficha.hojas == 1 ? "1 hoja" : "\(ficha.hojas) hojas"
        return personas.map { "\(TextosCompartido.chapa($0)) · \(hojas)" } ?? hojas
    }

    private func cambiar(_ tecla: String) {
        guard let pedida = EscalaChat.pedida(tecla: tecla, actual: escala) else { return }
        escala = pedida
        avisar(EscalaChat.aviso(pedida))
    }
}

// MARK: - Piezas del papel

/// La pildora que separa los dias («Hoy», «Ayer», «26 sept»), como Windows.
struct SeparadorDeDia: View {
    let cuando: Int64

    /// Si entre `antes` y `m` cambia el dia (o `m` es el primero).
    static func cambia(_ antes: MensajeChat?, _ m: MensajeChat) -> Bool {
        guard let antes else { return true }
        let c = Calendar.current
        return !c.isDate(fecha(antes.cuando), inSameDayAs: fecha(m.cuando))
    }

    static func fecha(_ ms: Int64) -> Date { Date(timeIntervalSince1970: TimeInterval(ms) / 1000) }

    /// «Hoy», «Ayer» o la fecha («26 sept», con el año si no es este).
    static func rotulo(_ ms: Int64, ahora: Date = Date(), calendario: Calendar = .current) -> String {
        let d = fecha(ms)
        if calendario.isDate(d, inSameDayAs: ahora) { return "Hoy" }
        if let ayer = calendario.date(byAdding: .day, value: -1, to: ahora), calendario.isDate(d, inSameDayAs: ayer) {
            return "Ayer"
        }
        let f = DateFormatter()
        f.locale = Locale(identifier: "es_ES")
        f.calendar = calendario
        f.timeZone = calendario.timeZone
        let mismoAno = calendario.component(.year, from: d) == calendario.component(.year, from: ahora)
        f.setLocalizedDateFormatFromTemplate(mismoAno ? "d MMM" : "d MMM y")
        return f.string(from: d)
    }

    var body: some View {
        Text(Self.rotulo(cuando))
            .font(.system(size: 13, weight: .medium))
            .foregroundStyle(Paleta.chat.textoSeparador)
            .padding(.horizontal, 10)
            .padding(.vertical, 3)
            .background(Paleta.chat.separadorDia, in: Capsule())
            .frame(maxWidth: .infinity)
            .padding(.vertical, 6)
    }
}

/// Un texto en pildora sobre el papel («Elige un proyecto», «Nada con eso»).
struct PildoraDelPapel: View {
    let texto: String

    var body: some View {
        Text(texto)
            .font(.system(size: 14, weight: .medium))
            .foregroundStyle(.white)
            .padding(.horizontal, 12).padding(.vertical, 5)
            .background(.black.opacity(0.25), in: Capsule())
    }
}

/// El papel del chat, en degradado como en Windows y el movil (el «mar»).
struct PapelChat: View {
    var body: some View {
        LinearGradient(colors: [Paleta.proyectos.papelArriba, Paleta.proyectos.papelAbajo],
                       startPoint: .top, endPoint: .bottom)
    }
}

/// Si el teclado del iPad tiene Mayus pulsada ahora mismo (⇧+toque marca un
/// tramo, como ⇧-clic en Windows). En iOS SwiftUI no da los modificadores al
/// tocar; el teclado fisico si los dice por GameController.
@MainActor
func mayusPulsada() -> Bool {
    guard let teclado = GCKeyboard.coalesced?.keyboardInput else { return false }
    return teclado.button(forKeyCode: .leftShift)?.isPressed == true
        || teclado.button(forKeyCode: .rightShift)?.isPressed == true
}

extension ChatAbierto {
    /// Si tocar `m` abre un visor (y por tanto puede ir a otra ventana).
    static func abreVisor(_ m: MensajeChat) -> Bool {
        switch m.clase {
        case "IMAGEN", "DIBUJO", "NOTA", "TABLA", "ARCHIVO", "PAGINA", "CROQUIS": return true
        case "MINIAPP": return BurbujaTablaChat.esTabla(m)
        default: return false
        }
    }
}
