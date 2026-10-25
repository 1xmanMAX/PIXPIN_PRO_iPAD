// **Ajustes** del iPad, como la ventana de Ajustes de Windows
// (`ventana_ajustes.rs`) y la del Mac (`VistaAjustes.swift`): las secciones
// con sus filas (`FilasAjustes`, datos), el **buscador** («Buscar un
// ajuste…», en todas las secciones a la vez), **«Restablecer <sección>»**,
// el punto azul de lo que no esta como de fabrica y «Deshacer» (⌘Z, hasta
// 100 pasos). Arriba, el nombre de este iPad.
//
// Lo de Windows y el Mac que aqui no sale: la captura de pantalla (iOS no
// deja capturar otras apps), los atajos globales, «Abrir al iniciar sesion»
// y las apps predeterminadas.
//
// Cada cambio se guarda al momento (en la cola del `ModeloAjustes`, el mismo
// que usa Aparatos) y se puede deshacer.

import PixPinCore
import SwiftUI
import UIKit

public struct VistaAjustesIPad: View {
    let almacen: Almacen
    let sincro: Sincronizador?

    public init(almacen: Almacen, sincro: Sincronizador?) {
        self.almacen = almacen
        self.sincro = sincro
    }

    public var body: some View {
        CuerpoAjustes(modelo: ModeloAjustes.compartido(nucleo: almacen.nucleo, sincro: sincro), sincro: sincro)
            .onAppear { FilasAjustes.almacen = almacen }
    }
}

struct CuerpoAjustes: View {
    @ObservedObject var modelo: ModeloAjustes
    let sincro: Sincronizador?
    @State private var restableciendo = false
    /// La seccion que se va a restablecer (lo pregunta antes).
    @State private var aRestablecer: SeccionAjustes?
    @State private var busqueda = ""

    private var buscando: Bool { !busqueda.trimmingCharacters(in: .whitespaces).isEmpty }

    var body: some View {
        NavigationStack {
            Form {
                if let error = modelo.error {
                    Section {
                        Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
                    }
                }
                if buscando {
                    resultados
                } else {
                    if let sincro { SeccionNombreAjustes(sincro: sincro) }
                    ForEach(SeccionAjustes.allCases) { s in seccion(s) }
                    Section {
                        Text("● = distinto del valor de fábrica.").font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
            .formStyle(.grouped)
            .disabled(!modelo.cargado)
            .navigationTitle("Ajustes")
            .searchable(text: $busqueda, placement: .navigationBarDrawer(displayMode: .always),
                        prompt: "Buscar un ajuste…")
            .toolbar {
                ToolbarItemGroup(placement: .primaryAction) {
                    Button {
                        modelo.deshacer()
                    } label: {
                        Label("Deshacer", systemImage: "arrow.uturn.backward")
                    }
                    .keyboardShortcut("z", modifiers: .command)
                    .disabled(!modelo.sePuedeDeshacer)
                    Menu {
                        ForEach(SeccionAjustes.allCases) { s in
                            Button("Restablecer \(s.titulo)") { aRestablecer = s }
                                .disabled(!FilasAjustes.hayCambios(en: s, modelo))
                        }
                        Divider()
                        Button("Restablecer todo a fábrica", role: .destructive) { restableciendo = true }
                            .disabled(!SeccionAjustes.allCases.contains { FilasAjustes.hayCambios(en: $0, modelo) })
                    } label: {
                        Label("Más", systemImage: "ellipsis.circle")
                    }
                }
            }
            .confirmationDialog("¿Volver a los valores de fábrica?", isPresented: $restableciendo,
                                titleVisibility: .visible) {
                Button("Restablecer", role: .destructive) {
                    for s in SeccionAjustes.allCases { FilasAjustes.restablecer(s, modelo) }
                }
                Button("Cancelar", role: .cancel) {}
            } message: {
                Text("Todo lo de esta pantalla vuelve a como viene PixPin. Se puede deshacer.")
            }
            .confirmationDialog(aRestablecer.map { "¿Restablecer \($0.titulo)?" } ?? "",
                                isPresented: Binding(get: { aRestablecer != nil },
                                                     set: { if !$0 { aRestablecer = nil } }),
                                titleVisibility: .visible, presenting: aRestablecer) { s in
                Button("Restablecer \(s.titulo)", role: .destructive) { FilasAjustes.restablecer(s, modelo) }
                Button("Cancelar", role: .cancel) {}
            } message: { _ in
                Text("Lo de esta sección vuelve a como viene PixPin. Se puede deshacer.")
            }
        }
        // Se relee al entrar: el asistente y Aparatos tambien escriben el TOML.
        .task { await modelo.cargar() }
    }

    // MARK: Secciones

    /// Una seccion: sus grupos en orden, con el titulo y «Restablecer» arriba.
    @ViewBuilder private func seccion(_ s: SeccionAjustes) -> some View {
        let filas = FilasAjustes.de(s)
        let grupos = gruposEnOrden(filas)
        ForEach(Array(grupos.enumerated()), id: \.offset) { i, g in
            Section {
                ForEach(filas.filter { $0.grupo == g }) { f in FilaAjusteIPad(fila: f, modelo: modelo) }
            } header: {
                if i == 0 {
                    CabeceraSeccionAjustes(seccion: s, hayCambios: FilasAjustes.hayCambios(en: s, modelo)) {
                        aRestablecer = s
                    }
                } else {
                    Text(g)
                }
            } footer: {
                if s == .sincro, i == grupos.count - 1 {
                    Text("Se guarda en pixpinmax.toml, el mismo archivo de ajustes que PixPin para Windows.")
                        .font(.caption).foregroundStyle(.secondary)
                }
            }
        }
    }

    /// «Resultados para «x»»: las filas de todas las secciones que hablan de
    /// eso, con su seccion delante.
    @ViewBuilder private var resultados: some View {
        let lista = FilasAjustes.buscar(busqueda)
        if lista.isEmpty {
            Section {
                Text("Ningún ajuste habla de «\(busqueda)». Prueba con otra palabra.")
                    .foregroundStyle(.secondary)
            }
        } else {
            Section {
                ForEach(lista) { f in
                    VStack(alignment: .leading, spacing: 4) {
                        Text("\(f.seccion.titulo) › \(f.grupo)").font(.caption2).foregroundStyle(.secondary)
                        FilaAjusteIPad(fila: f, modelo: modelo)
                    }
                }
            } header: {
                Text("Resultados para «\(busqueda)»")
            } footer: {
                Text(lista.count == 1 ? "1 ajuste encontrado en todas las secciones."
                    : "\(lista.count) ajustes encontrados en todas las secciones.")
            }
        }
    }

    private func gruposEnOrden(_ filas: [FilaAjuste]) -> [String] {
        var v: [String] = []
        for f in filas where !v.contains(f.grupo) { v.append(f.grupo) }
        return v
    }
}

/// La cabecera de una seccion: icono, titulo, lo que hay dentro y
/// «Restablecer» si algo no esta como de fabrica.
private struct CabeceraSeccionAjustes: View {
    let seccion: SeccionAjustes
    let hayCambios: Bool
    let restablecer: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Label(seccion.titulo, systemImage: seccion.icono)
                    .font(.headline)
                    .foregroundStyle(.primary)
                Spacer()
                if hayCambios {
                    Button("Restablecer \(seccion.titulo)", action: restablecer)
                        .font(.caption)
                        .buttonStyle(.borderless)
                }
            }
            Text(seccion.subtitulo).font(.caption).foregroundStyle(.secondary)
                .textCase(nil)
        }
        .textCase(nil)
        .padding(.top, 8)
    }
}

/// Una fila: el control de su clase con el titulo, la explicacion y el
/// punto azul. Mantener pulsado: «Volver al valor de fábrica».
struct FilaAjusteIPad: View {
    let fila: FilaAjuste
    @ObservedObject var modelo: ModeloAjustes

    private var titulo: TituloAjuste {
        TituloAjuste(fila.titulo, fila.ayuda, cambiado: fila.restablecible && fila.cambiada(modelo))
    }

    var body: some View {
        control
            .contextMenu {
                if fila.restablecible, fila.cambiada(modelo) {
                    Button("Volver al valor de fábrica") { FilasAjustes.restablecer(fila, modelo) }
                }
            }
    }

    @ViewBuilder private var control: some View {
        switch fila.control {
        case let .interruptor(get, set):
            Toggle(isOn: Binding(get: { get(modelo) }, set: { set(modelo, $0) })) { titulo }
        case let .opcion(opciones, get, set):
            let enlace = Binding(get: { get(modelo) }, set: { set(modelo, $0) })
            if opciones.count <= 4, opciones.allSatisfy({ $0.count <= 12 }) {
                VStack(alignment: .leading, spacing: 8) {
                    titulo
                    Picker(fila.titulo, selection: enlace) {
                        ForEach(opciones.indices, id: \.self) { i in Text(opciones[i]).tag(i) }
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                }
            } else {
                Picker(selection: enlace) {
                    ForEach(opciones.indices, id: \.self) { i in Text(opciones[i]).tag(i) }
                } label: {
                    titulo
                }
                .pickerStyle(.menu)
            }
        case let .numero(rango, paso, texto, get, set):
            Stepper(value: Binding(get: { get(modelo) }, set: { set(modelo, $0) }), in: rango, step: paso) {
                HStack {
                    titulo
                    Spacer()
                    Text(texto(get(modelo))).monospacedDigit().foregroundStyle(.secondary)
                }
            }
        case let .vista(hacer):
            VStack(alignment: .leading, spacing: 8) {
                if !fila.titulo.isEmpty { titulo }
                hacer(modelo)
            }
            .padding(.vertical, 2)
        case .nada:
            titulo
        }
    }
}

/// El titulo de una fila con su explicacion debajo y el punto azul de
/// Windows si no esta como de fabrica.
struct TituloAjuste: View {
    let titulo: String
    let ayuda: String
    let cambiado: Bool

    init(_ titulo: String, _ ayuda: String, cambiado: Bool) {
        self.titulo = titulo
        self.ayuda = ayuda
        self.cambiado = cambiado
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 6) {
                Text(titulo)
                if cambiado {
                    Circle().fill(Paleta.v2.acento).frame(width: 7, height: 7)
                        .accessibilityLabel("Distinto del valor de fábrica")
                }
            }
            if !ayuda.isEmpty {
                Text(ayuda).font(.caption).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// «Nombre de este iPad»: como lo ven los demas del grupo (y las personas
/// de los chats compartidos, si no se puso otro alli).
private struct SeccionNombreAjustes: View {
    @ObservedObject var sincro: Sincronizador
    @State private var cambiando = false

    var body: some View {
        Section {
            HStack {
                VStack(alignment: .leading, spacing: 3) {
                    Text(sincro.aparato?.nombre ?? nombreDeEsteAparato())
                    Text("Es como lo verán tus otros aparatos.").font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                Button("Cambiar…") { cambiando = true }
                    .disabled(sincro.ocupado)
            }
        } header: {
            Text("Este iPad")
        }
        .task { if sincro.aparato == nil { await sincro.refrescar() } }
        .sheet(isPresented: $cambiando) {
            HojaNombre(nombre: sincro.aparato?.nombre ?? nombreDeEsteAparato()) { nuevo in
                Task { await sincro.renombrar(nuevo) }
            }
        }
    }
}
