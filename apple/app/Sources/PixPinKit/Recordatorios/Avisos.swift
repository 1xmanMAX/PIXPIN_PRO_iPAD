// El aviso de un recordatorio (`notificaciones::de_recordatorio` de
// Windows): titulo «Recordatorio», lo que se apunto y los botones de su
// categoria (`CentroAvisos`): «Hecha» si es de una tarea, «+10 min» y
// «Abrir». Y las horas que se ofrecen al ponerlo (`recordatorios::atajos`).

import Foundation
import PixPinCore

enum AvisosRecordatorio {
    /// Lo que viaja en los datos del aviso (el `userInfo`).
    static let claveChat = "chat"
    static let claveId = "id"
    static let claveCuando = "cuando"
    static let claveLista = "lista"
    static let claveIndice = "indice"

    /// El id de la notificacion de un recordatorio: unico entre chats. Es
    /// el mismo que usa el menu del mensaje al ponerlo, asi que el vigia
    /// cambia esa notificacion en vez de duplicarla.
    static func identificador(chat: String, id: String) -> String { "recuerdaEn:\(chat):\(id)" }

    /// El aviso de un recordatorio puesto (a su hora).
    static func aviso(de r: RecordatorioPuesto) -> Aviso {
        var datos = [claveChat: r.chat, claveId: r.id, claveCuando: String(milisegundos(r.cuando))]
        if let t = r.tarea {
            datos[claveLista] = t.lista
            datos[claveIndice] = String(t.indice)
        }
        let texto = r.texto.trimmingCharacters(in: .whitespacesAndNewlines)
        if r.llamada {
            // Una llamada: con su timbre, y tocarla la abre contestada.
            datos[Llamada.claveNombre] = r.texto
            var a = Aviso(id: r.clave, categoria: .llamada, titulo: "Llamada entrante",
                          texto: Llamada.titulo(texto), cuando: r.cuando, datos: datos)
            a.sonido = TonoLlamada.nombre
            return a
        }
        return Aviso(id: r.clave, categoria: r.tarea == nil ? .recordatorio : .recordatorioTarea,
                     titulo: "Recordatorio", texto: texto.isEmpty ? "Recordatorio" : texto,
                     cuando: r.cuando, datos: datos)
    }

    /// La tarea de los datos de un aviso, si es de una.
    static func tarea(de datos: [String: String]) -> TareaRecordada? {
        guard let lista = datos[claveLista], let i = datos[claveIndice].flatMap(UInt32.init) else { return nil }
        return TareaRecordada(lista: lista, indice: i)
    }
}

/// Las horas que se ofrecen al poner un recordatorio (`recordatorios::atajos`
/// de Windows: las cuatro del movil y «dentro de 10 minutos»), en hora local.
enum AtajosDeHora {
    struct Atajo: Equatable {
        var titulo: String
        var cuando: Date
    }

    static func atajos(ahora: Date, calendario: Calendar = .current) -> [Atajo] {
        [
            Atajo(titulo: "Dentro de 10 minutos", cuando: ahora.addingTimeInterval(10 * 60)),
            Atajo(titulo: "Dentro de 1 hora", cuando: ahora.addingTimeInterval(60 * 60)),
            Atajo(titulo: "Dentro de 3 horas", cuando: ahora.addingTimeInterval(3 * 60 * 60)),
            Atajo(titulo: "Esta tarde (18:00)", cuando: aLas(18, dias: 0, ahora: ahora, calendario: calendario)),
            Atajo(titulo: "Mañana (9:00)", cuando: aLas(9, dias: 1, ahora: ahora, calendario: calendario)),
        ]
    }

    /// La proxima vez que el reloj marque `hora` en punto, `dias` despues de
    /// hoy. Si ya paso, la de manana: «esta tarde» pulsado a las ocho de la
    /// noche no puede ser una hora que ya fue (saltaria en el acto).
    static func aLas(_ hora: Int, dias: Int, ahora: Date, calendario: Calendar = .current) -> Date {
        let hoy = calendario.startOfDay(for: ahora)
        var cuando = calendario.date(byAdding: DateComponents(day: dias, hour: hora), to: hoy) ?? ahora
        if cuando <= ahora {
            cuando = calendario.date(byAdding: .day, value: 1, to: cuando) ?? cuando
        }
        return cuando
    }

    /// Para leerla: «18:00» si es de hoy y «21/09 09:00» si no
    /// (`cuando_legible`): la hora sola mentiria para manana.
    static func legible(_ cuando: Date, ahora: Date, calendario: Calendar = .current) -> String {
        let f = DateFormatter()
        f.calendar = calendario
        f.timeZone = calendario.timeZone
        f.locale = Locale(identifier: "es_ES")
        f.dateFormat = calendario.isDate(cuando, inSameDayAs: ahora) ? "HH:mm" : "dd/MM HH:mm"
        return f.string(from: cuando)
    }
}
