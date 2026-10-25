// Lecciones en el iPad: la barra comun (buscar y «Nueva leccion») y, debajo,
// la lista por paginas (filtrar por etiqueta, «para repasar») con la ficha
// para escribirla o repasarla al lado. Las piezas son las del Mac
// (`ModeloLecciones`, `FilaLeccion`, `FichaLeccionVista`, en
// `ModeloLecciones.swift` y `VistaLecciones.swift`); aqui solo se colocan
// sin una segunda `NavigationSplitView` (la de la app ya tiene la barra
// lateral). En una ventana estrecha la ficha sale en una hoja.

import PixPinCore
import SwiftUI

public struct VistaLeccionesIPad: View {
    let almacen: Almacen
    @StateObject private var modelo: ModeloLecciones

    public init(almacen: Almacen) {
        self.almacen = almacen
        _modelo = StateObject(wrappedValue: ModeloLecciones(nucleo: almacen.nucleo))
    }

    public var body: some View {
        CuerpoLeccionesConCabecera(modelo: modelo)
            .task { ArranqueOrganizar.apuntar() }
    }
}

/// La barra y el cuerpo, sobre un modelo ya hecho.
struct CuerpoLeccionesConCabecera: View {
    @ObservedObject var modelo: ModeloLecciones
    @State private var nueva = false

    var body: some View {
        VStack(spacing: 0) {
            Cabecera("Lecciones", subtitulo: modelo.total == 1 ? "1 lección" : "\(modelo.total) lecciones",
                     buscador: BuscadorCabecera($modelo.texto, pista: "Buscar: palabra, situación, etiqueta…")) {
                BotonCabecera(icono: "plus", rotulo: "Nueva lección") { nueva = true }
                    .keyboardShortcut("n")
            }
            ListaYFichaLecciones(modelo: modelo, nueva: $nueva)
        }
        .ventanaConCabecera()
    }
}

/// Las lecciones dentro de la pestana «Lecciones» del timeline: el buscador
/// es el del timeline (`texto`).
struct CuerpoLecciones: View {
    @StateObject private var modelo: ModeloLecciones
    let texto: String
    @State private var nueva = false

    init(nucleo: Nucleo, texto: String) {
        _modelo = StateObject(wrappedValue: ModeloLecciones(nucleo: nucleo))
        self.texto = texto
    }

    var body: some View {
        ListaYFichaLecciones(modelo: modelo, nueva: $nueva)
            .onAppear { if modelo.texto != texto { modelo.texto = texto } }
            .onChange(of: texto) { _, t in modelo.texto = t }
            .overlay(alignment: .bottomTrailing) {
                Button { nueva = true } label: {
                    Image(systemName: "plus")
                        .font(.system(size: 20, weight: .semibold))
                        .foregroundStyle(.white)
                        .frame(width: 52, height: 52)
                        .background(Paleta.v2.acento, in: Circle())
                        .shadow(color: .black.opacity(0.35), radius: 8, y: 3)
                }
                .buttonStyle(.plain)
                .padding(20)
                .accessibilityLabel("Nueva lección")
            }
    }
}

/// La lista (con sus filtros) y la ficha de la elegida o de la nueva.
struct ListaYFichaLecciones: View {
    @ObservedObject var modelo: ModeloLecciones
    @Binding var nueva: Bool
    @State private var elegida: String?
    @State private var enHoja = false
    @Environment(\.horizontalSizeClass) private var clase

    private var estrecha: Bool { clase == .compact }

    var body: some View {
        HStack(spacing: 0) {
            lista
                .frame(width: estrecha ? nil : 340)
                .frame(maxWidth: estrecha ? .infinity : nil)
            if !estrecha {
                Paleta.v2.linea.frame(width: 1)
                ficha.frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(Paleta.v2.fondo)
        .task { await modelo.recargar() }
        .onChange(of: elegida) { _, v in
            if v != nil { nueva = false }
            if estrecha, v != nil { enHoja = true }
        }
        .onChange(of: nueva) { _, v in
            if v { elegida = nil }
            if estrecha, v { enHoja = true }
        }
        .sheet(isPresented: $enHoja, onDismiss: {
            nueva = false
            elegida = nil
        }) {
            NavigationStack {
                ficha
                    .toolbar {
                        ToolbarItem(placement: .cancellationAction) { Button("Cerrar") { enHoja = false } }
                    }
            }
            .environment(\.colorScheme, .dark)
        }
        .overlay(alignment: .bottom) {
            if let error = modelo.error {
                AvisoAbajo(texto: error, esError: true)
            }
        }
    }

    private var lista: some View {
        VStack(spacing: 0) {
            filtros
            List(selection: $elegida) {
                ForEach(modelo.lecciones, id: \.id) { l in
                    FilaLeccion(leccion: l)
                        .tag(l.id)
                        .listRowBackground(elegida == l.id ? Paleta.v2.cajaElegida : Paleta.v2.tarjeta)
                        .onAppear {
                            if l.id == modelo.lecciones.last?.id {
                                Task { await modelo.cargarMas() }
                            }
                        }
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .refreshable { await modelo.recargar() }
            .overlay {
                if modelo.lecciones.isEmpty && !modelo.cargando {
                    let titulo: String = modelo.texto.isEmpty ? "Sin lecciones" : "Nada encontrado"
                    ContentUnavailableView { Label(titulo, systemImage: "lightbulb") }
                }
            }
        }
    }

    @ViewBuilder
    private var ficha: some View {
        if nueva {
            FichaLeccionVista(modelo: modelo, ficha: nil) { guardada in
                nueva = false
                enHoja = false
                if !estrecha { elegida = guardada?.id }
            }
            .id("nueva")
        } else if let id = elegida, let l = modelo.lecciones.first(where: { $0.id == id }) {
            FichaLeccionVista(modelo: modelo, ficha: l) { guardada in
                if guardada == nil {
                    elegida = nil
                    enHoja = false
                }
            }
            .id(l.id + "\(l.tocada)")
        } else {
            ContentUnavailableView("Elige una lección o escribe una nueva", systemImage: "lightbulb")
        }
    }

    private var filtros: some View {
        HStack {
            Toggle(isOn: $modelo.soloParaRepasar) {
                Text(modelo.paraRepasar > 0 ? "Para repasar (\(modelo.paraRepasar))" : "Para repasar")
            }
            .toggleStyle(.button)
            Spacer()
            Picker("Etiqueta", selection: $modelo.etiqueta) {
                Text("Todas").tag(String?.none)
                ForEach(modelo.etiquetas, id: \.etiqueta) { e in
                    Text("#\(e.etiqueta) (\(e.cuantas))").tag(Optional(e.etiqueta))
                }
            }
            .labelsHidden()
            .frame(maxWidth: 180)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
    }
}

/// Tocar una foto o un audio de una leccion: su visor (la foto en Vista
/// Rapida, el audio en el reproductor), y compartirlo en el menu.
struct AbrirAdjuntoLeccion: ViewModifier {
    let url: URL?
    let nombre: String

    func body(content: Content) -> some View {
        if let url {
            content
                .contentShape(Rectangle())
                .onTapGesture { Navegacion.compartida.abrirFichero(url, titulo: nombre.isEmpty ? nil : nombre) }
                .contextMenu {
                    Button { Navegacion.compartida.abrirFichero(url, titulo: nombre.isEmpty ? nil : nombre) } label: {
                        Label("Abrir", systemImage: "arrow.up.forward.square")
                    }
                    ShareLink(item: url) { Label("Compartir…", systemImage: "square.and.arrow.up") }
                }
        } else {
            content
        }
    }
}
