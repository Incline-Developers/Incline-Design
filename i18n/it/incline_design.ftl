# Incline — catalogo dei messaggi in italiano.
#
# Questo file può essere incompleto: le voci mancanti ricadono sul catalogo
# inglese canonico (`i18n/en/incline_design.ftl`).
#
# Non modificare gli id a sinistra di ogni `=` né i nomi delle variabili
# `{ $... }`: sono controllati dal codice a compile time e un id sconosciuto
# o un argomento mancante blocca la build.

## Shared

common-cancel = Annulla
common-clear = Cancella
common-close = Chiudi
common-fill = Riempimento
common-set = Imposta

## Status bar

# Titolo del menu della lingua nella barra di stato. Le lingue stesse non
# vengono mai tradotte: ognuna appare nel proprio nome, in `LanguageChoice`.
status-language = Lingua

## Menu bar — File

menu-file = File
menu-file-save-project = Salva progetto
menu-file-save-project-as = Salva progetto con nome...
menu-file-new-project = Nuovo progetto...
menu-file-open-project = Apri progetto...
menu-file-open-recent = Apri recente
menu-file-show-in-explorer = Mostra in Esplora file
menu-file-show-in-folder = Apri cartella contenitore
menu-file-import = Importa...
menu-file-export = Esporta...
menu-file-export-viewport-image = Esporta immagine della vista...
menu-file-export-engineering-drawing = Esporta disegno tecnico...
menu-file-about = Informazioni su { $app }...
menu-file-exit = Esci dall'applicazione

## Menu bar — View

menu-view = Vista

## Workspaces

ws-production = Produzione
ws-drill-and-blast = Perforazione e volata
ws-geology = Geologia
ws-planning = Pianificazione

## Menubars

ws-menubar-design = Progettazione
ws-menubar-triangulation = Triangolazione
ws-menubar-raster = Raster
ws-menubar-point-cloud = Nuvola di punti
ws-menubar-block-model = Modello a blocchi
ws-menubar-drillholes = Fori di sondaggio
ws-menubar-active-layer = Livello:

## Menubars functions

ws-menubar-design-insert-point = Inserisci punto
ws-menubar-design-insert-point-at-intersection = All'intersezione
ws-menubar-design-insert-point-at-elevation = A quota
ws-menubar-design-move-to = Sposta a
ws-menubar-design-create-triangulation = Crea triangolazione

## Rename / delete item dialogs

# { $kind } è un sostantivo dello spazio di lavoro dell'insieme ws-production-* sopra.
dialog-rename-title = Rinomina { $kind }
dialog-rename-field = Nuovo nome
dialog-rename-field-hint = Obbligatorio
dialog-rename-submit = Rinomina
dialog-delete-title = Elimina { $kind }
dialog-delete-confirm =
    Eliminare '{ $name }' dal progetto?
    Questa azione non può essere annullata.
confirm-delete-product =
    Eliminare il prodotto '{ $name }' dalla tavolozza?
    Questa azione non può essere annullata.

## Create Triangulation dialog

tri-create-title = Crea triangolazione
tri-create-help = Triangola gli oggetti selezionati all'apertura di questa finestra. Chiudila per cambiare la selezione.
tri-create-type-label = Tipo di triangolazione
tri-create-type-help =
    La superficie aperta crea una maglia in stile terreno. Il solido crea una
    mesh completamente chiusa e richiede un input in grado di formare un contorno stagno.
tri-create-output-name = Nome di output
tri-create-output-name-help = Nome assegnato alla triangolazione generata.
tri-create-output-name-hint = nome della triangolazione
tri-create-run = Triangola
tri-selection-none = Gli oggetti selezionati non sono più disponibili.

tri-selection-selected = { $summary } selezionati

tri-type-open-surface = Superficie
tri-type-solid-closed = Solido

# Elementi del riepilogo della selezione, ad es. "3 polilinee, 1 punto". Ogni
# sostantivo viene messo al plurale in base al proprio conteggio, così le
# lingue con più di due forme plurali restano corrette.
tri-count-polylines =
    { $count ->
        [one] { $count } polilinea
       *[other] { $count } polilinee
    }
tri-count-strings =
    { $count ->
        [one] { $count } linea
       *[other] { $count } linee
    }
tri-count-circles =
    { $count ->
        [one] { $count } cerchio
       *[other] { $count } cerchi
    }
tri-count-points =
    { $count ->
        [one] { $count } punto
       *[other] { $count } punti
    }
tri-count-texts =
    { $count ->
        [one] { $count } oggetto testo
       *[other] { $count } oggetti testo
    }
tri-count-objects =
    { $count ->
        [one] { $count } oggetto
       *[other] { $count } oggetti
    }

about-read-full-licence = Leggi la licenza completa ↗
about-source-code = Codice sorgente
about-website = Sito web
about-title = Informazioni su { $app }
drill-hole-colour-stop = Stop { $index }
properties-restore-defaults-tooltip = Ripristina le impostazioni di { $heading } ai valori predefiniti

## Dynamic UI messages

ui-selected-count = { $count } selezionati
ui-selected-objects = { $count } oggetto/i selezionato/i
ui-selected-polylines = { $count } polilinea/e selezionata/e
ui-invalid-axis-value = Inserisci un valore { $axis } valido.
ui-selection-spans = La selezione va da { $min } a { $max }.
confirm-delete-count = Eliminare { $count } elemento/i selezionato/i?
confirm-delete-layer = Eliminare il livello '{ $name }' e tutti gli oggetti che contiene?
    Questa azione non può essere annullata.
plot-preview-pixels = { $width } × { $height } px a { $dpi } dpi
tri-estimated-memory = Memoria di picco stimata ~{ $estimate }. { $detail }
block-grid-summary = Griglia: { $x } × { $y } × { $z } = { $count } blocchi
status-selected = Selezionati: { $count }
status-faces = Facce: { $drawn } / { $total } ({ $drawn_chunks }/{ $total_chunks } chunk)
status-clip = Clip vicino/lontano/Δ: { $near } / { $far } / { $delta } m
status-points = Punti: { $drawn } / { $target } di { $total } ({ $drawn_chunks }/{ $total_chunks } chunk)

explorer-no-rasters = Nessun raster
slice-viewport-gestures = trascina con tasto centrale: pan · trascina con tasto destro: orbita · Maiusc+rotella: cammina · W/S: sposta lastra · Q/E: ruota · Esc: esci

## Startup environment details

## Renderer startup diagnostics

color-aci = ACI
color-aci-value = ACI { $index }
color-index = Indice
color-rgb = RGB
color-opacity = Opacità
color-edit = Fai clic per modificare il colore
color-saturation-value = Saturazione e luminosità
color-hue = Tonalità
asset-loading = Caricamento dati risorsa
asset-unloading = Scaricamento dati risorsa
asset-load-failed = Impossibile caricare i dati della risorsa
asset-unload-failed = Impossibile scaricare i dati della risorsa
preferences-title = Preferenze
context-text-colour = Colore del testo
context-polylines = Polilinee
context-points = Punti
crs-unknown-ellipsoid = Modello terrestre non riconosciuto "{ $name }" in questa definizione di sistema di coordinate.
crs-no-ellipsoid = Questa definizione di sistema di coordinate non indica quale modello terrestre utilizza.
crs-unknown-code = EPSG:{ $code } non è presente nel registro dei sistemi di coordinate.
crs-transform-failed = Non è stato possibile convertire una coordinata; il risultato non era una posizione finita.
crs-no-datum-path = Non è disponibile alcuna trasformazione pubblicata tra i sistemi di riferimento di { $from } e { $to } (datum EPSG { $source } e { $target }). Convertire comunque sarebbe errato di una quantità sconosciuta, quindi non è stato cambiato nulla.
crs-unknown-datum = Il sistema di riferimento di { $from } o { $to } non può essere identificato, ed entrambi usano modelli terrestri diversi. La conversione tra loro sarebbe errata di una quantità sconosciuta.
ws-survey = Rilievo
survey-count-designs = { $count } { $count ->
    [one] progetto
   *[other] progetti
  }
survey-count-meshes = { $count } { $count ->
    [one] triangolazione
   *[other] triangolazioni
  }
survey-count-models = { $count } { $count ->
    [one] modello a blocchi
   *[other] modelli a blocchi
  }
survey-count-clouds = { $count } { $count ->
    [one] nuvola di punti
   *[other] nuvole di punti
  }
survey-count-holes = { $count } { $count ->
    [one] set di fori di sondaggio
   *[other] set di fori di sondaggio
  }
survey-count-rasters = { $count } { $count ->
    [one] raster
   *[other] raster
  }
survey-angle = Rotazione attorno a Z (antioraria)
survey-scale = Fattore di scala uniforme XYZ
survey-invalid-transform = Le origini, l'angolo e le coordinate risultanti devono essere finiti.
survey-invalid-scale = La scala deve essere un numero positivo finito con reciproco finito.
survey-empty-selection = Seleziona almeno un elemento supportato da trasformare.
survey-unavailable = Un elemento selezionato è mancante o non caricato. Caricalo prima di trasformare.
survey-wrong-project = Seleziona progetti solo dal progetto attivo.
survey-name-required = Inserisci un nome per il sistema di coordinate.
survey-working = Trasformazione dei dati selezionati…
survey-completed = Convertiti { $items } sul posto. L'annullamento li ripristina.
survey-failed = Trasformazione non riuscita: { $error }
survey-stale = Trasformazione scartata perché il progetto attivo o i dati di origine sono cambiati. Seleziona i dati di origine e riprova.
survey-coordinates-menu = Coordinate
survey-definitions-action = Definizioni…
survey-transform-action = Trasforma…
survey-definitions-title = Definizioni di coordinate
survey-transform-title = Trasforma coordinate
survey-new-system = Nuovo sistema di coordinate
survey-new-system-name = Sistema di coordinate
survey-set-local = Imposta come sistema di coordinate della miniera
survey-delete-system = Elimina sistema di coordinate
survey-systems-empty = Nessun sistema di coordinate
survey-system-name = Nome
survey-system-origin = Stesso punto — coordinate del sistema
survey-angle-help = Antiorario dalla X di riferimento verso la Y di riferimento, visto dall'alto.
survey-scale-help = Scala XYZ uniforme dal sistema di riferimento a questo sistema. Usa 1 per preservare le dimensioni.
survey-close = Chiudi
survey-from = Da
survey-to = A
survey-transform-button = Trasforma
survey-swap = Scambia
survey-drape-note = Le immagini drappeggiate vengono rimosse dalle superfici convertite e devono essere ridrappeggiate.
survey-needs-grid-block-model = Un modello a blocchi è una griglia regolare di celle, e un cambio di proiezione o sistema di riferimento non ne preserva la regolarità. Convertirlo significherebbe ricampionare ogni cella in una nuova griglia perdendo i valori che contiene, quindi è stato lasciato invariato.
survey-needs-grid-raster = Un raster è posizionato nel mondo tramite una mappa affine, cosa che un cambio di proiezione o sistema di riferimento non può preservare. Convertirlo significherebbe ricampionare l'immagine, quindi è stato lasciato invariato.
survey-conversion-exact = Esatta: solo cambio di griglia, senza riproiezione.
survey-conversion-accuracy = Precisione dichiarata { $accuracy } m.
survey-kind = Tipo
survey-axis-names = Nomi degli assi
survey-kind-registry-short = Sistema del registro
survey-kind-grid-short = Griglia su un altro sistema
survey-registry-search = Cerca
survey-registry-hint = Nome o codice EPSG, es. "mga zone 56"
survey-registry-none = Nulla nel registro corrisponde a tutte le parole.
survey-parent = Definito rispetto a
survey-parent-origin = Punto noto — coordinate del sistema padre
survey-pick-registry = Cerca il sistema e scegli tra i risultati.
survey-pick-parent = Scegli il sistema rispetto al quale è definita questa griglia.
survey-pick-system = Scegli un sistema
survey-pick-systems = Scegli il sistema da cui convertire e quello a cui convertire.
survey-no-selection = Scegli un sistema di coordinate a sinistra, o fai clic destro per aggiungerne uno.
survey-kind-grid = Griglia su { $parent }
survey-system-in-use = "{ $name }" non può essere eliminato: { $dependants } { $dependants ->
    [one] è
   *[other] sono
  } definiti rispetto ad esso. Reindirizzali altrove prima.
survey-system-cycle = "{ $name }" è definito rispetto a se stesso, direttamente o tramite i suoi sistemi padre.
survey-system-missing = Quel sistema di coordinate non esiste più. Seleziona un'altra definizione.
survey-same-system = Scegli sistemi di origine e destinazione diversi.
survey-name-exists = Esiste già un sistema di coordinate con questo nome. Selezionalo per modificarlo, oppure scegli un altro nome.
preferences-ui-size = Dimensione dell'interfaccia
preferences-ui-size-help = Regola testo e controlli rispetto alla scala di visualizzazione normale del dispositivo. 100% usa la dimensione predefinita. La risoluzione dello schermo e la dimensione della finestra non riducono l'interfaccia.
relimit-select-boundary = Seleziona la polilinea o il cerchio a cui ridelimitare
relimit-click-boundary = Fai clic sulla polilinea o sul cerchio da intersecare…
relimit-mode-help = Intersezione sposta un'estremità su una polilinea o un cerchio. Assoluta imposta la lunghezza finale della linea. Relativa aggiunge o sottrae lunghezza.
browser-graphics-device-lost = Il browser ha perso il dispositivo grafico. Riapri questa pagina in una nuova scheda. Dettagli GPU: { $message }

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins e i contributori di Incline Design. Con la presente si concede, gratuitamente, a chiunque ottenga una copia di questo software, il permesso di utilizzarlo senza restrizioni, alle condizioni della Licenza MIT.

    Incline Design è fornito "COSÌ COM'È", SENZA GARANZIA DI ALCUN TIPO, ESPRESSA O IMPLICITA, incluse a titolo esemplificativo le garanzie di COMMERCIABILITÀ, IDONEITÀ A UNO SCOPO PARTICOLARE e NON VIOLAZIONE.
about-free-open-source-mine-design = Progettazione mineraria libera e open source
about-licensed-under-mit-license = Distribuito sotto Licenza MIT

## App strings

app-activated-browser-project-name = Attivato il progetto del browser '{ $name }'.
app-browser-project-delete-failed = Eliminazione del progetto del browser non riuscita: { $error }
app-browser-project-no-longer-exists = Quel progetto del browser non esiste più
app-browser-save-failed-error = Salvataggio nel browser non riuscito: { $error }
app-could-not-activate-browser-project = Impossibile attivare il progetto del browser: { $error }
app-could-not-delete-browser-project = Impossibile eliminare il progetto del browser: { $error }
app-could-not-load-browser-project = Impossibile caricare il progetto del browser: { $error }
app-could-not-restore-browser-project = Impossibile ripristinare il progetto del browser: { $error }
app-deleted-browser-project = Progetto del browser eliminato
app-failed-create-window-error = Creazione della finestra non riuscita: { $error }
app-failed-create-window-icon-error = Creazione dell'icona della finestra non riuscita: { $error }
app-failed-detach-top-down-preview = Distacco dell'anteprima dall'alto non riuscito: { $error }
app-failed-initialize-graphics-error = Inizializzazione della grafica non riuscita: { $error }
app-browser-preferences-load-failed = Caricamento delle preferenze del browser non riuscito: { $error }
app-failed-load-config-file-error = Caricamento del file di configurazione non riuscito: { $error }
app-failed-load-session-file-error = Caricamento del file di sessione non riuscito: { $error }
app-failed-rasterize-window-icon-error = Rasterizzazione dell'icona della finestra non riuscita: { $error }
app-failed-save-browser-session-error = Salvataggio della sessione del browser non riuscito: { $error }
app-failed-save-session-error = Salvataggio della sessione non riuscito: { $error }
app-saved-name-browser-storage = Salvato '{ $name }' nella memoria del browser

## Block strings

block-model-between = Tra
block-model-block-grid = Griglia di blocchi
block-model-block-size = Dimensione blocco
block-model-choose-numeric-variable = Scegli una variabile numerica
block-model-choose-numeric-variables = Scegli variabili numeriche
block-model-count-variables-selected = { $count } variabili selezionate
block-model-estimate-variables = Stima variabili
block-model-full-x-y-z-dimensions = Dimensioni complete X, Y e Z di ciascun blocco. Blocchi più piccoli aumentano dettaglio, tempo di calcolo e uso di memoria.
block-model-grid-bounds-block-sizes-invalid = I limiti della griglia o le dimensioni dei blocchi non sono validi.
block-model-lower-x-y-z-edges = Bordi inferiori X, Y e Z del volume del modello a blocchi. I centri dei blocchi iniziano a metà blocco all'interno di questi limiti.
block-model-maximum = Massimo
block-model-maximum-nearest-samples-used-each = Numero massimo di campioni più vicini usati per ciascun blocco. Valori più bassi sono più veloci; valori più alti possono smussare le stime e aumentare il tempo di calcolo.
block-model-maximum-samples = Campioni massimi
block-model-minimum = Minimo
block-model-min-samples-help = Numero minimo di campioni vicini richiesti per stimare un blocco. I blocchi con meno campioni entro il raggio di ricerca vengono lasciati vuoti.
block-model-minimum-samples = Campioni minimi
block-model-no-block-model-selected = Nessun modello a blocchi selezionato
block-model-no-drill-holes-selected = Nessun foro di sondaggio selezionato
block-model-nugget = Nugget
block-model-numeric-interval-fields-interpolate = Campi di intervallo numerici da interpolare. Ogni campo selezionato diventa una variabile del modello a blocchi.
block-model-kriging-help = Il Kriging ordinario stima gli intervalli numerici dei fori di sondaggio in ogni centro di blocco usando un variogramma sferico.
block-model-partial-sill = Sill parziale
block-model-range-search-radius = Portata / raggio di ricerca
block-model-range-help = I campioni più distanti di questa distanza sono esclusi; la covarianza raggiunge zero a questa portata.
block-model-select-all = Seleziona tutto
block-model-selected-block-model-whose-blocks = Il modello a blocchi selezionato, i cui blocchi vengono trasformati in solido tramite soglia. Chiudi la finestra per applicare la soglia a un altro modello.
block-model-source-drill-holes-help = La raccolta di fori di sondaggio selezionata, i cui intervalli numerici vengono stimati nei blocchi. Chiudi la finestra per stimare da un'altra raccolta.
block-model-sill-help = Varianza spazialmente correlata fornita dal modello sferico. Insieme al nugget, imposta la covarianza a distanza zero.
block-model-spherical-variogram-search = Variogramma sferico e ricerca
block-model-threshold-at-most = <= soglia
block-model-threshold-at-least = >= soglia
block-model-threshold-min = Soglia / min
block-model-upper-x-y-z-extent = Estensione superiore X, Y e Z da coprire. L'ultimo blocco può estendersi oltre questa estensione quando lo spazio non è un multiplo esatto della dimensione del blocco.
block-model-variable = Variabile
block-model-variance-effectively-zero-separation = Varianza a separazione praticamente nulla causata da errore di misura o variazione al di sotto della scala di campionamento. Usa zero se non si intende alcun effetto nugget.
block-model-volume-feedback-disconnected = La lettura del feedback di utilizzo del volume di blocchi si è disconnessa
block-model-volume-feedback-failed = Lettura del feedback di utilizzo del volume di blocchi non riuscita: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z
borehole-inspector-checking-linked-geophysics-file = Verifica del file di geofisica collegato...
borehole-inspector-close-inspector = Chiudi l'ispettore
borehole-inspector-data = Dati
borehole-inspector-display = Visualizzazione
borehole-inspector-file-not-where-was-linked = { $file } non si trova nella posizione da cui è stato collegato.
borehole-inspector-guessed-name = Dedotto dal nome
borehole-inspector-hold-hole-while-you-work = Mantieni questo foro mentre lavori su quelli intorno.
borehole-inspector-holding-hole-click-follow-selection = Foro mantenuto. Fai clic per seguire di nuovo la selezione.
borehole-inspector-log = Log
borehole-inspector-no-hole-inspected = Nessun foro ispezionato
borehole-inspector-pick-file = Scegli { $file }...
borehole-inspector-pick-file-again-show-its = Scegli di nuovo { $file } per mostrarne la geofisica: una pagina del browser non può riaprire un file da sola.
borehole-inspector-reading-geophysics-file-its-index = Lettura del file di geofisica per il suo indice; la barra di stato mostra l'avanzamento.
borehole-inspector-strat = Strat
borehole-inspector-summary = Riepilogo
canvas-circle-summary = Cerchio | Livello: { $layer } | raggio { $radius }

## Canvas strings

canvas-not-selectable-closed-polyline = Non selezionabile | Scegli una polilinea chiusa
canvas-polyline-summary = Polilinea | Livello: { $layer } | { $count } vertici
canvas-surface-name = Superficie | { $name }
canvas-trimmed = Rifilata
cinematic-shadows-method = Ombre della vista cinematica: { $method }

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = Creata scarpa e berma dall'oggetto { $object_id }
cmd-bezier-replaced-polyline-span-first-last = Sostituito il tratto di polilinea { $first }→{ $last } con { $count } punti intermedi campionati
cmd-bezier-vertices-first-last = Vertici da { $first } a { $last }
cmd-block-model-block-model-loader-disconnected-path = Il caricatore del modello a blocchi si è disconnesso per { $path }
cmd-block-model-block-model-path-has-count = Il modello a blocchi { $path } ha { $count } variabile/i di un tipo non supportato che non sarà/anno leggibile/i: { $names }
cmd-block-model-building-ore-mesh = Costruzione della mesh del minerale…
cmd-block-model-could-not-create-block-model = Impossibile creare il modello a blocchi: { $error }
cmd-block-model-could-not-decode-block-model = Impossibile decodificare la variabile colore del modello a blocchi '{ $variable }': { $error }
cmd-block-model-created-block-model-name-ordinary = Creato modello a blocchi '{ $name }' tramite Kriging ordinario
cmd-block-model-failed-load-block-model-error = Caricamento del modello a blocchi non riuscito: { $error }
cmd-block-model-generated-ore-mesh-from-block = Generata mesh del minerale dal modello a blocchi '{ $name }'
cmd-block-model-imported-block-model-source-path = Sorgente del modello a blocchi importato { $path }
cmd-block-model-loaded-block-model-name-blocks = Caricato il modello a blocchi '{ $name }': { $blocks } blocchi ({ $renderable } renderizzabili), griglia { $dimx }x{ $dimy }x{ $dimz }, { $variables } variabili
cmd-block-model-loading-name = Caricamento di { $name }
cmd-block-model-loading-name-ellipsis = Caricamento di { $name }…
cmd-chamfer-applied = Smussato l'angolo { $corner } con raggio { $radius } e { $segments } segmenti
cmd-chamfer-radius = Raggio { $radius }
cmd-commands-clipped = Ritagliata
cmd-commands-command-failed-error = Comando non riuscito: { $error }
cmd-commands-count-control-string-s = { $count } linea/e di controllo
cmd-commands-count-control-string-s-layer = { $count } linea/e di controllo su '{ $layer }'
cmd-commands-count-point-s-across-layers = { $count } punto/i su { $layers } livelli
cmd-commands-count-point-s-layer = { $count } punto/i su '{ $layer }'
cmd-commands-kind-layer = { $kind } su '{ $layer }'
cmd-commands-no-control-strings = Nessuna linea di controllo
cmd-commands-no-extent = Nessuna estensione
cmd-commands-select-holes-place-reference-points = Seleziona i fori su cui posizionare i punti di riferimento
cmd-triangulate-needs-selection = Seleziona gli oggetti da triangolare prima di eseguire Crea triangolazione
cmd-commands-select-one-loaded-block-model = Seleziona un modello a blocchi caricato prima di creare da esso una triangolazione del minerale
cmd-commands-select-one-loaded-drill-hole = Seleziona una raccolta di fori di sondaggio caricata prima di creare da essa un modello a blocchi
cmd-commands-select-one-loaded-point-cloud = Seleziona una nuvola di punti caricata prima di creare da essa una triangolazione
cmd-contours-needs-triangulation = Seleziona una triangolazione caricata prima di generarne le curve di livello
cmd-slice-needs-triangulation = Seleziona una triangolazione caricata prima di sezionarla per intervallo Z
cmd-commands-select-one-loaded-triangulation-one = Seleziona una triangolazione caricata e una polilinea chiusa prima del ritaglio
cmd-commands-select-one-more-objects-before = Seleziona uno o più oggetti prima di impostare { $axis }
cmd-commands-sliced = Sezionata
cmd-contours-contour-generation-failed-error = Generazione delle curve di livello non riuscita: { $error }
cmd-contours-discarded-layer-exists = Le curve di livello per '{ $name }' sono state scartate: il livello '{ $layer_name }' esiste già
cmd-contours-discarded-project-closed = Le curve di livello per '{ $name }' sono state scartate: il progetto è stato chiuso
cmd-contours-discarded-layer-deleted = Le curve di livello per '{ $name }' sono state scartate: il livello di output selezionato è stato eliminato
cmd-contours-generated = Generata/e { $line_count } polilinea/e di curva di livello per la triangolazione '{ $name }' nel livello '{ $layer_name }'
cmd-creation-assembled-boundary-rings = Assemblato/i { $assembled_count } anello/i di confine chiuso/i da linee aperte frammentate
cmd-creation-created-triangulation-from-boundary = Creata triangolazione da { $boundary_count } anello/i di confine e { $constraint_count } vincolo/i aperto/i, tipo di superficie { $surface_type }
cmd-creation-creating-triangulation = Creazione della triangolazione…
cmd-creation-generate-upper-surface-ignored-count = Genera superficie superiore: ignorato/i { $count } segmento/i di linea di rottura in conflitto più basso/i; gli oggetti sorgente non sono stati modificati
cmd-creation-ignored-objects = Ignorato/i { $rejected } oggetto/i non polilinea o degenere/i durante la triangolazione
cmd-creation-weld-retry-moved-coarse-welded = Salda e riprova: spostato/i { $coarse_welded } vertice/i su posizioni condivise (fino a { $coarse_weld_tol } m); gli oggetti sorgente non sono stati modificati
cmd-creation-welded-breakline-vertices = Saldato/i { $welded } vertice/i di linea di rottura coincidenti entro la tolleranza
cmd-cuts-clipped-surface-name-polyline-mode = Ritagliata la superficie '{ $name }' con polilinea ({ $mode })
cmd-cuts-clipping-surface-polyline = Ritaglio della superficie con polilinea…
cmd-cuts-cut-topology-name-pit-shell = Tagliata la topologia '{ $name }' sul guscio della fossa
cmd-cuts-cut-triangulation-name-z-band = Tagliata la triangolazione '{ $name }' per fascia Z [{ $min }, { $max }]
cmd-cuts-cutting-topology-pit-shell = Taglio della topologia con guscio della fossa…
cmd-cuts-cutting-triangulation-z = Taglio della triangolazione per Z…
cmd-cuts-ignored-vertical-faces = Ignorata/e { $count } faccia/e verticale/i o degenere/i della topologia di riferimento priva/e di area XY
cmd-cuts-site-skipped-constraint-from-x = { $site }: vincolo saltato ({ $from_x }, { $from_y }) -> ({ $to_x }, { $to_y }) il triangolatore non è riuscito a suddividerlo
cmd-cuts-skipped-degenerate-edges = { $site }: saltato/i { $skipped } lato/i di vincolo quasi degenere/i; il contorno di taglio potrebbe essere impreciso di un pelo in prossimità
cmd-cuts-trimmed-surface = Rifilata la superficie '{ $surface }' sulla topologia '{ $topology }' ({ $mode })
cmd-cuts-trimming-surface-topology = Rifilatura della superficie sulla topologia…
cmd-drape-draped-intersected-vertices-changed = Adagiati { $intersected } vertici; { $changed } con quota modificata
cmd-drape-no-intersections = Nessuno dei vertici di progettazione selezionati interseca le topologie selezionate
cmd-drape-objects-changed-object-s-changed = { $objects } oggetto/i modificato/i · { $changed } di { $intersected } vertici intersecanti spostati
cmd-drape-select-one-more-design-objects = Seleziona uno o più oggetti di progettazione da adagiare
cmd-drape-select-one-more-topologies-drape = Seleziona una o più topologie su cui adagiare
cmd-drape-selected-topologies-no-longer-loaded = Le topologie selezionate non sono più caricate
cmd-drill-hole-choose-drillhole-source-files-again = Scegli di nuovo i file sorgente dei fori di sondaggio
cmd-drill-hole-drill-pattern-too-large-contains = Lo schema di perforazione è troppo grande o contiene coordinate di bocca foro non valide
cmd-drill-hole-drillhole-field-label-has-count = Il campo dei fori di sondaggio '{ $label }' ha { $count } codici distinti, più di quanti ne avrebbe tipicamente un campo codificato; sembra testo libero e non un campo categorico, ma tutti i codici vengono mantenuti e colorati
cmd-drill-hole-enter-name-drill-pattern = Inserisci un nome per lo schema di perforazione
cmd-drill-hole-failed-load-drillholes-error = Caricamento dei fori di sondaggio non riuscito: { $error }
cmd-drill-hole-depth-must-be-positive = La profondità del foro deve essere maggiore di zero
cmd-drill-hole-diameter-must-be-positive = Il diametro del foro deve essere maggiore di zero
cmd-drill-hole-loaded-drillhole-dataset-name-holes = Caricato il set di fori di sondaggio '{ $name }': { $holes } fori, { $fields } campi colore
cmd-drill-hole-name-already-loading = '{ $name }' è già in caricamento
cmd-drill-hole-name-reason = '{ $name }': { $reason }
cmd-drill-hole-name-side = { $name } { $side }
cmd-drill-hole-name-working-section-side = { $name } sezione di lavoro { $side }
cmd-drill-hole-no-hole-holds-value-field = Nessun foro contiene '{ $value }' in quel campo
cmd-drill-hole-only-mapped-csv-bundles-imported = Nel browser vengono importati solo i pacchetti CSV mappati
cmd-drill-hole-pattern-contains-no-holes = Lo schema non contiene fori
cmd-drill-hole-reading-name = Lettura di { $name }
cmd-drill-hole-reference-points-used-holes-placed = Punti di riferimento: { $used } fori posizionati, { $absent } senza '{ $value }', { $flagged } segnalati come possibili ripetizioni da faglia
cmd-drill-hole-uppermost-run-used-flagged-holes = Usato il tratto più alto, segnalati: { $holes }
cmd-drill-hole-working-section-name-not-same = La sezione di lavoro '{ $name }' non è la stessa in ogni dataset selezionato; è stata usata quella propria di ciascun dataset.
cmd-drill-hole-working-sections-not-kept-dataset = Sezioni di lavoro non mantenute in '{ $dataset }'. { $reasons }
cmd-explode-count-line-s = { $count } linea/e
cmd-explode-polyline = Esplodi polilinea
cmd-explode-exploded-polyline-into-count-line = Esplosa la polilinea in { $count } segmenti di linea
cmd-file-block-model-csv-encoding-failed = Codifica del CSV del modello a blocchi non riuscita: { $error }
cmd-file-block-model-csv-export-failed = Esportazione CSV del modello a blocchi non riuscita: { $error }
cmd-file-browser-recovery-unavailable = I file di ripristino del browser non sono disponibili; i progetti salvati restano in IndexedDB
cmd-file-closed-project-runtime-id-runtime = Progetto chiuso, id di runtime { $runtime_id }
cmd-file-could-not-create-new-project = Impossibile creare un nuovo progetto: { $error }
cmd-file-could-not-finish-pending-project = Impossibile completare l'azione di progetto in sospeso: { $error }
cmd-file-could-not-finish-saving-before = Impossibile completare il salvataggio prima dell'uscita: { $error }
cmd-file-could-not-open-browser-project = Impossibile aprire il progetto del browser: { $error }
cmd-file-could-not-open-path-error = Impossibile aprire { $path }: { $error }
cmd-file-could-not-read-selected-file = Impossibile leggere il file selezionato: { $error }
cmd-file-could-not-reload-layer-from = Impossibile ricaricare il livello dal disco: { $error }
cmd-file-could-not-reload-project-from = Impossibile ricaricare il progetto dal disco: { $error }
cmd-file-could-not-remove-browser-project = Impossibile rimuovere il progetto del browser: { $error }
cmd-file-could-not-restore-layer-from = Impossibile ripristinare il livello dal progetto: { $error }
cmd-file-could-not-snapshot-dirty-project = Impossibile creare uno snapshot del progetto modificato per il ripristino: { $error }
cmd-file-could-not-start-browser-export = Impossibile avviare l'esportazione dal browser: { $error }
cmd-file-could-not-write-recovery-copies = Impossibile scrivere le copie di ripristino: { $error }
cmd-file-created-new-browser-project = Creato nuovo progetto nel browser
cmd-file-created-new-project = Creato nuovo progetto
cmd-file-description-download-failed-error = Download di { $description } non riuscito: { $error }
cmd-file-discard-cancelled-project-changed = Lo scarto è stato annullato perché il progetto è cambiato durante il ricaricamento dell'OMF
cmd-file-discarded-changes-layer-target-name = Modifiche scartate per il livello '{ $target_name }'
cmd-file-discarded-changes-reloaded-path = Modifiche scartate: ricaricato { $path }
cmd-file-downhole-geophysics-csv = CSV di geofisica in foro
cmd-file-downloaded-description-file-name = Scaricato { $description }: { $file_name }
cmd-file-drillhole-csv-export-failed-error = Esportazione CSV dei fori di sondaggio non riuscita: { $error }
cmd-file-dxf-download-encoding-failed-error = Codifica del download DXF non riuscita: { $error }
cmd-file-dxf-import-failed-error = Importazione DXF non riuscita: { $error }
cmd-file-encoding-block-model-csv-download = Codifica del download CSV del modello a blocchi…
cmd-file-encoding-dxf-download = Codifica del download DXF…
cmd-file-encoding-triangulation-download = Codifica del download della triangolazione…
cmd-file-exit-deferred-exports = Uscita rimandata fino al completamento delle esportazioni in background
cmd-file-exit-requested-no-unsaved-changes = Uscita richiesta senza modifiche non salvate
cmd-file-exported-block-model-csv-path = CSV del modello a blocchi esportato in { $path }
cmd-file-exported-description-dxf-path = Esportato { $description } in DXF: { $path }
cmd-file-exported-three-drillhole-csvs-path = Esportati tre CSV dei fori di sondaggio in { $path }
cmd-file-exported-triangulation-name-path = Esportata la triangolazione '{ $name }' in { $path }
cmd-file-exporting-name = Esportazione di { $name }…
cmd-file-exporting-triangulation-name-path = Esportazione della triangolazione '{ $name }' in { $path }
cmd-file-fatal-renderer-failure-reason = Errore fatale del renderer: { $reason }
cmd-file-dialog-action-failed = Azione della finestra di dialogo file non riuscita: { $msg }
cmd-file-imported-added-object-s-from = Importato/i { $added } oggetto/i da { $name }
cmd-file-imported-total-dxf-object-s = Importato/i { $total } oggetto/i DXF
cmd-file-layer-discard-was-cancelled-because = Lo scarto del livello è stato annullato perché il progetto è cambiato durante il ricaricamento
cmd-file-no-recovery-directory = Nessuna cartella di ripristino disponibile: { $error }
cmd-file-no-unsaved-project-content-nothing = Nessun contenuto di progetto non salvato; niente da ripristinare
cmd-file-parsing-browser-dxf-import = Analisi dell'importazione DXF dal browser…
cmd-file-parsing-dxf-import = Analisi dell'importazione DXF…
cmd-file-project-closes-after-save = Il progetto si chiuderà al termine del salvataggio in corso
cmd-file-the-project-closes-after-save = Il progetto si chiuderà al termine del salvataggio in corso
cmd-file-queued-count-triangulation-file-s = Accodato/i { $count } file di triangolazione per l'importazione
cmd-file-recovery-copies-path-reopen-them = Le copie di ripristino si trovano in { $path }; riaprile dopo il riavvio
cmd-file-recovery-copy-failed-error = Copia di ripristino non riuscita: { $error }
cmd-file-recovery-copy-failed-failure = Copia di ripristino non riuscita: { $failure }
cmd-file-recovery-copy-written-path = Copia di ripristino scritta: { $path }
cmd-file-reverting-layer = Ripristino del livello…
cmd-file-reverting-project = Ripristino del progetto…
cmd-file-save-failed-message = Salvataggio non riuscito: { $message }
cmd-file-save-project-already-running-save = Un salvataggio di questo progetto è già in corso; salva di nuovo al termine
cmd-file-save-worker-ended-without-result = Il processo di salvataggio è terminato senza risultato
cmd-file-saved-project-as = Progetto salvato come: { $path }
cmd-file-saved-project = Progetto salvato: { $path }
cmd-file-saving-browser-storage = Salvataggio nella memoria del browser…
cmd-file-selected-block-model-no-longer = Il modello a blocchi selezionato non è più caricato
cmd-file-selected-drillhole-dataset-no-longer = Il dataset di fori di sondaggio selezionato non è più caricato
cmd-file-switching-project = Cambio di progetto…
cmd-file-triangulation-download-encoding-failed = Codifica del download della triangolazione non riuscita: { $error }
cmd-file-user-chose-exit-without-saving = L'utente ha scelto di uscire senza salvare
cmd-file-user-requested-exit-project-export = Uscita richiesta dall'utente (richiesta conferma per esportazione progetto o lavoro non salvato)
cmd-file-viewport = Vista
cmd-file-wait-current-project-save-finish = Attendi il completamento del salvataggio del progetto corrente
cmd-file-wait-current-project-switch-finish = Attendi il completamento del cambio di progetto corrente
cmd-file-wait-project-operation-finish-before = Attendi il completamento dell'operazione sul progetto prima di scartare le modifiche
cmd-file-wait-project-revert-finish-before = Attendi il completamento del ripristino del progetto prima di salvare
cmd-folder-collection-named-name-already-exists = Esiste già una raccolta chiamata '{ $name }'
cmd-folder-collection-no-longer-exists = Quella raccolta non esiste più
cmd-folder-created-collection-name = Creata la raccolta '{ $name }'
cmd-folder-deleted-collection-name = Eliminata la raccolta '{ $name }'
cmd-folder-moved-item-into-collection-name = Elemento spostato nella raccolta '{ $name }'
cmd-folder-moved-item-root-section = Elemento spostato nella radice di { $section }
cmd-folder-renamed-collection-before-after = Rinominata la raccolta '{ $before }' in '{ $after }'
cmd-folder-section-cannot-hold-item = Quella sezione non può contenere questo elemento
cmd-fuse-closed-polyline = Polilinea chiusa
cmd-fuse-count-source-line-s = { $count } linea/e sorgente
cmd-fuse-created-shape-object-id-vertices = Creato/a { $shape } { $object_id } con { $vertices } vertici da { $sources } linea/e sorgente
cmd-fuse-click-missed = Fusione: il clic non ha colpito alcun oggetto (nulla sotto il cursore)
cmd-fuse-click-not-near-endpoint = Fusione: il clic non era abbastanza vicino a nessuna delle estremità della linea selezionata
cmd-fuse-clicked-closed-polyline = Fusione: l'oggetto cliccato { $object_id } è una polilinea chiusa, la fusione funziona solo su polilinee aperte
cmd-fuse-clicked-not-open-polyline = Fusione: l'oggetto cliccato { $object_id } non è una polilinea aperta (è un/una { $kind })
cmd-fuse-clicked-object-missing = Fusione: l'oggetto cliccato { $object_id } non esiste più
cmd-fuse-clicked-too-few-vertices = Fusione: la polilinea cliccata { $object_id } ha solo { $count } vertice/i, ne servono almeno 2
cmd-fuse-endpoint-marker-missing = Fusione: il marcatore di estremità { $marker_index } non esiste più
cmd-fuse-close-needs-three-vertices = Fusione: la linea necessita di almeno 3 vertici distinti per chiudersi in una polilinea (ne ha { $count })
cmd-fuse-lines = Fondi linee
cmd-fuse-needs-two-segments = Fusione: servono almeno 2 segmenti per confermare (ne sono presenti { $count })
cmd-fuse-no-active-layer = Fusione: nessun livello attivo su cui collocare la linea fusa
cmd-fuse-no-active-project = Fusione: nessun progetto attivo, impossibile confermare
cmd-fuse-no-source-line = Fusione: nessuna linea sorgente da chiudere in una polilinea
cmd-fuse-awaiting-object-invalid = Fusione: l'oggetto { $awaiting_id } non è più una polilinea valida
cmd-fuse-object-already-in-chain = Fusione: l'oggetto { $object_id } fa già parte della catena di fusione, fai clic su una linea diversa
cmd-fuse-result-too-few-vertices = Fusione: il risultato ha troppo pochi vertici ({ $count }), operazione interrotta
cmd-fuse-segment-object-invalid = Fusione: l'oggetto segmento { $object_id } non è più una polilinea valida, operazione interrotta
cmd-fuse-source-object-invalid = Fusione: l'oggetto sorgente { $object_id } non è più una polilinea aperta valida
cmd-fuse-source-object-missing = Fusione: l'oggetto sorgente { $object_id } non esiste più
cmd-fuse-open-polyline = Polilinea aperta
cmd-include-failed = Inclusione non riuscita: { $message }
cmd-include-included-solid-shape-name-topology = Incluso il solido '{ $shape_name }' nella topologia '{ $topology_name }' (mantenute { $retained } facce della topologia, saltate { $skipped } facce di chiusura)
cmd-include-including-pit-stockpile-solid = Inclusione del solido fossa/cumulo…
cmd-insert-point-count-operation-point-s = { $count } punto/i di { $operation }
cmd-insert-point-elevation-must-be-finite = Inserisci punto a quota richiede una quota finita
cmd-insert-point-insert-points = Inserisci punti
cmd-insert-point-inserted-count-operation-point-s = Inserito/i { $count } punto/i di { $operation }
cmd-insert-point-intersection = Intersezione
cmd-insert-point-no-new-operation-points-were = Non è stato trovato alcun nuovo punto di { $operation }
cmd-insert-point-select-least-two-polylines-before = Seleziona almeno due polilinee prima di inserire punti di intersezione
cmd-insert-point-select-one-more-polylines-before = Seleziona una o più polilinee prima di inserire un punto a quota
cmd-layer-created-layer-name = Creato il livello '{ $name }'
cmd-layer-deleted-with-objects = Eliminato il livello { $layer_id } (e tutti gli oggetti che contiene)
cmd-layer-duplicated-layer-duplicate-name = Duplicato il livello '{ $duplicate_name }'
cmd-layer-locked = Bloccato
cmd-layer-name-copy = { $name } copia
cmd-layer-selected-count-object-s-layer = Selezionato/i { $count } oggetto/i nel livello { $layer_id }
cmd-layer-state-layer-name = Livello '{ $name }' { $state }
cmd-layer-unlocked = Sbloccato
cmd-move-tool-moved-collars = Applicato delta di spostamento ({ $delta }) a { $count } bocca/che foro
cmd-move-tool-moved-objects = Applicato delta di spostamento ({ $delta }) a { $count } oggetto/i
cmd-move-tool-count-hole-s = { $count } foro/i
cmd-object-edit-edited-kind = { $kind } modificato
cmd-object-edit-edited-kind-count-vertices = { $kind } modificato ({ $count } vertici)
cmd-object-edit-no-changes-apply = Nessuna modifica da applicare
cmd-object-edit-object-changed-since-editor-opened = Questo oggetto è cambiato da quando l'editor è stato aperto; riaprilo per modificare la versione attuale
cmd-object-edit-target-changed = L'oggetto in modifica è cambiato; modifica scartata
cmd-object-edit-object-no-longer-exists-document = Questo oggetto non esiste più nel documento
cmd-object-edit-select-single-design-object-edit = Seleziona un singolo oggetto di progetto da modificare
cmd-object-edit-unassigned = Non assegnato
cmd-offset-create-offset = Crea offset
cmd-offset-created-offset-count-object-s = Creato offset di { $count } oggetto/i
cmd-offset-distance-must-be-positive = La distanza di offset deve essere maggiore di zero
cmd-offset-skipped-count-circle-s-offset = Saltato/i { $count } cerchio/i: la distanza di offset è maggiore del raggio
cmd-omf-could-not-open-project-source = Impossibile aprire il progetto { $source_name }: { $error }
cmd-omf-create-open-project-before-merging = Crea o apri un progetto prima di unire i dati
cmd-omf-dataset-name-count-working-section = Dataset '{ $name }': impossibile ripristinare { $count } sezione/i di lavoro: { $details }
cmd-omf-field-codes-partly-coloured = Dataset '{ $name }': il campo '{ $field }' è stato salvato con { $saved } codici colorati su { $total }; agli altri sono stati assegnati colori generati.
cmd-omf-encoding-project = Codifica del progetto…
cmd-omf-exported-project-path = Progetto esportato in { $path }
cmd-omf-imported-project = Importato il progetto '{ $project_name }' da { $source_name }: { $count } dataset di primo livello
cmd-omf-importing-project = Importazione del progetto…
cmd-omf-export-failed = Esportazione OMF non riuscita: { $error }
cmd-omf-import-failed = Importazione OMF non riuscita: { $error }
cmd-omf-opened-project = Aperto il progetto '{ $project_name }' da { $source_name }
cmd-omf-project-source-name-contains-no = Il progetto '{ $source_name }' non contiene elementi di dati supportati
cmd-omf-source-name-applied-project-origin = { $source_name }: applicata l'origine del progetto { $origin } prima dell'unione
cmd-omf-crs-differs = { $source_name }: il sistema di riferimento delle coordinate '{ $source_crs }' differisce dal CRS del progetto '{ $target_crs }'; le coordinate sono state unite senza riproiezione
cmd-omf-source-name-units-source-units = { $source_name }: le unità '{ $source_units }' differiscono dalle unità del progetto '{ $target_units }'; le coordinate sono state unite senza conversione
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = Non ci sono dati di Incline Design aperti da esportare
cmd-placement-2-vertices = 2 vertici
cmd-placement-count-vertices = { $count } vertici
cmd-placement-created-circle = Creato cerchio con raggio { $radius } m
cmd-placement-created-closed-polyline = Creata polilinea chiusa con { $count } vertici
cmd-placement-created-line-segment-2-vertices = Creato segmento di linea con 2 vertici
cmd-placement-created-open-polyline-count-vertices = Creata polilinea aperta con { $count } vertici
cmd-placement-placed-point-x-y-z = Posizionato punto a { $x }, { $y }, { $z }
cmd-placement-radius = Raggio { $radius } m
cmd-plot-composing-engineering-drawing = Composizione del disegno tecnico…
cmd-plot-could-not-write-engineering-drawing = Impossibile scrivere il disegno tecnico: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Scala di disegno adattata ai dati visibili: 1:{ $scale }
cmd-plot = Stampa
cmd-plot-saved-drawing = Disegno tecnico salvato: { $description } ({ $width } × { $height } px a { $dpi } dpi)
cmd-point-cloud-classified = Classificata { $name }: { $ground } terreno, { $vegetation } vegetazione e { $noise } rumore su { $count } punti
cmd-point-cloud-classifying-point-clouds = Classificazione delle nuvole di punti
cmd-point-cloud-join-dropped-classifications = Classificazioni dei punti eliminate: alcune delle nuvole unite non sono classificate e una nuvola parzialmente classificata non può essere filtrata sul terreno.
cmd-point-cloud-failed-classify-point-clouds-error = Classificazione delle nuvole di punti non riuscita: { $error }
cmd-point-cloud-failed-join-point-clouds-error = Unione delle nuvole di punti non riuscita: { $error }
cmd-point-cloud-failed-load-point-cloud-error = Caricamento della nuvola di punti non riuscito: { $error }
cmd-point-cloud-joined-count-clouds-into-name = Unite { $count } nuvole in { $name } ({ $points } punti)
cmd-point-cloud-joining-name = Unione di { $name }
cmd-point-cloud-loaded-point-cloud-name-count = Caricata la nuvola di punti { $name } ({ $count } punti)
cmd-point-cloud-point-cloud-classification-discarded = Classificazione della nuvola di punti scartata: una nuvola è cambiata durante l'esecuzione. Eseguila di nuovo.
cmd-point-cloud-point-cloud-loader-disconnected-path = Il caricatore della nuvola di punti si è disconnesso per { $path }
cmd-point-cloud-select-one-more-loaded-point = Seleziona una o più nuvole di punti caricate prima di classificarle
cmd-point-cloud-select-two-more-loaded-point = Seleziona due o più nuvole di punti caricate prima di unirle
cmd-point-cloud-tin-max-edge-disabled = (lato massimo disattivato)
cmd-point-cloud-tin-max-edge-max-edge = (lato massimo { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = TIN da nuvola di punti non riuscito: { $error }
cmd-point-cloud-tin-filtered-ground = TIN del terreno: filtrati { $ground } punti di terreno su { $total }
cmd-point-cloud-tin-subsampled = TIN del terreno: sottocampionati spazialmente { $sampled } di { $total } punti
cmd-point-cloud-tin-triangulated = TIN del terreno: triangolati { $vertex_count } punti XY unici in { $face_count } facce{ $suffix }
cmd-products-added-product-delay-ms-ms = Aggiunto prodotto { $delay_ms } ms { $name }
cmd-products-deleted-product-delay-ms-ms = Eliminato prodotto { $delay_ms } ms { $name }
cmd-products-failed-save-products-error = Salvataggio dei prodotti non riuscito: { $error }
cmd-products-product-no-longer-palette = Quel prodotto non è più nella tavolozza
cmd-property-action-count-object-s-layer = { $action } { $count } oggetto/i al livello { $layer }
cmd-property-batch-set-axis-value-count = Imposta in blocco il valore { $axis } su { $count } oggetto/i
cmd-property-batch-set-closed-count-polyline = Imposta in blocco lo stato chiuso su { $count } polilinea/e
cmd-property-batch-set-color-count-object = Imposta in blocco il colore su { $count } oggetto/i
cmd-property-batch-set-fill-style-count = Imposta in blocco lo stile di riempimento su { $count } oggetto/i
cmd-property-batch-set-line-weight-count = Imposta in blocco lo spessore linea su { $count } polilinea/e
cmd-property-copied = Copiato
cmd-property-moved = Spostato
cmd-raster-draped = Adagiato il raster { $raster } sulla triangolazione { $triangulation } (estensioni sovrapposte)
cmd-raster-failed-load-raster-name-error = Caricamento del raster { $name } non riuscito: { $error }
cmd-raster-failed-load-raster-path-error = Caricamento del raster { $path } non riuscito: { $error }
cmd-raster-loaded-raster-name-via-driver = Caricato il raster { $name } tramite { $driver } ({ $srcx }x{ $srcy }, anteprima { $prevx }x{ $prevy })
cmd-raster-no-overlapping-triangulation = Nessuna triangolazione caricata si sovrappone all'estensione di { $name }
cmd-raster-loader-disconnected = Il caricatore del raster si è disconnesso per { $path }
cmd-raster-undraped = Rimosso l'adagiamento dei raster da { $count } triangolazione/i
cmd-reference-surface-build-surface-failed-error = Costruzione superficie non riuscita: { $error }
cmd-reference-surface-building-surface = Costruzione della superficie…
cmd-reference-surface-built-surface-name-from-vertex = Costruita la superficie { $name } da { $vertex_count } punto/i in { $face_count } faccia/e, riquadro z da { $low } a { $high }{ $support }{ $coincident }{ $controls }
cmd-reference-surface-control-string-index-could-not = Impossibile aggiungere alla mesh la linea di controllo { $index }
cmd-reference-surface-control-string-index-crosses-itself = La linea di controllo { $index } si interseca da sola in pianta
cmd-reference-surface-control-string-index-doubles-back = La linea di controllo { $index } torna su se stessa in pianta
cmd-reference-surface-control-string-index-ends-where = La linea di controllo { $index } termina dove inizia; chiudila per usarla come maschera
cmd-reference-surface-control-string-index-has-count = La linea di controllo { $index } ha { $count } vertice/i distinto/i; un controllo richiede almeno { $minimum }
cmd-reference-surface-control-string-index-has-non = La linea di controllo { $index } ha coordinate non finite
cmd-reference-surface-control-string-index-no-longer = La linea di controllo { $index } non è più disponibile
cmd-reference-surface-control-string-index-runs-along = La linea di controllo { $index } corre lungo la linea di estensione in pianta; non è ancora supportato
cmd-reference-surface-control-string-overrides-pick-x = La linea di controllo prevale sul punto scelto in ({ $x }, { $y }): punto { $pick } m, controllo { $control } m, differenza { $difference } m
cmd-reference-surface-control-strings-b-disagree-x = Le linee di controllo { $a } e { $b } sono in disaccordo in ({ $x }, { $y }): { $za } m contro { $zb } m, differenza di { $difference } m
cmd-reference-surface-control-strings-b-run-along = Le linee di controllo { $a } e { $b } corrono l'una lungo l'altra in pianta; non è ancora supportato
cmd-reference-surface-count-control-string-s-vertices = ; { $count } linea/e di controllo con { $vertices } vertice/i{ $crossings }
cmd-reference-surface-count-point-s-inside-extent = { $count } punto/i all'interno dell'estensione; una superficie ne richiede almeno { $minimum }
cmd-reference-surface-count-point-s-outside-extent = ; { $count } punto/i fuori dall'estensione hanno modellato la superficie come supporto
cmd-reference-surface-count-point-s-selected-surface = { $count } punto/i selezionato/i; una superficie ne richiede almeno { $minimum }
cmd-reference-surface-count-point-s-shared-plan = ; { $count } punto/i condividevano una posizione in pianta e sono stati mantenuti una sola volta
cmd-reference-surface-delaunay-insert-failed-error = Inserimento Delaunay non riuscito: { $error }
cmd-reference-surface-extent-must-closed-string = L'estensione deve essere una linea chiusa
cmd-reference-surface-extent-string-crosses-itself-plan = La linea di estensione si interseca da sola in pianta
cmd-reference-surface-extent-string-has-non-finite = La linea di estensione ha coordinate non finite
cmd-reference-surface-extent-string-needs-least-three = La linea di estensione richiede almeno tre vertici distinti
cmd-reference-surface-extent-string-no-longer-available = La linea di estensione non è più disponibile
cmd-reference-surface-meeting-count-crossing-s = che si incontrano in { $count } incrocio/i
cmd-reference-surface-and-more = , … e altri { $more }
cmd-reference-surface-no-mask-selected-surface-unclipped = Nessuna maschera selezionata; la superficie non è ritagliata
cmd-reference-surface-no-part-surface-falls-inside = Nessuna parte della superficie ricade all'interno dell'estensione
cmd-reference-surface-open-project-before-building-surface = Apri un progetto prima di costruire una superficie
cmd-reference-surface-points-collinear-plan-surface-needs = I punti sono collineari in pianta; una superficie ne richiede tre che non lo siano
cmd-reference-surface-select-exactly-one-closed-string = Seleziona esattamente una linea chiusa a cui ritagliare la superficie
cmd-reference-surface-selected-point-has-non-finite = Un punto selezionato ha coordinate non finite
cmd-reference-surface-selected-points-span-count-layers = I punti selezionati si estendono su { $count } livelli; la superficie viene collocata in { $section }
cmd-relimit-click-missed = Ridelimitazione: il clic non ha colpito alcun oggetto (nulla sotto il cursore)
cmd-relimit-click-ignored = Ridelimitazione: clic ignorato, lo strumento non è in attesa di una selezione della destinazione
cmd-relimit-clicked-source-line = Ridelimitazione: è stata cliccata la linea sorgente stessa, seleziona una linea diversa
cmd-relimit-no-source-line = Ridelimitazione: nessuna linea sorgente impostata, selezione interrotta
cmd-relimit-relimited-line-source-id-selected = Ridelimitata la linea { $source_id } alla destinazione selezionata
cmd-relimit-resized-line-source-id-using = Ridimensionata la linea { $source_id } usando { $mode } valore { $value }
cmd-rename-item-no-longer-belongs-active = Quell'elemento non appartiene più al progetto attivo
cmd-rename-renamed-before-name = Rinominato '{ $before }' in '{ $name }'
cmd-rename-renamed-name-taken = Rinominato '{ $before }' in '{ $name }' ('{ $requested }' è già in uso)
cmd-rotate-collar-turned-count-drillhole-collar-s = Ruotato/e { $count } bocca/che foro { $rotation }
cmd-section-verb-count-item-s-section = { $verb } { $count } elemento/i in { $section }
cmd-selection-delete-vertex = Elimina vertice
cmd-selection-deleted-count-selected-object-s = Eliminato/i { $count } oggetto/i selezionato/i
cmd-selection-deleted-vertex = Eliminato il vertice { $vertex } dalla polilinea { $object_id }
cmd-selection-duplicate-selection = Duplica selezione
cmd-selection-duplicated-count-object-s = Duplicato/i { $count } oggetto/i
cmd-session-created-triangulation = Creata la triangolazione '{ $name }' ({ $vertex_count } vertici, { $face_count } facce) dal tipo di superficie { $surface_type }
cmd-session-deleted-triangulation = Eliminata la triangolazione '{ $name }' dal progetto
cmd-session-failed-load-triangulation-error = Caricamento della triangolazione non riuscito: { $error }
cmd-session-failed-load-triangulation-message = Caricamento della triangolazione non riuscito: { $message }
cmd-session-loaded-triangulation = Caricata la triangolazione '{ $name }' ({ $path }, { $vertex_count } vertici, { $face_count } facce)
cmd-session-set-triangulation-tri-id-color = Impostato il colore della triangolazione { $tri_id } su { $color }
cmd-session-triangulation-load-no-result = Il caricamento della triangolazione per { $path } è terminato senza risultato
cmd-session-triangulation-failed = Operazione sulla triangolazione non riuscita: { $message }
cmd-session-unloaded-triangulation-name = Scaricata la triangolazione '{ $name }'
cmd-slice-entered-slice-view-cx-cy = Vista sezione avviata @ { $cx }, { $cy }, { $cz } lungo { $dx }, { $dy } (linea di { $length } m)
cmd-slice-exited-slice-view = Vista sezione chiusa
cmd-slice-reset-section-view-fit-extents = Reimposta la vista in sezione (adatta all'estensione)
cmd-slice-set-section-grid-enabled = Griglia della sezione attiva = { $enabled }
cmd-split-created-2-open-polylines = Create 2 polilinee aperte
cmd-split-line = Dividi linea
cmd-split-points-needs-interior-vertex = Dividi ai punti: scegli un vertice interno della linea aperta
cmd-split-polyline-into-two = Divisa la polilinea sorgente in due polilinee aperte
cmd-text-edit-finished = Modifica del testo completata per l'oggetto { $object_id }
cmd-text-updated = Testo aggiornato sull'oggetto { $object_id }
cmd-view-centre-rotation-not-available-flying = Il centro di rotazione non è disponibile in modalità volo
cmd-view-fixed-centre-rotation-x-y = Centro di rotazione fissato a { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = Nessun punto sotto il cursore su cui fissare il centro di rotazione
cmd-view-released-centre-rotation = Centro di rotazione rilasciato
cmd-view-reset-view-fit-extents = Ripristina vista (adatta all'estensione)
cmd-view-reset-view-plan-same-distance = Ripristina vista (pianta alla stessa distanza; fai clic di nuovo per adattare all'estensione)
cmd-view-set-cinematic-view-enabled = Vista cinematica = { $enabled }
cmd-view-set-topology-wireframes-enabled = Wireframe della topologia = { $enabled }
cmd-view-set-view-points-enabled = Visualizzazione punti = { $enabled }
cmd-view-set-xy-grid-enabled = Griglia XY attiva = { $enabled }
cmd-view-zoom-extents-preserving-angle = Zoom su estensione (angolazione invariata)

## Common strings

common-add-product = Aggiungi prodotto
common-appearance = Aspetto...
common-background = Sfondo
common-block-model = Modello a blocchi
common-block-models = Modelli a blocchi
common-borehole-inspector = Ispettore fori di sondaggio
common-build-surface = Costruisci superficie
common-build-surface-ellipsis = Costruisci superficie...
common-cancelled = Annullato
common-chamfer = Smusso
common-choose = Scegli...
common-circle = Cerchio
common-classify = Classifica
common-classify-point-clouds = Classifica nuvole di punti
common-click-point-fix-centre-rotation = Fai clic su un punto per fissare il centro di rotazione
common-clip-surface-polyline = Ritaglia superficie con polilinea...
common-closed = Chiuso
common-collection = Raccolta
common-colour = Colore
common-confirm-omf-rewrite = Conferma riscrittura OMF
common-could-not-replace-current-project = Impossibile sostituire il progetto corrente: { $error }
common-count-object-s = { $count } oggetto/i
common-create = Crea
common-create-batter-berm = Crea scarpa e berma
common-create-bezier-curve = Crea curva di Bézier
common-create-block-model = Crea modello a blocchi
common-create-block-model-ellipsis = Crea modello a blocchi...
common-create-circle = Crea cerchio
common-create-drill-pattern = Crea schema di perforazione
common-create-layer = Crea livello
common-create-line = Crea linea
common-create-ore-triangulation = Crea triangolazione del minerale
common-create-ore-triangulation-ellipsis = Crea triangolazione del minerale...
common-create-point = Crea punto
common-create-polyline = Crea polilinea
common-create-triangulation = Crea triangolazione...
common-crosses = Croci
common-cut = Taglia
common-cut-topology-pit-shell = Taglia topologia con guscio della fossa...
common-delete-collection = Elimina raccolta
common-delete-layer = Elimina livello
common-delete-product = Elimina prodotto
common-delete-selection = Elimina selezione
common-designs = Progettazioni
common-discard-layer-changes = Scarta modifiche al livello
common-down = Giù
common-drape-topology = Adagia sulla topologia
common-easting = Est
common-edit-object = Modifica oggetto
common-edit-text = Modifica testo
common-elevation = Quota
common-exit-without-saving = Esci senza salvare
common-export-engineering-drawing = Esporta disegno tecnico
common-file-was-left-out-downhole = { $file } è stato escluso dalla geofisica in foro: { $error }
common-filter = Filtro
common-fly-mode = Modalità volo
common-generate-contour-lines = Genera curve di livello...
common-hide-all = Nascondi tutto
common-hide-selection = Nascondi selezione
common-hole-id = ID foro
common-ignore = Ignora
common-import-csv-block-model = Importa modello a blocchi CSV
common-import-dxf = Importa DXF
common-incline-design-project = Progetto Incline Design
common-join = Unisci...
common-join-point-clouds = Unisci nuvole di punti
common-joined-cloud = Nuvola unita
common-layer = Livello
common-legend = Legenda
common-line = Linea
common-line-weight = Spessore linea
common-link-geophysics = Collega geofisica...
common-load-drillholes-before-linking-geophysics = Carica il dataset di fori di sondaggio prima di collegarvi la geofisica
common-lock-all = Blocca tutto
common-lock-selection = Blocca selezione
common-m = m
common-max = Max
common-merge-shell-into-topology = Unisci guscio nella topologia
common-merge-shell-into-topology-ellipsis = Unisci guscio nella topologia...
common-modelling = Modellazione
common-move-collar = Sposta bocca foro
common-move-collection = Sposta nella raccolta
common-move-design = Sposta progettazione
common-move-selection = Sposta selezione
common-name-has-no-readable-size = { $name } non ha una dimensione leggibile
common-new-product = Nuovo prodotto
common-no-block-models = Nessun modello a blocchi
common-no-design-layers = Nessun livello di progettazione
common-no-drill-holes = Nessun foro di sondaggio
common-no-file-chosen = Nessun file scelto
common-no-open-project = Nessun progetto aperto
common-no-point-clouds = Nessuna nuvola di punti
common-no-triangulations = Nessuna triangolazione
common-none = Nessuno
common-northing = Nord
common-offset = Offset
common-open = Apri
common-orientation = Orientamento
common-point = Punto
common-point-cloud = Nuvola di punti
common-point-clouds = Nuvole di punti
common-polyline = Polilinea
common-polyline-layer = Polilinea su '{ $layer }'
common-project = Progetto
common-rasters = Raster
common-redo = Ripeti
common-reference-points = Punti di riferimento...
common-relimit-line = Ridelimita linea
common-remove-project = Rimuovi progetto
common-reset-view = Ripristina vista
common-reveal-all = Rivela tutto
common-reveal-finder = Mostra nel Finder
common-rotate-collar = Ruota bocca foro
common-save-exit = Salva ed esci
common-scale-bar = Barra della scala
common-set-initiation-point = Imposta punto di innesco
common-shape = Forma
common-shell = Con guscio
common-slashes = Barre
common-slice = Sezione
common-slice-triangulation-z-range = Seziona triangolazione per intervallo Z...
common-surface-contours = Curve di livello della superficie
common-text = Testo
common-degree-suffix = °
common-tie-holes = Collega fori
common-triangulations = Triangolazioni
common-trim-topology = Rifila sulla topologia...
common-undo = Annulla
common-undrape-all = Rimuovi adagiamento da tutti
common-uniform-white = Bianco uniforme
common-unknown = Sconosciuto
common-unlock-all = Sblocca tutto
common-untitled = Senza titolo
common-up = Su
common-vertical-exaggeration = Esagerazione verticale
common-x = x
common-zoom-extents = Zoom su estensione

## Confirmations strings

confirmations-close-project-unsaved-changes = Chiudi progetto: modifiche non salvate
confirmations-close-without-saving = Chiudi senza salvare
confirmations-delete = Elimina
confirmations-delete-objects = Elimina oggetti
confirmations-discard = Scarta
confirmations-discard-all-unsaved-changes-layer =
    Scartare tutte le modifiche non salvate al livello '{ $name }'?
    Il livello salvato verrà ricaricato dal disco mentre le modifiche agli altri livelli vengono mantenute. Questa azione non può essere annullata.
confirmations-discard-all-unsaved-changes-name =
    Scartare tutte le modifiche non salvate a '{ $name }'?
    L'ultima versione salvata verrà ricaricata dal disco. Questa azione non può essere annullata.
confirmations-discard-changes = Scarta modifiche
confirmations-exit-unsaved-changes = Esci: modifiche non salvate
confirmations-incline-design-cannot-reproduce-all = Incline Design non può riprodurre tutto il contenuto dell'OMF originale. Il salvataggio omette il seguente contenuto:
confirmations-product = Prodotto
confirmations-project = questo progetto
confirmations-remove-name-delete-its-browser = Rimuovere '{ $name }' ed eliminarne la copia salvata nel browser? Le modifiche non salvate andranno perse.
confirmations-remove-project-unsaved-changes = Rimuovi progetto: modifiche non salvate
confirmations-remove-without-saving = Rimuovi senza salvare
confirmations-replace-project-unsaved-changes = Sostituisci progetto: modifiche non salvate
confirmations-save = Salva
confirmations-save-anyway = Salva comunque
confirmations-save-changes-current-project-before = Salvare le modifiche al progetto corrente prima di sostituirlo?
confirmations-save-changes-name-before-closing = Salvare le modifiche a '{ $name }' prima di chiuderlo?
confirmations-save-changes-name-before-removing = Salvare le modifiche a '{ $name }' prima di rimuoverlo da Incline Design?
confirmations-save-close = Salva e chiudi
confirmations-save-modified-project-before-exiting = Salvare il progetto modificato prima di uscire?
confirmations-save-to-browser-before-exit = Salvare il progetto modificato nella memoria del browser prima di uscire?
confirmations-save-remove = Salva e rimuovi

## Console strings

console-copy-all = Copia tutto
console-copy-message = Copia messaggio
console-error = ERRORE
console-info = INFO
console-no-console-activity-yet = Ancora nessuna attività in console
console-pending = IN ATTESA
console-progress-summary = In corso · { $summary }
console-success = SUCCESSO
console-warn = AVVISO

## Csv strings

csv-block-model-category = Categoria
csv-block-model-value = Valore
csv-drill-hole-rows-for-undefined-holes = { $count } righe riguardavano un foro che la geometria del pacchetto non definisce
csv-drill-hole-count-rows-were-skipped-total = { $count } righe sono state saltate in totale
csv-drill-hole-csv-file-empty = Il file CSV è vuoto
csv-drill-hole-csv-has-too-many-unreadable = Il CSV ha troppi byte illeggibili per essere riparato; probabilmente è in una codifica obsoleta, quindi salvalo come UTF-8 e importalo di nuovo
csv-drill-hole-csv-header-has-no-columns = L'intestazione del CSV non ha colonne
csv-drill-hole-csv-headers-must-nonblank-unique = Le intestazioni del CSV devono essere non vuote e univoche
csv-drill-hole-geophysics-needs-geometry = La geofisica in foro richiede nel pacchetto un file di bocche foro o di segmenti espliciti, a cui associare i propri fori
csv-drill-hole-azimuth-out-of-range = { $file } contiene { $count } righe il cui azimut non è compreso tra 0 e 360
csv-drill-hole-dip-out-of-range = { $file } contiene { $count } righe la cui inclinazione non è compresa tra -90 e 90; quelle righe sono state lette senza direzione
csv-drill-hole-file-inclination-values-could-angle = I valori di inclinazione di { $file } che potrebbero essere un angolo sono tutti pari o inferiori a zero, quindi la colonna è stata letta come inclinazione (dip), negativa verso il basso
csv-drill-hole-file-maps-gamma-density-column = { $file } mappa due volte una colonna gamma o densità
csv-drill-hole-invalid-utf8 = { $file } non è UTF-8 valido; { $count } byte illeggibile/i sostituito/i in { $cells } cella/e; una cella danneggiata non viene letta come dato
csv-drill-hole-file-requires-gamma-density-column = { $file } richiede una colonna gamma o densità
csv-drill-hole-row-undefined-hole = La riga { $row } di { $file } riguarda DHID '{ $dhid }', un foro che la geometria del pacchetto non definisce
csv-drill-hole-holes-hole-s-carry-overlapping = { $holes } foro/i hanno intervalli sovrapposti, ad esempio uno strato registrato insieme alle sue suddivisioni: { $summary }
csv-drill-hole-skipped-row-reason = Riga saltata: { $reason }
csv-geophysics-above-5 = sopra 5
csv-geophysics-below-0-5 = sotto 0,5
csv-geophysics-count-more = (+{ $count } altri)
csv-geophysics-count-rows-were-skipped-total = { $count } righe sono state saltate in totale in { $file }
csv-geophysics-csv-has-record-longer-than = Il CSV ha un record più lungo di { $limit } MiB: il file non ha interruzioni di riga dove un CSV le ha, oppure non è testo
csv-geophysics-csv-has-unterminated-quoted-field = Il CSV ha un campo tra virgolette non terminato
csv-geophysics-curve-file-was-left-out = { $curve } in { $file } è stata esclusa: la maggior parte delle sue letture è { $side }, quindi la mediana è fuori dall'intervallo 0,5-5 g/cc e l'unità sembra errata (atteso g/cc). Incline non converte unità; correggi l'esportazione e collega di nuovo
csv-geophysics-file-empty = { $file } è vuoto
csv-geophysics-file-has-no-curve-no = { $file } non ha curve: nessuna colonna oltre all'ID foro e alla profondità contiene numeri
csv-geophysics-file-mapping-has-mapped-columns = La mappatura di { $file } ha { $mapped } colonne, il CSV ne ha { $found }
csv-geophysics-file-no-longer-matches-its = { $file } non corrisponde più al suo indice: collegalo di nuovo
csv-geophysics-file-not-grouped-hole-its = { $file } non è raggruppato per foro: le righe dei suoi fori sono divise in troppi tratti. Ordinalo per ID foro, poi per profondità, e collegalo di nuovo
csv-geophysics-file-requires-one-dhid-one = { $file } richiede una colonna DHID e una di profondità
csv-geophysics-row-blank-hole-id = La riga { $row } di { $file } ha un ID foro vuoto
csv-geophysics-row-column-count = La riga { $row } di { $file } ha { $found } colonne; attese { $expected }
csv-geophysics-row-negative-depth = La riga { $row } di { $file } ha una profondità negativa
csv-geophysics-row-no-depth = La riga { $row } di { $file } non ha una profondità leggibile
csv-geophysics-file-s-path-not-valid = il percorso del file non è UTF-8 valido, e un progetto non può salvarlo: rinomina il file o la sua cartella e collegalo di nuovo
csv-geophysics-rows-skipped = { $file }: { $skipped } righe su { $rows } non hanno potuto essere lette; i motivi sono nella console
csv-geophysics-runs-not-grouped = La geofisica di { $count } foro/i arriva in più di un tratto, non raggruppata per foro; ogni tratto successivo aggiunge solo le profondità in cui il foro non ha letture: { $holes }
csv-geophysics-linked-downhole-geophysics-from-file = Collegata la geofisica in foro da { $file }: { $holes } foro/i, curve { $curves }; { $rows } riga/righe lette, { $skipped } saltate. Le letture restano nel file e vengono lette un foro alla volta
csv-geophysics-no-readings = nessuna lettura
csv-geophysics-no-usable-depth-step = nessun passo di profondità utilizzabile
csv-geophysics-run-count-mismatch = Letti { $read } tratto/i di { $hole }, il collegamento ne ha { $runs }
csv-geophysics-rows-geophysics-row-s-count = { $rows } riga/righe di geofisica per { $count } foro/i che il dataset non definisce non sono collegate: { $holes }
csv-geophysics-rows-readings-would-need-samples = { $rows } letture richiederebbero { $samples } campioni
csv-geophysics-run-hole-curve-was-not = Un tratto di { $hole } { $curve } non è stato mantenuto ({ $reason })
data-table-copy-selection = Copia selezione
data-table-copy-table = Copia tabella
drill-hole-add = Aggiungi
drill-hole-add-all = Aggiungi tutto

## Drill strings

drill-hole-add-stop = Aggiungi stop
drill-hole-add-working-section = Aggiungi sezione di lavoro
drill-hole-all-rendered-intervals-opaque-white = Tutti gli intervalli renderizzati sono bianco opaco.
drill-hole-another-working-section-field-has = Un'altra sezione di lavoro di questo campo ha quel nome.
drill-hole-assumed = Assunto
drill-hole-burden-spacing-must-greater-than = Resistenza e interasse devono essere maggiori di zero
drill-hole-cache-drill-hole-set-name-has = Il set di fori { $name } ha { $count } fori e collegamenti, oltre i { $capacity } che l'evidenziazione della selezione può gestire: selezionare il set nel suo insieme lo evidenzia ancora, selezionare singoli fori no
drill-hole-cache-drill-hole-set-name-stations = Set di fori { $name }: { $stations } stazioni, { $before } segmenti uniti in { $after }, { $cells } celle
drill-hole-choose-valid-closed-polyline = Scegli una polilinea chiusa valida
drill-hole-clear-filter = Cancella il filtro
drill-hole-code-already-in-section = { $code } è già nella sezione di lavoro { $section }.
drill-hole-code-outside-section-has-name = Un codice esterno a questa sezione ha quel nome. Una sezione può condividere il nome solo con un codice che contiene.
drill-hole-colour-scale = Scala di colore
drill-hole-count-codes = { $count } codici
drill-hole-count-codes-interval-no-logged = { $count } codici. Un intervallo senza valore registrato resta bianco.
drill-hole-disc-diameter = Diametro del disco
drill-hole-appearance-title = Aspetto dei fori di sondaggio: { $name }
drill-hole-drilled-diameter = Del diametro di perforazione
drill-hole-every-code-lists-already-another = Ogni codice che elenca è già in un'altra sezione di lavoro.
drill-hole-every-interval-value-colour-field = Ogni intervallo con un valore nel campo colore è disegnato come un disco di questa larghezza sulla linea. In lontananza non è mai più stretto di pochi pixel.
drill-hole-field = Campo
drill-hole-field-working-section = { $field } per sezione di lavoro
drill-hole-floor = Letto
drill-hole-grayscale = Scala di grigi
drill-hole-green-yellow-red = Verde–Giallo–Rosso
drill-hole-heat = Calore
drill-hole-drilled-width-help = Un foro alla sua larghezza di perforazione appare come un tubo accanto alla geologia; un set di migliaia appare come un tappeto.
drill-hole-line-width-help = Il foro stesso è disegnato come una linea di questa larghezza a ogni livello di zoom.
drill-hole-however-far-eye-hole-drawn = Per quanto lontano sia l'occhio, un foro è disegnato almeno di questa larghezza.
drill-hole-measured = Misurato
drill-hole-name-working-section = { $name } (sezione di lavoro)
drill-hole-never-thinner-than = Mai più sottile di
drill-hole-new-section-name = Nome della nuova sezione
drill-hole-new-working-section = Nuova sezione di lavoro
drill-hole-no-holes-fit-inside-boundary = Nessun foro rientra in questo confine con la resistenza e l'interasse correnti
drill-hole-part-code = Parte di un codice
drill-hole-pattern-too-many-holes = Lo schema supera il massimo di { $maximum } fori; aumenta la resistenza o l'interasse
drill-hole-preset = Preimpostazione
drill-hole-px = px
drill-hole-rainbow = Arcobaleno
drill-hole-reset-colours = Ripristina colori
drill-hole-reset-preset = Ripristina preimpostazione
drill-hole-reset-shown-colours = Ripristina colori mostrati
drill-hole-roof = Tetto
drill-hole-rotation-offsets-must-contain-valid = Rotazione e offset devono contenere numeri validi
drill-hole-selected-polyline-has-no-usable = La polilinea selezionata non ha un'area XY utilizzabile
drill-hole-shown-total-codes-shown = { $shown } di { $total } codici mostrati
drill-hole-shown-total-rows-shown = { $shown } di { $total } righe mostrate
drill-hole-smooth-interpolation = Interpolazione morbida
drill-hole-spacing-would-scan-too-many = Questo interasse esaminerebbe troppe celle della griglia; aumenta la resistenza o l'interasse (massimo { $maximum } fori)
drill-hole-square = Quadrato
drill-hole-staggered = Sfalsato
drill-hole-stepped-bands = Fasce a gradini
drill-hole-string-discs = Linea e dischi
drill-hole-string-discs-where-intervals-overlap = Come linea e dischi, dove gli intervalli si sovrappongono quello più corto è disegnato come disco.
drill-hole-string-width = Larghezza della linea
drill-hole-style = Stile
drill-hole-suggested-from-code-names-count = Suggeriti dai nomi dei codici ({ $count })
common-times-sign = ×
common-minus-sign = −
drill-hole-ticked-but-hidden-filter-count = Spuntati ma nascosti dal filtro: { $count }
drill-hole-true-diameter = Diametro reale
drill-hole-unsupported-drillhole-source = Sorgente di fori di sondaggio non supportata
drill-hole-width = Larghezza
drill-hole-working-section-needs-name = Una sezione di lavoro richiede un nome.
drill-hole-working-section-set-seams-plies = Una sezione di lavoro è un insieme di strati o banchi coltivati come un'unica unità. Colorare per sezione dà all'intero insieme un solo colore.
drill-hole-working-sections = Sezioni di lavoro
drill-pattern-arrangement = Disposizione
drill-pattern-axis-offset = Offset { $axis }
drill-pattern-blast-shape = Forma della volata
drill-pattern-burden = Resistenza (burden)
drill-pattern-choose-closed-blast-boundary-then = Scegli un confine di volata chiuso, quindi regola la griglia. I fori di sondaggio si aggiornano in tempo reale nella vista.
drill-pattern-closed-design-polyline-whose-xy = La polilinea di progettazione chiusa la cui impronta XY verrà riempita di fori.
drill-pattern-rotation-help = Rotazione antioraria del pattern dall'asse globale { $axis }.
drill-pattern-distance-between-holes-along-each = Distanza tra i fori lungo ciascuna fila dello schema.
drill-pattern-name-hint = es. Taglio Ovest 03
drill-pattern-diameter-help = Diametro finito del foro. Inserito in millimetri e memorizzato con ogni foro generato.
drill-pattern-hole-depth = Profondità del foro
drill-pattern-hole-diameter = Diametro del foro
drill-pattern-move-over-closed-polyline-then = Passa sopra una polilinea chiusa, poi fai clic su di essa nella vista. Esc annulla la selezione.
drill-pattern-name-help = Nome del set di fori di sondaggio creato nel progetto.
drill-pattern-none-picked = Nessuna selezionata
drill-pattern-pattern-name = Nome dello schema
drill-pattern-spacing-help = Distanza perpendicolare tra le file dello schema.
drill-pattern-pick = Seleziona
drill-pattern-preview-count-hole-s-diameter = Anteprima: { $count } foro/i · diametro { $diameter } mm · profondità { $depth } m
drill-pattern-rotation = Rotazione
drill-pattern-shift-pattern-grid-along-global = Sposta la griglia del pattern lungo l'asse globale { $axis } mantenendola ritagliata alla forma della volata.
drill-pattern-spacing = Interasse
drill-pattern-staggered-offsets-every-second-row = Sfalsato sposta ogni seconda fila di metà dell'interasse.
drill-pattern-vertical-depth-below-each-collar = Profondità verticale sotto ciascuna bocca foro.

## Dxf strings

dxf-block-nesting-too-deep = L'annidamento di blocchi DXF supera la profondità massima ({ $depth }), '{ $name }' saltato
dxf-circular-block-reference = Rilevato riferimento circolare a blocco DXF: '{ $name }'
dxf-undefined-layer = L'entità DXF faceva riferimento al livello non definito '{ $name }', importato come '{ $fallback }'
dxf-import-budget-exceeded = L'importazione DXF supera il budget di { $what } ({ $limit }); la geometria rimanente viene saltata
dxf-insert-unknown-block = L'INSERT DXF fa riferimento al blocco sconosciuto '{ $name }'

## Edit strings

edit-absolute-length = Lunghezza assoluta
edit-absolute-rl = RL assoluta
edit-action = Azione
edit-angle = Angolo
edit-dip-help = Angolo dall'orizzontale, negativo verso il basso: -90 è un foro verticale.
edit-app-web-not-recommended-production = { $app } Web non è consigliata per un uso in produzione. Usala solo come demo.
edit-application = Applicazione
edit-apply = Applica
edit-apply-pick-target = Applica e scegli destinazione
edit-axis-value = valore { $axis }
edit-azimuth = Azimut
edit-batter-angle = Angolo di scarpa (°)
edit-azimuth-help = Rilevamento su cui vengono perforati i fori, in gradi in senso orario dal nord di griglia.
edit-bench-height = Altezza gradino
edit-benches = Gradini
edit-berm-width = Larghezza berma
edit-bezier-curve = Curva di Bézier
edit-choose-layer = Scegli un livello
edit-measure-help = Scegli se il valore inserito è la distanza lungo il pendio, la larghezza orizzontale o l'altezza verticale.
edit-choose-which-two-polyline-paths = Scegli quale dei due percorsi della polilinea tra i vertici selezionati verrà sostituito. La lunghezza include quota e lati curvi.
edit-click-corner-closed-polyline = Fai clic su un angolo di una polilinea chiusa.
edit-click-open-closed-polyline-begin = Fai clic su una polilinea aperta o chiusa per iniziare.
edit-click-second-vertex-replacement-span = Fai clic sul secondo vertice del tratto di sostituzione.
edit-click-vertex-start-replacement-span = Fai clic su un vertice per iniziare il tratto di sostituzione.
edit-collide-triangulation = Collidi con la triangolazione
edit-confirm-selection = Conferma selezione
edit-control-point-1 = Punto di controllo 1
edit-control-point-2 = Punto di controllo 2
edit-copy = Copia
edit-corner-radius-limited-so-replacement = Raggio dell'angolo, limitato affinché la sostituzione non superi i vertici adiacenti.
edit-create-new-layer = Crea un nuovo livello
edit-create-new-project = Crea un nuovo progetto
edit-create-project = Crea progetto
edit-delta-length-m-use = Delta lunghezza (m, usa + o -)
edit-dip = Inclinazione
edit-direction = Direzione
edit-distance = Distanza
edit-distance-along-slope = Distanza lungo il pendio
edit-download-free-native-version-our = Scarica la versione nativa gratuita sul nostro sito web ↗
edit-drill-hole = Foro di sondaggio
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = Fine
edit-enter-valid-elevation = Inserisci una quota valida.
edit-exit-slice = Esci dalla sezione
edit-finish-polyline = Termina polilinea
edit-generate-batter-berms = Genera scarpe e berme
edit-height = Altezza
edit-height-change = Variazione di altezza
edit-height-mode = Modalità altezza
edit-horizontal-distance = Distanza orizzontale
edit-horizontal-width-each-flat-berm = Larghezza orizzontale di ciascuna berma piana tra scarpe successive.
edit-hover-choose-which-end-move = Passa sopra per scegliere quale estremità spostare, poi fai clic per confermare.
edit-insert-point-elevation = Inserisci punto a quota
edit-intersect = Interseca
edit-kind-properties = { $properties } { $kind }
edit-layer-name = Nome livello
edit-load-project = Carica progetto
edit-longest = Più lungo
edit-m-s = m/s
edit-measure = Misura
edit-mit-license = Licenza MIT
edit-mode = Modalità
edit-move = Sposta
edit-move-layer = Sposta al livello
edit-move-which-end = Quale estremità spostare
edit-movement-speed-slice-when-using = Velocità di movimento della sezione quando si usano i tasti di navigazione.
edit-moving-end-endpoint = Spostamento: estremità finale
edit-moving-start-endpoint = Spostamento: estremità iniziale
edit-new-length-m = Nuova lunghezza (m)
edit-new-project = Nuovo progetto
edit-number-complete-batter-berm-levels = Numero di livelli completi di scarpa e berma. Il massimo è limitato al livello più profondo che conserva la geometria specificata.
edit-bezier-segments-help = Numero di segmenti usati per approssimare la curva tra i due vertici selezionati.
edit-chamfer-segments-help = Numero di segmenti rettilinei usati per approssimare l'angolo arrotondato. Usa 1 per uno smusso diritto.
edit-object = Oggetto
edit-offset-element = Elemento offset
edit-pick-side = Seleziona lato
edit-pit = Fossa
edit-project-name = Nome progetto
edit-properties = Proprietà
edit-radius = Raggio
edit-recent = Recenti
edit-relative = Relativo (+/-)
edit-elevation-mode-help = Relativo applica una variazione verticale a ogni punto. RL assoluta proietta ogni punto su un'unica quota di destinazione.
edit-remove-from-list = Rimuovi dall'elenco
edit-replace-path = Sostituisci percorso
edit-rotate = Ruota
edit-rotation-speed-slice-when-using = Velocità di rotazione della sezione quando si usano Q ed E.
edit-s = °/s
edit-segments = Segmenti
edit-segments-lying-elevation-ignored = I segmenti che si trovano a questa quota vengono ignorati.
edit-endpoint-help = Seleziona l'estremità che cambia; l'altra estremità resta fissa.
edit-selected-holes-point-different-ways = I fori selezionati puntano in direzioni diverse. Applica li imposta tutti su questi angoli.
edit-selected-start-end-point-moves = Il punto iniziale o finale selezionato si sposta lungo la direzione della linea; l'estremità opposta resta fissa.
edit-set-axis = Imposta { $axis }
edit-shortest = Più corto
edit-slice-view = Vista sezione
edit-slope-angle-each-batter-face = Angolo di pendenza di ciascuna faccia di scarpa, misurato dall'orizzontale.
edit-slope-angle-offset-positive-negative = Angolo di pendenza dell'offset. Angoli positivi e negativi spostano la copia sopra o sotto la sorgente mentre si sposta lateralmente.
edit-speed = Velocità
edit-start = Inizio
edit-stockpile = Cumulo
edit-stop-generated-offset-where-its = Interrompi l'offset generato dove il suo percorso incontra per la prima volta una triangolazione visibile.
edit-target-rl = RL di destinazione
edit-text-colour-opacity = Colore e opacità del testo.
edit-thickness-visible-slice-slab-centred = Spessore della lastra di sezione visibile, centrata sull'indicatore della panoramica.
edit-translation-axis-help = Distanza di traslazione lungo l'asse mondiale { $axis }.
edit-type = Tipo
edit-type-direction-together-set-offset = Tipo e Direzione insieme impostano il lato dell'offset. Fossa + Su e Cumulo + Giù avanzano verso l'esterno; Fossa + Giù e Cumulo + Su avanzano verso l'interno.
edit-bench-direction-help = Su alza ogni gradino dell'altezza del gradino; Giù lo abbassa. Questo inverte anche il lato dell'offset: vedi Tipo.
edit-value-help = Il valore viene interpretato in base alla Misura e alla Modalità altezza selezionate.
edit-vertical-rise-fall-each-bench = Risalita o discesa verticale di ciascun gradino prima che venga creata la berma successiva.
edit-bezier-control-point-1-help = Coordinate mondo X, Y e Z del primo punto di controllo di Bézier.
edit-bezier-control-point-2-help = Coordinate mondo X, Y e Z del secondo punto di controllo di Bézier.

## Events strings

events-couldn-t-exit-error = Impossibile uscire: { $error }
events-couldn-t-save-error = Impossibile salvare: { $error }
events-set-elevation = Imposta quota
events-set-elevation-from-cursor-hit = Impostata la quota dall'intersezione del cursore a Z { $z }
events-tool-not-available-section-view = Questo strumento non è disponibile nella vista in sezione

## Explorer strings

explorer-clear-active-triangulation-texture = Cancella texture della triangolazione attiva
explorer-delete-from-project = Elimina dal progetto
explorer-discard-changes = Scarta modifiche...
explorer-download = Scarica
explorer-drape-over-surface = Adagia sulla superficie
explorer-draped-over-surface = Drappeggiato su una superficie
explorer-duplicate = Duplica
explorer-empty-collection = Raccolta vuota
explorer-face-colour = Colore faccia
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    { $count } variabile/i colore
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    { $holes } foro/i
    { $fields } campo/i colore
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    { $count } punto/i
explorer-raster-id =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = Carica
explorer-lock = Blocca
explorer-new-collection = Nuova raccolta
explorer-no-collection = Nessuna raccolta
explorer-select-all-objects = Seleziona tutti gli oggetti
explorer-source-name = Sorgente: { $name }
explorer-unload = Scarica
explorer-unlock = Sblocca

## Files strings

files-automatic-colour = Colore automatico
files-automatic-rl-spacing = Spaziatura quote automatica
files-axis-scale-ratio = Rapporto di scala { $axis }
files-ok = OK
files-reset-scale = Ripristina a 1×
files-rl-grid-options = Opzioni griglia quote
files-rl-spacing = Spaziatura quote
files-scales-z-distances-visually-without = Riscala visivamente le distanze Z senza modificare le coordinate memorizzate.
files-thickness = Spessore
files-xy-grid-options = Opzioni griglia XY
geophysics-checking-geophysics-files = Verifica dei file di geofisica
geophysics-downhole-geophysics-name-could-not = Impossibile collegare la geofisica in foro per '{ $name }': { $error }
geophysics-file-changed = Il file di geofisica è cambiato dopo l'indicizzazione
geophysics-file-unreadable = Il file di geofisica collegato a '{ $name }' non può essere letto in { $path } ({ $error }); collegalo di nuovo dal menu contestuale del dataset
geophysics-linked-changed-rereading = La geofisica collegata a '{ $name }' è cambiata dopo l'indicizzazione; lettura ripetuta
geophysics-hole-has-size-mib-geophysics = { $hole } ha { $size } MiB di righe di geofisica, più di quante un foro ne legga
geophysics-hole-needs-size-mib-its = { $hole } richiede { $size } MiB per la sua geofisica, più di quanto resti al browser: scarica altri elementi, poi scarica e ricarica questo dataset
geophysics-linking-geophysics-name = Collegamento della geofisica a { $name }
geophysics-reading-geophysics-hole = Lettura della geofisica per { $hole }
geophysics-web-could-not-read-name-error = Impossibile leggere '{ $name }': { $error }
geophysics-web-name-used-session-s-downhole = '{ $name }' è usato per la geofisica in foro di questa sessione

## Gpu strings

gpu-cache-block-model-surface-build-failed = Costruzione della superficie del modello a blocchi non riuscita: { $error }
gpu-cache-block-model-surface-build-worker = Il processo di costruzione della superficie del modello a blocchi si è disconnesso
gpu-cache-block-model-surface-chunk-rejected = Chunk della superficie del modello a blocchi rifiutato prima dell'allocazione GPU: instances={ $instances } byte, limit={ $limit } byte
gpu-cache-block-volume-worker-disconnected = Il processo di preparazione del volume di blocchi si è disconnesso
gpu-cache-translucent-volume-could-not-built = Impossibile costruire il volume traslucido ({ $error }); questo modello a blocchi verrà mostrato come cubi.
gpu-cache-edge-chunk-rejected = Chunk dei bordi della triangolazione rifiutato prima dell'allocazione GPU: instances={ $instances } byte, limit={ $limit } byte
gpu-cache-triangulation-chunk-rejected = Chunk GPU della triangolazione rifiutato prima dell'allocazione: vertices={ $vertices } byte, indices={ $indices } byte, limit={ $limit } byte
gpu-cache-triangulation-too-many-vertices = La triangolazione '{ $name }' ha { $count } vertici (> u32::MAX); impossibile suddividerla in chunk per la GPU
gpu-cache-triangulation-uploaded = La triangolazione '{ $name }' è stata caricata in { $chunks } chunk spaziali ({ $faces } facce)
i18n-active-language = La lingua attiva è { $language } (integrata: { $bundled })
i18n-could-not-select-language-error = Impossibile selezionare una lingua: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = Adattatore GPU: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = Driver GPU: { $driver } { $driver_info }
init-gpu-limits-max-buffer-size = Limiti GPU: max_buffer_size={ $max_buffer_size } MiB, max_storage_buffer_binding_size={ $max_storage_buffer_binding_size } MiB, max_storage_buffers_per_shader_stage={ $max_storage_buffers_per_shader_stage }, max_uniform_buffer_binding_size={ $max_uniform_buffer_binding_size } KiB, max_texture_dimension_2d={ $max_texture_dimension_2d }, max_bind_groups={ $max_bind_groups }
init-gpu-supports-maximum-buffer-size = La GPU supporta una dimensione massima del buffer di { $size } MiB; le scene di grandi dimensioni potrebbero non essere visualizzate completamente
init-surface-present-mode = Modalità di presentazione della superficie: { $mode }
init-wgpu-error-continuing-error = Errore wgpu (si continua): { $error }
input-could-not-read-name-error = impossibile leggere { $name }: { $error }
input-could-not-slice-name-error = impossibile sezionare { $name }: { $error }
io-add-collar-file-explicit-segments = Aggiungi il file delle bocche foro (o un file di segmenti espliciti): la geofisica in foro si associa ai fori che esso definisce.

## Io strings

io-ascii-points-xyz-pts = Punti ASCII (.xyz, .pts)
io-attribute = Attributo
io-blank-header = (intestazione vuota)
io-block-model = Modello a blocchi:
io-choose-file-purpose-map-its = Scegli lo scopo del file per mappare le sue colonne.
io-choose-loaded-block-model = Scegli un modello a blocchi caricato
io-choose-loaded-dataset = Scegli un dataset caricato
io-choose-loaded-layer = Scegli un livello caricato
io-choose-loaded-triangulation = Scegli una triangolazione caricata
io-choose-purpose = Scegli lo scopo…
io-choose-source-file-files-import = Scegli il file o i file sorgente da importare.
io-collar = Bocca foro
io-column-mapping = Mappatura colonne
io-comma-separated-values-csv = Comma-Separated Values (.csv)
io-csv-files = File CSV
io-dataset = Dataset:
io-default = Predefinito
io-density-read-g-cc-exported = Densità, letta come g/cc, come esportata. Una curva la cui mediana non è tra 0,5 e 5 g/cc viene esclusa dall'importazione con un avviso, perché la sua unità sembra errata.
io-depth = Profondità
io-diameter = Diametro
io-downhole-geophysics = Geofisica in foro
io-drawing-exchange-format-dxf = Drawing Exchange Format (.dxf)
io-drill-holes = Fori di sondaggio
io-east-x = Est / X
io-elevation-z = Quota / Z
io-end-x = X finale
io-end-y = Y finale
io-end-z = Z finale
io-explicit-segments = Segmenti espliciti
io-export = Esporta
io-export-csv-block-model = Esporta CSV modello a blocchi
io-export-csv-drillholes = Esporta fori di sondaggio CSV
io-export-dxf = Esporta DXF
io-export-one-layer = Esporta un livello
io-export-open-mining-format-2 = Esporta Open Mining Format 2
io-export-ply = Esporta PLY
io-export-stl = Esporta STL
io-export-wavefront-obj = Esporta Wavefront OBJ
io-gamma-api = Gamma (API)
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Importa
io-import-ascii-point-cloud = Importa nuvola di punti ASCII
io-import-drillhole-csv-bundle = Importa pacchetto CSV fori di sondaggio
io-import-geotiff = Importa GeoTIFF
io-import-las-laz-point-cloud = Importa nuvola di punti LAS/LAZ
io-import-open-mining-format-2 = Importa Open Mining Format 2
io-import-pcd-point-cloud = Importa nuvola di punti PCD
io-import-ply = Importa PLY
io-import-stl = Importa STL
io-import-wavefront-obj = Importa Wavefront OBJ
io-inclination = Inclinazione
io-interval = Intervallo
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-long-spaced-density-g-cc = Densità a spaziatura lunga (g/cc)
io-mapped-csv-bundle-csv = Pacchetto CSV mappato (.csv)
io-measured-depth-down-hole-read = Profondità misurata lungo il foro, letta in metri. Incline non converte unità: le imposta il database che ha esportato il file.
io-model-file = File del modello
io-name-count-files = { $name } + { $count } file
io-natural-gamma-read-api-units = Gamma naturale, letta in unità API, come esportata.
io-no-csv-chosen = Nessun .csv scelto
io-no-csv-files-chosen = Nessun file CSV scelto
io-no-dxf-chosen = Nessun .dxf scelto
io-no-omf-chosen = Nessun .omf scelto
io-north-y = Nord / Y
io-open-mining-format-2-omf = Open Mining Format 2 (.omf)
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-projects = Progetti
io-short-spaced-density-g-cc = Densità a spaziatura corta (g/cc)
io-source-file = File sorgente
io-start-x = X iniziale
io-start-y = Y iniziale
io-start-z = Z iniziale
io-stl = STL (.stl)
io-triangulation = Triangolazione:
io-unmapped = Non mappato
io-wavefront-obj = Wavefront OBJ (.obj)
io-writes-three-files-beside-name = Scrive tre file accanto al nome scelto: bocche foro, rilievo e intervalli, nelle colonne che questa finestra importa.

## Jobs strings

jobs-background-task-poll-label-ended = L'attività in background '{ $poll_label }' è terminata senza risultato
jobs-cancelled-label-its-project-no = Annullato '{ $label }': il suo progetto non è più attivo
jobs-discarded-stale-result = Scartato il risultato in background obsoleto per '{ $poll_label }' perché una sorgente è cambiata o è stata chiusa
jobs-drillhole-import = un'importazione di fori di sondaggio
log-traces-auto-from-hole = Auto, da questo foro
log-traces-curve-no-reading = { $curve }: nessuna lettura
log-traces-curve-value-unit = { $curve }: { $value } { $unit }
log-traces-custom-range = Intervallo personalizzato
log-traces-default-colour = Colore predefinito
log-traces-density-scale = Scala della densità
log-traces-depth-m = { $depth } m
log-traces-gamma = Gamma
log-traces-gamma-colour = Colore gamma
log-traces-gamma-scale = Scala gamma
log-traces-percentile-range-no-data = Dal 1° al 99° percentile del foro, arrotondato verso l'esterno. Questo foro non ha ancora dati per esso.
log-traces-percentile-range = Dal 1° al 99° percentile del foro, arrotondato verso l'esterno: { $range }.
log-traces-long-density = Densità lunga
log-traces-long-density-colour = Colore densità lunga
log-traces-min-max-unit = da { $min } a { $max } { $unit }
log-traces-reading = Lettura...
log-traces-short-density = Densità corta
log-traces-short-density-colour = Colore densità corta

## Logging strings

logging-activity-completed = Attività completata
logging-activity-started = Attività avviata
logging-application-id-id = ID applicazione: { $id }
logging-application-name = Nome applicazione: { $name }
logging-application-startup = Avvio dell'applicazione
logging-build-target-os-architecture = Target di build: { $os }-{ $architecture }
logging-completed = Completato
logging-count-messages = { $count } messaggi
logging-desktop-session-xdg-session-type = Sessione desktop: XDG_SESSION_TYPE={ $session }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Inizializzazione di Incline Design
logging-locale-environment = Ambiente locale: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session = Sessione macOS: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = Sistema operativo: GNU / Linux
logging-operating-system-macos = Sistema operativo: macOS
logging-operating-system-microsoft-windows = Sistema operativo: Microsoft Windows
logging-pointer-width = Larghezza puntatore: { $width } bit
logging-process-id-id = ID processo: { $id }
logging-release-version = Versione release: { $version }
logging-renderer = Renderer
logging-rust-compiler-host = Host del compilatore Rust: { $host }
logging-system = Sistema
logging-system-error = Errore di sistema
logging-unknown = sconosciuto
logging-windows-session-sessionname-session = Sessione Windows: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = In corso…

## Mac strings

mac-cannot-install-macos-menu-bar = Impossibile installare la barra dei menu di macOS al di fuori del thread principale
mac-quit-app = Esci da { $app }

## Main strings

main-incline-design-web-startup-failed = Avvio di Incline Design Web non riuscito: { $error }

## Menu strings

menu-count-files-selected = { $count } file selezionati

## Object strings

object-edit-appearance = Aspetto
object-edit-arc-circle = Arco e cerchio
object-edit-arc-segments = Segmenti d'arco
object-edit-bulge = Freccia
object-edit-bulge-arcs-horizontal-data-model = Gli archi con freccia sono orizzontali per modello dati: l'arco curva in pianta e la quota varia in linea retta da un vertice al successivo.
object-edit-centre-x = Centro X
object-edit-centre-y = Centro Y
object-edit-centre-z = Centro Z
object-edit-chord = Corda
object-edit-colour-layer = Colore per livello
object-edit-enter-number = Inserisci un numero
object-edit-follow-owning-layer-s-colour = Segui il colore del livello proprietario invece di un colore fissato a questo oggetto.
object-edit-id = ID
object-edit-identity = Identità
object-edit-insert-after = Inserisci dopo
object-edit-join-last-vertex-back-first = Ricongiunge l'ultimo vertice al primo.
object-edit-length = Lunghezza { $length } m
object-edit-move-down = Sposta giù
object-edit-move-up = Sposta su
object-edit-object-has-no-arc-segments = Questo oggetto non ha segmenti d'arco.
object-edit-object-has-single-position = Questo oggetto ha una sola posizione.
object-edit-object-needs-least-required-vertices = Questo oggetto richiede almeno { $required } vertici
object-edit-one-more-properties-not-valid = Una o più proprietà non sono un numero valido
object-edit-perimeter-area = Perimetro { $length } m, area { $area } m²
object-edit-reverse = Inverti
object-edit-row-invalid-number = Riga { $row }: la posizione o la freccia non sono un numero valido
object-edit-sweep = Angolo di spazzata
object-edit-text-not-number = "{ $text }" non è un numero
object-edit-vertices = Vertici

## Omf strings

omf-element-name-has-count-tie = L'elemento '{ $name }' ha { $count } collegamento/i che nominano fori non più presenti
omf-element-name-has-count-unreadable = L'elemento '{ $name }' ha { $count } sezione/i di lavoro illeggibile/i; sono state escluse
omf-element-unsupported-section = L'elemento '{ $name }' indica la sezione '{ $section }' che in questa versione non può mostrare questo tipo di elemento
omf-element-name-names-unknown-section = L'elemento '{ $name }' indica una sezione sconosciuta '{ $section }'
omf-ignoring-colour-map-omf-attribute = Mappa colori ignorata sull'attributo OMF '{ $attribute }': { $error }
omf-mining-data-exported-incline = Dati minerari esportati da Incline
omf-import = Importazione OMF
omf-texture = Texture OMF
omf-validation-warnings = Avvisi di validazione OMF: { $warnings }
omf-application-metadata-dropped = I metadati dell'applicazione di progetto '{ $application }' non vengono mantenuti
omf-project-author-not-retained = L'autore del progetto non viene mantenuto
omf-project-description-not-retained = La descrizione del progetto non viene mantenuta
omf-unsupported-metadata-keys = Il progetto ha chiavi di metadati non supportate: { $keys }
omf-skipped-drillhole-data-saved-older = Saltati i dati dei fori di sondaggio salvati in un layout precedente ({ $names }); importali di nuovo dai file sorgente

## Plot strings

plot-1-1000-one-millimetre-sheet = A 1:1000, un millimetro sul foglio corrisponde a un metro sul terreno.
plot-1-scale-covers-width-height = 1:{ $scale } · copre { $width } × { $height } m
plot-all-visible-data = Tutti i dati visibili
plot-automatic-grid-interval = Intervallo automatico della griglia
plot-border = Bordo
plot-centre = Centra su
plot-fit-scale-help = Scegli la scala convenzionale più piccola che fa stare tutto ciò che è visibile sul foglio.
plot-coordinate-grid = Griglia di coordinate
plot-current-view-centre = Centro della vista corrente
plot-date-caps = DATA
plot-date = Data
plot-dots-per-inch-paper-size = Punti per pollice. Questo formato carta può essere rasterizzato fino a { $max_dpi } dpi; 300 dpi è una qualità di stampa normale.
plot-dpi = dpi
plot-drawing-no = DISEGNO N.
plot-drawing-number = Numero di disegno
plot-drawn-by-caps = DISEGNATO DA
plot-drawn-by = Disegnato da
plot-e-g-example-gold-project = es. Progetto Oro di Esempio
plot-entered-coordinates = Coordinate inserite
plot-export-png = Esporta PNG...
plot-fit-scale-visible-data = Adatta scala ai dati visibili
plot-grid-interval = Intervallo griglia
plot-landscape = Orizzontale
plot-lists-visible-surfaces-design-layers = Elenca le superfici visibili e i livelli di progettazione con i rispettivi colori.
plot-margin = Margine
plot-margins-leave-no-room-map = I margini non lasciano spazio per la mappa
plot-metres-scale-1-scale = metri    Scala 1:{ $scale }
plot-mm = mm
plot-north-arrow = Freccia nord
plot-nothing-visible-draw = Nulla di visibile da disegnare
plot-paper = Carta
plot-paper-orientation-width-height-mm = { $paper } { $orientation } · { $width } × { $height } mm
plot-paper-size = Formato carta
plot-pick-interval-reads-roughly-every = Scegli un intervallo che si legga circa ogni 50 mm sul foglio stampato.
plot-plan = Planimetria
plot-scale-must-be-positive = La scala di stampa deve essere un numero positivo
plot-png-written-sheet-s-exact = Il PNG viene scritto nel formato carta esatto del foglio e registra i suoi dpi, quindi stampa in scala reale.
plot-portrait = Verticale
plot-resolution = Risoluzione
plot-rev = REV
plot-revision = Revisione
plot-scale = SCALA
plot-scale-ratio = Scala  1:
plot-scale-framing = Scala e inquadratura
plot-sheet-furniture = Elementi del foglio
plot-size-width-height-mm = { $size } ({ $width } × { $height } mm)
plot-subtitle = Sottotitolo
plot-title = Titolo
plot-title-block = Cartiglio
plot-today = oggi
point-cloud-classify = Classifica
point-cloud-classify-vegetation = Classifica vegetazione
point-cloud-cloth-resolution = Risoluzione del tessuto
point-cloud-cloth-resolution-about-one-half = Una risoluzione del tessuto pari a circa una volta e mezza la spaziatura dei punti della nuvola selezionata più rada, così che ogni particella abbia ritorni sotto di sé.
point-cloud-combine-selected-point-clouds-into = Combina le nuvole di punti selezionate in un'unica nuova nuvola, così da poter costruire una sola triangolazione su tutte. I colori per punto vengono mantenuti; una nuvola che ne è priva contribuisce con il proprio colore di visualizzazione.
point-cloud-selected-count = { $count } selezionate · { $points } punti
point-cloud-delete-selected-clouds-from-project = Elimina le nuvole selezionate dal progetto al termine dell'unione, liberando la memoria che la loro copia duplicata occuperebbe altrimenti.
point-cloud-flat-pads-structures = Piano (piazzali, strutture)
point-cloud-ground-cloud-covers-steep-follows = Il terreno coperto dalla nuvola. Ripido segue le pareti dalle loro sommità verso il basso; Piano usa un tessuto più rigido che scavalca grandi edifici e impianti ma arrotonda le rotture nette.
point-cloud-ground-threshold = Soglia del terreno
point-cloud-how-far-around-each-point = A che distanza attorno a ciascun punto contare i vicini.
point-cloud-join = Unisci
point-cloud-let-cloth-follow-walls-down = Lascia che il tessuto segua le pareti dalle loro sommità verso il basso, dove altrimenti la sua rigidità lo terrebbe staccato dalla faccia. Disattiva solo su terreno dolce affollato di impianti.
point-cloud-mark-each-point-ground-noise = Contrassegna ogni punto come terreno, rumore o non classificato. Un tessuto viene premuto sotto la nuvola e si adagia sulla superficie del terreno; i punti entro la soglia del terreno da esso sono terreno. Le classi esistenti vengono sostituite; l'annullamento le ripristina.
point-cloud-mark-isolated-returns-birds-dust = Contrassegna come rumore i ritorni isolati (uccelli, polvere, errori di multipath) prima di individuare il terreno, così che un punto basso isolato non possa trascinare in basso il tessuto.
point-cloud-mark-noise = Contrassegna rumore
point-cloud-minimum-neighbours = Vicini minimi
point-cloud-name-assigned-joined-point-cloud = Nome assegnato alla nuvola di punti unita.
point-cloud-name-count-points = { $name } ({ $count } punti)
point-cloud-noise-radius = Raggio del rumore
point-cloud-point-clouds = Nuvole di punti
point-cloud-points-closer-than-settled-cloth = I punti più vicini di questa distanza al tessuto adagiato, misurata lungo la sua superficie, sono terreno.
point-cloud-points-fewer-neighbours-than-within = I punti con meno vicini di questo numero entro il raggio del rumore sono rumore.
point-cloud-raise-cloth-resolution-if-your = Aumenta la risoluzione del tessuto se la tua macchina ha meno RAM.
point-cloud-recommended = Consigliato
point-cloud-recover-steep-slopes = Recupera pendii ripidi
point-cloud-relief-dumps-rolling-ground = Rilievo (discariche, terreno ondulato)
point-cloud-remove-sources = Rimuovi sorgenti
point-cloud-resolution-m-points-spacing-m = { $resolution } m (punti a ~{ $spacing } m di distanza)
point-cloud-selected-clouds-copied-into-joined = Le nuvole selezionate, copiate nella nuvola unita. Chiudi la finestra per unire un insieme diverso.
point-cloud-selected-clouds-each-classified-its = Le nuvole selezionate, ciascuna classificata separatamente. Chiudi la finestra per classificare un insieme diverso.
point-cloud-classify-help = Ordina i ritorni con un classificatore addestrato che legge la forma dei punti intorno a ciascuno: terreno, vegetazione (suddivisa per altezza in bassa, sotto 1 m, media, sotto 3 m, o alta) e tutto il resto, come edifici e impianti, lasciato non classificato. Disattiva per usare solo il tessuto.
point-cloud-spacing-cloth-s-particles-around = Spaziatura delle particelle del tessuto. Una spaziatura simile a quella dei punti della nuvola è un buon inizio; più fine segue meglio il terreno ma richiede punti più densi.
point-cloud-steep-pit-walls-benches = Ripido (pareti di fossa, gradoni)
point-cloud-terrain = Terreno
point-cloud-use = Usa

## Products strings

products-add-initiation = Aggiungi innesco
products-delay = Ritardo
products-delay-palette = Tavolozza dei ritardi
products-how-long-after-shot-fired = Quanto tempo dopo lo sparo questa bocca foro innesca la volata.
products-initiation-name = Innesco · { $name }
products-milliseconds-between-one-hole-firing = Millisecondi tra l'accensione di un foro e il successivo.
products-ms = ms
products-no-products = Nessun prodotto
products-remove = Rimuovi
products-update = Aggiorna

## Progress strings

progress-percent-done-total = { $percent } ({ $done } di { $total })
progress-task-finished = { $task }: completato

## Project strings

project-item = Elemento

## Properties strings

properties-adds-view-dependent-rim-highlight = Aggiunge un'evidenziazione del bordo dipendente dalla vista ai confini di blocchi e materiali. Disattivarla riduce leggermente il lavoro di rendering volumetrico.
properties-block-model-downscale = Riduzione scala modello a blocchi
properties-camera = Camera
properties-camera-clip-planes = Piani di clip della camera
properties-cap-while-resizing = Limita durante il ridimensionamento
properties-colours-each-point-cloud-chunk = Colora ogni chunk della nuvola di punti, ne evidenzia il riquadro usato per l'eliminazione dal frustum e mostra nella barra di stato i punti disegnati nell'ultimo fotogramma rispetto all'obiettivo del livello di dettaglio e al totale visibile.
properties-colours-each-surface-chunk-outlines = Colora ogni chunk della superficie, ne evidenzia il riquadro usato per l'eliminazione dal frustum e mostra nella barra di stato le facce disegnate nell'ultimo fotogramma rispetto al totale visibile.
properties-dark-mode = Modalità scura
properties-dataset = Dataset
properties-developer = Sviluppatore
properties-downscale-rasters = Riduci scala raster
properties-drillholes = Fori di sondaggio
properties-edit-object = Modifica oggetto...
properties-field-view = Campo visivo
properties-fps = FPS
properties-frame-counter = Contatore fotogrammi
properties-frame-rate-cap = Limite fotogrammi al secondo
properties-hz = Hz
properties-interface = Interfaccia
properties-invert-horizontal = Inverti orizzontale
properties-invert-vertical = Inverti verticale
properties-limits-newly-loaded-geotiff-previews = Limita le anteprime dei GeoTIFF appena caricati a 4096 pixel sul lato più lungo. Disattiva per usare la piena risoluzione fino al limite di texture della GPU, con un maggiore uso di memoria.
properties-line-colour = Colore linea
properties-look-sensitivity = Sensibilità di sguardo
properties-max-clip-span = Intervallo massimo di clip
properties-move-layer = Sposta al livello...
properties-near-clip-limit = Limite di clip vicino
properties-no-drillhole-datasets-open = Nessun dataset di fori di sondaggio aperto.
properties-orbit-sensitivity = Sensibilità di orbita
properties-panel-chrome = Cornice pannelli
properties-performance = Prestazioni
properties-plan-mode = Modalità planimetria
properties-point-cloud-chunk-debug-view = Vista di debug dei chunk della nuvola di punti
properties-presents-step-display-no-tearing = Si presenta in sincronia con lo schermo: nessun tearing, e lo schermo determina il frame rate. Se disattivato, i fotogrammi vengono presentati non appena disegnati e si applica il limite indicato sotto.
properties-reflective-block-edges = Bordi dei blocchi riflettenti
properties-restore-defaults = Ripristina predefiniti
properties-show-console = Mostra console
properties-shows-live-near-far-projection = Mostra le distanze di proiezione vicina e lontana in tempo reale nella barra di stato.
properties-snap-polling = Polling dello snap
properties-surface-chunk-debug-view = Vista di debug dei chunk della superficie
properties-vertical-sync = Sincronizzazione verticale
properties-world-axis-gizmo = Gizmo assi del mondo
properties-zoom-cursor = Zoom sul cursore
properties-zoom-sensitivity = Sensibilità zoom
reference-points-count-holes-from-dataset = { $count } fori da '{ $dataset }'
reference-points-holes-from-datasets = { $count } fori da { $datasets } dataset
reference-points-holes = Fori
reference-points-holes-points-placed-selected-when = I fori su cui vengono posizionati i punti, come selezionati all'apertura della finestra. Chiudi la finestra per selezionarne altri.
reference-points-make = Crea
reference-points-no-categorical-field = Nessun campo categorico
reference-points-no-values = Nessun valore
reference-points-one-point-per-hole-boundary = Un punto per foro a quel limite, come nuovo livello. Un foro che contiene la sezione due volte dà la più alta e viene segnalato.
reference-points-reference-points = Punti di riferimento
reference-points-side = Lato
reference-points-working-section = Sezione di lavoro
reference-points-working-section-field = Campo della sezione di lavoro
reference-surface-controls = Controlli
reference-surface-extent = Estensione
reference-surface-points-outside-extent-still-shape = I punti fuori dall'estensione modellano comunque la superficie; solo la superficie viene ritagliata su di essa.
reference-surface-points-surface-built-from-selected = I punti da cui è costruita la superficie, come selezionati all'apertura della finestra. Chiudi la finestra per selezionarne altri.
reference-surface-extent-help = La linea chiusa selezionata a cui viene ritagliata la superficie finita; i punti al di fuori modellano comunque la superficie.
reference-surface-selected-open-strings-surface-made = Le linee aperte selezionate attraverso cui passa la superficie, come selezionate all'apertura della finestra. Chiudi la finestra per selezionarne altre.
reference-surface-triangulates-selected-points-plan-in = Triangola in pianta i punti selezionati in una nuova superficie. Ogni costruzione aggiunge una superficie.

## Screenshot strings

screenshot-could-not-encode-viewport-image = Impossibile codificare l'immagine della vista: { $error }
screenshot-could-not-map-viewport-screenshot = Impossibile mappare lo screenshot della vista: { $error }
screenshot-could-not-save-viewport-image = Impossibile salvare l'immagine della vista { $path }: { $error }
screenshot-downloaded-viewport-image-file-name = Scaricata immagine della vista: { $file_name }
screenshot-saved-viewport-image-path = Immagine della vista salvata: { $path }
screenshot-viewport-image-download-failed-error = Download dell'immagine della vista non riuscito: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = Indice faccia BVH { $index } fuori intervallo per la mesh; sostituito con triangolo degenere

## State strings

state-above = a partire da
state-activate-project = Attiva progetto
state-all-open-incline-design-data = Tutti i dati aperti di Incline Design
state-apply-generated-rings = Applica anelli generati
state-apply-selection = Applica alla selezione
state-rotate-by-azimuth-dip = da azimut { $azimuth }°, inclinazione { $dip }°
state-rotate-to-azimuth-dip = ad azimut { $azimuth }°, inclinazione { $dip }°
state-below = fino a
state-build-reference-points = Costruisci punti di riferimento
state-centre-rotation = Centro di rotazione
state-checking-unsaved-work = Verifica delle modifiche non salvate
state-choose-destination = Scegli una destinazione
state-choose-one-more-files = Scegli uno o più file
state-clear-raster = Cancella raster
state-click-pit-shell-viewport = Fai clic sul guscio della fossa nella vista.
state-click-pit-stockpile-solid-viewport = Fai clic sul solido della fossa o del cumulo nella vista.
state-click-surface-viewport = Fai clic sulla superficie nella vista.
state-click-topology-viewport = Fai clic sulla topologia nella vista.
state-close-project = Chiudi progetto
state-colour-drillholes = Colora fori di sondaggio
state-colour-drillholes-working-section = Colora fori di sondaggio per sezione di lavoro
state-colour-points-classification = Colora punti per classificazione
state-copy-objects-layer = Copia oggetti nel livello
state-count-cloud-s = { $count } nuvola/e
state-count-file-s = { $count } file
state-count-object-s-axis-value = { $count } oggetto/i · { $axis } { $value }
state-count-object-s-closed = { $count } oggetto/i · { $closed }
state-count-object-s-layer = { $count } oggetto/i · { $layer }
state-count-object-s-weight = { $count } oggetto/i · { $weight }
state-count-object-s-z-elevation = { $count } oggetto/i · Z { $elevation }
state-points-controls-clipped = { $count } punto/i · { $controls } linea/e di controllo · ritagliata sulla linea di estensione
state-points-controls-unclipped = { $count } punto/i · { $controls } linea/e di controllo · non ritagliata
state-create-collection = Crea raccolta
state-create-point-cloud-tin = Crea TIN da nuvola di punti
state-create-project = Crea progetto
state-current-project = Progetto corrente
state-cut-topology-pit-shell = Taglia topologia sul guscio della fossa
state-cut-triangulation-polyline = Taglia triangolazione con polilinea
state-cut-triangulation-z = Taglia triangolazione per Z
state-dark-mode = Modalità scura
state-data-ticked-export-checklist = I dati spuntati nell'elenco di esportazione
state-detached = Staccato
state-disabled = Disattivato
state-discard-project-changes = Scarta modifiche al progetto
state-discard-replace-project = Scarta e sostituisci progetto
state-discarding-unsaved-changes = Eliminazione delle modifiche non salvate
state-docked = Ancorato
state-drape-raster = Adagia raster
state-drill-pattern = Schema di perforazione
state-duplicate-layer = Duplica livello
state-east = Est
state-enabled = Attivato
state-exit-incline-design = Esci da Incline Design
state-export-block-model-csv = Esporta CSV modello a blocchi
state-export-drillhole-csv = Esporta CSV fori di sondaggio
state-export-layer-dxf = Esporta livello in DXF
state-export-omf = Esporta OMF
state-export-project-dxf = Esporta progetto in DXF
state-export-triangulation = Esporta triangolazione
state-export-viewport-image = Esporta immagine della vista
state-finish-closed-polyline = Termina polilinea chiusa
state-finish-open-polyline = Termina polilinea aperta
state-fit-extents = Adatta all'estensione
state-plan-view-then-fit-extents = Vista in pianta alla stessa distanza, poi adatta all'estensione
state-fix-release-centre-both-views = Fissa o rilascia il centro attorno a cui orbitano entrambe le viste
state-folder-section = { $folder } in { $section }
state-generate-contours = Genera curve di livello
state-hidden = Nascosto
state-import-drillholes = Importa fori di sondaggio
state-import-omf = Importa OMF
state-import-point-cloud = Importa nuvola di punti
state-import-raster = Importa raster
state-import-triangulation = Importa triangolazione
state-insert-intersection-points = Inserisci punti di intersezione
state-insert-points-elevation = Inserisci punti a quota
state-keep-inside = Mantieni interno
state-keep-outside = Mantieni esterno
state-kriged-block-model = Modello a blocchi krigato
state-load-block-model = Carica modello a blocchi
state-load-drillholes = Carica fori di sondaggio
state-load-layer = Carica livello
state-load-point-cloud = Carica nuvola di punti
state-load-raster = Carica raster
state-load-triangulation = Carica triangolazione
state-locked-count-object-s = Bloccato/i { $count } oggetto/i
state-major-minor = Principale { $major } · secondario { $minor }
state-member-into-folder-section = { $member } in { $folder } in { $section }
state-member-root-section = { $member } nella radice di { $section }
state-move-axis-value = Sposta al valore dell'asse
state-move-objects-layer = Sposta oggetti nel livello
state-name-count-cloud-s = { $name } · { $count } nuvola/e
state-name-count-holes = { $name } · { $count } fori
state-name-count-object-s = { $name } · { $count } oggetto/i
state-name-z-min-z-max = { $name } · da { $z_min } a { $z_max }
state-new-collection-under-section = Nuova raccolta in { $section }
state-next-edit = Modifica successiva
state-north = Nord
state-off = Disattivato
state-on = Attivato
state-open-containing-folder = Apri la cartella contenitore
state-open-project = Apri progetto
state-preserve-view-angle = Mantieni angolazione vista
state-previous-edit = Modifica precedente
state-project-id = Progetto { $id }
state-remove-block-model = Rimuovi modello a blocchi
state-remove-drillholes = Rimuovi fori di sondaggio
state-remove-point-cloud = Rimuovi nuvola di punti
state-remove-raster = Rimuovi raster
state-remove-triangulation = Rimuovi triangolazione
state-removed-from-active-triangulation = Rimosso dalla triangolazione attiva
state-removed-from-every-triangulation = Rimosso da ogni triangolazione
state-rename-kind = Rinomina { $kind }
state-save-close-project = Salva e chiudi progetto
state-save-despite-unsupported-content = Salva nonostante il contenuto non supportato
state-save-project = Salva progetto con nome
state-save-replace-project = Salva e sostituisci progetto
state-saving-current-project = Salvataggio del progetto corrente
state-section-name = sezione { $section }
state-select-layer-objects = Seleziona oggetti del livello
state-selected-objects = Oggetti selezionati
state-selected-polylines = Polilinee selezionate
state-selected-scene-elements = Elementi di scena selezionati
state-set-block-model-variable = Imposta variabile modello a blocchi
state-set-cinematic-view = Imposta vista cinematica
state-set-drillhole-colour-preset = Imposta preimpostazione colore fori di sondaggio
state-set-drillhole-discs = Imposta dischi dei fori di sondaggio
state-set-drillhole-style = Imposta stile dei fori di sondaggio
state-set-drillhole-width = Imposta larghezza dei fori di sondaggio
state-set-entity-lock = Imposta blocco entità
state-set-grid = Imposta griglia
state-set-layer-lock = Imposta blocco livello
state-set-line-weight = Imposta spessore linea
state-set-object-colour = Imposta colore oggetto
state-set-object-fill = Imposta riempimento oggetto
state-set-point-visibility = Imposta visibilità punto
state-set-polyline-closed = Imposta polilinea chiusa
state-set-raster-lock = Imposta blocco raster
state-set-standard-view = Imposta vista standard
state-set-topology-wireframes = Imposta wireframe della topologia
state-set-triangulation-colour = Imposta colore triangolazione
state-show-console = Mostra console
state-show-project = Mostra progetto
state-shown = Mostrato
state-slice-mode = Modalità sezione
state-slice-preview = Anteprima sezione
state-south = Sud
state-stem-contours = Curve di livello di { $stem }
state-target-new-name = { $target } in “{ $new_name }”
state-trim-above = Rifila sopra
state-trim-below = Rifila sotto
state-trim-triangulation-surface = Rifila triangolazione sulla superficie
state-undrape-raster = Rimuovi adagiamento raster
state-undrape-rasters = Rimuovi adagiamento raster
state-unload-block-model = Scarica modello a blocchi
state-unload-drillholes = Scarica fori di sondaggio
state-unload-layer = Scarica livello
state-unload-point-cloud = Scarica nuvola di punti
state-unload-raster = Scarica raster
state-unload-triangulation = Scarica triangolazione
state-untitled-project = Progetto senza titolo
state-use-typed-radius = Usa raggio digitato
state-west = Ovest

## Status strings

status-clip-near-far = Clip vicino/lontano/Δ: -- / -- / --
status-faces-chunks = Facce: -- / -- (--/-- chunk)
status-frame-rate = Frequenza fotogrammi
status-points-chunks = Punti: -- / -- di -- (--/-- chunk)

## Text strings

text-could-not-build-vector-mesh = Impossibile costruire la mesh vettoriale per il font { $font }, glifo { $glyph }: { $error }
text-document-text-mesh-exceeded-its = La mesh di testo del documento ha superato il proprio intervallo di indici u32

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = Scegli prima il set di fori di sondaggio da collegare
tie-in-count-connector-s = { $count } connettore/i
tie-in-delete-tie-ins = Elimina collegamenti
tie-in-deleted-count-selected-tie-connector = Eliminato/i { $count } connettore/i di collegamento selezionato/i
tie-in-hole = foro
tie-in-initiation-point-lifted-from-name = Punto di innesco rimosso da { $name }
tie-in-initiation-point-set-name-delay = Punto di innesco impostato su { $name } a { $delay } ms
tie-in-select-delay-product-palette-before = Seleziona un prodotto di ritardo nella tavolozza prima di collegare i fori
tie-in-tied-connectors = Collegato/i { $count } connettore/i a { $delay } ms con { $product }
tie-in-tied-connectors-replacing = Collegato/i { $count } connettore/i a { $delay } ms con { $product }, sostituendo { $replaced }

## Toolbar strings

toolbar-fill-type = Tipo di riempimento

## Toolbars strings

toolbars-auto-bench = Gradino automatico
toolbars-bezier-polyline = Polilinea di Bézier
toolbars-chamfer-polyline-corners = Smussa angoli della polilinea
toolbars-create-text = Crea testo
toolbars-cursor-regular = Cursore: normale
toolbars-cursor-snap-line = Cursore: aggancio a linea
toolbars-cursor-snap-point = Cursore: aggancio a punto
toolbars-cursor-snap-surface = Cursore: aggancio a superficie
toolbars-delete-points = Elimina punti
toolbars-explode-polyline-lines = Esplodi polilinea in linee
toolbars-fuse-polylines = Fondi polilinee
toolbars-measure-distance = Misura distanza
toolbars-new-layer = Nuovo livello
toolbars-split-polyline-points = Dividi polilinea ai punti
toolbars-strike-dip = Direzione e inclinazione
toolbars-tool-not-available-section-view = { $tool } - non disponibile nella vista in sezione

## Tri strings

tri-sampling-method-help = Adattivo concentra i vertici sul terreno complesso in base all'errore di adattamento del piano; uniforme li distribuisce in modo omogeneo. In futuro potranno essere aggiunti altri metodi.
tri-adaptive-quadtree = Adattivo (quadtree)
tri-axis-range = Intervallo { $axis }
tri-base-topology-will-receive-pit = La topologia di base che riceverà la forma della fossa o del cumulo.
tri-boundary-polyline = Polilinea di confine
tri-bridge-gaps-help = Colma i vuoti e le concavità del contorno più stretti di questo valore sull'intera superficie. 0 colma comunque i vuoti fino a circa la dimensione della cella di campionamento; valori maggiori riempiono buchi più grandi ed erodono le concavità del contorno.
tri-budget = Budget per
tri-cancel-pick = Annulla selezione
tri-candidate-detail = Dettaglio candidati
tri-candidate-fine-cells-per-budgeted = Celle fini candidate per vertice previsto. Un valore più alto dà al campionatore adattivo più libertà di posizionare i dettagli, ma è più lento da costruire.
tri-cap-surface-share-source-points = Limita la superficie a una quota dei punti sorgente o a un numero esatto di vertici.
tri-choose-input-clicking-loaded-surface = Scegli questo input facendo clic su una superficie caricata nella vista
tri-choose-which-side-reference-topology = Scegli quale lato della topologia di riferimento rimuovere dalla superficie, nell'area XY condivisa.
tri-clip = Clip
tri-clip-creates-new-triangulation-name = Il ritaglio crea una nuova triangolazione con questo nome; la superficie sorgente non viene modificata.
tri-clip-surface-polyline = Ritaglia superficie con polilinea
tri-closed-pit-stockpile-solid-whose = Un solido chiuso di fossa o cumulo il cui contorno esposto verrà incluso nel risultato.
tri-cloud-carries-no-classifications-so = Questa nuvola non ha classificazioni, quindi ogni punto viene usato per la superficie. Importa un file LAS/LAZ passato attraverso un filtro del terreno per ricostruire il terreno nudo.
tri-create-new-layer-contours-append = Crea un nuovo livello per le curve di livello oppure aggiungile a un livello esistente nel progetto attivo.
tri-cut-topology-pit-shell = Taglia topologia con guscio della fossa
tri-e-g-design-trimmed = es. design_rifilato
tri-e-g-mysurf-cut = es. miasuperficie_tagliata
tri-e-g-mysurf-slice = es. miasuperficie_sezione
tri-e-g-surface-contour = es. superficie_curve
tri-e-g-topo-cut = es. topo_tagliata
tri-e-g-topo-pit = es. topo_con_fossa
tri-exact-number-surface-vertices-target = Numero esatto di vertici della superficie da raggiungere. Valori molto elevati richiedono più tempo di costruzione e memoria significativa.
tri-existing-ground-topology-will-cut = La topologia del terreno esistente che verrà tagliata dal guscio della fossa.
tri-fill-holes-up = Riempi i fori fino a
tri-generate = Genera
tri-generate-contour-lines = Genera curve di livello
tri-generate-upper-surface = Genera superficie superiore
tri-ground-points-only = Solo punti di terreno
tri-hide-unload-sources = Nascondi e scarica le sorgenti
tri-higher-edge-will-enforced-each = Il lato più alto verrà applicato a ogni conflitto. I segmenti in conflitto più bassi verranno ignorati come linee di rottura e la superficie interpolerà in quelle aree. Le polilinee sorgente non vengono modificate.
tri-breaklines-cross = I lati di linea di rottura evidenziati si incrociano o si sovrappongono in pianta a quote diverse. Una superficie del terreno non può seguirli entrambi.
tri-intervals-colours = Intervalli e colori
tri-keep-clipped-topology-included-shape = Mantieni la topologia ritagliata e la forma inclusa come triangolazioni separate invece di unirle in un'unica entità.
tri-keep-inside-discards-surface-outside = Mantieni interno scarta la superficie fuori dalla polilinea. Mantieni esterno crea un foro a forma di polilinea nella superficie.
tri-keeps-only-surface-within-polyline = Mantiene solo la superficie all'interno del contorno della polilinea.
tri-keep-surface-relation-help = Mantiene la superficie { $relation } la topologia entro la sua copertura XY.
tri-layer-already-exists-select-above = Quel livello esiste già; selezionalo sopra o scegli un altro nome.
tri-limit-z-range = Limita intervallo Z
tri-major = Principale
tri-max-edge-length = Lunghezza massima del lato
tri-merge = Unisci
tri-method = Metodo
tri-min = Min
tri-minimum-maximum-elevations-retained = Quote minima e massima mantenute nella superficie di output. Il minimo deve essere inferiore al massimo.
tri-minor = Secondario
tri-contour-interval-help = Secondario controlla le curve di livello ordinarie. Principale controlla le curve di livello enfatizzate e deve usare un intervallo almeno pari a Secondario.
tri-move-cursor-over-loaded-surface = Sposta il cursore sopra una superficie caricata.
tri-slice-output-name-help = Nome assegnato alla superficie di output ritagliata per quota.
tri-name-assigned-merged-topology-pit = Nome assegnato al risultato dell'unione tra topologia e fossa/cumulo.
tri-name-assigned-newly-created-contour = Nome assegnato al livello di curve di livello appena creato.
tri-reconstruct-output-name-help = Nome assegnato alla triangolazione ricostruita.
tri-name-assigned-topology-after-pit = Nome assegnato alla topologia dopo che il guscio della fossa ne è stato tagliato.
tri-name-assigned-trimmed-output-surface = Nome assegnato alla superficie di output rifilata.
tri-nearby-breakline-vertices-do-not = I vertici di linea di rottura vicini non coincidono esattamente nella stessa posizione, quindi la superficie non può essere triangolata.
tri-new-layer = Nuovo livello
tri-new-layer-name = Nome nuovo livello
tri-no-boundary-selected = Nessun contorno selezionato
tri-no-point-cloud-selected = Nessuna nuvola di punti selezionata
tri-no-surface-selected = Nessuna superficie selezionata
tri-once-clip-succeeds-unload-source = Dopo il successo del ritaglio, scarica la superficie sorgente così che nella scena resti solo il risultato ritagliato.
tri-once-cut-succeeds-unload-original = Dopo il successo del taglio, scarica la topologia originale così che nella scena resti solo il risultato del taglio. Il guscio della fossa resta caricato.
tri-once-merge-succeeds-unload-source = Una volta riuscita l'unione, scarica la topologia sorgente e il solido in modo che rimanga in scena solo il risultato unito.
tri-once-slice-succeeds-unload-source = Dopo il successo della sezione, scarica la superficie sorgente così che nella scena resti solo il risultato sezionato.
tri-once-trim-succeeds-unload-surface = Dopo il successo della rifilatura, scarica la superficie rifilata così che nella scena resti solo il risultato. La topologia resta caricata.
tri-only-loaded-pickable = È possibile selezionare solo triangolazioni caricate.
tri-operation = Operazione
tri-output-layer = Livello di output
tri-percentage = Percentuale
tri-percentage-cloud = Percentuale della nuvola
tri-pick-from-view = Seleziona dalla vista
tri-pit-design-surface-only-areas = La superficie di progetto della fossa. Solo le aree in cui scava sotto la topologia vengono usate per il taglio.
tri-pit-shell = Guscio della fossa
tri-pit-stockpile-solid = Solido fossa/cumulo
tri-recommended-weld-retry = Consigliato: Salda e riprova
tri-reconstruct-ground-only-help = Ricostruisci dai punti classificati come terreno nudo, scartando vegetazione, edifici, impianti e rumore. Disattiva per usare ogni punto della nuvola.
tri-reconstruct-help = Ricostruisce una superficie del terreno triangolata da una nuvola di punti. Il campionatore adattivo spende il budget di vertici dove il terreno è più complesso e mantiene sparse le aree planari.
tri-reduce-budget-candidate-detail-if = Riduci il budget o il dettaglio candidati se il tuo computer ha meno RAM.
tri-reference-topology-help = La topologia di riferimento che definisce dove viene rifilata l'altra superficie.
tri-reject-reconstructed-triangle-edges = Rifiuta i lati dei triangoli ricostruiti più lunghi di questa distanza. Usa 0 per nessun limite di lunghezza del lato.
tri-remove-inside-help = Rimuove la superficie all'interno del contorno della polilinea e mantiene il resto.
tri-removes-topology-where-pit-shell = Rimuove la topologia dove il guscio della fossa scava sotto di essa, in modo che il guscio riempia il vuoto. La giunzione segue la reale linea di contatto 3D tra le superfici; la topologia sotto le parti del guscio che stanno sopra il terreno viene mantenuta.
tri-result = Risultato
tri-save-two-entities = Salva come due entità
tri-select = Seleziona…
tri-selected-closed-polyline-whose-xy = La polilinea chiusa selezionata, il cui contorno XY definisce l'area di ritaglio.
tri-selected-point-cloud-whose-points = La nuvola di punti selezionata, i cui punti verranno ricostruiti in una superficie del terreno. Chiudi la finestra per ricostruirne un'altra.
tri-selected-surface-from-which-contour = La superficie selezionata, da cui verranno generate le curve di livello. Chiudi la finestra per generarle da un'altra.
tri-selected-surface-which-will-clipped = La superficie selezionata, che verrà ritagliata. Chiudi la finestra per ritagliarne un'altra.
tri-slice-source-help = La superficie selezionata, il cui intervallo di quota verrà ritagliato. Chiudi la finestra per sezionarne un'altra.
tri-share-source-points-keep-fractions = Quota di punti sorgente da mantenere. Sono ammesse frazioni come 0,125%.
tri-slice-triangulation-z-range = Seziona triangolazione per intervallo Z
tri-solution-generate-upper-surface = Soluzione: Genera superficie superiore
tri-surface-trim = Superficie da rifilare
tri-target-surface-help = La superficie che verrà modificata; la topologia selezionata resta intatta.
common-percent-suffix = %
tri-topology = Topologia
tri-triangulation-failed = Triangolazione non riuscita
tri-trim = Rifila
tri-trim-topology = Rifila sulla topologia
tri-uniform-grid = Griglia uniforme
tri-unload-source-surface = Scarica superficie sorgente
tri-unload-source-topology = Scarica topologia sorgente
tri-up-target-point-count-points = Fino a { $target } di { $point_count } punti diventeranno vertici della superficie ({ $percent }%).
tri-use-full-surface-elevation-range = Usa l'intero intervallo di quota della superficie
tri-vertex-count = Numero di vertici
tri-vertices-within-5-cm-xy = I vertici entro 5 cm in XY e Z condivideranno un'unica posizione per questa triangolazione. Questo può spostare localmente la superficie generata fino a 5 cm; le polilinee sorgente non vengono modificate.
tri-weld-retry = Salda e riprova
tri-when-enabled-generate-contours-only = Se attivato, genera curve di livello solo tra le quote minima e massima specificate.

## Ui strings

ui-choose-offset-side = Scegli il lato dell'offset
ui-choose-relimit-side = Scegli il lato di ridelimitazione
ui-click-circle-centre = Fai clic sul centro del cerchio
ui-click-closed-polyline-use-blast = Fai clic su una polilinea chiusa da usare come forma della volata
ui-click-collar-add-edit-initiation = Fai clic su una bocca foro per aggiungere o modificare un punto di innesco
ui-click-first-point-slice-line = Fai clic sul primo punto della linea di sezione
ui-click-first-vertex = Fai clic sul primo vertice
ui-click-perimeter-point-type-radius = Fai clic su un punto del perimetro o digita un raggio
ui-click-second-point-slice-line = Fai clic sul secondo punto della linea di sezione
ui-click-second-vertex = Fai clic sul secondo vertice
ui-click-use-pointer-radius = oppure fai clic per usare il raggio del puntatore
ui-could-not-copy-text-browser = Impossibile copiare il testo negli appunti del browser: { $error }
ui-dip-horizontal-no-strike = { $dip } (orizzontale, nessuna direzione)
ui-distance-meters = { $distance } metri
ui-drag-ring-type-azimuth-dip = Trascina un anello oppure digita un azimut e un'inclinazione
ui-each-hole-turns-about-its = ogni foro ruota attorno alla propria bocca
ui-enter-positive-decimal-radius = Inserisci un raggio decimale positivo
ui-esc-cancels = Esc annulla
ui-no-delay-product-tie = Nessun prodotto di ritardo da collegare
ui-press-enter-use-typed-radius = Premi Invio per usare il raggio digitato
ui-right-click-delay-palette-heading = fai clic destro sull'intestazione della tavolozza dei ritardi per aggiungerne uno
ui-select-designs = Seleziona progettazioni
ui-select-drill-hole = Seleziona un foro di sondaggio
ui-select-endpoint-join = Seleziona l'estremità da unire
ui-select-first-crest-toe-point = Seleziona il primo punto di cresta/piede
ui-select-item = Seleziona un elemento
ui-select-line-fuse = Seleziona una linea da fondere
ui-select-line-polyline = Seleziona una linea o polilinea
ui-select-line-relimit = Seleziona linea da ridelimitare
ui-select-next-line-fuse = Seleziona la linea successiva da fondere
ui-select-opposite-berm-point = Seleziona il punto opposto della berma
ui-select-point = Seleziona un punto
ui-select-polyline = Seleziona una polilinea
ui-select-polyline-open-line = Seleziona una polilinea o una linea aperta
ui-select-polyline-vertex = Seleziona un vertice della polilinea
ui-select-second-crest-toe-point = Seleziona il secondo punto di cresta/piede
ui-select-second-split-point = Seleziona il secondo punto di divisione
ui-select-split-point = Seleziona un punto di divisione
ui-select-topologies = Seleziona topologie
ui-slice-view = Vista sezione
ui-strike-dip = { $strike }° direzione · { $dip }
ui-value-dip = { $value }° inclinazione
viewport-1-1-true-shape = 1:1, forma reale
viewport-1-ratio = 1:{ $ratio }

## Viewport strings

viewport-all-total-categories-keep-their = Tutte le { $total } categorie mantengono il proprio colore; solo le prime { $shown } vengono disegnate in modo distinto
viewport-axis-maximum = massimo { $axis }
viewport-axis-minimum = minimo { $axis }
viewport-azimuth-dip = Azimut { $azimuth }, inclinazione { $dip }
viewport-back-whole-log = Torna all'intero log.
viewport-bar-blast-timeline-placeholder = Sequenza di volata [PLACEHOLDER]
viewport-bar-burden-relief-heatmap-placeholder = Mappa termica di sfogo della resistenza [PLACEHOLDER]
viewport-bar-cinematic-view = Vista cinematica
viewport-bar-color = Colore:
viewport-bar-contours-equal-time-placeholder = Curve di isotempo [PLACEHOLDER]
viewport-bar-disable-cinematic-view = Disattiva vista cinematica
viewport-bar-disable-flying-mode = Disattiva modalità volo
viewport-bar-disable-x-ray-vision = Disattiva visione a raggi X
viewport-bar-drill-holes = Fori di sondaggio:
viewport-bar-enable-flying-mode = Attiva modalità volo
viewport-bar-enable-x-ray-vision = Attiva visione a raggi X
viewport-bar-exit-slice-view = Esci dalla vista sezione
viewport-bar-fill = Riempimento:
viewport-bar-fix-centre-rotation = Fissa centro di rotazione
viewport-bar-hide-borehole-inspector = Nascondi ispettore fori di sondaggio
viewport-bar-hide-classification = Nascondi classificazione
viewport-bar-hide-points = Nascondi punti
viewport-bar-hide-rl-grid = Nascondi griglia quote
viewport-bar-hide-wireframes = Nascondi wireframe
viewport-bar-hide-xy-grid = Nascondi griglia XY
viewport-bar-release-centre-rotation = Rilascia centro di rotazione
viewport-bar-reset-view-plan-over-centre = Ripristina vista: pianta sopra il centro di rotazione, fai clic di nuovo per adattare tutto
viewport-bar-reset-view-plan-same-distance = Ripristina vista: pianta alla stessa distanza, fai clic di nuovo per adattare tutto
viewport-bar-show-borehole-inspector = Mostra ispettore fori di sondaggio
viewport-bar-show-classification = Mostra classificazione
viewport-bar-show-points = Mostra punti
viewport-bar-show-rl-grid = Mostra griglia quote
viewport-bar-show-wireframes = Mostra wireframe
viewport-bar-show-xy-grid = Mostra griglia XY
viewport-bar-vertical-slice-view = Vista sezione verticale
viewport-blank = (vuoto)
viewport-choose-active-block-model-variable = Scegli la variabile attiva del modello a blocchi
viewport-choose-variable = Scegli una variabile
viewport-click-edit-color-right-click = Fai clic per modificare il colore; clic destro per rimuovere
viewport-click-type-boundary-s-value = Fai clic per digitare il valore di questo confine
viewport-colour-mapping = Mappatura colori
viewport-count-categories = { $count } categorie
viewport-count-category = { $count } categoria
viewport-depth-m-hole-end = { $depth } m fondo foro
viewport-double-click-add-boundary-here = Doppio clic per aggiungere qui un confine
viewport-drag-move-middle-click-toggles = Trascina per spostare · Clic centrale attiva/disattiva ≤
viewport-drag-move-right-click-remove = Trascina per spostare · Clic destro per rimuovere · Clic centrale attiva/disattiva ≤
viewport-drag-spin-view-around-hole = Trascina per ruotare la vista attorno al foro. Doppio clic per rivolgerla a nord.
viewport-e = E
viewport-edit-category-colour = Modifica il colore di questa categoria
viewport-edit-colour-used-empty-values = Modifica il colore usato per i valori vuoti
viewport-empty = (vuoto)
viewport-empty-hidden = (vuoto · nascosto)
viewport-filter-variables = Filtra variabili
viewport-fit-hole-track = Adatta il foro alla traccia
viewport-from = da { $from } a { $to }
viewport-from-m = da { $from } a { $to } m
viewport-h-1-ratio = H 1:{ $ratio }
viewport-hole-has-no-trace-draw = Questo foro non ha una traccia da disegnare.
viewport-interval-data = Dati degli intervalli
viewport-intervals = Intervalli
viewport-m-from-collar-toward-bearing = m dalla bocca foro, verso { $bearing }°
viewport-navigation-hint = Trascina col tasto centrale per spostare · Rotella per zoomare
viewport-navigation-hint-detach = Trascina col tasto centrale per spostare · Rotella per zoomare · Clic per staccare
viewport-n = N
viewport-no-data-variable = Nessun dato per questa variabile
viewport-no-density-log-hole = Nessun log di densità per questo foro
viewport-no-downhole-geophysics-hole = Nessuna geofisica in foro per questo foro
viewport-no-gamma-log-hole = Nessun log gamma per questo foro
viewport-no-matches = Nessuna corrispondenza
viewport-no-trace = Nessuna traccia
viewport-no-usable-range = (nessun intervallo utilizzabile)
viewport-not-logged = Non registrato
viewport-orientation-source = Sorgente dell'orientamento
viewport-rebuild-variable-s-colours-from = Ricostruisci i colori di questa variabile dai suoi dati
viewport-reset = Ripristina
viewport-restore-full-model-range = Ripristina l'intero intervallo del modello
viewport-roll-wheel-over-log-zoom = Ruota la rotella sul log per ingrandire uno strato. Trascina il log per ruotare il foro e scorrerlo.
viewport-s = S
viewport-sideways-scale = Scala laterale
viewport-squeeze-sideways-just-enough-keep = Comprime lateralmente quanto basta per mantenere il foro in vista. Non allunga mai.
viewport-trace-extent = Estensione della traccia
viewport-w = O
viewport-widen-panel-show-density = Allarga il pannello per mostrare la densità
viewport-widen-panel-show-density-gamma = Allarga il pannello per mostrare densità e gamma
viewport-widen-panel-show-gamma = Allarga il pannello per mostrare il gamma
charging-edit-charge-product = Modifica prodotto di carica
charging-new-charge-product = Nuovo prodotto di carica
charging-explosive-decks-add-mass-primed-stemming = Le colonne di esplosivo aggiungono massa e hanno il primer; borraggio e camere d'aria hanno solo lunghezza.
charging-density = Densità
charging-density-hint = Densità in foro. La massa per metro è questo valore per la sezione del foro.
charging-another-product-already-has-name = Un altro prodotto ha già questo nome
charging-edit-charge-rule = Modifica regola di carica
charging-new-charge-rule = Nuova regola di carica
charging-decks-collar-toe = Colonne, da bocca a fondo
charging-priming = Innesco
charging-preview = Anteprima
charging-preview-use-pattern-hole = Usa il foro mediano dello schema
charging-preview-active-pattern-median-hole = Anteprima sul foro mediano dello schema attivo
charging-fixed-decks-longer-than-hole = Le colonne fisse sono più lunghe di questo foro
charging-mass-kg-explosive = { $mass } kg di esplosivo
charging-rate-kg-m = { $rate } kg/m
charging-count-primer = { $count } primer
charging-another-rule-already-has-name = Un'altra regola ha già questo nome
charging-save-reload-count-hole = Salva e ricarica { $count } foro/i
charging-length = Lunghezza
charging-rest-length-m = resto · { $length } m
charging-rest = resto
charging-deck-takes-whatever-length-fixed-decks = Questa colonna prende la lunghezza che le colonne fisse lasciano. Una sola colonna per regola riempie.
charging-remove-deck = Rimuovi colonna
charging-add-deck = Aggiungi colonna
charging-downhole-delay = Ritardo in foro
charging-hole-detonator-hole-fires-long-after = Il detonatore in foro. Un foro esplode a questo intervallo dall'arrivo del segnale di superficie.
charging-primer-height = Altezza del primer
charging-how-far-above-base-each-explosive = A che altezza sopra la base di ogni colonna di esplosivo si trova il suo primer.
charging-booster = Booster
charging-cast-booster-mass-each-primer = Massa del booster fuso in ciascun primer.
charging-count-rule-load-product-will-need = { $count } regola/e caricano questo prodotto e ne richiederanno un altro.
charging-rule = Regola
charging-holes-already-loaded-keep-their-charge = I fori già caricati con esso mantengono la loro carica.
blast-burden-relief = Sfogo della resistenza
blast-ms-per-metre-last-neighbour-fire = ms per metro dall'ultimo foro adiacente a esplodere
blast-below-hole-fires-before-rock-front = Sotto questo valore un foro esplode prima che la roccia davanti si sia mossa: stretto.
blast-above-rock-front-has-long-gone = Sopra questo valore la roccia davanti è già partita da tempo: lasco, con rischio di tranciatura e proiezione di roccia.
blast-tight = stretto
blast-good = buono
blast-slack = lasco
blast-free-face = faccia libera
blast-fires-at = Esplode a
blast-empty-won-t-detonate = vuoto, non detonerà
blast-not-reached = non raggiunto
blast-value-ms-m-from-hole = { $value } ms/m da { $hole }
blast-fires-first-free-face = esplode per primo: faccia libera
blast-relief = Sfogo
blast-explosive = Esplosivo
blast-powder-factor = Consumo specifico
blast-not-loaded = Non caricato
blast-count-primer-delay-ms-downhole = { $count } primer · { $delay } ms in foro
blast-set-initiation-point-tie-holes-play = Imposta un punto di innesco e collega i fori per riprodurre la volata
blast-pause = Pausa
blast-play = Riproduci
blast-back-start = Torna all'inizio
blast-duration-ms = di { $duration } ms
blast-real-time = Tempo reale
blast-mic-limit = Limite MIC
blast-most-explosive-allowed-detonate-any-8 = La massima quantità di esplosivo che può detonare in qualsiasi intervallo di 8 ms in questo sito. Le finestre che la superano sono segnalate.
blast-no-holes-loaded-surface-signal-plays = Nessun foro caricato: il segnale di superficie viene riprodotto, ma nulla detona. Carica i fori con lo strumento Carica fori.
blast-now-holes-hole = Ora: { $holes } foro/i
blast-in-8-ms = in 8 ms
blast-peak-mass-kg-time-ms = Picco { $mass } kg a { $time } ms
blast-peak-holes-hole-time-ms = Picco { $holes } foro/i a { $time } ms
blast-peak-over-limit = , { $over } kg oltre
blast-peak-within-limit = , entro il limite
blast-top-surface-signal-lighting-each-downline = In alto: il segnale di superficie che attiva ogni linea di discesa. Sotto: le detonazioni. Fai clic o trascina per spostare la testina di lettura.
products-charge-rules = Regole di carica
products-new-rule = Nuova regola
products-charge-products = Prodotti di carica
products-new-rule-default-name = Nuova regola
products-no-rules = Nessuna regola
products-load-selected-holes-count = Carica fori selezionati ({ $count })
products-unload-selected-holes-count = Scarica fori selezionati ({ $count })
products-edit-rule = Modifica regola
products-duplicate-rule = Duplica regola
products-delete-rule = Elimina regola
products-fill-product = riempimento  { $product }
products-primer-offset-m-off-each-explosive = Primer a { $offset } m dalla base di ogni colonna di esplosivo, booster da { $booster } kg, { $delay } ms in foro
products-double-click-edit = Doppio clic per modificare
products-edit-product = Modifica prodotto
blast-log-updated-charge-product-name = Aggiornato il prodotto di carica { $name }
blast-log-added-charge-product-name = Aggiunto il prodotto di carica { $name }
blast-log-updated-charge-rule-name = Aggiornata la regola di carica { $name }
blast-log-added-charge-rule-name = Aggiunta la regola di carica { $name }
blast-log-entry-no-longer-charge-library = Quella voce non è più nella libreria di carica
blast-log-deleted-name-from-charge-library = Eliminato { $name } dalla libreria di carica
blast-log-failed-save-charge-library-error = Salvataggio della libreria di carica non riuscito: { $error }
blast-log-cannot-load-rule-problem = Impossibile caricare con questa regola: { $problem }
blast-log-count-hole-too-short-fixed-decks = { $count } foro/i sono troppo corti per le colonne fisse di questa regola e sono stati lasciati invariati
blast-log-count-hole-have-no-depth-load = { $count } foro/i non hanno profondità da caricare
blast-log-count-loaded-hole-have-no-diameter = { $count } foro/i caricato/i non hanno diametro, quindi la loro massa di esplosivo è sconosciuta
common-charge-holes = Carica fori
blast-log-loaded-count-hole-rule = Caricato/i { $count } foro/i con { $rule }
blast-log-unload-holes = Scarica fori
blast-log-unloaded-count-hole = Scaricato/i { $count } foro/i
blast-log-select-holes-active-pattern-first = Seleziona prima i fori dello schema attivo
blast-log-rule-no-longer-charge-library = Quella regola non è più nella libreria di carica
blast-log-there-no-charge-rule-load-add = Non c'è alcuna regola di carica con cui caricare: aggiungine una nel pannello dei prodotti
blast-rule-stemming = Borraggio
blast-rule-air-deck = Camera d'aria
blast-rule-give-rule-name = Assegna un nome alla regola
blast-rule-add-least-one-deck = Aggiungi almeno una colonna
blast-rule-only-one-deck-can-fill-rest = Solo una colonna può riempire il resto del foro
blast-rule-deck-lengths-must-greater-than-zero = Le lunghezze delle colonne devono essere maggiori di zero
blast-rule-no-product-named-name = Nessun prodotto chiamato '{ $name }'
blast-rule-rule-needs-least-one-explosive-deck = Una regola richiede almeno una colonna di esplosivo
state-save-charge-product = Salva prodotto di carica
state-save-charge-rule = Salva regola di carica
state-delete-charge-library-entry = Elimina voce della libreria di carica
ui-click-drag-over-holes-load-them = Fai clic o trascina sui fori per caricarli con { $rule }
ui-hold-shift-unload = tieni premuto Maiusc per scaricare
ui-no-charge-rule-load = Nessuna regola di carica con cui caricare
ui-right-click-charge-rules-heading-add = fai clic destro sull'intestazione Regole di carica per aggiungerne una
omf-element-name-has-count-charge-naming = L'elemento '{ $name }' ha { $count } carica/che che indicano fori che non contiene più
common-currency-symbol = €
