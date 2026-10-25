// La ventana del iPad: la barra lateral con las secciones (lo que en Windows
// son las ventanas de la bandeja) y, encima de todo, el visor a pantalla
// completa y la capa de pines flotantes.

import PixPinCore
import SwiftUI

public struct RaizIPad: View {
    @ObservedObject var entorno: EntornoApp
    @ObservedObject private var nav = Navegacion.compartida
    @State private var columnas: NavigationSplitViewVisibility = .automatic

    public init(entorno: EntornoApp) {
        self.entorno = entorno
    }

    public var body: some View {
        ConAlmacen(entorno.almacen, error: entorno.error) { almacen in
            NavigationSplitView(columnVisibility: $columnas) {
                List(selection: $nav.seccion) {
                    ForEach(Seccion.allCases) { s in
                        Label(s.titulo, systemImage: s.icono).tag(s)
                    }
                }
                .navigationTitle("PixPin")
                .safeAreaInset(edge: .bottom) {
                    BotonCapturar()
                        .keyboardShortcut("1", modifiers: [.command, .shift])
                        .padding(12)
                }
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) { BotonPinesAbiertos() }
                }
                .navigationSplitViewColumnWidth(min: 200, ideal: 230, max: 280)
            } detail: {
                seccion(nav.seccion ?? .proyectos, almacen: almacen)
            }
            .overlay { CapaPines(almacen: almacen) }
            .fullScreenCover(item: $nav.visor) { visor in
                VistaDelVisor(visor: visor, almacen: almacen)
                    .overlay { CapaPines(almacen: almacen) }
                    .environmentObject(entorno)
                    .environmentObject(nav)
            }
        }
        .environmentObject(entorno)
        .environmentObject(nav)
        .onOpenURL { url in AbrirDesdeFuera.abrir(url, entorno: entorno) }
    }

    @ViewBuilder
    private func seccion(_ s: Seccion, almacen: Almacen) -> some View {
        switch s {
        case .proyectos: VistaProyectos(almacen: almacen)
        case .galeria: VistaGaleria(almacen: almacen)
        case .timeline: VistaTimeline(almacen: almacen)
        case .tareas: VistaTareas(almacen: almacen)
        case .lecciones: VistaLeccionesIPad(almacen: almacen)
        case .buscar: VistaBuscar(almacen: almacen)
        case .asistente: VistaAsistenteIPad(almacen: almacen)
        case .aparatos: VistaSincroIPad(almacen: almacen, sincro: entorno.sincro)
        case .ajustes: VistaAjustesIPad(almacen: almacen, sincro: entorno.sincro)
        }
    }
}

/// El visor a pantalla completa que toca a cada `Visor`.
public struct VistaDelVisor: View {
    let visor: Visor
    let almacen: Almacen

    public init(visor: Visor, almacen: Almacen) {
        self.visor = visor
        self.almacen = almacen
    }

    public var body: some View {
        switch visor {
        case let .lienzo(chat, mensaje, foto):
            EditorLienzoIPad(almacen: almacen, chat: chat, mensaje: mensaje, foto: foto)
        case let .documento(url, titulo, chat, mensaje):
            VisorDocumentoIPad(url: url, titulo: titulo, almacen: almacen, chat: chat, mensaje: mensaje)
        case let .nota(chat, mensaje):
            EditorNotaIPad(almacen: almacen, chat: chat, mensaje: mensaje)
        case let .hoja(chat, mensaje):
            HojaCalculoIPad(almacen: almacen, chat: chat, mensaje: mensaje)
        case let .plano(url, titulo):
            VisorPlanoIPad(url: url, titulo: titulo, nucleo: almacen.nucleo)
        case let .modelo3d(url, titulo):
            Visor3DIPad(url: url, titulo: titulo, nucleo: almacen.nucleo)
        case let .medio(url, titulo):
            VisorMedio(url: url, titulo: titulo)
        case let .vistaRapida(url):
            VistaRapida(url: url)
        }
    }
}
