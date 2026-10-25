// **El editor de una automatizacion** (tanda E6, solo del Mac: en Windows se
// crean pidiendoselas al asistente). Lo mismo que lleva el fichero: nombre,
// disparador (al capturar, a una hora con sus dias, al sincronizar, a mano)
// y hasta 10 pasos, cada uno una accion de la tabla del asistente (con sus
// argumentos, el chat y las imagenes si la accion los entiende) o un
// pedido al asistente. Las variables (`{captura}`, `{resultado}`…) van en
// cualquier texto.
//
// Lo que vale lo dice el nucleo (`automatizacion_revisar`, la misma
// `validar` que al leer el fichero): el motivo sale abajo y «Guardar» no se
// deja pulsar mientras haya uno.

import PixPinCore
import SwiftUI

struct EditorAutomatizacion: View {
    @State private var ficha: FichaAutomatizacion
    let nucleo: Nucleo
    let guardar: (FichaAutomatizacion) async -> String?
    @State private var motivo: String?
    @State private var guardando = false
    @Environment(\.dismiss) private var cerrar

    /// Las acciones que se pueden poner en un paso (la tabla del asistente).
    private let herramientas = automatizacionHerramientas()

    init(ficha: FichaAutomatizacion, nucleo: Nucleo, guardar: @escaping (FichaAutomatizacion) async -> String?) {
        _ficha = State(initialValue: ficha)
        self.nucleo = nucleo
        self.guardar = guardar
    }

    /// Una nueva: a mano, con un pedido al asistente vacio.
    static func nueva() -> FichaAutomatizacion {
        FichaAutomatizacion(id: "", nombre: "", activa: true, disparador: "a_mano", hora: "08:00", dias: Data(),
                            pasos: [pasoAsistente()], detalle: "", otroAparato: nil, rota: nil, ultimaVez: nil)
    }

    static func pasoAsistente() -> PasoAutomatizacion {
        PasoAutomatizacion(esAsistente: true, herramienta: "", args: [], pedido: "", proyecto: nil, imagenes: [])
    }

    static let disparadores: [(String, String)] = [
        ("al_capturar", "Al capturar"), ("a_la_hora", "A una hora"),
        ("al_sincronizar", "Al sincronizar"), ("a_mano", "A mano"),
    ]
    static let dias = ["L", "M", "X", "J", "V", "S", "D"]
    static let maximoPasos = 10

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(ficha.id.isEmpty ? "Nueva automatización" : "Cambiar automatización")
                    .font(.system(size: 17, weight: .bold))
                    .foregroundStyle(Paleta.v2.texto)
                Spacer()
            }
            .padding(.horizontal, 20)
            .frame(height: 56)
            .overlay(alignment: .bottom) { Paleta.v2.linea.frame(height: 1) }
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    seccion("Nombre") {
                        TextField("Capturas a la obra con OCR", text: $ficha.nombre)
                            .textFieldStyle(.roundedBorder)
                    }
                    seccion("Cuándo") {
                        SegmentosCabecera(Self.disparadores, eleccion: $ficha.disparador)
                        if ficha.disparador == "a_la_hora" { cuandoALaHora }
                        Text(ayudaDisparador)
                            .font(.system(size: 12))
                            .foregroundStyle(Paleta.v2.gris)
                        Toggle("Encendida", isOn: $ficha.activa)
                            .toggleStyle(.switch)
                    }
                    seccion("Pasos (en orden; el primero que falla para los demás)") {
                        ForEach(ficha.pasos.indices, id: \.self) { i in
                            tarjetaPaso(i)
                        }
                        HStack {
                            Button("Añadir pedido al asistente") { ficha.pasos.append(Self.pasoAsistente()) }
                            Button("Añadir acción") {
                                ficha.pasos.append(PasoAutomatizacion(esAsistente: false, herramienta: "escribir_en_chat",
                                                                      args: [], pedido: "", proyecto: nil, imagenes: []))
                            }
                        }
                        .disabled(ficha.pasos.count >= Self.maximoPasos)
                        Text("Variables: {captura} (la captura nueva), {resultado} (lo que contestó el paso anterior), {fecha}, {hora}, {sincro} (lo que trajo la sincronización), {nombre}.")
                            .font(.system(size: 12))
                            .foregroundStyle(Paleta.v2.gris)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                .padding(20)
            }
            HStack {
                if let motivo {
                    Label(motivo, systemImage: "exclamationmark.triangle")
                        .font(.system(size: 12))
                        .foregroundStyle(Paleta.v2.naranja)
                        .lineLimit(2)
                }
                Spacer()
                Button("Cancelar") { cerrar() }
                    .keyboardShortcut(.cancelAction)
                Button("Guardar") {
                    guardando = true
                    Task {
                        let error = await guardar(ficha)
                        guardando = false
                        if let error { motivo = error } else { cerrar() }
                    }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(motivo != nil || guardando)
            }
            .padding(16)
            .overlay(alignment: .top) { Paleta.v2.linea.frame(height: 1) }
        }
        .frame(width: 620, height: 640)
        .background(Paleta.v2.fondo)
        .environment(\.colorScheme, .dark)
        .onAppear(perform: revisar)
        .onChange(of: ficha) { revisar() }
    }

    private func revisar() {
        motivo = nucleo.automatizacionRevisar(ficha: ficha)
    }

    private var ayudaDisparador: String {
        switch ficha.disparador {
        case "al_capturar": "Al guardarse una captura nueva en la carpeta de capturas. No puede capturar (sería un bucle)."
        case "a_la_hora": "A esa hora de este Mac. Si estaba apagado, corre al encenderse si no han pasado 6 horas."
        case "al_sincronizar": "Al acabar una vuelta de la sincronización que trajo algo. No puede sincronizar."
        default: "Solo cuando se pide: «Correr ahora» o el pedido `automatizacion`."
        }
    }

    /// La hora y los dias (ninguno marcado: todos).
    private var cuandoALaHora: some View {
        HStack(spacing: 12) {
            TextField("08:00", text: $ficha.hora)
                .textFieldStyle(.roundedBorder)
                .frame(width: 80)
            ForEach(1...7, id: \.self) { d in
                let marcado = ficha.dias.contains(UInt8(d))
                Button(Self.dias[d - 1]) {
                    if marcado { ficha.dias.removeAll { $0 == UInt8(d) } } else { ficha.dias.append(UInt8(d)) }
                }
                .buttonStyle(.plain)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(marcado ? Color.white : Paleta.v2.cuerpo)
                .frame(width: 28, height: 28)
                .background(marcado ? Paleta.v2.azulLleno : Paleta.v2.caja, in: Circle())
                .help(marcado ? "Quitar este día" : "Solo estos días (ninguno: todos)")
            }
        }
    }

    // MARK: Un paso

    private func tarjetaPaso(_ i: Int) -> some View {
        let paso = $ficha.pasos[i]
        let fila = herramientas.first { $0.nombre == paso.wrappedValue.herramienta }
        return VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Paso \(i + 1)").font(.system(size: 13, weight: .semibold)).foregroundStyle(Paleta.v2.texto)
                Picker("", selection: paso.esAsistente) {
                    Text("Pedido al asistente").tag(true)
                    Text("Acción").tag(false)
                }
                .labelsHidden()
                .frame(width: 190)
                Spacer()
                BotonCabecera(icono: "arrow.up", ayuda: "Subir", tenue: true) { mover(i, -1) }
                    .disabled(i == 0)
                BotonCabecera(icono: "arrow.down", ayuda: "Bajar", tenue: true) { mover(i, 1) }
                    .disabled(i + 1 >= ficha.pasos.count)
                BotonCabecera(icono: "trash", ayuda: "Quitar este paso", tenue: true) { ficha.pasos.remove(at: i) }
            }
            if paso.wrappedValue.esAsistente {
                TextField("Resume mis tareas pendientes", text: paso.pedido, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(2...6)
                campoProyecto(paso)
                campoImagenes(paso)
            } else {
                Picker("Acción", selection: paso.herramienta) {
                    if fila == nil, !paso.wrappedValue.herramienta.isEmpty {
                        Text(paso.wrappedValue.herramienta).tag(paso.wrappedValue.herramienta)
                    }
                    ForEach(herramientas, id: \.nombre) { h in
                        Text(h.nombre).tag(h.nombre)
                    }
                }
                if let fila {
                    Text(fila.descripcion)
                        .font(.system(size: 12))
                        .foregroundStyle(Paleta.v2.gris)
                        .fixedSize(horizontal: false, vertical: true)
                    ForEach(fila.params, id: \.nombre) { p in
                        campoArg(paso, p)
                    }
                    if fila.conProyecto { campoProyecto(paso) }
                    if fila.conImagenes { campoImagenes(paso) }
                }
            }
        }
        .padding(12)
        .background(Paleta.v2.tarjeta, in: RoundedRectangle(cornerRadius: 12))
    }

    private func mover(_ i: Int, _ d: Int) {
        let j = i + d
        guard ficha.pasos.indices.contains(j) else { return }
        ficha.pasos.swapAt(i, j)
    }

    private func campoArg(_ paso: Binding<PasoAutomatizacion>, _ p: ParamAutomatizacion) -> some View {
        let valor = Binding<String>(
            get: { paso.wrappedValue.args.first { $0.nombre == p.nombre }?.valor ?? "" },
            set: { nuevo in
                if let k = paso.wrappedValue.args.firstIndex(where: { $0.nombre == p.nombre }) {
                    paso.wrappedValue.args[k].valor = nuevo
                } else {
                    paso.wrappedValue.args.append(ArgAutomatizacion(nombre: p.nombre, valor: nuevo))
                }
            })
        return HStack(alignment: .firstTextBaseline) {
            Text(p.nombre + (p.obligatorio ? " *" : ""))
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Paleta.v2.cuerpo)
                .frame(width: 110, alignment: .trailing)
            if p.opciones.isEmpty {
                TextField(p.descripcion, text: valor)
                    .textFieldStyle(.roundedBorder)
            } else {
                Picker("", selection: valor) {
                    Text("—").tag("")
                    ForEach(p.opciones, id: \.self) { Text($0).tag($0) }
                }
                .labelsHidden()
            }
        }
    }

    private func campoProyecto(_ paso: Binding<PasoAutomatizacion>) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text("chat")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Paleta.v2.cuerpo)
                .frame(width: 110, alignment: .trailing)
            TextField("nombre del chat (vacío: el de siempre)", text: Binding(
                get: { paso.wrappedValue.proyecto ?? "" },
                set: { paso.wrappedValue.proyecto = $0.isEmpty ? nil : $0 }))
                .textFieldStyle(.roundedBorder)
        }
    }

    private func campoImagenes(_ paso: Binding<PasoAutomatizacion>) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text("imágenes")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Paleta.v2.cuerpo)
                .frame(width: 110, alignment: .trailing)
            TextField("{captura} o rutas, separadas por comas", text: Binding(
                // Tal cual se escribe (con sus espacios): el nucleo recorta y
                // quita las vacias al guardar.
                get: { paso.wrappedValue.imagenes.joined(separator: ",") },
                set: { t in
                    paso.wrappedValue.imagenes = t.isEmpty ? []
                        : t.split(separator: ",", omittingEmptySubsequences: false).map(String.init)
                }))
                .textFieldStyle(.roundedBorder)
        }
    }

    @ViewBuilder
    private func seccion(_ titulo: String, @ViewBuilder _ contenido: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(titulo)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(Paleta.v2.gris)
            contenido()
        }
    }
}
