// **El calendario del propio iPad** (EventKit), junto a Google Calendar
// (`calendario_google.rs` de Windows): los eventos con hora de los
// calendarios del iPad (iCloud, Exchange, los suscritos...) llaman como
// las clases del horario, con su hora y su lugar, los minutos de antes del
// horario.
//
// - Se enciende desde el horario («Calendario del iPad»), que pide el
//   permiso; apagado no se lee nada.
// - Los avisos van con un id `horario:ical:…` para que el vigia los trate
//   como los del horario (la pantalla de la llamada); su detalle lo guarda
//   aqui, porque el nucleo no los conoce.
// - Un evento que ya trae Google Calendar (mismo nombre y misma hora) no se
//   repite.
// - Se mira lo de las proximas 36 h, que es lo que el vigia pone en la
//   agenda; si el calendario cambia (`EKEventStoreChanged`), se relee.

import EventKit
import Foundation
import PixPinCore

@MainActor
public final class CalendarioDelIPad: ObservableObject {
    public static let compartido = CalendarioDelIPad()

    nonisolated private static let clave = "pixpin.calendarioDelIPad"
    /// Si esta encendido, desde cualquier hilo (lo guardado).
    nonisolated static var compartidoActivo: Bool { UserDefaults.standard.bool(forKey: clave) }
    /// Lo que se mira hacia delante.
    nonisolated static let horas: Double = 36

    @Published public private(set) var activo: Bool
    /// Lo que dijo el sistema al pedir permiso (para explicarlo).
    @Published public private(set) var sinPermiso = false

    private let almacen = EKEventStore()
    /// El detalle de cada aviso puesto (hora y lugar), por su id.
    private var detalles: [String: String] = [:]
    private var vigia: NSObjectProtocol?

    private init() {
        activo = UserDefaults.standard.bool(forKey: Self.clave)
        if activo { vigilar() }
    }

    /// Si hay permiso para leer los eventos.
    var conPermiso: Bool {
        EKEventStore.authorizationStatus(for: .event) == .fullAccess
    }

    /// Lo enciende (pidiendo permiso) o lo apaga; los avisos se releen.
    public func alternar() async {
        if activo {
            activo = false
            UserDefaults.standard.set(false, forKey: Self.clave)
            detalles = [:]
            Recordatorios.compartido.releer()
            return
        }
        var permiso = conPermiso
        if !permiso {
            permiso = (try? await almacen.requestFullAccessToEvents()) ?? false
        }
        sinPermiso = !permiso
        guard permiso else { return }
        activo = true
        UserDefaults.standard.set(true, forKey: Self.clave)
        vigilar()
        Recordatorios.compartido.releer()
    }

    private func vigilar() {
        guard vigia == nil else { return }
        vigia = NotificationCenter.default.addObserver(forName: .EKEventStoreChanged, object: almacen,
                                                       queue: .main) { _ in
            MainActor.assumeIsolated { Recordatorios.compartido.releerConCalma() }
        }
    }

    /// **Los avisos de los eventos que vienen** (como los del horario),
    /// sin los que ya estan en `evitando` (los de Google Calendar).
    /// `antelacion`: los minutos de antes.
    func avisos(desde ahora: Date = Date(), antelacion: UInt32, evitando: [AvisoHorario]) -> [AvisoHorario] {
        guard activo, conPermiso else { return [] }
        let hasta = ahora.addingTimeInterval(Self.horas * 3600)
        let predicado = almacen.predicateForEvents(withStart: ahora, end: hasta, calendars: nil)
        let eventos = almacen.events(matching: predicado).filter { !$0.isAllDay }
        let lista = eventos.compactMap { e -> EventoDelCalendario? in
            guard let inicio = e.startDate else { return nil }
            return EventoDelCalendario(id: e.calendarItemIdentifier, titulo: e.title ?? "", inicio: inicio,
                                   fin: e.endDate, lugar: e.location)
        }
        let hechos = CalendarioDelIPad.avisos(lista, antelacion: antelacion, ahora: ahora, evitando: evitando)
        for a in hechos { detalles[a.id] = a.detalle }
        return hechos
    }

    /// El detalle (hora y lugar) de un aviso de este calendario.
    func detalle(de id: String) -> String? {
        detalles[id]
    }

    // MARK: Lo puro (se prueba solo)

    /// El prefijo de los ids (el vigia los trata como los del horario).
    nonisolated static let prefijo = "horario:ical:"

    /// Los avisos de `eventos`: `antelacion` minutos antes de empezar, los
    /// que aun no pasaron, sin los que ya trae `evitando` (mismo nombre y
    /// misma hora de empezar), por hora.
    nonisolated static func avisos(_ eventos: [EventoDelCalendario], antelacion: UInt32, ahora: Date,
                                   evitando: [AvisoHorario], calendario: Calendar = .current) -> [AvisoHorario] {
        let antes = TimeInterval(antelacion) * 60
        func clave(_ s: String) -> String {
            s.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil)
                .trimmingCharacters(in: .whitespacesAndNewlines)
        }
        let yaEstan = Set(evitando.map { "\(clave($0.nombre))@\($0.cuandoUtcMs / 60000)" })
        var vistos = Set<String>()
        var hechos: [AvisoHorario] = []
        for e in eventos {
            let nombre = e.titulo.trimmingCharacters(in: .whitespacesAndNewlines)
            let cuando = e.inicio.addingTimeInterval(-antes)
            guard cuando > ahora else { continue }
            let ms = Int64((cuando.timeIntervalSince1970 * 1000).rounded())
            let titulo = nombre.isEmpty ? "Evento" : nombre
            guard !yaEstan.contains("\(clave(titulo))@\(ms / 60000)") else { continue }
            let id = "\(prefijo)\(e.id)|\(ms)"
            guard vistos.insert(id).inserted else { continue }
            hechos.append(AvisoHorario(id: id, cuandoUtcMs: ms, nombre: titulo,
                                       detalle: detalle(e, calendario: calendario), texto: nil, audio: nil))
        }
        return hechos.sorted { $0.cuandoUtcMs < $1.cuandoUtcMs }
    }

    /// «10:00–11:30 · Aula 3»: la hora y el lugar.
    nonisolated static func detalle(_ e: EventoDelCalendario, calendario: Calendar = .current) -> String {
        func hora(_ d: Date) -> String {
            let c = calendario.dateComponents([.hour, .minute], from: d)
            return String(format: "%d:%02d", c.hour ?? 0, c.minute ?? 0)
        }
        var t = hora(e.inicio)
        if let fin = e.fin, fin > e.inicio { t += "–" + hora(fin) }
        if let l = e.lugar?.trimmingCharacters(in: .whitespacesAndNewlines), !l.isEmpty { t += " · " + l }
        return t
    }
}

/// Un evento del calendario del iPad, lo que importa de el (fuera de la
/// clase: se usa en lo puro, sin actor).
struct EventoDelCalendario: Equatable, Sendable {
    var id: String
    var titulo: String
    var inicio: Date
    var fin: Date?
    var lugar: String?
}
