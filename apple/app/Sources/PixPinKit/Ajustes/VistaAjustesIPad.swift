// **Ajustes** del iPad: lo de `pixpinmax.toml` que vale en todos los
// sistemas (con los mismos textos y rangos que `ventana_ajustes.rs` de
// Windows y `FilasAjustes.swift` del Mac), el tema, el nombre de este iPad,
// las conexiones del asistente, la carpeta de datos y la version.
//
// Lo de Windows y el Mac que aqui no sale: la captura de pantalla (iOS no
// deja capturar otras apps), los atajos globales, «Abrir al iniciar sesion»
// y lo del motor del asistente (lo lleva su propia pantalla).
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
    }
}

struct CuerpoAjustes: View {
    @ObservedObject var modelo: ModeloAjustes
    let sincro: Sincronizador?
    @State private var restableciendo = false

    var body: some View {
        NavigationStack {
            Form {
                if let error = modelo.error {
                    Section {
                        Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
                    }
                }
                if let sincro { SeccionNombreAjustes(sincro: sincro) }
                general
                galeria
                pdf
                sincronizar
                apariencia
                conexiones
                avanzado
            }
            .formStyle(.grouped)
            .disabled(!modelo.cargado)
            .navigationTitle("Ajustes")
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
                        Button("Restablecer todo a fábrica", role: .destructive) { restableciendo = true }
                            .disabled(!modelo.hayCambios)
                    } label: {
                        Label("Más", systemImage: "ellipsis.circle")
                    }
                }
            }
            .confirmationDialog("¿Volver a los valores de fábrica?", isPresented: $restableciendo,
                                titleVisibility: .visible) {
                Button("Restablecer", role: .destructive) { modelo.restablecer() }
                Button("Cancelar", role: .cancel) {}
            } message: {
                Text("Lo de esta pantalla vuelve a como viene PixPin. Se puede deshacer.")
            }
        }
        .task { if !modelo.cargado { await modelo.cargar() } }
    }

    // MARK: Secciones

    private var general: some View {
        Section {
            Picker(selection: enlace(\.idioma)) {
                Text("El del iPad").tag("sistema")
                Text("Español").tag("es")
                Text("English").tag("en")
            } label: {
                TituloAjuste("Idioma", "Vale desde el próximo arranque. Lo comparte con PixPin para Windows.",
                       cambiado: modelo.valores.idioma != modelo.fabrica.idioma)
            }
        } header: {
            Text("General")
        }
    }

    private var galeria: some View {
        Section {
            Stepper(value: Binding(get: { modelo.diasCaducidad }, set: { modelo.fijar(diasCaducidad: $0) }),
                    in: 0 ... 90) {
                HStack {
                    TituloAjuste("Días hasta borrar",
                           "Pasado el plazo, las capturas de la galería se van a la Papelera, salvo las conservadas.",
                           cambiado: modelo.valores.diasCaducidad != modelo.fabrica.diasCaducidad)
                    Spacer()
                    Text(Self.dias(modelo.diasCaducidad)).monospacedDigit().foregroundStyle(.secondary)
                }
            }
        } header: {
            Text("Galería")
        }
    }

    private var pdf: some View {
        let actual = modelo.pdf ?? ModeloAjustes.pdfDeFabrica
        let fabrica = ModeloAjustes.pdfDeFabrica
        let niveles: [(String, NivelAligerar)] = [("Exacto", .sinPerdida), ("Medio", .equilibrado),
                                                   ("Chico", .pequeno), ("Máx.", .extremo)]
        return Section {
            Toggle(isOn: Binding(get: { actual.aligerarAlEntrar },
                                 set: { v in modelo.cambiarPdf { $0.aligerarAlEntrar = v } })) {
                TituloAjuste("Aligerar los PDF al meterlos en un chat", "Después de meterlo al chat, en segundo plano.",
                       cambiado: actual.aligerarAlEntrar != fabrica.aligerarAlEntrar)
            }
            VStack(alignment: .leading, spacing: 8) {
                TituloAjuste("Cuánto se aligera",
                       "Al entrar solo se usan Exacto y Medio; Chico y Máx. quedan para «Aligerar el PDF» a mano.",
                       cambiado: actual.nivel != fabrica.nivel)
                Picker("Cuánto se aligera", selection: Binding(
                    get: { niveles.firstIndex { $0.1 == actual.nivel } ?? 1 },
                    set: { i in modelo.cambiarPdf { $0.nivel = niveles[i].1 } })) {
                    ForEach(niveles.indices, id: \.self) { i in Text(niveles[i].0).tag(i) }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
            }
        } header: {
            Text("Documentos PDF")
        }
    }

    private var sincronizar: some View {
        Section {
            Toggle(isOn: enlace(\.sincroPresencia)) {
                TituloAjuste("Dejarse encontrar",
                       "Tus otros aparatos ven este iPad en cuanto abren Sincronizar. Apagado no se abre ninguna puerta en la red.",
                       cambiado: modelo.valores.sincroPresencia != modelo.fabrica.sincroPresencia)
            }
            Toggle(isOn: enlace(\.sincroAutomatica)) {
                TituloAjuste("Sincronizar sola con los aparatos del grupo",
                       "Al aparecer un aparato del grupo en la Wi‑Fi, 30 s después de un cambio y cada 10 min, mientras PixPin está abierto. Se pausa en modo de bajo consumo.",
                       cambiado: modelo.valores.sincroAutomatica != modelo.fabrica.sincroAutomatica)
            }
            Toggle(isOn: enlace(\.sincroLoMioManda)) {
                TituloAjuste("Lo mío manda",
                       "Al sincronizar, lo de este iPad se impone: del otro lado solo llega lo que aquí no existe. Es como empieza el interruptor de «¿Qué sincronizar?».",
                       cambiado: modelo.valores.sincroLoMioManda != modelo.fabrica.sincroLoMioManda)
            }
        } header: {
            Text("Sincronizar")
        } footer: {
            Text("Se guarda en pixpinmax.toml, el mismo archivo de ajustes que PixPin para Windows.")
                .font(.caption).foregroundStyle(.secondary)
        }
    }

    private var apariencia: some View {
        Section {
            Picker(selection: $modelo.tema) {
                ForEach(Tema.allCases) { t in Text(t.nombre).tag(t) }
            } label: {
                TituloAjuste("Aspecto", "Claro, oscuro o como el iPad. Es de este iPad: no va en el archivo de ajustes.",
                       cambiado: modelo.tema != .sistema)
            }
            .pickerStyle(.menu)
        } header: {
            Text("Apariencia")
        }
    }

    @ViewBuilder private var conexiones: some View {
        let lista = iaConexiones()
        if !lista.isEmpty {
            Section {
                ForEach(lista, id: \.id) { c in
                    VStack(alignment: .leading, spacing: 8) {
                        TituloAjuste("Conectar \(c.nombre)",
                               "El asistente de la nube podrá buscar, leer y bajar tus archivos (y subir, preguntando). Crea el ID de cliente en \(c.consola).",
                               cambiado: false)
                        FilaConexion(id: c.id)
                    }
                    .padding(.vertical, 4)
                }
            } header: {
                Text("Conexiones del asistente")
            }
        }
    }

    private var avanzado: some View {
        Section {
            LabeledContent("Carpeta de datos") {
                Text(modelo.carpetaDatos.path)
                    .font(.caption.monospaced())
                    .multilineTextAlignment(.trailing)
                    .textSelection(.enabled)
            }
            LabeledContent("Archivo de ajustes") {
                Text((modelo.ficheroAjustes as NSString).lastPathComponent)
                    .font(.caption.monospaced())
                    .textSelection(.enabled)
            }
            LabeledContent("Versión", value: ModeloAjustes.version)
        } header: {
            Text("Avanzado")
        } footer: {
            Text("Proyectos, capturas, sincronización y el archivo de ajustes viven dentro de PixPin en este iPad, con la misma estructura que en Windows.")
                .font(.caption).foregroundStyle(.secondary)
        }
    }

    // MARK: Piezas

    /// Un valor del TOML como `Binding`: cada cambio se guarda.
    private func enlace<V: Equatable>(_ kp: WritableKeyPath<AjustesMac, V>) -> Binding<V> {
        Binding(get: { modelo.valores[keyPath: kp] }, set: { v in modelo.cambiar { $0[keyPath: kp] = v } })
    }

    nonisolated static func dias(_ n: Int) -> String { n == 0 ? "Nunca" : n == 1 ? "1 día" : "\(n) días" }
}

/// El titulo de una fila con su explicacion debajo y el punto azul de
/// Windows si no esta como de fabrica.
private struct TituloAjuste: View {
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
