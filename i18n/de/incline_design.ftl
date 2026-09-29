# Incline — deutscher Nachrichtenkatalog.
#
# Kann unvollständig sein: fehlende Nachrichten werden aus dem englischen
# Katalog (`i18n/en/incline_design.ftl`) übernommen. Die Bezeichner links
# von `=` und die Argumentnamen ({ $... }) dürfen nicht geändert werden —
# übersetzt wird nur der Text rechts davon.

## Gemeinsam

common-cancel = Abbrechen
common-clear = Löschen
common-close = Schließen
common-fill = Füllung
common-set = Festlegen

## Statusleiste

# Titel des Sprachmenüs in der Statusleiste. Die Sprachen selbst werden nie
# übersetzt: jede benennt sich in ihrer eigenen Schrift, aus `LanguageChoice`.
status-language = Sprache

## Menüleiste — Datei

menu-file = Datei
menu-file-save-project = Projekt speichern
menu-file-save-project-as = Projekt speichern unter...
menu-file-new-project = Neues Projekt...
menu-file-open-project = Projekt öffnen...
menu-file-open-recent = Zuletzt geöffnet
menu-file-show-in-explorer = Im Explorer anzeigen
menu-file-show-in-folder = Enthaltenden Ordner öffnen
menu-file-import = Importieren...
menu-file-export = Exportieren...
menu-file-export-viewport-image = Ansichtsbild exportieren...
menu-file-export-engineering-drawing = Technische Zeichnung exportieren...
menu-file-about = Über { $app }...
menu-file-exit = Anwendung beenden

## Menüleiste — Ansicht

menu-view = Ansicht

## Arbeitsbereiche

ws-production = Produktion
ws-drill-and-blast = Bohren & Sprengen
ws-geology = Geologie
ws-planning = Planung

## Menüleisten

ws-menubar-design = Design
ws-menubar-triangulation = Triangulation
ws-menubar-raster = Raster
ws-menubar-point-cloud = Punktwolke
ws-menubar-block-model = Blockmodell
ws-menubar-drillholes = Bohrlöcher
ws-menubar-active-layer = Ebene:

## Menüleisten-Funktionen

ws-menubar-design-insert-point = Punkt einfügen
ws-menubar-design-insert-point-at-intersection = An Schnittpunkt
ws-menubar-design-insert-point-at-elevation = Auf Höhenkote
ws-menubar-design-move-to = Verschieben nach
ws-menubar-design-create-triangulation = Triangulation erstellen

## Dialoge zum Umbenennen/Löschen von Elementen

# { $kind } ist ein Substantiv aus dem obigen ws-production-*-Satz.
dialog-rename-title = { $kind } umbenennen
dialog-rename-field = Neuer Name
dialog-rename-field-hint = Erforderlich
dialog-rename-submit = Umbenennen
dialog-delete-title = { $kind } löschen
dialog-delete-confirm =
    '{ $name }' aus dem Projekt löschen?
    Dies kann nicht rückgängig gemacht werden.
confirm-delete-product =
    Produkt '{ $name }' aus der Palette löschen?
    Dies kann nicht rückgängig gemacht werden.

## Dialog „Triangulation erstellen“

tri-create-title = Triangulation erstellen
tri-create-type-label = Triangulationstyp
tri-create-type-help =
    Eine offene Oberfläche erzeugt ein geländeartiges Blatt. Ein Volumenkörper
    erzeugt ein vollständig geschlossenes Netz und erfordert Eingaben, die
    eine wasserdichte Begrenzung bilden können.
tri-create-output-name = Ausgabename
tri-create-output-name-help = Name, der der erzeugten Triangulation zugewiesen wird.
tri-create-output-name-hint = Triangulationsname
tri-create-run = Triangulieren

tri-selection-selected = { $summary } ausgewählt

tri-type-open-surface = Oberfläche
tri-type-solid-closed = Volumenkörper

# Bestandteile der Auswahlübersicht, z. B. „3 Polylinien, 1 Punkt“. Jedes
# Substantiv wird nach seiner eigenen Anzahl flektiert, damit Sprachen mit
# mehr als zwei Pluralformen korrekt dargestellt werden.
tri-count-polylines =
    { $count ->
        [one] { $count } Polylinie
       *[other] { $count } Polylinien
    }
tri-count-strings =
    { $count ->
        [one] { $count } Linienzug
       *[other] { $count } Linienzüge
    }
tri-count-points =
    { $count ->
        [one] { $count } Punkt
       *[other] { $count } Punkte
    }
tri-count-texts =
    { $count ->
        [one] { $count } Textobjekt
       *[other] { $count } Textobjekte
    }
tri-count-objects =
    { $count ->
        [one] { $count } Objekt
       *[other] { $count } Objekte
    }

about-read-full-licence = Vollständige Lizenz lesen ↗
about-source-code = Quellcode
about-website = Website
about-title = Über { $app }
drill-hole-colour-stop = Stopp { $index }
properties-restore-defaults-tooltip = { $heading }-Einstellungen auf Standard zurücksetzen

## Dynamische Oberflächenmeldungen

ui-selected-count = { $count } ausgewählt
ui-selected-objects = { $count } Objekt(e) ausgewählt
ui-selected-polylines = { $count } Polylinie(n) ausgewählt
ui-invalid-axis-value = Geben Sie einen gültigen { $axis }-Wert ein.
ui-selection-spans = Auswahl reicht von { $min } bis { $max }.
confirm-delete-count = Möchten Sie die { $count } ausgewählten Elemente wirklich löschen?
confirm-delete-layer = Ebene '{ $name }' und alle darauf befindlichen Objekte löschen?
    Dies kann nicht rückgängig gemacht werden.
plot-preview-pixels = { $width } × { $height } px bei { $dpi } dpi
tri-estimated-memory = Geschätzter Spitzenspeicherbedarf ~{ $estimate }. { $detail }
block-grid-summary = Raster: { $x } × { $y } × { $z } = { $count } Blöcke
status-selected = Ausgewählt: { $count }
status-clip = Clip nah/fern/Δ: { $near } / { $far } / { $delta } m

explorer-no-rasters = Keine Raster
slice-viewport-gestures = Mittlere Taste ziehen: Schwenken · Rechte Taste ziehen: Orbit · Umschalt+Rad: Gehen · W/S: Schicht bewegen · Q/E: Drehen · Esc: Beenden

## Startumgebungsdetails

## Renderer-Startdiagnose

color-aci = ACI
color-aci-value = ACI { $index }
color-index = Index
color-rgb = RGB
color-opacity = Deckkraft
color-edit = Klicken zum Bearbeiten der Farbe
color-saturation-value = Sättigung und Helligkeit
color-hue = Farbton
asset-loading = Asset-Daten werden geladen
asset-unloading = Asset-Daten werden entladen
asset-load-failed = Asset-Daten konnten nicht geladen werden
asset-unload-failed = Asset-Daten konnten nicht entladen werden
preferences-title = Einstellungen
context-text-colour = Textfarbe
context-polylines = Polylinien
context-points = Punkte
crs-unknown-ellipsoid = Unbekanntes Erdmodell „{ $name }“ in dieser Koordinatensystem-Definition.
crs-no-ellipsoid = Diese Koordinatensystem-Definition gibt kein verwendetes Erdmodell an.
crs-unknown-code = EPSG:{ $code } befindet sich nicht in der Koordinatensystem-Registrierung.
crs-transform-failed = Eine Koordinate konnte nicht umgerechnet werden; das Ergebnis war keine endliche Position.
crs-no-datum-path = Zwischen den Referenzrahmen von { $from } und { $to } (EPSG-Datums { $source } und { $target }) ist keine veröffentlichte Transformation verfügbar. Eine Umrechnung wäre um einen unbekannten Betrag falsch, daher wurde nichts geändert.
crs-unknown-datum = Der Referenzrahmen von { $from } oder { $to } kann nicht bestimmt werden, und beide verwenden unterschiedliche Erdmodelle. Eine Umrechnung zwischen ihnen wäre um einen unbekannten Betrag falsch.
ws-survey = Vermessung
survey-count-designs = { $count } { $count ->
    [one] Design
   *[other] Designs
  }
survey-count-meshes = { $count } { $count ->
    [one] Triangulation
   *[other] Triangulationen
  }
survey-count-models = { $count } { $count ->
    [one] Blockmodell
   *[other] Blockmodelle
  }
survey-count-clouds = { $count } { $count ->
    [one] Punktwolke
   *[other] Punktwolken
  }
survey-count-holes = { $count } { $count ->
    [one] Bohrlochdatensatz
   *[other] Bohrlochdatensätze
  }
survey-count-rasters = { $count } { $count ->
    [one] Raster
   *[other] Raster
  }
survey-angle = Drehung um Z (gegen den Uhrzeigersinn)
survey-scale = Einheitlicher XYZ-Skalierungsfaktor
survey-invalid-transform = Ursprünge, Winkel und resultierende Koordinaten müssen endlich sein.
survey-invalid-scale = Der Maßstab muss eine endliche positive Zahl mit endlichem Kehrwert sein.
survey-empty-selection = Mindestens ein unterstütztes Element zum Transformieren auswählen.
survey-unavailable = Ein ausgewähltes Element fehlt oder ist nicht geladen. Vor der Transformation laden.
survey-wrong-project = Nur Designs aus dem aktiven Projekt auswählen.
survey-name-required = Namen für das Koordinatensystem eingeben.
survey-working = Ausgewählte Daten werden transformiert…
survey-completed = { $items } an Ort und Stelle umgerechnet. Rückgängig stellt sie wieder her.
survey-failed = Transformation fehlgeschlagen: { $error }
survey-stale = Transformation verworfen, da sich das aktive Projekt oder die Quelldaten geändert haben. Quelldaten auswählen und erneut versuchen.
survey-coordinates-menu = Koordinaten
survey-definitions-action = Definitionen…
survey-transform-action = Transformieren…
survey-definitions-title = Koordinatendefinitionen
survey-transform-title = Koordinaten transformieren
survey-new-system = Neues Koordinatensystem
survey-new-system-name = Koordinatensystem
survey-set-local = Als Grubenkoordinatensystem festlegen
survey-delete-system = Koordinatensystem löschen
survey-systems-empty = Keine Koordinatensysteme
survey-system-name = Name
survey-system-origin = Gleicher Punkt — Systemkoordinaten
survey-angle-help = Gegen den Uhrzeigersinn von der Referenz-X- zur Referenz-Y-Achse, von oben betrachtet.
survey-scale-help = Einheitliche XYZ-Skalierung vom Referenzrahmen zu diesem System. 1 verwenden, um Abmessungen zu erhalten.
survey-close = Schließen
survey-from = Von
survey-to = Nach
survey-transform-button = Transformieren
survey-swap = Tauschen
survey-drape-note = Drapierte Bilder werden bei umgerechneten Oberflächen entfernt und müssen neu drapiert werden.
survey-needs-grid-block-model = Ein Blockmodell ist ein regelmäßiges Zellgitter, und ein Wechsel von Projektion oder Referenzrahmen erhält diese Regelmäßigkeit nicht. Eine Umrechnung würde bedeuten, jede Zelle in ein neues Gitter neu abzutasten und dabei die enthaltenen Werte zu verlieren; daher wurde es unverändert gelassen.
survey-needs-grid-raster = Ein Raster wird durch eine affine Abbildung in der Welt platziert, was ein Wechsel von Projektion oder Referenzrahmen nicht erhalten kann. Eine Umrechnung würde ein Neuabtasten des Bildes bedeuten; daher wurde es unverändert gelassen.
survey-conversion-exact = Exakt: nur Gitteränderung, keine Neuprojektion.
survey-conversion-accuracy = Angegebene Genauigkeit { $accuracy } m.
survey-kind = Art
survey-axis-names = Achsennamen
survey-kind-registry-short = Registriertes System
survey-kind-grid-short = Raster über einem anderen System
survey-registry-search = Suchen
survey-registry-hint = Name oder EPSG-Code, z. B. „mga zone 56“
survey-registry-none = Nichts in der Registrierung entspricht allen Wörtern.
survey-parent = Definiert gegenüber
survey-parent-origin = Bekannter Punkt — übergeordnete Koordinaten
survey-pick-registry = Nach dem System suchen und es aus den Ergebnissen auswählen.
survey-pick-parent = Das System auswählen, gegenüber dem dieses Raster definiert ist.
survey-pick-system = Ein System auswählen
survey-pick-systems = Das System, von dem umgerechnet werden soll, und das Zielsystem auswählen.
survey-no-selection = Links ein Koordinatensystem auswählen oder mit Rechtsklick eines hinzufügen.
survey-kind-grid = Raster über { $parent }
survey-system-in-use = „{ $name }“ kann nicht gelöscht werden: { $dependants } { $dependants ->
    [one] ist
   *[other] sind
  } dagegen definiert. Diese zuerst auf ein anderes System verweisen.
survey-system-cycle = „{ $name }“ ist direkt oder über seine übergeordneten Systeme gegen sich selbst definiert.
survey-system-missing = Dieses Koordinatensystem existiert nicht mehr. Eine andere Definition auswählen.
survey-same-system = Unterschiedliches Quell- und Zielsystem auswählen.
survey-name-exists = Ein Koordinatensystem mit diesem Namen existiert bereits. Zum Bearbeiten auswählen oder einen anderen Namen wählen.

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins und die Incline-Design-Mitwirkenden. Hiermit wird jeder Person, die eine Kopie dieser Software erhält, kostenlos die Erlaubnis erteilt, uneingeschränkt damit zu verfahren, vorbehaltlich der Bedingungen der MIT-Lizenz.

    Incline Design wird „WIE BESEHEN“ bereitgestellt, OHNE JEGLICHE GEWÄHRLEISTUNG, AUSDRÜCKLICH ODER STILLSCHWEIGEND, EINSCHLIESSLICH, ABER NICHT BESCHRÄNKT AUF DIE GEWÄHRLEISTUNG DER MARKTGÄNGIGKEIT, DER EIGNUNG FÜR EINEN BESTIMMTEN ZWECK UND DER NICHTVERLETZUNG VON RECHTEN DRITTER.
about-free-open-source-mine-design = Freies Open-Source-Minendesign
about-licensed-under-mit-license = Lizenziert unter der MIT-Lizenz

## App strings

app-activated-browser-project-name = Browser-Projekt '{ $name }' aktiviert.
app-browser-project-delete-failed = Löschen des Browser-Projekts fehlgeschlagen: { $error }
app-browser-project-no-longer-exists = Dieses Browser-Projekt existiert nicht mehr
app-browser-save-failed-error = Browser-Speichern fehlgeschlagen: { $error }
app-could-not-activate-browser-project = Browser-Projekt konnte nicht aktiviert werden: { $error }
app-could-not-delete-browser-project = Browser-Projekt konnte nicht gelöscht werden: { $error }
app-could-not-load-browser-project = Das Browser-Projekt konnte nicht geladen werden: { $error }
app-could-not-restore-browser-project = Das Browser-Projekt konnte nicht wiederhergestellt werden: { $error }
app-deleted-browser-project = Browser-Projekt gelöscht
app-failed-create-window-error = Fenster konnte nicht erstellt werden: { $error }
app-failed-create-window-icon-error = Fenstersymbol konnte nicht erstellt werden: { $error }
app-failed-detach-top-down-preview = Draufsicht-Vorschau konnte nicht gelöst werden: { $error }
app-failed-initialize-graphics-error = Grafik konnte nicht initialisiert werden: { $error }
app-browser-preferences-load-failed = Browser-Einstellungen konnten nicht geladen werden: { $error }
app-failed-load-config-file-error = Konfigurationsdatei konnte nicht geladen werden: { $error }
app-failed-load-session-file-error = Sitzungsdatei konnte nicht geladen werden: { $error }
app-failed-rasterize-window-icon-error = Fenstersymbol konnte nicht gerastert werden: { $error }
app-failed-save-browser-session-error = Browser-Sitzung konnte nicht gespeichert werden: { $error }
app-failed-save-session-error = Sitzung konnte nicht gespeichert werden: { $error }
app-saved-name-browser-storage = '{ $name }' im Browser-Speicher gesichert

## Block strings

block-model-between = Zwischen
block-model-block-grid = Blockraster
block-model-block-size = Blockgröße
block-model-choose-numeric-variable = Wählen Sie eine numerische Variable
block-model-choose-numeric-variables = Numerische Variablen wählen
block-model-count-variables-selected = { $count } Variablen ausgewählt
block-model-estimate-variables = Schätzvariablen
block-model-full-x-y-z-dimensions = Vollständige X-, Y- und Z-Abmessungen jedes Blocks. Kleinere Blöcke erhöhen Detailgrad, Rechenzeit und Speicherbedarf.
block-model-grid-bounds-block-sizes-invalid = Rastergrenzen oder Blockgrößen sind ungültig.
block-model-lower-x-y-z-edges = Untere X-, Y- und Z-Kanten des Blockmodellvolumens. Blockzentren beginnen einen halben Block innerhalb dieser Grenzen.
block-model-maximum = Maximum
block-model-maximum-nearest-samples-used-each = Maximale Anzahl der nächstgelegenen Stichproben, die für jeden Block verwendet werden. Niedrigere Werte laufen schneller; höhere Werte können Schätzungen glätten und die Rechenzeit erhöhen.
block-model-maximum-samples = Maximale Stichproben
block-model-minimum = Minimum
block-model-min-samples-help = Minimale Anzahl nahegelegener Stichproben, die zur Schätzung eines Blocks erforderlich sind. Blöcke mit weniger Stichproben innerhalb des Suchradius bleiben leer.
block-model-minimum-samples = Minimale Stichproben
block-model-nugget = Nugget
block-model-numeric-interval-fields-interpolate = Numerische Intervallfelder zur Interpolation. Jedes ausgewählte Feld wird zu einer Blockmodellvariablen.
block-model-kriging-help = Gewöhnliches Kriging schätzt numerische Bohrloch-Intervalle an jedem Blockzentrum mithilfe eines sphärischen Variogramms.
block-model-partial-sill = Teilschwelle
block-model-range-search-radius = Reichweite / Suchradius
block-model-range-help = Stichproben, die weiter als dieser Abstand entfernt sind, werden ausgeschlossen; die Kovarianz erreicht bei dieser Reichweite null.
block-model-select-all = Alle auswählen
block-model-sill-help = Räumlich korrelierte Varianz, die vom sphärischen Modell beigetragen wird. Zusammen mit dem Nugget legt sie die Kovarianz bei Abstand null fest.
block-model-spherical-variogram-search = Sphärisches Variogramm und Suche
block-model-threshold-at-most = <= Schwellenwert
block-model-threshold-at-least = >= Schwellenwert
block-model-threshold-min = Schwellenwert / Min.
block-model-upper-x-y-z-extent = Obere X-, Y- und Z-Ausdehnung, die abgedeckt werden soll. Der letzte Block kann über diese Ausdehnung hinausragen, wenn die Spanne kein exaktes Vielfaches der Blockgröße ist.
block-model-variable = Variable
block-model-variance-effectively-zero-separation = Varianz bei praktisch null Abstand, verursacht durch Messfehler oder Variation unterhalb der Abtastskala. Verwenden Sie null, wenn kein Nugget-Effekt beabsichtigt ist.
block-model-volume-feedback-disconnected = Rücklesung der Blockvolumen-Nutzungsrückmeldung getrennt
block-model-volume-feedback-failed = Rücklesung der Blockvolumen-Nutzungsrückmeldung fehlgeschlagen: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z

## Canvas strings

canvas-not-selectable-closed-polyline = Nicht auswählbar | Wählen Sie eine geschlossene Polylinie
canvas-polyline-summary = Polylinie | Ebene: { $layer } | { $count } Eckpunkte
canvas-surface-name = Oberfläche | { $name }
canvas-trimmed = Zugeschnitten

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = Böschung/Berme aus Objekt { $object_id } erstellt
cmd-bezier-replaced-polyline-span-first-last = Polylinienbereich { $first }→{ $last } durch { $count } abgetastete Zwischenpunkte ersetzt
cmd-bezier-vertices-first-last = Eckpunkte { $first } bis { $last }
cmd-block-model-block-model-loader-disconnected-path = Blockmodell-Lader für { $path } getrennt
cmd-block-model-block-model-path-has-count = Blockmodell { $path } hat { $count } Variable(n) eines nicht unterstützten Typs, die nicht lesbar sein werden: { $names }
cmd-block-model-building-ore-mesh = Erznetz wird erstellt…
cmd-block-model-could-not-create-block-model = Blockmodell konnte nicht erstellt werden: { $error }
cmd-block-model-could-not-decode-block-model = Blockmodell-Farbvariable '{ $variable }' konnte nicht decodiert werden: { $error }
cmd-block-model-created-block-model-name-ordinary = Blockmodell '{ $name }' durch gewöhnliches Kriging erstellt
cmd-block-model-failed-load-block-model-error = Blockmodell konnte nicht geladen werden: { $error }
cmd-block-model-generated-ore-mesh-from-block = Erznetz aus Blockmodell '{ $name }' erzeugt
cmd-block-model-imported-block-model-source-path = Blockmodellquelle { $path } importiert
cmd-block-model-loaded-block-model-name-blocks = Blockmodell '{ $name }' geladen: { $blocks } Blöcke ({ $renderable } darstellbar), Raster { $dimx }x{ $dimy }x{ $dimz }, { $variables } Variablen
cmd-block-model-loading-name = { $name } wird geladen
cmd-block-model-loading-name-ellipsis = { $name } wird geladen…
cmd-chamfer-applied = Ecke { $corner } mit Radius { $radius } und { $segments } Segmenten gefast
cmd-chamfer-radius = Radius { $radius }
cmd-commands-clipped = Beschnitten
cmd-commands-command-failed-error = Befehl fehlgeschlagen: { $error }
cmd-commands-select-one-more-objects-before = Wählen Sie ein oder mehrere Objekte aus, bevor Sie { $axis } festlegen
cmd-commands-sliced = Geschnitten
cmd-contours-contour-generation-failed-error = Höhenlinienerzeugung fehlgeschlagen: { $error }
cmd-contours-discarded-layer-exists = Höhenlinien für '{ $name }' wurden verworfen: Ebene '{ $layer_name }' existiert nun
cmd-contours-discarded-project-closed = Höhenlinien für '{ $name }' wurden verworfen: das Projekt wurde geschlossen
cmd-contours-discarded-layer-deleted = Höhenlinien für '{ $name }' wurden verworfen: die ausgewählte Ausgabeebene wurde gelöscht
cmd-contours-generated = { $line_count } Höhenlinien-Polylinie(n) für Triangulation '{ $name }' in Ebene '{ $layer_name }' erzeugt
cmd-creation-assembled-boundary-rings = { $assembled_count } geschlossene(n) Begrenzungsring(e) aus fragmentierten offenen Linienzügen zusammengesetzt
cmd-creation-created-triangulation-from-boundary = Triangulation aus { $boundary_count } Begrenzungsring(en) und { $constraint_count } offenen Randbedingung(en) erstellt, Oberflächentyp { $surface_type }
cmd-creation-creating-triangulation = Triangulation wird erstellt…
cmd-creation-generate-upper-surface-ignored-count = Obere Oberfläche erzeugen: { $count } tiefer liegende(s), im Konflikt stehende(s) Bruchliniensegment(e) ignoriert; Quellobjekte bleiben unverändert
cmd-creation-ignored-objects = { $rejected } Nicht-Polylinien- oder entartete(s) Objekt(e) während der Triangulation ignoriert
cmd-creation-weld-retry-moved-coarse-welded = Verschweißen & erneut versuchen: { $coarse_welded } Eckpunkt(e) auf gemeinsame Positionen verschoben (bis zu { $coarse_weld_tol } m); Quellobjekte bleiben unverändert
cmd-creation-welded-breakline-vertices = { $welded } Bruchlinien-Eckpunkt(e) verschweißt, die innerhalb der Toleranz zusammenfielen
cmd-cuts-clipped-surface-name-polyline-mode = Oberfläche '{ $name }' an Polylinie beschnitten ({ $mode })
cmd-cuts-clipping-surface-polyline = Oberfläche wird an Polylinie beschnitten…
cmd-cuts-cut-topology-name-pit-shell = Topologie '{ $name }' an Tagebauhülle geschnitten
cmd-cuts-cut-triangulation-name-z-band = Triangulation '{ $name }' nach Z-Band [{ $min }, { $max }] geschnitten
cmd-cuts-cutting-topology-pit-shell = Topologie wird an Tagebauhülle geschnitten…
cmd-cuts-cutting-triangulation-z = Triangulation wird nach Z geschnitten…
cmd-cuts-ignored-vertical-faces = { $count } vertikale(s) oder entartete(s) Referenztopologie-Fläche(n) ohne XY-Fläche ignoriert
cmd-cuts-site-skipped-constraint-from-x = { $site }: Randbedingung ({ $from_x }, { $from_y }) -> ({ $to_x }, { $to_y }) übersprungen, die der Triangulator nicht teilen konnte
cmd-cuts-skipped-degenerate-edges = { $site }: { $skipped } nahezu entartete Randbedingungskante(n) übersprungen; die Schnittgrenze kann dort um Haaresbreite abweichen
cmd-cuts-trimmed-surface = Oberfläche '{ $surface }' auf Topologie '{ $topology }' zugeschnitten ({ $mode })
cmd-cuts-trimming-surface-topology = Oberfläche wird auf Topologie zugeschnitten…
cmd-drape-draped-intersected-vertices-changed = { $intersected } Eckpunkte drapiert; { $changed } änderten die Höhenkote
cmd-drape-no-intersections = Keiner der ausgewählten Design-Eckpunkte schneidet die ausgewählten Topologien
cmd-drape-objects-changed-object-s-changed = { $objects } geänderte(s) Objekt(e) · { $changed } von { $intersected } sich schneidenden Eckpunkten verschoben
cmd-drape-select-one-more-design-objects = Wählen Sie ein oder mehrere Design-Objekte zum Drapieren
cmd-drape-select-one-more-topologies-drape = Wählen Sie eine oder mehrere Topologien zum Drapieren
cmd-drape-selected-topologies-no-longer-loaded = Die ausgewählten Topologien sind nicht mehr geladen
cmd-drill-hole-drill-pattern-too-large-contains = Das Bohrmuster ist zu groß oder enthält ungültige Ansatzpunkt-Koordinaten
cmd-drill-hole-enter-name-drill-pattern = Geben Sie einen Namen für das Bohrmuster ein
cmd-drill-hole-failed-load-drillholes-error = Bohrlöcher konnten nicht geladen werden: { $error }
cmd-drill-hole-depth-must-be-positive = Lochtiefe muss größer als null sein
cmd-drill-hole-diameter-must-be-positive = Lochdurchmesser muss größer als null sein
cmd-drill-hole-loaded-drillhole-dataset-name-holes = Bohrloch-Datensatz '{ $name }' geladen: { $holes } Bohrlöcher, { $fields } Farbfelder
cmd-drill-hole-pattern-contains-no-holes = Das Muster enthält keine Löcher
cmd-explode-count-line-s = { $count } Linie(n)
cmd-explode-polyline = Polylinie auflösen
cmd-explode-exploded-polyline-into-count-line = Polylinie in { $count } Liniensegmente aufgelöst
cmd-file-block-model-csv-encoding-failed = Blockmodell-CSV-Kodierung fehlgeschlagen: { $error }
cmd-file-block-model-csv-export-failed = Blockmodell-CSV-Export fehlgeschlagen: { $error }
cmd-file-browser-recovery-unavailable = Browser-Wiederherstellungsdateien sind nicht verfügbar; gespeicherte Projekte bleiben in IndexedDB
cmd-file-closed-project-runtime-id-runtime = Projekt mit Laufzeit-ID { $runtime_id } geschlossen
cmd-file-could-not-create-new-project = Es konnte kein neues Projekt erstellt werden: { $error }
cmd-file-could-not-finish-pending-project = Die ausstehende Projektaktion konnte nicht abgeschlossen werden: { $error }
cmd-file-could-not-finish-saving-before = Speichern vor dem Beenden konnte nicht abgeschlossen werden: { $error }
cmd-file-could-not-open-browser-project = Browser-Projekt konnte nicht geöffnet werden: { $error }
cmd-file-could-not-open-path-error = { $path } konnte nicht geöffnet werden: { $error }
cmd-file-could-not-read-selected-file = Ausgewählte Datei konnte nicht gelesen werden: { $error }
cmd-file-could-not-reload-layer-from = Ebene konnte nicht von der Festplatte neu geladen werden: { $error }
cmd-file-could-not-reload-project-from = Projekt konnte nicht von der Festplatte neu geladen werden: { $error }
cmd-file-could-not-remove-browser-project = Browser-Projekt konnte nicht entfernt werden: { $error }
cmd-file-could-not-restore-layer-from = Ebene konnte nicht aus dem Projekt wiederhergestellt werden: { $error }
cmd-file-could-not-snapshot-dirty-project = Für das geänderte Projekt konnte kein Wiederherstellungsstand erstellt werden: { $error }
cmd-file-could-not-start-browser-export = Browser-Export konnte nicht gestartet werden: { $error }
cmd-file-could-not-write-recovery-copies = Wiederherstellungskopien konnten nicht geschrieben werden: { $error }
cmd-file-created-new-browser-project = Neues Browser-Projekt erstellt
cmd-file-created-new-project = Neues Projekt erstellt
cmd-file-description-download-failed-error = { $description }-Download fehlgeschlagen: { $error }
cmd-file-discard-cancelled-project-changed = Verwerfen wurde abgebrochen, da sich das Projekt änderte, während die OMF-Datei neu geladen wurde
cmd-file-discarded-changes-layer-target-name = Änderungen an Ebene '{ $target_name }' verworfen
cmd-file-discarded-changes-reloaded-path = Änderungen verworfen: { $path } neu geladen
cmd-file-downloaded-description-file-name = { $description } heruntergeladen: { $file_name }
cmd-file-dxf-download-encoding-failed-error = DXF-Download-Kodierung fehlgeschlagen: { $error }
cmd-file-dxf-import-failed-error = DXF-Import fehlgeschlagen: { $error }
cmd-file-encoding-block-model-csv-download = Blockmodell-CSV-Download wird kodiert…
cmd-file-encoding-dxf-download = DXF-Download wird kodiert…
cmd-file-encoding-triangulation-download = Triangulations-Download wird kodiert…
cmd-file-exit-deferred-exports = Beenden verzögert, bis Hintergrundexporte abgeschlossen sind
cmd-file-exit-requested-no-unsaved-changes = Beenden angefordert, keine ungespeicherten Änderungen
cmd-file-exported-block-model-csv-path = Blockmodell-CSV nach { $path } exportiert
cmd-file-exported-description-dxf-path = { $description } als DXF exportiert: { $path }
cmd-file-exported-triangulation-name-path = Triangulation '{ $name }' nach { $path } exportiert
cmd-file-exporting-name = { $name } wird exportiert…
cmd-file-exporting-triangulation-name-path = Exportiere Triangulation '{ $name }' nach { $path }
cmd-file-fatal-renderer-failure-reason = Schwerwiegender Renderer-Fehler: { $reason }
cmd-file-dialog-action-failed = Dateidialog-Aktion fehlgeschlagen: { $msg }
cmd-file-imported-added-object-s-from = { $added } Objekt(e) aus { $name } importiert
cmd-file-imported-total-dxf-object-s = { $total } DXF-Objekt(e) importiert
cmd-file-layer-discard-was-cancelled-because = Ebenen-Verwerfen wurde abgebrochen, da sich das Projekt änderte, während es neu geladen wurde
cmd-file-no-recovery-directory = Kein Wiederherstellungsverzeichnis verfügbar: { $error }
cmd-file-no-unsaved-project-content-nothing = Kein ungespeicherter Projektinhalt; nichts wiederherzustellen
cmd-file-parsing-browser-dxf-import = Browser-DXF-Import wird verarbeitet…
cmd-file-parsing-dxf-import = DXF-Import wird verarbeitet…
cmd-file-project-closes-after-save = Das Projekt wird geschlossen, sobald der aktuelle Speichervorgang abgeschlossen ist
cmd-file-the-project-closes-after-save = Das Projekt wird geschlossen, sobald der aktuelle Speichervorgang abgeschlossen ist
cmd-file-queued-count-triangulation-file-s = { $count } Triangulationsdatei(en) zum Import eingereiht
cmd-file-recovery-copies-path-reopen-them = Wiederherstellungskopien befinden sich in { $path }; öffnen Sie sie nach dem Neustart erneut
cmd-file-recovery-copy-failed-error = Wiederherstellungskopie fehlgeschlagen: { $error }
cmd-file-recovery-copy-failed-failure = Wiederherstellungskopie fehlgeschlagen: { $failure }
cmd-file-recovery-copy-written-path = Wiederherstellungskopie geschrieben: { $path }
cmd-file-reverting-layer = Ebene wird zurückgesetzt…
cmd-file-reverting-project = Projekt wird zurückgesetzt…
cmd-file-save-failed-message = Speichern fehlgeschlagen: { $message }
cmd-file-save-worker-ended-without-result = Speicher-Worker wurde ohne Ergebnis beendet
cmd-file-saved-project-as = Projekt gespeichert unter: { $path }
cmd-file-saved-project = Projekt gespeichert: { $path }
cmd-file-selected-block-model-no-longer = Das ausgewählte Blockmodell ist nicht mehr geladen
cmd-file-switching-project = Projekt wird gewechselt…
cmd-file-triangulation-download-encoding-failed = Triangulations-Download-Kodierung fehlgeschlagen: { $error }
cmd-file-user-chose-exit-without-saving = Benutzer hat sich entschieden, ohne Speichern zu beenden
cmd-file-user-requested-exit-project-export = Benutzer hat Beenden angefordert (Bestätigung für Projektexport oder ungespeicherte Arbeit erforderlich)
cmd-file-viewport = Ansichtsfenster
cmd-file-wait-current-project-save-finish = Warten Sie, bis das aktuelle Speichern des Projekts abgeschlossen ist
cmd-file-wait-current-project-switch-finish = Warten Sie, bis der aktuelle Projektwechsel abgeschlossen ist
cmd-file-wait-project-operation-finish-before = Warten Sie, bis der Projektvorgang abgeschlossen ist, bevor Sie Änderungen verwerfen
cmd-file-wait-project-revert-finish-before = Warten Sie, bis die Projektwiederherstellung abgeschlossen ist, bevor Sie speichern
cmd-fuse-closed-polyline = Geschlossene Polylinie
cmd-fuse-count-source-line-s = { $count } Quelllinie(n)
cmd-fuse-created-shape-object-id-vertices = { $shape } { $object_id } mit { $vertices } Eckpunkten aus { $sources } Quelllinie(n) erstellt
cmd-fuse-click-missed = Verschmelzen: Klick traf kein Objekt (nichts unter dem Zeiger)
cmd-fuse-click-not-near-endpoint = Verschmelzen: Klick war keinem Endpunkt der ausgewählten Linie nahe genug
cmd-fuse-clicked-closed-polyline = Verschmelzen: angeklicktes Objekt { $object_id } ist eine geschlossene Polylinie, Verschmelzen funktioniert nur mit offenen Polylinien
cmd-fuse-clicked-not-open-polyline = Verschmelzen: angeklicktes Objekt { $object_id } ist keine offene Polylinie (es ist ein/e { $kind })
cmd-fuse-clicked-object-missing = Verschmelzen: angeklicktes Objekt { $object_id } existiert nicht mehr
cmd-fuse-clicked-too-few-vertices = Verschmelzen: angeklickte Polylinie { $object_id } hat nur { $count } Eckpunkt(e), mindestens 2 erforderlich
cmd-fuse-endpoint-marker-missing = Verschmelzen: Endpunktmarkierung { $marker_index } existiert nicht mehr
cmd-fuse-close-needs-three-vertices = Verschmelzen: Linie benötigt mindestens 3 unterschiedliche Eckpunkte, um zu einer Polylinie geschlossen zu werden (hat { $count })
cmd-fuse-lines = Linien verschmelzen
cmd-fuse-needs-two-segments = Verschmelzen: mindestens 2 Segmente zum Abschließen erforderlich (vorhanden: { $count })
cmd-fuse-no-active-layer = Verschmelzen: keine aktive Ebene, um die verschmolzene Linie darauf zu platzieren
cmd-fuse-no-active-project = Verschmelzen: kein aktives Projekt, kann nicht abgeschlossen werden
cmd-fuse-no-source-line = Verschmelzen: keine Quelllinie zum Schließen zu einer Polylinie
cmd-fuse-awaiting-object-invalid = Verschmelzen: Objekt { $awaiting_id } ist keine gültige Polylinie mehr
cmd-fuse-object-already-in-chain = Verschmelzen: Objekt { $object_id } ist bereits Teil der Verschmelzungskette, klicken Sie eine andere Linie an
cmd-fuse-result-too-few-vertices = Verschmelzen: Ergebnis hat zu wenige Eckpunkte ({ $count }), Abbruch
cmd-fuse-segment-object-invalid = Verschmelzen: Segmentobjekt { $object_id } ist keine gültige Polylinie mehr, Abbruch
cmd-fuse-source-object-invalid = Verschmelzen: Quellobjekt { $object_id } ist keine gültige offene Polylinie mehr
cmd-fuse-source-object-missing = Verschmelzen: Quellobjekt { $object_id } existiert nicht mehr
cmd-fuse-open-polyline = Offene Polylinie
cmd-include-failed = Einbeziehen fehlgeschlagen: { $message }
cmd-include-included-solid-shape-name-topology = Körper '{ $shape_name }' in Topologie '{ $topology_name }' einbezogen ({ $retained } Topologieflächen behalten, { $skipped } Verschlusskappenflächen übersprungen)
cmd-include-including-pit-stockpile-solid = Tagebau-/Haldenkörper wird einbezogen…
cmd-insert-point-count-operation-point-s = { $count } { $operation }-Punkt(e)
cmd-insert-point-elevation-must-be-finite = „Punkt auf Höhenkote einfügen“ erfordert eine endliche Höhenkote
cmd-insert-point-insert-points = Punkte einfügen
cmd-insert-point-inserted-count-operation-point-s = { $count } { $operation }-Punkt(e) eingefügt
cmd-insert-point-intersection = Schnittpunkt
cmd-insert-point-no-new-operation-points-were = Es wurden keine neuen { $operation }-Punkte gefunden
cmd-insert-point-select-least-two-polylines-before = Wählen Sie mindestens zwei Polylinien aus, bevor Sie Schnittpunkte einfügen
cmd-insert-point-select-one-more-polylines-before = Wählen Sie eine oder mehrere Polylinien aus, bevor Sie einen Punkt auf Höhenkote einfügen
cmd-layer-created-layer-name = Ebene '{ $name }' erstellt
cmd-layer-deleted-with-objects = Ebene { $layer_id } (und alle darauf befindlichen Objekte) gelöscht
cmd-layer-duplicated-layer-duplicate-name = Ebene '{ $duplicate_name }' dupliziert
cmd-layer-locked = Gesperrt
cmd-layer-name-copy = { $name } Kopie
cmd-layer-selected-count-object-s-layer = { $count } Objekt(e) in Ebene { $layer_id } ausgewählt
cmd-layer-state-layer-name = Ebene '{ $name }' { $state }
cmd-layer-unlocked = Entsperrt
cmd-move-tool-moved-collars = Verschiebung ({ $delta }) auf { $count } Bohrloch-Ansatzpunkt(e) angewendet
cmd-move-tool-moved-objects = Verschiebung ({ $delta }) auf { $count } Objekt(e) angewendet
cmd-move-tool-count-hole-s = { $count } Loch/Löcher
cmd-object-edit-edited-kind = { $kind } bearbeitet
cmd-object-edit-edited-kind-count-vertices = { $kind } bearbeitet ({ $count } Scheitelpunkte)
cmd-object-edit-no-changes-apply = Keine Änderungen anzuwenden
cmd-object-edit-object-changed-since-editor-opened = Dieses Objekt hat sich geändert, seit der Editor geöffnet wurde; erneut öffnen, um die aktuelle Version zu bearbeiten
cmd-object-edit-target-changed = Bearbeitetes Objekt hat sich geändert; Bearbeitung wird verworfen
cmd-object-edit-object-no-longer-exists-document = Dieses Objekt existiert im Dokument nicht mehr
cmd-object-edit-select-single-design-object-edit = Ein einzelnes Design-Objekt zum Bearbeiten auswählen
cmd-object-edit-unassigned = Nicht zugewiesen
cmd-offset-create-offset = Versatz erstellen
cmd-offset-created-offset-count-object-s = Versatz von { $count } Objekt(en) erstellt
cmd-offset-distance-must-be-positive = Versatzabstand muss größer als null sein
cmd-omf-could-not-open-project-source = Projekt { $source_name } konnte nicht geöffnet werden: { $error }
cmd-omf-create-open-project-before-merging = Erstellen oder öffnen Sie ein Projekt, bevor Sie Daten zusammenführen
cmd-omf-encoding-project = Projekt wird kodiert…
cmd-omf-exported-project-path = Projekt nach { $path } exportiert
cmd-omf-imported-project = Projekt '{ $project_name }' aus { $source_name } importiert: { $count } Datensatz/Datensätze oberster Ebene
cmd-omf-importing-project = Projekt wird importiert…
cmd-omf-export-failed = OMF-Export fehlgeschlagen: { $error }
cmd-omf-import-failed = OMF-Import fehlgeschlagen: { $error }
cmd-omf-opened-project = Projekt '{ $project_name }' aus { $source_name } geöffnet
cmd-omf-project-source-name-contains-no = Projekt '{ $source_name }' enthält keine unterstützten Datenelemente
cmd-omf-source-name-applied-project-origin = { $source_name }: Projektursprung { $origin } vor dem Zusammenführen angewendet
cmd-omf-crs-differs = { $source_name }: Koordinatenreferenzsystem '{ $source_crs }' unterscheidet sich vom Projekt-KRS '{ $target_crs }'; Koordinaten wurden ohne Umprojektion zusammengeführt
cmd-omf-source-name-units-source-units = { $source_name }: Einheiten '{ $source_units }' unterscheiden sich von den Projekteinheiten '{ $target_units }'; Koordinaten wurden ohne Umrechnung zusammengeführt
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = Es gibt keine offenen Incline-Design-Daten zum Exportieren
cmd-placement-2-vertices = 2 Eckpunkte
cmd-placement-count-vertices = { $count } Eckpunkte
cmd-placement-created-circle = Kreis mit Radius { $radius } m erstellt
cmd-placement-created-closed-polyline = Geschlossene Polylinie mit { $count } Eckpunkten erstellt
cmd-placement-created-line-segment-2-vertices = Liniensegment mit 2 Eckpunkten erstellt
cmd-placement-created-open-polyline-count-vertices = Offene Polylinie mit { $count } Eckpunkten erstellt
cmd-placement-placed-point-x-y-z = Punkt bei { $x }, { $y }, { $z } platziert
cmd-placement-radius = Radius { $radius } m
cmd-plot-composing-engineering-drawing = Technische Zeichnung wird zusammengestellt…
cmd-plot-could-not-write-engineering-drawing = Die technische Zeichnung konnte nicht geschrieben werden: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Zeichnungsmaßstab an sichtbare Daten angepasst: 1:{ $scale }
cmd-plot = Plot
cmd-plot-saved-drawing = Technische Zeichnung gespeichert: { $description } ({ $width } × { $height } px bei { $dpi } dpi)
cmd-point-cloud-failed-load-point-cloud-error = Punktwolke konnte nicht geladen werden: { $error }
cmd-point-cloud-loaded-point-cloud-name-count = Punktwolke { $name } geladen ({ $count } Punkte)
cmd-point-cloud-point-cloud-loader-disconnected-path = Punktwolken-Lader für { $path } getrennt
cmd-point-cloud-tin-max-edge-disabled = (max. Kante deaktiviert)
cmd-point-cloud-tin-max-edge-max-edge = (max. Kante { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = Punktwolken-TIN fehlgeschlagen: { $error }
cmd-point-cloud-tin-subsampled = Gelände-TIN: { $sampled } von { $total } Punkten räumlich unterabgetastet
cmd-point-cloud-tin-triangulated = Gelände-TIN: { $vertex_count } eindeutige XY-Punkte zu { $face_count } Flächen trianguliert{ $suffix }
cmd-products-added-product-delay-ms-ms = Produkt { $delay_ms } ms { $name } hinzugefügt
cmd-products-deleted-product-delay-ms-ms = Produkt { $delay_ms } ms { $name } gelöscht
cmd-products-failed-save-products-error = Produkte konnten nicht gespeichert werden: { $error }
cmd-products-product-no-longer-palette = Dieses Produkt ist nicht mehr in der Palette
cmd-property-action-count-object-s-layer = { $action } { $count } Objekt(e) in Ebene { $layer }
cmd-property-batch-set-axis-value-count = { $axis }-Wert für { $count } Objekt(e) gesammelt festgelegt
cmd-property-batch-set-closed-count-polyline = „Geschlossen“ für { $count } Polylinie(n) gesammelt festgelegt
cmd-property-batch-set-color-count-object = Farbe für { $count } Objekt(e) gesammelt festgelegt
cmd-property-batch-set-fill-style-count = Füllstil für { $count } Objekt(e) gesammelt festgelegt
cmd-property-batch-set-line-weight-count = Linienstärke für { $count } Polylinie(n) gesammelt festgelegt
cmd-property-copied = Kopiert
cmd-property-moved = Verschoben
cmd-raster-draped = Raster { $raster } über Triangulation { $triangulation } drapiert (überlappende Ausdehnung)
cmd-raster-failed-load-raster-name-error = Raster { $name } konnte nicht geladen werden: { $error }
cmd-raster-failed-load-raster-path-error = Raster { $path } konnte nicht geladen werden: { $error }
cmd-raster-loaded-raster-name-via-driver = Raster { $name } über { $driver } geladen ({ $srcx }x{ $srcy }, Vorschau { $prevx }x{ $prevy })
cmd-raster-no-overlapping-triangulation = Keine geladene Triangulation überlappt die Ausdehnung von { $name }
cmd-raster-loader-disconnected = Raster-Lader für { $path } getrennt
cmd-raster-undraped = Rasterdrapierungen von { $count } Triangulation(en) entfernt
cmd-relimit-click-missed = Neubegrenzung: Klick traf kein Objekt (nichts unter dem Zeiger)
cmd-relimit-click-ignored = Neubegrenzung: Klick ignoriert, das Werkzeug wartet derzeit nicht auf eine Zielauswahl
cmd-relimit-clicked-source-line = Neubegrenzung: die Quelllinie selbst angeklickt, wählen Sie eine andere Linie
cmd-relimit-no-source-line = Neubegrenzung: keine Quelllinie festgelegt, Auswahl wird abgebrochen
cmd-relimit-relimited-line-source-id-selected = Linie { $source_id } auf das ausgewählte Ziel neu begrenzt
cmd-relimit-resized-line-source-id-using = Linie { $source_id } mit { $mode } Wert { $value } skaliert
cmd-rename-item-no-longer-belongs-active = Dieses Element gehört nicht mehr zum aktiven Projekt
cmd-rename-renamed-before-name = '{ $before }' in '{ $name }' umbenannt
cmd-rename-renamed-name-taken = '{ $before }' in '{ $name }' umbenannt ('{ $requested }' ist bereits vergeben)
cmd-rotate-collar-turned-count-drillhole-collar-s = { $count } Bohrloch-Ansatzpunkt(e) um { $rotation } gedreht
cmd-section-verb-count-item-s-section = { $verb } { $count } Element(e) in { $section }
cmd-selection-delete-vertex = Eckpunkt löschen
cmd-selection-deleted-count-selected-object-s = { $count } ausgewählte(s) Objekt(e) gelöscht
cmd-selection-deleted-vertex = Eckpunkt { $vertex } aus Polylinie { $object_id } gelöscht
cmd-selection-duplicate-selection = Auswahl duplizieren
cmd-selection-duplicated-count-object-s = { $count } Objekt(e) dupliziert
cmd-session-created-triangulation = Triangulation '{ $name }' erstellt ({ $vertex_count } Eckpunkte, { $face_count } Flächen) aus Oberflächentyp { $surface_type }
cmd-session-deleted-triangulation = Triangulation '{ $name }' aus Projekt gelöscht
cmd-session-failed-load-triangulation-error = Triangulation konnte nicht geladen werden: { $error }
cmd-session-failed-load-triangulation-message = Triangulation konnte nicht geladen werden: { $message }
cmd-session-loaded-triangulation = Triangulation '{ $name }' geladen ({ $path }, { $vertex_count } Eckpunkte, { $face_count } Flächen)
cmd-session-set-triangulation-tri-id-color = Farbe der Triangulation { $tri_id } auf { $color } gesetzt
cmd-session-triangulation-load-no-result = Laden der Triangulation für { $path } endete ohne Ergebnis
cmd-session-triangulation-failed = Triangulationsvorgang fehlgeschlagen: { $message }
cmd-session-unloaded-triangulation-name = Triangulation '{ $name }' entladen
cmd-slice-entered-slice-view-cx-cy = Schnittansicht betreten bei { $cx }, { $cy }, { $cz } entlang { $dx }, { $dy } ({ $length } m Linie)
cmd-slice-exited-slice-view = Schnittansicht verlassen
cmd-slice-reset-section-view-fit-extents = Schnittansicht zurücksetzen (an Ausdehnung anpassen)
cmd-slice-set-section-grid-enabled = Schnittraster aktiviert = { $enabled }
cmd-split-created-2-open-polylines = 2 offene Polylinien erstellt
cmd-split-line = Linie teilen
cmd-split-points-needs-interior-vertex = An Punkten teilen: Wählen Sie einen inneren Eckpunkt der offenen Linie
cmd-split-points-needs-non-adjacent-vertices = An Punkten teilen: Wählen Sie zwei nicht benachbarte Polylinien-Eckpunkte
cmd-split-polyline-into-two = Quellpolylinie in zwei offene Polylinien geteilt
cmd-text-edit-finished = Textbearbeitung für Objekt { $object_id } abgeschlossen
cmd-text-updated = Text auf Objekt { $object_id } aktualisiert
cmd-view-centre-rotation-not-available-flying = Der Rotationsmittelpunkt ist im Flugmodus nicht verfügbar
cmd-view-fixed-centre-rotation-x-y = Rotationsmittelpunkt bei { $x }, { $y }, { $z } festgelegt
cmd-view-no-point-under-cursor-fix = Kein Punkt unter dem Cursor, um den Rotationsmittelpunkt darauf festzulegen
cmd-view-released-centre-rotation = Rotationsmittelpunkt freigegeben
cmd-view-reset-view-fit-extents = Ansicht zurückgesetzt (an Ausdehnung angepasst)
cmd-view-set-topology-wireframes-enabled = Topologie-Drahtgitter = { $enabled } gesetzt
cmd-view-set-view-points-enabled = Punkte anzeigen = { $enabled } gesetzt
cmd-view-set-xy-grid-enabled = XY-Raster aktiviert = { $enabled }
cmd-view-zoom-extents-preserving-angle = Auf Ausdehnung zoomen (Blickwinkel beibehalten)

## Common strings

common-add-product = Produkt hinzufügen
common-background = Hintergrund
common-block-model = Blockmodell
common-block-models = Blockmodelle
common-cancelled = Abgebrochen
common-chamfer = Fase
common-choose = Wählen...
common-circle = Kreis
common-click-point-fix-centre-rotation = Auf einen Punkt klicken, um den Rotationsmittelpunkt festzulegen
common-clip-surface-polyline = Oberfläche an Polylinie beschneiden...
common-closed = Geschlossen
common-colour = Farbe
common-confirm-omf-rewrite = OMF-Überschreiben bestätigen
common-could-not-replace-current-project = Das aktuelle Projekt konnte nicht ersetzt werden: { $error }
common-count-object-s = { $count } Objekt(e)
common-create = Erstellen
common-create-batter-berm = Böschung/Berme erstellen
common-create-bezier-curve = Bézierkurve erstellen
common-create-block-model = Blockmodell erstellen
common-create-block-model-ellipsis = Blockmodell erstellen...
common-create-circle = Kreis erstellen
common-create-drill-pattern = Bohrmuster erstellen
common-create-layer = Ebene erstellen
common-create-line = Linie erstellen
common-create-ore-triangulation = Erz-Triangulation erstellen
common-create-ore-triangulation-ellipsis = Erz-Triangulation erstellen...
common-create-point = Punkt erstellen
common-create-polyline = Polylinie erstellen
common-create-triangulation = Triangulation erstellen...
common-crosses = Kreuze
common-cut = Ausschneiden
common-cut-topology-pit-shell = Topologie mit Tagebauhülle schneiden...
common-delete-layer = Ebene löschen
common-delete-product = Produkt löschen
common-delete-selection = Auswahl löschen
common-designs = Designs
common-discard-layer-changes = Ebenenänderungen verwerfen
common-down = Ab
common-drape-topology = Auf Topologie drapieren
common-easting = Rechtswert
common-edit-object = Objekt bearbeiten
common-edit-text = Text bearbeiten
common-elevation = Höhenkote
common-exit-without-saving = Ohne Speichern beenden
common-export-engineering-drawing = Technische Zeichnung exportieren
common-filter = Filter
common-fly-mode = Flugmodus
common-generate-contour-lines = Höhenlinien erzeugen...
common-hide-all = Alle ausblenden
common-hide-selection = Auswahl ausblenden
common-ignore = Ignorieren
common-import-csv-block-model = CSV-Blockmodell importieren
common-import-dxf = DXF importieren
common-incline-design-project = Incline-Design-Projekt
common-layer = Ebene
common-legend = Legende
common-line = Linie
common-line-weight = Linienstärke
common-lock-all = Alle sperren
common-lock-selection = Auswahl sperren
common-m = m
common-max = Max.
common-merge-shell-into-topology = Hülle in Topologie einfügen
common-merge-shell-into-topology-ellipsis = Hülle in Topologie einfügen...
common-move-collar = Ansatzpunkt verschieben
common-move-design = Design verschieben
common-move-selection = Auswahl verschieben
common-new-product = Neues Produkt
common-no-block-models = Keine Blockmodelle
common-no-design-layers = Keine Design-Ebenen
common-no-drill-holes = Keine Bohrlöcher
common-no-file-chosen = Keine Datei ausgewählt
common-no-open-project = Kein offenes Projekt
common-no-point-clouds = Keine Punktwolken
common-no-triangulations = Keine Triangulationen
common-none = Keine
common-northing = Hochwert
common-offset = Versatz
common-open = Öffnen
common-orientation = Ausrichtung
common-point = Punkt
common-point-cloud = Punktwolke
common-point-clouds = Punktwolken
common-polyline = Polylinie
common-polyline-layer = Polylinie auf '{ $layer }'
common-project = Projekt
common-rasters = Raster
common-redo = Wiederholen
common-relimit-line = Linie neu begrenzen
common-remove-project = Projekt entfernen
common-reset-view = Ansicht zurücksetzen
common-reveal-all = Alles anzeigen
common-reveal-finder = Im Finder anzeigen
common-rotate-collar = Ansatzpunkt drehen
common-save-exit = Speichern und beenden
common-scale-bar = Maßstabsleiste
common-set-initiation-point = Zündpunkt festlegen
common-shape = Form
common-shell = Mit Hülle
common-slashes = Schrägstriche
common-slice = Schnitt
common-slice-triangulation-z-range = Triangulation nach Z-Bereich schneiden...
common-surface-contours = Oberflächen-Höhenlinien
common-text = Text
common-degree-suffix = °
common-tie-holes = Löcher verbinden
common-triangulations = Triangulationen
common-trim-topology = Auf Topologie zuschneiden...
common-undo = Rückgängig
common-undrape-all = Alle Drapierungen entfernen
common-uniform-white = Einheitlich weiß
common-unlock-all = Alle entsperren
common-untitled = Unbenannt
common-up = Auf
common-vertical-exaggeration = Vertikale Überhöhung
common-x = x
common-zoom-extents = Auf Ausdehnung zoomen

## Confirmations strings

confirmations-close-project-unsaved-changes = Projekt schließen: Ungespeicherte Änderungen
confirmations-close-without-saving = Ohne Speichern schließen
confirmations-delete = Löschen
confirmations-delete-objects = Objekte löschen
confirmations-discard = Verwerfen
confirmations-discard-all-unsaved-changes-layer =
    Alle ungespeicherten Änderungen an Ebene '{ $name }' verwerfen?
    Die gespeicherte Ebene wird von der Festplatte neu geladen, während Änderungen an anderen Ebenen erhalten bleiben. Dies kann nicht rückgängig gemacht werden.
confirmations-discard-all-unsaved-changes-name =
    Alle ungespeicherten Änderungen an '{ $name }' verwerfen?
    Die zuletzt gespeicherte Version wird von der Festplatte neu geladen. Dies kann nicht rückgängig gemacht werden.
confirmations-discard-changes = Änderungen verwerfen
confirmations-exit-unsaved-changes = Beenden: Ungespeicherte Änderungen
confirmations-incline-design-cannot-reproduce-all = Incline Design kann nicht den gesamten Inhalt der ursprünglichen OMF-Datei reproduzieren. Beim Speichern wird folgender Inhalt weggelassen:
confirmations-product = Produkt
confirmations-project = dieses Projekt
confirmations-remove-name-delete-its-browser = '{ $name }' entfernen und dessen im Browser gespeicherte Kopie löschen? Ungespeicherte Änderungen gehen verloren.
confirmations-remove-project-unsaved-changes = Projekt entfernen: Ungespeicherte Änderungen
confirmations-remove-without-saving = Ohne Speichern entfernen
confirmations-replace-project-unsaved-changes = Projekt ersetzen: Ungespeicherte Änderungen
confirmations-save = Speichern
confirmations-save-anyway = Trotzdem speichern
confirmations-save-changes-current-project-before = Änderungen am aktuellen Projekt vor dem Ersetzen speichern?
confirmations-save-changes-name-before-closing = Änderungen an '{ $name }' vor dem Schließen speichern?
confirmations-save-changes-name-before-removing = Änderungen an '{ $name }' speichern, bevor es aus Incline Design entfernt wird?
confirmations-save-close = Speichern und schließen
confirmations-save-modified-project-before-exiting = Geändertes Projekt vor dem Beenden speichern?
confirmations-save-to-browser-before-exit = Geändertes Projekt vor dem Beenden im Browser-Speicher sichern?
confirmations-save-remove = Speichern und entfernen

## Console strings

console-copy-all = Alles kopieren
console-copy-message = Meldung kopieren
console-error = FEHLER
console-info = INFO
console-no-console-activity-yet = Noch keine Konsolenaktivität
console-pending = AUSSTEHEND
console-progress-summary = In Bearbeitung · { $summary }
console-success = ERFOLG
console-warn = WARNUNG

## Csv strings

csv-block-model-category = Kategorie
csv-block-model-value = Wert

## Drill strings

drill-hole-add-stop = Stopp hinzufügen
drill-hole-all-rendered-intervals-opaque-white = Alle dargestellten Intervalle sind deckend weiß.
drill-hole-burden-spacing-must-greater-than = Vorgabe und Abstand müssen größer als null sein
drill-hole-choose-valid-closed-polyline = Wählen Sie eine gültige geschlossene Polylinie
drill-hole-colour-scale = Farbskala
drill-hole-field = Feld
drill-hole-grayscale = Graustufen
drill-hole-green-yellow-red = Grün–Gelb–Rot
drill-hole-heat = Wärme
drill-hole-no-holes-fit-inside-boundary = Bei der aktuellen Vorgabe und dem Abstand passen keine Löcher innerhalb dieser Grenze
drill-hole-pattern-too-many-holes = Muster überschreitet das Maximum von { $maximum } Löchern; erhöhen Sie Vorgabe oder Abstand
drill-hole-preset = Voreinstellung
drill-hole-px = px
drill-hole-rainbow = Regenbogen
drill-hole-reset-preset = Voreinstellung zurücksetzen
drill-hole-rotation-offsets-must-contain-valid = Drehung und Versätze müssen gültige Zahlen enthalten
drill-hole-selected-polyline-has-no-usable = Die ausgewählte Polylinie hat keine nutzbare XY-Fläche
drill-hole-smooth-interpolation = Glatte Interpolation
drill-hole-spacing-would-scan-too-many = Dieser Abstand würde zu viele Rasterzellen durchsuchen; erhöhen Sie Vorgabe oder Abstand (Maximum { $maximum } Löcher)
drill-hole-square = Quadratisch
drill-hole-staggered = Versetzt
drill-hole-stepped-bands = Gestufte Bänder
common-times-sign = ×
common-minus-sign = −
drill-hole-unsupported-drillhole-source = Nicht unterstützte Bohrlochquelle
drill-hole-width = Breite
drill-pattern-arrangement = Anordnung
drill-pattern-axis-offset = { $axis }-Versatz
drill-pattern-blast-shape = Sprengform
drill-pattern-burden = Vorgabe
drill-pattern-choose-closed-blast-boundary-then = Wählen Sie eine geschlossene Sprenggrenze und passen Sie dann das Raster an. Die Bohrlöcher werden live im Ansichtsfenster aktualisiert.
drill-pattern-closed-design-polyline-whose-xy = Die geschlossene Design-Polylinie, deren XY-Grundfläche mit Löchern gefüllt wird.
drill-pattern-rotation-help = Gegenuhrzeigersinn-Drehung des Musters ab der globalen { $axis }-Achse.
drill-pattern-distance-between-holes-along-each = Abstand zwischen Löchern entlang jeder Musterreihe.
drill-pattern-name-hint = z. B. Westschnitt 03
drill-pattern-diameter-help = Fertiger Lochdurchmesser. In Millimetern eingegeben und bei jedem erzeugten Loch gespeichert.
drill-pattern-hole-depth = Lochtiefe
drill-pattern-hole-diameter = Lochdurchmesser
drill-pattern-move-over-closed-polyline-then = Bewegen Sie den Zeiger über eine geschlossene Polylinie und klicken Sie sie dann im Ansichtsfenster an. Esc bricht die Auswahl ab.
drill-pattern-name-help = Name des im Projekt erstellten Bohrloch-Datensatzes.
drill-pattern-none-picked = Keine ausgewählt
drill-pattern-pattern-name = Musterbezeichnung
drill-pattern-spacing-help = Senkrechter Abstand zwischen Musterreihen.
drill-pattern-pick = Auswählen
drill-pattern-preview-count-hole-s-diameter = Vorschau: { $count } Loch/Löcher · { $diameter } mm Durchmesser · { $depth } m tief
drill-pattern-rotation = Drehung
drill-pattern-shift-pattern-grid-along-global = Verschiebt das Musterraster entlang der globalen { $axis }-Achse, wobei es weiterhin an die Sprengform geklippt bleibt.
drill-pattern-spacing = Abstand
drill-pattern-staggered-offsets-every-second-row = „Versetzt“ verschiebt jede zweite Reihe um die halbe Abstandsbreite.
drill-pattern-vertical-depth-below-each-collar = Vertikale Tiefe unter jedem Ansatzpunkt.

## Dxf strings

dxf-block-nesting-too-deep = DXF-Blockverschachtelung überschreitet maximale Tiefe ({ $depth }), '{ $name }' wird übersprungen
dxf-circular-block-reference = Zirkuläre DXF-Blockreferenz erkannt: '{ $name }'
dxf-undefined-layer = DXF-Entität verwies auf nicht definierte Ebene '{ $name }', importiert als '{ $fallback }'
dxf-import-budget-exceeded = DXF-Import überschreitet das { $what }-Budget ({ $limit }); verbleibende Geometrie wird übersprungen
dxf-insert-unknown-block = DXF-INSERT verweist auf unbekannten Block '{ $name }'

## Edit strings

edit-absolute-length = Absolute Länge
edit-absolute-rl = Absolute Höhenkote
edit-action = Aktion
edit-angle = Winkel
edit-dip-help = Winkel von der Horizontalen, negativ nach unten: -90 ist ein vertikales Loch.
edit-app-web-not-recommended-production = { $app } Web wird für den Produktiveinsatz nicht empfohlen. Verwenden Sie es nur als Demo.
edit-application = Anwendung
edit-apply = Anwenden
edit-apply-pick-target = Anwenden und Ziel wählen
edit-axis-value = { $axis }-Wert
edit-azimuth = Azimut
edit-batter-angle = Böschungswinkel (°)
edit-azimuth-help = Peilung, auf der die Löcher gebohrt werden, in Grad im Uhrzeigersinn ab Gitternord.
edit-bench-height = Strossenhöhe
edit-benches = Strossen
edit-berm-width = Bermenbreite
edit-bezier-curve = Bézierkurve
edit-choose-layer = Wählen Sie eine Ebene
edit-measure-help = Wählen Sie, ob der eingegebene Wert der Abstand entlang der Böschung, die horizontale Breite oder die vertikale Höhe ist.
edit-choose-which-two-polyline-paths = Wählen Sie, welcher der beiden Polylinienpfade zwischen den ausgewählten Eckpunkten ersetzt wird. Die Länge berücksichtigt Höhenkote und gekrümmte Kanten.
edit-click-corner-closed-polyline = Klicken Sie eine Ecke auf einer geschlossenen Polylinie an.
edit-click-open-closed-polyline-begin = Klicken Sie eine offene oder geschlossene Polylinie an, um zu beginnen.
edit-click-second-vertex-replacement-span = Klicken Sie den zweiten Eckpunkt des Ersetzungsbereichs an.
edit-click-vertex-start-replacement-span = Klicken Sie einen Eckpunkt an, um den Ersetzungsbereich zu beginnen.
edit-collide-triangulation = Mit Triangulation kollidieren
edit-confirm-selection = Auswahl bestätigen
edit-control-point-1 = Kontrollpunkt 1
edit-control-point-2 = Kontrollpunkt 2
edit-copy = Kopieren
edit-corner-radius-limited-so-replacement = Eckenradius, begrenzt, damit die Ersetzung benachbarte Eckpunkte nicht überschreitet.
edit-create-new-layer = Neue Ebene erstellen
edit-create-new-project = Neues Projekt erstellen
edit-create-project = Projekt erstellen
edit-delta-length-m-use = Längenänderung (m, + oder − verwenden)
edit-dip = Neigung
edit-direction = Richtung
edit-distance = Abstand
edit-distance-along-slope = Abstand entlang der Böschung
edit-download-free-native-version-our = Laden Sie die kostenlose native Version auf unserer Website herunter ↗
edit-drill-hole = Bohrloch
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = Ende
edit-enter-valid-elevation = Geben Sie eine gültige Höhenkote ein.
edit-exit-slice = Schnittansicht verlassen
edit-finish-polyline = Polylinie abschließen
edit-generate-batter-berms = Böschungen/Bermen erzeugen
edit-height = Höhe
edit-height-change = Höhenänderung
edit-height-mode = Höhenmodus
edit-horizontal-distance = Horizontaler Abstand
edit-horizontal-width-each-flat-berm = Horizontale Breite jeder ebenen Berme zwischen aufeinanderfolgenden Böschungen.
edit-hover-choose-which-end-move = Zum Wählen des zu bewegenden Endes darüberfahren, dann zum Bestätigen klicken.
edit-insert-point-elevation = Punkt auf Höhenkote einfügen
edit-intersect = Schneiden
edit-kind-properties = { $kind } { $properties }
edit-layer-name = Ebenenname
edit-load-project = Projekt laden
edit-longest = Längste
edit-m-s = m/s
edit-measure = Messgröße
edit-mit-license = MIT-Lizenz
edit-mode = Modus
edit-move = Verschieben
edit-move-layer = Auf Ebene verschieben
edit-move-which-end = Welches Ende bewegen
edit-movement-speed-slice-when-using = Bewegungsgeschwindigkeit des Schnitts bei Verwendung der Navigationstasten.
edit-moving-end-endpoint = Verschieben: Endpunkt
edit-moving-start-endpoint = Verschieben: Startpunkt
edit-new-length-m = Neue Länge (m)
edit-new-project = Neues Projekt
edit-number-complete-batter-berm-levels = Anzahl vollständiger Böschungs-/Bermen-Ebenen. Das Maximum ist auf die tiefste Ebene begrenzt, die die angegebene Geometrie beibehält.
edit-bezier-segments-help = Anzahl der Liniensegmente zur Annäherung der Kurve zwischen den beiden ausgewählten Eckpunkten.
edit-chamfer-segments-help = Anzahl der geraden Segmente zur Annäherung der abgerundeten Ecke. Verwenden Sie 1 für eine gerade Fase.
edit-object = Objekt
edit-offset-element = Element versetzen
edit-pick-side = Seite auswählen
edit-pit = Tagebau
edit-project-name = Projektname
edit-properties = Eigenschaften
edit-radius = Radius
edit-recent = Zuletzt verwendet
edit-relative = Relativ (+/-)
edit-elevation-mode-help = „Relativ“ wendet auf jeden Punkt eine vertikale Änderung an. „Absolute Höhenkote“ projiziert jeden Punkt auf eine Zielhöhenkote.
edit-remove-from-list = Aus Liste entfernen
edit-replace-path = Pfad ersetzen
edit-rotate = Drehen
edit-rotation-speed-slice-when-using = Drehgeschwindigkeit des Schnitts bei Verwendung von Q und E.
edit-s = °/s
edit-segments = Segmente
edit-segments-lying-elevation-ignored = Segmente auf dieser Höhenkote werden ignoriert.
edit-endpoint-help = Wählen Sie den zu ändernden Endpunkt; der andere Endpunkt bleibt fest.
edit-selected-holes-point-different-ways = Ausgewählte Löcher zeigen in unterschiedliche Richtungen. „Anwenden“ setzt sie alle auf diese Winkel.
edit-selected-start-end-point-moves = Der ausgewählte Start- oder Endpunkt bewegt sich entlang der Linienrichtung; der gegenüberliegende Endpunkt bleibt fest.
edit-set-axis = { $axis } festlegen
edit-shortest = Kürzeste
edit-slice-view = Schnittansicht
edit-slope-angle-each-batter-face = Neigungswinkel jeder Böschungsfläche, gemessen von der Horizontalen.
edit-slope-angle-offset-positive-negative = Neigungswinkel des Versatzes. Positive und negative Winkel bewegen die Kopie beim seitlichen Versetzen über oder unter die Quelle.
edit-speed = Geschwindigkeit
edit-start = Start
edit-stockpile = Halde
edit-stop-generated-offset-where-its = Den erzeugten Versatz dort stoppen, wo sein Verlauf zuerst auf eine sichtbare Triangulation trifft.
edit-target-rl = Zielhöhenkote
edit-text-colour-opacity = Textfarbe und Deckkraft.
edit-thickness-visible-slice-slab-centred = Dicke der sichtbaren Schnittscheibe, zentriert auf die Übersichtsanzeige.
edit-translation-axis-help = Verschiebungsstrecke entlang der Welt-{ $axis }-Achse.
edit-type = Typ
edit-type-direction-together-set-offset = Typ und Richtung legen zusammen die Versatzseite fest. Tagebau + Auf und Halde + Ab schreiten nach außen; Tagebau + Ab und Halde + Auf schreiten nach innen.
edit-bench-direction-help = „Auf“ hebt jede Strosse um die Strossenhöhe an; „Ab“ senkt sie. Dies kehrt auch die Versatzseite um – siehe Typ.
edit-value-help = Der Wert wird anhand der ausgewählten Messgröße und des Höhenmodus interpretiert.
edit-vertical-rise-fall-each-bench = Vertikaler Anstieg oder Abfall jeder Strosse, bevor die nächste Berme erstellt wird.
edit-bezier-control-point-1-help = Welt-X-, Y- und Z-Koordinaten des ersten Bézier-Kontrollpunkts.
edit-bezier-control-point-2-help = Welt-X-, Y- und Z-Koordinaten des zweiten Bézier-Kontrollpunkts.

## Events strings

events-couldn-t-exit-error = Beenden nicht möglich: { $error }
events-couldn-t-save-error = Speichern nicht möglich: { $error }
events-set-elevation = Höhenkote festlegen
events-set-elevation-from-cursor-hit = Höhenkote vom Zeiger-Treffer auf Z { $z } gesetzt
events-tool-not-available-section-view = Dieses Werkzeug ist in der Schnittansicht nicht verfügbar

## Explorer strings

explorer-clear-active-triangulation-texture = Textur der aktiven Triangulation entfernen
explorer-delete-from-project = Aus Projekt löschen
explorer-discard-changes = Änderungen verwerfen...
explorer-download = Herunterladen
explorer-drape-over-surface = Über Oberfläche drapieren
explorer-draped-over-surface = Über eine Oberfläche drapiert
explorer-duplicate = Duplizieren
explorer-face-colour = Flächenfarbe
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    { $count } Farbvariable(n)
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    { $holes } Bohrloch/Bohrlöcher
    { $fields } Farbfeld(er)
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    { $count } Punkt(e)
explorer-raster-id =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = Laden
explorer-lock = Sperren
explorer-select-all-objects = Alle Objekte auswählen
explorer-source-name = Quelle: { $name }
explorer-unload = Entladen
explorer-unlock = Entsperren

## Files strings

files-automatic-colour = Automatische Farbe
files-automatic-rl-spacing = Automatischer Höhenabstand
files-axis-scale-ratio = { $axis }-Skalierungsverhältnis
files-ok = OK
files-reset-scale = Auf 1× zurücksetzen
files-rl-grid-options = Höhenraster-Optionen
files-rl-spacing = Höhenabstand
files-scales-z-distances-visually-without = Skaliert Z-Abstände visuell, ohne die gespeicherten Koordinaten zu ändern.
files-thickness = Dicke
files-xy-grid-options = XY-Raster-Optionen

## Gpu strings

gpu-cache-block-model-surface-build-failed = Aufbau der Blockmodell-Oberfläche fehlgeschlagen: { $error }
gpu-cache-block-model-surface-build-worker = Worker zum Aufbau der Blockmodell-Oberfläche getrennt
gpu-cache-block-model-surface-chunk-rejected = Blockmodell-Oberflächen-Chunk vor GPU-Zuweisung abgelehnt: instances={ $instances } Bytes, limit={ $limit } Bytes
gpu-cache-block-volume-worker-disconnected = Worker zur Blockvolumen-Vorbereitung getrennt
gpu-cache-translucent-volume-could-not-built = Durchscheinendes Volumen konnte nicht erstellt werden ({ $error }); dieses Blockmodell wird stattdessen als Würfel angezeigt.
gpu-cache-edge-chunk-rejected = Triangulations-Kanten-Chunk vor GPU-Zuweisung abgelehnt: instances={ $instances } Bytes, limit={ $limit } Bytes
gpu-cache-triangulation-chunk-rejected = Triangulations-GPU-Chunk vor Zuweisung abgelehnt: vertices={ $vertices } Bytes, indices={ $indices } Bytes, limit={ $limit } Bytes
gpu-cache-triangulation-too-many-vertices = Triangulation '{ $name }' hat { $count } Eckpunkte (> u32::MAX); kann nicht für GPU aufgeteilt werden
gpu-cache-triangulation-uploaded = Triangulation '{ $name }' in { $chunks } räumlichen Chunks hochgeladen ({ $faces } Flächen)
i18n-active-language = Aktive Sprache ist { $language } (mitgeliefert: { $bundled })
i18n-could-not-select-language-error = Es konnte keine Sprache ausgewählt werden: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = GPU-Adapter: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = GPU-Treiber: { $driver } { $driver_info }
init-gpu-supports-maximum-buffer-size = GPU unterstützt eine maximale Puffergröße von { $size } MiB; große Szenen werden möglicherweise nicht vollständig angezeigt
init-surface-present-mode = Oberflächen-Präsentationsmodus: { $mode }
init-wgpu-error-continuing-error = wgpu-Fehler (wird fortgesetzt): { $error }

## Io strings

io-ascii-points-xyz-pts = ASCII-Punkte (.xyz, .pts)
io-attribute = Attribut
io-blank-header = (leere Kopfzeile)
io-block-model = Blockmodell:
io-choose-file-purpose-map-its = Wählen Sie einen Dateizweck, um dessen Spalten zuzuordnen.
io-choose-loaded-block-model = Wählen Sie ein geladenes Blockmodell
io-choose-loaded-layer = Wählen Sie eine geladene Ebene
io-choose-loaded-triangulation = Wählen Sie eine geladene Triangulation
io-choose-purpose = Zweck wählen…
io-choose-source-file-files-import = Wählen Sie die zu importierende(n) Quelldatei(en).
io-collar = Ansatzpunkt
io-column-mapping = Spaltenzuordnung
io-comma-separated-values-csv = Comma-Separated Values (.csv)
io-csv-files = CSV-Dateien
io-default = Standard
io-depth = Teufe
io-diameter = Durchmesser
io-drawing-exchange-format-dxf = Drawing Exchange Format (.dxf)
io-drill-holes = Bohrlöcher
io-east-x = Ost / X
io-elevation-z = Höhenkote / Z
io-end-x = Ende X
io-end-y = Ende Y
io-end-z = Ende Z
io-explicit-segments = Explizite Segmente
io-export = Export
io-export-csv-block-model = CSV-Blockmodell exportieren
io-export-dxf = DXF exportieren
io-export-one-layer = Eine Ebene exportieren
io-export-ply = PLY exportieren
io-export-stl = STL exportieren
io-export-wavefront-obj = Wavefront OBJ exportieren
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Import
io-import-ascii-point-cloud = ASCII-Punktwolke importieren
io-import-drillhole-csv-bundle = Bohrloch-CSV-Paket importieren
io-import-geotiff = GeoTIFF importieren
io-import-las-laz-point-cloud = LAS/LAZ-Punktwolke importieren
io-import-pcd-point-cloud = PCD-Punktwolke importieren
io-import-ply = PLY importieren
io-import-stl = STL importieren
io-import-wavefront-obj = Wavefront OBJ importieren
io-interval = Intervall
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-mapped-csv-bundle-csv = Zugeordnetes CSV-Paket (.csv)
io-model-file = Modelldatei
io-name-count-files = { $name } + { $count } Dateien
io-no-csv-chosen = Keine .csv ausgewählt
io-no-csv-files-chosen = Keine CSV-Dateien ausgewählt
io-no-dxf-chosen = Keine .dxf ausgewählt
io-no-omf-chosen = Keine .omf ausgewählt
io-north-y = Nord / Y
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-source-file = Quelldatei
io-start-x = Start X
io-start-y = Start Y
io-start-z = Start Z
io-stl = STL (.stl)
io-triangulation = Triangulation:
io-unmapped = Nicht zugeordnet
io-wavefront-obj = Wavefront OBJ (.obj)

## Jobs strings

jobs-background-task-poll-label-ended = Hintergrundaufgabe '{ $poll_label }' endete ohne Ergebnis
jobs-discarded-stale-result = Veraltetes Hintergrundergebnis für '{ $poll_label }' verworfen, da sich eine Quelle geändert hat oder geschlossen wurde

## Logging strings

logging-activity-completed = Aktivität abgeschlossen
logging-activity-started = Aktivität gestartet
logging-application-id-id = Anwendungs-ID: { $id }
logging-application-name = Anwendungsname: { $name }
logging-application-startup = Anwendungsstart
logging-build-target-os-architecture = Build-Ziel: { $os }-{ $architecture }
logging-completed = Abgeschlossen
logging-count-messages = { $count } Meldungen
logging-desktop-session-xdg-session-type = Desktop-Sitzung: XDG_SESSION_TYPE={ $type }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Incline Design wird initialisiert
logging-locale-environment = Gebietsschema-Umgebung: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session = macOS-Sitzung: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = Betriebssystem: GNU/Linux
logging-operating-system-macos = Betriebssystem: macOS
logging-operating-system-microsoft-windows = Betriebssystem: Microsoft Windows
logging-pointer-width = Zeigerbreite: { $width }-Bit
logging-process-id-id = Prozess-ID: { $id }
logging-release-version = Release-Version: { $version }
logging-renderer = Renderer
logging-rust-compiler-host = Rust-Compiler-Host: { $host }
logging-system = System
logging-system-error = Systemfehler
logging-unknown = unbekannt
logging-windows-session-sessionname-session = Windows-Sitzung: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = Wird bearbeitet…

## Mac strings

mac-cannot-install-macos-menu-bar = Die macOS-Menüleiste kann nicht außerhalb des Hauptthreads installiert werden
mac-quit-app = { $app } beenden

## Main strings

main-incline-design-web-startup-failed = Start von Incline Design Web fehlgeschlagen: { $error }

## Menu strings

menu-count-files-selected = { $count } Dateien ausgewählt

## Object strings

object-edit-appearance = Erscheinungsbild
object-edit-arc-circle = Bogen & Kreis
object-edit-arc-segments = Bogensegmente
object-edit-bulge = Ausbuchtung
object-edit-bulge-arcs-horizontal-data-model = Ausbuchtungsbögen sind laut Datenmodell horizontal: Der Bogen verläuft im Grundriss, und die Höhe verändert sich linear von einem Scheitelpunkt zum nächsten.
object-edit-centre-x = Mittelpunkt X
object-edit-centre-y = Mittelpunkt Y
object-edit-centre-z = Mittelpunkt Z
object-edit-chord = Sehne
object-edit-colour-layer = Farbe nach Ebene
object-edit-enter-number = Zahl eingeben
object-edit-follow-owning-layer-s-colour = Der Farbe der übergeordneten Ebene folgen, statt einer an dieses Objekt gebundenen Farbe.
object-edit-id = ID
object-edit-identity = Identität
object-edit-insert-after = Danach einfügen
object-edit-join-last-vertex-back-first = Verbindet den letzten Scheitelpunkt wieder mit dem ersten.
object-edit-length = Länge { $length } m
object-edit-move-down = Nach unten verschieben
object-edit-move-up = Nach oben verschieben
object-edit-object-has-no-arc-segments = Dieses Objekt hat keine Bogensegmente.
object-edit-object-has-single-position = Dieses Objekt hat eine einzelne Position.
object-edit-object-needs-least-required-vertices = Dieses Objekt benötigt mindestens { $required } Scheitelpunkte
object-edit-one-more-properties-not-valid = Eine oder mehrere Eigenschaften sind keine gültige Zahl
object-edit-perimeter-area = Umfang { $length } m, Fläche { $area } m²
object-edit-reverse = Umkehren
object-edit-row-invalid-number = Zeile { $row }: Position oder Ausbuchtung ist keine gültige Zahl
object-edit-sweep = Schwenkwinkel
object-edit-text-not-number = „{ $text }“ ist keine Zahl
object-edit-vertices = Eckpunkte

## Omf strings

omf-element-name-has-count-tie = Element '{ $name }' hat { $count } Verbindung(en), die Löcher benennen, die es nicht mehr enthält
omf-ignoring-colour-map-omf-attribute = Farbtabelle des OMF-Attributs '{ $attribute }' wird ignoriert: { $error }
omf-mining-data-exported-incline = Von Incline exportierte Bergbaudaten
omf-import = OMF-Import
omf-texture = OMF-Textur
omf-validation-warnings = OMF-Validierungswarnungen: { $warnings }
omf-application-metadata-dropped = Projekt-Anwendungsmetadaten '{ $application }' werden nicht beibehalten
omf-project-author-not-retained = Projektautor wird nicht beibehalten
omf-project-description-not-retained = Projektbeschreibung wird nicht beibehalten
omf-unsupported-metadata-keys = Projekt enthält nicht unterstützte Metadatenschlüssel: { $keys }

## Plot strings

plot-1-1000-one-millimetre-sheet = Bei 1:1000 entspricht ein Millimeter auf dem Blatt einem Meter im Gelände.
plot-1-scale-covers-width-height = 1:{ $scale } · deckt { $width } × { $height } m ab
plot-all-visible-data = Alle sichtbaren Daten
plot-automatic-grid-interval = Automatisches Rasterintervall
plot-border = Rand
plot-centre = Zentrieren auf
plot-fit-scale-help = Wählen Sie den kleinsten üblichen Maßstab, bei dem alles Sichtbare auf das Blatt passt.
plot-coordinate-grid = Koordinatengitter
plot-current-view-centre = Aktuelles Ansichtszentrum
plot-date-caps = DATUM
plot-date = Datum
plot-dots-per-inch-paper-size = Punkte pro Zoll. Diese Papiergröße kann bis zu { $max_dpi } dpi gerastert werden; 300 dpi entsprechen normaler Druckqualität.
plot-dpi = dpi
plot-drawing-no = ZEICHNUNGS-NR.
plot-drawing-number = Zeichnungsnummer
plot-drawn-by-caps = GEZEICHNET VON
plot-drawn-by = Gezeichnet von
plot-e-g-example-gold-project = z. B. Beispiel-Goldprojekt
plot-entered-coordinates = Eingegebene Koordinaten
plot-export-png = PNG exportieren...
plot-fit-scale-visible-data = Maßstab an sichtbare Daten anpassen
plot-grid-interval = Rasterintervall
plot-landscape = Querformat
plot-lists-visible-surfaces-design-layers = Listet die sichtbaren Oberflächen und Design-Ebenen mit ihren Farben auf.
plot-margin = Rand
plot-margins-leave-no-room-map = Die Ränder lassen keinen Platz für die Karte
plot-metres-scale-1-scale = Meter    Maßstab 1:{ $scale }
plot-mm = mm
plot-north-arrow = Nordpfeil
plot-nothing-visible-draw = Nichts Sichtbares zum Zeichnen
plot-paper = Papier
plot-paper-orientation-width-height-mm = { $paper } { $orientation } · { $width } × { $height } mm
plot-paper-size = Papiergröße
plot-pick-interval-reads-roughly-every = Wählen Sie ein Intervall, das auf dem gedruckten Blatt etwa alle 50 mm erscheint.
plot-plan = Grundriss
plot-scale-must-be-positive = Der Plotmaßstab muss eine positive Zahl sein
plot-png-written-sheet-s-exact = Die PNG-Datei wird in der exakten Papiergröße des Blatts geschrieben und speichert ihre DPI, sodass sie maßstabsgetreu gedruckt wird.
plot-portrait = Hochformat
plot-resolution = Auflösung
plot-rev = REV.
plot-revision = Revision
plot-scale = MASSSTAB
plot-scale-ratio = Maßstab  1:
plot-scale-framing = Maßstab und Ausschnitt
plot-sheet-furniture = Blattrahmen
plot-size-width-height-mm = { $size } ({ $width } × { $height } mm)
plot-subtitle = Untertitel
plot-title = Titel
plot-title-block = Schriftfeld
plot-today = heute

## Products strings

products-add-initiation = Zündung hinzufügen
products-delay = Verzögerung
products-delay-palette = Verzögerungspalette
products-how-long-after-shot-fired = Wie lange nach der Zündung des Schusses dieser Ansatzpunkt die Runde zündet.
products-initiation-name = Zündung · { $name }
products-milliseconds-between-one-hole-firing = Millisekunden zwischen der Zündung eines Lochs und des nächsten.
products-ms = ms
products-no-products = Keine Produkte
products-remove = Entfernen
products-update = Aktualisieren

## Progress strings

progress-percent-done-total = { $percent } ({ $done } von { $total })
progress-task-finished = { $task }: Abgeschlossen

## Project strings

project-item = Element

## Properties strings

properties-adds-view-dependent-rim-highlight = Fügt an Block- und Materialgrenzen eine blickwinkelabhängige Kantenhervorhebung hinzu. Deaktivieren verringert den Aufwand für die Volumendarstellung geringfügig.
properties-block-model-downscale = Blockmodell-Herunterskalierung
properties-camera = Kamera
properties-camera-clip-planes = Kamera-Clipping-Ebenen
properties-cap-while-resizing = Beim Skalieren begrenzen
properties-dark-mode = Dunkler Modus
properties-developer = Entwickler
properties-downscale-rasters = Raster herunterskalieren
properties-edit-object = Objekt bearbeiten...
properties-field-view = Sichtfeld
properties-fps = FPS
properties-frame-counter = Bildzähler
properties-frame-rate-cap = Bildratenbegrenzung
properties-hz = Hz
properties-interface = Oberfläche
properties-invert-horizontal = Horizontal umkehren
properties-invert-vertical = Vertikal umkehren
properties-limits-newly-loaded-geotiff-previews = Begrenzt neu geladene GeoTIFF-Vorschauen auf 4096 Pixel an ihrer längsten Seite. Deaktivieren, um die volle Auflösung bis zum Texturlimit der GPU zu nutzen, was mehr Speicher verbraucht.
properties-line-colour = Linienfarbe
properties-look-sensitivity = Blickempfindlichkeit
properties-max-clip-span = Max. Clip-Spanne
properties-move-layer = Auf Ebene verschieben...
properties-near-clip-limit = Nahe Clip-Grenze
properties-orbit-sensitivity = Umlaufempfindlichkeit
properties-panel-chrome = Panel-Rahmen
properties-performance = Leistung
properties-plan-mode = Grundrissmodus
properties-presents-step-display-no-tearing = Wird synchron mit dem Bildschirm angezeigt: kein Tearing, und der Bildschirm bestimmt die Bildrate. Bei „Aus“ werden Bilder sofort nach dem Zeichnen angezeigt, und die Obergrenze unten gilt.
properties-reflective-block-edges = Reflektierende Blockkanten
properties-restore-defaults = Standardwerte wiederherstellen
properties-show-console = Konsole anzeigen
properties-shows-live-near-far-projection = Zeigt die aktuellen nahen und fernen Projektionsabstände in der Statusleiste an.
properties-snap-polling = Fangabfrage
properties-vertical-sync = Vertikale Synchronisierung
properties-world-axis-gizmo = Weltachsen-Gizmo
properties-zoom-cursor = Auf Zeiger zoomen
properties-zoom-sensitivity = Zoomempfindlichkeit

## Screenshot strings

screenshot-could-not-encode-viewport-image = Ansichtsbild konnte nicht kodiert werden: { $error }
screenshot-could-not-map-viewport-screenshot = Ansichts-Screenshot konnte nicht zugeordnet werden: { $error }
screenshot-could-not-save-viewport-image = Ansichtsbild { $path } konnte nicht gespeichert werden: { $error }
screenshot-downloaded-viewport-image-file-name = Ansichtsbild heruntergeladen: { $file_name }
screenshot-saved-viewport-image-path = Ansichtsbild gespeichert: { $path }
screenshot-viewport-image-download-failed-error = Herunterladen des Ansichtsbilds fehlgeschlagen: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = BVH-Flächenindex { $index } liegt außerhalb des Bereichs für das Netz; entartetes Dreieck wird eingesetzt

## State strings

state-above = bei oder über
state-activate-project = Projekt aktivieren
state-all-open-incline-design-data = Alle offenen Incline-Design-Daten
state-apply-generated-rings = Erzeugte Ringe anwenden
state-apply-selection = Auf Auswahl anwenden
state-rotate-by-azimuth-dip = mit Azimut { $azimuth }°, Neigung { $dip }°
state-rotate-to-azimuth-dip = zu Azimut { $azimuth }°, Neigung { $dip }°
state-below = bei oder unter
state-centre-rotation = Rotationsmittelpunkt
state-checking-unsaved-work = Ungespeicherte Arbeit wird überprüft
state-choose-destination = Wählen Sie ein Ziel
state-choose-one-more-files = Wählen Sie eine oder mehrere Dateien
state-clear-raster = Raster entfernen
state-click-pit-shell-viewport = Tagebauhülle im Ansichtsfenster anklicken.
state-click-pit-stockpile-solid-viewport = Tagebau- oder Haldenkörper im Ansichtsfenster anklicken.
state-click-surface-viewport = Oberfläche im Ansichtsfenster anklicken.
state-click-topology-viewport = Topologie im Ansichtsfenster anklicken.
state-close-project = Projekt schließen
state-colour-drillholes = Bohrlöcher einfärben
state-copy-objects-layer = Objekte in Ebene kopieren
state-count-file-s = { $count } Datei(en)
state-count-object-s-axis-value = { $count } Objekt(e) · { $axis } { $value }
state-count-object-s-closed = { $count } Objekt(e) · { $closed }
state-count-object-s-layer = { $count } Objekt(e) · { $layer }
state-count-object-s-weight = { $count } Objekt(e) · { $weight }
state-count-object-s-z-elevation = { $count } Objekt(e) · Z { $elevation }
state-create-point-cloud-tin = Punktwolken-TIN erstellen
state-create-project = Projekt erstellen
state-current-project = Aktuelles Projekt
state-cut-topology-pit-shell = Topologie an Tagebauhülle schneiden
state-cut-triangulation-polyline = Triangulation an Polylinie schneiden
state-cut-triangulation-z = Triangulation nach Z schneiden
state-dark-mode = Dunkler Modus
state-detached = Losgelöst
state-disabled = Deaktiviert
state-discard-project-changes = Projektänderungen verwerfen
state-discard-replace-project = Projekt verwerfen und ersetzen
state-discarding-unsaved-changes = Ungespeicherte Änderungen werden verworfen
state-docked = Angedockt
state-drape-raster = Raster drapieren
state-drill-pattern = Bohrmuster
state-duplicate-layer = Ebene duplizieren
state-east = Ost
state-enabled = Aktiviert
state-exit-incline-design = Incline Design beenden
state-export-block-model-csv = Blockmodell-CSV exportieren
state-export-layer-dxf = Ebene als DXF exportieren
state-export-omf = OMF exportieren
state-export-project-dxf = Projekt als DXF exportieren
state-export-triangulation = Triangulation exportieren
state-export-viewport-image = Ansichtsbild exportieren
state-finish-closed-polyline = Geschlossene Polylinie abschließen
state-finish-open-polyline = Offene Polylinie abschließen
state-fit-extents = An Ausdehnung anpassen
state-fix-release-centre-both-views = Legt den Mittelpunkt fest, um den beide Ansichten kreisen, oder gibt ihn frei
state-generate-contours = Höhenlinien erzeugen
state-hidden = Ausgeblendet
state-import-drillholes = Bohrlöcher importieren
state-import-omf = OMF importieren
state-import-point-cloud = Punktwolke importieren
state-import-raster = Raster importieren
state-import-triangulation = Triangulation importieren
state-insert-intersection-points = Schnittpunkte einfügen
state-insert-points-elevation = Punkte auf Höhenkote einfügen
state-keep-inside = Innenbereich behalten
state-keep-outside = Außenbereich behalten
state-kriged-block-model = Gekrigtes Blockmodell
state-load-block-model = Blockmodell laden
state-load-drillholes = Bohrlöcher laden
state-load-layer = Ebene laden
state-load-point-cloud = Punktwolke laden
state-load-raster = Raster laden
state-load-triangulation = Triangulation laden
state-locked-count-object-s = { $count } Objekt(e) gesperrt
state-major-minor = Hauptlinie { $major } · Nebenlinie { $minor }
state-move-axis-value = Auf Achsenwert verschieben
state-move-objects-layer = Objekte in Ebene verschieben
state-name-count-holes = { $name } · { $count } Löcher
state-name-count-object-s = { $name } · { $count } Objekt(e)
state-name-z-min-z-max = { $name } · { $z_min } bis { $z_max }
state-next-edit = Nächste Bearbeitung
state-north = Nord
state-open-containing-folder = Enthaltenden Ordner öffnen
state-open-project = Projekt öffnen
state-preserve-view-angle = Blickwinkel beibehalten
state-previous-edit = Vorherige Bearbeitung
state-project-id = Projekt { $id }
state-remove-block-model = Blockmodell entfernen
state-remove-drillholes = Bohrlöcher entfernen
state-remove-point-cloud = Punktwolke entfernen
state-remove-raster = Raster entfernen
state-remove-triangulation = Triangulation entfernen
state-removed-from-active-triangulation = Aus aktiver Triangulation entfernt
state-removed-from-every-triangulation = Aus jeder Triangulation entfernt
state-rename-kind = { $kind } umbenennen
state-save-close-project = Projekt speichern und schließen
state-save-despite-unsupported-content = Trotz nicht unterstütztem Inhalt speichern
state-save-project = Projekt speichern unter
state-save-replace-project = Projekt speichern und ersetzen
state-saving-current-project = Aktuelles Projekt wird gespeichert
state-section-name = Abschnitt { $section }
state-select-layer-objects = Ebenenobjekte auswählen
state-selected-objects = Ausgewählte Objekte
state-selected-polylines = Ausgewählte Polylinien
state-selected-scene-elements = Ausgewählte Szenenelemente
state-set-block-model-variable = Blockmodellvariable festlegen
state-set-drillhole-colour-preset = Bohrloch-Farbvoreinstellung festlegen
state-set-entity-lock = Entitätssperre festlegen
state-set-grid = Raster festlegen
state-set-layer-lock = Ebenensperre festlegen
state-set-line-weight = Linienstärke festlegen
state-set-object-colour = Objektfarbe festlegen
state-set-object-fill = Objektfüllung festlegen
state-set-point-visibility = Punkt-Sichtbarkeit festlegen
state-set-polyline-closed = Polylinie geschlossen festlegen
state-set-raster-lock = Rastersperre festlegen
state-set-standard-view = Standardansicht festlegen
state-set-topology-wireframes = Topologie-Drahtgitter festlegen
state-set-triangulation-colour = Triangulationsfarbe festlegen
state-show-console = Konsole anzeigen
state-show-project = Projekt anzeigen
state-shown = Eingeblendet
state-slice-mode = Schnittmodus
state-slice-preview = Schnittvorschau
state-south = Süd
state-stem-contours = { $stem } Höhenlinien
state-target-new-name = { $target } in „{ $new_name }“
state-trim-above = Oberhalb zuschneiden
state-trim-below = Unterhalb zuschneiden
state-trim-triangulation-surface = Triangulation auf Oberfläche zuschneiden
state-undrape-raster = Rasterdrapierung entfernen
state-undrape-rasters = Rasterdrapierungen entfernen
state-unload-block-model = Blockmodell entladen
state-unload-drillholes = Bohrlöcher entladen
state-unload-layer = Ebene entladen
state-unload-point-cloud = Punktwolke entladen
state-unload-raster = Raster entladen
state-unload-triangulation = Triangulation entladen
state-untitled-project = Unbenanntes Projekt
state-use-typed-radius = Eingegebenen Radius verwenden
state-west = West

## Status strings

status-clip-near-far = Clip nah/fern/Δ: -- / -- / --
status-frame-rate = Bildrate

## Text strings

text-could-not-build-vector-mesh = Vektornetz für Schrift { $font }, Glyphe { $glyph } konnte nicht erstellt werden: { $error }
text-document-text-mesh-exceeded-its = Textnetz des Dokuments überschritt seinen u32-Indexbereich

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = Wählen Sie zuerst den zu verbindenden Bohrloch-Datensatz
tie-in-count-connector-s = { $count } Verbinder
tie-in-delete-tie-ins = Verbindungen löschen
tie-in-deleted-count-selected-tie-connector = { $count } ausgewählte(r) Verbindungsanschluss/-anschlüsse gelöscht
tie-in-hole = Loch
tie-in-initiation-point-lifted-from-name = Zündpunkt von { $name } entfernt
tie-in-initiation-point-set-name-delay = Zündpunkt auf { $name } bei { $delay } ms gesetzt
tie-in-select-delay-product-palette-before = Wählen Sie ein Verzögerungsprodukt in der Palette aus, bevor Sie Löcher verbinden
tie-in-tied-connectors = { $count } Verbinder bei { $delay } ms mit { $product } verbunden
tie-in-tied-connectors-replacing = { $count } Verbinder bei { $delay } ms mit { $product } verbunden, ersetzt { $replaced }

## Toolbar strings

toolbar-fill-type = Füllungstyp

## Toolbars strings

toolbars-auto-bench = Auto-Strosse
toolbars-bezier-polyline = Bézier-Polylinie
toolbars-chamfer-polyline-corners = Polylinienecken fasen
toolbars-create-text = Text erstellen
toolbars-cursor-regular = Zeiger: Normal
toolbars-cursor-snap-line = Zeiger: An Linie einrasten
toolbars-cursor-snap-point = Zeiger: An Punkt einrasten
toolbars-cursor-snap-surface = Zeiger: An Oberfläche einrasten
toolbars-delete-points = Punkte löschen
toolbars-explode-polyline-lines = Polylinie in Linien auflösen
toolbars-fuse-polylines = Polylinien verschmelzen
toolbars-measure-distance = Abstand messen
toolbars-new-layer = Neue Ebene
toolbars-split-polyline-points = Polylinie an Punkten teilen
toolbars-strike-dip = Streichen und Fallen
toolbars-tool-not-available-section-view = { $tool } – in der Schnittansicht nicht verfügbar

## Tri strings

tri-sampling-method-help = Adaptiv konzentriert Eckpunkte anhand des Ebenenanpassungsfehlers auf komplexem Gelände; gleichmäßig verteilt sie gleichmäßig. Weitere Methoden können künftig hinzugefügt werden.
tri-adaptive-quadtree = Adaptiv (Quadtree)
tri-axis-range = { $axis }-Bereich
tri-base-topology-will-receive-pit = Die Basistopologie, die die Tagebau- oder Haldenform erhält.
tri-boundary-polyline = Begrenzungspolylinie
tri-bridge-gaps-help = Überbrückt Lücken und Randkonkavitäten, die schmaler als dieser Wert über die Oberfläche sind. 0 überbrückt weiterhin Lücken bis etwa zur Größe der Abtastzelle; größere Werte füllen größere Löcher und erodieren Randkonkavitäten.
tri-budget = Budget nach
tri-cancel-pick = Auswahl abbrechen
tri-candidate-detail = Kandidatendetail
tri-candidate-fine-cells-per-budgeted = Kandidaten-Feinzellen pro budgetiertem Eckpunkt. Höhere Werte geben dem adaptiven Sampler mehr Freiheit, Details zu platzieren, sind aber langsamer zu erstellen.
tri-cap-surface-share-source-points = Begrenzen Sie die Oberfläche durch einen Anteil der Quellpunkte oder eine exakte Eckpunktzahl.
tri-choose-input-clicking-loaded-surface = Wählen Sie diese Eingabe, indem Sie eine geladene Oberfläche im Ansichtsfenster anklicken
tri-choose-which-side-reference-topology = Wählen Sie, welche Seite der Referenztopologie innerhalb ihres gemeinsamen XY-Bereichs aus der Oberfläche entfernt wird.
tri-clip = Clip
tri-clip-creates-new-triangulation-name = Der Beschnitt erstellt eine neue Triangulation mit diesem Namen; die Quelloberfläche wird nicht verändert.
tri-clip-surface-polyline = Oberfläche an Polylinie beschneiden
tri-closed-pit-stockpile-solid-whose = Ein geschlossener Tagebau- oder Haldenkörper, dessen freiliegende Begrenzung in das Ergebnis einbezogen wird.
tri-create-new-layer-contours-append = Erstellen Sie eine neue Ebene für die Höhenlinien oder fügen Sie sie einer bestehenden Ebene im aktiven Projekt hinzu.
tri-cut-topology-pit-shell = Topologie mit Tagebauhülle schneiden
tri-e-g-design-trimmed = z. B. design_trimmed
tri-e-g-mysurf-cut = z. B. mysurf_cut
tri-e-g-mysurf-slice = z. B. mysurf_slice
tri-e-g-surface-contour = z. B. surface_contour
tri-e-g-topo-cut = z. B. topo_cut
tri-e-g-topo-pit = z. B. topo_with_pit
tri-exact-number-surface-vertices-target = Exakte Zielzahl von Oberflächen-Eckpunkten. Sehr große Werte werden langsam erstellt und benötigen erheblichen Speicher.
tri-existing-ground-topology-will-cut = Die bestehende Geländetopologie, die von der Tagebauhülle geschnitten wird.
tri-fill-holes-up = Löcher füllen bis zu
tri-generate = Erzeugen
tri-generate-contour-lines = Höhenlinien erzeugen
tri-generate-upper-surface = Obere Oberfläche erzeugen
tri-hide-unload-sources = Quellen ausblenden und entladen
tri-higher-edge-will-enforced-each = Bei jedem Konflikt wird die höhere Kante durchgesetzt. Tiefer liegende, im Konflikt stehende Segmente werden als Bruchlinien ignoriert, und die Oberfläche interpoliert durch diese Bereiche. Die Quellpolylinien bleiben unverändert.
tri-breaklines-cross = Die hervorgehobenen Bruchlinienkanten kreuzen oder überlappen sich im Grundriss auf unterschiedlichen Höhenkoten. Eine Geländeoberfläche kann nicht beiden folgen.
tri-intervals-colours = Intervalle & Farben
tri-keep-clipped-topology-included-shape = Die beschnittene Topologie und die einbezogene Form als separate Triangulationen behalten, anstatt sie zu einer Entität zusammenzuführen.
tri-keep-inside-discards-surface-outside = „Innenbereich behalten“ verwirft die Oberfläche außerhalb der Polylinie. „Außenbereich behalten“ schneidet ein polylinienförmiges Loch aus der Oberfläche.
tri-keeps-only-surface-within-polyline = Behält nur die Oberfläche innerhalb der Polyliniengrenze.
tri-keep-surface-relation-help = Behält die Oberfläche { $relation } der Topologie innerhalb ihrer XY-Abdeckung.
tri-layer-already-exists-select-above = Diese Ebene existiert bereits; wählen Sie sie oben aus oder wählen Sie einen anderen Namen.
tri-limit-z-range = Z-Bereich begrenzen
tri-major = Hauptlinie
tri-max-edge-length = Max. Kantenlänge
tri-merge = Zusammenführen
tri-method = Methode
tri-min = Min.
tri-minimum-maximum-elevations-retained = Minimale und maximale Höhenkoten, die in der Ausgabeoberfläche erhalten bleiben. Das Minimum muss unter dem Maximum liegen.
tri-minor = Nebenlinie
tri-contour-interval-help = „Nebenlinie“ steuert gewöhnliche Höhenlinien. „Hauptlinie“ steuert hervorgehobene Höhenlinien und muss ein Intervall verwenden, das mindestens so groß wie „Nebenlinie“ ist.
tri-move-cursor-over-loaded-surface = Bewegen Sie den Zeiger über eine geladene Oberfläche.
tri-slice-output-name-help = Name, der der höhenbegrenzten Ausgabeoberfläche zugewiesen wird.
tri-name-assigned-merged-topology-pit = Name, der dem zusammengeführten Topologie- und Tagebau-/Halden-Ergebnis zugewiesen wird.
tri-name-assigned-newly-created-contour = Name, der der neu erstellten Höhenlinien-Ebene zugewiesen wird.
tri-reconstruct-output-name-help = Name, der der rekonstruierten Triangulation zugewiesen wird.
tri-name-assigned-topology-after-pit = Name, der der Topologie zugewiesen wird, nachdem die Tagebauhülle davon abgeschnitten wurde.
tri-name-assigned-trimmed-output-surface = Name, der der zugeschnittenen Ausgabeoberfläche zugewiesen wird.
tri-nearby-breakline-vertices-do-not = Benachbarte Bruchlinien-Eckpunkte treffen sich nicht exakt an derselben Position, sodass die Oberfläche nicht trianguliert werden kann.
tri-new-layer = Neue Ebene
tri-new-layer-name = Neuer Ebenenname
tri-once-merge-succeeds-unload-source = Entladen Sie nach erfolgreichem Zusammenführen die Quelltopologie und den Körper, sodass nur das zusammengeführte Ergebnis in der Szene verbleibt.
tri-only-loaded-pickable = Es können nur geladene Triangulationen ausgewählt werden.
tri-operation = Vorgang
tri-output-layer = Ausgabeebene
tri-percentage = Prozentsatz
tri-percentage-cloud = Prozentsatz der Wolke
tri-pick-from-view = Aus Ansicht wählen
tri-pit-design-surface-only-areas = Die Tagebau-Designoberfläche. Nur Bereiche, in denen sie unterhalb der Topologie aushebt, werden für den Schnitt verwendet.
tri-pit-shell = Tagebauhülle
tri-pit-stockpile-solid = Tagebau-/Haldenkörper
tri-recommended-weld-retry = Empfohlen: Verschweißen & Erneut versuchen
tri-reconstruct-help = Rekonstruiert eine triangulierte Geländeoberfläche aus einer Punktwolke. Der adaptive Sampler setzt das Eckpunktbudget dort ein, wo das Gelände am komplexesten ist, und hält ebene Bereiche spärlich.
tri-reduce-budget-candidate-detail-if = Reduzieren Sie das Budget oder das Kandidatendetail, wenn Ihr Rechner weniger RAM hat.
tri-reference-topology-help = Die Referenztopologie, die festlegt, wo die andere Oberfläche zugeschnitten wird.
tri-reject-reconstructed-triangle-edges = Rekonstruierte Dreieckskanten verwerfen, die länger als dieser Abstand sind. Verwenden Sie 0 für kein Kantenlängenlimit.
tri-remove-inside-help = Entfernt die Oberfläche innerhalb der Polyliniengrenze und behält den Rest.
tri-removes-topology-where-pit-shell = Entfernt die Topologie dort, wo die Tagebauhülle darunter aushebt, sodass die Hülle das Loch ausfüllt. Die Naht folgt der tatsächlichen 3D-Kontaktlinie zwischen den Oberflächen; Topologie unter Teilen der Hülle, die über dem Gelände liegen, bleibt erhalten.
tri-result = Ergebnis
tri-save-two-entities = Als zwei Entitäten speichern
tri-select = Auswählen…
tri-share-source-points-keep-fractions = Anteil der zu behaltenden Quellpunkte. Bruchteile wie 0,125 % sind zulässig.
tri-slice-triangulation-z-range = Triangulation nach Z-Bereich schneiden
tri-solution-generate-upper-surface = Lösung: Obere Oberfläche erzeugen
tri-surface-trim = Zuzuschneidende Oberfläche
tri-target-surface-help = Die Oberfläche, die geändert wird; die ausgewählte Topologie bleibt unverändert.
common-percent-suffix = %
tri-topology = Topologie
tri-triangulation-failed = Triangulation fehlgeschlagen
tri-trim = Zuschneiden
tri-trim-topology = Auf Topologie zuschneiden
tri-uniform-grid = Gleichmäßiges Raster
tri-up-target-point-count-points = Bis zu { $target } von { $point_count } Punkten werden zu Oberflächen-Eckpunkten ({ $percent }%).
tri-use-full-surface-elevation-range = Vollständigen Höhenbereich der Oberfläche verwenden
tri-vertex-count = Eckpunktzahl
tri-vertices-within-5-cm-xy = Eckpunkte innerhalb von 5 cm in XY und Z teilen sich für diese Triangulation eine Position. Dies kann die erzeugte Oberfläche lokal um bis zu 5 cm verschieben; die Quellpolylinien bleiben unverändert.
tri-weld-retry = Verschweißen & Erneut versuchen
tri-when-enabled-generate-contours-only = Wenn aktiviert, werden Höhenlinien nur zwischen den angegebenen Mindest- und Höchst-Höhenkoten erzeugt.

## Ui strings

ui-choose-offset-side = Versatzseite wählen
ui-choose-relimit-side = Neubegrenzungsseite wählen
ui-click-circle-centre = Kreismittelpunkt anklicken
ui-click-closed-polyline-use-blast = Klicken Sie eine geschlossene Polylinie als Sprengform an
ui-click-collar-add-edit-initiation = Klicken Sie einen Ansatzpunkt an, um einen Zündpunkt hinzuzufügen oder zu bearbeiten
ui-click-first-point-slice-line = Ersten Punkt der Schnittlinie anklicken
ui-click-first-vertex = Ersten Eckpunkt anklicken
ui-click-perimeter-point-type-radius = Klicken Sie einen Randpunkt an oder geben Sie einen Radius ein
ui-click-second-point-slice-line = Zweiten Punkt der Schnittlinie anklicken
ui-click-second-vertex = Zweiten Eckpunkt anklicken
ui-click-use-pointer-radius = oder klicken, um den Zeigerradius zu verwenden
ui-could-not-copy-text-browser = Text konnte nicht in die Browser-Zwischenablage kopiert werden: { $error }
ui-dip-horizontal-no-strike = { $dip } (horizontal, kein Streichen)
ui-distance-meters = { $distance } Meter
ui-drag-ring-type-azimuth-dip = Ziehen Sie einen Ring oder geben Sie Azimut und Neigung ein
ui-each-hole-turns-about-its = jedes Loch dreht sich um seinen eigenen Ansatzpunkt
ui-enter-positive-decimal-radius = Geben Sie einen positiven Dezimalradius ein
ui-esc-cancels = Esc bricht ab
ui-no-delay-product-tie = Kein Verzögerungsprodukt zum Verbinden
ui-press-enter-use-typed-radius = Eingabetaste drücken, um den eingegebenen Radius zu verwenden
ui-right-click-delay-palette-heading = mit Rechtsklick auf die Überschrift der Verzögerungspalette eines hinzufügen
ui-select-designs = Designs auswählen
ui-select-drill-hole = Wählen Sie ein Bohrloch aus
ui-select-endpoint-join = Wählen Sie den zu verbindenden Endpunkt
ui-select-first-crest-toe-point = Ersten Kronen-/Fußpunkt auswählen
ui-select-item = Wählen Sie ein Element
ui-select-line-fuse = Wählen Sie eine zu verschmelzende Linie
ui-select-line-polyline = Wählen Sie eine Linie oder Polylinie
ui-select-line-relimit = Neu zu begrenzende Linie auswählen
ui-select-next-line-fuse = Wählen Sie die nächste zu verschmelzende Linie
ui-select-opposite-berm-point = Gegenüberliegenden Bermenpunkt auswählen
ui-select-point = Wählen Sie einen Punkt
ui-select-polyline = Wählen Sie eine Polylinie
ui-select-polyline-open-line = Wählen Sie eine Polylinie oder offene Linie
ui-select-polyline-vertex = Wählen Sie einen Polylinien-Eckpunkt
ui-select-second-crest-toe-point = Zweiten Kronen-/Fußpunkt auswählen
ui-select-second-split-point = Zweiten Teilungspunkt auswählen
ui-select-split-point = Wählen Sie einen Teilungspunkt
ui-select-topologies = Topologien auswählen
ui-slice-view = Schnittansicht
ui-strike-dip = { $strike }° Streichen · { $dip }
ui-value-dip = { $value }° Fallen

## Viewport strings

viewport-all-total-categories-keep-their = Alle { $total } Kategorien behalten ihre Farbe; nur die ersten { $shown } werden unterscheidbar dargestellt
viewport-axis-maximum = { $axis }-Maximum
viewport-axis-minimum = { $axis }-Minimum
viewport-bar-blast-timeline-placeholder = Sprengzeitachse [PLATZHALTER]
viewport-bar-burden-relief-heatmap-placeholder = Vorgabe-Entlastungs-Heatmap [PLATZHALTER]
viewport-bar-color = Farbe:
viewport-bar-contours-equal-time-placeholder = Isochronen [PLATZHALTER]
viewport-bar-disable-flying-mode = Flugmodus deaktivieren
viewport-bar-disable-x-ray-vision = Röntgensicht deaktivieren
viewport-bar-drill-holes = Bohrlöcher:
viewport-bar-enable-flying-mode = Flugmodus aktivieren
viewport-bar-enable-x-ray-vision = Röntgensicht aktivieren
viewport-bar-exit-slice-view = Schnittansicht verlassen
viewport-bar-fill = Füllung:
viewport-bar-fix-centre-rotation = Rotationsmittelpunkt festlegen
viewport-bar-hide-points = Punkte ausblenden
viewport-bar-hide-rl-grid = Höhenraster ausblenden
viewport-bar-hide-wireframes = Drahtgitter ausblenden
viewport-bar-hide-xy-grid = XY-Raster ausblenden
viewport-bar-release-centre-rotation = Rotationsmittelpunkt freigeben
viewport-bar-show-points = Punkte anzeigen
viewport-bar-show-rl-grid = Höhenraster einblenden
viewport-bar-show-wireframes = Drahtgitter anzeigen
viewport-bar-show-xy-grid = XY-Raster einblenden
viewport-bar-vertical-slice-view = Vertikale Schnittansicht
viewport-blank = (leer)
viewport-choose-active-block-model-variable = Wählen Sie die aktive Blockmodellvariable
viewport-choose-variable = Wählen Sie eine Variable
viewport-click-edit-color-right-click = Klicken zum Bearbeiten der Farbe, Rechtsklick zum Entfernen
viewport-click-type-boundary-s-value = Klicken, um den Wert dieser Grenze einzugeben
viewport-colour-mapping = Farbzuordnung
viewport-count-categories = { $count } Kategorien
viewport-count-category = { $count } Kategorie
viewport-double-click-add-boundary-here = Doppelklick, um hier eine Grenze hinzuzufügen
viewport-drag-move-middle-click-toggles = Ziehen zum Verschieben · Mittelklick schaltet ≤ um
viewport-drag-move-right-click-remove = Ziehen zum Verschieben · Rechtsklick zum Entfernen · Mittelklick schaltet ≤ um
viewport-e = E
viewport-edit-category-colour = Bearbeiten Sie diese Kategoriefarbe
viewport-edit-colour-used-empty-values = Bearbeiten Sie die Farbe für leere Werte
viewport-empty = (leer)
viewport-empty-hidden = (leer · ausgeblendet)
viewport-filter-variables = Variablen filtern
viewport-navigation-hint = Mittlere Maustaste ziehen zum Schwenken · Scrollen zum Zoomen
viewport-navigation-hint-detach = Mittlere Maustaste ziehen zum Schwenken · Scrollen zum Zoomen · Klicken zum Lösen
viewport-n = N
viewport-no-data-variable = Keine Daten für diese Variable
viewport-no-matches = Keine Treffer
viewport-no-usable-range = (kein nutzbarer Bereich)
viewport-rebuild-variable-s-colours-from = Farben dieser Variablen aus ihren Daten neu erstellen
viewport-reset = Zurücksetzen
viewport-restore-full-model-range = Vollständigen Modellbereich wiederherstellen
