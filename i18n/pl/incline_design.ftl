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
ws-menubar-modelling = Modelowanie
ws-menubar-modelling-select-holes = Najpierw zaznacz otwory
ws-menubar-modelling-select-points = Zaznacz co najmniej { $count } punkty
ws-menubar-modelling-select-surface = Zaznacz jedną powierzchnię siatkową
ws-menubar-modelling-select-surfaces = Zaznacz strop i spąg pokładu, dwie powierzchnie siatkowe
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
tri-create-help = Triangulacja obiektów zaznaczonych w chwili otwarcia tego okna. Zamknij je, aby zmienić zaznaczenie.
tri-create-type-label = Typ triangulacji
tri-create-type-help =
    Powierzchnia otwarta tworzy płat w stylu terenu. Bryła tworzy w pełni
    zamkniętą siatkę i wymaga danych wejściowych tworzących szczelną granicę.
tri-create-output-name = Nazwa wyniku
tri-create-output-name-help = Nazwa nadawana wygenerowanej triangulacji.
tri-create-output-name-hint = nazwa triangulacji
tri-create-run = Triangulacja
tri-selection-none = Zaznaczone obiekty nie są już dostępne.

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
tri-count-circles =
    { $count ->
        [one] { $count } okrąg
        [few] { $count } okręgi
        [many] { $count } okręgów
       *[other] { $count } okręgów
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
status-faces = Ścianki: { $drawn } / { $total } ({ $drawn_chunks }/{ $total_chunks } fragmentów)
status-clip = Bliska/daleka/Δ: { $near } / { $far } / { $delta } m
status-points = Punkty: { $drawn } / { $target } z { $total } ({ $drawn_chunks }/{ $total_chunks } fragmentów)

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
preferences-ui-size = Rozmiar interfejsu
preferences-ui-size-help = Dostosowuje tekst i kontrolki względem standardowego skalowania ekranu urządzenia. 100% oznacza rozmiar domyślny. Rozdzielczość ekranu i rozmiar okna nie zmniejszają interfejsu.
relimit-select-boundary = Wybierz polilinię lub okrąg, do którego ma nastąpić przycięcie
relimit-click-boundary = Kliknij polilinię lub okrąg do przecięcia…
relimit-mode-help = Przecięcie przesuwa jeden koniec do polilinii lub okręgu. Bezwzględnie ustawia końcową długość linii. Względnie dodaje lub odejmuje długość.
browser-graphics-device-lost = Przeglądarka utraciła urządzenie graficzne. Otwórz tę stronę ponownie w nowej karcie. Szczegóły GPU: { $message }

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
block-model-no-block-model-selected = Nie wybrano modelu blokowego
block-model-no-drill-holes-selected = Nie wybrano otworów wiertniczych
block-model-nugget = Efekt samorodkowy
block-model-numeric-interval-fields-interpolate = Liczbowe pola interwałów do interpolacji. Każde wybrane pole staje się jedną zmienną modelu blokowego.
block-model-kriging-help = Kriging zwyczajny szacuje liczbowe interwały otworów wiertniczych w środku każdego bloku, wykorzystując wariogram sferyczny.
block-model-partial-sill = Częściowy próg wariancji
block-model-range-search-radius = Zasięg / promień wyszukiwania
block-model-range-help = Próbki dalsze niż ta odległość są wykluczane; kowariancja osiąga zero przy tym zasięgu.
block-model-select-all = Zaznacz wszystko
block-model-selected-block-model-whose-blocks = Wybrany model blokowy, którego bloki są progowane do bryły. Zamknij okno, aby wybrać inny model.
block-model-source-drill-holes-help = Wybrany zbiór otworów wiertniczych, którego interwały liczbowe są szacowane do bloków. Zamknij okno, aby szacować z innego zbioru.
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
borehole-inspector-add-every-code = Dodaj wszystkie kody spoza listy
borehole-inspector-add-to-column = Dodaj
borehole-inspector-check = Sprawdź
borehole-inspector-check-accept = Akceptuj
borehole-inspector-check-column = Sprawdź
borehole-inspector-check-column-changed = Kolumna zmieniła się od ostatniego sprawdzenia. Sprawdź ponownie, aby zobaczyć, które otwory się z nią nie zgadzają.
borehole-inspector-check-column-hint = Ustala kolejność, w jakiej większość otworów podaje te kody, i wymienia otwory, które się z nią nie zgadzają. Pusta kolumna zostaje wypełniona; inna zmienia się dopiero po akceptacji.
borehole-inspector-check-differences-note = Kolejność podawana przez większość otworów, obok kolumny. Akceptuj ustawia kolumnę według niej jednym krokiem cofnięcia.
borehole-inspector-check-flagged = Sprawdzenie: oznaczone otwory: { $count }
borehole-inspector-check-moved = przesunięto
borehole-inspector-check-not-run = Jeszcze nie sprawdzono.
borehole-inspector-check-now = Teraz
borehole-inspector-check-order-differs = Większość otworów podaje te kody w innej kolejności niż kolumna.
borehole-inspector-check-overruled = Słabsze większości przegłosowane przez silniejsze: { $count }.
borehole-inspector-check-place = Miejsce
borehole-inspector-check-proposed = Proponowane
borehole-inspector-check-show-differences = Pokaż różnice
borehole-inspector-check-stale = Otwory zmieniły się od ostatniego sprawdzenia. Sprawdź ponownie.
borehole-inspector-check-summary = Odczytano otworów: { $holes }; niezgodnych z kolumną: { $flagged }.
borehole-inspector-check-too-many-codes = To pole zawiera zbyt wiele kodów, aby je uporządkować.
borehole-inspector-checking-linked-geophysics-file = Sprawdzanie powiązanego pliku geofizyki...
borehole-inspector-close-inspector = Zamknij inspektor
borehole-inspector-code-not-in-set = Wymieniony w kolumnie, ale nieposiadany przez żaden interwał tego zbioru
borehole-inspector-column = Kolumna
borehole-inspector-column-empty-check = Brak jeszcze kolumny stratygraficznej. Użyj Sprawdź, aby wypełnić ją kolejnością podawaną przez większość otworów, albo dodaj poniżej kody i uporządkuj je ręcznie.
borehole-inspector-data = Dane
borehole-inspector-display = Wyświetlanie
borehole-inspector-every-code-placed = Każdy kod pola jest w kolumnie.
borehole-inspector-flag-of-groups = { $kind } (grupy)
borehole-inspector-flag-out-of-place = Nie na miejscu
borehole-inspector-flag-overturned = Przewrócone
borehole-inspector-flag-repeat = Powtórzone
borehole-inspector-flagged-holes = Oznaczone otwory ({ $count })
borehole-inspector-flags-first-shown = Wymieniono pierwsze { $shown } z { $count } oznaczeń.
borehole-inspector-file-not-where-was-linked = Plik { $file } nie znajduje się tam, skąd został powiązany.
borehole-inspector-guessed-name = Dopasowano po nazwie
borehole-inspector-hold-hole-while-you-work = Przytrzymaj ten otwór podczas pracy z otworami wokół niego.
borehole-inspector-holding-hole-click-follow-selection = Ten otwór jest przytrzymany. Kliknij, aby ponownie podążać za zaznaczeniem.
borehole-inspector-log = Profil
borehole-inspector-inspect-hole = Przejrzyj
borehole-inspector-no-holes-flagged = Żaden otwór nie jest niezgodny z kolumną.
borehole-inspector-no-categorical-field = Ten zbiór nie ma pola kategorialnego do uporządkowania.
borehole-inspector-no-hole-inspected = Nie sprawdzono żadnego otworu
borehole-inspector-not-in-column = Brak w kolumnie ({ $count })
borehole-inspector-pick-file = Wskaż { $file }...
borehole-inspector-pick-file-again-show-its = Wskaż ponownie { $file }, aby wyświetlić geofizykę: strona przeglądarki nie może sama ponownie otworzyć pliku.
borehole-inspector-place-codes-note = Kody w danych, których kolumna jeszcze nie zawiera. Dodane trafiają na dół; przenieś je na miejsce.
borehole-inspector-reading-geophysics-file-its-index = Odczyt pliku geofizyki w celu zbudowania indeksu; postęp widać na pasku stanu.
borehole-inspector-remove-from-column = Usuń z kolumny
borehole-inspector-strat = Stratygrafia
borehole-inspector-strat-column = Kolumna stratygraficzna
borehole-inspector-summary = Podsumowanie
canvas-circle-summary = Okrąg | Warstwa: { $layer } | promień { $radius }

## Canvas strings

canvas-not-selectable-closed-polyline = Nie można wybrać | Wybierz zamkniętą polilinię
canvas-polyline-summary = Polilinia | Warstwa: { $layer } | { $count } wierzchołków
canvas-surface-name = Powierzchnia | { $name }
canvas-trimmed = Przycięte
cinematic-shadows-method = Cienie widoku filmowego: { $method }

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
cmd-commands-count-control-string-s = Linie kontrolne: { $count }
cmd-commands-count-control-string-s-layer = Linie kontrolne: { $count } w warstwie „{ $layer }”
cmd-commands-count-point-s-across-layers = Punkty: { $count } w warstwach: { $layers }
cmd-commands-count-point-s-layer = Punkty: { $count } w warstwie „{ $layer }”
cmd-commands-kind-layer = { $kind } w warstwie „{ $layer }”
cmd-commands-no-control-strings = Brak linii kontrolnych
cmd-commands-no-extent = Brak zasięgu
cmd-commands-select-holes-place-reference-points = Wybierz otwory, na których mają zostać umieszczone punkty odniesienia
cmd-triangulate-needs-selection = Wybierz obiekty do triangulacji przed uruchomieniem polecenia Utwórz triangulację
cmd-commands-select-one-loaded-block-model = Wybierz jeden wczytany model blokowy przed utworzeniem z niego triangulacji złoża
cmd-commands-select-one-loaded-drill-hole = Wybierz jeden wczytany zbiór otworów wiertniczych przed utworzeniem z niego modelu blokowego
cmd-commands-select-one-loaded-point-cloud = Wybierz jedną wczytaną chmurę punktów przed utworzeniem z niej triangulacji
cmd-contours-needs-triangulation = Wybierz jedną wczytaną triangulację przed wygenerowaniem z niej poziomic
cmd-slice-needs-triangulation = Wybierz jedną wczytaną triangulację przed jej przycięciem wg zakresu Z
cmd-commands-select-one-loaded-triangulation-one = Wybierz jedną wczytaną triangulację i jedną zamkniętą polilinię przed przycięciem
cmd-commands-select-one-more-objects-before = Wybierz jeden lub więcej obiektów przed ustawieniem { $axis }
cmd-commands-sliced = Przekrojone
cmd-commands-modelling-settings-set-settings = Ustawiono ustawienia modelowania. { $settings }
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
cmd-drill-hole-choose-drillhole-source-files-again = Wybierz ponownie pliki źródłowe otworów wiertniczych
cmd-drill-hole-drill-pattern-too-large-contains = Siatka wiertnicza jest zbyt duża lub zawiera nieprawidłowe współrzędne wylotów
cmd-drill-hole-drillhole-field-label-has-count = Pole otworów wiertniczych „{ $label }” ma { $count } różnych kodów, więcej niż zwykle ma pole kodowane; wygląda na tekst dowolny, a nie pole kategorialne, ale wszystkie kody zostały zachowane i pokolorowane
cmd-drill-hole-enter-name-drill-pattern = Wpisz nazwę siatki wiertniczej
cmd-drill-hole-failed-load-drillholes-error = Nie udało się wczytać otworów wiertniczych: { $error }
cmd-drill-hole-depth-must-be-positive = Głębokość otworu musi być większa od zera
cmd-drill-hole-diameter-must-be-positive = Średnica otworu musi być większa od zera
cmd-drill-hole-loaded-drillhole-dataset-name-holes = Wczytano zbiór otworów wiertniczych „{ $name }”: { $holes } otworów, { $fields } pól koloru
cmd-drill-hole-field-has-no-strat-column = Pole { $field } nie ma kolumny stratygraficznej; nic nie przesunięto
cmd-drill-hole-name-already-loading = „{ $name }” jest już wczytywany
cmd-drill-hole-name-reason = „{ $name }”: { $reason }
cmd-drill-hole-no-hole-holds-value-field = Żaden otwór nie zawiera wartości „{ $value }” w tym polu
cmd-drill-hole-names-shifted-down = Przesunięto nazwy otworu { $hole } w dół otworu: przesunięto { $moved }, nazwanych UNK { $unknown }, spoza kolumny pozostawionych bez zmian { $untouched }
cmd-drill-hole-names-shifted-up = Przesunięto nazwy otworu { $hole } w górę otworu: przesunięto { $moved }, nazwanych UNK { $unknown }, spoza kolumny pozostawionych bez zmian { $untouched }
cmd-drill-hole-no-interval-holds-seam = Żaden interwał nie zawiera już „{ $name }”; nic nie zmieniono
cmd-drill-hole-only-mapped-csv-bundles-imported = W przeglądarce importowane są tylko zestawy CSV z mapowaniem
cmd-drill-hole-pattern-contains-no-holes = Siatka nie zawiera żadnych otworów
cmd-drill-hole-no-interval-names-column-code = Żaden interwał otworu { $hole } nie ma kodu z kolumny; spoza kolumny pozostawiono bez zmian: { $untouched }
cmd-drill-hole-reading-name = Odczyt: { $name }
cmd-drill-hole-reference-points-used-holes-placed = Punkty odniesienia: umieszczono otworów: { $used }, bez „{ $value }”: { $absent }, oznaczonych jako możliwe powtórzenia uskoku: { $flagged }
cmd-drill-hole-no-collars = Żaden otwór nie ma wylotu, w którym można postawić punkt
cmd-drill-hole-collars-layer = Wyloty otworów
cmd-drill-hole-collar-points = Punkty wylotów: umieszczono otworów { $used }, bez wylotu { $absent }
cmd-drill-hole-seam-renamed = Zmieniono nazwę pokładu „{ $from }” na „{ $to }”; zaproponowane poprawki: { $count }
cmd-drill-hole-uppermost-run-used-flagged-holes = Użyto najwyższego odcinka, oznaczone: { $holes }
cmd-drill-hole-working-section-name-not-same = Przekrój roboczy „{ $name }” nie jest taki sam w każdym wybranym zbiorze danych; użyto własnego przekroju każdego zbioru.
cmd-drill-hole-working-sections-not-kept-dataset = Przekroje robocze nie zostały zachowane w „{ $dataset }”. { $reasons }
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
cmd-file-downhole-geophysics-csv = CSV geofizyki otworowej
cmd-file-downloaded-description-file-name = Pobrano { $description }: { $file_name }
cmd-file-drillhole-csv-export-failed-error = Eksport CSV otworów wiertniczych nie powiódł się: { $error }
cmd-file-dxf-download-encoding-failed-error = Kodowanie pobieranego pliku DXF nie powiodło się: { $error }
cmd-file-dxf-import-failed-error = Import DXF nie powiódł się: { $error }
cmd-file-encoding-block-model-csv-download = Kodowanie pobieranego CSV modelu blokowego…
cmd-file-encoding-dxf-download = Kodowanie pobieranego pliku DXF…
cmd-file-encoding-triangulation-download = Kodowanie pobieranej triangulacji…
cmd-file-exit-deferred-exports = Zakończenie odroczone do czasu ukończenia eksportów w tle
cmd-file-exit-requested-no-unsaved-changes = Zażądano zakończenia bez niezapisanych zmian
cmd-file-exported-block-model-csv-path = Wyeksportowano CSV modelu blokowego do { $path }
cmd-file-exported-description-dxf-path = Wyeksportowano { $description } do DXF: { $path }
cmd-file-exported-three-drillhole-csvs-path = Wyeksportowano trzy pliki CSV otworów wiertniczych do { $path }
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
cmd-file-save-project-already-running-save = Zapis tego projektu już trwa; zapisz ponownie po jego zakończeniu
cmd-file-save-worker-ended-without-result = Proces zapisu zakończył się bez wyniku
cmd-file-saved-project-as = Zapisano projekt jako: { $path }
cmd-file-saved-project = Zapisano projekt: { $path }
cmd-file-saving-browser-storage = Zapisywanie w pamięci przeglądarki…
cmd-file-selected-block-model-no-longer = Wybrany model blokowy nie jest już wczytany
cmd-file-selected-drillhole-dataset-no-longer = Wybrany zbiór otworów wiertniczych nie jest już wczytany
cmd-file-switching-project = Przełączanie projektu…
cmd-file-triangulation-download-encoding-failed = Kodowanie pobieranej triangulacji nie powiodło się: { $error }
cmd-file-user-chose-exit-without-saving = Użytkownik wybrał zakończenie bez zapisywania
cmd-file-user-requested-exit-project-export = Użytkownik zażądał zakończenia (wymagane potwierdzenie eksportu projektu lub niezapisanej pracy)
cmd-file-viewport = Widok
cmd-file-wait-current-project-save-finish = Poczekaj na zakończenie bieżącego zapisu projektu
cmd-file-wait-current-project-switch-finish = Poczekaj na zakończenie przełączania bieżącego projektu
cmd-file-wait-project-operation-finish-before = Poczekaj na zakończenie operacji na projekcie przed odrzuceniem zmian
cmd-file-wait-project-revert-finish-before = Poczekaj na zakończenie przywracania projektu przed zapisaniem
cmd-folder-collection-named-name-already-exists = Kolekcja o nazwie „{ $name }” już istnieje
cmd-folder-collection-no-longer-exists = Ta kolekcja już nie istnieje
cmd-folder-created-collection-name = Utworzono kolekcję „{ $name }”
cmd-folder-deleted-collection-name = Usunięto kolekcję „{ $name }”
cmd-folder-moved-item-into-collection-name = Przeniesiono element do kolekcji „{ $name }”
cmd-folder-moved-item-root-section = Przeniesiono element do katalogu głównego sekcji { $section }
cmd-folder-renamed-collection-before-after = Zmieniono nazwę kolekcji „{ $before }” na „{ $after }”
cmd-folder-section-cannot-hold-item = Ta sekcja nie może zawierać tego elementu
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
cmd-offset-skipped-count-circle-s-offset = Pominięto okręgi: { $count } — odległość przesunięcia jest większa niż promień
cmd-omf-could-not-open-project-source = Nie udało się otworzyć projektu { $source_name }: { $error }
cmd-omf-create-open-project-before-merging = Utwórz lub otwórz projekt przed scalaniem danych
cmd-omf-dataset-name-count-working-section = Zbiór danych „{ $name }”: nie udało się przywrócić przekrojów roboczych: { $count }: { $details }
cmd-omf-field-codes-partly-coloured = Zbiór danych „{ $name }”: pole „{ $field }” zapisano z pokolorowanymi kodami: { $saved } z { $total }; pozostałym nadano kolory wygenerowane.
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
cmd-point-cloud-classified = Sklasyfikowano { $name }: grunt { $ground }, roślinność { $vegetation } i szum { $noise } z { $count } punktów
cmd-point-cloud-classifying-point-clouds = Klasyfikowanie chmur punktów
cmd-point-cloud-join-dropped-classifications = Odrzucono klasyfikacje punktów: niektóre z łączonych chmur nie są sklasyfikowane, a częściowo sklasyfikowanej chmury nie można przefiltrować do gruntu.
cmd-point-cloud-failed-classify-point-clouds-error = Nie udało się sklasyfikować chmur punktów: { $error }
cmd-point-cloud-failed-join-point-clouds-error = Nie udało się połączyć chmur punktów: { $error }
cmd-point-cloud-failed-load-point-cloud-error = Nie udało się wczytać chmury punktów: { $error }
cmd-point-cloud-joined-count-clouds-into-name = Połączono { $count } chmur w { $name } ({ $points } punktów)
cmd-point-cloud-joining-name = Łączenie: { $name }
cmd-point-cloud-loaded-point-cloud-name-count = Wczytano chmurę punktów { $name } ({ $count } punktów)
cmd-point-cloud-point-cloud-classification-discarded = Klasyfikacja chmury punktów odrzucona: chmura zmieniła się w trakcie działania. Uruchom ją ponownie.
cmd-point-cloud-point-cloud-loader-disconnected-path = Program wczytujący chmurę punktów rozłączony dla { $path }
cmd-point-cloud-select-one-more-loaded-point = Wybierz jedną lub więcej wczytanych chmur punktów przed ich klasyfikacją
cmd-point-cloud-select-two-more-loaded-point = Wybierz dwie lub więcej wczytanych chmur punktów przed ich połączeniem
cmd-point-cloud-tin-max-edge-disabled = (maks. krawędź wyłączona)
cmd-point-cloud-tin-max-edge-max-edge = (maks. krawędź { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = TIN chmury punktów nie powiódł się: { $error }
cmd-point-cloud-tin-filtered-ground = TIN terenu: odfiltrowano do { $ground } punktów gruntu z { $total }
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
cmd-reference-surface-build-surface-failed-error = Budowa powierzchni nie powiodła się: { $error }
cmd-reference-surface-building-surface = Budowanie powierzchni…
cmd-reference-surface-built-surface-name-inside-grid = Zbudowano powierzchnię { $name } z węzłów siatki w zasięgu: { $inside }; w niej węzłów: { $vertex_count }, ścianek: { $face_count }, zakres z od { $low } do { $high }{ $support }{ $controls }
cmd-reference-surface-built-surface-name-from-vertex = Zbudowano powierzchnię { $name } z punktów: { $vertex_count } na ścianki: { $face_count }, zakres z od { $low } do { $high }{ $support }{ $coincident }{ $controls }
cmd-reference-surface-control-string-index-could-not = Nie udało się dodać linii kontrolnej { $index } do siatki
cmd-reference-surface-control-string-index-crosses-itself = Linia kontrolna { $index } przecina samą siebie w rzucie w ({ $x }, { $y })
cmd-reference-surface-control-string-index-doubles-back = Linia kontrolna { $index } zawraca na samą siebie w rzucie w ({ $x }, { $y })
cmd-reference-surface-control-string-index-ends-where = Linia kontrolna { $index } kończy się tam, gdzie się zaczyna; zamknij ją, aby użyć jej jako maski
cmd-reference-surface-control-string-index-has-count = Linia kontrolna { $index } ma różnych wierzchołków: { $count }; linia kontrolna wymaga co najmniej { $minimum }
cmd-reference-surface-control-string-index-has-non = Linia kontrolna { $index } ma współrzędne nieskończone
cmd-reference-surface-control-string-index-no-longer = Linia kontrolna { $index } nie jest już dostępna
cmd-reference-surface-control-string-index-runs-along = Linia kontrolna { $index } biegnie wzdłuż linii zasięgu w rzucie; nie jest to jeszcze obsługiwane
cmd-reference-surface-control-string-overrides-pick-x = Linia kontrolna zastępuje wskazanie w ({ $x }, { $y }): wskazanie { $pick } m, linia kontrolna { $control } m, różnica { $difference } m
cmd-reference-surface-control-strings-b-disagree-x = Linie kontrolne { $a } i { $b } są niezgodne w ({ $x }, { $y }): { $za } m wobec { $zb } m, różnica { $difference } m
cmd-reference-surface-control-strings-b-run-along = Linie kontrolne { $a } i { $b } biegną wzdłuż siebie w rzucie; nie jest to jeszcze obsługiwane
cmd-reference-surface-count-control-string-s-entered = ; linie kontrolne: { $count } wprowadzone jako punkty: { $points }{ $crossings }
cmd-reference-surface-count-other-strings-hidden = Pozostałe linie kontrolne ({ $count }) są ukryte; Odkryj wszystko na pasku widoku przywraca je
cmd-unhide-all-count = Ponownie pokazano ukryte obiekty: { $count }
cmd-unhide-all-objects-items-count = Ponownie pokazano ukryte obiekty: { $objects }, elementy: { $items }
cmd-unhide-all-nothing-hidden = Brak ukrytych obiektów we wczytanych warstwach
cmd-reference-surface-count-control-string-s-vertices = ; linie kontrolne: { $count } z wierzchołkami: { $vertices }{ $crossings }
cmd-reference-surface-count-point-s-inside-extent = Punktów w zasięgu: { $count }; powierzchnia wymaga co najmniej { $minimum }
cmd-reference-surface-count-point-s-outside-extent = ; punkty poza zasięgiem ukształtowały ją jako podparcie: { $count }
cmd-reference-surface-count-point-s-selected-surface = Wybrano punktów: { $count }; powierzchnia wymaga co najmniej { $minimum }
cmd-reference-surface-count-places-stop-build = Miejsca w liniach kontrolnych blokujące budowę: { $count }, każde oznaczone okręgiem:
cmd-reference-surface-cleaned-heading = Zbuduj powierzchnię oczyściła własną kopię linii kontrolnych, tak jak zrobiłyby to Oczyść linie i Połącz wszystko na średniej wysokości; linie w projekcie pozostają bez zmian:
cmd-reference-surface-cleaned-repeats = Miejsca, w których powtórzone punkty scalono w jeden: { $count }, w { $places }
cmd-reference-surface-cleaned-spikes = Usunięte szpice: { $count }, w { $places }
cmd-reference-surface-cleaned-retraces = Przycięte odcinki biegnące z powrotem po linii: { $count }, w { $places }
cmd-reference-surface-cleaned-loops = Wycięte pętle w miejscach, gdzie linia przecina samą siebie: { $count }, w { $places }
cmd-reference-surface-cleaned-zeros = Usunięte wierzchołki na z = 0: { $count }, w { $places }
cmd-reference-surface-cleaned-heights = Usunięte pojedyncze wysokości daleko od sąsiednich: { $count }, w { $places }
cmd-reference-surface-cleaned-shared-cut = Odcinki wspólne dla dwóch linii wycięte z krótszej: { $count }, w { $places }
cmd-reference-surface-cleaned-removed = Linie biegnące wzdłuż innej na całej długości usunięte z kopii: { $count }, w { $places }
cmd-reference-surface-cleaned-joined-small = Skrzyżowania z różnicą { $limit } m lub mniejszą połączone na średniej wysokości: { $count }, w { $places }
cmd-reference-surface-cleaned-joined-on-request = Skrzyżowania z różnicą ponad { $low } m i do { $high } m połączone na średniej wysokości: { $count }, w { $places }
cmd-reference-surface-cleaned-vertex-shared = Wstawione wspólne wierzchołki tam, gdzie linie różnią się o ponad { $limit } m: { $count }, w { $places }
cmd-reference-surface-left-out-count = Linie kontrolne pominięte w tej budowie: { $count }, każda oznaczona okręgiem i zaznaczona; powierzchnia jest budowana z pozostałych:
cmd-reference-surface-left-out-below = Linia { $string } pominięta: leży { $amount } m poniżej { $others } w { $places }
cmd-reference-surface-left-out-above = Linia { $string } pominięta: leży { $amount } m powyżej { $others } w { $places }
cmd-reference-surface-left-out-above-and-below = Linia { $string } pominięta: leży { $amount } m powyżej i poniżej { $others } w { $places }
cmd-reference-surface-left-out-along = Linia { $string } pominięta: biegnie wzdłuż { $others } w { $places }
cmd-reference-surface-left-out-range = od { $low } do { $high }
cmd-reference-surface-left-out-other-string = linii { $string }
cmd-reference-surface-left-out-other-strings = linii { $strings }
cmd-reference-surface-left-out-too-short = Linia { $string } pominięta: ma mniej niż dwa różne wierzchołki, w ({ $x }, { $y })
cmd-reference-surface-left-out-ends-where-it-starts = Linia { $string } pominięta: kończy się tam, gdzie zaczyna, w ({ $x }, { $y })
cmd-reference-surface-left-out-turns-back = Linia { $string } pominięta: zawraca w ({ $x }, { $y })
cmd-reference-surface-left-out-crosses-itself = Linia { $string } pominięta: przecina samą siebie w ({ $x }, { $y })
cmd-reference-surface-left-out-points-disagree = Linia { $string } pominięta: dwa jej punkty w jednym miejscu w rzucie różnią się wysokością o { $miss } m, w ({ $x }, { $y })
cmd-reference-surface-left-out-none-left = Pominięcie linii, które kolidują lub są zniekształcone, nie zostawiłoby żadnej linii kontrolnej, więc nic nie zostanie zbudowane
cmd-reference-surface-thinned = Linie kontrolne dawały zbyt wiele punktów dla jednej powierzchni, więc budowa przerzedziła ich kopię: zachowano punkty: { $kept }, na końcach, w miejscach przecięć i w każdym wierzchołku oddalonym o ponad { $tolerance } m od linii bez niego, w rzucie lub wysokości{ $raised }, a punkty rozmieszczono wzdłuż nich co { $spacing } m; użyto punktów: { $used } z budżetu { $budget }
cmd-reference-surface-thinned-raised = (podniesiona z { $first } m, bo mniej by się nie zmieściło)
cmd-reference-surface-thin-refused = Linie kontrolne nie mieszczą się w budżecie { $budget } punktów dla jednej powierzchni: nawet zachowując tylko końce, przecięcia i wierzchołki oddalone o ponad { $tolerance } m od linii bez nich, dają punkty: { $kept }, razem { $total } ze wskazaniami ({ $picks }); nic nie zostanie zbudowane
cmd-reference-surface-count-refused-strings-selected = Odrzucone linie kontrolne są teraz zaznaczone: { $count }
cmd-reference-surface-count-point-s-shared-plan = ; punkty o wspólnym położeniu w rzucie zachowano jednokrotnie: { $count }
cmd-reference-surface-delaunay-insert-failed-error = Wstawienie Delaunaya nie powiodło się: { $error }
cmd-reference-surface-extent-must-closed-string = Zasięg musi być linią zamkniętą
cmd-reference-surface-extent-string-crosses-itself-plan = Linia zasięgu przecina samą siebie w rzucie
cmd-reference-surface-extent-string-has-non-finite = Linia zasięgu ma współrzędne nieskończone
cmd-reference-surface-extent-string-needs-least-three = Linia zasięgu wymaga co najmniej trzech różnych wierzchołków
cmd-reference-surface-extent-string-no-longer-available = Linia zasięgu nie jest już dostępna
cmd-reference-surface-meeting-count-crossing-s = spotykają się w przecięciach: { $count }
cmd-reference-surface-and-more = , … i { $more } więcej
cmd-reference-surface-no-mask-selected-surface-outline = Nie wybrano maski; powierzchnia jest przycięta do obrysu punktów plus { $buffer } m
cmd-reference-surface-no-mask-selected-surface-unclipped = Nie wybrano maski; powierzchnia nie jest przycięta
cmd-reference-surface-no-part-surface-falls-inside = Żadna część powierzchni nie mieści się w zasięgu
cmd-reference-surface-open-project-before-building-surface = Otwórz projekt przed budową powierzchni
cmd-reference-surface-points-collinear-plan-surface-needs = Punkty są współliniowe w rzucie; powierzchnia wymaga trzech punktów, które nie są współliniowe
cmd-reference-surface-select-exactly-one-closed-string = Wybierz dokładnie jedną linię zamkniętą, do której ma zostać przycięta powierzchnia
cmd-reference-surface-selected-point-has-non-finite = Wybrany punkt ma współrzędne nieskończone
cmd-reference-surface-selected-points-span-count-layers = Wybrane punkty obejmują warstwy: { $count }; powierzchnia zostanie umieszczona w: { $section }
cmd-reference-surface-control-string-index-has-two = Linia kontrolna { $index } ma dwa wierzchołki w odległości { $distance } m od ({ $x }, { $y }) w rzucie, na różnych wysokościach
cmd-reference-surface-run-record-used-point = Zapis budowy: użyto punktów: { $used } z podanych { $picks }, scalono: { $merged }, pod liniami kontrolnymi pominięto: { $left_out } (na innej wysokości: { $overridden }); { $method }, rozstaw { $spacing } m; autor { $author }, data { $date }
cmd-reference-surface-count-pair-s-points-closer = Par punktów bliższych niż { $spacing } m w rzucie, bardziej stromych niż { $degrees } stopni: { $count }; siatka nie może ich odtworzyć bez falowania:
cmd-reference-surface-steep-pair = ({ $ax }, { $ay }, { $az }) i ({ $bx }, { $by }, { $bz }): odległość { $distance } m, różnica wysokości { $rise } m, { $slope } stopni
cmd-reference-surface-surface-could-not-cut = Nie udało się przyciąć powierzchni wzdłuż linii zasięgu w pobliżu ({ $x }, { $y })
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
cmd-strat-check-checking = Sprawdzanie kolumny stratygraficznej: { $name }
cmd-strat-check-failed = Sprawdzenie kolumny stratygraficznej nie powiodło się: { $error }
cmd-strat-check-summary = Sprawdzono pole { $field } w { $name }: otworów { $holes }, oznaczonych { $flagged }
cmd-strat-check-too-many-codes = Pole { $field } w { $name } zawiera zbyt wiele kodów, aby je uporządkować
cmd-strat-import-filled = Kolumna stratygraficzna wypełniona dla pola { $field }: nazw { $names }; niezgodnych otworów: { $flagged }. Użyj Sprawdź, aby przejrzeć.
cmd-strat-import-filled-groups = Kolumna stratygraficzna wypełniona dla pola { $field }: nazw { $names } w grupach { $groups }; niezgodnych otworów: { $flagged }. Użyj Sprawdź, aby przejrzeć.
cmd-string-clean-and = i
cmd-string-clean-checks-pass = Kontrole Zbuduj powierzchnię przechodzą na warstwie { $layer }
cmd-string-clean-build-would-leave-out = Zbuduj powierzchnię pominęłaby linie { $strings } warstwy { $layer } i zbudowała z pozostałych
cmd-string-clean-checks-refuse = Kontrole Zbuduj powierzchnię nadal odrzucają warstwę { $layer }: miejsca oznaczone okręgiem: { $count }
cmd-string-clean-clean-strings = Oczyść linie
cmd-string-clean-clean-this-string = Oczyść tę linię
cmd-string-clean-cleaning-strings = Czyszczenie linii
cmd-string-clean-hand-along = Do poprawienia ręcznie: linie { $strings } biegną wzdłuż siebie w ({ $x }, { $y })
cmd-string-clean-hand-build-refuses = Do poprawienia ręcznie: Zbuduj powierzchnię nadal odrzuca linie, których nic powyżej nie wymienia: { $refusal }
cmd-string-clean-hand-crosses-itself = Do poprawienia ręcznie: linia { $string } przecina samą siebie w ({ $x }, { $y })
cmd-string-clean-hand-crossing = Do poprawienia ręcznie: linie { $strings } różnią się o { $miss } m w ({ $x }, { $y })
cmd-string-clean-hand-ends-where-it-starts = Do poprawienia ręcznie: linia { $string } kończy się tam, gdzie zaczyna, w ({ $x }, { $y })
cmd-string-clean-hand-near-miss = Do poprawienia ręcznie: linie { $strings } mijają się blisko bez spotkania, z różnicą { $miss } m, w ({ $x }, { $y })
cmd-string-clean-hand-points-disagree = Do poprawienia ręcznie: linia { $string } ma dwa punkty w jednym miejscu w rzucie, { $miss } m różnicy wysokości, w ({ $x }, { $y })
cmd-string-clean-hand-too-short = Do poprawienia ręcznie: linia { $string } ma mniej niż dwa różne wierzchołki, w ({ $x }, { $y })
cmd-string-clean-hand-turns-back = Do poprawienia ręcznie: linia { $string } zawraca w ({ $x }, { $y })
cmd-string-clean-height-dropped = Linia { $string }: usunięto wysokość odległą o { $offset } m od sąsiednich w ({ $x }, { $y }, { $z })
cmd-string-clean-join-all-at-halfway = Połącz wszystko na średniej wysokości
cmd-string-clean-clear-rings = Usuń okręgi
cmd-string-clean-join-all-crossing = Do Połącz wszystko na średniej wysokości: linie { $strings } różnią się o { $miss } m w ({ $x }, { $y })
cmd-string-clean-join-here-at-halfway = Połącz tutaj na średniej wysokości
cmd-string-clean-joining-strings = Łączenie linii na średniej wysokości
cmd-string-clean-joined = Linie { $strings }: połączone na średniej wysokości { $z } w ({ $x }, { $y }), różniły się o { $miss } m
cmd-string-clean-layer = Warstwa { $layer }: linie: { $strings }
cmd-string-clean-left-arcs = Linia { $string } pozostawiona jak narysowano: ma łuki
cmd-string-clean-left-not-finite = Linia { $string } pozostawiona jak narysowano: ma nieskończone lub nieliczbowe współrzędne
cmd-string-clean-loop-cut = Linia { $string }: wycięto pętlę z { $count } wierzchołków tam, gdzie przecina samą siebie, w ({ $x }, { $y }, { $z })
cmd-string-clean-nothing-to-clean = W zaznaczonych liniach nie ma nic do oczyszczenia
cmd-string-clean-odd-above-every = Linia { $string } leży { $low } do { $high } m powyżej każdej linii, którą przecina ({ $count } z { $total } przecięć)
cmd-string-clean-odd-above-misses = Linia { $string } leży { $low } do { $high } m powyżej każdej linii, od której różni się o ponad { $limit } m ({ $count } z { $total } przecięć)
cmd-string-clean-odd-below-every = Linia { $string } leży { $low } do { $high } m poniżej każdej linii, którą przecina ({ $count } z { $total } przecięć)
cmd-string-clean-odd-below-misses = Linia { $string } leży { $low } do { $high } m poniżej każdej linii, od której różni się o ponad { $limit } m ({ $count } z { $total } przecięć)
cmd-string-clean-removed = Linia { $string }: usunięta, biegła wzdłuż linii { $kept } na całej długości
cmd-string-clean-repeats-merged = Linia { $string }: scalono { $count } powtórzone punkty w jeden w ({ $x }, { $y }, { $z })
cmd-string-clean-retrace-dropped = Linia { $string }: przycięto { $count } wierzchołki biegnące z powrotem po linii, w ({ $x }, { $y }, { $z })
cmd-string-clean-ring-title = Linie { $strings }
cmd-string-clean-ring-title-miss = Linie { $strings }, { $miss } m od siebie
cmd-string-clean-rings = Linie z okręgami
cmd-string-clean-run-finished = { $label }: gotowe, zmiany: { $edits }, miejsca oznaczone okręgiem: { $rings }
cmd-string-clean-run-started = { $label }: linie: { $strings }, warstwy: { $layers }
cmd-string-clean-shared-cut = Linia { $string }: wycięto { $length } m wspólne z linią { $kept }, w ({ $x }, { $y }, { $z })
cmd-string-clean-spike-dropped = Linia { $string }: usunięto szpic w ({ $x }, { $y }, { $z })
cmd-string-clean-vertex-shared = Linie { $strings }: wstawiono wspólny wierzchołek w ({ $x }, { $y }), różnią się o { $miss } m
cmd-string-clean-zero-dropped = Linia { $string }: usunięto wierzchołek na z = 0 w ({ $x }, { $y })
cmd-selection-duplicate-selection = Duplikuj zaznaczenie
cmd-selection-duplicated-count-object-s = Zduplikowano { $count } obiektów
cmd-seam-surface-clash = { $first } ({ $first_thickness } m) i { $second } ({ $second_thickness } m)
cmd-seam-surface-clash-heading = { $count } par(y) punktów miąższości leży w tym samym miejscu z różną miąższością; dokładna powierzchnia nie przejdzie przez oba:
cmd-seam-surface-failed = Powierzchnia miąższości nie powiodła się: { $error }
cmd-seam-surface-made = Utworzono { $name }: { $nodes } węzeł(y) co { $spacing } m z { $used } punktu(ów) miąższości, { $merged } scalonych, { $held } węzeł(y) utrzymanych przy zerowej miąższości; powierzchnia odniesienia { $surface }, punkty miąższości { $run }
cmd-cuts-to-surface-select-seam = Zaznacz strop i spąg pokładu do przycięcia, dwie powierzchnie siatkowe na jednej siatce
cmd-cuts-to-surface-not-one-lattice = Strop i spąg nie leżą na jednej siatce: zaznacz strop i spąg pokładu zbudowane na jednej siatce
cmd-cuts-to-surface-nothing-left = Między granicami nie zostało nic z pokładu, więc nic nie utworzono
cmd-cuts-to-surface-seam = { $roof } i { $floor }
cmd-cuts-to-surface-solid = Bryła
cmd-cuts-to-surface-no-cut = Wybierz Zachowaj poniżej, Zachowaj powyżej lub oba
cmd-cuts-to-surface-cuts-itself = Przycinana powierzchnia nie może być jednocześnie własną granicą
cmd-cuts-to-surface-no-memory = Za mało pamięci na przyciętą powierzchnię
cmd-cuts-to-surface-cutting = Przycinanie powierzchni
cmd-cuts-to-surface-upper = zachowaj poniżej { $name }
cmd-cuts-to-surface-upper-level = zachowaj poniżej rzędnej { $level }
cmd-cuts-to-surface-lower = zachowaj powyżej { $name }
cmd-cuts-to-surface-lower-level = zachowaj powyżej rzędnej { $level }
cmd-cuts-to-surface-lower-depth = zachowaj powyżej { $depth } m pod { $name }
cmd-cuts-to-surface-made = Utworzono { $roof }, { $floor } i { $solid } z { $surface }: z { $nodes } węzł(ów) w { $upper } strop położono płasko na Zachowaj poniżej, w { $lower } spąg położono płasko na Zachowaj powyżej, { $removed } usunięto tam, gdzie strop i spąg oba leżały poza granicą, { $crossed } tam, gdzie Zachowaj poniżej leży pod Zachowaj powyżej, { $uncovered } bez granicy pod nimi; bryła { $volume } m3; granice: { $cuts }
cmd-cuts-to-surface-not-cut = { $surface } nie przycięto: każdy węzeł leży już w granicach { $cuts }, więc nie utworzono powierzchni
cmd-cuts-to-surface-uncovered = { $surface }: { $count } węzł(ów) nie ma pod sobą powierzchni granicznej i pozostawiono je bez zmian
cmd-seam-surface-held-edge = { $count } węzeł(y) dalej niż { $reach } m poza obrysem punktów miąższości zachowało miąższość osiągniętą w tym miejscu
cmd-seam-surface-making = Tworzenie powierzchni miąższości
cmd-seam-surface-name = { $seam } { $side }
cmd-seam-surface-points-layer = { $seam } { $side } punkty
cmd-seam-surface-no-memory = Za mało pamięci na siatkę miąższości
cmd-seam-surface-no-run = { $name } nie ma jeszcze punktów miąższości: najpierw utwórz dla niej punkty miąższości
cmd-seam-surface-run = { $name }, { $count } punkt(y)
cmd-seam-surface-stale-run = { $name } zbudowano ponownie po utworzeniu jej punktów miąższości: utwórz punkty miąższości jeszcze raz
cmd-seam-surface-too-few-points = { $count } punkt(y) miąższości; powierzchnia miąższości wymaga co najmniej { $minimum }
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
cmd-thickness-not-a-grid = Nie można mierzyć względem { $name }: { $reason }
cmd-thickness-not-a-grid-cells = nie jest jedną regularną siatką kwadratowych komórek, jaką tworzy Zbuduj powierzchnię
cmd-thickness-not-a-grid-heights = dwa jej wierzchołki dzielą węzeł siatki na różnych wysokościach
cmd-thickness-not-a-grid-large = jej siatka przekroczyłaby limit węzłów { $budget }
cmd-thickness-points-and-more = i { $more } więcej
cmd-thickness-points-checking-grid = Sprawdzanie powierzchni
cmd-thickness-points-column-clash = { $dataset } ma już kolumnę "{ $column }", która przyszła z danymi, więc nie zapisano w niej miąższości. Punkty mimo to utworzono.
cmd-thickness-points-dialog-closed = Okno punktów miąższości zamknięto przed wybraniem pliku
cmd-thickness-points-failed = Punkty miąższości nie powiodły się: { $error }
cmd-thickness-points-layer = { $seam } punkty miąższości
cmd-thickness-points-left-out-heading = Pominięte ({ $count }):
cmd-thickness-points-left-out-hole = otwór { $hole }: { $reason }
cmd-thickness-points-left-out-measured = pomiar { $id }, wiersz { $line }: { $reason }
cmd-thickness-points-made = Punkty miąższości { $name }: { $holes } z otworów, { $measured } z pomiarów, { $left_out } pominiętych, { $without } otwór(ów) bez pokładu; mierzone względem { $surface }
cmd-thickness-points-making = Tworzenie punktów miąższości
cmd-thickness-points-no-layer = brak warstwy
cmd-thickness-points-open-project = Otwórz projekt przed tworzeniem punktów miąższości
cmd-thickness-points-pairs-filter = CSV zmierzonych par
cmd-thickness-points-pairs-missing-columns = W { $name } brakuje kolumn(y) { $columns }; plik zmierzonych par wymaga { $expected }
cmd-thickness-points-pairs-not-csv = { $name } nie jest czytelnym plikiem CSV: { $error }
cmd-thickness-points-pairs-not-read = Nie można odczytać { $name }
cmd-thickness-points-pairs-unreadable = Nie można odczytać pliku zmierzonych par: { $error }
cmd-thickness-points-project-changed = Projekt zmienił się podczas tworzenia punktów miąższości; nic nie dodano
cmd-thickness-points-reason-missing-value = współrzędna stropu lub spągu jest pusta lub nie jest liczbą
cmd-thickness-points-reason-no-floor = brak spągu
cmd-thickness-points-reason-no-trace = brak trajektorii, na której można go umieścić
cmd-thickness-points-reason-outside = poza powierzchnią odniesienia
cmd-thickness-points-reason-overturned = odwrócony: poza zakresem
cmd-thickness-points-saved = Zapisano { $count } wartości miąższości rzeczywistej w kolumnie "{ $column }" zbioru { $dataset }, na każdym interwale stropu
cmd-thickness-points-saved-cleared = Wyczyszczono { $count } wcześniejszych wartości w otworach pominiętych w tym przebiegu
cmd-thickness-points-saved-replaced = Zastąpiono { $count } wcześniejszych wartości z poprzedniego przebiegu
cmd-thickness-points-saved-unchanged = Kolumna "{ $column }" zbioru { $dataset } już zawiera te wartości
cmd-thickness-points-surface-gone = Zaznaczona powierzchnia nie jest już wczytana
cmd-thickness-points-select-one-surface = Zaznacz jedną powierzchnię odniesienia (zaznaczono { $count })
cmd-view-centre-rotation-not-available-flying = Środek obrotu jest niedostępny w trybie lotu
cmd-view-fixed-centre-rotation-x-y = Ustawiono środek obrotu w punkcie { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = Pod kursorem nie ma punktu, na którym można ustawić środek obrotu
cmd-view-released-centre-rotation = Zwolniono środek obrotu
cmd-view-reset-view-fit-extents = Zresetowano widok (dopasowano do zasięgu)
cmd-view-reset-view-plan-same-distance = Zresetowano widok (rzut z góry z tej samej odległości; kliknij ponownie, aby dopasować do zasięgu)
cmd-view-set-cinematic-view-enabled = Widok filmowy = { $enabled }
cmd-view-set-topology-wireframes-enabled = Ustawiono siatki krawędziowe topologii = { $enabled }
cmd-view-set-view-points-enabled = Ustawiono widoczność punktów = { $enabled }
cmd-view-set-xy-grid-enabled = Siatka XY włączona = { $enabled }
cmd-view-zoom-extents-preserving-angle = Przybliżono do zasięgu (z zachowaniem kąta)

## Common strings

common-add-product = Dodaj produkt
common-appearance = Wygląd...
common-background = Tło
common-block-model = Model blokowy
common-block-models = Modele blokowe
common-borehole-inspector = Inspektor otworów wiertniczych
common-build-surface = Zbuduj powierzchnię
common-build-surface-ellipsis = Zbuduj powierzchnię...
common-cancelled = Anulowano
common-chamfer = Fazowanie
common-choose = Wybierz...
common-circle = Okrąg
common-classify = Klasyfikuj
common-classify-point-clouds = Klasyfikuj chmury punktów
common-click-point-fix-centre-rotation = Kliknij punkt, aby ustawić na nim środek obrotu
common-clip-surface-polyline = Przytnij powierzchnię polilinią...
common-closed = Zamknięta
common-collection = Kolekcja
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
common-delete-collection = Usuń kolekcję
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
common-file-was-left-out-downhole = Plik { $file } został pominięty w geofizyce otworowej: { $error }
common-filter = Filtr
common-fly-mode = Tryb lotu
common-generate-contour-lines = Generuj poziomice...
common-hide-all = Ukryj wszystko
common-hide-selection = Ukryj zaznaczenie
common-unhide-all = Odkryj wszystko
common-hole-id = ID otworu
common-ignore = Ignoruj
common-import-csv-block-model = Importuj model blokowy CSV
common-import-dxf = Importuj DXF
common-incline-design-project = Projekt Incline Design
common-join = Połącz...
common-join-point-clouds = Połącz chmury punktów
common-joined-cloud = Połączona chmura
common-layer = Warstwa
common-legend = Legenda
common-line = Linia
common-line-weight = Grubość linii
common-link-geophysics = Powiąż geofizykę...
common-load-drillholes-before-linking-geophysics = Wczytaj zbiór otworów wiertniczych przed powiązaniem z nim geofizyki
common-lock-all = Zablokuj wszystko
common-lock-selection = Zablokuj zaznaczenie
common-m = m
common-max = Maks.
common-merge-shell-into-topology = Scal powłokę z topologią
common-merge-shell-into-topology-ellipsis = Scal powłokę z topologią...
common-modelling = Modelowanie
common-move-collar = Przesuń wylot
common-move-collection = Przenieś do kolekcji
common-move-design = Przesuń projekt
common-move-selection = Przesuń zaznaczenie
common-name-has-no-readable-size = { $name } nie ma czytelnego rozmiaru
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
common-ok = OK
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
common-reference-points = Punkty odniesienia...
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
common-thickness-points = Punkty miąższości
common-thickness-points-ellipsis = Punkty miąższości...
common-thickness-surfaces = Powierzchnie miąższości
common-thickness-surfaces-ellipsis = Powierzchnie miąższości...
common-clip-to-surface-ellipsis = Przytnij do powierzchni...
common-triangulations = Triangulacje
common-trim-topology = Przytnij do topologii...
common-undo = Cofnij
common-undrape-all = Zdejmij wszystkie nakładki
common-uniform-white = Jednolita biel
common-unknown = Nieznane
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
csv-drill-hole-rows-for-undefined-holes = Wierszy dotyczących otworu, którego nie definiuje geometria zestawu: { $count }
csv-drill-hole-count-rows-were-skipped-total = Łącznie pominięto wierszy: { $count }
csv-drill-hole-csv-file-empty = Plik CSV jest pusty
csv-drill-hole-csv-has-too-many-unreadable = Plik CSV ma zbyt wiele nieczytelnych bajtów, aby go naprawić; prawdopodobnie jest w starszym kodowaniu, więc zapisz go jako UTF-8 i zaimportuj ponownie
csv-drill-hole-csv-header-has-no-columns = Nagłówek CSV nie ma kolumn
csv-drill-hole-csv-headers-must-nonblank-unique = Nagłówki CSV muszą być niepuste i unikatowe
csv-drill-hole-geophysics-needs-geometry = Geofizyka otworowa wymaga w zestawie pliku wylotów lub jawnych odcinków, do którego otworów zostanie przypisana
csv-drill-hole-azimuth-out-of-range = Plik { $file } zawiera wiersze, w których azymut nie mieści się w zakresie od 0 do 360: { $count }
csv-drill-hole-dip-out-of-range = Plik { $file } zawiera wiersze, w których upad nie mieści się w zakresie od -90 do 90: { $count }; te wiersze odczytano bez kierunku
csv-drill-hole-file-inclination-values-could-angle = W pliku { $file } wszystkie wartości nachylenia, które mogłyby być kątem, są równe zero lub mniejsze, więc kolumnę odczytano jako upad, ujemny w dół
csv-drill-hole-file-maps-gamma-density-column = Plik { $file } mapuje kolumnę gamma lub gęstości dwukrotnie
csv-drill-hole-invalid-utf8 = Plik { $file } nie jest prawidłowym UTF-8; zastąpiono nieczytelne bajty: { $count } w komórkach: { $cells }; uszkodzona komórka nie jest odczytywana jako dane
csv-drill-hole-file-requires-gamma-density-column = Plik { $file } wymaga kolumny gamma lub gęstości
csv-drill-hole-row-undefined-hole = Wiersz { $row } pliku { $file } dotyczy DHID „{ $dhid }”, otworu, którego nie definiuje geometria zestawu
csv-drill-hole-holes-hole-s-carry-overlapping = Otwory z nakładającymi się interwałami, np. pokład zapisany obok swoich podziałów: { $holes }: { $summary }
csv-drill-hole-skipped-row-reason = Pominięto wiersz: { $reason }
csv-drill-hole-row-attribute-not-number = { $file } wiersz { $row } ma '{ $value }' w kolumnie liczbowej
csv-drill-hole-row-repeats-dhid = { $file } wiersz { $row } powtarza DHID '{ $dhid }'
csv-drill-hole-most-rows-unreadable = { $file }: nie udało się odczytać { $skipped } z { $count } wierszy; przyczyny są w konsoli
csv-drill-hole-file-maps-dip-column-twice = { $file } mapuje kolumnę upadu lub nachylenia dwukrotnie
csv-drill-hole-row-has-no-geometry = { $file } wiersz { $row } nie ma pełnej geometrii XYZ ani azymutu/upadu
csv-drill-hole-row-invalid-interval = { $file } wiersz { $row } ma nieprawidłowy interwał { $from }..{ $to } dla DHID '{ $dhid }'
csv-drill-hole-row-zero-length-segment = { $file } wiersz { $row } ma odcinek o zerowej długości na { $depth } dla DHID '{ $dhid }'
csv-drill-hole-row-unreadable-value = { $file } wiersz { $row } ma nieczytelną wartość
csv-drill-hole-csv-is-wide-text = CSV to tekst UTF-16 lub UTF-32; zapisz go jako UTF-8 i zaimportuj ponownie
csv-drill-hole-csv-holds-nul-bytes = CSV zawiera bajty NUL w całym pliku, więc nie jest tekstem UTF-8; jeśli zapisano go jako UTF-16 lub UTF-32, zapisz go jako UTF-8 i zaimportuj ponownie
csv-drill-hole-overlap-field-summary = { $field } w { $count } otworach, np. { $examples }
csv-geophysics-above-5 = powyżej 5
csv-geophysics-below-0-5 = poniżej 0,5
csv-geophysics-count-more = (+{ $count } więcej)
csv-geophysics-count-rows-were-skipped-total = Łącznie pominięto wierszy: { $count } w { $file }
csv-geophysics-csv-has-record-longer-than = CSV ma rekord dłuższy niż { $limit } MiB: plik nie ma podziałów wierszy tam, gdzie ma je CSV, lub nie jest tekstem
csv-geophysics-csv-has-unterminated-quoted-field = CSV ma niezamknięte pole w cudzysłowie
csv-geophysics-curve-file-was-left-out = Krzywa { $curve } w { $file } została pominięta: większość jej odczytów jest { $side }, więc jej mediana leży poza zakresem 0,5–5 g/cc, a jednostka wygląda na błędną (oczekiwano g/cc). Incline nie przelicza jednostek; popraw eksport i powiąż plik ponownie
csv-geophysics-file-empty = Plik { $file } jest pusty
csv-geophysics-file-has-no-curve-no = Plik { $file } nie ma krzywej: żadna kolumna poza ID otworu i głębokością nie zawiera liczb
csv-geophysics-file-mapping-has-mapped-columns = Mapowanie pliku { $file } ma kolumn: { $mapped }, CSV ma: { $found }
csv-geophysics-file-no-longer-matches-its = Plik { $file } nie zgadza się już ze swoim indeksem: powiąż go ponownie
csv-geophysics-file-not-grouped-hole-its = Plik { $file } nie jest pogrupowany według otworów: wiersze jego otworów są rozrzucone w zbyt wielu seriach. Posortuj go według ID otworu, następnie głębokości i powiąż ponownie
csv-geophysics-file-requires-one-dhid-one = Plik { $file } wymaga jednej kolumny DHID i jednej kolumny głębokości
csv-geophysics-row-blank-hole-id = Wiersz { $row } pliku { $file } ma puste ID otworu
csv-geophysics-row-column-count = Wiersz { $row } pliku { $file } ma kolumn: { $found }; oczekiwano: { $expected }
csv-geophysics-row-negative-depth = Wiersz { $row } pliku { $file } ma ujemną głębokość
csv-geophysics-row-no-depth = Wiersz { $row } pliku { $file } nie ma czytelnej głębokości
csv-geophysics-file-s-path-not-valid = ścieżka pliku nie jest prawidłowym UTF-8, którego projekt nie może zapisać: zmień nazwę pliku lub jego folderu i powiąż go ponownie
csv-geophysics-rows-skipped = { $file }: nie udało się odczytać wierszy: { $skipped } z { $rows }; przyczyny są w konsoli
csv-geophysics-runs-not-grouped = Geofizyka dla otworów: { $count } występuje w więcej niż jednej serii, bez grupowania według otworów; każda kolejna seria dodaje tylko głębokości, dla których otwór nie ma odczytu: { $holes }
csv-geophysics-linked-downhole-geophysics-from-file = Powiązano geofizykę otworową z { $file }: otwory: { $holes }, krzywe { $curves }; odczytano wierszy: { $rows }, pominięto: { $skipped }. Odczyty pozostają w pliku i są czytane po jednym otworze
csv-geophysics-no-readings = brak odczytów
csv-geophysics-no-usable-depth-step = brak użytecznego kroku głębokości
csv-geophysics-run-count-mismatch = Odczytano serii otworu { $hole }: { $read }, powiązanie ma: { $runs }
csv-geophysics-rows-geophysics-row-s-count = Wiersze geofizyki: { $rows } dla otworów, których zbiór danych nie definiuje, nie zostały powiązane: { $count }: { $holes }
csv-geophysics-rows-readings-would-need-samples = Odczyty: { $rows } wymagałyby próbek: { $samples }
csv-geophysics-run-hole-curve-was-not = Seria otworu { $hole } { $curve } nie została zachowana ({ $reason })
data-table-copy-selection = Kopiuj zaznaczenie
data-table-copy-table = Kopiuj tabelę
drill-hole-add = Dodaj
drill-hole-add-all = Dodaj wszystkie

## Drill strings

drill-hole-add-stop = Dodaj próg
drill-hole-add-working-section = Dodaj przekrój roboczy
drill-hole-all-rendered-intervals-opaque-white = Wszystkie renderowane interwały są nieprzezroczyście białe.
drill-hole-another-working-section-field-has = Inny przekrój roboczy tego pola ma taką nazwę.
drill-hole-assumed = Założona
drill-hole-burden-spacing-must-greater-than = Odprężenie i rozstaw muszą być większe od zera
drill-hole-cache-drill-hole-set-name-has = Zbiór otworów wiertniczych { $name } ma otworów i powiązań: { $count }, więcej niż { $capacity }, które może obsłużyć podświetlenie zaznaczenia: zaznaczenie całego zbioru nadal go podświetla, zaznaczenie pojedynczych otworów nie
drill-hole-cache-drill-hole-set-name-stations = Zbiór otworów wiertniczych { $name }: stacje: { $stations }, segmenty scalone z { $before } do { $after }, komórki: { $cells }
drill-hole-choose-valid-closed-polyline = Wybierz prawidłową zamkniętą polilinię
drill-hole-clear-filter = Wyczyść filtr
drill-hole-code-already-in-section = Kod { $code } jest już w przekroju roboczym { $section }.
drill-hole-code-outside-section-has-name = Kod spoza tego przekroju ma taką nazwę. Przekrój może mieć wspólną nazwę tylko z kodem, który zawiera.
drill-hole-colour-scale = Skala kolorów
drill-hole-count-codes = Kody: { $count }
drill-hole-count-codes-interval-no-logged = Kody: { $count }. Interwał bez zapisanej wartości pozostaje biały.
drill-hole-disc-diameter = Średnica dysku
drill-hole-appearance-title = Wygląd otworów wiertniczych: { $name }
drill-hole-drilled-diameter = Średnicy wierconej
drill-hole-every-code-lists-already-another = Każdy wymieniony kod jest już w innym przekroju roboczym.
drill-hole-every-interval-value-colour-field = Każdy interwał z wartością w polu koloru jest rysowany jako dysk o tej szerokości na linii otworu. Z daleka nigdy nie jest węższy niż kilka pikseli.
drill-hole-field = Pole
drill-hole-field-working-section = { $field } wg przekroju roboczego
drill-hole-floor = Spąg
drill-hole-grayscale = Skala szarości
drill-hole-green-yellow-red = Zielony–żółty–czerwony
drill-hole-heat = Ciepło
drill-hole-drilled-width-help = Otwór o szerokości wierconej wygląda jak rura obok geologii; zbiór tysięcy otworów wygląda jak mata.
drill-hole-line-width-help = Sam otwór jest rysowany jako linia o tej szerokości przy każdym powiększeniu.
drill-hole-however-far-eye-hole-drawn = Niezależnie od odległości obserwatora otwór jest rysowany co najmniej tak szeroko.
drill-hole-measured = Zmierzona
drill-hole-name-working-section = { $name } (przekrój roboczy)
drill-hole-never-thinner-than = Nigdy cieńszy niż
drill-hole-new-section-name = Nazwa nowego przekroju
drill-hole-new-working-section = Nowy przekrój roboczy
drill-hole-no-holes-fit-inside-boundary = Przy bieżącym odprężeniu i rozstawie żaden otwór nie mieści się w tej granicy
drill-hole-part-code = Część kodu
drill-hole-pattern-too-many-holes = Siatka przekracza maksimum { $maximum } otworów; zwiększ odprężenie lub rozstaw
drill-hole-preset = Ustawienie
drill-hole-px = px
drill-hole-rainbow = Tęcza
drill-hole-rename-out-of-sequence-hole = Zmiana nazwy „{ $from }” na „{ $to }” narusza kolejność kolumny stratygraficznej w tym otworze.
drill-hole-rename-out-of-sequence-holes = Zmiana nazwy „{ $from }” na „{ $to }” narusza kolejność kolumny stratygraficznej w otworach: { $count }.
drill-hole-rename-out-of-sequence-note = Warstwy przewrócone lub powtórzone leżą poza kolejnością, więc zmiana nazwy nie jest blokowana. OK i tak zmieni nazwę; Anuluj wraca do zmiany nazwy.
drill-hole-rename-out-of-sequence-title = Poza kolejnością
drill-hole-rename-seam-every-hole-of = Każdy otwór
drill-hole-rename-seam-hole = Otwór
drill-hole-rename-seam-holes = Otwory
drill-hole-rename-seam-horizon-intervals = Interwały w tym horyzoncie
drill-hole-rename-seam-intervals = Interwały
drill-hole-rename-seam-logged-name-kept = Nazwa z zapisu zostaje; nowa nazwa jest proponowana jako poprawka.
drill-hole-rename-seam-reason = Powód
drill-hole-rename-seam-reason-hint = Dlaczego nazwa się zmienia
drill-hole-rename-seam-seam = Pokład
drill-hole-rename-seam-title = Zmień nazwę pokładu
drill-hole-reset-colours = Zresetuj kolory
drill-hole-reset-preset = Resetuj ustawienie
drill-hole-reset-shown-colours = Zresetuj pokazane kolory
drill-hole-roof = Strop
drill-hole-rotation-offsets-must-contain-valid = Obrót i przesunięcia muszą zawierać prawidłowe liczby
drill-hole-selected-polyline-has-no-usable = Wybrana polilinia nie ma użytecznej powierzchni XY
drill-hole-shift-names-depths-kept = Przesuwane są tylko nazwy, nigdy głębokości. Nazwy z zapisu zostają; każda nowa nazwa jest proponowana jako poprawka.
drill-hole-shift-names-down-from-here-title = Przesuń nazwy w dół stąd
drill-hole-shift-names-down-title = Przesuń nazwy w dół
drill-hole-shift-names-field = Pole
drill-hole-shift-names-from-here-note = Wskazany horyzont i nazwy po tej stronie przesuwają się o jeden odcinek wzdłuż otworu; nazwy po drugiej stronie zostają. Wskazany horyzont dostaje nazwę UNK (nieznany), dopóki nie zostanie zmieniona.
drill-hole-shift-names-moved = Przesunięte nazwy
drill-hole-shift-names-not-in-column = Spoza kolumny, bez zmian
drill-hole-shift-names-reason-hint = Dlaczego nazwy się przesuwają
drill-hole-shift-names-submit = Przesuń
drill-hole-shift-names-unknown = Nazwane UNK
drill-hole-shift-names-unknown-note = Nazwy otworu przesuwają się o jeden odcinek wzdłuż otworu. Gdy kolumna nie ma nazwy za końcem przesunięcia, ten odcinek dostaje nazwę UNK (nieznany), dopóki nie zostanie zmieniona: jego interwały zostają, a nazwa jest proponowana jako poprawka.
drill-hole-shift-names-up-from-here-title = Przesuń nazwy w górę stąd
drill-hole-shift-names-up-title = Przesuń nazwy w górę
drill-hole-shown-total-codes-shown = Pokazano kodów: { $shown } z { $total }
drill-hole-shown-total-rows-shown = Pokazano wierszy: { $shown } z { $total }
drill-hole-smooth-interpolation = Interpolacja wygładzająca
drill-hole-spacing-would-scan-too-many = Ten rozstaw wymagałby przeskanowania zbyt wielu komórek siatki; zwiększ odprężenie lub rozstaw (maksimum { $maximum } otworów)
drill-hole-square = Kwadratowy
drill-hole-staggered = Przestawny
drill-hole-stepped-bands = Pasma stopniowane
drill-hole-string-discs = Linia i dyski
drill-hole-string-discs-where-intervals-overlap = Jako linia i dyski; tam, gdzie interwały się nakładają, najkrótszy jest rysowany jako dysk.
drill-hole-string-width = Szerokość linii
drill-hole-style = Styl
drill-hole-suggested-from-code-names-count = Sugerowane na podstawie nazw kodów ({ $count })
common-times-sign = ×
common-minus-sign = −
drill-hole-ticked-but-hidden-filter-count = Zaznaczone, ale ukryte przez filtr: { $count }
drill-hole-true-diameter = Średnica rzeczywista
drill-hole-unsupported-drillhole-source = Nieobsługiwane źródło otworów wiertniczych
drill-hole-width = Szerokość
drill-hole-working-section-needs-name = Przekrój roboczy wymaga nazwy.
drill-hole-working-section-set-seams-plies = Przekrój roboczy to zbiór pokładów lub warstw eksploatowanych jako jedna jednostka. Kolorowanie według niego nadaje całemu zbiorowi jeden kolor.
drill-hole-working-sections = Przekroje robocze
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
edit-delete-vertex-number = Usuń wierzchołek { $number }
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
edit-show-vertex-number-in-table = Pokaż wierzchołek { $number } w tabeli
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
explorer-empty-collection = Pusta kolekcja
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
explorer-new-collection = Nowa kolekcja
explorer-no-collection = Brak kolekcji
explorer-select-all-objects = Zaznacz wszystkie obiekty
explorer-show-thickness-table = Pokaż tabelę miąższości
explorer-settings = Ustawienia...
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
geophysics-checking-geophysics-files = Sprawdzanie plików geofizyki
geophysics-downhole-geophysics-name-could-not = Nie udało się powiązać geofizyki otworowej dla „{ $name }”: { $error }
geophysics-file-changed = Plik geofizyki zmienił się od momentu indeksowania
geophysics-file-unreadable = Plik geofizyki powiązany z „{ $name }” nie może zostać odczytany w { $path } ({ $error }); powiąż go ponownie z menu prawego przycisku zbioru danych
geophysics-linked-changed-rereading = Geofizyka powiązana z „{ $name }” zmieniła się od momentu indeksowania; ponowny odczyt
geophysics-hole-has-size-mib-geophysics = Otwór { $hole } ma { $size } MiB wierszy geofizyki, więcej niż można odczytać dla jednego otworu
geophysics-hole-needs-size-mib-its = Otwór { $hole } potrzebuje { $size } MiB na geofizykę, więcej niż pozostało w przeglądarce: wyładuj inne elementy, a następnie wyładuj i wczytaj ten zbiór danych ponownie
geophysics-linking-geophysics-name = Powiązywanie geofizyki z { $name }
geophysics-reading-geophysics-hole = Odczyt geofizyki dla { $hole }
geophysics-web-could-not-read-name-error = Nie udało się odczytać „{ $name }”: { $error }
geophysics-web-name-used-session-s-downhole = „{ $name }” jest używany do geofizyki otworowej tej sesji

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
init-gpu-limits-max-buffer-size = Limity GPU: max_buffer_size={ $max_buffer_size } MiB, max_storage_buffer_binding_size={ $max_storage_buffer_binding_size } MiB, max_storage_buffers_per_shader_stage={ $max_storage_buffers_per_shader_stage }, max_uniform_buffer_binding_size={ $max_uniform_buffer_binding_size } KiB, max_texture_dimension_2d={ $max_texture_dimension_2d }, max_bind_groups={ $max_bind_groups }
init-gpu-supports-maximum-buffer-size = GPU obsługuje maksymalny rozmiar bufora { $size } MiB; duże sceny mogą nie wyświetlać się w całości
init-surface-present-mode = Tryb prezentacji powierzchni: { $mode }
init-wgpu-error-continuing-error = Błąd wgpu (kontynuowanie): { $error }
input-could-not-read-name-error = nie udało się odczytać { $name }: { $error }
input-could-not-slice-name-error = nie udało się przyciąć { $name }: { $error }
io-add-collar-file-explicit-segments = Dodaj plik wylotów (lub plik jawnych odcinków): geofizyka otworowa jest przypisywana do otworów, które on definiuje.

## Io strings

io-ascii-points-xyz-pts = Punkty ASCII (.xyz, .pts)
io-attribute = Atrybut
io-blank-header = (pusty nagłówek)
io-block-model = Model blokowy:
io-choose-file-purpose-map-its = Wybierz przeznaczenie pliku, aby zmapować jego kolumny.
io-choose-loaded-block-model = Wybierz wczytany model blokowy
io-choose-loaded-dataset = Wybierz wczytany zbiór danych
io-choose-loaded-layer = Wybierz wczytaną warstwę
io-choose-loaded-triangulation = Wybierz wczytaną triangulację
io-choose-purpose = Wybierz przeznaczenie…
io-choose-source-file-files-import = Wybierz plik lub pliki źródłowe do zaimportowania.
io-collar = Wylot otworu
io-column-mapping = Mapowanie kolumn
io-comma-separated-values-csv = Comma-Separated Values (.csv)
io-csv-files = Pliki CSV
io-dataset = Zbiór danych:
io-default = Domyślne
io-density-read-g-cc-exported = Gęstość, odczytywana jako g/cc, tak jak wyeksportowano. Krzywa, której mediana nie mieści się w zakresie 0,5–5 g/cc, jest pomijana przy imporcie z ostrzeżeniem, ponieważ jej jednostka wygląda na błędną.
io-depth = Głębokość
io-diameter = Średnica
io-downhole-geophysics = Geofizyka otworowa
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
io-export-csv-drillholes = Eksportuj otwory wiertnicze do CSV
io-export-dxf = Eksportuj DXF
io-export-one-layer = Eksportuj jedną warstwę
io-export-open-mining-format-2 = Eksportuj Open Mining Format 2
io-export-ply = Eksportuj PLY
io-export-stl = Eksportuj STL
io-export-wavefront-obj = Eksportuj Wavefront OBJ
io-gamma-api = Gamma (API)
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Importuj
io-import-ascii-point-cloud = Importuj chmurę punktów ASCII
io-import-drillhole-csv-bundle = Importuj pakiet CSV otworów wiertniczych
io-import-geotiff = Importuj GeoTIFF
io-import-las-laz-point-cloud = Importuj chmurę punktów LAS/LAZ
io-import-open-mining-format-2 = Importuj Open Mining Format 2
io-import-pcd-point-cloud = Importuj chmurę punktów PCD
io-import-ply = Importuj PLY
io-import-stl = Importuj STL
io-import-wavefront-obj = Importuj Wavefront OBJ
io-inclination = Nachylenie
io-interval = Interwał
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-long-spaced-density-g-cc = Gęstość sondą długą (g/cc)
io-mapped-csv-bundle-csv = Zmapowany pakiet CSV (.csv)
io-measured-depth-down-hole-read = Głębokość mierzona wzdłuż otworu, odczytywana jako metry. Incline nie przelicza jednostek: ustala je baza danych, która wyeksportowała plik.
io-model-file = Plik modelu
io-name-count-files = { $name } + { $count } plików
io-natural-gamma-read-api-units = Gamma naturalna, odczytywana w jednostkach API, tak jak wyeksportowano.
io-no-csv-chosen = Nie wybrano pliku .csv
io-no-csv-files-chosen = Nie wybrano plików CSV
io-no-dxf-chosen = Nie wybrano pliku .dxf
io-no-omf-chosen = Nie wybrano pliku .omf
io-north-y = Północ / Y
io-open-mining-format-2-omf = Open Mining Format 2 (.omf)
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-projects = Projekty
io-short-spaced-density-g-cc = Gęstość sondą krótką (g/cc)
io-source-file = Plik źródłowy
io-start-x = X początku
io-start-y = Y początku
io-start-z = Z początku
io-stl = STL (.stl)
io-triangulation = Triangulacja:
io-unmapped = Niezmapowane
io-wavefront-obj = Wavefront OBJ (.obj)
io-writes-three-files-beside-name = Zapisuje trzy pliki obok wybranej nazwy: wyloty, pomiary inklinometryczne i interwały, w kolumnach importowanych przez to okno.

## Jobs strings

jobs-background-task-poll-label-ended = Zadanie w tle „{ $poll_label }” zakończyło się bez wyniku
jobs-cancelled-label-its-project-no = Anulowano „{ $label }”: jego projekt nie jest już aktywny
jobs-discarded-stale-result = Odrzucono nieaktualny wynik zadania w tle „{ $poll_label }”, ponieważ źródło zmieniło się lub zostało zamknięte
jobs-drillhole-import = import otworów wiertniczych
log-traces-auto-from-hole = Automatycznie, z tego otworu
log-traces-curve-no-reading = { $curve }: brak odczytu
log-traces-curve-value-unit = { $curve }: { $value } { $unit }
log-traces-custom-range = Zakres własny
log-traces-default-colour = Kolor domyślny
log-traces-density-scale = Skala gęstości
log-traces-depth-m = { $depth } m
log-traces-gamma = Gamma
log-traces-gamma-colour = Kolor gamma
log-traces-gamma-scale = Skala gamma
log-traces-percentile-range-no-data = Od 1. do 99. percentyla otworu, zaokrąglone na zewnątrz. Ten otwór nie ma jeszcze dla niego danych.
log-traces-percentile-range = Od 1. do 99. percentyla otworu, zaokrąglone na zewnątrz: { $range }.
log-traces-long-density = Gęstość długa
log-traces-long-density-colour = Kolor gęstości długiej
log-traces-min-max-unit = { $min } do { $max } { $unit }
log-traces-reading = Odczyt...
log-traces-short-density = Gęstość krótka
log-traces-short-density-colour = Kolor gęstości krótkiej

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
modelling-settings-modelling-settings = Ustawienia modelowania
modelling-settings-thin-plate-spline-exact = Spline cienkiej płyty, dokładny
modelling-settings-anisotropic-spline = Spline anizotropowy
modelling-settings-auto-axis-spline = Spline z automatycznymi osiami
modelling-settings-surface-method = Metoda budowy powierzchni
modelling-settings-hermite-dips = Hermite'a z upadami (wymaga pomiarów upadu)
modelling-settings-steep-pair-distance = Odległość stromej pary
modelling-settings-steep-pair-distance-help = Pary punktów bliższych niż ta odległość w rzucie i bardziej stromych niż kąt poniżej są wymieniane po udanej budowie. Nigdy nie są odrzucane ani naprawiane.
modelling-settings-steep-pair-angle = Kąt stromej pary
modelling-settings-surface-help = Jak Zbuduj powierzchnię rysuje swoją siatkę. Ustawienia na poziomie projektu, zapisywane z projektem.

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
omf-element-name-has-count-unreadable = Element „{ $name }” ma nieczytelne przekroje robocze: { $count }; zostały pominięte
omf-element-unsupported-section = Element „{ $name }” wskazuje sekcję „{ $section }”, która w tej wersji nie może pokazywać tego rodzaju elementu
omf-element-name-names-unknown-section = Element „{ $name }” wskazuje nieznaną sekcję „{ $section }”
omf-ignoring-colour-map-omf-attribute = Pominięto mapę kolorów atrybutu OMF „{ $attribute }”: { $error }
omf-mining-data-exported-incline = Dane górnicze wyeksportowane przez Incline
omf-import = Import OMF
omf-texture = Tekstura OMF
omf-validation-warnings = Ostrzeżenia walidacji OMF: { $warnings }
omf-application-metadata-dropped = Metadane aplikacji projektu „{ $application }” nie są zachowywane
omf-project-author-not-retained = Autor projektu nie jest zachowywany
omf-project-description-not-retained = Opis projektu nie jest zachowywany
omf-unsupported-metadata-keys = Projekt zawiera nieobsługiwane klucze metadanych: { $keys }
omf-skipped-drillhole-data-saved-older = Pominięto dane otworów wiertniczych zapisane w starszym układzie ({ $names }); zaimportuj je ponownie z plików źródłowych
omf-modelling-settings-unreadable = Nie udało się odczytać ustawień modelowania projektu; użyto wartości domyślnych

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
point-cloud-classify = Klasyfikuj
point-cloud-classify-vegetation = Klasyfikuj roślinność
point-cloud-cloth-resolution = Rozdzielczość tkaniny
point-cloud-cloth-resolution-about-one-half = Rozdzielczość tkaniny równa około półtorakrotnemu rozstawowi punktów najrzadszej z wybranych chmur, tak aby pod każdą cząstką znajdowały się odbicia.
point-cloud-combine-selected-point-clouds-into = Łączy wybrane chmury punktów w jedną nową chmurę, aby można było zbudować jedną triangulację obejmującą wszystkie. Kolory poszczególnych punktów są zachowane; chmura bez nich wnosi swój kolor wyświetlania.
point-cloud-selected-count = Wybrano: { $count } · punktów: { $points }
point-cloud-delete-selected-clouds-from-project = Usuń wybrane chmury z projektu po zakończeniu łączenia, zwalniając pamięć, którą zajmowałaby ich zduplikowana kopia.
point-cloud-flat-pads-structures = Płaski (place, konstrukcje)
point-cloud-ground-cloud-covers-steep-follows = Grunt, który obejmuje chmura. Stromy podąża w dół ścian od ich krawędzi; Płaski używa sztywniejszej tkaniny, która przechodzi nad dużymi budynkami i instalacjami, ale zaokrągla ostre załamania.
point-cloud-ground-threshold = Próg gruntu
point-cloud-how-far-around-each-point = Jak daleko wokół każdego punktu zliczać sąsiadów.
point-cloud-join = Połącz
point-cloud-let-cloth-follow-walls-down = Pozwól tkaninie podążać w dół ścian od ich krawędzi, gdzie jej sztywność w przeciwnym razie trzymałaby ją z dala od lica. Wyłącz tylko na łagodnym terenie zabudowanym instalacjami.
point-cloud-mark-each-point-ground-noise = Oznacz każdy punkt jako grunt, szum lub niesklasyfikowany. Tkanina jest dociskana od dołu do chmury i osiada na powierzchni gruntu; punkty w granicach progu gruntu od niej są gruntem. Istniejące klasy są zastępowane; cofnięcie je przywraca.
point-cloud-mark-isolated-returns-birds-dust = Oznacz odosobnione odbicia (ptaki, pył, błędy wielodrogowe) jako szum przed znalezieniem gruntu, aby zabłąkany niski punkt nie ściągnął tkaniny w dół.
point-cloud-mark-noise = Oznacz szum
point-cloud-minimum-neighbours = Minimalna liczba sąsiadów
point-cloud-name-assigned-joined-point-cloud = Nazwa nadana połączonej chmurze punktów.
point-cloud-name-count-points = { $name } (punktów: { $count })
point-cloud-noise-radius = Promień szumu
point-cloud-point-clouds = Chmury punktów
point-cloud-points-closer-than-settled-cloth = Punkty bliższe niż ta wartość osiadłej tkaniny, mierzone wzdłuż jej powierzchni, są gruntem.
point-cloud-points-fewer-neighbours-than-within = Punkty z mniejszą liczbą sąsiadów niż ta wartość w promieniu szumu są szumem.
point-cloud-raise-cloth-resolution-if-your = Zwiększ rozdzielczość tkaniny, jeśli komputer ma mniej pamięci RAM.
point-cloud-recommended = Zalecane
point-cloud-recover-steep-slopes = Odzyskaj strome zbocza
point-cloud-relief-dumps-rolling-ground = Rzeźba (zwałowiska, teren falisty)
point-cloud-remove-sources = Usuń źródła
point-cloud-resolution-m-points-spacing-m = { $resolution } m (punkty co ok. { $spacing } m)
point-cloud-selected-clouds-copied-into-joined = Wybrane chmury, kopiowane do połączonej chmury. Zamknij okno, aby połączyć inny zestaw.
point-cloud-selected-clouds-each-classified-its = Wybrane chmury, każda klasyfikowana osobno. Zamknij okno, aby sklasyfikować inny zestaw.
point-cloud-classify-help = Sortuje odbicia za pomocą wytrenowanego klasyfikatora, który odczytuje kształt punktów wokół każdego z nich: grunt, roślinność (podzielona wg wysokości na niską — poniżej 1 m, średnią — poniżej 3 m i wysoką) oraz wszystko inne, np. budynki i instalacje, pozostawione jako niesklasyfikowane. Wyłącz, aby używać samej tkaniny.
point-cloud-spacing-cloth-s-particles-around = Rozstaw cząstek tkaniny. Dobrym punktem wyjścia jest rozstaw punktów chmury; drobniejszy dokładniej podąża za gruntem, ale wymaga gęstszych punktów.
point-cloud-steep-pit-walls-benches = Stromy (ściany wyrobiska, piętra)
point-cloud-terrain = Teren
point-cloud-use = Użyj

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
project-steep-pair-distance-positive = Odległość stromej pary musi być dodatnią liczbą metrów
project-steep-pair-angle-range = Kąt stromej pary musi być większy niż 0 i co najwyżej 90 stopni
project-cut-depth-positive = Głębokość cięcia musi być liczbą metrów większą od 0
project-thin-plate-spline-exact = spline cienkiej płyty, dokładny
project-method-steep-pairs-under = Metoda: { $method } · strome pary bliższe niż { $distance } m, bardziej strome niż { $degrees } stopni

## Properties strings

properties-adds-view-dependent-rim-highlight = Dodaje zależne od widoku podświetlenie krawędzi na granicach bloków i materiałów. Wyłączenie tego nieznacznie zmniejsza obciążenie renderowania objętościowego.
properties-block-model-downscale = Zmniejszanie rozdzielczości modelu blokowego
properties-camera = Kamera
properties-camera-clip-planes = Płaszczyzny przycinania kamery
properties-cap-while-resizing = Ogranicz podczas zmiany rozmiaru
properties-colours-each-point-cloud-chunk = Koloruje każdy fragment chmury punktów, obrysowuje bryłę, względem której jest odrzucany poza frustum, i pokazuje na pasku stanu punkty narysowane w ostatniej klatce względem celu poziomu szczegółowości oraz łączną liczbę widocznych.
properties-colours-each-surface-chunk-outlines = Koloruje każdy fragment powierzchni, obrysowuje bryłę, względem której jest odrzucany poza frustum, i pokazuje na pasku stanu ścianki narysowane w ostatniej klatce względem łącznej liczby widocznych.
properties-dark-mode = Tryb ciemny
properties-dataset = Zbiór danych
properties-developer = Deweloperskie
properties-downscale-rasters = Zmniejsz rozdzielczość rastrów
properties-drillholes = Otwory wiertnicze
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
properties-no-drillhole-datasets-open = Brak otwartych zbiorów otworów wiertniczych.
properties-orbit-sensitivity = Czułość obrotu
properties-panel-chrome = Obramowanie paneli
properties-performance = Wydajność
properties-plan-mode = Tryb planu
properties-point-cloud-chunk-debug-view = Widok debugowania fragmentów chmury punktów
properties-presents-step-display-no-tearing = Wyświetla w synchronizacji z ekranem: bez rozrywania obrazu, a częstotliwość odświeżania ustala ekran. Po wyłączeniu klatki są wyświetlane zaraz po narysowaniu i stosowany jest limit poniżej.
properties-reflective-block-edges = Odbijające krawędzie bloków
properties-restore-defaults = Przywróć domyślne
properties-show-console = Pokaż konsolę
properties-shows-live-near-far-projection = Wyświetla na pasku stanu bieżące odległości bliskiej i dalekiej projekcji.
properties-snap-polling = Odpytywanie przyciągania
properties-surface-chunk-debug-view = Widok debugowania fragmentów powierzchni
properties-vertical-sync = Synchronizacja pionowa
properties-world-axis-gizmo = Gizmo osi świata
properties-zoom-cursor = Przybliżaj do kursora
properties-zoom-sensitivity = Czułość przybliżania
reference-points-count-holes-from-dataset = Otwory: { $count } z „{ $dataset }”
reference-points-holes-from-datasets = Otwory: { $count } z { $datasets } zbiorów danych
reference-points-holes = Otwory
reference-points-holes-points-placed-selected-when = Otwory zaznaczone w chwili otwarcia okna. Zamknij je, aby wybrać inne.
reference-points-make = Utwórz
reference-points-no-categorical-field = Brak pola kategorialnego
reference-points-no-values = Brak wartości
reference-points-one-point-per-hole-boundary = Umieszcza po jednym punkcie na otwór na stropie lub spągu pokładu, jako nową warstwę. Otwór, który dwukrotnie przechodzi przez pokład, daje wyższy i zostaje oznaczony.
reference-points-one-point-per-hole-collar = Stawia jeden punkt na otwór w jego wylocie, jako nową warstwę, z której Zbuduj powierzchnię tworzy powierzchnię terenu.
reference-points-points-at = Punkty w
reference-points-at-logged-pick = Zapisanym kontakcie
reference-points-at-collars = Wylotach otworów
reference-points-reference-points = Punkty odniesienia
reference-points-side = Strona
reference-points-working-section = Przekrój roboczy
reference-points-working-section-field = Pole przekroju roboczego
reference-surface-controls = Linie kontrolne
reference-surface-extent = Zasięg
reference-surface-points-outside-extent-still-shape = Punkty poza zasięgiem nadal kształtują powierzchnię; przycinana jest tylko powierzchnia.
reference-surface-points-surface-built-from-selected = Punkty, z których budowana jest powierzchnia, zaznaczone w chwili otwarcia okna. Zamknij okno, aby wybrać inne.
reference-surface-extent-help = Wybrana zamknięta linia, do której przycinana jest gotowa powierzchnia; punkty poza nią nadal kształtują powierzchnię.
reference-surface-selected-open-strings-surface-made = Wybrane otwarte linie, przez które przechodzi powierzchnia, zaznaczone w chwili otwarcia okna. Zamknij okno, aby wybrać inne.
reference-surface-grids-selected-points-plan-into = Buduje siatką nową powierzchnię z wybranych punktów w rzucie. Każda budowa dodaje powierzchnię.
reference-surface-change-these-under-modelling-settings = Zmień je w: Modelowanie, Ustawienia
reference-surface-triangulates-selected-points-plan-in = Triangulacja wybranych punktów w rzucie do nowej powierzchni. Każda budowa dodaje powierzchnię.

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
state-build-reference-points = Zbuduj punkty odniesienia
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
state-colour-drillholes-working-section = Koloruj otwory wiertnicze wg przekroju roboczego
state-colour-points-classification = Koloruj punkty wg klasyfikacji
state-copy-objects-layer = Kopiuj obiekty do warstwy
state-count-cloud-s = Chmury: { $count }
state-count-file-s = { $count } plików
state-count-object-s-axis-value = { $count } obiektów · { $axis } { $value }
state-count-object-s-closed = { $count } obiektów · { $closed }
state-count-object-s-layer = { $count } obiektów · { $layer }
state-count-object-s-weight = { $count } obiektów · { $weight }
state-count-object-s-z-elevation = { $count } obiektów · Z { $elevation }
state-points-controls-clipped = Punkty: { $count } · linie kontrolne: { $controls } · przycięte do linii zasięgu
state-points-controls-outline = Punkty: { $count } · linie kontrolne: { $controls } · przycięte do obrysu punktów
state-points-controls-unclipped = Punkty: { $count } · linie kontrolne: { $controls } · nieprzycięte
state-create-collection = Utwórz kolekcję
state-create-point-cloud-tin = Utwórz TIN z chmury punktów
state-create-project = Utwórz projekt
state-current-project = Bieżący projekt
state-cut-topology-pit-shell = Przytnij topologię do powłoki wyrobiska
state-cut-triangulation-polyline = Przytnij triangulację polilinią
state-cut-triangulation-z = Przytnij triangulację wg Z
state-dark-mode = Tryb ciemny
state-data-ticked-export-checklist = Dane zaznaczone na liście eksportu
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
state-export-drillhole-csv = Eksportuj CSV otworów wiertniczych
state-export-layer-dxf = Eksportuj warstwę do DXF
state-export-omf = Eksportuj OMF
state-export-project-dxf = Eksportuj projekt do DXF
state-export-triangulation = Eksportuj triangulację
state-export-viewport-image = Eksportuj obraz widoku
state-finish-closed-polyline = Zakończ zamkniętą polilinię
state-finish-open-polyline = Zakończ otwartą polilinię
state-fit-extents = Dopasuj do zasięgu
state-plan-view-then-fit-extents = Rzut z góry z tej samej odległości, potem dopasuj do zasięgu
state-fix-release-centre-both-views = Ustawia lub zwalnia środek, wokół którego obracają się oba widoki
state-folder-section = { $folder } w { $section }
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
state-member-into-folder-section = { $member } do { $folder } w { $section }
state-member-root-section = { $member } do katalogu głównego sekcji { $section }
state-move-axis-value = Przesuń do wartości osi
state-move-objects-layer = Przenieś obiekty do warstwy
state-name-count-cloud-s = { $name } · chmury: { $count }
state-name-count-holes = { $name } · { $count } otworów
state-name-count-object-s = { $name } · { $count } obiektów
state-name-z-min-z-max = { $name } · od { $z_min } do { $z_max }
state-new-collection-under-section = Nowa kolekcja w sekcji { $section }
state-next-edit = Następna edycja
state-north = Północ
state-off = Wył.
state-on = Wł.
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
state-rename-seam = Zmień nazwę pokładu
state-rename-seam-from-to = { $from } na { $to }
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
state-hidden-objects = Każdy ukryty obiekt
state-set-block-model-variable = Ustaw zmienną modelu blokowego
state-set-cinematic-view = Ustaw widok filmowy
state-set-drillhole-colour-preset = Ustaw ustawienie koloru otworów wiertniczych
state-set-drillhole-discs = Ustaw dyski otworów wiertniczych
state-set-drillhole-style = Ustaw styl otworów wiertniczych
state-set-drillhole-width = Ustaw szerokość otworów wiertniczych
state-set-entity-lock = Ustaw blokadę obiektu
state-set-grid = Ustaw siatkę
state-set-layer-lock = Ustaw blokadę warstwy
state-set-line-weight = Ustaw grubość linii
state-set-modelling-settings = Ustaw ustawienia modelowania
state-seam-surface-from-thickness = druga powierzchnia pokładu z jego punktów miąższości
state-clip-to-surface-count = powierzchnie: { $count }
state-collar-points-holes = wyloty, otwory: { $count }
state-thickness-points-holes-only = tylko otwory
state-thickness-points-with-pairs = otwory i zmierzone pary z { $name }
state-set-object-colour = Ustaw kolor obiektu
state-set-object-fill = Ustaw wypełnienie obiektu
state-set-point-visibility = Ustaw widoczność punktu
state-set-polyline-closed = Ustaw zamknięcie polilinii
state-set-raster-lock = Ustaw blokadę rastra
state-set-standard-view = Ustaw widok standardowy
state-set-topology-wireframes = Ustaw siatki krawędziowe topologii
state-set-triangulation-colour = Ustaw kolor triangulacji
state-shift-names = Przesuń nazwy
state-shift-names-down = { $field } w dół otworu
state-shift-names-down-from-here = { $field } w dół otworu od horyzontu
state-shift-names-up = { $field } w górę otworu
state-shift-names-up-from-here = { $field } w górę otworu od horyzontu
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
status-faces-chunks = Ścianki: -- / -- (--/-- fragmentów)
status-frame-rate = Liczba klatek
status-points-chunks = Punkty: -- / -- z -- (--/-- fragmentów)

## Text strings

text-could-not-build-vector-mesh = Nie udało się zbudować siatki wektorowej dla czcionki { $font }, glifu { $glyph }: { $error }
text-document-text-mesh-exceeded-its = Siatka tekstu dokumentu przekroczyła zakres indeksów u32

## Seam surface strings

seam-surface-column-other = z drugiej powierzchni
seam-surface-column-reference = z odniesienia
seam-surface-note = Tworzy drugą powierzchnię pokładu z jego punktów miąższości. Każdy przebieg dodaje powierzchnię.
seam-surface-output = Tworzy
seam-surface-output-help = Spąg pod powierzchnią stropu albo strop nad powierzchnią spągu.
seam-surface-reference-help = Powierzchnia zaznaczona w chwili otwarcia okna. Nowa powierzchnia podąża za jej siatką i obrysem.
seam-surface-run = Punkty miąższości
seam-surface-run-help = Ostatnie punkty miąższości utworzone na tej powierzchni w tej sesji.
seam-surface-table-surface = { $count } węzeł(y), zawieszone od { $surface }
seam-surface-table-title = Siatka miąższości: { $name }

## Thickness strings

thickness-points-choose-pairs = Wybierz CSV...
thickness-points-checking-surface = Sprawdzanie powierzchni...
thickness-points-clear-pairs = Wyczyść
thickness-points-column-along = Wzdłuż otworu
thickness-points-column-dip = Upad
thickness-points-column-direction = Kierunek upadu
thickness-points-column-floor = Spąg (głębokość lub z)
thickness-points-column-roof = Strop (głębokość lub z)
thickness-points-column-source = Źródło
thickness-points-column-true = Miąższość rzeczywista
thickness-points-column-vertical = Miąższość pionowa
thickness-points-column-x = X
thickness-points-column-y = Y
thickness-points-holes = Otwory
thickness-points-holes-help = Otwory zaznaczone razem z powierzchnią albo każdy wczytany otwór, jeśli żadnego nie zaznaczono. Każdy otwór, w którym zalogowano pokład, daje punkt.
thickness-points-no-pairs = Brak
thickness-points-note = Mierzy miąższość rzeczywistą pokładu w każdym otworze, prostopadle do uławicenia zaznaczonej powierzchni.
thickness-points-pairs = Zmierzone pary
thickness-points-pairs-help = Opcjonalne. Punkty stropu i spągu zmierzone w terenie, jako CSV z kolumnami id, roof_x, roof_y, roof_z, floor_x, floor_y, floor_z.
thickness-points-field-measurements = Pomiary terenowe
thickness-points-every-hole = Każdy wczytany otwór zawierający przekrój roboczy ({ $datasets } zbiór(y) danych)
thickness-points-side-note = Strona: czy zaznaczona powierzchnia to strop, czy spąg pokładu?
thickness-points-surface = Powierzchnia
thickness-points-surface-help = Powierzchnia zaznaczona w chwili otwarcia okna. Jej nachylenie przy każdym otworze wyznacza uławicenie.
thickness-points-table-surface = { $count } punkt(y), mierzone względem { $surface }
thickness-points-table-title = Punkty miąższości: { $name }
thickness-points-then-surface = Potem utwórz drugą powierzchnię
thickness-points-then-surface-help = Po utworzeniu punktów tworzy z nich drugą powierzchnię pokładu. Powierzchnie miąższości robią to samo osobno.

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
tri-clip-to-surface = Przytnij do powierzchni
tri-clip-to-surface-targets = Pokład
tri-clip-to-surface-targets-help = Strop i spąg pokładu, dwie powierzchnie siatkowe zaznaczone przy otwarciu okna. Wyższa jest stropem. Przycięcie tworzy nowy strop, spąg i bryłę; oryginały pozostają bez zmian.
tri-clip-to-surface-upper = Zachowaj poniżej
tri-clip-to-surface-upper-help = Nic nie zostaje powyżej tej granicy. Gdzie ponad nią wznosi się tylko strop, strop kładzie się płasko na granicy aż do spotkania ze spągiem; gdzie wznosi się też spąg, ta część pokładu jest usuwana. Zostaw puste, aby przycinać tylko od dołu.
tri-clip-to-surface-lower = Zachowaj powyżej
tri-clip-to-surface-lower-help = Nic nie zostaje poniżej tej granicy. Gdzie pod nią opada tylko spąg, spąg kładzie się płasko na granicy aż do spotkania ze stropem; gdzie opada też strop, ta część pokładu jest usuwana. Zostaw puste, aby przycinać tylko od góry.
tri-clip-to-surface-from-surface = Powierzchnia
tri-clip-to-surface-from-level = Rzędna
tri-clip-to-surface-from-depth = Głębokość pod powierzchnią
tri-clip-to-surface-surface = Powierzchnia
tri-clip-to-surface-surface-help = Powierzchnia wyznaczająca tę granicę. Wybierz ją tutaj lub wskaż w widoku.
tri-clip-to-surface-ground-help = Powierzchnia, od której głębokość mierzy się w dół, zwykle teren. Wybierz ją tutaj lub wskaż w widoku.
tri-clip-to-surface-level = Rzędna (m)
tri-clip-to-surface-level-help = Poziom w metrach. Granica jest wszędzie płaska na tej wysokości.
tri-clip-to-surface-level-invalid = Rzędna musi być liczbą metrów
tri-clip-to-surface-depth = Głębokość (m)
tri-clip-to-surface-depth-help = Metry pod powierzchnią powyżej. Różni się dla każdego złoża i jest zapisywana z projektem.
tri-clip-to-surface-note = Najpierw stosowane jest Zachowaj poniżej, potem Zachowaj powyżej. Strop i spąg kończą się tam, gdzie spotykają się na granicy, a między nimi powstaje zamknięta bryła.
tri-closed-pit-stockpile-solid-whose = Zamknięta bryła wyrobiska lub zwałowiska, której odsłonięta granica zostanie uwzględniona w wyniku.
tri-cloud-carries-no-classifications-so = Ta chmura nie ma klasyfikacji, więc powierzchnia obejmuje każdy punkt. Zaimportuj plik LAS/LAZ po filtrze gruntu, aby odtworzyć nagą powierzchnię terenu.
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
tri-ground-points-only = Tylko punkty gruntu
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
tri-no-boundary-selected = Nie wybrano granicy
tri-no-point-cloud-selected = Nie wybrano chmury punktów
tri-no-surface-selected = Nie wybrano powierzchni
tri-once-clip-succeeds-unload-source = Po udanym przycięciu wyładuj powierzchnię źródłową, aby w scenie pozostał tylko wynik przycięcia.
tri-once-cut-succeeds-unload-original = Po udanym cięciu wyładuj oryginalną topologię, aby w scenie pozostał tylko wynik cięcia. Powłoka wyrobiska pozostaje wczytana.
tri-once-merge-succeeds-unload-source = Po udanym scaleniu wyładuj topologię źródłową i bryłę, aby w scenie pozostał tylko wynik scalenia.
tri-once-slice-succeeds-unload-source = Po udanym przekroju wyładuj powierzchnię źródłową, aby w scenie pozostał tylko wynik przekroju.
tri-once-trim-succeeds-unload-surface = Po udanym przycięciu wyładuj przycinaną powierzchnię, aby w scenie pozostał tylko wynik. Topologia pozostaje wczytana.
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
tri-reconstruct-ground-only-help = Odtwarza powierzchnię z punktów sklasyfikowanych jako naga ziemia, odrzucając roślinność, budynki, instalacje i szum. Wyłącz, aby uwzględnić każdy punkt chmury.
tri-reconstruct-help = Zrekonstruuj triangulowaną powierzchnię terenu z chmury punktów. Adaptacyjny próbnik wydaje budżet wierzchołków tam, gdzie teren jest najbardziej złożony, a obszary płaskie pozostawia rzadkie.
tri-reduce-budget-candidate-detail-if = Zmniejsz budżet lub szczegółowość kandydatów, jeśli Twój komputer ma mniej pamięci RAM.
tri-reference-topology-help = Topologia odniesienia określająca, gdzie przycinana jest druga powierzchnia.
tri-reject-reconstructed-triangle-edges = Odrzucaj zrekonstruowane krawędzie trójkątów dłuższe niż ta odległość. Użyj 0, aby nie ograniczać długości krawędzi.
tri-remove-inside-help = Usuwa powierzchnię wewnątrz granicy polilinii, zachowując resztę.
tri-removes-topology-where-pit-shell = Usuwa topologię tam, gdzie powłoka wyrobiska schodzi poniżej niej, tak aby powłoka wypełniła otwór. Szew podąża za rzeczywistą trójwymiarową linią styku obu powierzchni; topologia pod fragmentami powłoki wystającymi ponad teren jest zachowywana.
tri-result = Wynik
tri-save-two-entities = Zapisz jako dwa elementy
tri-select = Wybierz…
tri-selected-closed-polyline-whose-xy = Wybrana zamknięta polilinia, której granica XY określa obszar przycięcia.
tri-selected-point-cloud-whose-points = Wybrana chmura punktów, której punkty zostaną odtworzone w powierzchnię terenu. Zamknij okno, aby odtworzyć inną.
tri-selected-surface-from-which-contour = Wybrana powierzchnia, z której zostaną wygenerowane poziomice. Zamknij okno, aby wygenerować poziomice dla innej.
tri-selected-surface-which-will-clipped = Wybrana powierzchnia, która zostanie przycięta. Zamknij okno, aby przyciąć inną.
tri-slice-source-help = Wybrana powierzchnia, której zakres wysokości zostanie przycięty. Zamknij okno, aby przyciąć inną.
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
tri-unload-source-surface = Wyładuj powierzchnię źródłową
tri-unload-source-topology = Wyładuj topologię źródłową
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
viewport-1-1-true-shape = 1:1, rzeczywisty kształt
viewport-1-ratio = 1:{ $ratio }

## Viewport strings

viewport-all-total-categories-keep-their = Wszystkie { $total } kategorie zachowują swój kolor; wyraźnie rysowane są tylko pierwsze { $shown }
viewport-axis-maximum = Maksimum { $axis }
viewport-axis-minimum = Minimum { $axis }
viewport-azimuth-dip = Azymut { $azimuth }, upad { $dip }
viewport-back-whole-log = Wróć do całego profilu.
viewport-bar-blast-timeline-placeholder = Oś czasu strzelania [ZASTĘPCZE]
viewport-bar-burden-relief-heatmap-placeholder = Mapa cieplna odprężenia calizny [ZASTĘPCZE]
viewport-bar-cinematic-view = Widok filmowy
viewport-bar-color = Kolor:
viewport-bar-contours-equal-time-placeholder = Izochrony [ZASTĘPCZE]
viewport-bar-disable-cinematic-view = Wyłącz widok filmowy
viewport-bar-disable-flying-mode = Wyłącz tryb lotu
viewport-bar-disable-x-ray-vision = Wyłącz widzenie rentgenowskie
viewport-bar-drill-holes = Otwory wiertnicze:
viewport-bar-enable-flying-mode = Włącz tryb lotu
viewport-bar-enable-x-ray-vision = Włącz widzenie rentgenowskie
viewport-bar-unhide-all = Odkryj wszystko: pokaż ukryte obiekty we wczytanych warstwach
viewport-bar-exit-slice-view = Wyjdź z widoku przekroju
viewport-bar-fill = Wypełnienie:
viewport-bar-fix-centre-rotation = Ustaw środek obrotu
viewport-bar-hide-borehole-inspector = Ukryj inspektor otworów wiertniczych
viewport-bar-hide-classification = Ukryj klasyfikację
viewport-bar-hide-points = Ukryj punkty
viewport-bar-hide-rl-grid = Ukryj siatkę rzędnych
viewport-bar-hide-wireframes = Ukryj siatki krawędziowe
viewport-bar-hide-xy-grid = Ukryj siatkę XY
viewport-bar-release-centre-rotation = Zwolnij środek obrotu
viewport-bar-reset-view-plan-over-centre = Resetuj widok: rzut z góry nad środkiem obrotu, kliknij ponownie, aby dopasować wszystko
viewport-bar-reset-view-plan-same-distance = Resetuj widok: rzut z góry z tej samej odległości, kliknij ponownie, aby dopasować wszystko
viewport-bar-show-borehole-inspector = Pokaż inspektor otworów wiertniczych
viewport-bar-show-classification = Pokaż klasyfikację
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
viewport-depth-m-hole-end = { $depth } m — koniec otworu
viewport-double-click-add-boundary-here = Kliknij dwukrotnie, aby dodać tutaj granicę
viewport-drag-move-middle-click-toggles = Przeciągnij, aby przesunąć · Środkowy przycisk przełącza ≤
viewport-drag-move-right-click-remove = Przeciągnij, aby przesunąć · Kliknij prawym przyciskiem, aby usunąć · Środkowy przycisk przełącza ≤
viewport-drag-spin-view-around-hole = Przeciągnij, aby obracać widok wokół otworu. Kliknij dwukrotnie, aby zwrócić się na północ.
viewport-e = E
viewport-edit-category-colour = Edytuj kolor tej kategorii
viewport-edit-colour-used-empty-values = Edytuj kolor używany dla pustych wartości
viewport-empty = (puste)
viewport-empty-hidden = (puste · ukryte)
viewport-field-has-no-strat-column = To pole nie ma jeszcze kolumny stratygraficznej; utwórz ją na karcie Kolumna inspektora
viewport-filter-variables = Filtruj zmienne
viewport-fit-hole-track = Dopasuj otwór do toru
viewport-from = od { $from } do { $to }
viewport-from-m = od { $from } do { $to } m
viewport-h-1-ratio = H 1:{ $ratio }
viewport-hole-has-no-trace-draw = Ten otwór nie ma toru do narysowania.
viewport-interval-data = Dane interwałowe
viewport-intervals = Interwały
viewport-m-from-collar-toward-bearing = m od wylotu, w kierunku { $bearing }°
viewport-navigation-hint = Przeciągnij środkowym przyciskiem, aby przesunąć widok · Przewiń, aby przybliżyć
viewport-navigation-hint-detach = Przeciągnij środkowym przyciskiem, aby przesunąć widok · Przewiń, aby przybliżyć · Kliknij, aby odłączyć
viewport-move-all-down = Wszystkie w dół
viewport-move-all-up = Wszystkie w górę
viewport-move-down-from-here = W dół stąd
viewport-move-up-from-here = W górę stąd
viewport-n = N
viewport-name-not-in-strat-column = Tej nazwy nie ma w kolumnie stratygraficznej pola, więc nie ma horyzontu, od którego można przesuwać
viewport-no-data-variable = Brak danych dla tej zmiennej
viewport-no-density-log-hole = Brak profilu gęstości dla tego otworu
viewport-no-downhole-geophysics-hole = Brak geofizyki otworowej dla tego otworu
viewport-no-gamma-log-hole = Brak profilu gamma dla tego otworu
viewport-no-matches = Brak dopasowań
viewport-no-trace = Brak toru
viewport-no-usable-range = (brak użytecznego zakresu)
viewport-not-logged = Nie opisano
viewport-orientation-source = Źródło orientacji
viewport-rebuild-variable-s-colours-from = Przebuduj kolory tej zmiennej na podstawie jej danych
viewport-rename-seam-in-every-hole = Zmień nazwę w każdym otworze
viewport-rename-seam-in-this-hole = Zmień nazwę w tym otworze
viewport-reset = Resetuj
viewport-restore-full-model-range = Przywróć pełny zakres modelu
viewport-roll-wheel-over-log-zoom = Obróć kółko nad profilem, aby przybliżyć pokład. Przeciągnij profil, aby obracać otwór i przesuwać się wzdłuż niego.
viewport-s = S
viewport-sideways-scale = Skala boczna
viewport-squeeze-sideways-just-enough-keep = Zwęża na boki tylko tyle, aby otwór pozostał w widoku. Nigdy nie rozciąga.
viewport-trace-extent = Zasięg toru
viewport-w = W
viewport-widen-panel-show-density = Poszerz panel, aby pokazać gęstość
viewport-widen-panel-show-density-gamma = Poszerz panel, aby pokazać gęstość i gamma
viewport-widen-panel-show-gamma = Poszerz panel, aby pokazać gamma
charging-edit-charge-product = Edytuj produkt ładunkowy
charging-new-charge-product = Nowy produkt ładunkowy
charging-explosive-decks-add-mass-primed-stemming = Odcinki z materiałem wybuchowym dodają masę i mają inicjator; przybitka i odcinki powietrzne mają tylko długość.
charging-density = Gęstość
charging-density-hint = Gęstość w otworze. Masa na metr to ta wartość razy przekrój poprzeczny otworu.
charging-another-product-already-has-name = Inny produkt ma już tę nazwę
charging-edit-charge-rule = Edytuj regułę ładowania
charging-new-charge-rule = Nowa reguła ładowania
charging-decks-collar-toe = Odcinki, od wylotu do dna
charging-priming = Inicjowanie
charging-preview = Podgląd
charging-preview-use-pattern-hole = Użyj otworu medianowego siatki
charging-preview-active-pattern-median-hole = Podgląd na otworze o medianowej głębokości aktywnej siatki
charging-fixed-decks-longer-than-hole = Odcinki o stałej długości są dłuższe niż ten otwór
charging-mass-kg-explosive = { $mass } kg materiału wybuchowego
charging-rate-kg-m = { $rate } kg/m
charging-count-primer =Inicjatory: { $count }
charging-another-rule-already-has-name = Inna reguła ma już tę nazwę
charging-save-reload-count-hole = Zapisz i przeładuj otwory: { $count }
charging-length = Długość
charging-rest-length-m = reszta · { $length } m
charging-rest = reszta
charging-deck-takes-whatever-length-fixed-decks = Ten odcinek przyjmuje całą długość, jaką pozostawiają odcinki o stałej długości. Jeden odcinek na regułę wypełnia resztę.
charging-remove-deck = Usuń odcinek
charging-add-deck = Dodaj odcinek
charging-downhole-delay = Opóźnienie w otworze
charging-hole-detonator-hole-fires-long-after = Detonator w otworze. Otwór odpala się po takim czasie od nadejścia sygnału powierzchniowego.
charging-primer-height = Wysokość inicjatora
charging-how-far-above-base-each-explosive = Jak wysoko nad spągiem każdego odcinka z materiałem wybuchowym znajduje się jego inicjator.
charging-booster = Wzmacniacz
charging-cast-booster-mass-each-primer = Masa wzmacniacza odlewanego w każdym inicjatorze.
charging-count-rule-load-product-will-need = Reguły ładujące ten produkt: { $count }; będą wymagały wyboru innego.
charging-rule = Reguła
charging-holes-already-loaded-keep-their-charge = Otwory już załadowane tym produktem zachowują swój ładunek.
blast-burden-relief = Odprężenie calizny
blast-ms-per-metre-last-neighbour-fire = ms na metr do ostatniego odpalającego sąsiada
blast-below-hole-fires-before-rock-front = Poniżej tej wartości otwór odpala się, zanim skała przed nim się poruszy: ciasno.
blast-above-rock-front-has-long-gone = Powyżej tej wartości skała przed otworem dawno odeszła: luźno, z ryzykiem odcięcia i rozrzutu skał.
blast-tight = ciasno
blast-good = dobrze
blast-slack = luźno
blast-free-face = wolne czoło
blast-fires-at = Odpala o
blast-empty-won-t-detonate = pusty, nie zdetonuje
blast-not-reached = nieosiągnięty
blast-value-ms-m-from-hole = { $value } ms/m od { $hole }
blast-fires-first-free-face = odpala pierwszy: wolne czoło
blast-relief = Odprężenie
blast-explosive = Materiał wybuchowy
blast-powder-factor = Zużycie jednostkowe
blast-not-loaded = Nie załadowany
blast-count-primer-delay-ms-downhole = Inicjatory: { $count } · { $delay } ms w otworze
blast-set-initiation-point-tie-holes-play = Ustaw punkt inicjacji i połącz otwory, aby odtworzyć strzelanie
blast-pause = Wstrzymaj
blast-play = Odtwórz
blast-back-start = Wróć na początek
blast-duration-ms = z { $duration } ms
blast-real-time = Czas rzeczywisty
blast-mic-limit = Limit MIC
blast-most-explosive-allowed-detonate-any-8 = Największa masa materiału wybuchowego dozwolona do detonacji w dowolnym oknie 8 ms na tym stanowisku. Okna powyżej limitu są oznaczane.
blast-no-holes-loaded-surface-signal-plays = Brak załadowanych otworów: sygnał powierzchniowy jest odtwarzany, ale nic nie detonuje. Załaduj otwory narzędziem Ładuj otwory.
blast-now-holes-hole = Teraz: otwory: { $holes }
blast-in-8-ms = w 8 ms
blast-peak-mass-kg-time-ms = Szczyt { $mass } kg o { $time } ms
blast-peak-holes-hole-time-ms = Szczyt: otwory: { $holes } o { $time } ms
blast-peak-over-limit = , { $over } kg ponad limit
blast-peak-within-limit = , w granicach limitu
blast-top-surface-signal-lighting-each-downline = Góra: sygnał powierzchniowy docierający do kolejnych łączników. Dół: detonacje. Kliknij lub przeciągnij, aby przesunąć znacznik odtwarzania.
products-charge-rules = Reguły ładowania
products-new-rule = Nowa reguła
products-charge-products = Produkty ładunkowe
products-new-rule-default-name = Nowa reguła
products-no-rules = Brak reguł
products-load-selected-holes-count = Załaduj wybrane otwory ({ $count })
products-unload-selected-holes-count = Rozładuj wybrane otwory ({ $count })
products-edit-rule = Edytuj regułę
products-duplicate-rule = Duplikuj regułę
products-delete-rule = Usuń regułę
products-fill-product = wypełnienie  { $product }
products-primer-offset-m-off-each-explosive = Inicjator { $offset } m nad spągiem każdego odcinka z materiałem wybuchowym, wzmacniacz { $booster } kg, { $delay } ms w otworze
products-double-click-edit = Kliknij dwukrotnie, aby edytować
products-edit-product = Edytuj produkt
blast-log-updated-charge-product-name = Zaktualizowano produkt ładunkowy { $name }
blast-log-added-charge-product-name = Dodano produkt ładunkowy { $name }
blast-log-updated-charge-rule-name = Zaktualizowano regułę ładowania { $name }
blast-log-added-charge-rule-name = Dodano regułę ładowania { $name }
blast-log-entry-no-longer-charge-library = Tego wpisu nie ma już w bibliotece ładunków
blast-log-deleted-name-from-charge-library = Usunięto { $name } z biblioteki ładunków
blast-log-failed-save-charge-library-error = Nie udało się zapisać biblioteki ładunków: { $error }
blast-log-cannot-load-rule-problem = Nie można załadować tą regułą: { $problem }
blast-log-count-hole-too-short-fixed-decks = Otwory zbyt krótkie na odcinki o stałej długości tej reguły, pozostawione bez zmian: { $count }
blast-log-count-hole-have-no-depth-load = Otwory bez głębokości do załadowania: { $count }
blast-log-count-loaded-hole-have-no-diameter = Załadowane otwory bez średnicy, więc masa materiału wybuchowego jest nieznana: { $count }
common-charge-holes = Ładuj otwory
blast-log-loaded-count-hole-rule = Załadowano otwory: { $count } regułą { $rule }
blast-log-unload-holes = Rozładuj otwory
blast-log-unloaded-count-hole = Rozładowano otwory: { $count }
blast-log-select-holes-active-pattern-first = Najpierw wybierz otwory aktywnej siatki
blast-log-rule-no-longer-charge-library = Tej reguły nie ma już w bibliotece ładunków
blast-log-there-no-charge-rule-load-add = Brak reguły ładowania do użycia: dodaj ją w panelu produktów
blast-rule-stemming = Przybitka
blast-rule-air-deck = Odcinek powietrzny
blast-rule-give-rule-name = Nadaj regule nazwę
blast-rule-add-least-one-deck = Dodaj co najmniej jeden odcinek
blast-rule-only-one-deck-can-fill-rest = Tylko jeden odcinek może wypełnić resztę otworu
blast-rule-deck-lengths-must-greater-than-zero = Długości odcinków muszą być większe od zera
blast-rule-no-product-named-name = Brak produktu o nazwie „{ $name }”
blast-rule-rule-needs-least-one-explosive-deck = Reguła wymaga co najmniej jednego odcinka z materiałem wybuchowym
state-save-charge-product = Zapisz produkt ładunkowy
state-save-charge-rule = Zapisz regułę ładowania
state-delete-charge-library-entry = Usuń wpis biblioteki ładunków
ui-click-drag-over-holes-load-them = Kliknij lub przeciągnij nad otworami, aby załadować je regułą { $rule }
ui-hold-shift-unload = przytrzymaj Shift, aby rozładować
ui-no-charge-rule-load = Brak reguły ładowania do użycia
ui-right-click-charge-rules-heading-add = kliknij prawym przyciskiem nagłówek Reguły ładowania, aby dodać regułę
omf-element-name-has-count-charge-naming = Element „{ $name }” ma ładunki: { $count } wskazujące otwory, których już nie zawiera
