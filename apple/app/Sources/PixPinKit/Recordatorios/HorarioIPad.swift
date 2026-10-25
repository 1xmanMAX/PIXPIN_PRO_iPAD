// **El horario de clases semanal** en el iPad (Windows `horario/ventana.rs`,
// Mac `Recordatorios/VentanaHorario.swift` y `HorarioClases.swift`).
//
// - A la izquierda, las **asignaturas**: la caja «Nueva asignatura» (con el
//   aula detras si se quiere: `Geometría @A-12`), el circulo de color de
//   cada una (tocarlo pasa al siguiente color) y su aula. Mantenerla
//   pulsada: arrastrarla a la semana, o su menu (Borrar). Debajo, las
//   proximas llamadas (las clases y los eventos de Google Calendar).
// - A la derecha, la **semana**: lunes a sabado (el domingo con su boton),
//   de 7:00 a 22:00 en franjas de 15 min, con hoy y la hora de ahora
//   marcados. **Con el dedo**: arrastrar una asignatura a la semana pone
//   una clase de 1 h; o tocar la asignatura y luego el hueco. Una clase se
//   mueve arrastrandola (mantener y mover) y se alarga o se acorta tirando
//   del asa de abajo, en saltos de 15 min. Su menu la borra (con Deshacer)
//   o le cambia el color.
// - Arriba: «Nota de voz para la próxima clase» (la oye la llamada de esa
//   clase) y «Editar como texto…» (el mismo `horario.md`).
//
// Todo se guarda al momento en `horario.md` (`SemanaHorario` del nucleo) y
// se relee la agenda del vigia: las llamadas salen de lo mismo que se ve.

import AVFoundation
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

// MARK: - Abrir

@MainActor
public enum HorarioIPad {
    /// Abre el horario semanal a pantalla completa, encima de lo que haya.
    public static func abrir(nucleo: Nucleo) {
        ArribaUIKit.presentar(estilo: .fullScreen) { cerrar in
            VistaHorarioIPad(nucleo: nucleo, cerrar: cerrar)
        }
    }

    /// **«Nota de voz para la próxima clase»**: graba para la clase en
    /// marcha (o la ultima de hoy, o la proxima).
    public static func grabarParaLaProxima(nucleo: Nucleo) {
        guard let nota = nucleo.notaDeVozDeClase(desfaseMs: desfaseLocalMs()) else {
            CentroAvisos.compartido.mostrarEnLaApp(
                Aviso(id: "horario", categoria: .mini, titulo: "Horario",
                      texto: "El horario no tiene clases: añade una asignatura y ponla en la semana"),
                sonar: false)
            return
        }
        let g = GrabadoraDeClase()
        ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 380, height: 220)) { cerrar in
            VistaGrabadoraDeClase(g: g, cerrar: cerrar)
        }
        g.empezar(nota)
    }
}

// MARK: - Lo puro de la rejilla

/// Donde cae cada dia y cada franja de 15 min, y al reves (la `Rejilla` de
/// `horario/semana.rs`).
struct ReglasHorario: Equatable {
    /// De 7:00 a 22:00, en minutos del dia.
    static let desde = 7 * 60
    static let hasta = 22 * 60
    static let paso = 15
    static let dias = ["Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom"]

    /// Cuantos dias se ven (6, o 7 con el domingo).
    var cuantosDias: Int
    /// El ancho de un dia y el alto de una hora, en puntos.
    var anchoDia: Double
    var altoHora: Double

    var altoTotal: Double { Double(Self.hasta - Self.desde) / 60 * altoHora }

    /// La y de un minuto del dia.
    func y(_ minuto: Int) -> Double { Double(minuto - Self.desde) / 60 * altoHora }

    /// El minuto bajo una y, **redondeado abajo** a su franja de 15 min y
    /// dentro de la rejilla.
    func minuto(enY y: Double) -> Int {
        let m = Self.desde + Int((y / altoHora * 60).rounded(.down))
        let abajo = Int((Double(m) / Double(Self.paso)).rounded(.down)) * Self.paso
        return min(max(abajo, Self.desde), Self.hasta - Self.paso)
    }

    /// El minuto mas cercano a una y, en saltos de 15 min (el borde de abajo
    /// al alargar).
    func minutoCercano(enY y: Double) -> Int {
        let m = Double(Self.desde) + y / altoHora * 60
        let r = Int((m / Double(Self.paso)).rounded()) * Self.paso
        return min(max(r, Self.desde), Self.hasta)
    }

    /// El dia (0 el lunes) bajo una x.
    func dia(enX x: Double) -> Int {
        min(max(Int((x / anchoDia).rounded(.down)), 0), cuantosDias - 1)
    }

    /// «8:00», «10:15».
    static func hora(_ minuto: Int) -> String {
        String(format: "%d:%02d", minuto / 60, minuto % 60)
    }

    /// «8:00–10:15».
    static func tramo(_ inicio: Int, _ fin: Int) -> String { "\(hora(inicio))–\(hora(fin))" }

    /// El dia de la semana de hoy (0 el lunes), como los del horario.
    static func hoy(_ fecha: Date = Date(), calendario: Calendar = .current) -> Int {
        (calendario.component(.weekday, from: fecha) + 5) % 7
    }

    /// El minuto del dia de ahora.
    static func ahora(_ fecha: Date = Date(), calendario: Calendar = .current) -> Int {
        let c = calendario.dateComponents([.hour, .minute], from: fecha)
        return (c.hour ?? 0) * 60 + (c.minute ?? 0)
    }
}

// MARK: - El modelo

@MainActor
final class ModeloSemana: ObservableObject {
    let nucleo: Nucleo
    private var semana: SemanaHorario?
    @Published private(set) var asignaturas: [AsignaturaHorario] = []
    @Published private(set) var bloques: [BloqueHorario] = []
    @Published private(set) var proximas: [AvisoHorario] = []
    @Published var error: String?
    /// Lo que se puede deshacer (lo ultimo que se borro) y como estaba el
    /// horario antes.
    @Published private(set) var deshacer: String?
    private var antes: String?

    init(nucleo: Nucleo) {
        self.nucleo = nucleo
        abrir()
    }

    private func abrir() {
        do {
            semana = try SemanaHorario.abrir(nucleo: nucleo)
            error = nil
        } catch {
            semana = nil
            self.error = "No se pudo abrir el horario: \(error)"
        }
        releer()
    }

    func releer() {
        asignaturas = semana?.asignaturas() ?? []
        bloques = semana?.bloques() ?? []
        let nucleo = self.nucleo
        let d = desfaseLocalMs()
        Task {
            let v = await Task.detached(priority: .utility) { nucleo.avisosDelHorario(desfaseMs: d) }.value
            self.proximas = Array(v.prefix(8))
        }
    }

    /// Lo escrito en `horario.md` (para deshacer y para editarlo a mano).
    func texto() -> String? {
        guard let ruta = try? nucleo.ficheroDelHorario() else { return nil }
        return try? String(contentsOfFile: ruta, encoding: .utf8)
    }

    /// Escribe `horario.md` entero (Deshacer, «Editar como texto») y lo
    /// vuelve a abrir.
    func escribir(_ texto: String) {
        do {
            let ruta = try nucleo.ficheroDelHorario()
            try texto.write(toFile: ruta, atomically: true, encoding: .utf8)
        } catch {
            self.error = "No se pudo guardar el horario: \(error)"
        }
        abrir()
        Recordatorios.compartido.releer()
    }

    /// Hace un cambio, lo guarda y avisa al vigia. `deshacible`: lo que se
    /// dice en «Deshacer» (borrar).
    func cambiar(deshacible: String? = nil, _ hacer: (SemanaHorario) -> Void) {
        guard let semana else { return }
        let previo = deshacible == nil ? nil : texto()
        hacer(semana)
        do {
            try semana.guardar()
            Recordatorios.compartido.releer()
            if let deshacible, let previo {
                antes = previo
                deshacer = deshacible
            }
        } catch {
            self.error = "No se pudo guardar el horario: \(error)"
        }
        releer()
    }

    func hacerDeshacer() {
        guard let antes else { return }
        self.antes = nil
        deshacer = nil
        escribir(antes)
    }

    func olvidarDeshacer() {
        antes = nil
        deshacer = nil
    }

    /// El indice de la asignatura de un bloque (para su color).
    func asignatura(de b: BloqueHorario) -> Int? {
        asignaturas.firstIndex { $0.nombre == b.nombre }
    }
}

// MARK: - La vista

struct VistaHorarioIPad: View {
    @StateObject private var modelo: ModeloSemana
    let cerrar: () -> Void
    @State private var nueva = ""
    @AppStorage("PixPinHorarioDomingo") private var conDomingo = false
    /// La asignatura tocada: el siguiente toque en la semana pone ahi una
    /// clase (lo de arrastrar, con un dedo y sin arrastrar).
    @State private var elegida: Int?
    /// El bloque que se esta alargando y donde va su borde.
    @State private var alargando: (Int, Int)?
    @State private var editandoTexto = false
    @Environment(\.horizontalSizeClass) private var clase

    init(nucleo: Nucleo, cerrar: @escaping () -> Void) {
        _modelo = StateObject(wrappedValue: ModeloSemana(nucleo: nucleo))
        self.cerrar = cerrar
    }

    var body: some View {
        VStack(spacing: 0) {
            Cabecera("Horario de clases", subtitulo: subtitulo) {
                BotonCabecera(icono: "calendar", rotulo: clase == .compact ? "" : "Domingo",
                              ayuda: conDomingo ? "Quitar el domingo" : "Ver el domingo", activo: conDomingo) {
                    conDomingo.toggle()
                }
                BotonCabecera(icono: "mic", rotulo: clase == .compact ? "" : "Nota de voz",
                              ayuda: "Nota de voz para la próxima clase") {
                    HorarioIPad.grabarParaLaProxima(nucleo: modelo.nucleo)
                }
                BotonCabecera(icono: "doc.plaintext", ayuda: "Editar como texto…") { editandoTexto = true }
                BotonCabecera(icono: "xmark", ayuda: "Cerrar") { cerrar() }
                    .keyboardShortcut(.cancelAction)
            }
            if clase == .compact {
                VStack(spacing: 0) {
                    lista.frame(height: 210)
                    Paleta.v2.linea.frame(height: 1)
                    rejilla
                }
            } else {
                HStack(spacing: 0) {
                    lista.frame(width: 270)
                    Paleta.v2.linea.frame(width: 1)
                    rejilla
                }
            }
        }
        .background(Paleta.v2.fondo.ignoresSafeArea())
        .environment(\.colorScheme, .dark)
        .overlay(alignment: .bottom) {
            if let e = modelo.error {
                AvisoAbajo(texto: e, esError: true)
            } else if let d = modelo.deshacer {
                AvisoAbajo(texto: d) { modelo.hacerDeshacer() }
                    .task(id: d) {
                        try? await Task.sleep(for: .seconds(6))
                        modelo.olvidarDeshacer()
                    }
            } else if let i = elegida, i < modelo.asignaturas.count {
                AvisoAbajo(texto: "Toca un hueco de la semana para poner \(modelo.asignaturas[i].nombre)")
            }
        }
        .sheet(isPresented: $editandoTexto) {
            HojaTextoHorario(inicial: modelo.texto() ?? "") { modelo.escribir($0) }
        }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinTrajoCambios)) { _ in modelo.releer() }
    }

    private var subtitulo: String {
        let n = modelo.bloques.count
        return n == 1 ? "1 clase a la semana" : "\(n) clases a la semana"
    }

    // MARK: Las asignaturas

    private var lista: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Asignaturas").font(.system(size: 15, weight: .semibold)).foregroundStyle(Paleta.v2.texto)
            HStack(spacing: 8) {
                TextField("", text: $nueva, prompt: Text("Nueva asignatura (Geometría @A-12)").foregroundStyle(Paleta.v2.gris))
                    .textFieldStyle(.plain)
                    .foregroundStyle(Paleta.v2.texto)
                    .padding(.horizontal, 10).frame(height: 38)
                    .background(Paleta.v2.caja, in: RoundedRectangle(cornerRadius: 9))
                    .submitLabel(.done)
                    .onSubmit(anadir)
                BotonCabecera(icono: "plus", ayuda: "Añadir la asignatura", activo: true, accion: anadir)
                    .disabled(nueva.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(Array(modelo.asignaturas.enumerated()), id: \.element.nombre) { i, a in
                        filaAsignatura(i, a)
                    }
                    if modelo.asignaturas.isEmpty {
                        Text("Escribe tus asignaturas arriba y arrástralas a la semana.")
                            .font(.caption).foregroundStyle(Paleta.v2.gris)
                    }
                    if !modelo.proximas.isEmpty, clase != .compact {
                        Text("Próximas llamadas").font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(Paleta.v2.gris).padding(.top, 14)
                        ForEach(modelo.proximas, id: \.id) { p in
                            VStack(alignment: .leading, spacing: 1) {
                                Text(p.nombre).font(.system(size: 13)).foregroundStyle(Paleta.v2.texto).lineLimit(1)
                                Text("\(AtajosDeHora.legible(fechaDe(ms: p.cuandoUtcMs), ahora: Date())) · \(p.detalle)")
                                    .font(.caption).foregroundStyle(Paleta.v2.gris).lineLimit(1)
                            }
                        }
                    }
                }
            }
        }
        .padding(14)
    }

    private func filaAsignatura(_ i: Int, _ a: AsignaturaHorario) -> some View {
        let color = Color(hex: a.color)
        return HStack(spacing: 10) {
            Button { modelo.cambiar { $0.cambiarColor(i: UInt32(i)) } } label: {
                Circle().fill(color).frame(width: 18, height: 18)
                    .frame(width: 32, height: 32).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Otro color para \(a.nombre)")
            VStack(alignment: .leading, spacing: 1) {
                Text(a.nombre).font(.system(size: 14.5)).foregroundStyle(Paleta.v2.texto).lineLimit(1)
                if let aula = a.aula, !aula.isEmpty {
                    Text(aula).font(.caption).foregroundStyle(Paleta.v2.gris).lineLimit(1)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 6).padding(.vertical, 4)
        .background(elegida == i ? color.opacity(0.35) : color.opacity(0.14), in: RoundedRectangle(cornerRadius: 8))
        .overlay {
            if elegida == i { RoundedRectangle(cornerRadius: 8).strokeBorder(color, lineWidth: 2) }
        }
        .contentShape(RoundedRectangle(cornerRadius: 8))
        .onTapGesture { elegida = elegida == i ? nil : i }
        .draggable("asignatura:\(i)") {
            Text(a.nombre).padding(8).background(color.opacity(0.8), in: RoundedRectangle(cornerRadius: 8))
        }
        .contextMenu {
            Button("Ponerla en la semana") { elegida = i }
            Button("Otro color") { modelo.cambiar { $0.cambiarColor(i: UInt32(i)) } }
            Button("Borrar", role: .destructive) {
                if elegida == i { elegida = nil }
                modelo.cambiar(deshacible: "Borrada: \(a.nombre)") { $0.borrarAsignatura(i: UInt32(i)) }
            }
        }
    }

    private func anadir() {
        let t = nueva.trimmingCharacters(in: .whitespaces)
        guard !t.isEmpty else { return }
        nueva = ""
        modelo.cambiar { _ = $0.anadirAsignatura(escrito: t) }
    }

    // MARK: La semana

    private var rejilla: some View {
        GeometryReader { geo in
            let horas: CGFloat = 50, cabecera: CGFloat = 30
            let dias = conDomingo ? 7 : 6
            let ancho = max(40, (geo.size.width - horas - 8) / CGFloat(dias))
            let r = ReglasHorario(cuantosDias: dias, anchoDia: Double(ancho), altoHora: 60)
            VStack(spacing: 0) {
                cabeceraDias(r, horas: horas).frame(height: cabecera)
                ScrollView(.vertical) {
                    HStack(alignment: .top, spacing: 0) {
                        columnaHoras(r).frame(width: horas)
                        semana(r).frame(width: ancho * CGFloat(dias), height: CGFloat(r.altoTotal))
                    }
                    .padding(.vertical, 10)
                    .padding(.trailing, 8)
                }
                .scrollIndicators(.visible)
            }
        }
    }

    private func cabeceraDias(_ r: ReglasHorario, horas: CGFloat) -> some View {
        let hoy = ReglasHorario.hoy()
        return HStack(spacing: 0) {
            Color.clear.frame(width: horas)
            ForEach(0 ..< r.cuantosDias, id: \.self) { d in
                Text(ReglasHorario.dias[d])
                    .font(.system(size: 13, weight: d == hoy ? .bold : .medium))
                    .foregroundStyle(d == hoy ? Paleta.v2.acento : Paleta.v2.cuerpo)
                    .frame(width: CGFloat(r.anchoDia))
            }
            Spacer(minLength: 0)
        }
    }

    private func columnaHoras(_ r: ReglasHorario) -> some View {
        ZStack(alignment: .topTrailing) {
            ForEach(Array(stride(from: ReglasHorario.desde, through: ReglasHorario.hasta, by: 60)), id: \.self) { m in
                Text(ReglasHorario.hora(m)).font(.system(size: 11).monospacedDigit()).foregroundStyle(Paleta.v2.gris)
                    .padding(.trailing, 6)
                    .offset(y: CGFloat(r.y(m)) - 7)
            }
        }
        .frame(height: CGFloat(r.altoTotal), alignment: .topTrailing)
    }

    private func semana(_ r: ReglasHorario) -> some View {
        let hoy = ReglasHorario.hoy()
        let ahora = ReglasHorario.ahora()
        let ancho = CGFloat(r.anchoDia)
        return ZStack(alignment: .topLeading) {
            // Hoy, un poco mas claro.
            if hoy < r.cuantosDias {
                Paleta.v2.acento.opacity(0.06)
                    .frame(width: ancho, height: CGFloat(r.altoTotal))
                    .offset(x: ancho * CGFloat(hoy))
            }
            // Las lineas de las horas, las medias y los dias.
            Canvas { c, tam in
                for m in stride(from: ReglasHorario.desde, through: ReglasHorario.hasta, by: 30) {
                    let y = CGFloat(r.y(m))
                    var p = Path()
                    p.move(to: CGPoint(x: 0, y: y))
                    p.addLine(to: CGPoint(x: tam.width, y: y))
                    c.stroke(p, with: .color(.white.opacity(m % 60 == 0 ? 0.10 : 0.04)), lineWidth: 1)
                }
                for d in 0 ... r.cuantosDias {
                    let x = CGFloat(d) * ancho
                    var p = Path()
                    p.move(to: CGPoint(x: x, y: 0))
                    p.addLine(to: CGPoint(x: x, y: tam.height))
                    c.stroke(p, with: .color(.white.opacity(0.10)), lineWidth: 1)
                }
            }
            // Tocar un hueco con una asignatura elegida pone ahi la clase.
            Color.clear
                .contentShape(Rectangle())
                .onTapGesture(coordinateSpace: .local) { punto in
                    guard let i = elegida else { return }
                    let dia = r.dia(enX: Double(punto.x))
                    let minuto = r.minuto(enY: Double(punto.y))
                    modelo.cambiar { $0.soltar(asignatura: UInt32(i), dia: UInt8(dia), minuto: UInt32(minuto)) }
                    elegida = nil
                }
            // Los bloques.
            ForEach(Array(modelo.bloques.enumerated()), id: \.offset) { i, b in
                bloque(i, b, r)
            }
            // La hora de ahora en el dia de hoy.
            if hoy < r.cuantosDias, ahora >= ReglasHorario.desde, ahora <= ReglasHorario.hasta {
                Paleta.v2.rojo.frame(width: ancho, height: 2)
                    .offset(x: ancho * CGFloat(hoy), y: CGFloat(r.y(ahora)) - 1)
                    .allowsHitTesting(false)
            }
        }
        .frame(width: ancho * CGFloat(r.cuantosDias), height: CGFloat(r.altoTotal), alignment: .topLeading)
        .dropDestination(for: String.self) { cosas, punto in
            guard let cosa = cosas.first else { return false }
            let dia = UInt8(r.dia(enX: Double(punto.x)))
            let minuto = r.minuto(enY: Double(punto.y))
            if cosa.hasPrefix("asignatura:"), let i = UInt32(cosa.dropFirst(11)) {
                modelo.cambiar { $0.soltar(asignatura: i, dia: dia, minuto: UInt32(minuto)) }
            } else if cosa.hasPrefix("bloque:"), let i = Int(cosa.dropFirst(7)), i < modelo.bloques.count {
                // Se suelta donde va su borde de arriba (el dedo coge la
                // clase por arriba, como la etiqueta que se arrastra).
                modelo.cambiar { $0.mover(i: UInt32(i), dia: dia, inicio: Int64(minuto)) }
            } else {
                return false
            }
            elegida = nil
            return true
        }
    }

    private func bloque(_ i: Int, _ b: BloqueHorario, _ r: ReglasHorario) -> some View {
        let ancho = CGFloat(r.anchoDia)
        let inicio = Int(b.inicio)
        let fin = alargando?.0 == i ? alargando!.1 : Int(b.fin)
        let y0 = CGFloat(r.y(inicio))
        let alto = max(18, CGFloat(r.y(fin)) - y0)
        let color = Color(hex: b.color)
        return VStack(alignment: .leading, spacing: 1) {
            Text(b.nombre).font(.system(size: 12.5, weight: .semibold)).lineLimit(alto > 40 ? 2 : 1)
            if alto > 34 {
                Text(ReglasHorario.tramo(inicio, fin)).font(.system(size: 11).monospacedDigit())
            }
            if alto > 50, let a = b.aula, !a.isEmpty {
                Text(a).font(.system(size: 11)).lineLimit(1)
            }
            Spacer(minLength: 0)
        }
        .foregroundStyle(.white)
        .padding(.horizontal, 5).padding(.top, 3)
        .frame(width: ancho - 4, height: alto, alignment: .topLeading)
        .background(color.opacity(0.85), in: RoundedRectangle(cornerRadius: 6))
        .overlay(alignment: .bottom) { asa(i, b, r, color: color) }
        .contentShape(RoundedRectangle(cornerRadius: 6))
        .draggable("bloque:\(i)") {
            Text(b.nombre).font(.caption.bold()).foregroundStyle(.white)
                .padding(6).background(color, in: RoundedRectangle(cornerRadius: 6))
        }
        .contextMenu {
            Text("\(ReglasHorario.dias[Int(b.dia) % 7]) \(ReglasHorario.tramo(inicio, Int(b.fin)))")
            if let a = modelo.asignatura(de: b) {
                Button("Otro color") { modelo.cambiar { $0.cambiarColor(i: UInt32(a)) } }
            }
            Button("Borrar esta clase", role: .destructive) {
                modelo.cambiar(deshacible: "Clase borrada: \(b.nombre)") { $0.borrar(i: UInt32(i)) }
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(b.nombre), \(ReglasHorario.dias[Int(b.dia) % 7]) de \(ReglasHorario.tramo(inicio, Int(b.fin)))")
        .offset(x: ancho * CGFloat(b.dia) + 2, y: y0)
    }

    /// El asa de abajo de una clase: tirar de ella la alarga o la acorta en
    /// saltos de 15 min (se ve mientras se tira).
    private func asa(_ i: Int, _ b: BloqueHorario, _ r: ReglasHorario, color: Color) -> some View {
        Capsule().fill(Color.white.opacity(0.7)).frame(width: 26, height: 4)
            .frame(maxWidth: .infinity).frame(height: 16)
            .contentShape(Rectangle())
            .highPriorityGesture(
                DragGesture(minimumDistance: 2, coordinateSpace: .local)
                    .onChanged { g in
                        let y = r.y(Int(b.fin)) + Double(g.translation.height)
                        alargando = (i, max(Int(b.inicio) + ReglasHorario.paso, r.minutoCercano(enY: y)))
                    }
                    .onEnded { g in
                        let y = r.y(Int(b.fin)) + Double(g.translation.height)
                        let fin = max(Int(b.inicio) + ReglasHorario.paso, r.minutoCercano(enY: y))
                        alargando = nil
                        modelo.cambiar { $0.alargar(i: UInt32(i), fin: UInt32(fin)) }
                    }
            )
            .accessibilityLabel("Alargar o acortar")
    }
}

// MARK: - Editar como texto

/// `horario.md` a mano (una linea por clase: `Lun 8:00-10:00 Geometría
/// @A-12`), como en Windows con el editor de notas.
struct HojaTextoHorario: View {
    let inicial: String
    let guardar: (String) -> Void
    @State private var texto = ""
    @Environment(\.dismiss) private var cerrar

    var body: some View {
        NavigationStack {
            TextEditor(text: $texto)
                .font(.system(size: 15, design: .monospaced))
                .scrollContentBackground(.hidden)
                .background(Paleta.v2.fondo)
                .padding(8)
                .navigationTitle("Horario como texto")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) { Button("Cancelar") { cerrar() } }
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Guardar") {
                            guardar(texto)
                            cerrar()
                        }
                        .keyboardShortcut("s")
                    }
                }
        }
        .environment(\.colorScheme, .dark)
        .onAppear { texto = inicial }
    }
}

// MARK: - La nota de voz de la proxima clase

/// La grabadora de la nota de una clase: graba aparte y, al guardar, la
/// deja donde la busca la llamada de esa clase.
@MainActor
final class GrabadoraDeClase: ObservableObject {
    @Published private(set) var clase = ""
    @Published private(set) var grabando = false
    @Published private(set) var dicho: String?
    private var grabadora: AVAudioRecorder?

    func empezar(_ nota: NotaDeClase) {
        clase = nota.clase
        dicho = nil
        Task {
            guard await GrabadoraVoz.permisoMicrofono() else {
                dicho = "Sin permiso para el micrófono"
                return
            }
            let sesion = AVAudioSession.sharedInstance()
            try? sesion.setCategory(.playAndRecord, mode: .default, options: [.defaultToSpeaker, .allowBluetooth])
            try? sesion.setActive(true)
            let ajustes: [String: Any] = [AVFormatIDKey: kAudioFormatMPEG4AAC, AVSampleRateKey: 44_100,
                                          AVNumberOfChannelsKey: 1,
                                          AVEncoderAudioQualityKey: AVAudioQuality.high.rawValue]
            // Primero aparte: si se descarta, la nota de antes sigue.
            let destino = URL(fileURLWithPath: nota.ruta)
            try? FileManager.default.createDirectory(at: destino.deletingLastPathComponent(), withIntermediateDirectories: true)
            let tmp = destino.deletingPathExtension().appendingPathExtension("parte.m4a")
            guard let g = try? AVAudioRecorder(url: tmp, settings: ajustes), g.record() else {
                dicho = "No se pudo grabar"
                return
            }
            grabadora = g
            grabando = true
        }
    }

    /// Para. `true` si ya se puede cerrar.
    func parar(guardar: Bool) {
        guard let g = grabadora else { return }
        g.stop()
        grabadora = nil
        grabando = false
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
        let tmp = g.url
        let destino = tmp.deletingPathExtension().deletingPathExtension().appendingPathExtension("m4a")
        if guardar {
            try? FileManager.default.removeItem(at: destino)
            if (try? FileManager.default.moveItem(at: tmp, to: destino)) != nil {
                dicho = "Guardada para la próxima clase de \(clase)"
            } else {
                dicho = "No se pudo guardar"
            }
        } else {
            try? FileManager.default.removeItem(at: tmp)
            dicho = "Descartada"
        }
    }
}

struct VistaGrabadoraDeClase: View {
    @ObservedObject var g: GrabadoraDeClase
    let cerrar: () -> Void

    var body: some View {
        VStack(spacing: 14) {
            Text("Nota de voz para la próxima clase").font(.subheadline).foregroundStyle(Paleta.v2.gris)
            Text(g.clase).font(.title3.bold()).foregroundStyle(Paleta.v2.texto)
            if let d = g.dicho {
                Text(d).font(.callout).foregroundStyle(Paleta.v2.texto)
                    .task {
                        try? await Task.sleep(for: .seconds(1.6))
                        cerrar()
                    }
            } else {
                HStack(spacing: 8) {
                    Image(systemName: "mic.fill").foregroundStyle(g.grabando ? Paleta.v2.rojo : Paleta.v2.gris)
                    Text(g.grabando ? "Grabando…" : "Preparando…").foregroundStyle(Paleta.v2.texto)
                }
                HStack(spacing: 12) {
                    BotonCabecera(rotulo: "Descartar") {
                        if g.grabando { g.parar(guardar: false) } else { cerrar() }
                    }
                    .keyboardShortcut(.cancelAction)
                    BotonCabecera(icono: "checkmark", rotulo: "Guardar", activo: true) { g.parar(guardar: true) }
                        .keyboardShortcut(.defaultAction)
                        .disabled(!g.grabando)
                }
            }
        }
        .padding(20)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Paleta.v2.fondo.ignoresSafeArea())
        .environment(\.colorScheme, .dark)
        .interactiveDismissDisabled(g.grabando)
    }
}
