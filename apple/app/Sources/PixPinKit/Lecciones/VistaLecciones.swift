// La ventana de Lecciones: a la izquierda la lista (buscar, filtrar por
// etiqueta, «para repasar») por paginas; a la derecha la ficha para
// escribirla o repasarla. Como la pestana «Lecciones» del timeline de
// Windows y la ficha «Nueva leccion».
//
// iPad: del Mac; los adjuntos se abren con `AbrirAdjuntoLeccion`
// (`VistaLeccionesIPad.swift`) en vez de las acciones de fichero del chat.

import PixPinCore
import SwiftUI

public struct VentanaLecciones: View {
    @StateObject private var modelo: ModeloLecciones
    @State private var elegida: String?
    @State private var nueva = false

    public init(nucleo: Nucleo) {
        _modelo = StateObject(wrappedValue: ModeloLecciones(nucleo: nucleo))
    }

    public var body: some View {
        VStack(spacing: 0) {
            // La barra comun de Windows (`Cabecera`): buscar y escribir una
            // nueva.
            Cabecera("Lecciones", subtitulo: modelo.total == 1 ? "1 lección" : "\(modelo.total) lecciones",
                     buscador: BuscadorCabecera($modelo.texto, pista: "Buscar: palabra, situación, etiqueta…")) {
                BotonCabecera(icono: "plus", rotulo: "Nueva lección") {
                    elegida = nil
                    nueva = true
                }
                .keyboardShortcut("n")
            }
            dividida
        }
        .ventanaConCabecera()
    }

    private var dividida: some View {
        NavigationSplitView {
            VStack(spacing: 0) {
                filtros
                List(selection: $elegida) {
                    ForEach(modelo.lecciones, id: \.id) { l in
                        FilaLeccion(leccion: l)
                            .tag(l.id)
                            .onAppear {
                                if l.id == modelo.lecciones.last?.id {
                                    Task { await modelo.cargarMas() }
                                }
                            }
                    }
                }
                .overlay {
                    if modelo.lecciones.isEmpty && !modelo.cargando {
                        let titulo: String = modelo.texto.isEmpty ? "Sin lecciones" : "Nada encontrado"
                        ContentUnavailableView { Label(titulo, systemImage: "lightbulb") }
                    }
                }
            }
            .navigationSplitViewColumnWidth(min: 260, ideal: 320)
        } detail: {
            if nueva {
                FichaLeccionVista(modelo: modelo, ficha: nil) { guardada in
                    nueva = false
                    elegida = guardada?.id
                }
                .id("nueva")
            } else if let id = elegida, let l = modelo.lecciones.first(where: { $0.id == id }) {
                FichaLeccionVista(modelo: modelo, ficha: l) { _ in }
                    .id(l.id + "\(l.tocada)")
            } else {
                ContentUnavailableView("Elige una lección o escribe una nueva", systemImage: "lightbulb")
            }
        }
        .toolbar(removing: .sidebarToggle)
        .onChange(of: elegida) { _, v in if v != nil { nueva = false } }
        .task { await modelo.recargar() }
        .overlay(alignment: .bottom) {
            if let error = modelo.error {
                Text(error).font(.caption).padding(6)
                    .background(.red.opacity(0.15), in: RoundedRectangle(cornerRadius: 6))
                    .padding()
            }
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
            .frame(maxWidth: 160)
        }
        .padding(8)
    }
}

struct FilaLeccion: View {
    let leccion: FichaLeccion

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(alignment: .firstTextBaseline) {
                Text(leccion.icono)
                Text(leccion.titulo).lineLimit(2)
                Spacer(minLength: 0)
                if leccion.veces > 1 {
                    Text("🔁\(leccion.veces)").font(.caption)
                }
                if leccion.tocaRepasar {
                    Image(systemName: "clock.badge.exclamationmark").foregroundStyle(.orange)
                        .help("Toca repasarla")
                }
            }
            let etiquetas = leccion.etiquetas + leccion.etiquetasAuto
            if !etiquetas.isEmpty {
                Text(etiquetas.map { "#\($0)" }.joined(separator: " "))
                    .font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
        }
        .padding(.vertical, 2)
    }
}

/// La ficha: escribir una nueva, editar una que existe y repasarla.
struct FichaLeccionVista: View {
    @ObservedObject var modelo: ModeloLecciones
    let ficha: FichaLeccion?
    /// Lo que trae escrito una nueva (la de «Hacer lección» de un mensaje).
    var inicial: BorradorLeccion?
    let alAcabar: (FichaLeccion?) -> Void

    @State private var b = BorradorLeccion()
    @State private var preguntarBorrar = false
    @State private var guardando = false
    @State private var proponer: Task<Void, Never>?

    var body: some View {
        Form {
            Section("Lo que aprendí") {
                TextField("En una frase (lo único obligatorio)", text: $b.titulo, axis: .vertical)
                    .lineLimit(1 ... 3)
                Picker("Tipo", selection: $b.tipo) {
                    ForEach(tiposDeLeccion) { t in Text(t.nombre).tag(t.id) }
                }
                Picker("Gravedad", selection: $b.gravedad) {
                    Text("Leve").tag(Int64(1))
                    Text("Importante").tag(Int64(2))
                    Text("Grave").tag(Int64(3))
                }
                .pickerStyle(.segmented)
            }
            Section("Detalle") {
                TextField("Qué pasó", text: $b.quePaso, axis: .vertical).lineLimit(1 ... 5)
                TextField("Por qué pasó", text: $b.porQue, axis: .vertical).lineLimit(1 ... 5)
                TextField("La próxima vez (si…, entonces…)", text: $b.proxima, axis: .vertical).lineLimit(1 ... 5)
                Picker("Área", selection: $b.area) {
                    Text("Sin área").tag("")
                    ForEach(areasDeLeccion, id: \.self) { Text($0).tag($0) }
                    if !b.area.isEmpty && !areasDeLeccion.contains(b.area) {
                        Text(b.area).tag(b.area)
                    }
                }
            }
            Section("Etiquetas") {
                TextField("Etiquetas (separadas por comas)", text: $b.etiquetasEscritas)
                if !b.etiquetasAuto.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Propuestas ✨").font(.caption).foregroundStyle(.secondary)
                        HStack {
                            ForEach(b.etiquetasAuto, id: \.self) { e in
                                HStack(spacing: 2) {
                                    Button("#\(e)") { b.aceptar(e) }
                                        .buttonStyle(.borderless)
                                        .help("Ponerla")
                                    Button { b.quitar(e) } label: { Image(systemName: "xmark.circle.fill") }
                                        .buttonStyle(.borderless)
                                        .foregroundStyle(.secondary)
                                        .help("Quitarla: no vuelve a salir")
                                }
                            }
                        }
                    }
                }
                DisclosureGroup("Causas (\(b.causas.count))") {
                    ForEach(causasDeLeccion, id: \.self) { c in
                        Toggle(c, isOn: Binding(
                            get: { b.causas.contains(c) },
                            set: { on in
                                if on { if !b.causas.contains(c) { b.causas.append(c) } } else { b.causas.removeAll { $0 == c } }
                            }
                        ))
                    }
                }
            }
            if let f = ficha {
                AdjuntosDeLeccion(nucleo: modelo.nucleo, leccion: f.id)
                Section("Repaso") {
                    Text(textoDeRepaso(f))
                        .foregroundStyle(f.tocaRepasar ? Color.orange : Color.secondary)
                    HStack {
                        Button("Lo recordaba") { repasar(.recordaba) }
                        Button("A medias") { repasar(.aMedias) }
                        Button("Lo olvidé") { repasar(.olvide) }
                        Spacer()
                        Button("Volvió a pasar") { repasar(.repetida) }
                    }
                    if f.veces > 1 {
                        Text("Pasó \(f.veces) veces").font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
        }
        .formStyle(.grouped)
        // Borrar y guardar, en la barra de abajo de la ficha, como en
        // Windows (la de arriba es la comun de la ventana).
        .safeAreaInset(edge: .bottom, spacing: 0) {
            HStack(spacing: 10) {
                Spacer()
                if ficha != nil {
                    BotonCabecera(icono: "trash", rotulo: "Borrar") { preguntarBorrar = true }
                }
                BotonCabecera(icono: "checkmark", rotulo: ficha == nil ? "Guardar lección" : "Guardar cambios",
                              ayuda: "Guardar (⌘S)", activo: true) { guardar() }
                    .keyboardShortcut("s")
                    .disabled(b.titulo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || guardando)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(Paleta.v2.fondo)
            .overlay(alignment: .top) { Paleta.v2.linea.frame(height: 1) }
        }
        .confirmationDialog("¿Borrar esta lección?", isPresented: $preguntarBorrar) {
            Button("Borrar", role: .destructive) {
                guard let id = ficha?.id else { return }
                Task {
                    await modelo.borrar(id)
                    alAcabar(nil)
                }
            }
        } message: {
            Text("Se borra también en los demás aparatos al sincronizar.")
        }
        .onAppear {
            if let ficha { b = BorradorLeccion(ficha) } else if let inicial { b = inicial }
        }
        .onChange(of: b.textoEntero) { _, _ in programarPropuesta() }
        .onDisappear { proponer?.cancel() }
    }

    /// Las etiquetas propuestas, tras una pausa al escribir (sin pisar lo
    /// que el usuario ya puso o quito).
    private func programarPropuesta() {
        proponer?.cancel()
        let texto = b.textoEntero
        let quitadas = b.quitadas
        proponer = Task {
            try? await Task.sleep(nanoseconds: 400_000_000)
            guard !Task.isCancelled, !texto.isEmpty else { return }
            let p = await modelo.proponer(texto: texto, quitadas: quitadas)
            guard !Task.isCancelled else { return }
            b.etiquetasAuto = p.etiquetas.filter { !b.etiquetas.contains($0) && !b.quitadas.contains($0) }
            if ficha == nil {
                if b.area.isEmpty, let a = p.area { b.area = a }
                if b.tipo == "leccion", let t = p.tipo { b.tipo = t }
            }
        }
    }

    private func guardar() {
        guardando = true
        let editada = b.editada
        Task {
            let f = await modelo.guardar(editada)
            guardando = false
            if f != nil { alAcabar(f) }
        }
    }

    private func textoDeRepaso(_ f: FichaLeccion) -> String {
        if f.tocaRepasar { return "Toca repasarla hoy." }
        let cuando = Date(timeIntervalSince1970: Double(f.repasar) / 1000)
        return "Próximo repaso: " + cuando.formatted(date: .abbreviated, time: .omitted)
    }

    private func repasar(_ nota: NotaRepaso) {
        guard let id = ficha?.id else { return }
        Task { _ = await modelo.repasar(id, nota: nota) }
    }
}

/// **Las fotos y audios de una leccion** (`AdjuntosDeLaLeccion.kt`): la foto
/// del error en la obra, la nota de voz con lo que dijo el profesor. Los
/// pone el movil (o Windows) como mensajes que le responden; aqui se ven y
/// se abren (doble clic: Vista Rapida). Sin ninguno, no sale la seccion.
struct AdjuntosDeLeccion: View {
    let nucleo: Nucleo
    let leccion: String
    @State private var adjuntos: [AdjuntoLeccion] = []

    var body: some View {
        Group {
            if !adjuntos.isEmpty {
                Section("Fotos y audios (\(adjuntos.count))") {
                    ScrollView(.horizontal) {
                        HStack(alignment: .top, spacing: 10) {
                            ForEach(adjuntos, id: \.mensaje.id) { a in celda(a) }
                        }
                        .padding(.vertical, 4)
                    }
                }
            }
        }
        .task(id: leccion) {
            let (nucleo, id) = (nucleo, leccion)
            adjuntos = await Task.detached(priority: .utility) {
                (try? nucleo.adjuntosDeLeccion(id: id)) ?? []
            }.value
        }
    }

    @ViewBuilder
    private func celda(_ a: AdjuntoLeccion) -> some View {
        let m = a.mensaje
        let url = a.fichero.map { URL(fileURLWithPath: $0) }
        if m.clase == "IMAGEN" {
            Group {
                if let url {
                    MiniaturaVista(url: url, lado: 110).modifier(AbrirAdjuntoLeccion(url: url, nombre: m.nombre))
                } else {
                    Label("Aún no ha llegado", systemImage: "photo").font(.caption).foregroundStyle(.secondary)
                        .frame(width: 110, height: 80)
                }
            }
            .help(m.texto.isEmpty ? m.nombre : m.texto)
        } else {
            VStack(alignment: .leading, spacing: 2) {
                Label(textoDeVoz(m), systemImage: "waveform")
                if let t = m.transcripcion, !t.isEmpty {
                    Text(t).font(.caption).foregroundStyle(.secondary).lineLimit(3)
                }
            }
            .frame(width: 160, alignment: .leading)
            .padding(6)
            .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 8))
            .modifier(AbrirAdjuntoLeccion(url: url, nombre: m.nombre))
        }
    }

    private func textoDeVoz(_ m: MensajeChat) -> String {
        guard m.duracionMs > 0 else { return "Nota de voz" }
        let s = Int(m.duracionMs / 1000)
        return String(format: "Nota de voz · %d:%02d", s / 60, s % 60)
    }
}
