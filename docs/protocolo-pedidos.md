# Pedidos a PixPin Max (protocolo v1)

Cualquier programa del mismo equipo puede pedirle cosas a la PixPin Max que esta abierta:
abrir un proyecto, escribir en el chat, crear un lienzo, grabar, añadir una tarea... Lo usa
el plugin de Flow Launcher (`apps/pixpin-lanzador`), y sirve igual para un script propio
(AutoHotkey, PowerShell, otro lanzador).

## Transporte

- `WM_COPYDATA` a la ventana de clase **`PixPinMaxVentanaMensajes`** (`FindWindowW`), con
  `dwData = 0x5049_5851` y en `lpData` el **JSON en UTF-8, sin nulo final**, de 64 KB como
  mucho. Se manda con `SendMessageW` (sincrono) y despues se le postea `WM_NULL` para
  despertar su bucle.
- Respuesta inmediata (`LRESULT`): `1` aceptado (en cola), `2` version no soportada, `3` JSON
  roto o accion desconocida, `0` no era para nosotros o no hay nadie escuchando.
- El trabajo de verdad lo hace el bucle principal de PixPin despues: lo que salga mal lo
  dice PixPin con su aviso, no la respuesta.
- Si no hay ventana, PixPin no esta abierta: se arranca
  `%LOCALAPPDATA%\Programs\PixPinMax\pixpinmax.exe` y se espera a la ventana (hasta 8 s).

## El pedido

```json
{ "pixpin": 1, "accion": "<accion>", ... }
```

- `pixpin` es la version. Un campo nuevo opcional no la sube; quitar o cambiar uno si.
- Lo que no se conoce se ignora.
- `proyecto` es el `id` de la carpeta del proyecto (`proyectos/<id>`, el de
  `proyectos/indice.json`). `null` o ausente = **Mensajes guardados**.
- `codigo` es el `id` del mensaje dentro de `guardados.jsonl`.

| accion | campos | que hace |
|---|---|---|
| `abrir` | `que: {tipo:"proyecto", proyecto}` | El chat en ese proyecto |
| `abrir` | `que: {tipo:"mensaje", proyecto, codigo}` | El chat en ese mensaje, resaltado |
| `abrir` | `que: {tipo:"hoja", proyecto, referencia}` | Ese lienzo u hoja, en el lienzo |
| `abrir` | `que: {tipo:"nota", proyecto, codigo}` | Esa nota en el editor de notas |
| `abrir` | `que: {tipo:"fichero", ruta, pagina?, buscar?, zona?}` | Como «Abrir con PixPin» (lector, editor, pin). Un `.leccion` abre la ficha de esa lección. Con `pagina` (desde 1), un PDF, Word o PowerPoint se abre en esa página en vez de donde se dejó (las citas del asistente, `buscar_en_mis_cosas`) Con `buscar` (14-oct), un PDF, Word o PowerPoint se abre con la caja de buscar ya escrita y salta a la primera coincidencia (desde `pagina` si va), marcada como con Ctrl+F. Con `zona: [x, y, ancho]` (coordenadas del dibujo), un plano DWG/DXF se abre mirando ahí en vez de entero: lo usa el asistente para llevar a un texto encontrado («¿dónde dice JR. ANTISUYO?») |
| `chat` | `texto`, `proyecto?`, `imagenes?`, `archivos?` | Un mensaje de texto nuevo en ese chat. Con `imagenes` (rutas; `[img 01]` en `texto` = `imagenes[0]`), el texto sin sus fichas va primero y cada imagen detras como una foto del chat (un BMP se guarda como PNG). Con `archivos` (ver abajo), cada fichero va detras como un adjunto
| `nota_nueva` | `texto?`, `proyecto?`, `imagenes?` | Una nota nueva, abierta en el editor. Con `imagenes`, cada `[img NN]` del texto queda como esa imagen dentro de la nota
| `lienzo_nuevo` | `nombre?`, `proyecto?` | Un lienzo nuevo, abierto |
| `grabar` | `nombre`, `proyecto?` | Saca el micrófono flotante y graba una nota de voz; un clic la guarda con ese nombre (`<nombre>.m4a`) en ese chat |
| `lista_nueva` | `titulo`, `proyecto?` | Una lista de tareas nueva (mini-app `tareas`) |
| `tabla_nueva` | `nombre`, `proyecto?`, `filas?`, `cabecera?`, `abrir?` | Una **tabla del chat** (mini-app `tabla`, la del pin de hoja, que viaja al móvil) ya rellena: `filas[f][c]` en texto va a la celda de la columna `c` y la fila `f` (A1 arriba a la izquierda). Los números escritos como en papel («1 250,50», «1.250») se guardan como número. `cabecera: true` pone la primera fila en negrita. `abrir: true` la saca ya como pin de hoja; si no, se avisa. Lo usa el asistente al pasar una tabla escaneada (14-oct) |
| `anadir_tarea` | `texto`, `proyecto?`, `codigo?`, `imagenes?` | Añade `- [ ] texto` (con la fecha de hoy). **Sin `proyecto` ni `codigo`**: a la lista **«Inbox»** de «Mensajes guardados» (se crea si no esta), como la caja de la ventana de tareas. Con `codigo`: a esa lista. Con `proyecto` y sin `codigo`: a la ultima lista de tareas de ese chat, o a una nueva «Tareas». Con `imagenes` (ver abajo), la tarea lleva esas imagenes |
| `marcar_tarea` | `proyecto?`, `codigo`, `indice`, `hecha` | Marca (o desmarca) la tarea numero `indice` (desde 0, en el orden del documento) de la lista `codigo` |
| `mover_tarea` | `proyecto?`, `codigo`, `indice`, `a_proyecto?`, `a_codigo` | Pasa la tarea numero `indice` (desde 0, en el orden del documento) de la lista `codigo` (del chat `proyecto`) al final de la lista `a_codigo` (del chat `a_proyecto`; ausente = «Mensajes guardados»), con su fecha y su estado, como «Mover a…» de la ventana de tareas. Si la tarea cambio entre medias no se toca nada y se avisa |
| `corregir_tarea` | `proyecto?`, `codigo`, `indice`, `texto` | Cambia lo que se lee de la tarea numero `indice` (desde 0) de la lista `codigo` por `texto`, como «Editar» de la ventana de tareas (`tareas::corregir`, `Tareas.renombrar` del móvil): conserva su fecha de creacion y sus imagenes. Una fecha va en `texto` como su marca de recordatorio (`⏰ AAAA-MM-DD HH:MM`, hora local). Un `texto` vacio no borra la tarea (eso es `quitar_tarea`): se avisa |
| `quitar_tarea` | `proyecto?`, `codigo`, `indice` | Quita la tarea numero `indice` (desde 0, en el orden del documento) de la lista `codigo`, como su aspa en la ventana de tareas (`Tareas.borrar` del móvil). La lista se reescribe en su mensaje y asi viaja al móvil. Si no hay esa tarea, o `codigo` no es una lista, no se toca nada y se avisa |
| `borrar_lista` | `proyecto?`, `codigo` | Borra la lista `codigo` **entera**, con todas sus tareas: su mensaje sale del chat por el mismo camino que «Borrar» en el chat (sale de `guardados.jsonl` y deja su marca en `sincro\borrados.jsonl`, para que el móvil tambien la quite en la siguiente vuelta). No se pregunta ni se puede deshacer: quien lo manda ya pregunto. Las imagenes de sus tareas se quedan en `archivos/`. El Inbox tambien se puede borrar: se vuelve a crear con la siguiente tarea apuntada. Si `codigo` no es una lista, no se toca nada y se avisa |
| `ventana` | `cual: "tareas" \| "galeria" \| "lecciones" \| "sincronizar" \| "ajustes" \| "timeline" \| "buscar"` | Abre (o trae delante) esa ventana, como su entrada de la bandeja (ajustes y buscar, por su comando). Otro `cual` es un pedido roto |
| `capturar` | `modo?` | Empieza una captura por el mismo camino que su comando de la bandeja, unos 300 ms despues para que el lanzador ya se haya escondido. `modo`: `"zona"` (la del atajo general, con la barra; sin `modo`, esta), `"copiar"` (directa al portapapeles), `"pin"`, `"en_vivo"`, `"anotar"`, `"texto"` (copiar el texto que se lea), `"scroll"` (larga), `"repetir"` (la ultima zona), `"retardo"` (tras la cuenta atras) o `"gif"` (grabar la zona). Otro modo es un pedido roto |
| `pinear_ultima` | — | Saca a la pantalla como pin la captura mas reciente de la carpeta de capturas (la primera de la galeria), como `pinear` con su `ruta`. Sin capturas, se avisa |
| `conservar_captura` | `ruta` | Como «Conservar» de la galeria: la captura entra en «Mensajes guardados» y deja de caducar. Solo vale una `ruta` que este en la carpeta de capturas (`<raiz>\capturas\`); otra se rechaza con aviso |
| `borrar_captura` | `ruta` | Como «Borrar» de la galeria: la captura va a la papelera de PixPin (`<raiz>\papelera\capturas\`) y, si estaba conservada, se olvida. Solo una `ruta` de la carpeta de capturas |
| `copiar_imagen` | `ruta` | Pone esa imagen (png, jpg, bmp, webp) en el portapapeles **como imagen**, no como fichero. Un GIF o algo que no es imagen se rechaza con aviso |
| `ventana_principal` | `proyecto?` | Saca el chat; con `proyecto` (15-oct), abierto en ese chat (si no esta, se avisa) |
| `pinear` | `ruta` o (`proyecto?`, `codigo`) | Saca ese fichero (o el del mensaje) a la pantalla como pin, como «Sacar a la pantalla» del chat |
| `iconos` | `extensiones: ["pdf","docx",...]` | Pinta el icono de archivo de cada extension (el del chat: hoja de su color con la extension escrita) en `<raiz>/cache/iconos-de-extension/<ext>.png` (64 px, fondo transparente) si aun no esta. No abre nada ni avisa |
| `soltar` | `proyecto?` | Saca una caja flotante (siempre encima) del proyecto: los ficheros que se le suelten entran en su chat como si se soltaran en la ventana del chat. Una por proyecto; pedirla otra vez la trae delante. Se cierra con su aspa o Esc |
| `reproducir` | `ruta`, `titulo?` | Hace sonar un audio (m4a, mp3, wav…) en un reproductor flotante siempre encima, sin abrir el chat: atras 10 s, play/pausa, adelante 10 s, velocidad, pista para saltar y aspa. Espacio pausa, flechas saltan, Esc cierra. Se cierra sola al acabar. Si ya hay uno abierto, cambia de pista. `titulo` es lo que se lee; sin el, el nombre del fichero |
| `leccion_nueva` | `texto?`, `proyecto?`, `imagenes?` | Abre la ficha de una lección aprendida nueva, como «Nueva lección» del móvil. Con `texto`, se reparte solo («pasó que…, porque…, la próxima vez…») y se proponen etiquetas, área, tipo y causas. Nada se guarda hasta pulsar Guardar o Esc (que guarda, como el «atrás» del móvil); la «×» descarta. Irá al chat de `proyecto` (sin él, «Mensajes guardados»). `imagenes` (rutas absolutas, como en `anadir_tarea`) son **las fotos de la lección**: se leen al recibir el pedido, la ficha dice «📎 N fotos» y, al guardar, cada una va a `archivos/` del chat (`leccion-<ms>-NN.<ext>`, un `.bmp` como `.png`) con su mensaje `IMAGEN` «Foto de la lección» que **responde** al de la lección (`respondeA: "lec-<id>"`), y su id entra en `adjuntos` del `.leccion`, como hace el móvil. Las fichas `[img 01]`… salen del texto. Si falta una imagen o no lo es, no se abre nada y se avisa |
| `lecciones` | `proyecto?`, `consulta?` | Abre la lista de lecciones (buscador, repaso de hoy, lista de comprobación). Con `proyecto`, primero las suyas y las que hablan de lo mismo; con `consulta`, ya buscando. Una sola ventana: pedirla otra vez la trae delante |
| `enviar_al_movil` | `aparato`, `imagenes?`, `archivos?` | Manda esas imagenes (rutas absolutas) **al lienzo que el móvil `aparato` tiene abierto** (`aparato` es su `id` en el grupo, `sincro\identidad.json`), sin abrir Sincronizar en ninguno de los dos. Ver abajo. Sin nada que mandar, o con un fichero que no esta, no sale nada y se avisa |
| `sincronizar` | `aparato` | Sincroniza con ese aparato del grupo (`id`), o con todos si `aparato` es `todos`, **sin abrir la ventana**: lo mismo que «Sincronizar con todos», sin preguntar y con lo elegido la última vez. Avisa al empezar y, al acabar, dice en el globo lo hecho o qué falló. Una sola a la vez. El plugin (`p s`) ensena si cada aparato contesta leyendo `sincroestado.json` (`{"cuando": ms, "responden": {id: bool}}`), que apunta la sonda de fondo. `enviar_al_movil` acepta también `aparato: "todos"` |
| `posponer` | `mensaje`, `proyecto?`, `indice?`, `minutos` | «+10 min» de la notificación de un recordatorio (15-oct): vuelve a sonar dentro de `minutos`. `mensaje` es el id del mensaje con la hora; con `indice`, el de la lista de esa tarea (la hora va en su texto, `⏰ AAAA-MM-DD HH:MM`) |
| `asistente` | `texto?`, `proyecto?`, `imagenes?`, `automatizacion?`, `encargo?` | **Le encarga algo al asistente sin abrir el chat** (`p asistente <pedido>` en Flow). Va a la **cola de encargos** (15-oct, uno tras otro en segundo plano, `encargo` es su id) y lo hace con el motor elegido en Ajustes → Asistente (local, Claude o DeepSeek), guarda la pregunta y la respuesta en el chat «Asistente» y al acabar lo dice en una **notificación de Windows** con «Abrir» (si no sale, el aviso flotante propio). `proyecto`: el chat sobre el que se pide (se le dice al modelo y lo que cree sin chat —una tarea, una nota— va ahí). `imagenes` como en `chat` (`[img NN]` en `texto`): van al chat «Asistente» como fotos y el modelo las ve. Lo que borra o saca algo del PC **pregunta antes**. Sin `texto` ni `imagenes`, abre el chat «Asistente». Con `automatizacion` (su `id`), corre esa automatización ya. Ver abajo |
| `automatizacion` | `operacion: "crear" \| "activar" \| "desactivar" \| "borrar" \| "ejecutar"`, `id?`, `receta?` | Las automatizaciones del asistente (ver abajo). `crear` lleva `receta` (`nombre`, `disparador: {tipo, hora?, dias?}`, `pasos`); las demás, el `id`. Lo hecho se dice en el globo; una receta que no vale no se guarda y se avisa del motivo. `borrar` no pregunta: quien lo manda ya preguntó |
| `pines` | `operacion: "cerrar" \| "ocultar_mostrar" \| "devolver_ultimo" \| "paso_de_clics"` | (15-oct) Los comandos de los pines de la bandeja, por el mismo camino que `capturar` (`WM_COMMAND` a la ventana del bucle): cerrarlos todos, esconderlos o devolverlos, devolver el último que se cerró o que dejen pasar el clic. `cerrar` no pregunta: quien lo manda ya preguntó |
| `timeline` | `texto?` | (15-oct) Con `texto`, apunta un momento en el timeline con la hora de ahora (la primera frase es el título, como la caja de su ventana) y lo dice en el globo; sin `texto`, abre el timeline |
| `leer_en_voz` | `texto?`, `ruta?`, `titulo?`, `parar?` | **Lee en voz alta** (15-oct) con la voz de la app (la de Windows o las de Microsoft, como en los lectores), **sin abrir ninguna ventana**; calla lo que sonara antes. Un `texto` suelto (la selección de una página desde la extensión del navegador): un párrafo por renglón. O un fichero (`ruta`, el asistente: PDF, Word, PowerPoint, Excel, nota o texto): su texto se saca en un hilo. Se pausa, salta y para con los mandos de reproductor de Windows y las teclas multimedia, que enseñan `titulo` (sin él, «PixPin Max» o el nombre del fichero). Con `parar: true`, calla lo que suene, sea de quien sea. Sin nada que leer, o un fichero que no está, se avisa |
| `ocr` | `ruta`, `rutas?` | **El texto de esa imagen al portapapeles** (15-oct, «Texto (OCR)» de la extensión del navegador; con `rutas`, más imágenes detrás de `ruta`, todo en un texto con una línea en blanco entre imagen e imagen: «Copiar texto (OCR)» del menú del Explorador con varias elegidas): el OCR de Windows, en orden de lectura, como «Texto» de la captura. En su hilo; el globo dice lo copiado («Texto copiado (120 caracteres): «…»»), que no había texto o que falta el OCR de Windows. Una `ruta` que no está o no es una imagen se rechaza con aviso |
| `herramienta` | `nombre`, `args?`, `respuesta?`, `preguntar?`, `origen?` | **Hace una herramienta del asistente con esos argumentos y devuelve lo que salió** (15-oct-2026, el servidor MCP `apps/pixpin-mcp`). `nombre` es una fila de `pixpin_ia::acciones::todas()` (`anadir_tarea`, `borrar_lista`, `anotar_documento`…; no `hoja_a_lienzo`), `args` sus argumentos tal como los describe su JSON Schema. Se hace en su hilo por el mismo camino que el asistente: las tareas por su nombre, los documentos por su nombre o ruta, y **lo que la fila manda preguntar se pregunta** (chat «Asistente» y aviso flotante, «Sí»/«No»; sin respuesta en 5 min, no). `preguntar`: otra pregunta que hacer antes (la usa MCP para lo que cambia algo que ya existe). Al acabar escribe `{"ok": bool, "texto": "…"}` en `respuesta`, que tiene que ser un `.json` **nuevo directamente dentro de `%TEMP%\pixpin-mcp\`** (si no, se hace igual pero no se escribe nada). Se escribe al lado (`.parte`) y se renombra: quien espera puede leerlo en cuanto aparece. `origen` («Claude») sale en el aviso y en la pregunta |

### El asistente desde fuera (`asistente`, 13-oct-2026)

```json
{ "pixpin": 1, "accion": "asistente", "texto": "apunta comprar yeso y mañana revisar el encofrado",
  "proyecto": "pr-1789412424738", "imagenes": [] }
```

- La app contesta «aceptado» al momento y el turno va **en su hilo** (`automatizaciones::encargo`):
  escribe la pregunta en el chat «Asistente» (como el pedido `chat`; con `proyecto`, la pregunta
  lleva `@<nombre>`), hace nacer la burbuja de la respuesta y corre el turno con el mismo ejecutor
  del chat: las acciones de la app van por este mismo protocolo, y lo dibujado, sin un lienzo
  abierto, al lienzo del asistente en su chat.
- **Cola de encargos** (15-oct, `automatizaciones::cola`): el pedido solo apunta el encargo; un
  hilo los hace de uno en uno (con el modelo local solo cabe uno cargado, `pixpin_ia::relevo`).
  Se guardan en `<raiz>\asistente-encargos.json`
  (`{"version":1,"encargos":[{id, texto, proyecto?, imagenes, estado: "en_cola"|"haciendo"|"hecha"|"fallo", creado, empezado?, acabado?, resumen, con_fallos?, abrir?: {proyecto, codigo, lienzo?}}]}`):
  lo que quedó en cola al cerrar PixPin se hace al volver; el que estaba a medias **no se
  repite** (podría apuntar dos veces lo mismo): queda como fallido y se avisa. Se guardan los
  10 últimos acabados.
- Al acabar cada uno, **notificación de Windows** (`pixpin_shell::notificacion`): «✅ Hecho: …» /
  «⚠️ Hecho a medias: …» / «❌ No se pudo: …», una línea con la respuesta o el motivo y
  **«Abrir»** (el lienzo si dibujó; si no, la respuesta en el chat «Asistente»). Si Windows no
  la deja salir, el aviso flotante de siempre. Si el turno pide permiso (algo que borra o
  envía), sale el aviso flotante con «Sí»/«No» en ese momento.
- En el lanzador: `p asistente <pedido>` o `p a <pedido>` (también `ia`, `?`, `ai`); `@proyecto`
  al final elige el chat; Ctrl+V pega la imagen copiada (`[img 01]`), como en `p s`. Intro lo
  manda y **Flow se queda en `p asistente `** con el encargo ya «en cola»: se mandan varios
  seguidos. La lista enseña el motor que contesta y los encargos (en cola, haciendo, hecha,
  falló). `p asistente` a secas ofrece además abrir el chat y las automatizaciones «a mano».

### Notificaciones con botones (15-oct-2026)

Las notificaciones de Windows de PixPin (asistente, recordatorios, capturas que caducan) llevan
botones; cada botón es **un pedido de este protocolo**. Como PixPin no está empaquetado, Windows
los devuelve por protocolo: `pixpin:pedido?json=<pedido en %-código>` (o `pixpinmax:` si `pixpin:`
es de otro programa, como el PixPin original; `orden::esquema_de_la_casa`), que abre
`pixpinmax.exe --url "<seña>"`. Es **el mismo protocolo, traductor y lista blanca** que los enlaces
de `docs/integraciones.md` (`orden::de_enlace`, `orden::seguridad`); los botones van **firmados**
(`orden::firma`) y se hacen sin preguntar. `--atajo url "<seña>"` (como se registraba antes) es un
alias de `--url`.

### Automatizaciones (`automatizacion`, 13-oct-2026)

Una automatización es **un disparador y una lista de pasos**. Se crean pidiéndoselo al asistente
(«cada vez que capture, ponlo en el chat de la obra y hazle OCR»: herramienta
`crear_automatizacion`, que **pregunta antes**), se encienden, apagan, corren y borran en
Ajustes → Asistente. El formato (JSON en camelCase, igual en el móvil) vive en
`proyectos/<asistente>/android/guardados/asistente/automatizaciones.json`, que viaja como
`pixpin:files/guardados/asistente/automatizaciones.json` porque lo señala un mensaje ARCHIVO fijo
del chat «Asistente» (id `auto-automatizaciones`): **solo va a los aparatos propios** del grupo,
como un `.leccion`.

```json
{ "version": 1,
  "automatizaciones": [
    { "id": "auto-1789412424738-0", "nombre": "Capturas a la obra con OCR", "activa": true,
      "disparador": { "tipo": "al_capturar" },
      "pasos": [
        { "tipo": "accion", "herramienta": "escribir_en_chat", "args": { "texto": "Captura {fecha} {hora}" },
          "proyecto": "Obra", "imagenes": ["{captura}"] },
        { "tipo": "asistente", "pedido": "Hazle OCR a la captura", "imagenes": ["{captura}"] },
        { "tipo": "accion", "herramienta": "escribir_en_chat", "args": { "texto": "{resultado}" }, "proyecto": "Obra" }
      ],
      "aparato": "K7Q2", "creada": 1789412424738, "tocada": 1789412424738 },
    { "id": "auto-1789412500000-1", "nombre": "Resumen de pendientes", "activa": true,
      "disparador": { "tipo": "a_la_hora", "hora": "08:00", "dias": [1, 2, 3, 4, 5] },
      "pasos": [ { "tipo": "asistente", "pedido": "Resume mis tareas pendientes" } ] }
  ] }
```

- `disparador.tipo`: `al_capturar` (al guardarse una captura nueva en la carpeta de capturas),
  `a_la_hora` (`hora` local `HH:MM`; `dias` 1 lunes … 7 domingo, vacío = todos; si el aparato
  estaba apagado a esa hora, corre al encenderse si no han pasado 6 h), `al_sincronizar` (al acabar
  una vuelta de fondo que trajo algo) o `a_mano`.
- Cada paso es `{"tipo":"accion", herramienta, args?, proyecto?, imagenes?}` —una fila de la tabla
  del asistente (`crates/pixpin-ia/src/acciones.rs`, el pedido que armaría el asistente; `proyecto`
  es el nombre o el id del chat)— o `{"tipo":"asistente", pedido, proyecto?, imagenes?}`, un turno
  del asistente como `asistente` (al capturar, la captura va adjunta aunque no se diga).
- Variables en cualquier texto: `{captura}`, `{resultado}` (lo que contestó el paso anterior),
  `{fecha}`, `{hora}`, `{sincro}`, `{nombre}`. Los pasos van en orden y el primero que falla para
  los demás.
- `aparato`: el código del aparato que la creó, el único donde corre (que «a las 8» no salga dos
  veces); sin él, en todos. Lo que ya corrió (`ultimas`) es de cada aparato y no viaja
  (`<raiz>\asistente\automatizaciones-estado.json`).
- **Sin bucles**: una `al_capturar` no puede capturar, una `al_sincronizar` no puede sincronizar y
  ninguna puede crear automatizaciones (se rechaza al crearla y se bloquea al correr, también si lo
  pide el asistente dentro de un paso); además, cada una corre como mucho 30 veces por hora y no
  dos veces en 3 s.
- Lo que borra o saca algo del PC **pregunta siempre**, también desde una automatización.
- Una entrada que no se entiende (de una versión más nueva) se guarda tal cual y se lista como
  rota. Al acabar cada vuelta sale el aviso flotante con lo que pasó.

### Biblioteca de figuras (para Android)

Las figuras guardadas del lienzo («Guardar como figura» del panel, o «guarda esto como árbol
isométrico» al asistente: herramienta `figuras_guardadas`) viven en **el mismo fichero y formato
que `BibliotecaStore.kt`**: `figuras.json`, lista de `{id, nombre, elementos, propia}` con las
piezas como elementos de Excalidraw. En el PC está en
`proyectos/<asistente>/android/pins/draw/figuras.json`, que viaja como
`pixpin:files/pins/draw/figuras.json` —la ruta de `BibliotecaStore` en el móvil— porque lo
señala un mensaje ARCHIVO fijo del chat «Asistente» (id `auto-figuras`): **solo va a los aparatos
propios**, como las automatizaciones; nunca a personas de un chat compartido.

Lo que falta en Android: al guardar o quitar una figura (`BibliotecaStore.guardar`), asegurar en
el chat «Asistente» el mensaje ARCHIVO con id `auto-figuras` y ruta
`pixpin:files/pins/draw/figuras.json` (uno solo, no uno por guardado), igual que con
`auto-automatizaciones`; y releer la lista (`revision++`) cuando la sincronización traiga el
fichero. El PC escribe las figuras nuevas al principio, conserva los `id` y lee los `type` de
Excalidraw del móvil (prueba de ida y vuelta: `biblioteca_figuras::pruebas`).

### Una imagen al lienzo del móvil (`enviar_al_movil`, 6-oct-2026)

```json
{ "pixpin": 1, "accion": "enviar_al_movil", "aparato": "6f1c…-…",
  "imagenes": ["C:\\Users\\yo\\AppData\\Roaming\\PixPinMax\\cache\\lanzador-imagenes\\img-1.png"] }
```

- La app contesta «aceptado» al momento y lo hace **en un hilo** (`sincronizar::al_movil`):
  busca al móvil en su ultima direccion (`sincro\direcciones.txt`, un `PING` de un segundo) y,
  si no contesta, por mDNS (`_pixpin._tcp`, por el `id` y la etiqueta del grupo de su TXT);
  conecta por el canal cifrado del grupo (puerto 47474), saluda y manda cada fichero con la
  peticion `suelto` (`crates/pixpin-sincro/src/al_lienzo.rs`).
- El móvil la pone **en el centro del lienzo que tenga delante** y la guarda; si no tiene un
  lienzo abierto, va a su Conversación general. Lo que no es una imagen (`archivos`) va siempre
  a su chat.
- Lo que pasó se dice en el globo al acabar: «Enviada al lienzo de Pixel», «Pixel la guardó en
  el chat (no tenía un lienzo abierto)», «Pixel no está abierto: abre PixPin en el móvil» (solo
  escucha con una pantalla de PixPin a la vista). Un PixPin del móvil anterior a esto contesta
  «No sé qué es «suelto»»: se dice «Actualiza PixPin en Pixel…» y se abre «Enviar por Wi-Fi» con
  los mismos ficheros, el envío de siempre.
- En el lanzador: `p s` (o `sincronizar`, `sync`, `móvil`, `celular`, `enviar`) + Ctrl+V de la imagen (`[img 01]`):
  sale «Todos» (si hay más de uno) y una fila por aparato del grupo, con su estado; Intro manda. Lo escrito detras de las fichas elige el
  móvil por su nombre (`p s [img 01] pixel`). Sin imagen, las mismas filas sincronizan (pedido `sincronizar`). Sin grupo, la fila abre Sincronizar
  (`ventana {cual:"sincronizar"}`).
- Lo que tiene que hacer el móvil: `docs/investigacion/2026-10-06-foto-al-lienzo-android.md`.

### Tareas con imagenes (`anadir_tarea.imagenes`)

`imagenes` es una lista de **rutas absolutas** de imagenes (png, jpg, jpeg, bmp, gif, webp) de
este equipo. En `texto`, las fichas `[img 01]`, `[img 02]`… (desde 1, en el orden de la lista:
`[img 01]` es `imagenes[0]`) marcan donde va cada una:

```json
{ "pixpin": 1, "accion": "anadir_tarea", "texto": "comprar yeso [img 01]",
  "imagenes": ["C:\\Users\\yo\\AppData\\Roaming\\PixPinMax\\cache\\lanzador-imagenes\\pegada-1.bmp"] }
```

- PixPin **copia** cada imagen a `archivos/` del chat de la lista (con nombre propio,
  `tarea-<ms>-<n>.<ext>`; un `.bmp` se guarda como `.png`) y cambia su ficha por
  `![img 01](pixpin:files/guardados/pc/<chat>/archivos/tarea-…png)`. El original no se toca y
  la tarea no depende de el: se puede borrar despues.
- La imagen cuya ficha no esta en `texto` va al final del texto (antes de la fecha). Una
  ficha sin imagen (`[img 05]` con tres imagenes) se queda como texto.
- Un `texto` vacio vale si hay imagenes: la tarea es solo la imagen.
- Si falta alguna imagen o alguna no es una imagen, **no se apunta nada ni se copia nada** y se
  avisa («No se encuentra la imagen …»). Igual si `codigo` no es una lista.
- El formato de la tarea esta en `docs/investigacion/2026-10-03-tareas-con-imagenes-android.md`.

### Ficheros en un mensaje del chat (`chat.archivos`, 4-oct-2026)

`archivos` es una lista de **rutas absolutas** de ficheros cualesquiera (un PDF, un Excel, un
`.dwg`…). En `texto`, `[archivo 01]`, `[archivo 02]`… (desde 1, en el orden de la lista) marcan
cada uno y se quitan del texto:

```json
{ "pixpin": 1, "accion": "chat", "proyecto": "pr-1789412424738",
  "texto": "el plano [archivo 01] y la foto [img 01]",
  "imagenes": ["C:\\Users\\yo\\AppData\\Roaming\\PixPinMax\\cache\\lanzador-imagenes\\img-1.png"],
  "archivos": ["C:\\Users\\yo\\Desktop\\plano.pdf"] }
```

- Va primero el texto sin fichas (si queda algo), luego las imagenes como fotos y luego cada
  fichero como un adjunto con su nombre, **igual que al soltarlo en el chat**
  (`ventana_chat::meter_en_proyecto`): se copia a `archivos/` del proyecto.
- Es un campo aparte de `imagenes` a proposito: una imagen se guarda como foto (un BMP pasa a
  PNG) y un fichero tal cual, sin mirar su extension.
- Si falta un fichero o es una carpeta, **no se escribe nada** y se avisa («No existe …»).
- Solo `chat` lo entiende. En el lanzador, una ficha `[archivo NN]` en una tarea, una nota o una
  leccion se queda como texto (y lo dice el subtitulo).
- De donde salen: con Flow delante y ficheros copiados en el Explorador, el Ctrl+V lo atrapa la
  app (`pegar_en_flow`) y escribe ` [archivo 01] ` por cada fichero (una imagen copiada como
  fichero sigue siendo ` [img NN] `). Un fichero, un nombre: el mismo dos veces no se repite. El
  borrador (`<raiz>\cache\lanzador-imagenes\borrador.json`, campo `archivos`) guarda la ruta
  original; no se copia nada hasta mandar el mensaje. Se escribe en el chat de un proyecto con
  `p <proyecto> > texto` (la fila «Escribir en «P»: …») o, a proposito, con `p chat texto`.

`grabar` no abre el chat: saca un **microfono flotante** siempre encima, con la señal de que
graba y el tiempo; un clic lo para y lo guarda (con su `nombre`) en el chat pedido. Al
guardarse ofrece **«Convertir en llamada»** con un selector de hora (la llamada secreta de
PixPin) y desaparece al acabar (o sola a los 10 s). Esc mientras graba la descarta sin preguntar.

## Donde leer (sin pedir nada)

Buscar se hace leyendo el disco, sin pasar por la app (funciona aunque este cerrada). La raiz
es `%APPDATA%\PixPinMax` (o la carpeta del exe si al lado hay un `pixpinmax.toml`):

- `proyectos/indice.json`: `{"proyectos":[{id, nombre, tocado, hojas, guardados?, ...}]}`.
  El de `guardados: true` es «Mensajes guardados».
- `proyectos/<id>/guardados.jsonl`: un mensaje por linea (`id, cuando, clase, texto, nombre,
  ruta, referencia, miniapp, transcripcion, enBuzon...`). `clase`: `NOTA, IMAGEN, ARCHIVO,
  VOZ, DIBUJO, PAGINA, PROYECTO, MINIAPP`. Las listas de tareas son `MINIAPP` con
  `miniapp:"tareas"` y el documento en `texto` (`# Titulo` y casillas `- [ ] x` / `- [x] x`).
  Una tarea puede acabar en `➕ AAAA-MM-DD` (su fecha de creacion) y llevar delante imagenes
  `![img 01](pixpin:files/…)` (`mini::partir` y `mini::imagenes_de` las separan del texto).
- `proyectos/<id>/proyecto.json`: las hojas del proyecto.
- Las **lecciones aprendidas**: cada una es un mensaje `ARCHIVO` con id `lec-<id>` cuya `ruta`
  acaba en `.leccion` (normalmente `pixpin:files/guardados/lecciones/<id>.leccion`, que vive en
  `proyectos/<id>/android/guardados/lecciones/`). Ese fichero es la verdad: JSON en camelCase
  (`titulo, quePaso, porQue, proxima, tipo, area, gravedad, etiquetas, etiquetasAuto,
  referencias, causas, repeticiones, caja, repasar, adjuntos…`, ver `crates/pixpin-lecciones`).
  `adjuntos` son ids de mensajes `IMAGEN`/`VOZ` del mismo chat con `respondeA: "lec-<id>"`.
  Toca repasarla cuando `repasar <= ahora` (ms).
- Nunca escribir en esos ficheros desde fuera: la app los protege con un cerrojo que solo
  existe dentro de ella. Para cambiar algo, un pedido.

## Desde la linea de mandatos

Lo normal es el propio `pixpinmax.exe` (15-oct): `pixpinmax --pedido '<json>'`,
`pixpinmax --pedido-archivo <ruta|->`, los verbos legibles (`pixpinmax tarea "comprar yeso"`) y
los enlaces `pixpinmax://…` / `pixpin://…`. Todo en **`docs/integraciones.md`**. Desde Rust, la
función para reutilizar es `pixpin_shell::mensajero::enviar_pedidos_o_arrancar`.

`pixpin-lanzador.exe pedido '<json>'` manda un pedido (arranca PixPin si hace falta) y
`pixpin-lanzador.exe buscar "<texto>"` escribe los resultados, una linea JSON por resultado.
Sin argumentos es el plugin de Flow Launcher (JSON-RPC por stdin/stdout).
