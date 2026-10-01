# Incline — polski katalog komunikatów.
#
# Może być niekompletny: brakujące komunikaty są pobierane z pliku angielskiego
# (`i18n/en/incline_design.ftl`). Identyfikatorów po lewej stronie znaku `=`
# ani nazw argumentów ({ $... }) nie wolno zmieniać — tłumaczy się wyłącznie
# tekst po prawej stronie.

## Wspólne

common-cancel = Anuluj
common-clear = Wyczyść
common-close = Zamknij
common-fill = Wypełnienie
common-set = Ustaw

## Pasek stanu

status-language = Język

## Menu — Plik

menu-file = Plik
menu-file-save-project = Zapisz projekt
menu-file-save-project-as = Zapisz projekt jako...
menu-file-new-project = Nowy projekt...
menu-file-open-project = Otwórz projekt...
menu-file-open-recent = Ostatnio otwierane
menu-file-show-in-explorer = Pokaż w Eksploratorze
menu-file-show-in-folder = Otwórz folder zawierający plik
menu-file-import = Importuj...
menu-file-export = Eksportuj...
menu-file-export-viewport-image = Eksportuj obraz widoku...
menu-file-export-engineering-drawing = Eksportuj rysunek techniczny...
menu-file-about = O programie { $app }...
menu-file-exit = Zakończ działanie aplikacji

## Menu — Widok

menu-view = Widok

## Obszary robocze

ws-production = Produkcja
ws-drill-and-blast = Wiercenie i strzelanie
ws-geology = Geologia
ws-planning = Planowanie

## Paski menu

ws-menubar-design = Projekt
ws-menubar-triangulation = Triangulacja
ws-menubar-raster = Raster
ws-menubar-point-cloud = Chmura punktów
ws-menubar-block-model = Model blokowy
ws-menubar-drillholes = Otwory wiertnicze
ws-menubar-active-layer = Warstwa:

## Funkcje pasków menu

ws-menubar-design-insert-point = Wstaw punkt
ws-menubar-design-insert-point-at-intersection = Na przecięciu
ws-menubar-design-insert-point-at-elevation = Na wysokości
ws-menubar-design-move-to = Przesuń do
ws-menubar-design-create-triangulation = Utwórz triangulację

## Okna zmiany nazwy / usuwania elementu

dialog-rename-title = Zmień nazwę: { $kind }
dialog-rename-field = Nowa nazwa
dialog-rename-field-hint = Wymagane
dialog-rename-submit = Zmień nazwę
dialog-delete-title = Usuń: { $kind }
dialog-delete-confirm =
    Usunąć „{ $name }” z projektu?
    Tej operacji nie można cofnąć.
confirm-delete-product =
    Usunąć produkt „{ $name }” z palety?
    Tej operacji nie można cofnąć.

## Okno „Utwórz triangulację”

tri-create-title = Utwórz triangulację
tri-create-type-label = Typ triangulacji
tri-create-type-help =
    Powierzchnia otwarta tworzy płat w stylu terenu. Bryła tworzy w pełni
    zamkniętą siatkę i wymaga danych wejściowych tworzących szczelną granicę.
tri-create-output-name = Nazwa wyniku
tri-create-output-name-help = Nazwa nadawana wygenerowanej triangulacji.
tri-create-output-name-hint = nazwa triangulacji
tri-create-run = Triangulacja

tri-selection-selected = Wybrano: { $summary }

tri-type-open-surface = Powierzchnia
tri-type-solid-closed = Bryła

tri-count-polylines =
    { $count ->
        [one] { $count } polilinia
        [few] { $count } polilinie
        [many] { $count } polilinii
       *[other] { $count } polilinii
    }
tri-count-strings =
    { $count ->
        [one] { $count } linia
        [few] { $count } linie
        [many] { $count } linii
       *[other] { $count } linii
    }
tri-count-points =
    { $count ->
        [one] { $count } punkt
        [few] { $count } punkty
        [many] { $count } punktów
       *[other] { $count } punktów
    }
tri-count-texts =
    { $count ->
        [one] { $count } obiekt tekstowy
        [few] { $count } obiekty tekstowe
        [many] { $count } obiektów tekstowych
       *[other] { $count } obiektów tekstowych
    }
tri-count-objects =
    { $count ->
        [one] { $count } obiekt
        [few] { $count } obiekty
        [many] { $count } obiektów
       *[other] { $count } obiektów
    }

about-read-full-licence = Przeczytaj pełną licencję ↗
about-source-code = Kod źródłowy
about-website = Strona internetowa
about-title = O programie { $app }
drill-hole-colour-stop = Próg { $index }
properties-restore-defaults-tooltip = Przywróć domyślne ustawienia { $heading }

## Dynamiczne komunikaty interfejsu

ui-selected-count = Wybrano: { $count }
ui-selected-objects = Wybrano obiektów: { $count }
ui-selected-polylines = Wybrano polilinii: { $count }
ui-invalid-axis-value = Podaj prawidłową wartość osi { $axis }.
ui-selection-spans = Zaznaczenie obejmuje zakres od { $min } do { $max }.
confirm-delete-count = Czy na pewno usunąć zaznaczone elementy ({ $count })?
confirm-delete-layer = Usunąć warstwę „{ $name }” wraz ze wszystkimi znajdującymi się na niej obiektami?
    Tej operacji nie można cofnąć.
plot-preview-pixels = { $width } × { $height } px przy { $dpi } dpi
tri-estimated-memory = Szacowane szczytowe zużycie pamięci ~{ $estimate }. { $detail }
block-grid-summary = Siatka: { $x } × { $y } × { $z } = { $count } bloków
status-selected = Zaznaczono: { $count }
status-clip = Bliska/daleka/Δ: { $near } / { $far } / { $delta } m

## Literały źródłowe o wysokiej częstotliwości

## Literały źródłowe

## Dodatkowe literały źródłowe

explorer-no-rasters = Brak rastrów
slice-viewport-gestures = przeciąganie środkowym przyciskiem: przesuwanie · przeciąganie prawym przyciskiem: orbita · Shift+kółko: chodzenie · W/S: przesuń warstwę · Q/E: obrót · Esc: wyjście

## Szczegóły środowiska uruchomieniowego

## Literały źródłowe wykryte przez audyt pokrycia

## Diagnostyka uruchomieniowa renderera

## Wiercenie i strzelanie oraz pozostałe wpisy katalogu literałów

color-aci = ACI
color-aci-value = ACI { $index }
color-index = Indeks
color-rgb = RGB
color-opacity = Nieprzezroczystość
color-edit = Kliknij, aby edytować kolor
color-saturation-value = Nasycenie i jasność
color-hue = Odcień
asset-loading = Wczytywanie danych zasobu
asset-unloading = Wyładowywanie danych zasobu
asset-load-failed = Nie udało się wczytać danych zasobu
asset-unload-failed = Nie udało się wyładować danych zasobu
preferences-title = Preferencje
context-text-colour = Kolor tekstu
context-polylines = Polilinie
context-points = Punkty
crs-unknown-ellipsoid = Nierozpoznany model Ziemi „{ $name }” w tej definicji układu współrzędnych.
crs-no-ellipsoid = Ta definicja układu współrzędnych nie określa używanego modelu Ziemi.
crs-unknown-code = EPSG:{ $code } nie znajduje się w rejestrze układów współrzędnych.
crs-transform-failed = Nie udało się przekonwertować współrzędnej; wynik nie był pozycją skończoną.
crs-no-datum-path = Brak opublikowanej transformacji między układami odniesienia { $from } i { $to } (datum EPSG { $source } i { $target }). Konwersja mimo to byłaby błędna o nieznaną wartość, więc nic nie zostało zmienione.
crs-unknown-datum = Nie można zidentyfikować układu odniesienia { $from } lub { $to }, a oba używają różnych modeli Ziemi. Konwersja między nimi byłaby błędna o nieznaną wartość.
ws-survey = Geodezja
survey-count-designs = { $count } { $count ->
    [one] projekt
    [few] projekty
    [many] projektów
   *[other] projektów
  }
survey-count-meshes = { $count } { $count ->
    [one] triangulacja
    [few] triangulacje
    [many] triangulacji
   *[other] triangulacji
  }
survey-count-models = { $count } { $count ->
    [one] model blokowy
    [few] modele blokowe
    [many] modeli blokowych
   *[other] modeli blokowych
  }
survey-count-clouds = { $count } { $count ->
    [one] chmura punktów
    [few] chmury punktów
    [many] chmur punktów
   *[other] chmur punktów
  }
survey-count-holes = { $count } { $count ->
    [one] zbiór otworów wiertniczych
    [few] zbiory otworów wiertniczych
    [many] zbiorów otworów wiertniczych
   *[other] zbiorów otworów wiertniczych
  }
survey-count-rasters = { $count } { $count ->
    [one] raster
    [few] rastry
    [many] rastrów
   *[other] rastrów
  }
survey-angle = Obrót wokół Z (przeciwnie do ruchu wskazówek zegara)
survey-scale = Jednolity współczynnik skali XYZ
survey-invalid-transform = Początki, kąt i wynikowe współrzędne muszą być skończone.
survey-invalid-scale = Skala musi być skończoną liczbą dodatnią o skończonej odwrotności.
survey-empty-selection = Wybierz co najmniej jeden obsługiwany element do przekształcenia.
survey-unavailable = Wybrany element brakuje lub nie jest wczytany. Wczytaj go przed przekształceniem.
survey-wrong-project = Wybieraj projekty tylko z aktywnego projektu.
survey-name-required = Wprowadź nazwę układu współrzędnych.
survey-working = Przekształcanie wybranych danych…
survey-completed = Przekonwertowano na miejscu: { $items }. Cofnięcie je przywróci.
survey-failed = Przekształcenie nie powiodło się: { $error }
survey-stale = Przekształcenie odrzucone, ponieważ aktywny projekt lub dane źródłowe uległy zmianie. Wybierz dane źródłowe i spróbuj ponownie.
survey-coordinates-menu = Współrzędne
survey-definitions-action = Definicje…
survey-transform-action = Przekształć…
survey-definitions-title = Definicje współrzędnych
survey-transform-title = Przekształć współrzędne
survey-new-system = Nowy układ współrzędnych
survey-new-system-name = Układ współrzędnych
survey-set-local = Ustaw jako układ współrzędnych kopalni
survey-delete-system = Usuń układ współrzędnych
survey-systems-empty = Brak układów współrzędnych
survey-system-name = Nazwa
survey-system-origin = Ten sam punkt — współrzędne układu
survey-angle-help = Przeciwnie do ruchu wskazówek zegara od osi X odniesienia do osi Y odniesienia, patrząc z góry.
survey-scale-help = Jednolita skala XYZ z układu odniesienia do tego układu. Użyj 1, aby zachować wymiary.
survey-close = Zamknij
survey-from = Z
survey-to = Do
survey-transform-button = Przekształć
survey-swap = Zamień
survey-drape-note = Nałożone obrazy są usuwane z przekonwertowanych powierzchni i muszą zostać nałożone ponownie.
survey-needs-grid-block-model = Model blokowy to regularna siatka komórek, a zmiana odwzorowania lub układu odniesienia nie zachowuje tej regularności. Konwersja oznaczałaby ponowne próbkowanie każdej komórki do nowej siatki i utratę zawartych w niej wartości, więc pozostawiono go bez zmian.
survey-needs-grid-raster = Raster jest umieszczany w świecie za pomocą mapowania afinicznego, czego zmiana odwzorowania lub układu odniesienia nie może zachować. Konwersja oznaczałaby ponowne próbkowanie obrazu, więc pozostawiono go bez zmian.
survey-conversion-exact = Dokładna: tylko zmiana siatki, bez reprojekcji.
survey-conversion-accuracy = Podana dokładność { $accuracy } m.
survey-kind = Rodzaj
survey-axis-names = Nazwy osi
survey-kind-registry-short = Układ z rejestru
survey-kind-grid-short = Siatka nad innym układem
survey-registry-search = Szukaj
survey-registry-hint = Nazwa lub kod EPSG, np. „mga zone 56”
survey-registry-none = Nic w rejestrze nie pasuje do wszystkich słów.
survey-parent = Zdefiniowany względem
survey-parent-origin = Znany punkt — współrzędne układu nadrzędnego
survey-pick-registry = Wyszukaj układ i wybierz go z wyników.
survey-pick-parent = Wybierz układ, względem którego zdefiniowana jest ta siatka.
survey-pick-system = Wybierz układ
survey-pick-systems = Wybierz układ źródłowy i docelowy konwersji.
survey-no-selection = Wybierz układ współrzędnych po lewej lub kliknij prawym przyciskiem, aby dodać nowy.
survey-kind-grid = Siatka nad { $parent }
survey-system-in-use = „{ $name }” nie można usunąć: względem niego zdefiniowano { $dependants } { $dependants ->
    [one] układ
    [few] układy
    [many] układów
   *[other] układów
  }. Najpierw przekieruj je na inny układ.
survey-system-cycle = „{ $name }” jest zdefiniowany względem samego siebie, bezpośrednio lub poprzez swoje układy nadrzędne.
survey-system-missing = Ten układ współrzędnych już nie istnieje. Wybierz inną definicję.
survey-same-system = Wybierz różne układy źródłowy i docelowy.
survey-name-exists = Układ współrzędnych o tej nazwie już istnieje. Wybierz go, aby edytować, lub wybierz inną nazwę.

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins oraz współtwórcy Incline Design. Niniejszym udziela się bezpłatnie każdej osobie uzyskującej kopię tego oprogramowania zgody na dysponowanie nim bez ograniczeń, z zastrzeżeniem warunków licencji MIT.

    Incline Design jest dostarczany „TAK JAK JEST”, BEZ JAKIEJKOLWIEK GWARANCJI, WYRAŹNEJ ANI DOROZUMIANEJ, w tym między innymi gwarancji PRZYDATNOŚCI HANDLOWEJ, PRZYDATNOŚCI DO OKREŚLONEGO CELU i NIENARUSZANIA PRAW.
about-free-open-source-mine-design = Darmowe oprogramowanie open source do projektowania kopalń
about-licensed-under-mit-license = Na licencji MIT

## App strings

app-activated-browser-project-name = Aktywowano projekt przeglądarki „{ $name }”.
app-browser-project-delete-failed = Usuwanie projektu przeglądarki nie powiodło się: { $error }
app-browser-project-no-longer-exists = Ten projekt przeglądarki już nie istnieje
app-browser-save-failed-error = Zapis w przeglądarce nie powiódł się: { $error }
app-could-not-activate-browser-project = Nie udało się aktywować projektu przeglądarki: { $error }
app-could-not-delete-browser-project = Nie udało się usunąć projektu przeglądarki: { $error }
app-could-not-load-browser-project = Nie udało się wczytać projektu przeglądarki: { $error }
app-could-not-restore-browser-project = Nie udało się przywrócić projektu przeglądarki: { $error }
app-deleted-browser-project = Usunięto projekt przeglądarki
app-failed-create-window-error = Nie udało się utworzyć okna: { $error }
app-failed-create-window-icon-error = Nie udało się utworzyć ikony okna: { $error }
app-failed-detach-top-down-preview = Nie udało się odłączyć podglądu z góry: { $error }
app-failed-initialize-graphics-error = Nie udało się zainicjować grafiki: { $error }
app-browser-preferences-load-failed = Nie udało się wczytać ustawień przeglądarki: { $error }
app-failed-load-config-file-error = Nie udało się wczytać pliku konfiguracyjnego: { $error }
app-failed-load-session-file-error = Nie udało się wczytać pliku sesji: { $error }
app-failed-rasterize-window-icon-error = Nie udało się zrasteryzować ikony okna: { $error }
app-failed-save-browser-session-error = Nie udało się zapisać sesji przeglądarki: { $error }
app-failed-save-session-error = Nie udało się zapisać sesji: { $error }
app-saved-name-browser-storage = Zapisano „{ $name }” w pamięci przeglądarki

## Block strings

block-model-between = Pomiędzy
block-model-block-grid = Siatka blokowa
block-model-block-size = Rozmiar bloku
block-model-choose-numeric-variable = Wybierz zmienną liczbową
block-model-choose-numeric-variables = Wybierz zmienne liczbowe
block-model-count-variables-selected = Wybrano zmiennych: { $count }
block-model-estimate-variables = Zmienne do oszacowania
block-model-full-x-y-z-dimensions = Pełne wymiary X, Y i Z każdego bloku. Mniejsze bloki zwiększają szczegółowość, czas obliczeń i zużycie pamięci.
block-model-grid-bounds-block-sizes-invalid = Granice siatki lub rozmiary bloków są nieprawidłowe.
block-model-lower-x-y-z-edges = Dolne krawędzie X, Y i Z bryły modelu blokowego. Środki bloków zaczynają się pół bloku wewnątrz tych granic.
block-model-maximum = Maksimum
block-model-maximum-nearest-samples-used-each = Maksymalna liczba najbliższych próbek używanych dla każdego bloku. Niższe wartości działają szybciej; wyższe mogą wygładzać oszacowania i wydłużać czas obliczeń.
block-model-maximum-samples = Maksymalna liczba próbek
block-model-minimum = Minimum
block-model-min-samples-help = Minimalna liczba pobliskich próbek wymaganych do oszacowania bloku. Bloki z mniejszą liczbą próbek w promieniu wyszukiwania pozostają puste.
block-model-minimum-samples = Minimalna liczba próbek
block-model-nugget = Efekt samorodkowy
block-model-numeric-interval-fields-interpolate = Liczbowe pola interwałów do interpolacji. Każde wybrane pole staje się jedną zmienną modelu blokowego.
block-model-kriging-help = Kriging zwyczajny szacuje liczbowe interwały otworów wiertniczych w środku każdego bloku, wykorzystując wariogram sferyczny.
block-model-partial-sill = Częściowy próg wariancji
block-model-range-search-radius = Zasięg / promień wyszukiwania
block-model-range-help = Próbki dalsze niż ta odległość są wykluczane; kowariancja osiąga zero przy tym zasięgu.
block-model-select-all = Zaznacz wszystko
block-model-sill-help = Przestrzennie skorelowana wariancja wnoszona przez model sferyczny. Razem z efektem samorodkowym ustala kowariancję przy zerowej odległości.
block-model-spherical-variogram-search = Wariogram sferyczny i wyszukiwanie
block-model-threshold-at-most = <= próg
block-model-threshold-at-least = >= próg
block-model-threshold-min = Próg / min
block-model-upper-x-y-z-extent = Górny zasięg X, Y i Z do pokrycia. Ostatni blok może wykraczać poza ten zasięg, gdy rozpiętość nie jest dokładną wielokrotnością rozmiaru bloku.
block-model-variable = Zmienna
block-model-variance-effectively-zero-separation = Wariancja przy praktycznie zerowej odległości, spowodowana błędem pomiaru lub zmiennością poniżej skali próbkowania. Użyj zera, gdy nie ma być efektu samorodkowego.
block-model-volume-feedback-disconnected = Odczyt zwrotny wykorzystania objętości bloków rozłączony
block-model-volume-feedback-failed = Odczyt zwrotny wykorzystania objętości bloków nie powiódł się: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z

## Canvas strings

canvas-not-selectable-closed-polyline = Nie można wybrać | Wybierz zamkniętą polilinię
canvas-polyline-summary = Polilinia | Warstwa: { $layer } | { $count } wierzchołków
canvas-surface-name = Powierzchnia | { $name }
canvas-trimmed = Przycięte

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = Utworzono skarpę z bermą z obiektu { $object_id }
cmd-bezier-replaced-polyline-span-first-last = Zastąpiono odcinek polilinii { $first }→{ $last } { $count } próbkowanymi punktami pośrednimi
cmd-bezier-vertices-first-last = Wierzchołki od { $first } do { $last }
cmd-block-model-block-model-loader-disconnected-path = Program wczytujący model blokowy rozłączony dla { $path }
cmd-block-model-block-model-path-has-count = Model blokowy { $path } zawiera { $count } zmiennych nieobsługiwanego typu, które nie będą odczytywalne: { $names }
cmd-block-model-building-ore-mesh = Budowanie siatki złoża…
cmd-block-model-could-not-create-block-model = Nie udało się utworzyć modelu blokowego: { $error }
cmd-block-model-could-not-decode-block-model = Nie udało się zdekodować zmiennej koloru modelu blokowego „{ $variable }”: { $error }
cmd-block-model-created-block-model-name-ordinary = Utworzono model blokowy „{ $name }” metodą krigingu zwyczajnego
cmd-block-model-failed-load-block-model-error = Nie udało się wczytać modelu blokowego: { $error }
cmd-block-model-generated-ore-mesh-from-block = Wygenerowano siatkę złoża z modelu blokowego „{ $name }”
cmd-block-model-imported-block-model-source-path = Zaimportowano źródło modelu blokowego { $path }
cmd-block-model-loaded-block-model-name-blocks = Wczytano model blokowy „{ $name }”: { $blocks } bloków ({ $renderable } renderowalnych), siatka { $dimx }x{ $dimy }x{ $dimz }, { $variables } zmiennych
cmd-block-model-loading-name = Wczytywanie { $name }
cmd-block-model-loading-name-ellipsis = Wczytywanie { $name }…
cmd-chamfer-applied = Sfazowano narożnik { $corner } promieniem { $radius } i { $segments } segmentami
cmd-chamfer-radius = Promień { $radius }
cmd-commands-clipped = Przycięte
cmd-commands-command-failed-error = Polecenie nie powiodło się: { $error }
cmd-commands-select-one-more-objects-before = Wybierz jeden lub więcej obiektów przed ustawieniem { $axis }
cmd-commands-sliced = Przekrojone
cmd-contours-contour-generation-failed-error = Generowanie poziomic nie powiodło się: { $error }
cmd-contours-discarded-layer-exists = Poziomice dla „{ $name }” zostały odrzucone: warstwa „{ $layer_name }” już istnieje
cmd-contours-discarded-project-closed = Poziomice dla „{ $name }” zostały odrzucone: projekt został zamknięty
cmd-contours-discarded-layer-deleted = Poziomice dla „{ $name }” zostały odrzucone: wybrana warstwa wynikowa została usunięta
cmd-contours-generated = Wygenerowano { $line_count } polilinii poziomic dla triangulacji „{ $name }” w warstwie „{ $layer_name }”
cmd-creation-assembled-boundary-rings = Złożono { $assembled_count } zamkniętych pierścieni granicznych z pofragmentowanych linii otwartych
cmd-creation-created-triangulation-from-boundary = Utworzono triangulację z { $boundary_count } pierścieni granicznych i { $constraint_count } otwartych ograniczeń, typ powierzchni { $surface_type }
cmd-creation-creating-triangulation = Tworzenie triangulacji…
cmd-creation-generate-upper-surface-ignored-count = Wygeneruj powierzchnię górną: pominięto { $count } niższych, konfliktowych segmentów linii nieciągłości; obiekty źródłowe pozostają bez zmian
cmd-creation-ignored-objects = Pominięto { $rejected } obiektów niebędących polilinią lub zdegenerowanych podczas triangulacji
cmd-creation-weld-retry-moved-coarse-welded = Zespól i spróbuj ponownie: przesunięto { $coarse_welded } wierzchołków na wspólne pozycje (do { $coarse_weld_tol } m); obiekty źródłowe pozostają bez zmian
cmd-creation-welded-breakline-vertices = Zespolono { $welded } wierzchołków linii nieciągłości pokrywających się w granicach tolerancji
cmd-cuts-clipped-surface-name-polyline-mode = Przycięto powierzchnię „{ $name }” polilinią ({ $mode })
cmd-cuts-clipping-surface-polyline = Przycinanie powierzchni polilinią…
cmd-cuts-cut-topology-name-pit-shell = Przycięto topologię „{ $name }” do powłoki wyrobiska
cmd-cuts-cut-triangulation-name-z-band = Przycięto triangulację „{ $name }” wg pasma Z [{ $min }, { $max }]
cmd-cuts-cutting-topology-pit-shell = Przycinanie topologii powłoką wyrobiska…
cmd-cuts-cutting-triangulation-z = Przycinanie triangulacji wg Z…
cmd-cuts-ignored-vertical-faces = Pominięto { $count } pionowych lub zdegenerowanych ścianek topologii odniesienia bez powierzchni XY
cmd-cuts-site-skipped-constraint-from-x = { $site }: pominięto ograniczenie ({ $from_x }, { $from_y }) -> ({ $to_x }, { $to_y }), którego triangulator nie mógł podzielić
cmd-cuts-skipped-degenerate-edges = { $site }: pominięto { $skipped } niemal zdegenerowanych krawędzi ograniczeń; w ich pobliżu granica cięcia może być przesunięta o włos
cmd-cuts-trimmed-surface = Przycięto powierzchnię „{ $surface }” do topologii „{ $topology }” ({ $mode })
cmd-cuts-trimming-surface-topology = Przycinanie powierzchni do topologii…
cmd-drape-draped-intersected-vertices-changed = Naciągnięto { $intersected } wierzchołków; { $changed } zmieniło wysokość
cmd-drape-no-intersections = Żaden z wybranych wierzchołków projektowych nie przecina wybranych topologii
cmd-drape-objects-changed-object-s-changed = Zmieniono { $objects } obiektów · przesunięto { $changed } z { $intersected } przecinających się wierzchołków
cmd-drape-select-one-more-design-objects = Wybierz jeden lub więcej obiektów projektowych do naciągnięcia
cmd-drape-select-one-more-topologies-drape = Wybierz jedną lub więcej topologii, na które nastąpi naciągnięcie
cmd-drape-selected-topologies-no-longer-loaded = Wybrane topologie nie są już wczytane
cmd-drill-hole-drill-pattern-too-large-contains = Siatka wiertnicza jest zbyt duża lub zawiera nieprawidłowe współrzędne wylotów
cmd-drill-hole-enter-name-drill-pattern = Wpisz nazwę siatki wiertniczej
cmd-drill-hole-failed-load-drillholes-error = Nie udało się wczytać otworów wiertniczych: { $error }
cmd-drill-hole-depth-must-be-positive = Głębokość otworu musi być większa od zera
cmd-drill-hole-diameter-must-be-positive = Średnica otworu musi być większa od zera
cmd-drill-hole-loaded-drillhole-dataset-name-holes = Wczytano zbiór otworów wiertniczych „{ $name }”: { $holes } otworów, { $fields } pól koloru
cmd-drill-hole-pattern-contains-no-holes = Siatka nie zawiera żadnych otworów
cmd-explode-count-line-s = { $count } linii
cmd-explode-polyline = Rozbij polilinię
cmd-explode-exploded-polyline-into-count-line = Rozbito polilinię na { $count } odcinków linii
cmd-file-block-model-csv-encoding-failed = Kodowanie CSV modelu blokowego nie powiodło się: { $error }
cmd-file-block-model-csv-export-failed = Eksport CSV modelu blokowego nie powiódł się: { $error }
cmd-file-browser-recovery-unavailable = Pliki odzyskiwania przeglądarki są niedostępne; zapisane projekty pozostają w IndexedDB
cmd-file-closed-project-runtime-id-runtime = Zamknięto projekt o identyfikatorze roboczym { $runtime_id }
cmd-file-could-not-create-new-project = Nie udało się utworzyć nowego projektu: { $error }
cmd-file-could-not-finish-pending-project = Nie udało się dokończyć oczekującej operacji na projekcie: { $error }
cmd-file-could-not-finish-saving-before = Nie udało się dokończyć zapisu przed zakończeniem: { $error }
cmd-file-could-not-open-browser-project = Nie udało się otworzyć projektu przeglądarki: { $error }
cmd-file-could-not-open-path-error = Nie udało się otworzyć { $path }: { $error }
cmd-file-could-not-read-selected-file = Nie udało się odczytać wybranego pliku: { $error }
cmd-file-could-not-reload-layer-from = Nie udało się ponownie wczytać warstwy z dysku: { $error }
cmd-file-could-not-reload-project-from = Nie udało się ponownie wczytać projektu z dysku: { $error }
cmd-file-could-not-remove-browser-project = Nie udało się usunąć projektu przeglądarki: { $error }
cmd-file-could-not-restore-layer-from = Nie udało się przywrócić warstwy z projektu: { $error }
cmd-file-could-not-snapshot-dirty-project = Nie udało się utworzyć migawki niezapisanego projektu na potrzeby odzyskiwania: { $error }
cmd-file-could-not-start-browser-export = Nie udało się rozpocząć eksportu w przeglądarce: { $error }
cmd-file-could-not-write-recovery-copies = Nie udało się zapisać kopii odzyskiwania: { $error }
cmd-file-created-new-browser-project = Utworzono nowy projekt przeglądarki
cmd-file-created-new-project = Utworzono nowy projekt
cmd-file-description-download-failed-error = Pobieranie { $description } nie powiodło się: { $error }
cmd-file-discard-cancelled-project-changed = Odrzucenie zostało anulowane, ponieważ projekt zmienił się w trakcie ponownego wczytywania OMF
cmd-file-discarded-changes-layer-target-name = Odrzucono zmiany warstwy „{ $target_name }”
cmd-file-discarded-changes-reloaded-path = Odrzucono zmiany: ponownie wczytano { $path }
cmd-file-downloaded-description-file-name = Pobrano { $description }: { $file_name }
cmd-file-dxf-download-encoding-failed-error = Kodowanie pobieranego pliku DXF nie powiodło się: { $error }
cmd-file-dxf-import-failed-error = Import DXF nie powiódł się: { $error }
cmd-file-encoding-block-model-csv-download = Kodowanie pobieranego CSV modelu blokowego…
cmd-file-encoding-dxf-download = Kodowanie pobieranego pliku DXF…
cmd-file-encoding-triangulation-download = Kodowanie pobieranej triangulacji…
cmd-file-exit-deferred-exports = Zakończenie odroczone do czasu ukończenia eksportów w tle
cmd-file-exit-requested-no-unsaved-changes = Zażądano zakończenia bez niezapisanych zmian
cmd-file-exported-block-model-csv-path = Wyeksportowano CSV modelu blokowego do { $path }
cmd-file-exported-description-dxf-path = Wyeksportowano { $description } do DXF: { $path }
cmd-file-exported-triangulation-name-path = Wyeksportowano triangulację „{ $name }” do { $path }
cmd-file-exporting-name = Eksportowanie { $name }…
cmd-file-exporting-triangulation-name-path = Eksportowanie triangulacji „{ $name }” do { $path }
cmd-file-fatal-renderer-failure-reason = Krytyczny błąd renderera: { $reason }
cmd-file-dialog-action-failed = Działanie okna wyboru pliku nie powiodło się: { $msg }
cmd-file-imported-added-object-s-from = Zaimportowano { $added } obiektów z { $name }
cmd-file-imported-total-dxf-object-s = Zaimportowano { $total } obiektów DXF
cmd-file-layer-discard-was-cancelled-because = Odrzucenie zmian warstwy zostało anulowane, ponieważ projekt zmienił się w trakcie ponownego wczytywania
cmd-file-no-recovery-directory = Brak dostępnego katalogu odzyskiwania: { $error }
cmd-file-no-unsaved-project-content-nothing = Brak niezapisanej zawartości projektu; nie ma nic do odzyskania
cmd-file-parsing-browser-dxf-import = Analizowanie importu DXF w przeglądarce…
cmd-file-parsing-dxf-import = Analizowanie importowanego pliku DXF…
cmd-file-project-closes-after-save = Projekt zostanie zamknięty po zakończeniu bieżącego zapisu
cmd-file-the-project-closes-after-save = Projekt zostanie zamknięty po zakończeniu bieżącego zapisu
cmd-file-queued-count-triangulation-file-s = Umieszczono w kolejce { $count } plików triangulacji do importu
cmd-file-recovery-copies-path-reopen-them = Kopie odzyskiwania znajdują się w { $path }; otwórz je ponownie po restarcie
cmd-file-recovery-copy-failed-error = Utworzenie kopii odzyskiwania nie powiodło się: { $error }
cmd-file-recovery-copy-failed-failure = Utworzenie kopii odzyskiwania nie powiodło się: { $failure }
cmd-file-recovery-copy-written-path = Zapisano kopię odzyskiwania: { $path }
cmd-file-reverting-layer = Przywracanie warstwy…
cmd-file-reverting-project = Przywracanie projektu…
cmd-file-save-failed-message = Zapis nie powiódł się: { $message }
cmd-file-save-worker-ended-without-result = Proces zapisu zakończył się bez wyniku
cmd-file-saved-project-as = Zapisano projekt jako: { $path }
cmd-file-saved-project = Zapisano projekt: { $path }
cmd-file-selected-block-model-no-longer = Wybrany model blokowy nie jest już wczytany
cmd-file-switching-project = Przełączanie projektu…
cmd-file-triangulation-download-encoding-failed = Kodowanie pobieranej triangulacji nie powiodło się: { $error }
cmd-file-user-chose-exit-without-saving = Użytkownik wybrał zakończenie bez zapisywania
cmd-file-user-requested-exit-project-export = Użytkownik zażądał zakończenia (wymagane potwierdzenie eksportu projektu lub niezapisanej pracy)
cmd-file-viewport = Widok
cmd-file-wait-current-project-save-finish = Poczekaj na zakończenie bieżącego zapisu projektu
cmd-file-wait-current-project-switch-finish = Poczekaj na zakończenie przełączania bieżącego projektu
cmd-file-wait-project-operation-finish-before = Poczekaj na zakończenie operacji na projekcie przed odrzuceniem zmian
cmd-file-wait-project-revert-finish-before = Poczekaj na zakończenie przywracania projektu przed zapisaniem
cmd-fuse-closed-polyline = Polilinia zamknięta
cmd-fuse-count-source-line-s = { $count } linii źródłowych
cmd-fuse-created-shape-object-id-vertices = Utworzono { $shape } { $object_id } z { $vertices } wierzchołkami z { $sources } linii źródłowych
cmd-fuse-click-missed = Połączenie: kliknięcie nie trafiło w żaden obiekt (nic pod kursorem)
cmd-fuse-click-not-near-endpoint = Połączenie: kliknięcie nie było wystarczająco blisko żadnego z końców wybranej linii
cmd-fuse-clicked-closed-polyline = Połączenie: kliknięty obiekt { $object_id } jest zamkniętą polilinią, łączenie działa tylko na polilinach otwartych
cmd-fuse-clicked-not-open-polyline = Połączenie: kliknięty obiekt { $object_id } nie jest polilinią otwartą (to { $kind })
cmd-fuse-clicked-object-missing = Połączenie: kliknięty obiekt { $object_id } już nie istnieje
cmd-fuse-clicked-too-few-vertices = Połączenie: kliknięta polilinia { $object_id } ma tylko { $count } wierzchołków, potrzeba co najmniej 2
cmd-fuse-endpoint-marker-missing = Połączenie: znacznik końca { $marker_index } już nie istnieje
cmd-fuse-close-needs-three-vertices = Połączenie: linia potrzebuje co najmniej 3 różnych wierzchołków, aby zamknąć się w polilinię (ma { $count })
cmd-fuse-lines = Połącz linie
cmd-fuse-needs-two-segments = Połączenie: potrzeba co najmniej 2 segmentów do zatwierdzenia (jest { $count })
cmd-fuse-no-active-layer = Połączenie: brak aktywnej warstwy do umieszczenia połączonej linii
cmd-fuse-no-active-project = Połączenie: brak aktywnego projektu, nie można zatwierdzić
cmd-fuse-no-source-line = Połączenie: brak linii źródłowej do zamknięcia w polilinię
cmd-fuse-awaiting-object-invalid = Połączenie: obiekt { $awaiting_id } nie jest już prawidłową polilinią
cmd-fuse-object-already-in-chain = Połączenie: obiekt { $object_id } jest już częścią łańcucha połączenia, kliknij inną linię
cmd-fuse-result-too-few-vertices = Połączenie: wynik ma zbyt mało wierzchołków ({ $count }), przerwano
cmd-fuse-segment-object-invalid = Połączenie: segment obiektu { $object_id } nie jest już prawidłową polilinią, przerwano
cmd-fuse-source-object-invalid = Połączenie: obiekt źródłowy { $object_id } nie jest już prawidłową polilinią otwartą
cmd-fuse-source-object-missing = Połączenie: obiekt źródłowy { $object_id } już nie istnieje
cmd-fuse-open-polyline = Polilinia otwarta
cmd-include-failed = Dołączenie nie powiodło się: { $message }
cmd-include-included-solid-shape-name-topology = Dołączono bryłę „{ $shape_name }” do topologii „{ $topology_name }” (zachowano { $retained } ścianek topologii, pominięto { $skipped } ścianek zamykających)
cmd-include-including-pit-stockpile-solid = Dołączanie bryły wyrobiska/zwałowiska…
cmd-insert-point-count-operation-point-s = { $count } punktów { $operation }
cmd-insert-point-elevation-must-be-finite = Wstawienie punktu na wysokości wymaga skończonej wartości wysokości
cmd-insert-point-insert-points = Wstaw punkty
cmd-insert-point-inserted-count-operation-point-s = Wstawiono { $count } punktów { $operation }
cmd-insert-point-intersection = Przecięcie
cmd-insert-point-no-new-operation-points-were = Nie znaleziono nowych punktów operacji { $operation }
cmd-insert-point-select-least-two-polylines-before = Wybierz co najmniej dwie polilinie przed wstawieniem punktów przecięcia
cmd-insert-point-select-one-more-polylines-before = Wybierz jedną lub więcej polilinii przed wstawieniem punktu na wysokości
cmd-layer-created-layer-name = Utworzono warstwę „{ $name }”
cmd-layer-deleted-with-objects = Usunięto warstwę { $layer_id } (oraz wszystkie znajdujące się na niej obiekty)
cmd-layer-duplicated-layer-duplicate-name = Zduplikowano warstwę „{ $duplicate_name }”
cmd-layer-locked = Zablokowany
cmd-layer-name-copy = { $name } kopia
cmd-layer-selected-count-object-s-layer = Zaznaczono { $count } obiektów w warstwie { $layer_id }
cmd-layer-state-layer-name = { $state } warstwę „{ $name }”
cmd-layer-unlocked = Odblokowany
cmd-move-tool-moved-collars = Zastosowano przesunięcie ({ $delta }) do { $count } wylotów otworów
cmd-move-tool-moved-objects = Zastosowano przesunięcie ({ $delta }) do { $count } obiektów
cmd-move-tool-count-hole-s = { $count } otworów
cmd-object-edit-edited-kind = Edytowano { $kind }
cmd-object-edit-edited-kind-count-vertices = Edytowano { $kind } (wierzchołków: { $count })
cmd-object-edit-no-changes-apply = Brak zmian do zastosowania
cmd-object-edit-object-changed-since-editor-opened = Ten obiekt zmienił się od otwarcia edytora; otwórz go ponownie, aby edytować bieżącą wersję
cmd-object-edit-target-changed = Edytowany obiekt uległ zmianie; edycja odrzucona
cmd-object-edit-object-no-longer-exists-document = Ten obiekt już nie istnieje w dokumencie
cmd-object-edit-select-single-design-object-edit = Wybierz jeden obiekt projektu do edycji
cmd-object-edit-unassigned = Nieprzypisane
cmd-offset-create-offset = Utwórz przesunięcie
cmd-offset-created-offset-count-object-s = Utworzono przesunięcie { $count } obiektów
cmd-offset-distance-must-be-positive = Odległość przesunięcia musi być większa od zera
cmd-omf-could-not-open-project-source = Nie udało się otworzyć projektu { $source_name }: { $error }
cmd-omf-create-open-project-before-merging = Utwórz lub otwórz projekt przed scalaniem danych
cmd-omf-encoding-project = Kodowanie projektu…
cmd-omf-exported-project-path = Wyeksportowano projekt do { $path }
cmd-omf-imported-project = Zaimportowano projekt „{ $project_name }” z { $source_name }: { $count } zbiorów danych najwyższego poziomu
cmd-omf-importing-project = Importowanie projektu…
cmd-omf-export-failed = Eksport OMF nie powiódł się: { $error }
cmd-omf-import-failed = Import OMF nie powiódł się: { $error }
cmd-omf-opened-project = Otwarto projekt „{ $project_name }” z { $source_name }
cmd-omf-project-source-name-contains-no = Projekt „{ $source_name }” nie zawiera obsługiwanych elementów danych
cmd-omf-source-name-applied-project-origin = { $source_name }: przed scaleniem zastosowano początek układu projektu { $origin }
cmd-omf-crs-differs = { $source_name }: układ współrzędnych „{ $source_crs }” różni się od układu projektu „{ $target_crs }”; współrzędne scalono bez przeliczenia
cmd-omf-source-name-units-source-units = { $source_name }: jednostki „{ $source_units }” różnią się od jednostek projektu „{ $target_units }”; współrzędne scalono bez przeliczenia
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = Brak otwartych danych Incline Design do wyeksportowania
cmd-placement-2-vertices = 2 wierzchołki
cmd-placement-count-vertices = { $count } wierzchołków
cmd-placement-created-circle = Utworzono okrąg o promieniu { $radius } m
cmd-placement-created-closed-polyline = Utworzono zamkniętą polilinię z { $count } wierzchołkami
cmd-placement-created-line-segment-2-vertices = Utworzono odcinek linii z 2 wierzchołkami
cmd-placement-created-open-polyline-count-vertices = Utworzono otwartą polilinię z { $count } wierzchołkami
cmd-placement-placed-point-x-y-z = Umieszczono punkt w { $x }, { $y }, { $z }
cmd-placement-radius = Promień { $radius } m
cmd-plot-composing-engineering-drawing = Tworzenie rysunku technicznego…
cmd-plot-could-not-write-engineering-drawing = Nie udało się zapisać rysunku technicznego: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Skala rysunku dopasowana do widocznych danych: 1:{ $scale }
cmd-plot = Wydruk
cmd-plot-saved-drawing = Zapisano rysunek techniczny: { $description } ({ $width } × { $height } px przy { $dpi } dpi)
cmd-point-cloud-failed-load-point-cloud-error = Nie udało się wczytać chmury punktów: { $error }
cmd-point-cloud-loaded-point-cloud-name-count = Wczytano chmurę punktów { $name } ({ $count } punktów)
cmd-point-cloud-point-cloud-loader-disconnected-path = Program wczytujący chmurę punktów rozłączony dla { $path }
cmd-point-cloud-tin-max-edge-disabled = (maks. krawędź wyłączona)
cmd-point-cloud-tin-max-edge-max-edge = (maks. krawędź { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = TIN chmury punktów nie powiódł się: { $error }
cmd-point-cloud-tin-subsampled = TIN terenu: przestrzennie podpróbkowano { $sampled } z { $total } punktów
cmd-point-cloud-tin-triangulated = TIN terenu: przetriangulowano { $vertex_count } unikatowych punktów XY na { $face_count } ścianek{ $suffix }
cmd-products-added-product-delay-ms-ms = Dodano produkt { $delay_ms } ms { $name }
cmd-products-deleted-product-delay-ms-ms = Usunięto produkt { $delay_ms } ms { $name }
cmd-products-failed-save-products-error = Nie udało się zapisać produktów: { $error }
cmd-products-product-no-longer-palette = Tego produktu nie ma już w palecie
cmd-property-action-count-object-s-layer = { $action } { $count } obiektów do warstwy { $layer }
cmd-property-batch-set-axis-value-count = Grupowe ustawienie wartości { $axis } dla { $count } obiektów
cmd-property-batch-set-closed-count-polyline = Grupowe ustawienie zamknięcia dla { $count } polilinii
cmd-property-batch-set-color-count-object = Grupowe ustawienie koloru dla { $count } obiektów
cmd-property-batch-set-fill-style-count = Grupowe ustawienie stylu wypełnienia dla { $count } obiektów
cmd-property-batch-set-line-weight-count = Grupowe ustawienie grubości linii dla { $count } polilinii
cmd-property-copied = Skopiowano
cmd-property-moved = Przesunięto
cmd-raster-draped = Nałożono raster { $raster } na triangulację { $triangulation } (zachodzące zasięgi)
cmd-raster-failed-load-raster-name-error = Nie udało się wczytać rastra { $name }: { $error }
cmd-raster-failed-load-raster-path-error = Nie udało się wczytać rastra { $path }: { $error }
cmd-raster-loaded-raster-name-via-driver = Wczytano raster { $name } za pomocą { $driver } ({ $srcx }x{ $srcy }, podgląd { $prevx }x{ $prevy })
cmd-raster-no-overlapping-triangulation = Żadna wczytana triangulacja nie pokrywa się z zasięgiem { $name }
cmd-raster-loader-disconnected = Program wczytujący raster rozłączony dla { $path }
cmd-raster-undraped = Zdjęto rastry z { $count } triangulacji
cmd-relimit-click-missed = Przycięcie: kliknięcie nie trafiło w żaden obiekt (nic pod kursorem)
cmd-relimit-click-ignored = Przycięcie: kliknięcie zignorowane, narzędzie obecnie nie oczekuje na wybór celu
cmd-relimit-clicked-source-line = Przycięcie: kliknięto samą linię źródłową, wybierz inną linię
cmd-relimit-no-source-line = Przycięcie: nie ustawiono linii źródłowej, przerwano wybór
cmd-relimit-relimited-line-source-id-selected = Przycięto linię { $source_id } do wybranego celu
cmd-relimit-resized-line-source-id-using = Zmieniono rozmiar linii { $source_id } metodą { $mode }, wartość { $value }
cmd-rename-item-no-longer-belongs-active = Ten element nie należy już do aktywnego projektu
cmd-rename-renamed-before-name = Zmieniono nazwę „{ $before }” na „{ $name }”
cmd-rename-renamed-name-taken = Zmieniono nazwę „{ $before }” na „{ $name }” („{ $requested }” jest już zajęte)
cmd-rotate-collar-turned-count-drillhole-collar-s = Obrócono { $count } wylotów otworów o { $rotation }
cmd-section-verb-count-item-s-section = { $verb } { $count } elementów w { $section }
cmd-selection-delete-vertex = Usuń wierzchołek
cmd-selection-deleted-count-selected-object-s = Usunięto { $count } zaznaczonych obiektów
cmd-selection-deleted-vertex = Usunięto wierzchołek { $vertex } z polilinii { $object_id }
cmd-selection-duplicate-selection = Duplikuj zaznaczenie
cmd-selection-duplicated-count-object-s = Zduplikowano { $count } obiektów
cmd-session-created-triangulation = Utworzono triangulację „{ $name }” ({ $vertex_count } wierzchołków, { $face_count } ścianek) z powierzchni typu { $surface_type }
cmd-session-deleted-triangulation = Usunięto triangulację „{ $name }” z projektu
cmd-session-failed-load-triangulation-error = Nie udało się wczytać triangulacji: { $error }
cmd-session-failed-load-triangulation-message = Nie udało się wczytać triangulacji: { $message }
cmd-session-loaded-triangulation = Wczytano triangulację „{ $name }” ({ $path }, { $vertex_count } wierzchołków, { $face_count } ścianek)
cmd-session-set-triangulation-tri-id-color = Ustawiono kolor triangulacji { $tri_id } na { $color }
cmd-session-triangulation-load-no-result = Wczytywanie triangulacji dla { $path } zakończyło się bez wyniku
cmd-session-triangulation-failed = Operacja triangulacji nie powiodła się: { $message }
cmd-session-unloaded-triangulation-name = Wyładowano triangulację „{ $name }”
cmd-slice-entered-slice-view-cx-cy = Wejście do widoku przekroju @ { $cx }, { $cy }, { $cz } wzdłuż { $dx }, { $dy } (linia { $length } m)
cmd-slice-exited-slice-view = Zakończono widok przekroju
cmd-slice-reset-section-view-fit-extents = Zresetuj widok przekroju (dopasuj do zasięgu)
cmd-slice-set-section-grid-enabled = Siatka przekroju włączona = { $enabled }
cmd-split-created-2-open-polylines = Utworzono 2 polilinie otwarte
cmd-split-line = Podziel linię
cmd-split-points-needs-interior-vertex = Podział w punktach: wybierz wewnętrzny wierzchołek linii otwartej
cmd-split-polyline-into-two = Podzielono polilinię źródłową na dwie polilinie otwarte
cmd-text-edit-finished = Zakończono edycję tekstu obiektu { $object_id }
cmd-text-updated = Zaktualizowano tekst obiektu { $object_id }
cmd-view-centre-rotation-not-available-flying = Środek obrotu jest niedostępny w trybie lotu
cmd-view-fixed-centre-rotation-x-y = Ustawiono środek obrotu w punkcie { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = Pod kursorem nie ma punktu, na którym można ustawić środek obrotu
cmd-view-released-centre-rotation = Zwolniono środek obrotu
cmd-view-reset-view-fit-extents = Zresetowano widok (dopasowano do zasięgu)
cmd-view-set-topology-wireframes-enabled = Ustawiono siatki krawędziowe topologii = { $enabled }
cmd-view-set-view-points-enabled = Ustawiono widoczność punktów = { $enabled }
cmd-view-set-xy-grid-enabled = Siatka XY włączona = { $enabled }
cmd-view-zoom-extents-preserving-angle = Przybliżono do zasięgu (z zachowaniem kąta)

## Common strings

common-add-product = Dodaj produkt
common-background = Tło
common-block-model = Model blokowy
common-block-models = Modele blokowe
common-cancelled = Anulowano
common-chamfer = Fazowanie
common-choose = Wybierz...
common-circle = Okrąg
common-click-point-fix-centre-rotation = Kliknij punkt, aby ustawić na nim środek obrotu
common-clip-surface-polyline = Przytnij powierzchnię polilinią...
common-closed = Zamknięta
common-colour = Kolor
common-confirm-omf-rewrite = Potwierdź nadpisanie OMF
common-could-not-replace-current-project = Nie udało się zastąpić bieżącego projektu: { $error }
common-count-object-s = { $count } obiektów
common-create = Utwórz
common-create-batter-berm = Utwórz skarpę z bermą
common-create-bezier-curve = Utwórz krzywą Béziera
common-create-block-model = Utwórz model blokowy
common-create-block-model-ellipsis = Utwórz model blokowy...
common-create-circle = Utwórz okrąg
common-create-drill-pattern = Utwórz siatkę wiertniczą
common-create-layer = Utwórz warstwę
common-create-line = Utwórz linię
common-create-ore-triangulation = Utwórz triangulację złoża
common-create-ore-triangulation-ellipsis = Utwórz triangulację złoża...
common-create-point = Utwórz punkt
common-create-polyline = Utwórz polilinię
common-create-triangulation = Utwórz triangulację...
common-crosses = Krzyżyki
common-cut = Wytnij
common-cut-topology-pit-shell = Przytnij topologię powłoką wyrobiska...
common-delete-layer = Usuń warstwę
common-delete-product = Usuń produkt
common-delete-selection = Usuń zaznaczenie
common-designs = Projekty rysunkowe
common-discard-layer-changes = Odrzuć zmiany warstwy
common-down = W dół
common-drape-topology = Naciągnij na topologię
common-easting = Współrzędna X
common-edit-object = Edytuj obiekt
common-edit-text = Edytuj tekst
common-elevation = Wysokość
common-exit-without-saving = Zakończ bez zapisywania
common-export-engineering-drawing = Eksportuj rysunek techniczny
common-filter = Filtr
common-fly-mode = Tryb lotu
common-generate-contour-lines = Generuj poziomice...
common-hide-all = Ukryj wszystko
common-hide-selection = Ukryj zaznaczenie
common-ignore = Ignoruj
common-import-csv-block-model = Importuj model blokowy CSV
common-import-dxf = Importuj DXF
common-incline-design-project = Projekt Incline Design
common-layer = Warstwa
common-legend = Legenda
common-line = Linia
common-line-weight = Grubość linii
common-lock-all = Zablokuj wszystko
common-lock-selection = Zablokuj zaznaczenie
common-m = m
common-max = Maks.
common-merge-shell-into-topology = Scal powłokę z topologią
common-merge-shell-into-topology-ellipsis = Scal powłokę z topologią...
common-move-collar = Przesuń wylot
common-move-design = Przesuń projekt
common-move-selection = Przesuń zaznaczenie
common-new-product = Nowy produkt
common-no-block-models = Brak modeli blokowych
common-no-design-layers = Brak warstw projektowych
common-no-drill-holes = Brak otworów wiertniczych
common-no-file-chosen = Nie wybrano pliku
common-no-open-project = Brak otwartego projektu
common-no-point-clouds = Brak chmur punktów
common-no-triangulations = Brak triangulacji
common-none = Brak
common-northing = Współrzędna Y
common-offset = Przesunięcie
common-open = Otwórz
common-orientation = Orientacja
common-point = Punkt
common-point-cloud = Chmura punktów
common-point-clouds = Chmury punktów
common-polyline = Polilinia
common-polyline-layer = Polilinia na „{ $layer }”
common-project = Projekt
common-rasters = Rastry
common-redo = Ponów
common-relimit-line = Przytnij linię
common-remove-project = Usuń projekt
common-reset-view = Resetuj widok
common-reveal-all = Pokaż wszystko
common-reveal-finder = Pokaż w Finderze
common-rotate-collar = Obróć wylot
common-save-exit = Zapisz i zakończ
common-scale-bar = Podziałka liniowa
common-set-initiation-point = Ustaw punkt inicjacji
common-shape = Kształt
common-shell = Z powłoką
common-slashes = Ukośniki
common-slice = Przekrój
common-slice-triangulation-z-range = Przytnij triangulację wg zakresu Z...
common-surface-contours = Poziomice powierzchni
common-text = Tekst
common-degree-suffix = °
common-tie-holes = Połącz otwory
common-triangulations = Triangulacje
common-trim-topology = Przytnij do topologii...
common-undo = Cofnij
common-undrape-all = Zdejmij wszystkie nakładki
common-uniform-white = Jednolita biel
common-unlock-all = Odblokuj wszystko
common-untitled = Bez nazwy
common-up = W górę
common-vertical-exaggeration = Przewyższenie pionowe
common-x = x
common-zoom-extents = Przybliż do zasięgu

## Confirmations strings

confirmations-close-project-unsaved-changes = Zamknij projekt: niezapisane zmiany
confirmations-close-without-saving = Zamknij bez zapisywania
confirmations-delete = Usuń
confirmations-delete-objects = Usuń obiekty
confirmations-discard = Odrzuć
confirmations-discard-all-unsaved-changes-layer =
    Odrzucić wszystkie niezapisane zmiany w warstwie „{ $name }”?
    Zapisana warstwa zostanie ponownie wczytana z dysku, a zmiany w innych warstwach zostaną zachowane. Tej operacji nie można cofnąć.
confirmations-discard-all-unsaved-changes-name =
    Odrzucić wszystkie niezapisane zmiany w „{ $name }”?
    Ostatnio zapisana wersja zostanie ponownie wczytana z dysku. Tej operacji nie można cofnąć.
confirmations-discard-changes = Odrzuć zmiany
confirmations-exit-unsaved-changes = Zakończenie: niezapisane zmiany
confirmations-incline-design-cannot-reproduce-all = Incline Design nie jest w stanie odtworzyć całej zawartości oryginalnego pliku OMF. Zapis pominie następującą zawartość:
confirmations-product = Produkt
confirmations-project = tego projektu
confirmations-remove-name-delete-its-browser = Usunąć „{ $name }” i skasować jego kopię przechowywaną w przeglądarce? Niezapisane zmiany zostaną utracone.
confirmations-remove-project-unsaved-changes = Usuń projekt: niezapisane zmiany
confirmations-remove-without-saving = Usuń bez zapisywania
confirmations-replace-project-unsaved-changes = Zastąp projekt: niezapisane zmiany
confirmations-save = Zapisz
confirmations-save-anyway = Zapisz mimo to
confirmations-save-changes-current-project-before = Zapisać zmiany w bieżącym projekcie przed jego zastąpieniem?
confirmations-save-changes-name-before-closing = Zapisać zmiany w „{ $name }” przed jego zamknięciem?
confirmations-save-changes-name-before-removing = Zapisać zmiany w „{ $name }” przed usunięciem go z Incline Design?
confirmations-save-close = Zapisz i zamknij
confirmations-save-modified-project-before-exiting = Zapisać zmodyfikowany projekt przed zakończeniem?
confirmations-save-to-browser-before-exit = Zapisać zmodyfikowany projekt w pamięci przeglądarki przed zakończeniem?
confirmations-save-remove = Zapisz i usuń

## Console strings

console-copy-all = Kopiuj wszystko
console-copy-message = Kopiuj komunikat
console-error = BŁĄD
console-info = INFO
console-no-console-activity-yet = Brak aktywności w konsoli
console-pending = OCZEKUJE
console-progress-summary = W trakcie · { $summary }
console-success = SUKCES
console-warn = OSTRZEŻENIE

## Csv strings

csv-block-model-category = Kategoria
csv-block-model-value = Wartość

## Drill strings

drill-hole-add-stop = Dodaj próg
drill-hole-all-rendered-intervals-opaque-white = Wszystkie renderowane interwały są nieprzezroczyście białe.
drill-hole-burden-spacing-must-greater-than = Odprężenie i rozstaw muszą być większe od zera
drill-hole-choose-valid-closed-polyline = Wybierz prawidłową zamkniętą polilinię
drill-hole-colour-scale = Skala kolorów
drill-hole-field = Pole
drill-hole-grayscale = Skala szarości
drill-hole-green-yellow-red = Zielony–żółty–czerwony
drill-hole-heat = Ciepło
drill-hole-no-holes-fit-inside-boundary = Przy bieżącym odprężeniu i rozstawie żaden otwór nie mieści się w tej granicy
drill-hole-pattern-too-many-holes = Siatka przekracza maksimum { $maximum } otworów; zwiększ odprężenie lub rozstaw
drill-hole-preset = Ustawienie
drill-hole-px = px
drill-hole-rainbow = Tęcza
drill-hole-reset-preset = Resetuj ustawienie
drill-hole-rotation-offsets-must-contain-valid = Obrót i przesunięcia muszą zawierać prawidłowe liczby
drill-hole-selected-polyline-has-no-usable = Wybrana polilinia nie ma użytecznej powierzchni XY
drill-hole-smooth-interpolation = Interpolacja wygładzająca
drill-hole-spacing-would-scan-too-many = Ten rozstaw wymagałby przeskanowania zbyt wielu komórek siatki; zwiększ odprężenie lub rozstaw (maksimum { $maximum } otworów)
drill-hole-square = Kwadratowy
drill-hole-staggered = Przestawny
drill-hole-stepped-bands = Pasma stopniowane
common-times-sign = ×
common-minus-sign = −
drill-hole-unsupported-drillhole-source = Nieobsługiwane źródło otworów wiertniczych
drill-hole-width = Szerokość
drill-pattern-arrangement = Układ
drill-pattern-axis-offset = Przesunięcie { $axis }
drill-pattern-blast-shape = Kształt strzelania
drill-pattern-burden = Odprężenie
drill-pattern-choose-closed-blast-boundary-then = Wybierz zamkniętą granicę strzelania, a następnie dostosuj siatkę. Otwory wiertnicze aktualizują się na bieżąco w widoku.
drill-pattern-closed-design-polyline-whose-xy = Zamknięta polilinia projektowa, której obrys XY zostanie wypełniony otworami.
drill-pattern-rotation-help = Obrót wzoru w kierunku przeciwnym do ruchu wskazówek zegara od globalnej osi { $axis }.
drill-pattern-distance-between-holes-along-each = Odległość między otworami wzdłuż każdego rzędu siatki.
drill-pattern-name-hint = np. Zachodni Odkrywka 03
drill-pattern-diameter-help = Docelowa średnica otworu. Wprowadzana w milimetrach i zapisywana przy każdym wygenerowanym otworze.
drill-pattern-hole-depth = Głębokość otworu
drill-pattern-hole-diameter = Średnica otworu
drill-pattern-move-over-closed-polyline-then = Najedź na zamkniętą polilinię, a następnie kliknij ją w widoku. Esc anuluje wybór.
drill-pattern-name-help = Nazwa zbioru otworów wiertniczych utworzonego w projekcie.
drill-pattern-none-picked = Nic nie wybrano
drill-pattern-pattern-name = Nazwa siatki
drill-pattern-spacing-help = Odległość prostopadła między rzędami siatki.
drill-pattern-pick = Wybierz
drill-pattern-preview-count-hole-s-diameter = Podgląd: { $count } otworów · średnica { $diameter } mm · głębokość { $depth } m
drill-pattern-rotation = Obrót
drill-pattern-shift-pattern-grid-along-global = Przesuwa siatkę wzoru wzdłuż globalnej osi { $axis }, zachowując jej przycięcie do kształtu strzału.
drill-pattern-spacing = Rozstaw
drill-pattern-staggered-offsets-every-second-row = Układ przestawny przesuwa co drugi rząd o połowę rozstawu.
drill-pattern-vertical-depth-below-each-collar = Głębokość pionowa pod każdym wylotem.

## Dxf strings

dxf-block-nesting-too-deep = Zagnieżdżenie bloków DXF przekracza maksymalną głębokość ({ $depth }), pominięto „{ $name }”
dxf-circular-block-reference = Wykryto cykliczne odwołanie bloku DXF: „{ $name }”
dxf-undefined-layer = Obiekt DXF odwoływał się do niezdefiniowanej warstwy „{ $name }”, zaimportowano jako „{ $fallback }”
dxf-import-budget-exceeded = Import DXF przekracza limit { $what } ({ $limit }); pozostała geometria zostaje pominięta
dxf-insert-unknown-block = Wstawienie DXF odwołuje się do nieznanego bloku „{ $name }”

## Edit strings

edit-absolute-length = Długość bezwzględna
edit-absolute-rl = Bezwzględna RL
edit-action = Działanie
edit-angle = Kąt
edit-dip-help = Kąt od poziomu, ujemny w dół: -90 to otwór pionowy.
edit-app-web-not-recommended-production = { $app } Web nie jest zalecany do zastosowań produkcyjnych. Używaj go wyłącznie jako wersji demonstracyjnej.
edit-application = Aplikacja
edit-apply = Zastosuj
edit-apply-pick-target = Zastosuj i wskaż cel
edit-axis-value = Wartość { $axis }
edit-azimuth = Azymut
edit-batter-angle = Kąt skarpy (°)
edit-azimuth-help = Azymut, wg którego wiercone są otwory, w stopniach zgodnie z ruchem wskazówek zegara od północy siatki.
edit-bench-height = Wysokość piętra
edit-benches = Piętra
edit-berm-width = Szerokość bermy
edit-bezier-curve = Krzywa Béziera
edit-choose-layer = Wybierz warstwę
edit-measure-help = Wybierz, czy wprowadzona wartość to odległość wzdłuż skarpy, szerokość pozioma czy wysokość pionowa.
edit-choose-which-two-polyline-paths = Wybierz, która z dwóch tras polilinii między wybranymi wierzchołkami zostanie zastąpiona. Długość uwzględnia wysokość i zakrzywione krawędzie.
edit-click-corner-closed-polyline = Kliknij narożnik na zamkniętej polilinii.
edit-click-open-closed-polyline-begin = Kliknij otwartą lub zamkniętą polilinię, aby rozpocząć.
edit-click-second-vertex-replacement-span = Kliknij drugi wierzchołek zastępowanego odcinka.
edit-click-vertex-start-replacement-span = Kliknij wierzchołek, aby rozpocząć zastępowany odcinek.
edit-collide-triangulation = Kolizja z triangulacją
edit-confirm-selection = Potwierdź wybór
edit-control-point-1 = Punkt kontrolny 1
edit-control-point-2 = Punkt kontrolny 2
edit-copy = Kopiuj
edit-corner-radius-limited-so-replacement = Promień narożnika, ograniczony tak, aby zastąpienie nie mogło przekroczyć sąsiednich wierzchołków.
edit-create-new-layer = Utwórz nową warstwę
edit-create-new-project = Utwórz nowy projekt
edit-create-project = Utwórz projekt
edit-delta-length-m-use = Zmiana długości (m, użyj + lub -)
edit-dip = Upad
edit-direction = Kierunek
edit-distance = Odległość
edit-distance-along-slope = Odległość wzdłuż skarpy
edit-download-free-native-version-our = Pobierz bezpłatną wersję natywną na naszej stronie internetowej ↗
edit-drill-hole = Otwór wiertniczy
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = Koniec
edit-enter-valid-elevation = Wpisz prawidłową wysokość.
edit-exit-slice = Wyjdź z przekroju
edit-finish-polyline = Zakończ polilinię
edit-generate-batter-berms = Generuj skarpy z bermami
edit-height = Wysokość
edit-height-change = Zmiana wysokości
edit-height-mode = Tryb wysokości
edit-horizontal-distance = Odległość pozioma
edit-horizontal-width-each-flat-berm = Szerokość pozioma każdej płaskiej bermy między kolejnymi skarpami.
edit-hover-choose-which-end-move = Najedź kursorem, aby wybrać przesuwany koniec, a następnie kliknij, aby potwierdzić.
edit-insert-point-elevation = Wstaw punkt na wysokości
edit-intersect = Przecięcie
edit-kind-properties = { $kind } – { $properties }
edit-layer-name = Nazwa warstwy
edit-load-project = Wczytaj projekt
edit-longest = Najdłuższa
edit-m-s = m/s
edit-measure = Zmierz
edit-mit-license = Licencja MIT
edit-mode = Tryb
edit-move = Przesuń
edit-move-layer = Przenieś do warstwy
edit-move-which-end = Który koniec przesunąć
edit-movement-speed-slice-when-using = Prędkość przesuwania przekroju przy użyciu klawiszy nawigacji.
edit-moving-end-endpoint = Przesuwanie: punkt końcowy
edit-moving-start-endpoint = Przesuwanie: punkt początkowy
edit-new-length-m = Nowa długość (m)
edit-new-project = Nowy projekt
edit-number-complete-batter-berm-levels = Liczba pełnych poziomów skarp i berm. Maksimum jest ograniczone do najgłębszego poziomu zachowującego zadaną geometrię.
edit-bezier-segments-help = Liczba odcinków linii używanych do przybliżenia krzywej między dwoma wybranymi wierzchołkami.
edit-chamfer-segments-help = Liczba prostych segmentów używanych do przybliżenia zaokrąglonego narożnika. Użyj 1 dla prostego fazowania.
edit-object = Obiekt
edit-offset-element = Przesuń element
edit-pick-side = Wybierz stronę
edit-pit = Wyrobisko
edit-project-name = Nazwa projektu
edit-properties = Właściwości
edit-radius = Promień
edit-recent = Ostatnie
edit-relative = Względne (+/-)
edit-elevation-mode-help = Względne stosuje zmianę pionową do każdego punktu. Bezwzględna RL rzutuje każdy punkt na jedną docelową wysokość.
edit-remove-from-list = Usuń z listy
edit-replace-path = Zastąp ścieżkę
edit-rotate = Obróć
edit-rotation-speed-slice-when-using = Prędkość obrotu przekroju przy użyciu klawiszy Q i E.
edit-s = °/s
edit-segments = Segmenty
edit-segments-lying-elevation-ignored = Segmenty leżące na tej wysokości są pomijane.
edit-endpoint-help = Wybierz punkt końcowy, który się zmienia; drugi koniec pozostaje nieruchomy.
edit-selected-holes-point-different-ways = Zaznaczone otwory są skierowane w różne strony. Zastosuj ustawi je wszystkie na te kąty.
edit-selected-start-end-point-moves = Wybrany punkt początkowy lub końcowy przesuwa się wzdłuż kierunku linii; przeciwległy koniec pozostaje nieruchomy.
edit-set-axis = Ustaw { $axis }
edit-shortest = Najkrótsza
edit-slice-view = Widok przekroju
edit-slope-angle-each-batter-face = Kąt nachylenia każdej ściany skarpy, mierzony od poziomu.
edit-slope-angle-offset-positive-negative = Kąt nachylenia przesunięcia. Kąty dodatnie i ujemne przesuwają kopię powyżej lub poniżej źródła w miarę przesuwania na boki.
edit-speed = Prędkość
edit-start = Początek
edit-stockpile = Zwałowisko
edit-stop-generated-offset-where-its = Zatrzymaj wygenerowane przesunięcie tam, gdzie jego trasa po raz pierwszy napotka widoczną triangulację.
edit-target-rl = Docelowa RL
edit-text-colour-opacity = Kolor i nieprzezroczystość tekstu.
edit-thickness-visible-slice-slab-centred = Grubość widocznej warstwy przekroju wyśrodkowanej na wskaźniku przeglądu.
edit-translation-axis-help = Odległość przesunięcia wzdłuż globalnej osi { $axis }.
edit-type = Typ
edit-type-direction-together-set-offset = Typ i Kierunek razem określają stronę przesunięcia. Wyrobisko + Góra i Zwałowisko + Dół przesuwają się na zewnątrz; Wyrobisko + Dół i Zwałowisko + Góra przesuwają się do wewnątrz.
edit-bench-direction-help = Góra podnosi każde piętro o jego wysokość; Dół je obniża. Zmienia to również stronę przesunięcia — zobacz Typ.
edit-value-help = Wartość jest interpretowana zgodnie z wybraną miarą i trybem wysokości.
edit-vertical-rise-fall-each-bench = Pionowy wznios lub spadek każdego piętra przed utworzeniem kolejnej bermy.
edit-bezier-control-point-1-help = Globalne współrzędne X, Y i Z pierwszego punktu kontrolnego Béziera.
edit-bezier-control-point-2-help = Globalne współrzędne X, Y i Z drugiego punktu kontrolnego Béziera.

## Events strings

events-couldn-t-exit-error = Nie można zakończyć: { $error }
events-couldn-t-save-error = Nie można zapisać: { $error }
events-set-elevation = Ustaw wysokość
events-set-elevation-from-cursor-hit = Ustawiono wysokość z trafienia kursora na Z { $z }
events-tool-not-available-section-view = To narzędzie jest niedostępne w widoku przekroju

## Explorer strings

explorer-clear-active-triangulation-texture = Wyczyść teksturę aktywnej triangulacji
explorer-delete-from-project = Usuń z projektu
explorer-discard-changes = Odrzuć zmiany...
explorer-download = Pobierz
explorer-drape-over-surface = Naciągnij na powierzchnię
explorer-draped-over-surface = Nałożone na powierzchnię
explorer-duplicate = Duplikuj
explorer-face-colour = Kolor ścianki
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    { $count } zmiennych koloru
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    { $holes } otworów
    { $fields } pól koloru
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    { $count } punktów
explorer-raster-id =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = Wczytaj
explorer-lock = Zablokuj
explorer-select-all-objects = Zaznacz wszystkie obiekty
explorer-source-name = Źródło: { $name }
explorer-unload = Wyładuj
explorer-unlock = Odblokuj

## Files strings

files-automatic-colour = Kolor automatyczny
files-automatic-rl-spacing = Automatyczny odstęp rzędnych
files-axis-scale-ratio = Współczynnik skali { $axis }
files-ok = OK
files-reset-scale = Resetuj do 1×
files-rl-grid-options = Opcje siatki rzędnych
files-rl-spacing = Odstęp rzędnych
files-scales-z-distances-visually-without = Skaluje wizualnie odległości Z bez zmiany zapisanych współrzędnych.
files-thickness = Grubość
files-xy-grid-options = Opcje siatki XY

## Gpu strings

gpu-cache-block-model-surface-build-failed = Budowa powierzchni modelu blokowego nie powiodła się: { $error }
gpu-cache-block-model-surface-build-worker = Proces budowy powierzchni modelu blokowego rozłączony
gpu-cache-block-model-surface-chunk-rejected = Odrzucono fragment powierzchni modelu blokowego przed alokacją GPU: instances={ $instances } B, limit={ $limit } B
gpu-cache-block-volume-worker-disconnected = Proces przygotowania objętości bloków rozłączony
gpu-cache-translucent-volume-could-not-built = Nie udało się zbudować półprzezroczystej objętości ({ $error }); zamiast tego pokazano ten model blokowy jako kostki.
gpu-cache-edge-chunk-rejected = Odrzucono fragment krawędzi triangulacji przed alokacją GPU: instances={ $instances } B, limit={ $limit } B
gpu-cache-triangulation-chunk-rejected = Odrzucono fragment GPU triangulacji przed alokacją: vertices={ $vertices } B, indices={ $indices } B, limit={ $limit } B
gpu-cache-triangulation-too-many-vertices = Triangulacja „{ $name }” ma { $count } wierzchołków (> u32::MAX); nie można podzielić na fragmenty dla GPU
gpu-cache-triangulation-uploaded = Triangulację „{ $name }” przesłano w { $chunks } fragmentach przestrzennych ({ $faces } ścianek)
i18n-active-language = Aktywny język to { $language } (wbudowany: { $bundled })
i18n-could-not-select-language-error = Nie udało się wybrać języka: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = Adapter GPU: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = Sterownik GPU: { $driver } { $driver_info }
init-gpu-supports-maximum-buffer-size = GPU obsługuje maksymalny rozmiar bufora { $size } MiB; duże sceny mogą nie wyświetlać się w całości
init-surface-present-mode = Tryb prezentacji powierzchni: { $mode }
init-wgpu-error-continuing-error = Błąd wgpu (kontynuowanie): { $error }

## Io strings

io-ascii-points-xyz-pts = Punkty ASCII (.xyz, .pts)
io-attribute = Atrybut
io-blank-header = (pusty nagłówek)
io-block-model = Model blokowy:
io-choose-file-purpose-map-its = Wybierz przeznaczenie pliku, aby zmapować jego kolumny.
io-choose-loaded-block-model = Wybierz wczytany model blokowy
io-choose-loaded-layer = Wybierz wczytaną warstwę
io-choose-loaded-triangulation = Wybierz wczytaną triangulację
io-choose-purpose = Wybierz przeznaczenie…
io-choose-source-file-files-import = Wybierz plik lub pliki źródłowe do zaimportowania.
io-collar = Wylot otworu
io-column-mapping = Mapowanie kolumn
io-comma-separated-values-csv = Comma-Separated Values (.csv)
io-csv-files = Pliki CSV
io-default = Domyślne
io-depth = Głębokość
io-diameter = Średnica
io-drawing-exchange-format-dxf = Drawing Exchange Format (.dxf)
io-drill-holes = Otwory wiertnicze
io-east-x = Wschód / X
io-elevation-z = Wysokość / Z
io-end-x = X końca
io-end-y = Y końca
io-end-z = Z końca
io-explicit-segments = Jawne segmenty
io-export = Eksportuj
io-export-csv-block-model = Eksportuj model blokowy CSV
io-export-dxf = Eksportuj DXF
io-export-one-layer = Eksportuj jedną warstwę
io-export-ply = Eksportuj PLY
io-export-stl = Eksportuj STL
io-export-wavefront-obj = Eksportuj Wavefront OBJ
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Importuj
io-import-ascii-point-cloud = Importuj chmurę punktów ASCII
io-import-drillhole-csv-bundle = Importuj pakiet CSV otworów wiertniczych
io-import-geotiff = Importuj GeoTIFF
io-import-las-laz-point-cloud = Importuj chmurę punktów LAS/LAZ
io-import-pcd-point-cloud = Importuj chmurę punktów PCD
io-import-ply = Importuj PLY
io-import-stl = Importuj STL
io-import-wavefront-obj = Importuj Wavefront OBJ
io-interval = Interwał
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-mapped-csv-bundle-csv = Zmapowany pakiet CSV (.csv)
io-model-file = Plik modelu
io-name-count-files = { $name } + { $count } plików
io-no-csv-chosen = Nie wybrano pliku .csv
io-no-csv-files-chosen = Nie wybrano plików CSV
io-no-dxf-chosen = Nie wybrano pliku .dxf
io-no-omf-chosen = Nie wybrano pliku .omf
io-north-y = Północ / Y
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-source-file = Plik źródłowy
io-start-x = X początku
io-start-y = Y początku
io-start-z = Z początku
io-stl = STL (.stl)
io-triangulation = Triangulacja:
io-unmapped = Niezmapowane
io-wavefront-obj = Wavefront OBJ (.obj)

## Jobs strings

jobs-background-task-poll-label-ended = Zadanie w tle „{ $poll_label }” zakończyło się bez wyniku
jobs-discarded-stale-result = Odrzucono nieaktualny wynik zadania w tle „{ $poll_label }”, ponieważ źródło zmieniło się lub zostało zamknięte

## Logging strings

logging-activity-completed = Ukończono działanie
logging-activity-started = Rozpoczęto działanie
logging-application-id-id = Identyfikator aplikacji: { $id }
logging-application-name = Nazwa aplikacji: { $name }
logging-application-startup = Uruchamianie aplikacji
logging-build-target-os-architecture = Platforma docelowa kompilacji: { $os }-{ $architecture }
logging-completed = Ukończono
logging-count-messages = { $count } wiadomości
logging-desktop-session-xdg-session-type = Sesja pulpitu: XDG_SESSION_TYPE={ $session }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Inicjalizowanie Incline Design
logging-locale-environment = Środowisko lokalizacji: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session = Sesja macOS: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = System operacyjny: GNU/Linux
logging-operating-system-macos = System operacyjny: macOS
logging-operating-system-microsoft-windows = System operacyjny: Microsoft Windows
logging-pointer-width = Szerokość wskaźnika: { $width }-bit
logging-process-id-id = Identyfikator procesu: { $id }
logging-release-version = Wersja wydania: { $version }
logging-renderer = Renderer
logging-rust-compiler-host = Host kompilatora Rust: { $host }
logging-system = System
logging-system-error = Błąd systemu
logging-unknown = nieznana
logging-windows-session-sessionname-session = Sesja Windows: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = Trwa przetwarzanie…

## Mac strings

mac-cannot-install-macos-menu-bar = Nie można zainstalować paska menu macOS poza wątkiem głównym
mac-quit-app = Zamknij { $app }

## Main strings

main-incline-design-web-startup-failed = Uruchamianie Incline Design Web nie powiodło się: { $error }

## Menu strings

menu-count-files-selected = Wybrano plików: { $count }

## Object strings

object-edit-appearance = Wygląd
object-edit-arc-circle = Łuk i okrąg
object-edit-arc-segments = Segmenty łuku
object-edit-bulge = Strzałka wybrzuszenia
object-edit-bulge-arcs-horizontal-data-model = Zgodnie z modelem danych łuki z wybrzuszeniem są poziome: łuk skręca w rzucie, a rzędna zmienia się liniowo od jednego wierzchołka do następnego.
object-edit-centre-x = Środek X
object-edit-centre-y = Środek Y
object-edit-centre-z = Środek Z
object-edit-chord = Cięciwa
object-edit-colour-layer = Kolor według warstwy
object-edit-enter-number = Wprowadź liczbę
object-edit-follow-owning-layer-s-colour = Użyj koloru warstwy właściciela zamiast koloru przypisanego do tego obiektu.
object-edit-id = ID
object-edit-identity = Tożsamość
object-edit-insert-after = Wstaw po
object-edit-join-last-vertex-back-first = Łączy ostatni wierzchołek ponownie z pierwszym.
object-edit-length = Długość { $length } m
object-edit-move-down = Przesuń w dół
object-edit-move-up = Przesuń w górę
object-edit-object-has-no-arc-segments = Ten obiekt nie ma segmentów łuku.
object-edit-object-has-single-position = Ten obiekt ma jedną pozycję.
object-edit-object-needs-least-required-vertices = Ten obiekt wymaga co najmniej { $required } wierzchołków
object-edit-one-more-properties-not-valid = Co najmniej jedna właściwość nie jest prawidłową liczbą
object-edit-perimeter-area = Obwód { $length } m, pole { $area } m²
object-edit-reverse = Odwróć
object-edit-row-invalid-number = Wiersz { $row }: pozycja lub strzałka wybrzuszenia nie jest prawidłową liczbą
object-edit-sweep = Kąt zamiatania
object-edit-text-not-number = „{ $text }” nie jest liczbą
object-edit-vertices = Wierzchołki

## Omf strings

omf-element-name-has-count-tie = Element „{ $name }” ma { $count } połączeń wskazujących otwory, których już nie zawiera
omf-ignoring-colour-map-omf-attribute = Pominięto mapę kolorów atrybutu OMF „{ $attribute }”: { $error }
omf-mining-data-exported-incline = Dane górnicze wyeksportowane przez Incline
omf-import = Import OMF
omf-texture = Tekstura OMF
omf-validation-warnings = Ostrzeżenia walidacji OMF: { $warnings }
omf-application-metadata-dropped = Metadane aplikacji projektu „{ $application }” nie są zachowywane
omf-project-author-not-retained = Autor projektu nie jest zachowywany
omf-project-description-not-retained = Opis projektu nie jest zachowywany
omf-unsupported-metadata-keys = Projekt zawiera nieobsługiwane klucze metadanych: { $keys }

## Plot strings

plot-1-1000-one-millimetre-sheet = Przy skali 1:1000 jeden milimetr na arkuszu odpowiada jednemu metrowi w terenie.
plot-1-scale-covers-width-height = 1:{ $scale } · obejmuje { $width } × { $height } m
plot-all-visible-data = Wszystkie widoczne dane
plot-automatic-grid-interval = Automatyczny interwał siatki
plot-border = Obramowanie
plot-centre = Wyśrodkuj na
plot-fit-scale-help = Wybierz najmniejszą standardową skalę, przy której wszystko widoczne mieści się na arkuszu.
plot-coordinate-grid = Siatka współrzędnych
plot-current-view-centre = Środek bieżącego widoku
plot-date-caps = DATA
plot-date = Data
plot-dots-per-inch-paper-size = Liczba punktów na cal. Ten rozmiar papieru można rastrować do { $max_dpi } dpi; 300 dpi to standardowa jakość druku.
plot-dpi = dpi
plot-drawing-no = NR RYSUNKU
plot-drawing-number = Numer rysunku
plot-drawn-by-caps = RYSOWAŁ
plot-drawn-by = Rysował
plot-e-g-example-gold-project = np. Przykładowy Projekt Złota
plot-entered-coordinates = Wprowadzone współrzędne
plot-export-png = Eksportuj PNG...
plot-fit-scale-visible-data = Dopasuj skalę do widocznych danych
plot-grid-interval = Interwał siatki
plot-landscape = Poziomo
plot-lists-visible-surfaces-design-layers = Wyświetla listę widocznych powierzchni i warstw projektowych wraz z ich kolorami.
plot-margin = Margines
plot-margins-leave-no-room-map = Marginesy nie pozostawiają miejsca na mapę
plot-metres-scale-1-scale = metry    Skala 1:{ $scale }
plot-mm = mm
plot-north-arrow = Strzałka północy
plot-nothing-visible-draw = Brak widocznych danych do narysowania
plot-paper = Papier
plot-paper-orientation-width-height-mm = { $paper } { $orientation } · { $width } × { $height } mm
plot-paper-size = Rozmiar papieru
plot-pick-interval-reads-roughly-every = Wybierz interwał, który odczytuje się mniej więcej co 50 mm na wydrukowanym arkuszu.
plot-plan = Plan
plot-scale-must-be-positive = Skala wydruku musi być liczbą dodatnią
plot-png-written-sheet-s-exact = Plik PNG jest zapisywany w dokładnym rozmiarze arkusza i zawiera informację o DPI, dzięki czemu drukuje się w rzeczywistej skali.
plot-portrait = Pionowo
plot-resolution = Rozdzielczość
plot-rev = WERSJA
plot-revision = Wersja
plot-scale = SKALA
plot-scale-ratio = Skala  1:
plot-scale-framing = Skala i kadrowanie
plot-sheet-furniture = Elementy oprawy arkusza
plot-size-width-height-mm = { $size } ({ $width } × { $height } mm)
plot-subtitle = Podtytuł
plot-title = Tytuł
plot-title-block = Tabliczka rysunkowa
plot-today = dzisiaj

## Products strings

products-add-initiation = Dodaj inicjację
products-delay = Opóźnienie
products-delay-palette = Paleta opóźnień
products-how-long-after-shot-fired = Jak długo po odpaleniu strzału ten wylot inicjuje serię.
products-initiation-name = Inicjacja · { $name }
products-milliseconds-between-one-hole-firing = Liczba milisekund między odpaleniem jednego otworu a kolejnym.
products-ms = ms
products-no-products = Brak produktów
products-remove = Usuń
products-update = Aktualizuj

## Progress strings

progress-percent-done-total = { $percent } ({ $done } z { $total })
progress-task-finished = { $task }: zakończono

## Project strings

project-item = Element

## Properties strings

properties-adds-view-dependent-rim-highlight = Dodaje zależne od widoku podświetlenie krawędzi na granicach bloków i materiałów. Wyłączenie tego nieznacznie zmniejsza obciążenie renderowania objętościowego.
properties-block-model-downscale = Zmniejszanie rozdzielczości modelu blokowego
properties-camera = Kamera
properties-camera-clip-planes = Płaszczyzny przycinania kamery
properties-cap-while-resizing = Ogranicz podczas zmiany rozmiaru
properties-dark-mode = Tryb ciemny
properties-developer = Deweloperskie
properties-downscale-rasters = Zmniejsz rozdzielczość rastrów
properties-edit-object = Edytuj obiekt...
properties-field-view = Pole widzenia
properties-fps = FPS
properties-frame-counter = Licznik klatek
properties-frame-rate-cap = Limit liczby klatek
properties-hz = Hz
properties-interface = Interfejs
properties-invert-horizontal = Odwróć w poziomie
properties-invert-vertical = Odwróć w pionie
properties-limits-newly-loaded-geotiff-previews = Ogranicza podglądy nowo wczytywanych plików GeoTIFF do 4096 pikseli na dłuższym boku. Wyłącz, aby użyć pełnej rozdzielczości do limitu tekstur GPU, co zużywa więcej pamięci.
properties-line-colour = Kolor linii
properties-look-sensitivity = Czułość rozglądania się
properties-max-clip-span = Maks. rozpiętość przycięcia
properties-move-layer = Przenieś do warstwy...
properties-near-clip-limit = Granica bliskiego przycięcia
properties-orbit-sensitivity = Czułość obrotu
properties-panel-chrome = Obramowanie paneli
properties-performance = Wydajność
properties-plan-mode = Tryb planu
properties-presents-step-display-no-tearing = Wyświetla w synchronizacji z ekranem: bez rozrywania obrazu, a częstotliwość odświeżania ustala ekran. Po wyłączeniu klatki są wyświetlane zaraz po narysowaniu i stosowany jest limit poniżej.
properties-reflective-block-edges = Odbijające krawędzie bloków
properties-restore-defaults = Przywróć domyślne
properties-show-console = Pokaż konsolę
properties-shows-live-near-far-projection = Wyświetla na pasku stanu bieżące odległości bliskiej i dalekiej projekcji.
properties-snap-polling = Odpytywanie przyciągania
properties-vertical-sync = Synchronizacja pionowa
properties-world-axis-gizmo = Gizmo osi świata
properties-zoom-cursor = Przybliżaj do kursora
properties-zoom-sensitivity = Czułość przybliżania

## Screenshot strings

screenshot-could-not-encode-viewport-image = Nie udało się zakodować obrazu widoku: { $error }
screenshot-could-not-map-viewport-screenshot = Nie udało się zmapować zrzutu ekranu widoku: { $error }
screenshot-could-not-save-viewport-image = Nie udało się zapisać obrazu widoku { $path }: { $error }
screenshot-downloaded-viewport-image-file-name = Pobrano obraz widoku: { $file_name }
screenshot-saved-viewport-image-path = Zapisano obraz widoku: { $path }
screenshot-viewport-image-download-failed-error = Pobieranie obrazu widoku nie powiodło się: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = Indeks ścianki BVH { $index } poza zakresem siatki; zastąpiono zdegenerowanym trójkątem

## State strings

state-above = na poziomie lub powyżej
state-activate-project = Aktywuj projekt
state-all-open-incline-design-data = Wszystkie otwarte dane Incline Design
state-apply-generated-rings = Zastosuj wygenerowane pierścienie
state-apply-selection = Zastosuj do zaznaczenia
state-rotate-by-azimuth-dip = wg azymutu { $azimuth }°, upadu { $dip }°
state-rotate-to-azimuth-dip = do azymutu { $azimuth }°, upadu { $dip }°
state-below = na poziomie lub poniżej
state-centre-rotation = Środek obrotu
state-checking-unsaved-work = Sprawdzanie niezapisanej pracy
state-choose-destination = Wybierz miejsce docelowe
state-choose-one-more-files = Wybierz jeden lub więcej plików
state-clear-raster = Wyczyść raster
state-click-pit-shell-viewport = Kliknij powłokę wyrobiska w widoku.
state-click-pit-stockpile-solid-viewport = Kliknij bryłę wyrobiska lub zwałowiska w widoku.
state-click-surface-viewport = Kliknij powierzchnię w widoku.
state-click-topology-viewport = Kliknij topologię w widoku.
state-close-project = Zamknij projekt
state-colour-drillholes = Koloruj otwory wiertnicze
state-copy-objects-layer = Kopiuj obiekty do warstwy
state-count-file-s = { $count } plików
state-count-object-s-axis-value = { $count } obiektów · { $axis } { $value }
state-count-object-s-closed = { $count } obiektów · { $closed }
state-count-object-s-layer = { $count } obiektów · { $layer }
state-count-object-s-weight = { $count } obiektów · { $weight }
state-count-object-s-z-elevation = { $count } obiektów · Z { $elevation }
state-create-point-cloud-tin = Utwórz TIN z chmury punktów
state-create-project = Utwórz projekt
state-current-project = Bieżący projekt
state-cut-topology-pit-shell = Przytnij topologię do powłoki wyrobiska
state-cut-triangulation-polyline = Przytnij triangulację polilinią
state-cut-triangulation-z = Przytnij triangulację wg Z
state-dark-mode = Tryb ciemny
state-detached = Odłączone
state-disabled = Wyłączone
state-discard-project-changes = Odrzuć zmiany projektu
state-discard-replace-project = Odrzuć i zastąp projekt
state-discarding-unsaved-changes = Odrzucanie niezapisanych zmian
state-docked = Zadokowane
state-drape-raster = Naciągnij raster
state-drill-pattern = Siatka wiertnicza
state-duplicate-layer = Duplikuj warstwę
state-east = Wschód
state-enabled = Włączone
state-exit-incline-design = Zamknij Incline Design
state-export-block-model-csv = Eksportuj model blokowy do CSV
state-export-layer-dxf = Eksportuj warstwę do DXF
state-export-omf = Eksportuj OMF
state-export-project-dxf = Eksportuj projekt do DXF
state-export-triangulation = Eksportuj triangulację
state-export-viewport-image = Eksportuj obraz widoku
state-finish-closed-polyline = Zakończ zamkniętą polilinię
state-finish-open-polyline = Zakończ otwartą polilinię
state-fit-extents = Dopasuj do zasięgu
state-fix-release-centre-both-views = Ustawia lub zwalnia środek, wokół którego obracają się oba widoki
state-generate-contours = Generuj poziomice
state-hidden = Ukryty
state-import-drillholes = Importuj otwory wiertnicze
state-import-omf = Importuj OMF
state-import-point-cloud = Importuj chmurę punktów
state-import-raster = Importuj raster
state-import-triangulation = Importuj triangulację
state-insert-intersection-points = Wstaw punkty przecięcia
state-insert-points-elevation = Wstaw punkty na wysokości
state-keep-inside = Zachowaj wewnątrz
state-keep-outside = Zachowaj na zewnątrz
state-kriged-block-model = Model blokowy z krigingu
state-load-block-model = Wczytaj model blokowy
state-load-drillholes = Wczytaj otwory wiertnicze
state-load-layer = Wczytaj warstwę
state-load-point-cloud = Wczytaj chmurę punktów
state-load-raster = Wczytaj raster
state-load-triangulation = Wczytaj triangulację
state-locked-count-object-s = Zablokowano { $count } obiektów
state-major-minor = Główna { $major } · podrzędna { $minor }
state-move-axis-value = Przesuń do wartości osi
state-move-objects-layer = Przenieś obiekty do warstwy
state-name-count-holes = { $name } · { $count } otworów
state-name-count-object-s = { $name } · { $count } obiektów
state-name-z-min-z-max = { $name } · od { $z_min } do { $z_max }
state-next-edit = Następna edycja
state-north = Północ
state-open-containing-folder = Otwórz folder zawierający plik
state-open-project = Otwórz projekt
state-preserve-view-angle = Zachowaj kąt widoku
state-previous-edit = Poprzednia edycja
state-project-id = Projekt { $id }
state-remove-block-model = Usuń model blokowy
state-remove-drillholes = Usuń otwory wiertnicze
state-remove-point-cloud = Usuń chmurę punktów
state-remove-raster = Usuń raster
state-remove-triangulation = Usuń triangulację
state-removed-from-active-triangulation = Usunięto z aktywnej triangulacji
state-removed-from-every-triangulation = Usunięto z każdej triangulacji
state-rename-kind = Zmień nazwę: { $kind }
state-save-close-project = Zapisz i zamknij projekt
state-save-despite-unsupported-content = Zapisz mimo nieobsługiwanej zawartości
state-save-project = Zapisz projekt jako
state-save-replace-project = Zapisz i zastąp projekt
state-saving-current-project = Zapisywanie bieżącego projektu
state-section-name = przekrój { $section }
state-select-layer-objects = Zaznacz obiekty warstwy
state-selected-objects = Zaznaczone obiekty
state-selected-polylines = Zaznaczone polilinie
state-selected-scene-elements = Zaznaczone elementy sceny
state-set-block-model-variable = Ustaw zmienną modelu blokowego
state-set-drillhole-colour-preset = Ustaw ustawienie koloru otworów wiertniczych
state-set-entity-lock = Ustaw blokadę obiektu
state-set-grid = Ustaw siatkę
state-set-layer-lock = Ustaw blokadę warstwy
state-set-line-weight = Ustaw grubość linii
state-set-object-colour = Ustaw kolor obiektu
state-set-object-fill = Ustaw wypełnienie obiektu
state-set-point-visibility = Ustaw widoczność punktu
state-set-polyline-closed = Ustaw zamknięcie polilinii
state-set-raster-lock = Ustaw blokadę rastra
state-set-standard-view = Ustaw widok standardowy
state-set-topology-wireframes = Ustaw siatki krawędziowe topologii
state-set-triangulation-colour = Ustaw kolor triangulacji
state-show-console = Pokaż konsolę
state-show-project = Pokaż projekt
state-shown = Pokazano
state-slice-mode = Tryb przekroju
state-slice-preview = Podgląd przekroju
state-south = Południe
state-stem-contours = Poziomice { $stem }
state-target-new-name = { $target } na „{ $new_name }”
state-trim-above = Przytnij powyżej
state-trim-below = Przytnij poniżej
state-trim-triangulation-surface = Przytnij triangulację do powierzchni
state-undrape-raster = Zdejmij raster
state-undrape-rasters = Zdejmij rastry
state-unload-block-model = Wyładuj model blokowy
state-unload-drillholes = Wyładuj otwory wiertnicze
state-unload-layer = Wyładuj warstwę
state-unload-point-cloud = Wyładuj chmurę punktów
state-unload-raster = Wyładuj raster
state-unload-triangulation = Wyładuj triangulację
state-untitled-project = Projekt bez nazwy
state-use-typed-radius = Użyj wpisanego promienia
state-west = Zachód

## Status strings

status-clip-near-far = Bliska/daleka/Δ: -- / -- / --
status-frame-rate = Liczba klatek

## Text strings

text-could-not-build-vector-mesh = Nie udało się zbudować siatki wektorowej dla czcionki { $font }, glifu { $glyph }: { $error }
text-document-text-mesh-exceeded-its = Siatka tekstu dokumentu przekroczyła zakres indeksów u32

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = Wybierz najpierw zbiór otworów wiertniczych do połączenia
tie-in-count-connector-s = { $count } łączników
tie-in-delete-tie-ins = Usuń połączenia
tie-in-deleted-count-selected-tie-connector = Usunięto { $count } zaznaczonych łączników połączeń
tie-in-hole = otwór
tie-in-initiation-point-lifted-from-name = Punkt inicjacji zdjęty z { $name }
tie-in-initiation-point-set-name-delay = Ustawiono punkt inicjacji na { $name } przy opóźnieniu { $delay } ms
tie-in-select-delay-product-palette-before = Wybierz produkt opóźniający w palecie przed połączeniem otworów
tie-in-tied-connectors = Połączono { $count } łączników przy opóźnieniu { $delay } ms produktem { $product }
tie-in-tied-connectors-replacing = Połączono { $count } łączników przy opóźnieniu { $delay } ms produktem { $product }, zastępując { $replaced }

## Toolbar strings

toolbar-fill-type = Typ wypełnienia

## Toolbars strings

toolbars-auto-bench = Auto-piętro
toolbars-bezier-polyline = Polilinia Béziera
toolbars-chamfer-polyline-corners = Fazuj narożniki polilinii
toolbars-create-text = Utwórz tekst
toolbars-cursor-regular = Kursor: zwykły
toolbars-cursor-snap-line = Kursor: przyciąganie do linii
toolbars-cursor-snap-point = Kursor: przyciąganie do punktu
toolbars-cursor-snap-surface = Kursor: przyciąganie do powierzchni
toolbars-delete-points = Usuń punkty
toolbars-explode-polyline-lines = Rozbij polilinię na linie
toolbars-fuse-polylines = Połącz polilinie
toolbars-measure-distance = Zmierz odległość
toolbars-new-layer = Nowa warstwa
toolbars-split-polyline-points = Podziel polilinię w punktach
toolbars-strike-dip = Kierunek i upad
toolbars-tool-not-available-section-view = { $tool } - niedostępne w widoku przekroju

## Tri strings

tri-sampling-method-help = Metoda adaptacyjna koncentruje wierzchołki na złożonym terenie na podstawie błędu dopasowania płaszczyzny; jednorodna rozprowadza je równomiernie. W przyszłości mogą zostać dodane kolejne metody.
tri-adaptive-quadtree = Adaptacyjna (quadtree)
tri-axis-range = Zakres { $axis }
tri-base-topology-will-receive-pit = Topologia bazowa, która otrzyma kształt wyrobiska lub zwałowiska.
tri-boundary-polyline = Polilinia granicy
tri-bridge-gaps-help = Wypełnia mostkami luki i wklęsłości granicy węższe niż ta wartość w obrębie powierzchni. Wartość 0 nadal mostkuje luki o rozmiarze mniej więcej komórki próbkowania; większe wartości wypełniają większe otwory i zacierają wklęsłości granicy.
tri-budget = Budżet wg
tri-cancel-pick = Anuluj wybór
tri-candidate-detail = Szczegółowość kandydatów
tri-candidate-fine-cells-per-budgeted = Liczba kandydujących drobnych komórek na jeden wierzchołek budżetu. Wyższa wartość daje próbnikowi adaptacyjnemu większą swobodę rozmieszczania szczegółów, ale wydłuża budowanie.
tri-cap-surface-share-source-points = Ogranicz powierzchnię udziałem punktów źródłowych lub dokładną liczbą wierzchołków.
tri-choose-input-clicking-loaded-surface = Wybierz te dane wejściowe, klikając wczytaną powierzchnię w widoku
tri-choose-which-side-reference-topology = Wybierz, którą stronę topologii odniesienia usunąć z powierzchni w ich wspólnym obszarze XY.
tri-clip = Przytnij
tri-clip-creates-new-triangulation-name = Przycięcie tworzy nową triangulację o tej nazwie; powierzchnia źródłowa nie jest modyfikowana.
tri-clip-surface-polyline = Przytnij powierzchnię polilinią
tri-closed-pit-stockpile-solid-whose = Zamknięta bryła wyrobiska lub zwałowiska, której odsłonięta granica zostanie uwzględniona w wyniku.
tri-create-new-layer-contours-append = Utwórz nową warstwę dla poziomic lub dołącz je do istniejącej warstwy w aktywnym projekcie.
tri-cut-topology-pit-shell = Przytnij topologię powłoką wyrobiska
tri-e-g-design-trimmed = np. design_trimmed
tri-e-g-mysurf-cut = np. mysurf_cut
tri-e-g-mysurf-slice = np. mysurf_slice
tri-e-g-surface-contour = np. surface_contour
tri-e-g-topo-cut = np. topo_cut
tri-e-g-topo-pit = np. topo_with_pit
tri-exact-number-surface-vertices-target = Docelowa dokładna liczba wierzchołków powierzchni. Bardzo duże wartości budują się wolno i zużywają dużo pamięci.
tri-existing-ground-topology-will-cut = Istniejąca topologia terenu, która zostanie przecięta powłoką wyrobiska.
tri-fill-holes-up = Wypełnij otwory do
tri-generate = Generuj
tri-generate-contour-lines = Generuj poziomice
tri-generate-upper-surface = Wygeneruj powierzchnię górną
tri-hide-unload-sources = Ukryj i wyładuj źródła
tri-higher-edge-will-enforced-each = Przy każdym konflikcie wymuszona zostanie wyższa krawędź. Niższe, konfliktowe segmenty zostaną pominięte jako linie nieciągłości, a powierzchnia zinterpoluje te obszary. Polilinie źródłowe pozostają bez zmian.
tri-breaklines-cross = Podświetlone krawędzie linii nieciągłości przecinają się lub nakładają w rzucie na różnych wysokościach. Jedna powierzchnia terenu nie może odzwierciedlić obu naraz.
tri-intervals-colours = Interwały i kolory
tri-keep-clipped-topology-included-shape = Zachowaj przyciętą topologię i dołączony kształt jako osobne triangulacje zamiast łączyć je w jeden obiekt.
tri-keep-inside-discards-surface-outside = Zachowaj wewnątrz odrzuca powierzchnię poza polilinią. Zachowaj na zewnątrz wycina z powierzchni otwór w kształcie polilinii.
tri-keeps-only-surface-within-polyline = Zachowuje tylko powierzchnię wewnątrz granicy polilinii.
tri-keep-surface-relation-help = Zachowuje powierzchnię { $relation } topologii w obrębie jej zasięgu XY.
tri-layer-already-exists-select-above = Ta warstwa już istnieje; wybierz ją powyżej lub podaj inną nazwę.
tri-limit-z-range = Ogranicz zakres Z
tri-major = Główna
tri-max-edge-length = Maks. długość krawędzi
tri-merge = Scal
tri-method = Metoda
tri-min = Min.
tri-minimum-maximum-elevations-retained = Minimalna i maksymalna wysokość zachowana w powierzchni wynikowej. Minimum musi być mniejsze od maksimum.
tri-minor = Podrzędna
tri-contour-interval-help = Podrzędna steruje zwykłymi poziomicami. Główna steruje wyróżnionymi poziomicami i musi używać interwału co najmniej tak dużego jak Podrzędna.
tri-move-cursor-over-loaded-surface = Najedź kursorem na wczytaną powierzchnię.
tri-slice-output-name-help = Nazwa nadawana wynikowej powierzchni przyciętej wg wysokości.
tri-name-assigned-merged-topology-pit = Nazwa nadawana scalonej topologii i wynikowi wyrobiska/zwałowiska.
tri-name-assigned-newly-created-contour = Nazwa nadawana nowo utworzonej warstwie poziomic.
tri-reconstruct-output-name-help = Nazwa nadawana zrekonstruowanej triangulacji.
tri-name-assigned-topology-after-pit = Nazwa nadawana topologii po odjęciu od niej powłoki wyrobiska.
tri-name-assigned-trimmed-output-surface = Nazwa nadawana przyciętej powierzchni wynikowej.
tri-nearby-breakline-vertices-do-not = Pobliskie wierzchołki linii nieciągłości nie schodzą się dokładnie w tym samym miejscu, więc powierzchni nie można stworzyć triangulacją.
tri-new-layer = Nowa warstwa
tri-new-layer-name = Nazwa nowej warstwy
tri-once-merge-succeeds-unload-source = Po udanym scaleniu wyładuj topologię źródłową i bryłę, aby w scenie pozostał tylko wynik scalenia.
tri-only-loaded-pickable = Można wybierać tylko wczytane triangulacje.
tri-operation = Operacja
tri-output-layer = Warstwa wynikowa
tri-percentage = Procent
tri-percentage-cloud = Procent chmury
tri-pick-from-view = Wybierz z widoku
tri-pit-design-surface-only-areas = Powierzchnia projektowa wyrobiska. Do przycięcia wykorzystywane są tylko obszary, w których powłoka schodzi poniżej topologii.
tri-pit-shell = Powłoka wyrobiska
tri-pit-stockpile-solid = Bryła wyrobiska/zwałowiska
tri-recommended-weld-retry = Zalecane: Zespól i spróbuj ponownie
tri-reconstruct-help = Zrekonstruuj triangulowaną powierzchnię terenu z chmury punktów. Adaptacyjny próbnik wydaje budżet wierzchołków tam, gdzie teren jest najbardziej złożony, a obszary płaskie pozostawia rzadkie.
tri-reduce-budget-candidate-detail-if = Zmniejsz budżet lub szczegółowość kandydatów, jeśli Twój komputer ma mniej pamięci RAM.
tri-reference-topology-help = Topologia odniesienia określająca, gdzie przycinana jest druga powierzchnia.
tri-reject-reconstructed-triangle-edges = Odrzucaj zrekonstruowane krawędzie trójkątów dłuższe niż ta odległość. Użyj 0, aby nie ograniczać długości krawędzi.
tri-remove-inside-help = Usuwa powierzchnię wewnątrz granicy polilinii, zachowując resztę.
tri-removes-topology-where-pit-shell = Usuwa topologię tam, gdzie powłoka wyrobiska schodzi poniżej niej, tak aby powłoka wypełniła otwór. Szew podąża za rzeczywistą trójwymiarową linią styku obu powierzchni; topologia pod fragmentami powłoki wystającymi ponad teren jest zachowywana.
tri-result = Wynik
tri-save-two-entities = Zapisz jako dwa elementy
tri-select = Wybierz…
tri-share-source-points-keep-fractions = Udział zachowywanych punktów źródłowych. Dopuszczalne są ułamki, np. 0,125%.
tri-slice-triangulation-z-range = Przytnij triangulację wg zakresu Z
tri-solution-generate-upper-surface = Rozwiązanie: Wygeneruj powierzchnię górną
tri-surface-trim = Powierzchnia do przycięcia
tri-target-surface-help = Powierzchnia, która zostanie zmieniona; wybrana topologia pozostaje nienaruszona.
common-percent-suffix = %
tri-topology = Topologia
tri-triangulation-failed = Triangulacja nie powiodła się
tri-trim = Przytnij
tri-trim-topology = Przytnij do topologii
tri-uniform-grid = Siatka jednorodna
tri-up-target-point-count-points = Do { $target } z { $point_count } punktów stanie się wierzchołkami powierzchni ({ $percent }%).
tri-use-full-surface-elevation-range = Użyj pełnego zakresu wysokości powierzchni
tri-vertex-count = Liczba wierzchołków
tri-vertices-within-5-cm-xy = Wierzchołki znajdujące się w promieniu 5 cm w XY i Z będą współdzielić jedną pozycję na potrzeby tej triangulacji. Może to lokalnie przesunąć wygenerowaną powierzchnię o maks. 5 cm; polilinie źródłowe pozostają bez zmian.
tri-weld-retry = Zespól i spróbuj ponownie
tri-when-enabled-generate-contours-only = Gdy włączone, generuj poziomice tylko między podaną minimalną a maksymalną wysokością.

## Ui strings

ui-choose-offset-side = Wybierz stronę przesunięcia
ui-choose-relimit-side = Wybierz stronę przycięcia
ui-click-circle-centre = Kliknij środek okręgu
ui-click-closed-polyline-use-blast = Kliknij zamkniętą polilinię, aby użyć jej jako kształtu strzelania
ui-click-collar-add-edit-initiation = Kliknij wylot otworu, aby dodać lub edytować punkt inicjacji
ui-click-first-point-slice-line = Kliknij pierwszy punkt linii przekroju
ui-click-first-vertex = Kliknij pierwszy wierzchołek
ui-click-perimeter-point-type-radius = Kliknij punkt na obwodzie lub wpisz promień
ui-click-second-point-slice-line = Kliknij drugi punkt linii przekroju
ui-click-second-vertex = Kliknij drugi wierzchołek
ui-click-use-pointer-radius = lub kliknij, aby użyć promienia wskaźnika
ui-could-not-copy-text-browser = Nie udało się skopiować tekstu do schowka przeglądarki: { $error }
ui-dip-horizontal-no-strike = { $dip } (poziomo, brak kierunku)
ui-distance-meters = { $distance } m
ui-drag-ring-type-azimuth-dip = Przeciągnij pierścień lub wpisz azymut i upad
ui-each-hole-turns-about-its = każdy otwór obraca się wokół własnego wylotu
ui-enter-positive-decimal-radius = Wpisz dodatni promień dziesiętny
ui-esc-cancels = Esc anuluje
ui-no-delay-product-tie = Brak środka opóźniającego do połączenia
ui-press-enter-use-typed-radius = Naciśnij Enter, aby użyć wpisanego promienia
ui-right-click-delay-palette-heading = kliknij prawym przyciskiem nagłówek palety opóźnień, aby dodać
ui-select-designs = Wybierz projekty rysunkowe
ui-select-drill-hole = Wybierz otwór wiertniczy
ui-select-endpoint-join = Wybierz punkt końcowy do połączenia
ui-select-first-crest-toe-point = Wybierz pierwszy punkt korony/stopy
ui-select-item = Wybierz element
ui-select-line-fuse = Wybierz linię do połączenia
ui-select-line-polyline = Wybierz linię lub polilinię
ui-select-line-relimit = Wybierz linię do przycięcia
ui-select-next-line-fuse = Wybierz kolejną linię do połączenia
ui-select-opposite-berm-point = Wybierz przeciwległy punkt bermy
ui-select-point = Wybierz punkt
ui-select-polyline = Wybierz polilinię
ui-select-polyline-open-line = Wybierz polilinię lub linię otwartą
ui-select-polyline-vertex = Wybierz wierzchołek polilinii
ui-select-second-crest-toe-point = Wybierz drugi punkt korony/stopy
ui-select-second-split-point = Wybierz drugi punkt podziału
ui-select-split-point = Wybierz punkt podziału
ui-select-topologies = Wybierz topologie
ui-slice-view = Widok przekroju
ui-strike-dip = kierunek { $strike }° · upad { $dip }
ui-value-dip = upad { $value }°

## Viewport strings

viewport-all-total-categories-keep-their = Wszystkie { $total } kategorie zachowują swój kolor; wyraźnie rysowane są tylko pierwsze { $shown }
viewport-axis-maximum = Maksimum { $axis }
viewport-axis-minimum = Minimum { $axis }
viewport-bar-blast-timeline-placeholder = Oś czasu strzelania [ZASTĘPCZE]
viewport-bar-burden-relief-heatmap-placeholder = Mapa cieplna odprężenia calizny [ZASTĘPCZE]
viewport-bar-color = Kolor:
viewport-bar-contours-equal-time-placeholder = Izochrony [ZASTĘPCZE]
viewport-bar-disable-flying-mode = Wyłącz tryb lotu
viewport-bar-disable-x-ray-vision = Wyłącz widzenie rentgenowskie
viewport-bar-drill-holes = Otwory wiertnicze:
viewport-bar-enable-flying-mode = Włącz tryb lotu
viewport-bar-enable-x-ray-vision = Włącz widzenie rentgenowskie
viewport-bar-exit-slice-view = Wyjdź z widoku przekroju
viewport-bar-fill = Wypełnienie:
viewport-bar-fix-centre-rotation = Ustaw środek obrotu
viewport-bar-hide-points = Ukryj punkty
viewport-bar-hide-rl-grid = Ukryj siatkę rzędnych
viewport-bar-hide-wireframes = Ukryj siatki krawędziowe
viewport-bar-hide-xy-grid = Ukryj siatkę XY
viewport-bar-release-centre-rotation = Zwolnij środek obrotu
viewport-bar-show-points = Pokaż punkty
viewport-bar-show-rl-grid = Pokaż siatkę rzędnych
viewport-bar-show-wireframes = Pokaż siatki krawędziowe
viewport-bar-show-xy-grid = Pokaż siatkę XY
viewport-bar-vertical-slice-view = Pionowy widok przekroju
viewport-blank = (puste)
viewport-choose-active-block-model-variable = Wybierz aktywną zmienną modelu blokowego
viewport-choose-variable = Wybierz zmienną
viewport-click-edit-color-right-click = Kliknij, aby edytować kolor; kliknij prawym przyciskiem, aby usunąć
viewport-click-type-boundary-s-value = Kliknij, aby wpisać wartość tej granicy
viewport-colour-mapping = Mapowanie kolorów
viewport-count-categories = { $count } kategorii
viewport-count-category = { $count } kategoria
viewport-double-click-add-boundary-here = Kliknij dwukrotnie, aby dodać tutaj granicę
viewport-drag-move-middle-click-toggles = Przeciągnij, aby przesunąć · Środkowy przycisk przełącza ≤
viewport-drag-move-right-click-remove = Przeciągnij, aby przesunąć · Kliknij prawym przyciskiem, aby usunąć · Środkowy przycisk przełącza ≤
viewport-e = E
viewport-edit-category-colour = Edytuj kolor tej kategorii
viewport-edit-colour-used-empty-values = Edytuj kolor używany dla pustych wartości
viewport-empty = (puste)
viewport-empty-hidden = (puste · ukryte)
viewport-filter-variables = Filtruj zmienne
viewport-navigation-hint = Przeciągnij środkowym przyciskiem, aby przesunąć widok · Przewiń, aby przybliżyć
viewport-navigation-hint-detach = Przeciągnij środkowym przyciskiem, aby przesunąć widok · Przewiń, aby przybliżyć · Kliknij, aby odłączyć
viewport-n = N
viewport-no-data-variable = Brak danych dla tej zmiennej
viewport-no-matches = Brak dopasowań
viewport-no-usable-range = (brak użytecznego zakresu)
viewport-rebuild-variable-s-colours-from = Przebuduj kolory tej zmiennej na podstawie jej danych
viewport-reset = Resetuj
viewport-restore-full-model-range = Przywróć pełny zakres modelu
