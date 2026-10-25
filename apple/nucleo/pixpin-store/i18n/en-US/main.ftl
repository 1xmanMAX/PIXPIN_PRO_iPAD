# PixPin Max — English strings.

app-nombre = PixPin Max


resultado-copiar = Copy
resultado-guardar-como = Save as…
resultado-guardar = Save
resultado-descartar = Discard

error-otra-instancia = PixPin Max is already running.
error-atajo-ocupado = Could not register { $atajo }: another application is using it.

captura-guardada = Screenshot saved to { $ruta }
captura-fallo = Could not capture the screen: { $motivo }

barra-copiar = Copy
barra-guardar = Save
barra-guardar-como = Save as…
barra-descartar = Discard

pin-no-encontrado = Not found

pin-copiar = Copy
pin-guardar-como = Save as…
pin-abrir-ubicacion = Open file location
pin-tamano-original = Original size
pin-grupo = Group
pin-sin-grupo = No group
pin-color-rojo = Red
pin-color-naranja = Orange
pin-color-ambar = Amber
pin-color-verde = Green
pin-color-cian = Cyan
pin-color-azul = Blue
pin-color-violeta = Violet
pin-color-rosa = Pink
pin-ocultar-grupo = Hide this group
pin-cerrar = Close
pin-eliminar = Delete from store…
pin-eliminar-confirmar = This will permanently delete it from the store. Are you sure?
grupos-ocultos = Hidden groups

capa-guardar-titulo = Screen annotation
capa-guardar-pregunta = Keep the drawing as a pin?
pin-reproducir = Play
pin-pausar = Pause
pin-sonido = Sound
barra-todo = Select all
pin-sin-codec = no video codec
comando-capturar-region = Capture
comando-capturar-y-copiar = Capture and copy
comando-capturar-con-scroll = Scrolling capture
comando-cuentagotas = Colour picker
comando-pinear = Crop and pin
comando-pinear-en-vivo = Live pin (watch an area live)
comando-pinear-portapapeles = Pin the clipboard
comando-anotar = Annotate the screen
comando-anotar-congelada = Annotate a screenshot
comando-abrir-chat = Open the project chat
comando-abrir-ajustes = Settings
comando-salir = Quit
comando-alternar-pines = Hide or show pins
comando-restaurar-ultimo-pin = Reopen last closed pin
comando-cerrar-todos-los-pines = Close all pins
comando-ventana-encima = Keep the window under the pointer on top
comando-copiar-texto = Copy text from an area
comando-capturar-y-anotar = Capture and annotate
comando-capturar-ultima-region = Repeat last capture
comando-grabar-gif = Record an area as GIF

# The GIF recording bar (P5b).
grabar-empezar = Record
grabar-parar = Stop
grabar-pausa = Pause
grabar-seguir = Resume
grabar-cerrar = Close
grabar-por-segundo = { $ritmo }/s
grabar-cuenta-atras = { $segundos }...

# The editor that opens when recording stops (P5b.4).
editor-ver = Play
editor-pausar = Pause
editor-anterior = Previous
editor-siguiente = Next
editor-velocidad = { $veces }x
editor-guardar = Save
editor-guardado-rapido = Quick
editor-copiar = Copy
editor-descartar = Discard

# Click-through pins (P1.4).
comando-alternar-paso-de-clics = Let clicks pass through pins
pin-dejar-pasar-clic = Let clicks pass through

# Muting the shortcuts for a while (P1.7).
comando-silenciar-atajos = Mute the shortcuts
bandeja-silenciada = PixPin Max — shortcuts muted
aviso-atajos-silenciados = Shortcuts muted
aviso-atajos-silenciados-detalle = Turn them back on from this same menu.
aviso-atajos-activos = Shortcuts active
aviso-atajos-activos-detalle = { $cuantos } shortcuts respond again.

# Delayed capture (P2.1).
comando-capturar-con-retardo = Capture after a countdown
cuenta-atras-cancelar = Esc to stop

# Pinning the Explorer selection (P1.6).
comando-pinear-seleccion = Pin the Explorer selection

# The settings window (P6).
ajustes-pestana-atajos = Shortcuts
ajustes-pestana-general = General
ajustes-pestana-captura = Capture
ajustes-pestana-dibujo = Drawing
ajustes-iman = Snap to notable points
ajustes-iman-esquinas = Corners and endpoints
ajustes-iman-medios = Midpoints
ajustes-iman-centros = Centres
ajustes-iman-radio = Grab distance (px)
ajustes-idioma = Language
ajustes-idioma-sistema = Same as Windows
ajustes-arranque = Start with Windows
ajustes-color = Color picker format
ajustes-nivel = Performance
ajustes-nivel-auto = Automatic
ajustes-nivel-completo = Full
ajustes-nivel-ligero = Light
ajustes-pdf-aligerar = Make PDFs lighter when added to the chat
ajustes-pdf-nivel = How much lighter (Small and Max: by hand only)
ajustes-pdf-nivel-sin-perdida = Exact
ajustes-pdf-nivel-equilibrado = Medium
ajustes-pdf-nivel-pequeno = Small
ajustes-pdf-nivel-extremo = Max
ajustes-retardo-captura = Countdown seconds before capturing
ajustes-limite-scroll = Maximum height of a scrolling capture, in pixels
ajustes-gif-ritmo = Frames per second when recording
ajustes-gif-retardo = Grace seconds before recording
ajustes-sin-atajo = No shortcut
ajustes-pulsa-combinacion = Press the combination…
ajustes-choca = duplicate
ajustes-cerrar = Close
ajustes-abrir-fichero = Open the settings file

# Reading the text of a pin (P4.2).
pin-copiar-texto = Copy the text in the image

# Opening an image pin in the editor canvas.
pin-abrir-en-lienzo = Open in canvas
pin-congelar = Freeze as image
pin-manejar = Control remotely
pin-dejar-de-manejar = Stop controlling remotely

# Pages of a pinned PDF.
pin-pagina-siguiente = Next page
pin-pagina-anterior = Previous page
pin-extraer-pagina = Extract this page as a pin
pin-extraer-todas = Extract every page
aviso-paginas-extraidas = Pages extracted
aviso-paginas-extraidas-detalle = { $hechas } of { $total }. The rest were left out so the screen stays usable.
chat-titulo = Projects
chat-sin-proyectos = No projects yet
chat-elige-proyecto = Pick a project to see it
chat-hojas = { $cuantas ->
    [one] 1 sheet
   *[other] { $cuantas } sheets
  }
chat-sin-mensajes = Nothing in this project yet
chat-clase-imagen = Image
chat-clase-archivo = File
chat-clase-voz = Voice note
chat-clase-dibujo = Drawing
chat-clase-pagina = Page
chat-clase-proyecto = Project
chat-clase-miniapp = Mini app
chat-escribe = Write something…
# The chat, like PixPin Android's (guardados/MensajesActivity.kt).
chat-cosas = { $n ->
    [one] 1 thing
   *[other] { $n } things
}
chat-elegidos = { $n ->
    [one] 1 selected
   *[other] { $n } selected
}
chat-lienzo = Canvas
chat-tabla = Table
chat-viene-de = Comes from
chat-comentar = Comment on this
chat-copiar = Copy
chat-fijar = Pin to top
chat-soltar = Unpin from top
chat-compartir = Share…
chat-abrir-con = Open with…
abrir-con = Open with…
abrir-con-no-existe = Can’t find the file “{ $nombre }”: it may have been moved or deleted.
abrir-con-fallo = Windows couldn’t show “Open with…” for this file.
chat-aligerar = Make the PDF lighter
chat-renombrar = Rename
chat-renombrar-barra = Name:
chat-renombrar-teclas = Enter saves · Esc keeps the old one
chat-describir = Add caption
chat-describir-editar = Edit caption
chat-describir-barra = Caption:
chat-describir-teclas = Enter saves · Esc keeps it as it was
chat-pinear = Pin
chat-recordar = Remind me
chat-rescatar = Keep this
chat-solo-la-foto = Show only the photo
chat-ver-hilo = See comments
chat-etiquetar = Add a tag
chat-quitar-etiqueta = Remove the tag
chat-reenviar = Forward
chat-unir = Add to project
chat-transcribir = Convert to text
chat-letra = Lyrics or text
chat-elegir = Select
chat-borrar = Delete
chat-borrar-aviso = It will be removed from the conversation. An attached file is deleted with it. Continue?
chat-biblioteca-audio = Audio library
chat-fondo = Chat background
chat-escala-puesta = Interface scale: { $cuanto } %
chat-fondo-mar = Sea
chat-fondo-arena = Sand
chat-fondo-brezo = Heather
chat-fondo-bosque = Forest
chat-fondo-ocaso = Sunset
chat-fondo-pizarra = Slate
chat-adj-archivo = A file
chat-adj-pagina = A page from a project
chat-adj-proyecto = A whole project
chat-adj-conversacion = A conversation
chat-adj-telepronter = Teleprompter
chat-adj-pronunciar = Pronunciation
chat-adj-miniapp = A mini-app
chat-reenviado = Forwarded
chat-copiado = Copied
chat-compartido = Copied to the clipboard: paste it wherever you want to share it
chat-compartir-sin-panel = Windows did not open the Share panel: it is on the clipboard so you can paste it wherever you want
chat-pineado = Pinned
chat-foto-en-la-nota = Photo added to your text: it is all sent as one note
chat-sin-archivo = This message has no file on this computer
chat-sin-origen = That canvas is not in this project
chat-sin-proyectos-otros = There is no other project to send it to
chat-ajustes-bandeja = Settings open from the PixPin icon in the tray
chat-proyectos-lista = Projects are the list on the left
chat-no-hay-voz = Recording voice notes does not exist on Windows yet
chat-no-hay-aligerar = Making a PDF lighter does not exist on Windows yet
chat-no-hay-unir = On Windows every chat is already a project: use Forward to take it to another
chat-no-hay-biblioteca = The audio library does not exist on Windows yet
chat-ver-dibujo-entero = Show the whole drawing
chat-no-hay-pagina = Attaching a page from a project does not exist on Windows yet
chat-no-hay-telepronter = The teleprompter does not exist on Windows yet
chat-no-se-pudo = It could not be done: check the PixPin log
chat-fecha = { $mes } { $dia }
chat-fecha-con-ano = { $mes } { $dia }, { $ano }
chat-mes-1 = January
chat-mes-2 = February
chat-mes-3 = March
chat-mes-4 = April
chat-mes-5 = May
chat-mes-6 = June
chat-mes-7 = July
chat-mes-8 = August
chat-mes-9 = September
chat-mes-10 = October
chat-mes-11 = November
chat-mes-12 = December
chat-hoy = Today
chat-ayer = Yesterday
chat-buscar = Search
chat-adjuntar = Attach
chat-fijado = Pinned message
chat-sistema-titulo = System · only travels to your devices
chat-sistema-letras = Drawing fonts
chat-sistema-figuras = Shapes
chat-sistema-automatizaciones = Automations
chat-sistema-otros = Other
chat-sistema-quitar-letra = Remove font
chat-sistema-figuras-menu = Saved canvas shapes
chat-sistema-figuras-aviso = Saved shapes are used from the canvas shapes menu
chat-sistema-automatizaciones-menu = Automations (in Settings → Assistant)
chat-sistema-memoria = Memory
chat-sistema-memoria-menu = Assistant memory (open its folder to read or edit it)
info-galeria = Gallery
info-todo = All
info-fotos = Photos
info-archivos = Files
info-voz = Voice
info-dibujos = Drawings
info-fijados = Pinned
info-buzon = Inbox
info-videos = Videos
info-enlaces = Links
info-proyectos = Projects
info-resumen = { $n ->
    [one] 1 item
   *[other] { $n } items
}  ·  { $codigo }
info-previa-posicion = { $k } of { $n }
info-vacia = Nothing here yet
info-mes = { $mes } { $ano }
info-vacia-galeria = The project's sheets will show up here
info-vacia-fotos = Photos from this chat will show up here
info-vacia-videos = Videos from this chat will show up here
info-vacia-archivos = Files from this chat will show up here
info-vacia-enlaces = Links from this chat will show up here
info-vacia-voz = Voice notes will show up here
info-vacia-dibujos = Drawings from this chat will show up here
info-vacia-fijados = What you pin will show up here
info-vacia-buzon = The inbox is empty
info-vacia-proyectos = No projects yet
info-proyecto-este = This chat
info-abrir = Open
info-abrir-enlace = Open link
info-reproducir = Play
info-ver-en-el-chat = Show in chat
info-titulo = Info
adjuntar-imagen = Photo or video
adjuntar-archivo = File
adjuntar-lienzo = New canvas
confirmar-titulo = { $cuantos ->
    [one] Add one file
   *[other] Add { $cuantos } files
}
confirmar-resto = and { $cuantos } more
confirmar-pie = Add a caption…
confirmar-cancelar = Cancel
confirmar-aceptar = Add
adjuntar-tabla = New table
tabla-nueva = Table
chat-nombre-nuevo = Project name…
hoja-volver = Esc to go back
menu-foto-lienzo = Open in canvas
menu-foto-pin = Pin it again
menu-foto-abrir = Open with Windows
recibir-titulo = Receive from phone
recibir-como = Scan the code with PixPin on your phone
recibir-esperando = Waiting for the phone…
recibir-hecho = { $cuantos } items arrived
recibir-salir = Escape to close
comando-recibir-del-movil = Receive from phone…
comando-sincronizar = Sync
adjuntar-del-movil = Bring from phone…
menu-foto-aqui = Draw right here
proyecto-renombrar = Rename
proyecto-logo-poner = Set logo image…
proyecto-logo-quitar = Remove logo image
proyecto-logo-error = Couldn't set that logo image
proyecto-borrar = Delete project
proyecto-borrar-varios = Delete selected
proyecto-borrar-aviso = It will leave the list and its folder will go to the PixPin bin. Continue?
proyecto-papelera = Deleted projects…
# Where each project is saved: its chosen folder or the usual place.
ubicacion-cambiar = Change location…
ubicacion-habitual = Move back to the usual location
ubicacion-abrir = Open project folder
ubicacion-elegir-titulo = Choose where to save the project
ubicacion-nuevo = New project
ubicacion-nuevo-en = New project in a folder…
ubicacion-no-disponible = Location not available
ubicacion-no-disponible-abrir = This project's folder is not available ({ $ruta }). Connect the drive and try again.
ubicacion-movido = Project saved in { $ruta }
ubicacion-vuelto = The project is back in the usual location
ubicacion-error-red = Projects can only be saved on a drive of this computer, not on a network drive.
ubicacion-error-no-es-carpeta = That folder does not exist on this computer.
ubicacion-error-dentro = That folder is inside the project itself or inside PixPin's data.
ubicacion-error-mover = The project could not be moved; it is still where it was.
ubicacion-error-abrir = The project folder could not be opened.

## Sync (copy of the phone's SincronizarActivity.kt)
sinc-titulo = Sync
sinc-subtitulo = Over Wi-Fi, no internet.
sinc-este-aparato = This device
sinc-tus-aparatos = Your devices
sinc-sin-grupo = Create a group on one of your devices and join from the others with its code. Only devices in the group sync with each other.
sinc-crear-grupo = Create a group
sinc-unirme = Join with a code
sinc-grupo-vacio = There is no other device in the group yet. Add one with the code below.
sinc-disponible = Available
sinc-no-encontrado = Not found
sinc-sincronizar = Sync
sinc-probar = Try
sinc-con-todos = Sync with all ({ $n })
sinc-con-todos-como = Without asking: uses what you chose last time with each one.
sinc-si-no-aparece = If it does not show up: it needs PixPin open and this Wi-Fi.
sinc-con-direccion = Connect to an address
sinc-anadir = Add another device
sinc-anadir-como = On the other device: Sync → “Join with a code”.
sinc-toca-codigo = Click to see the code
sinc-salir = Leave the group
sinc-pasar = Send something to someone
sinc-pasar-como = Just once, to anyone with PixPin on your Wi-Fi. No group needed.
sinc-recibir = Receive
sinc-enviar = Send
sinc-pasar-tambien = What arrives goes to the project list or to “Saved messages”.
sinc-actividad = Activity
sinc-escuchando = This computer can be found at { $dir } while this window is open.
sinc-nombre-titulo = Name of this device
sinc-nombre-como = This is how your other devices will see it.
sinc-unirme-titulo = Join a group
sinc-unirme-como = Type the code shown under “Add another device” on a device in the group. That device needs PixPin open and your same Wi-Fi.
sinc-codigo = Code
sinc-direccion = Address (only if it is not found)
sinc-direccion-titulo = Connect to an address
sinc-direccion-como = Type the address shown on the other device, in this same window.
sinc-la-de-este = This device's:
sinc-salir-titulo = Leave the group?
sinc-salir-como = This device will stop syncing with the others. Nothing is deleted. If you join again, the first time it will ask about what changed on both sides.
sinc-salir-boton = Leave
sinc-un-momento = One moment
sinc-no-se-pudo = It didn't work
sinc-guardar = Save
sinc-unirme-boton = Join
sinc-conectar = Connect
sinc-cancelar = Cancel
sinc-ocultar = Hide
sinc-vale = OK
sinc-con-todos-rondas = Without asking, one after another and in two rounds, so that everyone ends up with everything.
sinc-elegir-titulo = What to sync?
sinc-elegir-con = With { $otro }. What you tick ends up the same on both.
sinc-en-los-dos = On both
sinc-solo-en-uno = Only on one
sinc-este-aparato-n = This device · { $n }
sinc-elegir-como = What changed on both is merged: shape by shape, cell by cell, paragraph by paragraph. If both touched the same thing, the last change wins, and the earlier one stays in «Backups».
sinc-todo = All
sinc-nada = None
sinc-sincronizar-n = Sync { $n }
sinc-sincronizando = Syncing
sincro-borrado-alli-vuelve = Deleted on { $otro } · will be sent back
sincro-borrado-alli-se-borra = Deleted on { $otro } · WILL BE DELETED HERE (a backup is kept)
sincro-borrado-alli-se-queda = Deleted on { $otro } · unchecked: stays here
sincro-se-borran-aqui = Will be deleted HERE, because it was deleted on { $otro }: { $lista }. Uncheck it to keep it, or choose «Mine wins» to send it back.
sincro-se-borran-alli = Will be deleted on { $otro }, because you deleted it here: { $lista }.
sincro-juntar = Merge
sincro-lo-mio-manda = Mine wins
sincro-lo-mio-manda-como = One way only: what { $yo } has overwrites { $otro }. Nothing is deleted or changed here; there, whatever differs ends up as here and what they deleted comes back. Whatever only { $otro } has is kept. A backup of what gets overwritten is kept.
sincro-mandar-n = Send { $n }

## Receive over Wi-Fi (copy of the phone's RecibirActivity.kt)
rw-titulo = Receive over Wi-Fi
rw-misma-wifi = You have to be on the same Wi-Fi as the sender (or connected to their hotspot).
rw-o-escribe = or type the code
rw-pista-pega = The computer has no camera: type the six digits or paste the text of their QR with Ctrl+V.
rw-recibir = Receive
rw-prefieres = Rather be scanned? Show your own code and let the sender point their camera at it.
rw-ensenar = Show my code so they can send to me
rw-que-te-escaneen = Let them scan this
rw-esperando-mio = Waiting for someone to send you something. You can also read this code out for «Send → Scan the receiver».
rw-buscando = Looking for the sender…
rw-conectando = Connecting…
rw-no-encontrado = Nobody is sending with that code. Check the code and that you are both on the same Wi-Fi (or on one's hotspot).
rw-codigo-malo = That code is not the right one. Check it.
rw-se-corto = Cut off: { $motivo }
rw-quiere = { $de } wants to send you
rw-aparato = Device { $codigo }
rw-proyecto = Project «{ $nombre }»
rw-lienzo = Canvas «{ $nombre }»
rw-nada-coincide = None of this matches what you have: it comes in as new, without touching anything of yours.
rw-no-gracias = No, thanks
rw-aceptar = Accept
rw-recibiendo = Receiving from { $de }
rw-de = { $hechos } of { $total }
rw-recibido = Received from { $de }
rw-no-guardado = It arrived, but could not be saved.
rw-en-proyectos = { $nombre }: in the project list
rw-en-guardados = { $nombre }: in «Saved messages»
rw-en-carpeta = { $nombre }: only in the «recibidos» folder
rw-no-se-pudo = It didn't work
rw-probar-otra-vez = Try again
rw-cerrar = Close

## Send over Wi-Fi (copy of the phone's EnviarActivity.kt)
ew-titulo = Send over Wi-Fi
ew-que = What do you want to send?
ew-que-como = On the phone you choose when sharing; here, files from this computer or a whole project from the chat.
ew-archivos = Files
ew-un-proyecto = A project
ew-elige-proyecto = Choose the project
ew-sin-proyectos = There are no projects yet.
ew-preparando = Preparing…
ew-no-preparado = What you wanted to send could not be prepared.
ew-vas-a-enviar = You are sending
ew-vas-a-enviar-n = You are sending { $n } things
ew-y-mas = and { $n } more
ew-un-aparato = One device
ew-n-aparatos = { $n } devices
ew-pidiendo = Wants to receive what you send
ew-enviando = Sending · { $hechos } of { $total }
ew-esperando-acepte = Waiting for them to accept…
ew-enviado = Sent
ew-no-acepto = Declined
ew-no-aprobado = Not approved
ew-no = No
ew-enviar = Send
ew-enviar-a-todos = Send to everyone waiting
ew-pegar = Paste the receiver's code
ew-pegar-como = On the other device: Receive over Wi-Fi → «Show my code so they can send to me». Type its six digits here or paste the text of its QR with Ctrl+V.
ew-mandar = Send it
ew-dejar-de-pegar = Stop pasting
ew-en-el-otro = On the other phone: PixPin → Sync → Receive
ew-o-escribe = or type this code
ew-misma-wifi = Both on the same Wi-Fi
ew-misma-wifi-como = If there is no Wi-Fi, one can turn on their hotspot and the other join it. No internet is used.
ew-malos = Someone tried a wrong code ({ $n } of { $de }).
ew-esperando = Waiting… Several devices can connect; you press the button for each one. The code stops working when this screen closes.
ew-quemado = Someone tried a wrong code several times, so this code no longer works. Send again to get another one.
ew-cerrar = Close
ew-grupo = Your devices · no code
ew-grupo-como = They are in your group: it goes straight there, no scanning. It lands in whatever they have open (a chat; a canvas only takes photos) or in Saved messages.
ew-grupo-se-ve = On the same Wi-Fi
ew-grupo-no-se-ve = Not visible now: open PixPin on that device
ew-grupo-enviando = Sending…
ew-grupo-todos = Send to everyone visible
ew-grupo-actualiza = Update PixPin on { $movil }: it can't take direct files yet. Let it use the code below.

## «Update mine / Create as new» when receiving (RecibirActivity.kt)
rw-ya-tienes = You already have «{ $nombre }» with the same codes
rw-actualizar = Update the one I have
rw-como-nuevo = Create as new
rw-encima-de-lo-tuyo = It is written over yours. What was there stays in «Backups».
rw-entra-aparte = It comes in separately, with new codes: from now on it is another thing.
rw-proyecto-al-dia = «{ $nombre }» brought up to date
rw-guardados-al-dia = «{ $nombre }» brought up to date in Saved messages

# The header magnifier, which searches inside the conversation (task 1.3).
chat-buscar-aqui = Search here…
chat-buscar-nada = Nothing matches
chat-buscar-resultados = { $n ->
    [one] 1 result
   *[other] { $n } results
}
chat-buscar-en-contexto = See in context
chat-pagina-n = Page { $n }
chat-unido-a = Added to "{ $nombre }"
chat-unir-nada = Nothing here can be added: only photos, drawings, notes and PDFs
chat-devolver = Add back to the project
chat-devuelto = Back in "{ $nombre }"
chat-devolver-no = Could not add it back
unir-pdf-como-imagenes = Add as images
unir-pdf-empezado = Adding the PDF to the project…
unir-pdf-ya-va = That PDF is already being added
unir-pdf-no = Could not add the PDF
unir-pdf-hecho = { $hojas ->
    [one] Added 1 sheet to the project
   *[other] Added { $hojas } sheets to the project
}
unir-pdf-pintadas = { $hojas ->
    [one] Added 1 sheet (the PDF could not be copied: it was drawn)
   *[other] Added { $hojas } sheets (the PDF could not be copied: they were drawn)
}
unir-pdf-fotos = { $hojas ->
    [one] Added 1 page as an image
   *[other] Added { $hojas } of { $paginas } pages as images
}
unir-pdf-pintando = Drawing pages… { $hechas } of { $total }
unir-pdf-imagenes-progreso = Turning into images… { $hechas } of { $total }

## Backups (copy of sincro/CopiasActivity.kt from the phone)
cop-titulo = Backups
cop-portada-como = How each conversation was before receiving something over Wi-Fi and before each sync.
cop-ver = See the backups
cop-versiones = Earlier versions
cop-como-estaba = How the project was before each received transfer and each sync. Going back to one does not delete what you did afterwards.
cop-todavia-nada = No backups yet. They are made on their own before receiving something over Wi-Fi and before syncing.
cop-como-se-hacen = The last 30 of each conversation are kept. Files over 40 MB are not in the backup: there is no going back for those.
cop-hojas-mensajes = { $hojas } sheets · { $mensajes } messages
cop-sin-grandes =  · without the PDF (too large)
cop-volver = Go back
cop-cancelar = Cancel
cop-volver-titulo = Go back to this backup?
cop-volver-texto = «{ $nombre }» goes back to how it was on { $cuando } ({ $motivo }): { $hojas } sheets. The canvases you added afterwards stay. A backup of how it is now is saved first, so you can undo this.
cop-ha-vuelto = «{ $nombre }» has gone back to the backup
cop-no-se-pudo = Could not go back to the backup: { $motivo }
papelera-titulo = Deleted projects
papelera-como = They come back whole: with their name, their sheets in order, their PDF and their chat. And they stop being deleted, so syncing won't remove them again.
papelera-detalle = { $hojas } sheets · { $mensajes } messages · deleted on { $cuando }
papelera-recuperar = Recover
papelera-recuperar-todos = Recover all
papelera-ha-vuelto = «{ $nombre }» is back, whole
papelera-no-se-pudo = Could not recover «{ $nombre }»: { $motivo }
papelera-recuperados = Recovered { $n } of { $total } projects

## The document viewer: Word, EPUB books and web pages (task 2.1)
visor-html-titulo = PixPin — page
## The Office pin: Office's preview as a pin (2026-10-10)
office-titulo = Office preview
office-abriendo = Opening with the Office preview…
office-tarda = Office is taking a while (it has to start). You can wait, or choose an app with “Open with…” (the ↗ button on the bar).
office-fallo = Office couldn't preview this file. Open it in another app with “Open with…” (the ↗ button on the bar).
office-internet = This file comes from the internet and, for safety, Windows doesn't allow previewing it. Open it in another app with “Open with…” (the ↗ button on the bar), or unblock it in its Properties.
visor-titulo = PixPin viewer
visor-marcador = Bookmark here
visor-ajustes = Reading settings
visor-tamano = Text size
visor-quitar-marcadores = Remove bookmarks
visor-guardar-pagina = Save as a web page
visor-guardar-pdf = Save as PDF
visor-abrir-carpeta = Open the folder
visor-guardado = Saved to "{ $nombre }"
visor-no-guardado = Could not save it
visor-vacio = This document has no text to show
visor-ayuda = Wheel to read · +/− size · M bookmark · G settings · Esc to leave
visor-elige-emoticono = What do you mark it with?
visor-sin-marcadores = No bookmarks yet

## The readers: comfortable reading, annotating on top and the PDF reader
lector-listo = Done
lector-indice = Contents
lector-letra-fijada = The type is locked: there are notes drawn on top
lector-letra-normal = Normal type
lector-letra-fija = Monospace
lector-grosor-normal = Regular
lector-grosor-gruesa = Bold
lector-quitar-tinta = Remove the notes
lector-tinta-quitada = Notes removed (Ctrl+Z brings them back while you stay here)
lector-ayuda-docs = Wheel to read · Ctrl+wheel zoom · L listen · A annotate · M bookmark · I contents · +/− size · T type · B weight · Esc to leave
lector-pdf-titulo = PixPin PDF reader
lector-pdf-no-se-abre = Could not open the PDF
lector-pdf-abriendo = Opening the PDF…
lector-exportando = Saving the annotated PDF… { $hechas } of { $total }
lector-exportar-pdf = Save the PDF with the notes
lector-anotado = annotated
lector-ayuda-pdf = Wheel to turn pages · Ctrl+wheel zoom · Ctrl+[ Ctrl+] space to annotate · L listen · A annotate · M bookmark · Esc to leave
lector-office-titulo = Word and PowerPoint in PixPin
lector-ayuda-office = Wheel to turn pages · Ctrl+wheel zoom · drag over text to select · Ctrl+C copy · Ctrl+F find · Ctrl+P print · F5 present · A annotate · N negative · Esc to leave
lector-presentar = Present (F5)
lector-modo-lectura = Reading mode (reflowed text)
lector-imprimir = Print… (Ctrl+P)
lector-no-imprime = Couldn't open the print dialog
lector-copiado = Copied
lector-escuchar = Listen (L)
lector-sin-texto = This document has no text to read
lector-leyendo-texto = Getting the text out of the PDF to read it…
lector-sin-voz = Windows has no voice for “{ $idioma }”: add one in Settings > Time & language > Speech
lector-voz-idioma = en-US
lector-voces-microsoft-no = ☐ Microsoft voices · very natural and free; not an official service, and the text goes to Microsoft
lector-voces-microsoft-si = ☑ Microsoft voices · very natural and free; not an official service, and the text goes to Microsoft
lector-voz-de-microsoft = Microsoft voice: { $voz }
lector-voz-del-pais = the country’s
lector-voz-pidiendo = Asking Microsoft for its voices… press again in a moment
lector-voz-sin-voces = No network, or Microsoft has no voices for this language
lector-voz-de-ia = AI, reads any language
lector-voz-mujer = female
lector-voz-hombre = male
lector-voz-parrafo = Paragraph { $n } of { $total }
lector-voz-preparando = Getting the voice ready…
lector-voz-no-respondio = Microsoft did not answer
lector-voz-sin-voz = No voice
lector-verde-aqui = Green bookmark here: reading will start from this point
lector-espacio-tope = No more room on that side
lector-espacio-nada = That side has no space left
lector-sin-lado = Sideways locked: it only scrolls up and down
lector-con-lado = It moves everywhere
lector-auto = Scroll by itself (D)
lector-auto-ppm = { $ppm } words/min
lector-auto-quedan = Done in { $cuanto } · at { $hora }
lector-auto-menos-de-un-minuto = under 1 min
lector-auto-terminado = Finished
lector-auto-contando = Counting the words…
lector-ver-como-texto = View as text (like a Word file)
lector-pasando-a-texto = Turning the PDF into text…
lector-texto-escaneo = This PDF has no text to extract: it is probably a scan (a photo of the pages)
lector-texto-cifrado = The PDF is password protected
lector-texto-no-se-lee = The PDF could not be read
lector-ver-pdf-original = View the original PDF, as pages
lector-hoja = Page
sinc-presencia-apagada = Presence is off in the settings ([sincro] presencia = false): you can sync from here, but the phone will not find this computer or be able to call it.
bandeja-abrir-documento = Open a document…

## Canvas shapes and ink: arrowheads, fills and text style (group A)
lienzo-punta-ninguna = No arrowhead
lienzo-punta-flecha = Arrow
lienzo-punta-barra = Bar
lienzo-punta-circulo = Circle
lienzo-punta-circulo-hueco = Circle outline
lienzo-punta-triangulo = Triangle
lienzo-punta-triangulo-hueco = Triangle outline
lienzo-punta-rombo = Diamond
lienzo-punta-rombo-hueco = Diamond outline
lienzo-punta-inicio = Start arrowhead
lienzo-punta-fin = End arrowhead
lienzo-trama-zigzag = Zigzag
lienzo-trama-tiralineas = Ruled lines
lienzo-texto-negrita = Bold
lienzo-texto-cursiva = Italic
lienzo-texto-tachado = Strikethrough
lienzo-presion-firme = Fixed-width stroke
lienzo-flecha-codos = Elbow arrow
lienzo-flecha-libre = Freehand arrow
lienzo-arco-guia = Guide oval, not traced yet
lienzo-luces-fuerza = Light strength

## Build and measure: fill, trim, extend, points and angles (group C)
lienzo-bote = Fill bucket
lienzo-bote-ayuda = Click inside a closed gap and it gets filled, even if several different shapes form it.
lienzo-bote-abierto = This gap is not closed: the paint would leak out through a crack.
lienzo-recortar = Trim
lienzo-recortar-ayuda = Click the leftover piece of a stroke and it goes, up to where the others cross it.
lienzo-extender = Extend
lienzo-extender-ayuda = Click the end that falls short and it reaches the first thing in its way.
lienzo-extender-sin-tope = Nothing in its way: a line stretched into the void is not an extended line.
lienzo-punto = Labelled point
lienzo-punto-ayuda = Click a crossing, an endpoint or the centre of a circle and the point appears with its letter.
lienzo-punto-sin-sitio = There is nothing to name there: a point goes on a crossing, an endpoint or a centre.
lienzo-punto-serie-mayusculas = Capital letters (A, B, C…)
lienzo-punto-serie-minusculas = Small letters (a, b, c…)
lienzo-punto-serie-numeros = Numbers (1, 2, 3…)
lienzo-angulos = Live angles
lienzo-angulos-ayuda = While you move something, it shows how big the corners you are touching are.
lienzo-area = Area

## Select, transform and bind (group B)
lienzo-lazo = Lasso
lienzo-lazo-ayuda = Draw around whatever you want to pick. What falls fully inside is taken; with Shift, whatever the stroke touches too.
lienzo-lazo-encerrar = Only what falls inside
lienzo-lazo-rozar = Whatever the stroke touches too
lienzo-voltear-horizontal = Flip horizontally
lienzo-voltear-vertical = Flip vertically
lienzo-voltear-ayuda = Mirrors the selection about the centre of the whole group, not each shape about itself.
lienzo-rejilla = Ruled paper
lienzo-rejilla-ninguna = Plain
lienzo-rejilla-lineas = Squares
lienzo-rejilla-puntos = Dots
lienzo-rejilla-imanta = Snap the cursor to the squares
lienzo-rejilla-ayuda = The reference that is there before the first stroke. It does not show up when exporting.
lienzo-iman-intersecciones = Crossings between shapes
lienzo-iman-intersecciones-ayuda = The point where two strokes cut each other, which exists as a corner of neither.
lienzo-iman-bordes = Shape edges
lienzo-iman-bordes-ayuda = The set square: the cursor slides along the whole edge, not just its corners.
lienzo-iman-eje = Origin and coordinate axes
lienzo-enganche = Bind the arrow to the shape
lienzo-enganche-ayuda = Drop the tip on a shape and the arrow will follow it when you move it.
lienzo-enganche-soltar = Release the tip
lienzo-tirador-punta = Move the end
lienzo-tirador-anadir = Bend here
lienzo-copiar-estilo = Copy style
lienzo-copiar-estilo-ayuda = Take the colour, stroke and fill from one shape and paste them on others. Only what both types accept travels.
lienzo-estilo-tomado = Style taken
lienzo-estilo-sin-tomar = You have not taken a style yet.

# The attachment the document viewer can open without leaving PixPin.
chat-abrir-aqui = Open here

# The seven mini-apps from the phone, and their panel.
mini-moneda = USD
mini-tareas = To-do list
mini-gastos = Expenses
mini-cronometro = Stopwatch
mini-temporizador = Timer
mini-contador = Counter
mini-ruleta = Spinner
mini-alarma = Alarm
mini-tarea-nueva = Something to do, then Enter · ↑↓ pick, Space ticks, F2 edits
mini-gasto-nuevo = Item and amount (Dinner 42.50), then Enter · ↑↓ pick, F2 edits
mini-nombre-nuevo = A name, then Enter · Enter on empty spins, Del removes
mini-limpiar-hechas = Clear the done ones
mini-ocultar-hechas = Hide the done ones
mini-mostrar-hechas = Show the done ones
mini-avance = { $hechas } of { $de } done
mini-tarea-edad = { $dias ->
    [0] today
    [one] 1 day ago
   *[other] { $dias } days ago
  }
mini-arrancar = Start
mini-parar = Stop
mini-vuelta = Lap
mini-reiniciar = Reset
mini-sortear = Spin
mini-alarma-encender = Turn on
mini-alarma-apagar = Turn off
mini-guia-titulo = New name, then press Enter (Esc keeps the old one)
mini-guia-cambiar = Fix it and press Enter (Esc leaves it as it was)
mini-guia-duracion = Minutes or m:ss, then Enter · Space starts
mini-guia-hora = Time (7:30), then Enter · Space turns it on
chat-biblioteca-vacia = No voice notes or songs yet.
chat-biblioteca-musica = Music
chat-biblioteca-notas = Voice notes

# Recording a voice note from the chat.
chat-voz-muy-corta = Too short: hold it a little longer.
chat-voz-sin-microfono = No microphone, or PixPin isn't allowed to use it. Check Settings › Privacy › Microphone.
chat-voz-sin-codec = This Windows has no audio encoder (usually an N edition without the media feature pack).
chat-voz-fallo = The voice note couldn't be recorded.

# Shrinking an attached PDF: it lowers the photos inside, and never for the worse.
pdf-aligerado = Shrunk: from { $antes } to { $despues }
pdf-aligerar-cifrado = The PDF is encrypted and can't be rewritten without risking it.
pdf-aligerar-indice-comprimido = This PDF keeps its index compressed, and rebuilding that isn't supported here yet.
pdf-aligerar-objetos-comprimidos = This PDF keeps its objects compressed, and rebuilding those isn't supported here yet.
pdf-aligerar-sin-trailer = The PDF has no readable trailer.
pdf-aligerar-sin-objetos = No object inside the PDF was recognised.
pdf-aligerar-largo-indirecto = A stream in the PDF doesn't state its length.
pdf-aligerar-objetos-sin-entender = There are parts of the PDF this reader does not understand; rewriting it could drop them.
pdf-aligerar-sin-fotos = It has no photos to lower: the weight is text or vectors.
pdf-aligerar-ya-al-minimo = The photos are already at the minimum.
pdf-aligerar-no-compensa = It would only drop from { $antes } to { $despues }: not worth it.
pdf-aligerar-no-se-lee = The shrunk file didn't read back properly; the original stays.
aligerar-progreso = Making lighter… { $por }%
aligerar-empezado = Making the PDF lighter…
aligerar-sin-mejora = This PDF was already light: it stays as it was.

# Reminders: the time set on a message and the notice that shows up when it
# arrives. The same times the phone offers, plus ten minutes.
chat-recordar-10-min = In 10 minutes
chat-recordar-1-hora = In 1 hour
chat-recordar-3-horas = In 3 hours
chat-recordar-esta-tarde = This evening (18:00)
chat-recordar-manana = Tomorrow (9:00)
chat-recordatorio-puesto = I will remind you at { $cuando }
chat-recordatorio-quitar = Remove the reminder
chat-recordatorio-quitado = Reminder removed
chat-recordatorio-titulo = Reminder

# A message menu entry: send the attachment to someone over Wi-Fi.
chat-enviar-wifi = Send over Wi-Fi

# Turning a voice note into text with Vosk. What is missing is named with its
# file and link: PixPin never downloads anything on its own.
chat-transcribiendo = Converting to text… { $pct }%
chat-transcribir-hecha = Voice note converted to text
chat-transcribir-en-marcha = Another note is already being converted; wait for it to finish
chat-transcribir-sin-mensaje = The note is no longer in the conversation: the text has nowhere to go
chat-transcribir-cancelada = Text conversion was cancelled
chat-transcribir-otra-vez = Convert to text again
chat-transcripcion-no = Could not convert to text: nothing in the audio was understood.
chat-letra-vacia = This note has no text yet.
chat-letra-copiada = Text copied, with its timestamps
chat-recordar-hora = Pick the time…
chat-recordar-hora-a-las = Remind me at
chat-recordar-hora-teclas = Type the time · Enter sets it · Esc leaves it
chat-recordar-hora-queda = Enter: { $cuando }
chat-recordar-hora-mal = That is not a time: type it like 18:30
chat-voz-sin-motor = The speech recogniser is missing: copy { $motor } into { $donde }
chat-voz-sin-modelo = The { $modelo } speech model is missing: download it from { $enlace } and unzip it inside { $donde }
chat-voz-idioma-sin-modelo = Vosk has no speech model for this language
chat-voz-ruta-imposible = The model folder ({ $donde }) has characters the recogniser cannot open; move it to a path without accents
chat-voz-modelo-ilegible = The model in { $donde } is incomplete or could not be loaded
chat-voz-no-es-audio = This note's audio could not be read
chat-voz-audio-vacio = The audio is empty: there is nothing to convert
chat-voz-no-se-entiende = Nothing said in the note could be understood
chat-voz-sin-reconocedor = Windows has no speech recognizer for this language: add it in Settings › Time & language › Speech.
chat-voz-bajar-whisper = Converting to text needs the Whisper speech model (about { $megas } MB). It is downloading now, only this once, and then the note will be converted.
chat-voz-bajando-whisper = Downloading the speech model… { $pct }%
chat-voz-sin-modelo-whisper = The Whisper speech model (about { $megas } MB) is missing: press «Convert to text» again to download it.
chat-voz-sin-onnx = This Windows lacks the AI runtime (onnxruntime.dll): copy it into { $donde } or next to PixPin.
chat-voz-descarga = The speech model could not be downloaded ({ $que }). Check the connection and press «Convert to text» again.
chat-voz-onnx = The Whisper speech engine failed: { $detalle }
chat-dictado-oyendo = Listening… speak, then click the microphone when done
chat-dictado-fallo = Dictation failed: { $razon }
chat-dictado-preparando = Getting the microphone ready…

# The teleprompter: the text scrolls by and you read it aloud while it
# records. There is no synthetic voice anywhere, same as on the phone.
telepronter-titulo = Read aloud
telepronter-ensayar = Rehearse
telepronter-parar = Stop
telepronter-grabar = Record
telepronter-terminar = Finish
telepronter-tirar = Discard
telepronter-velocidad = speed and size
telepronter-preparate = Get ready
telepronter-muy-corta = That reading was too short
telepronter-sin-micro = No microphone available
chat-telepronter-borrador = What is typed in the box
chat-telepronter-sin-texto = There is no text to read: type something in the box or attach a .txt or .md
chat-telepronter-abierto = The teleprompter is already open
chat-telepronter-hecha = Reading saved as a voice note

# The capture stack: the little pile in the corner (icon and panel).
pila-titulo-una = 1 capture
pila-titulo-varias = { $cuantas } captures stacked
pila-copiar-elegidas = Copy selected
pila-copiar-todas = Copy all
pila-quitar = Remove
pila-copiadas = { $cuantas } captures copied: paste them with Ctrl+V

# The drawing editor's properties panel (Excalidraw's).
panel-trazo = Stroke
panel-fondo = Background
panel-relleno = Fill
panel-grosor = Stroke width
panel-estilo-trazo = Stroke style
panel-presion = Pressure
panel-trazo-a-mano = Sloppiness
panel-bordes = Edges
panel-tipo-flecha = Arrow type
panel-fuente = Font family
panel-tamano-fuente = Font size
panel-alineacion-texto = Text align
panel-puntas = Arrowheads
panel-opacidad = Opacity
panel-capas = Layers
panel-alinear = Align
panel-acciones = Actions
panel-colores = Colors
panel-tonos = Shades
panel-codigo-hex = Hex code

## The canvas paper (Excalidraw viewBackgroundColor)
fondo-lienzo = Canvas background
fondo-papeles-del-movil = Phone papers
fondo-lienzo-menu = Canvas background…

## Export and print the canvas (G1, G2, G3, F11)
exportar-menu = Export…	Ctrl+Shift+E
exportar-copiar-png = Copy as PNG	Ctrl+Shift+C
exportar-titulo = Export the canvas
exportar-que = What
exportar-todo = Whole canvas
exportar-seleccion = Selection
exportar-marcos = Each frame
exportar-escala = Scale
exportar-transparente = Transparent background
exportar-tipo-png = PNG image
exportar-tipo-svg = SVG drawing
exportar-tipo-pdf = PDF document
exportar-tipo-html = Single-file web page
exportar-vacio = There is nothing to export with this.
exportar-fallo = The canvas could not be exported.
exportar-hecho = Exported: { $nombre }
imprimir-menu = Print…	Ctrl+P
imprimir-vacio = There is nothing to print.
imprimir-fallo = The canvas could not be printed.
exportar-chat = Export…
imprimir-chat = Print…
# F11: imprimir con vista previa (el dialogo de Windows y el boton de la barra)
imprimir-pista = Print… (Ctrl+P)
imprimir-que = What to print
imprimir-que-marcos = Each frame on its own page
imprimir-que-todo = The whole canvas on one page
imprimir-que-seleccion = Only the selection
marca-elige = What do you mark it with? (1-9, 0) · Esc to cancel
marca-plantar = Click on the canvas where the mark goes · Esc cancels
marca-vista-guardada = View saved with the mark · Shift+click it to update
hojita-borrar = Clear
hojita-insertar = Insert
hojita-pegar = As a pin

# The share sheet (compartir.rs): one for everything, like the phone's.
compartir-titulo = Share
compartir-menu = Share…	Ctrl+Shift+S
compartir-proyecto = Share…
compartir-lector = Share…  (Ctrl+Shift+S)
compartir-boton = Share
compartir-guardar = Save as…
compartir-copiar = Copy to the clipboard
compartir-wifi = Send over Wi-Fi
compartir-boton-copiar = Copy
compartir-boton-wifi = Wi-Fi
compartir-boton-enlace = Link
compartir-enlace-pista = Uploads it to litterbox (3 days) and copies the link: anyone with it can open it
compartir-enlace-subiendo = Uploading…
compartir-enlace-copiado = Link copied · deleted in 3 days
compartir-enlace-fallo = Could not upload: { $motivo }
compartir-web = HTML
compartir-adjuntos = Attachments
compartir-pdf = PDF
compartir-png = PNG
compartir-jpg = JPG
compartir-svg = SVG
compartir-original = Original
compartir-originales = Originals
compartir-editable = Editable
compartir-texto = Text
compartir-csv = CSV
compartir-pixpin = .pixpin
compartir-dxf = DXF
compartir-xlsx = Excel (xlsx)
compartir-docx = Word (docx)
compartir-md = Markdown
compartir-exportar = Export…
compartir-paginas-de = What to include · { $marcadas } of { $total }
compartir-una-pagina = Which page
compartir-va-entero = Goes whole, as it is
compartir-todas = All
compartir-ninguna = None
compartir-preparando = Preparing…
compartir-marca-alguna = Tick some page
compartir-en-cuanto-este = As soon as it is ready
compartir-peso-ficheros = { $peso } · { $n } files
compartir-panel-abierto = Choose where to send it
compartir-guardado = Saved: { $nombre }
compartir-copiado = Copied to the clipboard
compartir-no-se-pudo = It could not be prepared
compartir-nada = There is nothing to share here.
compartir-coma-decimal = .
compartir-lienzo = Canvas
compartir-lienzo-entero = Whole canvas
compartir-marco = Frame
compartir-pagina = Page
compartir-anotada = With the notes
compartir-sin-nombre = Untitled
compartir-tipo-lienzo = Canvas
compartir-tipo-pagina = PDF page
compartir-tipo-foto = Photo
compartir-tipo-tabla = Table
compartir-tipo-miniapp = Mini-app
compartir-tipo-voz = Voice note
compartir-tipo-nota = Note

# Drawing tools: the ones shown in the canvas, the reader, pins and the
# screen annotator. Turning one off removes it from the bar and its key.
ajustes-pestana-herramientas = Tools
herramientas-lazo = Lasso
herramientas-rectangulo = Rectangle
herramientas-rombo = Diamond
herramientas-elipse = Ellipse
herramientas-flecha = Arrow
herramientas-flecha-codos = Elbow arrow
herramientas-flecha-libre = Freehand arrow
herramientas-linea = Line
herramientas-lapiz = Pen
herramientas-grafito = Pencil
herramientas-texto = Text
herramientas-borrador = Eraser
herramientas-resaltador = Highlighter
herramientas-foco = Spotlight
herramientas-lupa = Magnifier
herramientas-mosaico = Mosaic
herramientas-arco = Arc
herramientas-serie = Numbered steps
herramientas-punto = Labelled point
herramientas-cota = Dimension
herramientas-escalar = Calibrate scale
herramientas-escala-grafica = Scale bar
herramientas-marco = Frame
herramientas-relleno = Fill bucket
herramientas-recortar = Trim
herramientas-extender = Extend
herramientas-copiar-estilo = Copy style

## D10: a PowerPoint in the chat (diapositivas.rs)
diapositivas-titulo = Presentation
diapositivas-no-es = This is not a presentation
diapositivas-keynote = Keynote files can't be read directly: export to PowerPoint or PDF
diapositivas-sin-powerpoint = Presenting a PowerPoint needs PowerPoint installed
diapositivas-abrir-con-otra = Open it with another app?
diapositivas-preparando = Preparing the presentation with PowerPoint…
diapositivas-no-se-pudo = Could not prepare the presentation
diapositivas-no-esta = The presentation is no longer on this computer
diapositivas-siguiente = Next
diapositivas-notas = Notes
diapositivas-sin-notas = This slide has no notes
diapositivas-fin = End of the presentation
diapositivas-transiciones-si = Transitions on (T)
diapositivas-transiciones-no = Transitions off (T)

## Chat: a page from a project, the link card, «Received from» and the inbox
chat-sin-paginas = No project has pages to attach
chat-recibido-de = Received from { $de }
chat-fecha-corta = { $mes } { $dia }
chat-enlace-no-abre = The link could not be opened

## D11: a spreadsheet in the chat is a book of tables (ventana_chat/libro.rs)
libro-titulo = Spreadsheet
libro-xls-antiguo = Old .xls files can't be read: open it and save it as .xlsx
libro-abrir-fuera = Open it with another app?
libro-no-es = The file isn't a valid spreadsheet
libro-sin-hojas = The workbook has no sheet with data
hoja-preparando = Getting the sheet ready to edit…
hoja-editar-aviso = Changes are saved in the chat sheet; the original file stays the same
hoja-editando = Editing · Esc to view
hoja-rango-grande = Too many cells to do at once
hoja-no-editable = This sheet has no table: it cannot be edited
hoja-fondo-sin = No color
hoja-letra-automatica = Automatic
hoja-color-amarillo = Yellow
hoja-color-verde = Green
hoja-color-azul = Blue
hoja-color-rosa = Pink
hoja-color-morado = Purple
hoja-color-naranja = Orange
hoja-color-gris = Gray
hoja-color-celeste = Light blue
hoja-color-rojo = Red
hoja-numero-general = General
hoja-numero-numero = Number (1,234.50)
hoja-numero-moneda = Currency
hoja-numero-porcentaje = Percent (12.50 %)
hoja-numero-fecha = Date

## D9: find in the document (buscador.rs, visor.rs, lector_pdf.rs)
buscar-pista = Find in document
buscar-cuenta = { $n } of { $total }
buscar-nada = No results
buscar-leyendo = Reading the text…
buscar-sin-texto = This PDF has no text (it's an image)
buscar-cifrado = Can't search an encrypted PDF

## D5: rename from the reader (renombrar_doc.rs)
renombrar-hecho = Now called "{ $nombre }"
renombrar-ya-existe = There's already a file called "{ $nombre }"
renombrar-no-se-pudo = Couldn't change the name

## Canvas: zone, laser, image, shapes, graph and present (F8, F12, F14, G5)
herramientas-zona = Zone (rounded copy)
herramientas-laser = Laser pointer
herramientas-imagen = Image
herramientas-figuras = Shapes
figuras-grafica = Graph of a function…
figuras-tabla-en-blanco = Blank table
figuras-pegar-tabla = Paste table (from Excel)
figuras-recta = Number line
figuras-plano = Cartesian plane
figuras-espacio = xyz space
figuras-coordenadas = Coordinate table…
figuras-coordenadas-serie = Coordinates: series
figuras-coordenadas-nueva = New coordinate series…
figuras-coordenadas-esconder = Hide the series
figuras-coordenadas-ver = Show the series
coordenadas-titulo = Coordinate table
coordenadas-ayuda = X and Y with comma or point, Y goes up · Paste: from Excel or a CSV · no rows removes the series · Enter: accept · Esc: cancel
figuras-guardar-seleccion = Save selection as a shape…
figuras-quitar = Remove
figuras-mis-figuras = My shapes
figuras-ventana-titulo = Shapes
figuras-ventana-subtitulo = { $n } shapes and templates
figuras-ventana-buscar = Search shapes
figuras-ventana-todas = All
figuras-ventana-insertar-seccion = Insert
figuras-ventana-insertar = Insert
figuras-ventana-guardar-seleccion = Save selection
figuras-ventana-vacio = No shape matches
figuras-ventana-elige = Pick a shape to see it large
figuras-ventana-piezas = { $n } pieces
figuras-ventana-intervalo = Range
figuras-ventana-desde = From
figuras-ventana-hasta = To
figuras-ventana-de-a = Split every … (one sheet each)
figuras-ventana-de-a-pista = All on one sheet
figuras-ventana-semanas-desde = From week
figuras-ventana-semanas-hasta = To week
figuras-ventana-mes = Month
figuras-ventana-meses = How many months
figuras-ventana-una-hoja = 1 sheet
figuras-ventana-hojas = { $n } sheets
figuras-ventana-renombrar = Rename
figuras-ventana-quitar-seguro = Click again to remove it
figuras-nombre-titulo = Save as a shape
figuras-nombre = Name
figuras-nombre-ayuda = Enter: save · Esc: cancel
figuras-sin-tabla = The clipboard has no table
grafica-titulo = Graph of a function
grafica-formulas = y = (one curve per line)
grafica-x-desde = x from
grafica-x-hasta = x to
grafica-y-desde = y from
grafica-y-hasta = y to
grafica-escala = Pixels per unit
grafica-ayuda = Enter: insert · Shift+Enter: another curve · Tab: next field · Esc: cancel · piecewise: x^2 si x<0; 2x si x>=0
grafica-limites-mal = The limits make no sense: "from" must be less than "to"
grafica-sin-formula = Type a formula
grafica-formula-mal = Can't understand:
grafica-solo-x = It can only use x:
presentar-ayuda = ← → PgDn: next · B: black · J: laser · L: pen · Esc: exit
vista-mirando = View only · the pencil on the right edits · F5: present

## --- Voice: two languages, lyrics, teleprompter, pronounce, conversation, call (B6-B11) ---

ajustes-voz-segundo = Second language of voice notes
ajustes-voz-modo = With two languages
ajustes-voz-modo-cada-uno = Each part as it was said
ajustes-voz-modo-todo-en-uno = All in the first one
ajustes-voz-ninguno = None
ajustes-voz-ingles = English
ajustes-voz-espanol = Spanish
ajustes-voz-portugues = Portuguese
chat-letra-editar = Edit the lyrics
chat-letra-editando = The lyrics are already being edited
chat-letra-guardar = Save
chat-letra-guardada = Lyrics saved
chat-cancelar-caja = Cancel
chat-llamada-secreta = Secret call
telepronter-pausa = Pause
telepronter-seguir = Resume
telepronter-velocidad-muy-lenta = very slow
telepronter-velocidad-lenta = slow
telepronter-velocidad-normal = normal
telepronter-velocidad-rapida = fast
telepronter-velocidad-muy-rapida = very fast
pronunciar-titulo = Pronounce
pronunciar-manten = Hold the space bar (or the microphone) and speak
pronunciar-suelta = Release to hear yourself
pronunciar-otra-vez = Hold again to repeat it
pronunciar-guia = Guide
pronunciar-guia-texto = Write a text
pronunciar-guia-fichero = An image, a PDF or a note from this computer
pronunciar-sin-guia = No guide: write one, or bring an image, a PDF or a note
pronunciar-repetir = Hear again (R)
pronunciar-guardar = Save to the chat (Ctrl+G)
pronunciar-ya-guardada = Saved
pronunciar-guardado = Saved; it is being transcribed so you can see what was understood
pronunciar-idioma = Language you practise
pronunciar-idioma-ajustes = The usual one
pronunciar-oir-texto = Hear the text
pronunciar-sin-voz = Windows has no voice for that language
pronunciar-no-se-pudo = That could not be opened
pronunciar-toma = Last take: { $duracion }
pronunciar-pagina = Page { $pagina } of { $paginas }
pronunciar-abierto = Pronounce is already open
conversacion-titulo = Conversation
conversacion-explicacion = It records non-stop; each person presses their number (1 to 9) when they start talking, and 0 when nobody is. The transcript comes out with the names.
conversacion-cuantos = How many people are talking?
conversacion-persona = Person { $n }
conversacion-empezar = Start recording
conversacion-terminar = Finish and transcribe
conversacion-turnos = { $n } turns
conversacion-hablando = Talking…
conversacion-teclas = Press the number of who is talking · 0: nobody
conversacion-transcribiendo = Transcribing the conversation…
conversacion-abierta = The conversation is already open
llamada-titulo = Call
llamada-entrante = Incoming call
llamada-contestar = Answer
llamada-colgar = Hang up
llamada-altavoz = Speaker
llamada-perdida = Missed call

## PDF zone (E9): merge PDF pages into one canvas, as on the phone
fusionar-paginas = Merge into one canvas
fusionar-paginas-empieza = Putting the pages together in a canvas…
fusionar-paginas-hecho = Done: “{ $nombre }” is now in the project
fusionar-paginas-no = Those pages could not be merged

## Universe zone (batch 2: H2, H3, M1): the Cosmos theme outside the universe
ajustes-tema-cosmos = Cosmos theme (starry sky in the chat)

## Zona pines (tanda 2: C3, C4, L3): magic word, board, sheet and tools
pin-convertir-en = Convert to
pin-herramienta-temporizador = Timer
pin-herramienta-cronometro = Stopwatch
pin-herramienta-tareas = To-do list
pin-herramienta-contador = Counter
pin-herramienta-gastos = Expenses
pin-herramienta-pizarra = Board
pin-herramienta-ruleta = Roulette
pin-herramienta-lienzo = Canvas
pin-herramienta-hoja = Sheet
pin-herramienta-tabla = Table
pin-fondo-pizarra = Board background
pin-pizarra-blanca = White
pin-pizarra-negra = Black
pin-pizarra-azul = Slate blue
pin-pizarra-verde = Chalkboard green
pin-pauta-lisa = Plain
pin-pauta-cuadros = Grid
pin-pauta-rayas = Lines
pin-pauta-columnas = Columns
pin-pauta-puntos = Dots
pin-tiempo-cumplido = Time's up
pin-hoja-nombre = Sheet

## Window groups (H9) and Markdown note editor (H12)
bandeja-grupos-ventanas = Window groups…
chat-adj-nota-md = Note
chat-editar-nota = Open in the note editor
grupos-nada-abierto = No open canvases, documents or notes to save
grupos-guardar = { $n ->
    [one] Save the open window as a group…
   *[other] Save the { $n } open windows as a group…
}
grupos-abrir = Open “{ $nombre }” · { $n }
grupos-borrar = Delete “{ $nombre }”
grupos-alguna-falta = Some window in the group can no longer be opened
grupos-sin-nombre = Group
grupos-nombre = Window group name
grupos-guardar-boton = Save
nota-md-nueva = New note
nota-md-sufijo = PixPin note
nota-md-negrita = Bold
nota-md-cursiva = Italic
nota-md-tachado = Strikethrough
nota-md-codigo = Code
nota-md-enlace = Link
nota-md-titulo1 = Heading
nota-md-titulo2 = Subheading
nota-md-titulo3 = Section
nota-md-lista = List
nota-md-numerada = Numbered list
nota-md-casilla = Task list
nota-md-marcar = Check or uncheck the task
nota-md-cita = Quote
nota-md-bloque = Code block
nota-md-raya = Divider
nota-md-formula = Formula
nota-md-cortar = Cut
nota-md-copiar = Copy
nota-md-pegar = Paste
nota-md-guardar = Save
nota-md-copia = Save a .md copy…
nota-md-tipo = Markdown note
nota-md-no-guardada = The note could not be saved. Close anyway and lose the changes?
nota-md-mayus = Shift
nota-md-intro = Enter

# H12, the new note editor window (header, toolbar, menus and tables)
nota-md-compartir = Share
nota-md-tabla = Table
nota-md-imagen = Image
nota-md-fecha = Today's date
nota-md-pista-barra = Type / in the note for more
nota-md-cambiar-titulo = Rename
nota-md-fila-encima = Insert row above
nota-md-fila-debajo = Insert row below
nota-md-quitar-fila = Delete row
nota-md-columna-izquierda = Insert column left
nota-md-columna-derecha = Insert column right
nota-md-quitar-columna = Delete column
nota-md-quitar-tabla = Delete table
nota-md-boton-fila-mas = + Row
nota-md-boton-fila-menos = − Row
nota-md-boton-columna-mas = + Column
nota-md-boton-columna-menos = − Column
nota-md-meses = Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec
## Note tables: merge and colour cells (H12)
nota-md-boton-combinar = Merge
nota-md-boton-color = Colour
nota-md-combinar-celdas = Merge cells
nota-md-separar-celdas = Unmerge cells
nota-md-color-fondo = Background colour
nota-md-color-letra = Text colour
nota-md-aplicar-a = Cells|Whole row|Whole column
nota-md-nombres-fondo = No colour|Red|Green|Blue|Yellow
nota-md-nombres-letra = Automatic|Red|Green|Blue|Orange
nota-md-pista-color = Whole rows and columns: right-click
nota-md-imagen-no-copiada = The image could not be copied next to the note.

## Note comments (H12, Sep 30): the panel on the right
nota-md-com-titulo = Comments
nota-md-com-comentar = Comment
nota-md-com-ninguno = No comments
nota-md-com-pista = Select some text and press Comment (Ctrl+Alt+M)
nota-md-com-sin-ancla = Their text is no longer in the note
nota-md-com-responder-pista = Reply…
nota-md-com-responder = Reply
nota-md-com-guardar = Save
nota-md-com-cancelar = Cancel
nota-md-com-editar = Edit
nota-md-com-borrar = Delete
nota-md-com-resuelto = resolved
nota-md-com-ver-resueltos = Show resolved
nota-md-com-ocultar-resueltos = Hide resolved
nota-md-com-editado = edited
nota-md-com-borrar-hilo = Delete the comment and its replies?

## Markdown notes: images and live pages (H12)
nota-md-pagina-viva = Page from a project
nota-md-enlace-hoja = Link to a sheet
nota-md-captura = Screenshot
nota-md-ver-grande = View full size
nota-md-abrir-hoja = Open the sheet
nota-md-foto-pequena = Small
nota-md-foto-mediana = Medium
nota-md-foto-grande = Large
nota-md-foto-columna = Column width
nota-md-quitar-foto = Remove image
nota-md-hoja-borrada = The sheet is gone: this is the last copy
nota-md-otros-proyectos = Other projects
nota-md-sin-hojas = There are no project sheets to add to the note yet.
nota-md-solo-imagenes = Only images can be dropped here.
chat-insertar-en-nota = Insert into a note

## Canvas: weld vertices and number points (lienzo-geometria, 26-sep)
herramientas-nudo = Weld vertices
herramientas-bolita = Ball: sweep over to select
lienzo-nudo = Weld vertices
lienzo-nudo-ayuda = Click where two shapes touch and they are pinned together: they no longer come apart and, with a single pin, they turn around it. Click the pin to remove it; drag it with the hand to take the pinned shapes along.
lienzo-punto-serie = Point labels
lienzo-cota-pedir = Ask for the length when drawn
lienzo-panel-dureza = Pencil grade
lienzo-guia-corto = Guide
lienzo-panel-guias = Guide lines
lienzo-cota-titulo = How long and which way? (Tab switches field)
lienzo-cota-largo = Length
lienzo-cota-angulo = Angle

## Canvas: image (zone, magnifier, pixelate, share and reference; Sep 26)
lienzo-referencia-menu = Reference image…
lienzo-pegar-menu = Paste
lienzo-pista-compartir = Share… (Ctrl+Shift+S)
lienzo-pista-lupa = Magnifier (Q): on the canvas, tap a closed shape to show what lies beneath, enlarged; on screen, zooms under the cursor
lienzo-pista-mosaico = Pixelate (mosaic): drag over what you want to hide
lienzo-pista-zona = Zone (Z): drag a box to get a cropped copy you can move
lienzo-panel-mosaico = Cover
lienzo-panel-aumento = Magnification
lienzo-panel-guia = Pointer
# The spotlight (Propiedad.OSCURECER and Propiedad.ZONA on the phone)
lienzo-panel-oscurecer = Darken
lienzo-panel-zona-foco = Lit area
lienzo-pista-foco = Spotlight (F): tap a closed shape and the area around it darkens, inside its frame
lienzo-pista-serie = Numbered steps: each click places the next number

## The Chat / Shapes switch in the chat title bar
proyectos-interruptor-chat = Chat
proyectos-interruptor-figuras = Shapes
figuras-mias = My shapes
figuras-todas = All
figuras-vacia = No shapes yet. On a canvas, select something and “Save as a shape”.
figuras-cerrar = Close
figuras-borrar = Delete the shape
figuras-borrar-seguro = Press again to delete
figuras-elegir-para = Pick a shape for “{ $chat }”
figuras-adjuntar-a = Attach to “{ $chat }”
figuras-adjuntar-pista = Or double-click the shape, or press Enter.
figuras-adjuntada = Attached to “{ $chat }” ✓
asistente-adj-figura = Shape
asistente-adj-pedir-figura = Ask for a new shape
asistente-pide-figura = Make me a shape of 
asistente-mejora-figura = Improve the shape “{ $figura }”: 
chat-guardar-en-figuras = Save to my shapes
chat-figura-sin-nombre = Shape from { $chat }
chat-figura-guardada = Saved to your shapes as “{ $nombre }”
chat-figura-vacia = That canvas is empty: nothing to save
figuras-titulo = Shapes
figuras-buscar = Search a shape…
figuras-sin-resultados = No shape has that in its name or its texts.
figuras-de-serie = built-in
figuras-tuya = yours
figuras-mide = Size
figuras-piezas = Pieces
figuras-textos = Texts it has
figuras-lineas = { $n } lines
figuras-formas = { $n } shapes
figuras-de-texto = { $n } texts
figuras-trazos = { $n } hand strokes
figuras-otras = { $n } others
bandeja-figuras = Shapes…

## --- Canvas: graph and table dialogs, put into a cell (F12/F14) ---
cajetin-cancelar = Cancel
grafica-insertar = Insert
figuras-guardar = Save
figuras-editar-tabla = Edit table… (Enter)
figuras-meter-en-celda = Put the selection into its cell
tabla-blanco-titulo = Blank table
tabla-pegada-titulo = Pasted table
tabla-editar-titulo = Edit table
tabla-insertar = Insert
tabla-aplicar = Apply
tabla-mas-fila = + Row
tabla-menos-fila = − Row
tabla-mas-columna = + Column
tabla-menos-columna = − Column
tabla-cabecera = Header
tabla-pegar = Paste
tabla-ayuda = Tab and arrows: another cell · − removes the active cell's row or column · Enter: accept · Esc: cancel

## --- Canvas: the Zone to the project chat (F8, mandarLaZona on the phone) ---
zona-arrastra-copia = Drag: a copy comes out
zona-arrastra-chat = Drag: it goes to the project chat
zona-al-chat = To chat
zona-mandada = Zone sent to the «{ $proyecto }» chat
zona-no-mandada = The zone could not be sent
zona-viene-pdf = PDF «{ $proyecto }» → page { $pagina }
zona-viene-lienzo = Canvas «{ $nombre }»
zona-nombre-pagina = Zone of page { $pagina }
zona-nombre-lienzo = Zone of { $nombre }
zona-lienzo = canvas
zona-hoja-lienzo = Canvas
# Linked zone that leads to no canvas (salto_por_enlace).
zona-enlace-roto = This zone does not lead to any canvas in this project

## The canvas toolbar grouped like the phone's (GRUPOS_DE_FABRICA)
herramientas-mano = Select and move
herramientas-imprimir = Print
herramientas-compartir = Share
barra-grupo-elegir = Other ways to select
barra-grupo-trazar = Freehand
barra-grupo-formas = Shapes
barra-grupo-flechas = Arrows and lines
barra-grupo-arreglar = Fix what is drawn
barra-grupo-nombrar = Text and labels
barra-grupo-tapar = Cover and point
barra-grupo-medir = Measure
barra-grupo-marco = Frame and image
barra-grupo-sacar = Share and print
barra-pista-mano = Select and move (M)
barra-pista-deshacer = Undo (Ctrl+Z)
barra-pista-rehacer = Redo (Ctrl+Y)
barra-pista-salir = Exit (Esc)
ajustes-herramientas-sueltas = Always visible
ajustes-herramientas-donde = Tools in
ajustes-herramientas-donde-ayuda = Pick a place to turn off the tools you do not use there, without removing them elsewhere.
ajustes-herramientas-sitio-todos = Everywhere
ajustes-herramientas-sitio-pantalla = Screen
ajustes-herramientas-sitio-pin = Pins
ajustes-herramientas-sitio-lienzo = Canvas
ajustes-herramientas-sitio-lector = Reader

## PDF reader: «To project» (the notes move to a project's sheets and travel when syncing)
lector-al-proyecto = To project
lector-al-proyecto-hecho = It's in projects now, with your notes
lector-al-proyecto-fallo = The project could not be created

## Word and book reader: the four types and four weights of the phone (K16)
lector-letra-serif = Serif
lector-letra-sans = Sans
lector-letra-cursiva = Cursive
lector-grosor-fina = Light
lector-grosor-negra = Black

## Screen annotator: its pill (CapaPantalla.kt on the phone) and saving to «Saved messages»
anotador-rotulo-dibujando = Alt + double middle click: click through
anotador-rotulo-atravesando = Alt + double middle click: draw again
anotador-globo-titulo = Annotated screen
anotador-globo-guardado = Saved to Saved messages
anotador-globo-fallo = Could not save to Saved messages
anotador-globo-copiado = Copied to the clipboard
# Screen annotator: click-through in the toolbar (2026-09-29)
anotador-boton-atravesar = Click through
anotador-pista-atravesar = Click through (Space): the mouse goes to what is underneath and the drawing stays visible. Come back with the hand on the pill or with Alt + middle double-click

# Share sheet: a document PDF with its annotations or clean (2026-09-29)
compartir-con-anotaciones = With annotations
compartir-con-anotaciones-detalle = Ink, margins and bookmarks on top of the PDF; off, the clean PDF

# Share sheet: a PDF web page as lines or as images (2026-09-30, Android v0.98.1)
compartir-texto-buscable = Searchable text
compartir-texto-buscable-detalle = Pages as lines with their text; off, as images

# Secret call: who is calling and call back later (2026-09-30, Android v0.98.6)
chat-quien-llama = Who is calling: { $nombre }
chat-quien-llama-barra = Who is calling:
chat-quien-llama-teclas = Enter: pick the time · Esc leaves it
llamada-volver-en = Call back in
llamada-vuelve-a-las = It will call you back at { $hora }

## H12: editing without marks, the floating toolbar and the note font (Sep 30)
nota-md-texto-normal = Normal text
nota-md-quitar-formato = Clear formatting
nota-md-enlace-pista = Paste or type a link and press Enter
nota-md-emojis = Smile|Laugh|Love it|Fine|Like|Dislike|Thanks|Applause|Party|Done|No|Warning|Heart|Fire|Star|Idea
nota-md-letra-texto = Text font
nota-md-letra-titulos = Heading font
nota-md-tamano = Font size
nota-md-tamanos = Small|Normal|Large|Extra large
nota-md-solo-esta-nota = Only in this note
nota-md-pista-tamano = Ctrl + wheel changes it one step at a time

# --- Notes: embedded documents, chat messages and audio (H12, Oct 1) ---
nota-md-documento = Document
nota-md-del-chat = From the chat
nota-md-audio = Audio
nota-md-abrir = Open
nota-md-quitar = Remove from the note
nota-md-ir-al-mensaje = Go to the message in the chat
nota-md-mensaje-borrado = Deleted message
nota-md-no-esta = Not on this computer
nota-md-pasar-a-texto = Transcribe
nota-md-pasando-a-texto = Transcribing { $pct } %
nota-md-sin-mensajes = This project's chat has no messages to insert (a standalone note has no chat).
nota-md-nota-de-voz = Voice note

## Export a note to Word (H12, 1-oct)
nota-md-exportar-word = Export to Word…
nota-md-tipo-word = Word document
compartir-word = Word

## Requests from other programs (Flow Launcher, scripts; docs/protocolo-pedidos.md)
pedido-tareas = Tasks
pedido-escrito = Written in “{ $proyecto }”.
pedido-al-lienzo = Stacked on the canvas “{ $lienzo }”.
microfono-abriendo = Opening the microphone…
microfono-guardando = Saving…
microfono-parar = Click to stop · Esc discards it
microfono-guardada = Voice note saved
microfono-convertir = Turn into a call
microfono-listo = Done
microfono-cuando = What time should I call you?
microfono-en-15-min = In 15 min
microfono-en-1-hora = In 1 h
microfono-manana = Tomorrow 9:00
microfono-poner = Set the call
microfono-volver = Back
microfono-sonara = It will call you: { $cuando }
microfono-puesta = Done: it will call you { $cuando }
microfono-teclas = Type the time (18:30) or use the arrows · Enter sets it
soltar-aqui = Drop here to add to “{ $proyecto }”
soltar-anadiendo = Adding…
soltar-anadido-uno = 1 added to the chat
soltar-anadidos = { $n } added to the chat
soltar-ninguno = Nothing could be added
chat-anadir-arrastrando = Add by dragging…
pedido-lista-hecha = List “{ $lista }” created in “{ $proyecto }”.
pedido-tabla-hecha = Table “{ $tabla }” created in “{ $proyecto }”.
pedido-tarea-anadida = “{ $tarea }” added to “{ $lista }”.
pedido-tarea-hecha = “{ $tarea }” marked as done.
pedido-tarea-pendiente = “{ $tarea }” is pending again.
pedido-roto = A request from another program was missing something; nothing was done.
pedido-sin-proyecto = That project is no longer in PixPin.
pedido-sin-mensaje = That message is no longer in the chat.
pedido-no-es-lista = That message isn't a to-do list.
pedido-sin-tarea = The list has no task number { $indice }.
pedido-tarea-vacia = A task with no text isn't added.
pedido-sin-fichero = That file is no longer on disk.
pedido-fallo-disco = What was asked couldn't be saved. Check PixPin's log.
pedido-tarea-movida = “{ $tarea }” moved to “{ $lista }”.
pedido-tarea-cambio = The list changed while the task was being moved; nothing was touched. Look it up again.
pedido-tarea-quitada = “{ $tarea }” removed from its list.
pedido-tarea-corregida = Task changed: “{ $tarea }”.
pedido-lista-borrada = List “{ $lista }” deleted from “{ $proyecto }”.
pedido-timeline-apuntado = Added to the timeline: “{ $titulo }”.
pedido-sin-capturas = There are no captures yet.
pedido-fuera-de-capturas = That file isn't in the captures folder; it's left alone.
pedido-no-es-imagen = That file isn't an image.
pedido-imagen-no-se-pudo = The image couldn't be copied: { $motivo }
pedido-sin-imagen = The image { $ruta } can't be found; the task wasn't added.
pedido-nada-que-leer = No text arrived to read aloud.
pedido-ocr-copiado = Text copied ({ $n } characters): “{ $texto }”
pedido-ocr-sin-texto = No text was found in the image.
pedido-ocr-no-disponible = Windows OCR is not installed: add a language with text recognition in Settings.
pedido-ocr-fallo = Couldn't read the text in the image: { $motivo }

## Command line and pixpin:// links (Oct 15)
orden-titulo = PixPin Max
orden-confirmar-cabecera = A link asks PixPin Max to:
orden-confirmar-pregunta = Do it?
orden-motivo-ficheros = It opens or sends files from your disk.
orden-motivo-asistente = It asks the assistant, which can do things in PixPin.
orden-motivo-microfono = It records with the microphone.
orden-motivo-otros-aparatos = It sends data to your other devices.
orden-motivo-automatizacion = It touches an assistant automation.
orden-motivo-borra = It deletes something.
orden-motivo-cambia = It changes PixPin data.
orden-prohibido-ruta-no-local = A link can't open network files or paths outside this computer. Nothing was done.
orden-prohibido-crear-automatizacion = A link can't create automations. Nothing was done.
orden-no-se-pudo = PixPin Max couldn't do it: { $motivo }
orden-sin-app = PixPin Max isn't open and couldn't be started: { $motivo }
orden-sin-respuesta = PixPin Max didn't answer. If it's stuck, close it and open it again.
orden-version = This PixPin Max is older than the request: update it.
orden-preguntar-titulo = Ask the assistant about { $cuantos ->
    [one] { $nombre }
   *[other] { $cuantos } files
}
orden-preguntar-enviar = Ask
orden-preguntar-cancelar = Cancel

## Capture gallery (2-oct)
galeria-titulo = Captures
galeria-vacia = No captures yet. The ones you copy with Alt + drag will show up here.
galeria-abrir-carpeta = Open folder
galeria-copiada = Copied to the clipboard
galeria-pineada = Pinned
galeria-borrada = Moved to PixPin's recycle bin
galeria-no-se-pudo = Couldn't do it: { $motivo }
galeria-conservada-aviso = Kept: it is in Saved messages and won't be deleted
bandeja-galeria-capturas = Capture gallery…
chat-galeria-capturas = Capture gallery
# Lessons learned (3-oct, as on the phone)
chat-lecciones = Lessons
chat-leccion-nueva = New lesson
chat-hacer-leccion = Make a lesson
bandeja-lecciones = 💡 Lessons…
bandeja-leccion-nueva = New lesson…
bandeja-tareas = ☑ Tasks…
bandeja-timeline = 🕒 Timeline and lessons…

## Tareas (ventana)
tareas-titulo = Tasks
tareas-apuntar-boton = Add
tareas-hechas = Done ({ $n })
tareas-vacia = No tasks yet. Type above and press Enter: it goes to the Inbox, and from there you move it to its group.
tareas-cambio = The list changed in the meantime; it's up to date now. Tick it again.
tareas-mover = Move to…
tareas-mover-titulo = Move “{ $tarea }” to…
tareas-sin-grupos = There are no other task groups yet. Create a task list in a chat and it will show up here.
tareas-movida = Moved to { $lista }
tareas-imagen-no-esta = That image isn't on this computer yet: it will arrive with the sync.
tareas-pegar-no-imagen = Only images or text can be pasted into a task.
tareas-pegar-repetida = That image is already in as { $ficha }.

# Opening a photo that Windows can only read with a Microsoft Store extension.
abrir-falta-extension = Can't open { $nombre }: Windows needs "{ $extension }", free in the Microsoft Store. Install it and open the file again.

## v2-menus
v2menus-responder = Reply
v2menus-anotar-encima = Annotate on top
v2menus-mas = More
v2menus-elegir-varios = Select several
v2menus-tecla-supr = Del
v2menus-tecla-clic = click
v2menus-adj-buscar = Search: table, stopwatch, PDF…
v2menus-adj-mas = MORE
v2menus-adj-nada = Nothing matches your search
v2menus-adj-lienzo = Canvas
v2menus-adj-tareas = To-do list
v2menus-adj-tabla = Table
v2menus-adj-conversacion = Conversation
v2menus-adj-captura = Screenshot
v2menus-adj-pagina = A project page
v2menus-adj-proyecto = Whole project
v2menus-adj-movil = Bring from phone
v2menus-envio-enviar = Send
v2menus-envio-anadir = Add
v2menus-envio-fotos = { $cuantos ->
    [one] 1 photo
   *[other] { $cuantos } photos
} · to { $proyecto }
v2menus-envio-archivos = { $cuantos ->
    [one] 1 file
   *[other] { $cuantos } files
} · to { $proyecto }
## v2-ajustes
ajustes2-titulo = Settings
ajustes2-buscar = Search settings…
ajustes2-nota = Saved when you close with Done or Esc.
ajustes2-nota-punto = = different from the default.
ajustes2-deshacer = Undo
ajustes2-listo = Done
ajustes2-restablecer = Reset { $seccion }
ajustes2-resultados = Results for “{ $busqueda }”
ajustes2-resultados-sub = { $n ->
    [one] 1 setting found across all sections.
   *[other] { $n } settings found across all sections.
  }
ajustes2-sin-resultados = No setting mentions “{ $busqueda }”. Try another word.
ajustes2-sec-general = General
ajustes2-sec-general-sub = How PixPin starts and what it does with PDFs in the chat.
ajustes2-sec-captura = Capture
ajustes2-sec-captura-sub = How a capture is taken and what happens to it afterwards.
ajustes2-sec-lienzo = Canvas and tools
ajustes2-sec-lienzo-sub = The magnet, freehand strokes and which tools show in the toolbar.
ajustes2-sec-voz = Voice and transcription
ajustes2-sec-voz-sub = Turning voice notes with two languages into text.
ajustes2-sec-atajos = Shortcuts
ajustes2-sec-atajos-sub = Click a box, then press the keys. Delete leaves it without a shortcut; Esc cancels.
ajustes2-sec-sincro = Sync
ajustes2-sec-sincro-sub = How this computer finds the phone.
ajustes2-sec-apps = Default apps
ajustes2-sec-apps-sub = Open images and videos with PixPin from File Explorer.
ajustes2-sec-apariencia = Appearance
ajustes2-sec-apariencia-sub = How the chat looks.
ajustes2-sec-avanzado = Advanced
ajustes2-sec-avanzado-sub = Performance, the pen filter and the settings file.
ajustes2-g-arranque = On startup
ajustes2-g-pdf = PDF documents
# Text recognition (OCR) for the whole app: Windows or Indra (docs/ocr.md).
ajustes2-g-ocr = Text recognition (OCR)
ajustes2-ocr-motor = OCR engine
ajustes2-ocr-windows = Windows
ajustes2-ocr-indra = Indra
ajustes2-ayuda-ocr = Used by text capture, «Trace this», pins, the gallery, scanned PDFs and the assistant. Windows comes with the system. Indra (Datalab) reads photos, scans, tilted pages and small print much better, on your PC and offline; choosing it downloads { $megas } MB. Its weights are under Datalab's OpenRAIL-M license: free for personal use, research and companies under $20M; other commercial use needs their license.
ajustes2-ayuda-ocr-indra-listo = Indra is downloaded and reads all the text in the app. If it ever fails, Windows OCR is used.
ajustes2-ayuda-ocr-windows-con-indra = Windows OCR is reading. Indra is still downloaded: choose it to use it, or delete it below.
ajustes2-ocr-bajando = Downloading Indra… { $por-ciento } %. Windows OCR reads in the meantime.
ajustes2-ocr-fallo = Indra could not be downloaded: { $fallo }. Choose it again to retry; Windows OCR reads in the meantime.
ajustes2-ocr-licencia = Indra license
ajustes2-ayuda-ocr-licencia = Indra's code is Apache 2.0; its weights, Datalab's modified OpenRAIL-M. PixPin does not ship them: you download them from Hugging Face when choosing Indra.
ajustes2-ocr-corregir = Correct with the dictionary
ajustes2-ayuda-ocr-corregir = Only with Indra and on Spanish pages: fixes accents and low-confidence letters. Never touches numbers, codes or names.
ajustes2-ocr-vocabulario = My vocabulary
ajustes2-ocr-vocabulario-ninguno = None. Names, acronyms or terms Indra should prefer (E030, Arequipa…).
ajustes2-ocr-vocabulario-marcador = term, another term…
ajustes2-ocr-borrar = Delete Indra
ajustes2-ayuda-ocr-borrar = Frees its 26 MB of disk; Windows OCR reads again.
ajustes2-ocr-borrar-boton = Delete
ajustes2-g-al-capturar = When capturing
ajustes2-g-gif = GIF recording
ajustes2-g-guardar = Saving and cleanup
ajustes2-g-pila = The capture stack
ajustes2-g-ignorados = Ignored programs
ajustes2-g-regiones = Saved regions
ajustes2-g-iman = Magnet
ajustes2-g-trazo = Freehand
ajustes2-g-voz = Voice notes
ajustes2-g-atajos = Global shortcuts
ajustes2-g-sincro = On the local network
ajustes2-g-apps = Opening images and videos
ajustes2-g-tema = Theme
ajustes2-g-rendimiento = Performance
ajustes2-g-lapiz = Pen filter (Natural mode)
ajustes2-g-fichero = The settings file
ajustes2-ayuda-arranque = Stays in the tray without opening any window. Not available in portable mode.
ajustes2-ayuda-idioma = Applies from the next start.
ajustes2-g-ventanas = Windows
ajustes2-escala = Interface scale
ajustes2-ayuda-escala = The size of everything in the chat. Applies right away; in the chat also with Ctrl + "+", "-" and "0".
ajustes2-grupos-ventanas = Window groups
ajustes2-ayuda-grupos-ventanas = Save the windows you have open as a group and open them again together.
ajustes2-ayuda-pdf-aligerar = After it enters the chat, in the background.
ajustes2-pdf-nivel = How much lighter
ajustes2-ayuda-pdf-nivel = On entry only Exact and Medium are used; Small and Max. are for “Make PDF lighter” by hand.
ajustes2-retardo = Delayed capture
ajustes2-ayuda-retardo = Countdown to open a menu or a tooltip before capturing.
ajustes2-color = Color format
ajustes2-ayuda-color = How the eyedropper copies the color.
ajustes2-scroll = Maximum scrolling height
ajustes2-ayuda-scroll = Limit for long captures, so an endless page does not use up memory.
ajustes2-gif-ritmo = Frames per second
ajustes2-ayuda-gif-ritmo = Fewer = lighter file. 10 or 15 is enough for tutorials.
ajustes2-gif-retardo = Delay before recording
ajustes2-ayuda-gif-retardo = Seconds between pressing “Record” and the first frame.
ajustes2-carpeta = Captures folder
ajustes2-abrir = Open
ajustes2-caducidad = Days until deleted
ajustes2-ayuda-caducidad = After this they go to the Windows Recycle Bin, except the kept ones.
ajustes2-apilar = Stack in the corner
ajustes2-ayuda-apilar = Each capture leaves a box in the corner; click it to gather the next ones and paste them all with one Ctrl+V.
ajustes2-esquina = Box corner
ajustes2-ayuda-esquina = On the monitor where the capture was taken.
ajustes2-icono = Time on screen
ajustes2-ayuda-icono = How long the unarmed box stays after the last capture.
ajustes2-ayuda-ignorado = While this one is in front, PixPin shortcuts do nothing.
ajustes2-anadir-programa = Add a program
ajustes2-ayuda-anadir-programa = Type the executable name and press Enter.
ajustes2-anadir = Add
ajustes2-ayuda-region = { $x }, { $y } · { $ancho } × { $alto } px
ajustes2-ayuda-iman = While drawing, the cursor snaps to what is already there.
ajustes2-ayuda-iman-esquinas = Shape corners and line ends.
ajustes2-ayuda-iman-medios = The midpoint of each side.
ajustes2-ayuda-iman-centros = The center of each shape.
ajustes2-iman-radio = Snapping distance
ajustes2-ayuda-iman-radio = In screen pixels.
ajustes2-suavizado = Stroke smoothing
ajustes2-ayuda-suavizado = Natural removes the shake of a still hand without delaying the tip.
ajustes2-ayuda-voz-segundo = Whisper also understands this language inside the same note.
ajustes2-ayuda-voz-modo = What to do with what is said in the second language.
ajustes2-presencia = Let the phone find me
ajustes2-ayuda-presencia = The phone sees this computer as soon as it opens Sync. Off, no port is opened on the network.
ajustes2-lo-mio = Mine wins
ajustes2-ayuda-lo-mio = When syncing, this computer wins: only what does not exist here comes from the other side.
ajustes2-vista-rapida = Quick look with the space bar
ajustes2-ayuda-vista-rapida = In File Explorer or on the desktop, select files and press Space: they open right away in PixPin (photos, videos, PDF, Word, DWG plans, IFC and Revit models…). Space again closes it.
ajustes2-abrir-con = Offer PixPin for images and videos
ajustes2-ayuda-abrir-con = Shows in “Open with” and in Default apps. Turning it off removes what was written to the registry.
ajustes2-predeterminada = Default app
ajustes2-ayuda-predeterminada = Windows only lets you choose it in Settings: there, PixPin Max → Set default.
ajustes2-hacer-predeterminada = Make PixPin the default app…
ajustes2-ayuda-cosmos = Starry sky behind the chat project list. Applies from the next chat.
ajustes2-nivel = Performance level
ajustes2-ayuda-nivel = Automatic measures the computer at startup. Applies from the next start.
ajustes2-medir = Measure editor frames
ajustes2-ayuda-medir = Logs how long each frame takes. Only for diagnostics.
ajustes2-ritmo = Paint right before the refresh
ajustes2-ayuda-ritmo = The pen tip lags less; a missed calculation loses one refresh.
ajustes2-paneo = Scroll without repainting
ajustes2-ayuda-paneo = The Windows compositor moves and zooms the canvas. Off, every frame is repainted.
ajustes2-corte = Cutoff with the pen still
ajustes2-ayuda-corte = Lower = less shake and more lag when starting. Auto = 1 Hz.
ajustes2-beta = How much it follows speed
ajustes2-ayuda-beta = Higher = tip closer to the cursor and more shake. Auto = 0.007.
ajustes2-fichero = Settings file
ajustes2-ayuda-fichero = Whatever is not here is still there, with its comments. Hand edits apply after a restart.
ajustes2-abrir-fichero = Open the file
ajustes2-u-s = { $n } s
ajustes2-u-px = { $n } px
ajustes2-u-dias = { $n ->
    [one] 1 day
   *[other] { $n } days
  }
ajustes2-u-hz = { $n } Hz
ajustes2-nunca = Never
ajustes2-siempre = Always
ajustes2-auto = Auto
## v2-buscar
comando-buscar = 🔍 Search PixPin…
comando-grabar-clase = 🎙 Record class or meeting…
clase-titulo = Record a class or meeting
clase-sub = Records the audio and puts screenshots at their minute
clase-microfono = Microphone
clase-equipo = Computer sound (Meet, Zoom…)
clase-guardar-en = Save to: { $proyecto }
clase-empezar = Start recording
clase-nada = Choose the microphone, the computer sound or both
clase-captura = Capture
clase-terminar = Finish
clase-capturas = Screenshots: { $n } · one click repeats the area
clase-pista = “Capture” saves an area of the screen at this minute
clase-elige-zona = Pick the area on the screen…
clase-captura-hecha = Screenshot saved at { $minuto }
clase-zona-cambiada = Area changed
clase-en-pausa = Paused: nothing is being recorded
clase-tirar = Press ✕ again to discard the recording
clase-sin-zona = Couldn't capture the area
clase-abriendo = Opening the audio…
clase-guardando = Saving the class…
clase-texto = Transcribing… { $avance } %
clase-bajando = Downloading the recognizer (first time only)… { $avance } %
clase-lista = Saved to “{ $proyecto }”
clase-lista-con-texto = With { $n } screenshots and the transcript. Open it in the chat to see it next to the images.
clase-lista-sin-texto = With { $n } screenshots. For the text, press “Convert to text” on the note in the chat.
clase-sin-texto = Leave it without text
clase-listo = Done
clase-nombre = Class { $fecha }
clase-sin-sonido-equipo = Nothing is heard from the computer: no speakers or headphones are active
clase-video = Video of this screen
clase-video-corto = Recording video
clase-video-roto = The video stopped; the audio is still recording
clase-video-guardado = With the screen video.
clase-calidad-ligero = Light
clase-calidad-normal = Normal
clase-calidad-nitido = Sharp
clase-calidad-ligero-pista = 720p video at 10 fps: the smallest files (about 180 MB per hour)
clase-calidad-normal-pista = 1080p video at 15 fps: at most about 300 MB per hour
clase-calidad-nitido-pista = The screen as it is at 30 fps: small text stays readable, bigger files
clase-video-con = Video with { $codificador }
clase-codificador-grafica = the graphics card (barely uses the processor)
clase-codificador-software = the processor
clase-codificador-cpu = the processor, simple mode
clase-zona-perdida = No area was picked: press “Capture” again
clase-salvado = The rest was saved: { $n } screenshots
clase-texto-de-fondo = Close (keeps going)
clase-modo-pantalla = Screen and sound
clase-modo-capturas = Audio and screenshots
clase-modo-pantalla-sub = Records this screen's video with the sounds you choose
clase-modo-capturas-sub = Records the audio; “Capture” saves the whole screen
clase-captura-entera = Capture: the whole screen, instantly
clase-pista-entera = “Capture” saves the whole screen at this minute; the area button, just a part
clase-capturas-entera = Screenshots: { $n } · the whole screen
clase-abrir-para-estudiar = Open to study
chat-estudiar-clase = Study the class
estudiar-titulo = Study the class
estudiar-transcripcion = Transcript
estudiar-capturas = Screenshots
estudiar-marcadores = Bookmarks
estudiar-marcador = Bookmark
estudiar-seguir = Follow
estudiar-pista-teclas = Space plays · ←/→ 5 s · J/L 10 s · M bookmark
estudiar-nota-del-marcador = Type a note and press Enter (Esc cancels)
estudiar-sin-marcadores = Press “Bookmark” or M to note something at this minute.
estudiar-marcador-sin-nota = Bookmark
estudiar-marcador-guardado = Bookmark saved
estudiar-no-se-guardo = Couldn't save it
estudiar-sin-texto = No transcript yet: press “Convert to text” on the note in the chat and it will show up here by itself.
estudiar-sin-capturas = This class has no screenshots.
estudiar-sin-video = This video can't be played here; the voice note plays instead
estudiar-sin-sonido = The audio of this class can't be found
estudiar-solo-audio = Audio only
estudiar-cargando = Loading…
estudiar-ver-video = Show the video
estudiar-ver-captura = Show the screenshot
anki-enviar = Send to Anki
anki-pregunta-clase = What was explained here?
anki-nada = No cards here. Write one per line as “question :: answer”, or “Q: …” and “A: …”, or with gaps {"{{"}c1::like this{"}}"}.
anki-hecho = { $n } cards in Anki, deck “{ $mazo }”
anki-ya-estaban = Those cards were already in “{ $mazo }”
anki-fichero = Anki isn't open (or is missing AnkiConnect). I saved { $n } cards in { $donde }: in Anki, File → Import.
anki-fallo = Anki didn't accept them: { $motivo }
buscar-todo-titulo = Search PixPin
buscar-todo-pista = Search files, tasks, lessons, screenshots… or type a letter
buscar-todo-pestana-todo = All
buscar-todo-pestana-archivos = Files
buscar-todo-pestana-tareas = Tasks
buscar-todo-pestana-lecciones = Lessons
buscar-todo-pestana-capturas = Screenshots
buscar-todo-pestana-acciones = Actions
buscar-todo-cambia = switch
buscar-todo-resultados = { $n ->
    [one] 1 result
   *[other] { $n } results
  }
buscar-todo-ninguno = Nothing for “{ $consulta }”
buscar-todo-vacia = Nothing here yet
buscar-todo-ninguno-pista = Try fewer letters, or look in another tab (Tab)
buscar-todo-recientes = Recent searches
buscar-todo-borrar-todas = Clear all
buscar-todo-abierto-hace-poco = Recently opened
buscar-todo-letras = Type a letter and space
buscar-todo-mejor = Top result
buscar-todo-grupo-tareas = Tasks and lessons
buscar-todo-grupo-archivos = Files
buscar-todo-grupo-capturas = Screenshots
buscar-todo-grupo-acciones = Actions
buscar-todo-n-de-m = { $n } of { $total }
buscar-todo-pegar = Paste an image with
buscar-todo-pegar-2 = and it goes with what you type:
buscar-todo-pegar-ejemplo = t check crack
buscar-todo-letra-t = New task
buscar-todo-letra-t-sub = to the Inbox · “t buy cement”
buscar-todo-letra-n = New note
buscar-todo-letra-n-sub = in Saved messages · “n idea @project”
buscar-todo-letra-l = New canvas
buscar-todo-letra-l-sub = blank · “l plan @project”
buscar-todo-letra-g = Screenshot gallery
buscar-todo-letra-g-sub = “g wall” searches inside
buscar-todo-letra-c = Capture
buscar-todo-letra-c-sub = an area of the screen
buscar-todo-letra-u = Pin the last screenshot
buscar-todo-letra-u-sub-nada = no screenshots yet
buscar-todo-letra-a = New lesson
buscar-todo-letra-a-sub = what you learned · “a check the scale”
buscar-todo-accion-pinear = Pin
buscar-todo-accion-abrir = Open
buscar-todo-accion-apuntar = Add
buscar-todo-accion-capturar = Capture
buscar-todo-accion-crear = Create
buscar-todo-accion-grabar = Record
buscar-todo-accion-enviar = Send
buscar-todo-accion-copiar = Copy
buscar-todo-accion-reproducir = Play
buscar-todo-accion-hecha = Done
buscar-todo-accion-desmarcar = Undo done
buscar-todo-accion-entrar = Enter
buscar-todo-accion-mostrar = Show
buscar-todo-accion-pegar = Paste
buscar-todo-ver-chat = Show in chat
buscar-todo-mas = More actions
buscar-todo-pie-moverse = move
buscar-todo-pie-abrir = open
buscar-todo-pie-adjuntar = attach image
buscar-todo-pie-cerrar = close
buscar-todo-pie-borrar = clear
buscar-todo-pie-mas = more actions
buscar-todo-pie-volver = back
buscar-todo-pie-flow = Same in Flow Launcher with “p”
buscar-todo-abrir-con = Open with…
buscar-todo-borrar = Delete (can be undone)
buscar-todo-medidas = Screenshot · { $ancho } × { $alto } · { $peso }
buscar-todo-caduca = Expires in { $dias ->
    [one] 1 day
   *[other] { $dias } days
  }
buscar-todo-caduca-hoy = Expires today
buscar-todo-conservada = Kept: never expires
buscar-todo-conservada-ya = Kept
buscar-todo-conservar = Keep
buscar-todo-borrada = Screenshot deleted
buscar-todo-deshacer = Undo
buscar-todo-copiado = Copied
## v2-lecciones
# The lessons window of the v2 redesign (Lecciones2.dc.html and Lecciones2-repaso.dc.html).
lec2-que-aprendiste = What did you learn? Type or dictate it and press Enter
lec2-guardar = Save
lec2-rellena-solo = Filled in for you, change it with a click:
lec2-rellena-solo-vacio = Area, project and severity fill in by themselves as you type.
lec2-area-de = Area: { $area }
lec2-proyecto-de = Project: { $proyecto }
lec2-ya-la-tienes = You already have it: “{ $titulo }” · It happened again
lec2-pega-capturas = pastes screenshots
lec2-guardada = Lesson saved in { $donde }
lec2-apuntado-otra-vez = Noted: it happened again
lec2-no-se-pudo = That didn’t work. Try again.
lec2-solo-imagenes = Only images can be added to a lesson.
lec2-sin-area = No area
lec2-sin-proyecto = No project (general chat)
lec2-buscar = Search: word, situation, tag…
lec2-todas = All
lec2-repetidas = Repeated
lec2-graves = Serious
lec2-para-repasar = To review today · { $n }
lec2-que-harias-si = What would you do if it happened again?
lec2-repasar = Review
lec2-este-mes = This month
lec2-antes = Earlier
lec2-lista-vacia = No lessons yet. Write what you learned above.
lec2-nada-con-filtro = Nothing with this filter.
lec2-nada-con = Nothing with “{ $q }”. Try another word or note it above.
lec2-moverse = move
lec2-leve = Minor
lec2-importante = Important
lec2-grave = Serious
lec2-hoy = today
lec2-ayer = yesterday
lec2-manana = tomorrow
lec2-hace-dias = { $n } days ago
lec2-hace-semanas = { $n ->
    [one] 1 week ago
   *[other] { $n } weeks ago
  }
lec2-hace-meses = { $n ->
    [one] 1 month ago
   *[other] { $n } months ago
  }
lec2-hace-anios = { $n ->
    [one] 1 year ago
   *[other] { $n } years ago
  }
lec2-en-dias = in { $n } days
lec2-meses = Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec
lec2-clic-para-editar = Click any text to edit it · saves by itself
lec2-mas-campos = More fields…
lec2-que-paso = What happened
lec2-por-que = Why
lec2-proxima = Next time
lec2-que-paso-vacio = Add what happened…
lec2-por-que-vacio = Add why it happened…
lec2-proxima-vacio = Add what you’ll do differently next time…
lec2-editar = Edit
lec2-escribe-aqui = Type here
lec2-ayuda-linea = Enter saves · Esc finishes
lec2-ayuda-bloque = Enter saves · Shift+Enter new line · Tab to the next one
lec2-nota-de-voz = Voice note
lec2-anadir-foto = Add a photo
lec2-ver-en-chat = See in chat
lec2-pinear = Pin
lec2-pineado = Pinned on screen
lec2-borrar = Delete
lec2-borrada = Lesson deleted
lec2-deshacer = Undo
lec2-veces = { $n ->
    [one] time it happened
   *[other] times it happened
  }
lec2-la-ultima = Last time: { $fecha }
lec2-me-volvio = It happened again
lec2-gravedad = Severity
lec2-area = Area
lec2-proyecto = Project
lec2-etiquetas = Tags
lec2-anadir = + Add
lec2-nueva-etiqueta = tag and Enter
lec2-relacionadas = Related
lec2-proximo-repaso = Next review: { $cuando }
lec2-vacia-titulo = Note what you learn so you don’t repeat mistakes
lec2-vacia-texto = Write it above in one go: “what happened…, because…, next time…”. PixPin splits it, sets area, project and severity, and tells you if it happened before.
lec2-comprobacion = Before you start, check: { $hechas } of { $total }
lec2-comprobacion-vacia = Nothing under “next time…” yet. It’s the third block of each lesson.
lec2-nada-que-repasar = Nothing to review today.
lec2-repaso-titulo = Today’s review
lec2-repaso-de = { $va } of { $total }
lec2-salir = Exit
lec2-paso-pensar = Think what you’d do
lec2-paso-mirar = Look at what you noted
lec2-paso-recordar = Did you remember?
lec2-situacion = The situation
lec2-repaso-cuando = When { $pista }…
lec2-repaso-recuerda = Remember this lesson
lec2-repaso-veces = it happened { $n } times
lec2-que-harias = What would you do?
lec2-escribe-o-dicta = Type or dictate
lec2-dictar = Dictate
lec2-mostrar = Show the answer
lec2-mostrada = Answer shown
lec2-lo-que-apuntaste = What you noted
lec2-oculta = Think before looking: recalling sticks much better than rereading.
lec2-por-que-paso = Why it happened: { $causa }
lec2-abrir-completa = Open the full lesson
lec2-lo-recordabas = Did you remember? Choose with the mouse or with
lec2-nota-recordaba = I remembered
lec2-nota-a-medias = Partly
lec2-nota-olvide = I forgot
lec2-vuelve-manana = Back tomorrow
lec2-vuelve-en = Back in { $dias } days
lec2-saltar = Skip this one
lec2-repaso-pie = Your answer is for comparing: it isn’t saved.
lec2-siguiente = Next
lec2-repaso-elige = Choose 1, 2 or 3: did you remember?
lec2-repaso-hecho = Review done
lec2-repaso-hechas = { $n ->
    [one] You reviewed 1 lesson. Come back when it’s due.
   *[other] You reviewed { $n } lessons. They come back when due.
  }
lec2-repaso-volver = Back to the list
## v2-galeria
galeria-subtitulo = { $cuantas } · kept for 7 days
galeria-buscar = Search, including the text in captures
galeria-buscar-sin-ocr = Search by name or date
galeria-elegir = Select
galeria-sin-resultados = No captures with “{ $texto }”.
galeria-hoy = Today
galeria-ayer = Yesterday
galeria-dia-0 = Monday
galeria-dia-1 = Tuesday
galeria-dia-2 = Wednesday
galeria-dia-3 = Thursday
galeria-dia-4 = Friday
galeria-dia-5 = Saturday
galeria-dia-6 = Sunday
galeria-elegir-dia = Select the whole day
galeria-chip-video = Video
galeria-pinear = Pin
galeria-copiar = Copy
galeria-conservar = Keep
galeria-borrar = Delete
galeria-elegidas = { $cuantas ->
    [one] 1 selected
   *[other] { $cuantas } selected
}
galeria-elegir-todas = All
galeria-deshacer = Undo
galeria-borradas = { $cuantas } moved to PixPin's recycle bin
galeria-recuperadas = { $cuantas ->
    [one] Restored
   *[other] { $cuantas } restored
}
galeria-pineadas = { $cuantas } pinned on screen
galeria-copiadas = { $cuantas } copied as files
galeria-conservadas-aviso = { $cuantas } kept: they are in Saved messages
galeria-prorrogada = Seven more days: it will be deleted later
## v2-pines
# The pin toolbar, its right-click menu and the "Open pins" panel.
pin-v2-anotar = Annotate
pin-v2-mas = More
pin-v2-opacidad = Opacity
pin-v2-pines-abiertos = Open pins…
pin-v2-abrir = Open
pin-v2-pinear-pagina = Pin this page
pin-v2-pinear-todas = Pin all pages
pin-v2-cerrar-pin = Close pin
pin-v2-alejar = Zoom out
pin-v2-acercar = Zoom in
pin-v2-en-vivo = LIVE
pin-v2-tecla-espacio = Space
pin-v2-tecla-rueda = Wheel
pin-v2-tecla-re-pag = PgUp
pin-v2-tecla-av-pag = PgDn
pin-v2-tecla-mayus-rueda = Shift+Wheel
pin-v2-panel-titulo = Open pins
pin-v2-panel-buscar = Search a pin
pin-v2-panel-mostrar-todos = Show all
pin-v2-panel-ocultar-todos = Hide all
pin-v2-panel-cerrar-todos = Close all · can be undone
pin-v2-panel-deshacer = Undo: reopen them
pin-v2-panel-sin-grupo = No group
pin-v2-panel-oculto = Hidden
pin-v2-panel-ayuda = Click a row to bring the pin to the centre and highlight it. The eye hides it or shows it again.
pin-v2-panel-vacio = No open pins. Pin a capture, a note or a file and it will show up here.
pin-v2-panel-ocultar = Hide
pin-v2-panel-mostrar = Show
pin-v2-panel-cerrar-este = Close this pin
pin-v2-panel-cerrar-panel = Close panel
pin-v2-tipo-foto = Photo
pin-v2-tipo-nota = Note
pin-v2-tipo-pdf = PDF
pin-v2-tipo-video = Video
pin-v2-tipo-archivo = File
pin-v2-tipo-vivo = Live pin
pin-v2-tipo-herramienta = Tool
pin-v2-marca-copiar = Copy
pin-v2-marca-resaltar = Highlight
pin-v2-marca-ondulada = Wavy line
pin-v2-marca-subrayar = Underline
pin-v2-marca-tachar = Strikethrough
pin-v2-marca-tapar = Cover
pin-v2-marca-quitar = Remove mark
pin-v2-modo-texto = Select text
pin-v2-leyendo = Reading…
pin-v2-invertir-colores = Invert colors
## v2-captura
captura2-zona = Area
captura2-ventana = Window
captura2-pantalla = Screen
captura2-scroll = Scrolling
captura2-gif = Record GIF
captura2-pin-vivo = Live pin
captura2-texto = Text (OCR)
captura2-libre = Free
captura2-repetir = Repeat last area
captura2-ancho = W
captura2-alto = H
captura2-sin-abrir = Without opening this:
captura2-arrastrar-izq = + left drag
captura2-arrastrar-der = + right drag
captura2-arrastrar-centro = + middle drag
captura2-doble-centro = + middle double-click
captura2-gesto-copiar = copy
captura2-gesto-pinear = pin
captura2-gesto-pin-vivo = live pin
captura2-gesto-anotar = draw on screen
captura2-salir = exit
captura2-pista-ventana = Click: capture window · Drag: free area
captura2-pista-zona = Click: capture this · Drag: free area
captura2-copiar-color = copy the color
captura2-cambiar-formato = HEX / RGB
captura2-copiar = Copy
captura2-pinear = Pin
captura2-al-chat = To chat
captura2-ocr = Text
captura2-guardar = Save
captura2-descartar = Discard
captura2-buscar-proyecto = Search project…
captura2-recientes = Recent
captura2-el-ultimo = last used
captura2-comentario = Add a comment (optional)
captura2-enviar = Send
captura2-sin-proyectos = No project matches
captura2-enviada = Capture sent to “{ $proyecto }”.
## v2-bandeja
# The tray panel (left click on the icon). The right-click menu keeps its
# usual keys.
bandeja2-sincronizar = Sync now
bandeja2-sincronizado = Synced { $cuando }
bandeja2-sin-sincronizar = Not synced yet
bandeja2-hace-nada = just now
bandeja2-hace-min = { $n } min ago
bandeja2-hace-h = { $n } h ago
bandeja2-ayer = yesterday
bandeja2-hace-dias = { $n } days ago
bandeja2-buscar = Search an action… (just type)
bandeja2-buscar-favorito = Search what to add as a favourite…
bandeja2-sin-resultados = No action is called that
bandeja2-capturar = Capture
bandeja2-capturar-sub = Pick the area with the mouse
bandeja2-grabar = Record
bandeja2-modo-zona = Area
bandeja2-modo-scroll = Scroll
bandeja2-modo-gif = GIF
bandeja2-modo-texto = Text
bandeja2-modo-retardo = Delay
bandeja2-modo-color = Colour
bandeja2-favoritos = Favourites
bandeja2-editar = Edit
bandeja2-anadir = Add
bandeja2-elige-favoritos = Pick your favourites ({ $n } of { $tope })
bandeja2-favoritos-llenos = There are already { $tope } favourites: remove one first.
bandeja2-hecho = Done
bandeja2-fav-pinear-copiado = Pin copied
bandeja2-fav-anotar = Annotate screen
bandeja2-fav-pin-vivo = Live pin
bandeja2-pines = Pins · { $n } on screen
bandeja2-ocultar-pines = Hide the pins
bandeja2-paso-clic = Let clicks through
bandeja2-atajos = Shortcuts
bandeja2-silenciar = Mute the shortcuts
bandeja2-silenciar-sub = for games or talks
bandeja2-ultimas = Latest captures
bandeja2-ver-galeria = See the gallery
bandeja2-sin-capturas = No captures yet
bandeja2-abrir-pixpin = Open PixPin
bandeja2-tareas = Tasks
bandeja2-lecciones = Lessons
bandeja2-nueva-leccion = New lesson
bandeja2-galeria = Capture gallery
bandeja2-documento = Open document…
bandeja2-grupos = Window groups
bandeja2-para-hoy = { $n } for today
bandeja2-salir = Quit PixPin
bandeja2-salir-sub = pins will close
bandeja2-copiada = Copied
bandeja2-borrada = Moved to the bin
bandeja2-devuelta = Restored
bandeja2-deshacer = Undo
bandeja2-no-se-pudo = Couldn't do it: { $motivo }

## tareas-v3
tareas3-buscar = Search tasks, their lists and chats… ("today", "yesterday")
tareas3-encontradas = { $n ->
    [one] 1 task found
   *[other] { $n } tasks found
  }
tareas3-pendientes = { $n ->
    [0] nothing pending
    [one] 1 pending
   *[other] { $n } pending
  }
tareas3-sin-resultados = Nothing matches "{ $busqueda }"
tareas3-sin-resultados-pista = Searches each task's text and the name of its list and chat, ignoring accents and case. Try fewer words, or "today" or "yesterday".
tareas3-vaciar-busqueda = Clear search
tareas3-mover-intro = move
tareas3-mover-esc = close
tareas3-quitada = “{ $tarea }” removed
tareas3-deshacer = Undo
tareas3-repuesta = Task put back in its list
tareas3-cambio-quitar = The list changed in the meantime; it's up to date now. Remove it again.
tareas3-borrar-lista = Delete list
tareas3-borrar-lista-aviso = Delete the list “{ $lista }” in “{ $chat }” with { $n ->
    [one] its task
   *[other] its { $n } tasks
  }? It is also deleted from the chat and the phone, and can't be undone.
tareas3-lista-borrada = List “{ $lista }” deleted

## timeline (5-oct-2026)
timeline-titulo = Timeline
timeline-sub-ultimas = Last 24 hours · { $n ->
    [one] 1 moment
   *[other] { $n } moments
  }
timeline-sub-dia = { $n ->
    [one] 1 moment
   *[other] { $n } moments
  }
timeline-ultimas-24h = Last 24 hours
timeline-hoy = Today
timeline-ayer = Yesterday
timeline-exportar = Export
timeline-volver = Back
timeline-pista = What's happening? Type or dictate…
timeline-vacio = Nothing in the last 24 hours yet.
    Type or dictate what's happening and press Enter.
    When dictating, «con esto pasó…» starts the title and «y así te lo cuento…» the description.
timeline-vacio-dia = Nothing was noted that day.
timeline-nota-de-voz = Voice note
timeline-sin-titulo-voz = Voice note
timeline-sin-titulo-foto = Photo
timeline-apuntado = Noted
timeline-corregido = Corrected
timeline-borrado = Moment deleted
timeline-deshacer = Undo
timeline-abrir = Open
timeline-escribe-algo = Type something for the title
timeline-no-guardado = Couldn't save the timeline
timeline-corrigiendo = Correcting the { $hora } moment · Enter saves, Esc cancels
timeline-voz-esperando = 🎙 The voice note goes with the text: press Enter
timeline-pegar-no-imagen = That's neither an image nor text
timeline-cal-elegidos = { $n ->
    [one] 1 day selected
   *[other] { $n } days selected
  }
timeline-cal-quitar = Clear selection
timeline-exportar-html = Web page (HTML)
timeline-exportar-pdf = PDF document
timeline-tipo-html = Web page
timeline-tipo-pdf = PDF document
timeline-nada-que-exportar = Nothing to export there
timeline-no-exportado = Couldn't save the file
timeline-exportado = Saved: { $nombre }
timeline-exportado-pie = Exported with PixPin Max · { $fecha }
timeline-pdf-haciendo = Making the PDF…
timeline-pdf-sin-edge = The PDF needs Microsoft Edge; export as HTML
timeline-pdf-fallo = Couldn't make the PDF
timeline-mic-abriendo = Opening the microphone…
timeline-mic-escuchando = Listening… press the microphone (or Ctrl+M) to finish
timeline-mic-guardando = Saving what you said…
timeline-mic-bajando = Downloading the voice model (once)… { $pc } %
timeline-mic-pasando = Transcribing… { $pc } %
timeline-mic-sin-dictado = This computer can't transcribe: type it
timeline-mic-no-abre = Couldn't open the microphone
timeline-mic-no-guarda = Couldn't save what you said
timeline-mic-sin-microfono = No microphone
timeline-mic-corto = Too short: speak a bit longer
timeline-mic-error = Couldn't record: { $error }
timeline-mic-sin-texto = Not understood: only the audio is kept


## simple-lecciones
lecs-menos-campos = Fewer fields
lecs-ficha-completa = Type, causes and keywords…

## galeria-menu (5-oct-2026, right click)
galeria-guardar-como = Save as…
galeria-mostrar-en-carpeta = Show in folder
galeria-guardadas = { $cuantas ->
    [one] Saved
   *[other] { $cuantas } saved
}
galeria-prorrogar = Give 7 more days

## timeline-v2 (5-oct-2026): common bar, WeChat-like tabs and detail
timeline-pestana-hoy = Today
timeline-pestana-momentos = Moments
timeline-pestana-estado = Calendar
timeline-buscar-pista = Search the timeline
timeline-sub-buscar = { $n ->
    [0] Nothing found
    [one] 1 result
   *[other] { $n } results
  }
timeline-sub-momentos = { $n ->
    [0] No moments yet
    [one] 1 moment in total
   *[other] { $n } moments in total
  }
timeline-sin-resultados = No moment has that.
    Title and description are searched, ignoring accents and case.
timeline-momentos-vacio = The moments you pick go here. In "Today", hover a card and press the bookmark to send it here.
timeline-a-momentos = Sent to Moments
timeline-fuera-de-momentos = Removed from Moments
timeline-estado-sobre-todo = Mostly
timeline-estado-lo-mas = "{ $que }"
timeline-exportar-busqueda = Search: "{ $que }"
timeline-pestana-lecciones = Lessons
timeline-sub-lecciones = { $n ->
    [0] No lessons
    [one] 1 lesson learned
   *[other] { $n } lessons learned
  }
timeline-lecciones-vacio = No lessons yet. Press a moment's light bulb to make it a lesson.
timeline-gravedad-leve = Minor
timeline-gravedad-importante = Important
timeline-gravedad-grave = Serious
timeline-otra-vez = It happened again +1
timeline-otra-vez-hecho = Noted: { $n } times now
timeline-leccion = Lesson
timeline-leccion-hecha = 💡 Made a lesson: it is in "Lessons"
timeline-leccion-quitada = No longer marked. The lesson stays in "Lessons".
timeline-leccion-fallo = Could not save the lesson
timeline-leccion-por-que = Why
timeline-leccion-proxima = Next time
timeline-tambien-lecciones = { $n ->
    [one] 1 lesson also matches · See
   *[other] { $n } lessons also match · See
  }
timeline-copiar-imagen = Copy as image
timeline-compartir-html = Share as HTML
timeline-guardar-imagen = Save as image…
timeline-imagen-copiada = Copied as image
timeline-imagen-fallo = Could not make the image
timeline-tipo-png = PNG image
timeline-compartido-pie = PixPin Max · Timeline

## tareas-v4
# The search box hint in the shared top bar: shorter than tareas3-buscar,
# which does not fit in the space the bar leaves.
tareas4-buscar = Search tasks…
timeline-otra-vez-corto = ↻ It happened again +1
timeline-pagina-linea = Timeline
timeline-exportado-pesa = it is { $mb } MB: it may be hard to send

## v2-diseno
# The gallery with the v2 design system (Oct 6).
galeria-chapa-hoy = today
galeria-pista-caduca = { $dias ->
    [0] Deleted today
    [one] Deleted tomorrow
   *[other] Deleted in { $dias } days
  } · right-click to keep
galeria-ya-conservadas = They were already kept
galeria-dia-cuantas = { $cuantas ->
    [one] 1 capture
   *[other] { $cuantas } captures
  }
timeline-sub-estado-dias = { $n ->
    [0] No days with anything yet
    [one] 1 day with something
   *[other] { $n } days with something
  }
timeline-filtro-todas = All
timeline-orden-recientes = Recent
timeline-orden-repetidas = Most repeated
timeline-lecciones-sin-resultados = No lesson has that.
timeline-otra-vez-pildora = ↻ +1

## tareas-v5
# The bar subtitle: each group already counts what is pending.
tareas5-subtitulo = { $listas ->
    [one] 1 list
   *[other] { $listas } lists
  }
tareas5-apuntar-pista = New task…
tareas-recordare = ⏰ I will remind you { $cuando }
tareas-sin-recordatorio = Reminder removed
tareas-apuntar-en-pista = New task in “{ $lista }”…
tareas5-corregir-pista = Task text…
tareas5-imagen-pista = Add images · Ctrl+V pastes a copied one
tareas5-guardar = Save
tareas5-corregida = Task updated
tareas5-vista-lista = List · Ctrl+L
tareas5-vista-tarjetas = Cards · Ctrl+T
tareas5-vista-kanban = Kanban board · Ctrl+K
tareas5-marcar = Mark as done
tareas5-desmarcar = Mark as pending
tareas5-editar = Edit
tareas5-copiar-texto = Copy text
tareas5-copiada = Text copied
tareas5-pinear-imagenes = { $n ->
    [one] Pin the image
   *[other] Pin the { $n } images
  }
tareas5-quitar = Remove

## salida (6-oct-2026): the floating window to drag out what was exported
salida-titulo = Ready to share
salida-titulo-exportado = Exported
salida-titulo-guardado = Saved
salida-pista = Drag a file to another app or a folder
salida-todos = Drag all { $n } together
salida-arrastra-todos = Hold and drag to take them all
salida-soltado = Dropped · you can drop it somewhere else too
salida-copiado = Copied to the clipboard
salida-pineado = Pinned
salida-no-se-pudo = Couldn't do it
salida-compartir = Share
salida-copiar = Copy
salida-abrir = Open
salida-carpeta = Show in folder
salida-pinear = Pin
salida-compartir-menu = Share…
compartir-arrastrar = Drag from here to another app · click to keep it on screen

## al-movil (6-oct-2026): a photo from the Flow plugin to the phone's open canvas
al-movil-lienzo = { $n ->
    [one] Sent to the canvas on { $movil }
   *[other] { $n } images sent to the canvas on { $movil }
  }
al-movil-chat = { $movil } saved it in the general conversation (no chat or canvas was open)
al-movil-chat-abierto = { $n ->
    [one] Sent to the «{ $chat }» chat on { $movil }
   *[other] { $n } files sent to the «{ $chat }» chat on { $movil }
  }
al-movil-solo-fotos = { $movil } has a canvas open: it only takes photos. Open a chat to send it files.
al-movil-negados = { $negados ->
    [one] 1 file was not sent: it was not a photo and a canvas is open
   *[other] { $negados } files were not sent: they were not photos and a canvas is open
  }
al-movil-no-esta = { $movil } is not open: open PixPin on the phone
al-movil-actualiza = Update PixPin on { $movil }: it can't receive direct files yet. Opening «Send over Wi-Fi».
al-movil-ocupado = { $movil } is syncing with another device. Try again in a moment.
al-movil-otra-version = { $movil } has another version of PixPin: update both
al-movil-cortado = Couldn't send to { $movil }: { $motivo }
al-movil-sin-grupo = This PC is not in a group: set it up in PixPin's Sync
al-movil-no-es-del-grupo = That device is no longer in this PC's group
al-movil-sin-imagenes = There was nothing to send
al-frente-lienzo = { $de } sent you “{ $nombre }”: it's on the open canvas
al-frente-chat-abierto = { $de } sent you “{ $nombre }” to the chat “{ $chat }”
al-frente-guardados = { $de } sent you “{ $nombre }”: it's in Saved messages

## Excel sheet viewer (visor_hoja)
hoja-copiado = { $celdas ->
    [one] 1 cell copied
   *[other] { $celdas } cells copied
  }
hoja-demasiado = Too many cells to copy at once ({ $celdas })
hoja-cargando = Loading… { $filas } rows
hoja-enlace-no-abre = The link could not be opened

## Chats shared with people (Android 13-oct-2026, ChatCompartidoActivity)
cc-menu-compartir = Share with people
cc-menu-entrar = Join a shared chat
chat-nuevo-normal = Normal chat
chat-nuevo-normal-detalle = Just you and your devices
chat-nuevo-compartido = Shared chat
chat-nuevo-compartido-detalle = With other people, each on their own side
cc-chapa-personas = { $n ->
    [one] Shared · 1 person
   *[other] Shared · { $n } people
}
cc-nuevo-error = Couldn't share the new chat: it stays a normal chat
cc-portada-titulo = Shared chats
cc-portada-como = A project chat you keep with someone else: they only see that chat. To share one, right-click the project → Share with people.
cc-portada-entrar = Join a shared chat
cc-titulo-personas = People
cc-titulo-entrar = Join a chat
cc-tu-nombre = Your name in shared chats
cc-guardar = Save
cc-compartir-explica = Share this chat with other people.
cc-compartir-detalle = Only this chat travels: its messages, files, canvases and annotations. Your other chats and projects stay hidden. Everyone syncs with the others on the same Wi-Fi.
cc-compartir = Share
cc-ya-no-estas = You are no longer in this chat. What you had stays here, but it no longer syncs.
cc-tu = (you)
cc-creo = created the chat
cc-tu-otro-aparato = your other device
cc-salio = left
cc-quitado = removed
cc-quitar = Remove
cc-anadir = Add
cc-cerrar-invitacion = Close
cc-sincronizar = Sync
cc-invitacion-como = On the other device: PixPin → Join a shared chat. Scan this or type the code.
cc-invitacion-vale = Valid while this screen stays open. You must be on the same Wi-Fi.
cc-salir-del-chat = Leave chat
cc-salir-titulo = Leave the chat?
cc-quitar-titulo = Remove { $nombre }?
cc-salir-aviso = What you have stays here, but it will stop syncing with the others.
cc-quitar-aviso = They keep what they already have, but will get nothing new from this chat.
cc-salir = Leave
cc-cancelar = Cancel
cc-entrar-como = Whoever invites you opens the chat → Share with people → Add, and shows you a code. You only get that chat: nothing else from their device, and you give them nothing of yours. This computer has no camera: type the code or paste the text of their QR.
cc-escribe-codigo = Type the code or paste the QR text
cc-entrar = Join
cc-ya-estas = You are in “{ $nombre }”
cc-con = With: { $nombres }
cc-cerrar = Close
cc-compartido = Shared. Press Add to invite someone.
cc-entro = { $nombre } joined. Bringing over the chat…
cc-listo = Done: they have the chat.
cc-demasiados = Too many wrong codes: invitation closed.
cc-has-salido = You left. The others will know when you sync.
cc-quitado-aviso = Removed. They will know when they sync.
cc-nadie = Nobody else yet. Press Add.
cc-buscando = Looking for the chat’s people…
cc-sincronizando-con = Syncing with { $nombre }…
cc-nadie-en-wifi = Nobody from the chat is on this Wi-Fi right now.
cc-sincronizado = Synced with { $nombres }.
cc-buscando-invitacion = Looking for whoever invited you…
cc-entrando = Joining…
cc-trayendo = Bringing over “{ $nombre }”…
cc-no-encontrada = The invitation was not found. Check the code and that you are on the same Wi-Fi.
cc-se-corto = Interrupted: { $motivo }
cc-abrir-chat = Open chat
cc-no-se-abre = That chat is not here yet: sync again.

## Pins like on Android (batch 6): color, stickers, paged note and own words
pin-v2-pegatina = Sticker
pin-v2-sin-pegatina = No sticker
pin-v2-paginas = Pages
pin-v2-leer-seguida = Read continuously
pin-v2-pagina = Page
pin-v2-minutos = min
pin-color-copiado = Copied
ajustes2-sec-pinear = Pinning
ajustes2-sec-pinear-sub = The words that become a tool when pinned.
ajustes2-sec-tareas = Tasks
ajustes2-sec-tareas-sub = What is understood when you type a task.
ajustes2-g-urgencia = Urgency words (with none, the task is normal)
ajustes2-urgente = Urgent
ajustes2-muy-urgente = Very urgent
ajustes2-urgencia-ninguna = None: nothing gets this urgency
ajustes2-g-palabras = Magic words
ajustes2-ayuda-palabras = Copy one on its own and pin it: the tool opens. Separate them with commas.
ajustes2-palabras-ninguna = None
ajustes2-palabras-cambiar = Change
ajustes2-palabras-fabrica = Factory words
ajustes2-palabras-volver = Reset
ajustes2-palabras-marcador = word, another word…

## Local assistant (13-oct-2026)
asistente-titulo = Local assistant
asistente-mencionar = Mention: the assistant or where to draw
asistente-abrir-lienzo = Open canvas
asistente-abrir-fuente = Open cited source
asistente-explica = A small model that runs on this PC, offline: it answers, draws on the canvas, makes charts and reads the text of scanned PDFs and photos. Download it once; it shuts down by itself after 5 idle minutes and gives the memory back.
asistente-modelo-normal = Qwen3.5 2B (recommended)
asistente-modelo-ligero = Qwen3.5 0.8B (light)
asistente-detalle-normal = Draws and sees well · ~2.9 GB of memory while answering
asistente-detalle-ligero = For low-memory computers · draws and sees worse
asistente-bajar = Download
asistente-si-bajar = Yes, download
asistente-cancelar = Cancel
asistente-pregunta = Download now?
asistente-bajando = Downloading
asistente-comprobando = Checking
asistente-cancelada = Download cancelled.
asistente-poca-memoria = This computer has less than 8 GB of memory: the light one is better.
asistente-listo = The assistant is ready.
asistente-ocupado = The assistant is still answering: wait or press stop.
asistente-sin-modelo = First download the assistant in the «Asistente» chat.
asistente-fallo = The assistant could not start answering.
ajustes-asistente-modelo = Assistant model
ajustes2-ayuda-asistente-modelo = With the Local engine. The 2B draws and sees better; the 0.8B uses less.

## Cloud assistant (13-oct-2026)
ajustes2-sec-asistente = Assistant
ajustes2-sec-asistente-sub = Who answers in the «Asistente» chat: the model on this PC, Claude or DeepSeek.
ajustes2-g-asistente = Assistant engine
ajustes2-asistente-motor = Engine
ajustes2-ayuda-asistente-motor = Local: free and offline. Cloud: with your key.
ajustes2-asistente-local = Local (this PC)
ajustes2-asistente-respaldo = If the cloud fails, fall back to local
ajustes2-ayuda-asistente-respaldo = With no network, a bad key or no credit, the local model answers (if downloaded) and says so in the reply.
ajustes2-asistente-mejor-vista = Better vision for photos (Gemma 4 E2B)
ajustes2-ayuda-mejor-vista = Optional: to describe a photo or read what it says, use Gemma 4 E2B, which sees better. Downloaded separately ({ $tamano }) from the «Assistant» chat; uses ~3.7 GB of memory while looking.
ajustes2-ayuda-mejor-vista-lista = Downloaded. Used only to look at photos; everything else keeps the usual model, and never both at once.
asistente-vista-titulo = Better vision for photos
asistente-vista-explica = Gemma 4 E2B reads text in photos and understands scenes better than the usual model. It is used only when you ask it to look at a photo; everything else keeps the usual one, and both are never loaded at once.
asistente-modelo-vista = Gemma 4 E2B (better vision)
asistente-detalle-vista = Reads photos and scenes better · ~3.7 GB of memory while looking
asistente-ahora-no = Not now
asistente-vista-lista = Better vision for photos is ready.
asistente-confirmar-si = Yes
asistente-confirmar-no = No
asistente-otro = Other…
asistente-otro-escribe = Type your answer below and send it
asistente-recomendada = recommended
asistente-receta-guardar = Save as recipe
asistente-modelo = Model
asistente-nueva-conversacion = New conversation
asistente-nuevo-chat = New chat
asistente-borrar-chat = Clear chat
asistente-borrar-chat-aviso = Every message of the «Assistant» chat is deleted (attachments with them). Continue?
asistente-escuchar = Listen
asistente-modelo-cambiado = { $modelo } answers now.
asistente-receta-guardada = Recipe saved: { $nombre }
asistente-receta-llenas = There are too many recipes: delete one first.
asistente-receta-renombrar = Rename
asistente-receta-editar = Edit the request
asistente-receta-icono = Change the icon
asistente-receta-izquierda = Move left
asistente-receta-derecha = Move right
asistente-receta-borrar = Delete the recipe
asistente-receta-editando-nombre = Name of the recipe «{ $nombre }» · Enter saves · Esc cancels
asistente-receta-editando-pedido = Request of the recipe «{ $nombre }» · Enter saves · Esc cancels
ajustes2-asistente-docs-nube = Your documents and the cloud
ajustes2-ayuda-asistente-docs-nube = With Claude or DeepSeek, what the assistant finds in your PDFs, notes and chats goes over the internet to answer you. With the local model it never leaves the PC.
ajustes2-docs-nube-preguntar = Ask
ajustes2-docs-nube-si = Allow
ajustes2-docs-nube-no = Do not allow
ajustes2-g-claves = API keys
ajustes2-clave-de = { $proveedor } key
ajustes2-clave-marcador = Paste the key here
ajustes2-clave-probar = Save and test
ajustes2-clave-borrar-de = Delete the { $proveedor } key
ajustes2-ayuda-clave-borrar = It is deleted from this PC. The key stays valid in your account until you revoke it there.
ajustes2-clave-borrar = Delete
ajustes2-clave-guardada = Saved and encrypted on this PC:
ajustes2-clave-falta = No key. Get one at
ajustes2-clave-probando = testing…
ajustes2-g-conexiones = Assistant connections
ajustes2-conexion-cliente = { $servicio } client ID
ajustes2-conexion-cliente-marcador = Paste the client ID (or the JSON Google gives you)
ajustes2-conexion-guardar = Save
ajustes2-conexion-de = { $servicio }
ajustes2-conexion-conectar = Connect
ajustes2-conexion-desconectar = Disconnect
ajustes2-conexion-sin-cliente = No client ID. Create it once at { $consola } (steps: docs/conexiones-google-drive.md).
ajustes2-conexion-cliente-guardado = ID saved and encrypted on this PC:
ajustes2-conexion-conectado = Connected
ajustes2-conexion-sin-conectar = Not connected. With your permission, the assistant searches, reads, downloads and uploads files in your { $servicio }.
ajustes2-conexion-conectando = Finish in the browser…

## Assistant automations (Oct 13)
automatizacion-creada = Automation “{ $nombre }” created.
automatizacion-activada = Automation “{ $nombre }” turned on.
automatizacion-desactivada = Automation “{ $nombre }” turned off.
automatizacion-borrada = Automation “{ $nombre }” deleted.
automatizacion-en-marcha = Running “{ $nombre }”…
automatizacion-no-esta = There is no automation “{ $id }”.
automatizacion-rota = The automation could not be saved: { $motivo }
ajustes2-g-automatizaciones = Automations
ajustes2-automatizaciones-vacio = None yet
ajustes2-ayuda-automatizaciones-vacio = Ask the assistant: “every time I capture, put it in the site chat”, “every day at 8, summary of pending tasks”.
ajustes2-automatizacion-correr = Run now
ajustes2-automatizacion-otro-aparato = runs on { $aparato }
ajustes2-automatizacion-rota = Not understood: { $motivo }
ajustes2-automatizacion-borrar = Delete the automation “{ $nombre }”? This cannot be undone.
ajustes-asistente-pensar = Think before answering
ajustes2-ayuda-asistente-pensar = Before acting, the assistant thinks and writes what it understood; its thinking shows, folded, above its answer. Thinking takes time (minutes with the model on this PC): on automatic it only thinks on complex tasks, such as solving a problem, planning or several steps in a row, or when you ask («think it over», «step by step»).
asistente-pensar-auto = Automatic (only on complex tasks)
asistente-pensar-si = Always
asistente-pensar-no = Never
asistente-pensando = Thinking…
asistente-penso = Thought for { $s } s
asistente-penso-poco = Thought for a moment
asistente-reiniciado = Context renewed: continuing from the worksheet

## Assistant voice (13-oct-2026): talk to it and hear the answer
ajustes2-g-asistente-voz = Assistant voice
ajustes2-asistente-revisar-voz = Review what you said before sending
ajustes2-ayuda-asistente-revisar-voz = With the microphone of the «Asistente» chat: what you say is written in the box so you can fix it and send it yourself. Off, it is sent right away.
asistente-voz-pasando = Turning your voice into text… { $pct } %
asistente-voz-bajando = Downloading the speech recognizer (once, ~160 MB)… { $pct } %
asistente-voz-nada = Nothing was heard: speak closer to the microphone and try again.
asistente-voz-sin-whisper = This PC can't turn voice into text: Windows' ONNX Runtime is missing.
asistente-voz-fallo = Your voice couldn't be turned into text.
asistente-voz-cancelado = Cancelled: nothing was sent.
asistente-voz-nada-que-leer = This answer has no text to read.
asistente-voz-leer = Read aloud
asistente-voz-callar = Stop reading

# The buttons of the Windows notifications (15-oct).
notif-hecha = Done
notif-mas-10 = +10 min
notif-abrir = Open
notif-conservar = Keep
notif-ver-galeria = Open the gallery
notif-captura-se-va = This screenshot goes away { $cuando }
notif-capturas-se-van = { $n } screenshots go away in the next 24 h
notif-capturas-titulo = Screenshots expiring

# Geometry sheets (exact arc, angle mark, gradient)
herramientas-angulo = Angle mark
lienzo-pista-arco = Arc (on a guide: trace a reference oval; or by centre and two points, or by three points, in the panel)
lienzo-pista-angulo = Angle (W): click inside the angle, near where two lines meet; puts α, β, θ… without repeating (↑ ↓ or Alt+wheel change the letter; W again: the 90° one with its little square)
lienzo-panel-modo-angulo = Angle letter
lienzo-angulo-modo-griega = α β θ
lienzo-angulo-modo-simple = No letter
lienzo-angulo-modo-recto = 90° ∟
lienzo-panel-degradado = Gradient fill
lienzo-panel-cierre-arco = Arc closure
lienzo-panel-modo-arco = Draw the arc
lienzo-panel-arcos-angulo = Arcs
lienzo-degradado-liso = Flat
lienzo-degradado-lineal = Linear
lienzo-degradado-radial = Radial
lienzo-cierre-abierto = Open
lienzo-cierre-sector = Sector
lienzo-cierre-segmento = Segment
lienzo-arco-modo-guia = Guide
lienzo-arco-modo-centro = Centre
lienzo-arco-modo-tres = 3 points

## Tasks: single-task pin and class timetable (17-oct-2026)
tareas-fijar-pin = 📌 Pin as reminder
pin-tarea-nota = 💬 Why not now?
pin-tarea-pista = Why not now · Enter saves
# Tasks: urgency and day/time picker (18-oct-2026)
tareas-urgente = Urgent
tareas-muy-urgente = Very urgent
tareas-selector-pista = Pick the reminder day and time
tareas-selector-sin = No reminder
tareas-selector-quitar = Remove
tareas-selector-hoy = Today
tareas-selector-manana = Tomorrow
tareas-dia-0 = Mon
tareas-dia-1 = Tue
tareas-dia-2 = Wed
tareas-dia-3 = Thu
tareas-dia-4 = Fri
tareas-dia-5 = Sat
tareas-dia-6 = Sun
bandeja-horario = 🎓 Class timetable…
bandeja-horario-grabar = 🎙 Voice note for the next class
horario-sin-clases = There are no classes in the timetable yet: open “Class timetable…” and write one per line.
horario-guardada = You will hear it at the next { $clase } class

## Weekly timetable window and subjects (18-oct-2026)
horario-titulo = Timetable
horario-subtitulo = { $asignaturas } subjects · { $clases } classes
horario-nueva = New subject (@room optional)
horario-domingo = Sunday
horario-como-texto = As text
horario-vacio = Type a subject above and drag it onto the week.
horario-tareas-de = { $asignatura } tasks
horario-sin-tareas = No pending tasks. In Tasks, pick the subject when adding one.
horario-abrir-tareas = Open in Tasks
horario-aula = Room
horario-nota = Voice note (what the call reads)
horario-avisar = Remind (minutes before)
horario-avisar-pista = { $min } (the timetable's)
horario-guardar = Save
horario-cancelar = Cancel
horario-borrar = Delete
horario-deshacer = Undo
horario-borrada = Class deleted
horario-asignatura-borrada = “{ $asignatura }” removed from the timetable (its tasks stay in Tasks)
horario-repetida = “{ $asignatura }” is already in the list
horario-arrastra = Drag “{ $asignatura }” onto the week
horario-no-guardado = The timetable could not be saved
tareas-asignatura-pista = Task subject
tareas-sin-asignatura = No subject (to the Inbox)
tareas-sin-asignaturas = No subjects yet: create them in “Class timetable…”
tareas-apuntar-en-asignatura = New { $asignatura } task…

# Viewers: the frame to print (2026-10-19)
marco-imprimir-pista = Drag a box over what you want to print · Enter: everything · Esc: leave
lector-marco = Frame to print (P)
marcos-pista = Drag to frame · a number: go to that frame · Ctrl+P: print all · Del: remove the last one · I or Esc: leave
marcos-imprimir = Print the frames (Ctrl+P)
marcos-borrar = Clear frames
riel-marcador = Bookmark (Ctrl+M)
riel-anotar = Annotate (A)
riel-marcos = Frames to print (I)
barra-mover = Move (drag)
barra-oscuro = Night mode
barra-pantalla = Full screen (F11)
barra-fijar = Always on top
barra-cerrar = Close (Esc)
visor-letra = Font and spacing (T / B)
lector-mas = More options (G)
visor-indice-pista = Contents (C)
riel-hojita = Notepad (Ctrl+Shift+N)
riel-editar = Edit (Alt+R)
riel-mirar = Back to viewing (Alt+R or Esc)

# Pin rail: the frame button of a pin that is not a photo (19-oct-2026).
riel-imprimir-zona = Print an area (Ctrl+P)

# The spreadsheet viewer bar: name and key of each button (19-oct-2026).
hoja-pista-editar = Edit cells (F2)
hoja-pista-ver = Back to viewing (Esc)
hoja-pista-negrita = Bold (Ctrl+B)
hoja-pista-cursiva = Italic (Ctrl+I)
hoja-pista-subrayado = Underline (Ctrl+U)
hoja-pista-tachado = Strikethrough (Ctrl+5)
hoja-pista-resaltar = Fill color
hoja-pista-letra = Font color
hoja-pista-alinear = Align (Ctrl+Shift+L / E / R)
hoja-pista-numero = Number format
hoja-pista-ajustar = Wrap text
hoja-pista-borrar-formato = Clear formatting
## Tooltips of the web viewers' and Office pin bars (19-oct)
web-recargar = Reload (F5)
web-exportar = Export (Ctrl+E)
web-navegador = Open in the browser
nota-pista-ver = Read view
nota-pista-editar = Edit
nota-pista-buscar = Find (Ctrl+F)
nota-pista-comentarios = Comments
nota-pista-compartir = Share
office-pista-abrir-con = Open with…
office-pista-presentar = Present

## Page folders in the chat (26-oct-2026): a named notebook
carpeta-nueva = Page folder
carpeta-nombre-de-serie = Notebook
carpeta-pagina = Page
carpeta-paginas = { $n ->
    [one] 1 page
   *[other] { $n } pages
}
carpeta-vacia-detalle = Empty
carpeta-anadir-a = Add to folder…
carpeta-nueva-con = New folder with this page
carpeta-anadida = Added to “{ $carpeta }” as “{ $titulo }”
carpeta-ya-estaba = Already in “{ $carpeta }”
carpeta-abrir = Open folder
carpeta-nueva-pagina = New page
carpeta-exportar-pdf = Export PDF
carpeta-exportando = Exporting the PDF…
carpeta-exportado = PDF saved: { $nombre }
carpeta-no-se-exporto = Could not export: no page has anything on it
carpeta-vacia = This folder is empty. Press “New page”, or in the chat choose “Add to folder…” in the menu of a canvas or an image.
carpeta-antes = Earlier
carpeta-despues = Later
carpeta-renombrar = Rename
carpeta-quitar = Remove from folder
carpeta-no-esta = That folder is no longer in the chat
carpeta-pista = Drag a page to move it · F2 changes its title · Del removes it (the canvas stays in the chat)
carpeta-escribe-titulo = Type the title and press Enter
