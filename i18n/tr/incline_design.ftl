# Incline — Türkçe mesaj kataloğu.
#
# Bu dosya eksik olabilir; eksik girdilerde İngilizce kaynak katalog
# (`i18n/en/incline_design.ftl`) yedek olarak kullanılır.
#
# `=` işaretinin solundaki kimlikleri ve `{ $... }` biçimindeki değişken
# adlarını değiştirmeyin — yalnızca sağdaki metni çevirin.

## Ortak

common-cancel = İptal
common-clear = Temizle
common-close = Kapat
common-fill = Dolgu
common-set = Ayarla

## Durum çubuğu

# Durum çubuğundaki dil menüsünün başlığı. Dillerin kendileri asla
# çevrilmez: her biri `LanguageChoice` içinde kendi alfabesiyle adlandırılır.
status-language = Dil

## Menü çubuğu — Dosya

menu-file = Dosya
menu-file-save-project = Projeyi Kaydet
menu-file-save-project-as = Projeyi Farklı Kaydet...
menu-file-new-project = Yeni Proje...
menu-file-open-project = Proje Aç...
menu-file-open-recent = Son Kullanılanları Aç
menu-file-show-in-explorer = Dosya Gezgininde Göster
menu-file-show-in-folder = İçeren Klasörü Aç
menu-file-import = İçe Aktar...
menu-file-export = Dışa Aktar...
menu-file-export-viewport-image = Görüntü Alanı Resmini Dışa Aktar...
menu-file-export-engineering-drawing = Mühendislik Çizimini Dışa Aktar...
menu-file-about = { $app } Hakkında...
menu-file-exit = Uygulamadan Çık

## Menü çubuğu — Görünüm

menu-view = Görünüm

## Çalışma alanları

ws-production = Üretim
ws-drill-and-blast = Delme & Patlatma
ws-geology = Jeoloji
ws-planning = Planlama

## Menü çubukları

ws-menubar-design = Tasarım
ws-menubar-triangulation = Üçgenleme
ws-menubar-raster = Raster
ws-menubar-point-cloud = Nokta Bulutu
ws-menubar-block-model = Blok Model
ws-menubar-drillholes = Sondaj Delikleri
ws-menubar-active-layer = Katman:

## Menü çubuğu işlevleri

ws-menubar-design-insert-point = Nokta Ekle
ws-menubar-design-insert-point-at-intersection = Kesişimde
ws-menubar-design-insert-point-at-elevation = Kotta
ws-menubar-design-move-to = Taşı
ws-menubar-design-create-triangulation = Üçgenleme Oluştur

## Yeniden adlandırma / silme iletişim kutuları

# { $kind }, yukarıdaki ws-production-* kümesinden bir çalışma alanı adıdır.
dialog-rename-title = { $kind } Yeniden Adlandır
dialog-rename-field = Yeni ad
dialog-rename-field-hint = Zorunlu
dialog-rename-submit = Yeniden Adlandır
dialog-delete-title = { $kind } Sil
dialog-delete-confirm =
    '{ $name }' projeden silinsin mi?
    Bu işlem geri alınamaz.
confirm-delete-product =
    '{ $name }' ürünü paletten silinsin mi?
    Bu işlem geri alınamaz.

## Üçgenleme Oluştur iletişim kutusu

tri-create-title = Üçgenleme Oluştur
tri-create-type-label = Üçgenleme türü
tri-create-type-help =
    Açık yüzey, arazi tarzı bir levha oluşturur. Katı, tam kapalı bir ağ
    oluşturur ve su geçirmez bir sınır oluşturabilecek girdi gerektirir.
tri-create-output-name = Çıktı adı
tri-create-output-name-help = Oluşturulan üçgenlemeye atanacak ad.
tri-create-output-name-hint = üçgenleme adı
tri-create-run = Üçgenle

tri-selection-selected = { $summary } seçildi

tri-type-open-surface = Yüzey
tri-type-solid-closed = Katı

# Seçim özeti parçaları, örn. "3 çizgi, 1 nokta". Her ad kendi sayısına göre
# çoğullaştırılır; böylece ikiden fazla çoğul biçimi olan diller de doğru okunur.
tri-count-polylines =
    { $count ->
        [one] { $count } çoklu çizgi
       *[other] { $count } çoklu çizgi
    }
tri-count-strings =
    { $count ->
        [one] { $count } dizgi
       *[other] { $count } dizgi
    }
tri-count-points =
    { $count ->
        [one] { $count } nokta
       *[other] { $count } nokta
    }
tri-count-texts =
    { $count ->
        [one] { $count } metin nesnesi
       *[other] { $count } metin nesnesi
    }
tri-count-objects =
    { $count ->
        [one] { $count } nesne
       *[other] { $count } nesne
    }

about-read-full-licence = Lisansın tamamını okuyun ↗
about-source-code = Kaynak Kod
about-website = Web Sitesi
about-title = { $app } Hakkında
drill-hole-colour-stop = Durak { $index }
properties-restore-defaults-tooltip = { $heading } ayarlarını varsayılanlarına sıfırla

## Dinamik arayüz mesajları

ui-selected-count = { $count } seçildi
ui-selected-objects = { $count } nesne seçildi
ui-selected-polylines = { $count } çoklu çizgi seçildi
ui-invalid-axis-value = Geçerli bir { $axis } değeri girin.
ui-selection-spans = Seçim { $min } ile { $max } arasında.
confirm-delete-count = Seçili { $count } öğeyi silmek istediğinizden emin misiniz?
confirm-delete-layer = '{ $name }' katmanı ve üzerindeki tüm nesneler silinsin mi?
    Bu işlem geri alınamaz.
plot-preview-pixels = { $dpi } dpi'de { $width } × { $height } piksel
tri-estimated-memory = Tahmini tepe bellek kullanımı ~{ $estimate }. { $detail }
block-grid-summary = Izgara: { $x } × { $y } × { $z } = { $count } blok
status-selected = Seçili: { $count }
status-clip = Kırpma yakın/uzak/Δ: { $near } / { $far } / { $delta } m

## Kaynak sabit metinler

explorer-no-rasters = Raster yok
slice-viewport-gestures = orta tuşla sürükle: kaydır · sağ tuşla sürükle: döndür · Shift+tekerlek: yürü · W/S: dilimi taşı · Q/E: döndür · Esc: çık

## Startup environment details

## Renderer startup diagnostics

color-aci = ACI
color-aci-value = ACI { $index }
color-index = İndeks
color-rgb = RGB
color-opacity = Saydamlık
color-edit = Rengi düzenlemek için tıklayın
color-saturation-value = Doygunluk ve parlaklık
color-hue = Ton
asset-loading = Varlık verisi yükleniyor
asset-unloading = Varlık verisi kaldırılıyor
asset-load-failed = Varlık verisi yüklenemedi
asset-unload-failed = Varlık verisi kaldırılamadı
preferences-title = Tercihler
context-text-colour = Metin rengi
context-polylines = Çoklu çizgiler
context-points = Noktalar
crs-unknown-ellipsoid = Bu koordinat sistemi tanımında tanınmayan yeryüzü modeli "{ $name }".
crs-no-ellipsoid = Bu koordinat sistemi tanımı hangi yeryüzü modelini kullandığını belirtmiyor.
crs-unknown-code = EPSG:{ $code } koordinat sistemi kayıt defterinde yok.
crs-transform-failed = Bir koordinat dönüştürülemedi; sonuç sonlu bir konum değildi.
crs-no-datum-path = { $from } ve { $to } referans çerçeveleri arasında (EPSG datumları { $source } ve { $target }) yayımlanmış bir dönüşüm yok. Yine de dönüştürmek bilinmeyen bir miktarda hatalı olurdu, bu yüzden hiçbir şey değiştirilmedi.
crs-unknown-datum = { $from } veya { $to } referans çerçevesi tanımlanamıyor ve ikisi farklı yeryüzü modelleri kullanıyor. Aralarında dönüştürme bilinmeyen bir miktarda hatalı olurdu.
ws-survey = Ölçüm
survey-count-designs = { $count } { $count ->
    [one] tasarım
   *[other] tasarım
  }
survey-count-meshes = { $count } { $count ->
    [one] üçgenleme
   *[other] üçgenleme
  }
survey-count-models = { $count } { $count ->
    [one] blok model
   *[other] blok model
  }
survey-count-clouds = { $count } { $count ->
    [one] nokta bulutu
   *[other] nokta bulutu
  }
survey-count-holes = { $count } { $count ->
    [one] sondaj veri kümesi
   *[other] sondaj veri kümesi
  }
survey-count-rasters = { $count } { $count ->
    [one] raster
   *[other] raster
  }
survey-angle = Z etrafında döndürme (saat yönünün tersine)
survey-scale = Tekdüze XYZ ölçek faktörü
survey-invalid-transform = Başlangıç noktaları, açı ve sonuçtaki koordinatlar sonlu olmalıdır.
survey-invalid-scale = Ölçek, sonlu tersi olan sonlu bir pozitif sayı olmalıdır.
survey-empty-selection = Dönüştürmek için en az bir desteklenen öge seçin.
survey-unavailable = Seçilen bir öge eksik veya yüklenmemiş. Dönüştürmeden önce yükleyin.
survey-wrong-project = Yalnızca etkin projeden tasarım seçin.
survey-name-required = Bir koordinat sistemi adı girin.
survey-working = Seçilen veriler dönüştürülüyor…
survey-completed = { $items } yerinde dönüştürüldü. Geri alma bunları eski haline getirir.
survey-failed = Dönüşüm başarısız oldu: { $error }
survey-stale = Etkin proje veya kaynak veriler değiştiği için dönüşüm iptal edildi. Kaynak verileri seçip yeniden deneyin.
survey-coordinates-menu = Koordinatlar
survey-definitions-action = Tanımlar…
survey-transform-action = Dönüştür…
survey-definitions-title = Koordinat Tanımları
survey-transform-title = Koordinatları Dönüştür
survey-new-system = Yeni Koordinat Sistemi
survey-new-system-name = Koordinat sistemi
survey-set-local = Maden Koordinat Sistemi Olarak Ayarla
survey-delete-system = Koordinat Sistemini Sil
survey-systems-empty = Koordinat sistemi yok
survey-system-name = Ad
survey-system-origin = Aynı nokta — sistem koordinatları
survey-angle-help = Yukarıdan bakıldığında referans X'ten referans Y'ye doğru saat yönünün tersine.
survey-scale-help = Referans çerçeveden bu sisteme tekdüze XYZ ölçeği. Boyutları korumak için 1 kullanın.
survey-close = Kapat
survey-from = Kimden
survey-to = Kime
survey-transform-button = Dönüştür
survey-swap = Değiştir
survey-drape-note = Örtülü görüntüler dönüştürülen yüzeylerden kaldırılır ve yeniden örtülmelidir.
survey-needs-grid-block-model = Blok model, düzenli bir hücre ızgarasıdır ve projeksiyon veya referans çerçevesi değişikliği bu düzenliliği korumaz. Dönüştürmek, her hücreyi yeni bir ızgaraya yeniden örneklemek ve taşıdığı değerleri kaybetmek anlamına gelir, bu yüzden değiştirilmeden bırakıldı.
survey-needs-grid-raster = Bir raster, dünyaya afin bir eşleme ile yerleştirilir ve bunu projeksiyon veya referans çerçevesi değişikliği koruyamaz. Dönüştürmek, görüntüyü yeniden örneklemek anlamına gelir, bu yüzden değiştirilmeden bırakıldı.
survey-conversion-exact = Kesin: yalnızca ızgara değişikliği, yeniden projeksiyon yok.
survey-conversion-accuracy = Belirtilen doğruluk { $accuracy } m.
survey-kind = Tür
survey-axis-names = Eksen adları
survey-kind-registry-short = Kayıt defteri sistemi
survey-kind-grid-short = Başka bir sistem üzerindeki ızgara
survey-registry-search = Ara
survey-registry-hint = Ad veya EPSG kodu, örn. "mga zone 56"
survey-registry-none = Kayıt defterinde tüm kelimelerle eşleşen bir şey yok.
survey-parent = Şuna göre tanımlı
survey-parent-origin = Bilinen nokta — üst sistem koordinatları
survey-pick-registry = Sistemi arayın ve sonuçlardan seçin.
survey-pick-parent = Bu ızgaranın tanımlandığı sistemi seçin.
survey-pick-system = Bir sistem seçin
survey-pick-systems = Dönüştürülecek kaynak ve hedef sistemi seçin.
survey-no-selection = Solda bir koordinat sistemi seçin, ya da eklemek için sağ tıklayın.
survey-kind-grid = { $parent } üzerinde ızgara
survey-system-in-use = "{ $name }" silinemez: buna göre { $dependants } { $dependants ->
    [one] sistem
   *[other] sistem
  } tanımlı. Önce bunları başka bir yere yönlendirin.
survey-system-cycle = "{ $name }", doğrudan ya da üst sistemleri aracılığıyla kendisine göre tanımlanmış.
survey-system-missing = Bu koordinat sistemi artık yok. Başka bir tanım seçin.
survey-same-system = Farklı kaynak ve hedef sistemler seçin.
survey-name-exists = Bu adla bir koordinat sistemi zaten var. Düzenlemek için onu seçin ya da başka bir ad seçin.

## About strings

about-copyright-c-2026-leo-timmins =
    Telif Hakkı (c) 2026 Leo Timmins, Lucas Timmins ve Incline Design katkıda bulunanları. Bu yazılımın bir kopyasını edinen herhangi bir kişiye, MIT Lisansı koşullarına tabi olarak, üzerinde kısıtlama olmaksızın işlem yapma izni işbu belgeyle ücretsiz olarak verilir.

    Incline Design, "OLDUĞU GİBİ", TİCARİ ELVERİŞLİLİK, BELİRLİ BİR AMACA UYGUNLUK ve İHLAL ETMEME garantileri dahil ancak bunlarla sınırlı olmamak üzere AÇIK VEYA ZIMNİ HİÇBİR TÜR GARANTİ OLMAKSIZIN sağlanmaktadır.
about-free-open-source-mine-design = Ücretsiz Açık Kaynak Maden Tasarımı
about-licensed-under-mit-license = MIT Lisansı kapsamında lisanslanmıştır

## App strings

app-activated-browser-project-name = '{ $name }' tarayıcı projesi etkinleştirildi.
app-browser-project-delete-failed = Tarayıcı projesi silme işlemi başarısız oldu: { $error }
app-browser-project-no-longer-exists = Bu tarayıcı projesi artık mevcut değil
app-browser-save-failed-error = Tarayıcı kaydetme işlemi başarısız oldu: { $error }
app-could-not-activate-browser-project = Tarayıcı projesi etkinleştirilemedi: { $error }
app-could-not-delete-browser-project = Tarayıcı projesi silinemedi: { $error }
app-could-not-load-browser-project = Tarayıcı projesi yüklenemedi: { $error }
app-could-not-restore-browser-project = Tarayıcı projesi geri yüklenemedi: { $error }
app-deleted-browser-project = Tarayıcı projesi silindi
app-failed-create-window-error = Pencere oluşturulamadı: { $error }
app-failed-create-window-icon-error = Pencere simgesi oluşturulamadı: { $error }
app-failed-detach-top-down-preview = Üstten görünüm önizlemesi ayrılamadı: { $error }
app-failed-initialize-graphics-error = Grafikler başlatılamadı: { $error }
app-browser-preferences-load-failed = Tarayıcı tercihleri yüklenemedi: { $error }
app-failed-load-config-file-error = Yapılandırma dosyası yüklenemedi: { $error }
app-failed-load-session-file-error = Oturum dosyası yüklenemedi: { $error }
app-failed-rasterize-window-icon-error = Pencere simgesi rasterleştirilemedi: { $error }
app-failed-save-browser-session-error = Tarayıcı oturumu kaydedilemedi: { $error }
app-failed-save-session-error = Oturum kaydedilemedi: { $error }
app-saved-name-browser-storage = '{ $name }' tarayıcı deposuna kaydedildi

## Block strings

block-model-between = Arasında
block-model-block-grid = Blok ızgarası
block-model-block-size = Blok boyutu
block-model-choose-numeric-variable = Sayısal bir değişken seçin
block-model-choose-numeric-variables = Sayısal değişkenleri seçin
block-model-count-variables-selected = { $count } değişken seçildi
block-model-estimate-variables = Tahmin değişkenleri
block-model-full-x-y-z-dimensions = Her bloğun tam X, Y ve Z boyutları. Daha küçük bloklar detayı, hesaplama süresini ve bellek kullanımını artırır.
block-model-grid-bounds-block-sizes-invalid = Izgara sınırları veya blok boyutları geçersiz.
block-model-lower-x-y-z-edges = Blok model hacminin alt X, Y ve Z sınırları. Blok merkezleri bu sınırların yarım blok içeriden başlar.
block-model-maximum = Maksimum
block-model-maximum-nearest-samples-used-each = Her blok için kullanılan maksimum en yakın örnek sayısı. Düşük değerler daha hızlı çalışır; yüksek değerler tahminleri yumuşatabilir ve hesaplama süresini artırabilir.
block-model-maximum-samples = Maksimum örnek
block-model-minimum = Minimum
block-model-min-samples-help = Bir bloğu tahmin etmek için gereken minimum yakın örnek sayısı. Arama yarıçapı içinde daha az örneği olan bloklar boş bırakılır.
block-model-minimum-samples = Minimum örnek
block-model-nugget = Nugget
block-model-numeric-interval-fields-interpolate = Enterpolasyon yapılacak sayısal aralık alanları. Seçilen her alan bir blok model değişkeni olur.
block-model-kriging-help = Sıradan Kriging, küresel bir varyogram kullanarak her blok merkezindeki sayısal sondaj deliği aralıklarını tahmin eder.
block-model-partial-sill = Kısmi sill
block-model-range-search-radius = Menzil / arama yarıçapı
block-model-range-help = Bu mesafeden daha uzak örnekler hariç tutulur; kovaryans bu menzilde sıfıra ulaşır.
block-model-select-all = Tümünü seç
block-model-sill-help = Küresel model tarafından katkıda bulunulan konumsal olarak ilişkili varyans. Nugget ile birlikte, sıfır mesafede kovaryansı belirler.
block-model-spherical-variogram-search = Küresel varyogram ve arama
block-model-threshold-at-most = <= eşik
block-model-threshold-at-least = >= eşik
block-model-threshold-min = Eşik / min
block-model-upper-x-y-z-extent = Kapsanacak üst X, Y ve Z kapsamı. Aralık, blok boyutunun tam katı olmadığında son blok bu kapsamı aşabilir.
block-model-variable = Değişken
block-model-variance-effectively-zero-separation = Ölçüm hatası veya örnekleme ölçeğinin altındaki değişimden kaynaklanan, etkin olarak sıfır ayrımdaki varyans. Nugget etkisi istenmediğinde sıfır kullanın.
block-model-volume-feedback-disconnected = Blok hacim kullanım geri bildirimi okumasının bağlantısı kesildi
block-model-volume-feedback-failed = Blok hacim kullanım geri bildirimi okuma işlemi başarısız oldu: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z

## Canvas strings

canvas-not-selectable-closed-polyline = Seçilemez | Kapalı bir çoklu çizgi seçin
canvas-polyline-summary = Çoklu çizgi | Katman: { $layer } | { $count } köşe
canvas-surface-name = Yüzey | { $name }
canvas-trimmed = Kırpıldı

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = { $object_id } nesnesinden şev-berm oluşturuldu
cmd-bezier-replaced-polyline-span-first-last = { $first }→{ $last } çoklu çizgi aralığı, { $count } örneklenmiş ara nokta ile değiştirildi
cmd-bezier-vertices-first-last = { $first } - { $last } köşeleri
cmd-block-model-block-model-loader-disconnected-path = { $path } için blok model yükleyici bağlantısı kesildi
cmd-block-model-block-model-path-has-count = { $path } blok modelinde okunamayacak, desteklenmeyen türde { $count } değişken var: { $names }
cmd-block-model-building-ore-mesh = Cevher ağı oluşturuluyor…
cmd-block-model-could-not-create-block-model = Blok model oluşturulamadı: { $error }
cmd-block-model-could-not-decode-block-model = Blok model renk değişkeni '{ $variable }' çözülemedi: { $error }
cmd-block-model-created-block-model-name-ordinary = '{ $name }' blok modeli Sıradan Kriging ile oluşturuldu
cmd-block-model-failed-load-block-model-error = Blok model yüklenemedi: { $error }
cmd-block-model-generated-ore-mesh-from-block = '{ $name }' blok modelinden cevher ağı oluşturuldu
cmd-block-model-imported-block-model-source-path = İçe aktarılan blok model kaynağı { $path }
cmd-block-model-loaded-block-model-name-blocks = '{ $name }' blok modeli yüklendi: { $blocks } blok ({ $renderable } çizilebilir), ızgara { $dimx }x{ $dimy }x{ $dimz }, { $variables } değişken
cmd-block-model-loading-name = { $name } yükleniyor
cmd-block-model-loading-name-ellipsis = { $name } yükleniyor…
cmd-chamfer-applied = { $corner } köşesi { $radius } yarıçapı ve { $segments } segment ile pahlandı
cmd-chamfer-radius = Yarıçap { $radius }
cmd-commands-clipped = Kırpıldı
cmd-commands-command-failed-error = Komut başarısız oldu: { $error }
cmd-commands-select-one-more-objects-before = { $axis } değerini ayarlamadan önce bir veya daha fazla nesne seçin
cmd-commands-sliced = Kesildi
cmd-contours-contour-generation-failed-error = Kontur oluşturma başarısız oldu: { $error }
cmd-contours-discarded-layer-exists = '{ $name }' için konturlar atıldı: '{ $layer_name }' katmanı artık mevcut
cmd-contours-discarded-project-closed = '{ $name }' için konturlar atıldı: proje kapatıldı
cmd-contours-discarded-layer-deleted = '{ $name }' için konturlar atıldı: seçili çıktı katmanı silindi
cmd-contours-generated = '{ $name }' üçgenlemesi için '{ $layer_name }' katmanında { $line_count } kontur çoklu çizgisi oluşturuldu
cmd-creation-assembled-boundary-rings = Parçalanmış açık dizgilerden { $assembled_count } kapalı sınır halkası birleştirildi
cmd-creation-created-triangulation-from-boundary = { $boundary_count } sınır halkası ve { $constraint_count } açık kısıttan üçgenleme oluşturuldu, yüzey türü { $surface_type }
cmd-creation-creating-triangulation = Üçgenleme oluşturuluyor…
cmd-creation-generate-upper-surface-ignored-count = Üst yüzeyi oluştur: { $count } daha alçak çakışan kırılma çizgisi segmenti yok sayıldı; kaynak nesneler değişmedi
cmd-creation-ignored-objects = Üçgenleme sırasında { $rejected } çoklu çizgi olmayan veya dejenere nesne yok sayıldı
cmd-creation-weld-retry-moved-coarse-welded = Kaynakla ve yeniden dene: { $coarse_welded } köşe ortak konumlara taşındı ({ $coarse_weld_tol } m'ye kadar); kaynak nesneler değişmedi
cmd-creation-welded-breakline-vertices = Tolerans dahilinde çakışan { $welded } kırılma çizgisi köşesi kaynaklandı
cmd-cuts-clipped-surface-name-polyline-mode = '{ $name }' yüzeyi çoklu çizgiyle kırpıldı ({ $mode })
cmd-cuts-clipping-surface-polyline = Yüzey çoklu çizgiyle kırpılıyor…
cmd-cuts-cut-topology-name-pit-shell = '{ $name }' topolojisi ocak kabuğuna kesildi
cmd-cuts-cut-triangulation-name-z-band = '{ $name }' üçgenlemesi Z bandına göre kesildi [{ $min }, { $max }]
cmd-cuts-cutting-topology-pit-shell = Topoloji ocak kabuğuyla kesiliyor…
cmd-cuts-cutting-triangulation-z = Üçgenleme Z'ye göre kesiliyor…
cmd-cuts-ignored-vertical-faces = XY alanı olmayan { $count } dikey veya dejenere referans topoloji yüzeyi yok sayıldı
cmd-cuts-site-skipped-constraint-from-x = { $site }: kısıt atlandı ({ $from_x }, { $from_y }) -> ({ $to_x }, { $to_y }) üçgenleyici bölemedi
cmd-cuts-skipped-degenerate-edges = { $site }: neredeyse dejenere { $skipped } kısıt kenarı atlandı; kesim sınırı bu bölgelerde kıl payı sapabilir
cmd-cuts-trimmed-surface = '{ $surface }' yüzeyi '{ $topology }' topolojisine kırpıldı ({ $mode })
cmd-cuts-trimming-surface-topology = Yüzey topolojiye kırpılıyor…
cmd-drape-draped-intersected-vertices-changed = { $intersected } köşe örtüldü; { $changed } tanesinin kotu değişti
cmd-drape-no-intersections = Seçili tasarım köşelerinin hiçbiri seçili topolojilerle kesişmiyor
cmd-drape-objects-changed-object-s-changed = { $objects } nesne değişti · kesişen { $intersected } köşeden { $changed } tanesi taşındı
cmd-drape-select-one-more-design-objects = Örtülecek bir veya daha fazla tasarım nesnesi seçin
cmd-drape-select-one-more-topologies-drape = Üzerine örtülecek bir veya daha fazla topoloji seçin
cmd-drape-selected-topologies-no-longer-loaded = Seçili topolojiler artık yüklü değil
cmd-drill-hole-drill-pattern-too-large-contains = Delme deseni çok büyük veya geçersiz ağız koordinatları içeriyor
cmd-drill-hole-enter-name-drill-pattern = Delme deseni için bir ad girin
cmd-drill-hole-failed-load-drillholes-error = Sondaj delikleri yüklenemedi: { $error }
cmd-drill-hole-depth-must-be-positive = Delik derinliği sıfırdan büyük olmalıdır
cmd-drill-hole-diameter-must-be-positive = Delik çapı sıfırdan büyük olmalıdır
cmd-drill-hole-loaded-drillhole-dataset-name-holes = '{ $name }' sondaj deliği veri kümesi yüklendi: { $holes } delik, { $fields } renk alanı
cmd-drill-hole-pattern-contains-no-holes = Desen hiçbir delik içermiyor
cmd-explode-count-line-s = { $count } çizgi
cmd-explode-polyline = Çoklu Çizgiyi Ayır
cmd-explode-exploded-polyline-into-count-line = Çoklu çizgi { $count } çizgi segmentine ayrıldı
cmd-file-block-model-csv-encoding-failed = Blok model CSV kodlaması başarısız oldu: { $error }
cmd-file-block-model-csv-export-failed = Blok model CSV dışa aktarımı başarısız oldu: { $error }
cmd-file-browser-recovery-unavailable = Tarayıcı kurtarma dosyaları kullanılamıyor; kaydedilen projeler IndexedDB'de kalır
cmd-file-closed-project-runtime-id-runtime = { $runtime_id } çalışma zamanı kimlikli proje kapatıldı
cmd-file-could-not-create-new-project = Yeni proje oluşturulamadı: { $error }
cmd-file-could-not-finish-pending-project = Bekleyen proje eylemi tamamlanamadı: { $error }
cmd-file-could-not-finish-saving-before = Çıkmadan önce kaydetme tamamlanamadı: { $error }
cmd-file-could-not-open-browser-project = Tarayıcı projesi açılamadı: { $error }
cmd-file-could-not-open-path-error = { $path } açılamadı: { $error }
cmd-file-could-not-read-selected-file = Seçili dosya okunamadı: { $error }
cmd-file-could-not-reload-layer-from = Katman diskten yeniden yüklenemedi: { $error }
cmd-file-could-not-reload-project-from = Proje diskten yeniden yüklenemedi: { $error }
cmd-file-could-not-remove-browser-project = Tarayıcı projesi kaldırılamadı: { $error }
cmd-file-could-not-restore-layer-from = Katman projeden geri yüklenemedi: { $error }
cmd-file-could-not-snapshot-dirty-project = Kurtarma için değiştirilmiş projenin anlık görüntüsü alınamadı: { $error }
cmd-file-could-not-start-browser-export = Tarayıcı dışa aktarımı başlatılamadı: { $error }
cmd-file-could-not-write-recovery-copies = Kurtarma kopyaları yazılamadı: { $error }
cmd-file-created-new-browser-project = Yeni tarayıcı projesi oluşturuldu
cmd-file-created-new-project = Yeni proje oluşturuldu
cmd-file-description-download-failed-error = { $description } indirme işlemi başarısız oldu: { $error }
cmd-file-discard-cancelled-project-changed = OMF yeniden yüklenirken proje değiştiği için atma işlemi iptal edildi
cmd-file-discarded-changes-layer-target-name = '{ $target_name }' katmanındaki değişiklikler atıldı
cmd-file-discarded-changes-reloaded-path = Değişiklikler atıldı: { $path } yeniden yüklendi
cmd-file-downloaded-description-file-name = { $description } indirildi: { $file_name }
cmd-file-dxf-download-encoding-failed-error = DXF indirme kodlaması başarısız oldu: { $error }
cmd-file-dxf-import-failed-error = DXF içe aktarma başarısız oldu: { $error }
cmd-file-encoding-block-model-csv-download = Blok model CSV indirmesi kodlanıyor…
cmd-file-encoding-dxf-download = DXF indirmesi kodlanıyor…
cmd-file-encoding-triangulation-download = Üçgenleme indirmesi kodlanıyor…
cmd-file-exit-deferred-exports = Arka plan dışa aktarmaları bitene kadar çıkış ertelendi
cmd-file-exit-requested-no-unsaved-changes = Kaydedilmemiş değişiklik olmadan çıkış istendi
cmd-file-exported-block-model-csv-path = Blok model CSV'si { $path } konumuna dışa aktarıldı
cmd-file-exported-description-dxf-path = { $description } DXF'e dışa aktarıldı: { $path }
cmd-file-exported-triangulation-name-path = '{ $name }' üçgenlemesi { $path } konumuna dışa aktarıldı
cmd-file-exporting-name = { $name } dışa aktarılıyor…
cmd-file-exporting-triangulation-name-path = '{ $name }' üçgenlemesi { $path } konumuna dışa aktarılıyor
cmd-file-fatal-renderer-failure-reason = Ölümcül işleyici hatası: { $reason }
cmd-file-dialog-action-failed = Dosya iletişim kutusu eylemi başarısız oldu: { $msg }
cmd-file-imported-added-object-s-from = { $name } konumundan { $added } nesne içe aktarıldı
cmd-file-imported-total-dxf-object-s = { $total } DXF nesnesi içe aktarıldı
cmd-file-layer-discard-was-cancelled-because = Proje yeniden yüklenirken proje değiştiği için katman atma işlemi iptal edildi
cmd-file-no-recovery-directory = Kullanılabilir kurtarma dizini yok: { $error }
cmd-file-no-unsaved-project-content-nothing = Kaydedilmemiş proje içeriği yok; kurtarılacak bir şey yok
cmd-file-parsing-browser-dxf-import = Tarayıcı DXF içe aktarımı ayrıştırılıyor…
cmd-file-parsing-dxf-import = DXF içe aktarımı ayrıştırılıyor…
cmd-file-project-closes-after-save = Geçerli kaydı bittikten sonra proje kapanacak
cmd-file-the-project-closes-after-save = Geçerli kaydı bittikten sonra proje kapanacak
cmd-file-queued-count-triangulation-file-s = İçe aktarma için { $count } üçgenleme dosyası sıraya alındı
cmd-file-recovery-copies-path-reopen-them = Kurtarma kopyaları { $path } konumunda; yeniden başlattıktan sonra onları tekrar açın
cmd-file-recovery-copy-failed-error = Kurtarma kopyası başarısız oldu: { $error }
cmd-file-recovery-copy-failed-failure = Kurtarma kopyası başarısız oldu: { $failure }
cmd-file-recovery-copy-written-path = Kurtarma kopyası yazıldı: { $path }
cmd-file-reverting-layer = Katman geri alınıyor…
cmd-file-reverting-project = Proje geri alınıyor…
cmd-file-save-failed-message = Kaydetme başarısız oldu: { $message }
cmd-file-save-worker-ended-without-result = Kaydetme işçisi sonuç döndürmeden sona erdi
cmd-file-saved-project-as = Proje şu şekilde kaydedildi: { $path }
cmd-file-saved-project = Proje kaydedildi: { $path }
cmd-file-selected-block-model-no-longer = Seçili blok model artık yüklü değil
cmd-file-switching-project = Proje değiştiriliyor…
cmd-file-triangulation-download-encoding-failed = Üçgenleme indirme kodlaması başarısız oldu: { $error }
cmd-file-user-chose-exit-without-saving = Kullanıcı kaydetmeden çıkmayı seçti
cmd-file-user-requested-exit-project-export = Kullanıcı çıkışı istedi (proje dışa aktarma veya kaydedilmemiş çalışma onayı gerekli)
cmd-file-viewport = Görüntü alanı
cmd-file-wait-current-project-save-finish = Geçerli proje kaydının bitmesini bekleyin
cmd-file-wait-current-project-switch-finish = Geçerli proje değişiminin bitmesini bekleyin
cmd-file-wait-project-operation-finish-before = Değişiklikleri atmadan önce proje işleminin bitmesini bekleyin
cmd-file-wait-project-revert-finish-before = Kaydetmeden önce proje geri alma işleminin bitmesini bekleyin
cmd-fuse-closed-polyline = Kapalı çoklu çizgi
cmd-fuse-count-source-line-s = { $count } kaynak çizgi
cmd-fuse-created-shape-object-id-vertices = { $sources } kaynak çizgiden { $vertices } köşeli { $shape } { $object_id } oluşturuldu
cmd-fuse-click-missed = Kaynaştırma: tıklama herhangi bir nesneye isabet etmedi (imleç altında hiçbir şey yok)
cmd-fuse-click-not-near-endpoint = Kaynaştırma: tıklama, seçili çizginin uç noktalarından hiçbirine yeterince yakın değildi
cmd-fuse-clicked-closed-polyline = Kaynaştırma: tıklanan nesne { $object_id } kapalı bir çoklu çizgi, kaynaştırma yalnızca açık çoklu çizgilerde çalışır
cmd-fuse-clicked-not-open-polyline = Kaynaştırma: tıklanan nesne { $object_id } açık bir çoklu çizgi değil (bir { $kind })
cmd-fuse-clicked-object-missing = Kaynaştırma: tıklanan nesne { $object_id } artık mevcut değil
cmd-fuse-clicked-too-few-vertices = Kaynaştırma: tıklanan çoklu çizgi { $object_id }'in yalnızca { $count } köşesi var, en az 2 gerekli
cmd-fuse-endpoint-marker-missing = Kaynaştırma: uç nokta işareti { $marker_index } artık mevcut değil
cmd-fuse-close-needs-three-vertices = Kaynaştırma: çizginin bir çoklu çizgi olarak kapanması için en az 3 farklı köşesi olmalı (mevcut: { $count })
cmd-fuse-lines = Çizgileri Kaynaştır
cmd-fuse-needs-two-segments = Kaynaştırma: uygulamak için en az 2 segment gerekli (mevcut: { $count })
cmd-fuse-no-active-layer = Kaynaştırma: kaynaştırılan çizginin yerleştirileceği etkin katman yok
cmd-fuse-no-active-project = Kaynaştırma: etkin proje yok, uygulanamıyor
cmd-fuse-no-source-line = Kaynaştırma: çoklu çizgi olarak kapatılacak kaynak çizgi yok
cmd-fuse-awaiting-object-invalid = Kaynaştırma: nesne { $awaiting_id } artık geçerli bir çoklu çizgi değil
cmd-fuse-object-already-in-chain = Kaynaştırma: nesne { $object_id } zaten kaynaştırma zincirinin bir parçası, farklı bir çizgi tıklayın
cmd-fuse-result-too-few-vertices = Kaynaştırma: sonucun köşe sayısı çok az ({ $count }), iptal ediliyor
cmd-fuse-segment-object-invalid = Kaynaştırma: segment nesnesi { $object_id } artık geçerli bir çoklu çizgi değil, iptal ediliyor
cmd-fuse-source-object-invalid = Kaynaştırma: kaynak nesne { $object_id } artık geçerli bir açık çoklu çizgi değil
cmd-fuse-source-object-missing = Kaynaştırma: kaynak nesne { $object_id } artık mevcut değil
cmd-fuse-open-polyline = Açık çoklu çizgi
cmd-include-failed = Dahil etme başarısız oldu: { $message }
cmd-include-included-solid-shape-name-topology = '{ $shape_name }' katısı '{ $topology_name }' topolojisine dahil edildi ({ $retained } topoloji yüzeyi korundu, { $skipped } kapanış yüzeyi atlandı)
cmd-include-including-pit-stockpile-solid = Ocak/stok sahası katısı dahil ediliyor…
cmd-insert-point-count-operation-point-s = { $count } { $operation } nokta
cmd-insert-point-elevation-must-be-finite = Kotta Nokta Ekle işlemi sonlu bir kot değeri gerektirir
cmd-insert-point-insert-points = Noktalar Ekle
cmd-insert-point-inserted-count-operation-point-s = { $count } { $operation } nokta eklendi
cmd-insert-point-intersection = Kesişim
cmd-insert-point-no-new-operation-points-were = Yeni { $operation } noktası bulunamadı
cmd-insert-point-select-least-two-polylines-before = Kesişim noktaları eklemeden önce en az iki çoklu çizgi seçin
cmd-insert-point-select-one-more-polylines-before = Kotta nokta eklemeden önce bir veya daha fazla çoklu çizgi seçin
cmd-layer-created-layer-name = '{ $name }' katmanı oluşturuldu
cmd-layer-deleted-with-objects = { $layer_id } katmanı (ve üzerindeki tüm nesneler) silindi
cmd-layer-duplicated-layer-duplicate-name = '{ $duplicate_name }' katmanı çoğaltıldı
cmd-layer-locked = Kilitli
cmd-layer-name-copy = { $name } kopyası
cmd-layer-selected-count-object-s-layer = { $layer_id } katmanında { $count } nesne seçildi
cmd-layer-state-layer-name = '{ $name }' katmanı { $state }
cmd-layer-unlocked = Kilidi Açık
cmd-move-tool-moved-collars = { $count } sondaj deliği ağzına taşıma deltası ({ $delta }) uygulandı
cmd-move-tool-moved-objects = { $count } nesneye taşıma deltası ({ $delta }) uygulandı
cmd-move-tool-count-hole-s = { $count } delik
cmd-object-edit-edited-kind = { $kind } düzenlendi
cmd-object-edit-edited-kind-count-vertices = { $kind } düzenlendi ({ $count } köşe)
cmd-object-edit-no-changes-apply = Uygulanacak değişiklik yok
cmd-object-edit-object-changed-since-editor-opened = Bu nesne düzenleyici açıldıktan sonra değişti; mevcut sürümü düzenlemek için yeniden açın
cmd-object-edit-target-changed = Düzenlenen nesne değişti; düzenleme atlandı
cmd-object-edit-object-no-longer-exists-document = Bu nesne artık belgede yok
cmd-object-edit-select-single-design-object-edit = Düzenlemek için tek bir tasarım nesnesi seçin
cmd-object-edit-unassigned = Atanmamış
cmd-offset-create-offset = Ofset Oluştur
cmd-offset-created-offset-count-object-s = { $count } nesnenin ofseti oluşturuldu
cmd-offset-distance-must-be-positive = Ofset mesafesi sıfırdan büyük olmalıdır
cmd-omf-could-not-open-project-source = { $source_name } projesi açılamadı: { $error }
cmd-omf-create-open-project-before-merging = Veri birleştirmeden önce bir proje oluşturun veya açın
cmd-omf-encoding-project = Proje kodlanıyor…
cmd-omf-exported-project-path = Proje { $path } konumuna dışa aktarıldı
cmd-omf-imported-project = '{ $project_name }' projesi { $source_name } konumundan içe aktarıldı: { $count } üst düzey veri kümesi
cmd-omf-importing-project = Proje içe aktarılıyor…
cmd-omf-export-failed = OMF dışa aktarma başarısız oldu: { $error }
cmd-omf-import-failed = OMF içe aktarma başarısız oldu: { $error }
cmd-omf-opened-project = '{ $project_name }' projesi { $source_name } konumundan açıldı
cmd-omf-project-source-name-contains-no = '{ $source_name }' projesi desteklenen hiçbir veri öğesi içermiyor
cmd-omf-source-name-applied-project-origin = { $source_name }: birleştirmeden önce proje orijini { $origin } uygulandı
cmd-omf-crs-differs = { $source_name }: koordinat referans sistemi '{ $source_crs }', proje CRS'sinden '{ $target_crs }' farklı; koordinatlar yeniden izdüşümlenmeden birleştirildi
cmd-omf-source-name-units-source-units = { $source_name }: birimler '{ $source_units }', proje birimlerinden '{ $target_units }' farklı; koordinatlar dönüştürülmeden birleştirildi
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = Dışa aktarılacak açık Incline Design verisi yok
cmd-placement-2-vertices = 2 köşe
cmd-placement-count-vertices = { $count } köşe
cmd-placement-created-circle = { $radius } m yarıçaplı çember oluşturuldu
cmd-placement-created-closed-polyline = { $count } köşeli kapalı çoklu çizgi oluşturuldu
cmd-placement-created-line-segment-2-vertices = 2 köşeli çizgi segmenti oluşturuldu
cmd-placement-created-open-polyline-count-vertices = { $count } köşeli açık çoklu çizgi oluşturuldu
cmd-placement-placed-point-x-y-z = Nokta { $x }, { $y }, { $z } konumuna yerleştirildi
cmd-placement-radius = Yarıçap { $radius } m
cmd-plot-composing-engineering-drawing = Mühendislik çizimi oluşturuluyor…
cmd-plot-could-not-write-engineering-drawing = Mühendislik çizimi yazılamadı: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Çizim ölçeği görünen verilere sığdırıldı: 1:{ $scale }
cmd-plot = Çizim
cmd-plot-saved-drawing = Mühendislik çizimi kaydedildi: { $description } ({ $dpi } dpi'de { $width } × { $height } piksel)
cmd-point-cloud-failed-load-point-cloud-error = Nokta bulutu yüklenemedi: { $error }
cmd-point-cloud-loaded-point-cloud-name-count = { $name } nokta bulutu yüklendi ({ $count } nokta)
cmd-point-cloud-point-cloud-loader-disconnected-path = { $path } için nokta bulutu yükleyici bağlantısı kesildi
cmd-point-cloud-tin-max-edge-disabled = (maks kenar devre dışı)
cmd-point-cloud-tin-max-edge-max-edge = (maks kenar { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = Nokta bulutu TIN işlemi başarısız oldu: { $error }
cmd-point-cloud-tin-subsampled = Arazi TIN: { $total } noktadan { $sampled } tanesi konumsal olarak alt örneklendi
cmd-point-cloud-tin-triangulated = Arazi TIN: { $vertex_count } benzersiz XY noktası { $face_count } yüzeye üçgenlendi{ $suffix }
cmd-products-added-product-delay-ms-ms = { $delay_ms } ms { $name } ürünü eklendi
cmd-products-deleted-product-delay-ms-ms = { $delay_ms } ms { $name } ürünü silindi
cmd-products-failed-save-products-error = Ürünler kaydedilemedi: { $error }
cmd-products-product-no-longer-palette = Bu ürün artık palette değil
cmd-property-action-count-object-s-layer = { $count } nesne { $layer } katmanına { $action }
cmd-property-batch-set-axis-value-count = { $count } nesne için { $axis } değeri toplu ayarlandı
cmd-property-batch-set-closed-count-polyline = { $count } çoklu çizgi için kapalı durumu toplu ayarlandı
cmd-property-batch-set-color-count-object = { $count } nesne için renk toplu ayarlandı
cmd-property-batch-set-fill-style-count = { $count } nesne için dolgu stili toplu ayarlandı
cmd-property-batch-set-line-weight-count = { $count } çoklu çizgi için çizgi kalınlığı toplu ayarlandı
cmd-property-copied = Kopyalandı
cmd-property-moved = Taşındı
cmd-raster-draped = { $raster } rasteri { $triangulation } üçgenlemesi üzerine örtüldü (kapsamlar örtüşüyor)
cmd-raster-failed-load-raster-name-error = { $name } rasteri yüklenemedi: { $error }
cmd-raster-failed-load-raster-path-error = { $path } rasteri yüklenemedi: { $error }
cmd-raster-loaded-raster-name-via-driver = { $name } rasteri { $driver } ile yüklendi ({ $srcx }x{ $srcy }, önizleme { $prevx }x{ $prevy })
cmd-raster-no-overlapping-triangulation = Yüklü hiçbir üçgenleme { $name } kapsamıyla örtüşmüyor
cmd-raster-loader-disconnected = { $path } için raster yükleyici bağlantısı kesildi
cmd-raster-undraped = { $count } üçgenlemeden raster örtüleri kaldırıldı
cmd-relimit-click-missed = Yeniden sınırlama: tıklama herhangi bir nesneye isabet etmedi (imleç altında hiçbir şey yok)
cmd-relimit-click-ignored = Yeniden sınırlama: tıklama yok sayıldı, araç şu anda bir hedef seçimi beklemiyor
cmd-relimit-clicked-source-line = Yeniden sınırlama: kaynak çizginin kendisi tıklandı, farklı bir çizgi seçin
cmd-relimit-no-source-line = Yeniden sınırlama: kaynak çizgi ayarlanmadı, seçim iptal ediliyor
cmd-relimit-relimited-line-source-id-selected = { $source_id } çizgisi seçili hedefe yeniden sınırlandı
cmd-relimit-resized-line-source-id-using = { $source_id } çizgisi { $mode } modu { $value } değeri kullanılarak yeniden boyutlandırıldı
cmd-rename-item-no-longer-belongs-active = Bu öğe artık etkin projeye ait değil
cmd-rename-renamed-before-name = '{ $before }' adı '{ $name }' olarak değiştirildi
cmd-rename-renamed-name-taken = '{ $before }' adı '{ $name }' olarak değiştirildi ('{ $requested }' zaten kullanımda)
cmd-rotate-collar-turned-count-drillhole-collar-s = { $count } sondaj deliği ağzı { $rotation } döndürüldü
cmd-section-verb-count-item-s-section = { $section } içinde { $count } öğe { $verb }
cmd-selection-delete-vertex = Köşeyi Sil
cmd-selection-deleted-count-selected-object-s = Seçili { $count } nesne silindi
cmd-selection-deleted-vertex = { $object_id } çoklu çizgisinden { $vertex } köşesi silindi
cmd-selection-duplicate-selection = Seçimi Çoğalt
cmd-selection-duplicated-count-object-s = { $count } nesne çoğaltıldı
cmd-session-created-triangulation = '{ $name }' üçgenlemesi ({ $vertex_count } köşe, { $face_count } yüzey) { $surface_type } yüzey türünden oluşturuldu
cmd-session-deleted-triangulation = '{ $name }' üçgenlemesi projeden silindi
cmd-session-failed-load-triangulation-error = Üçgenleme yüklenemedi: { $error }
cmd-session-failed-load-triangulation-message = Üçgenleme yüklenemedi: { $message }
cmd-session-loaded-triangulation = '{ $name }' üçgenlemesi yüklendi ({ $path }, { $vertex_count } köşe, { $face_count } yüzey)
cmd-session-set-triangulation-tri-id-color = { $tri_id } üçgenlemesinin rengi { $color } olarak ayarlandı
cmd-session-triangulation-load-no-result = { $path } için üçgenleme yükleme sonuç döndürmeden sona erdi
cmd-session-triangulation-failed = Üçgenleme işlemi başarısız oldu: { $message }
cmd-session-unloaded-triangulation-name = '{ $name }' üçgenlemesi kaldırıldı
cmd-slice-entered-slice-view-cx-cy = Kesit görünümüne girildi @ { $cx }, { $cy }, { $cz }, { $dx }, { $dy } boyunca ({ $length }m çizgi)
cmd-slice-exited-slice-view = Kesit görünümünden çıkıldı
cmd-slice-reset-section-view-fit-extents = Kesit görünümünü sıfırla (sınırlara sığdır)
cmd-slice-set-section-grid-enabled = Kesit ızgarası etkin = { $enabled }
cmd-split-created-2-open-polylines = 2 açık çoklu çizgi oluşturuldu
cmd-split-line = Çizgiyi Böl
cmd-split-points-needs-interior-vertex = Noktalarda Böl: açık çizginin bir iç köşesini seçin
cmd-split-points-needs-non-adjacent-vertices = Noktalarda Böl: komşu olmayan iki çoklu çizgi köşesi seçin
cmd-split-polyline-into-two = Kaynak çoklu çizgi iki açık çoklu çizgiye bölündü
cmd-text-edit-finished = { $object_id } nesnesi için metin düzenleme tamamlandı
cmd-text-updated = { $object_id } nesnesindeki metin güncellendi
cmd-view-centre-rotation-not-available-flying = Dönüş merkezi uçuş modunda kullanılamaz
cmd-view-fixed-centre-rotation-x-y = Dönüş merkezi { $x }, { $y }, { $z } noktasına sabitlendi
cmd-view-no-point-under-cursor-fix = İmlecin altında dönüş merkezinin sabitleneceği bir nokta yok
cmd-view-released-centre-rotation = Dönüş merkezi serbest bırakıldı
cmd-view-reset-view-fit-extents = Görünümü sıfırla (kapsama sığdır)
cmd-view-set-topology-wireframes-enabled = Topoloji tel kafesleri = { $enabled } olarak ayarlandı
cmd-view-set-view-points-enabled = Görünüm noktaları = { $enabled } olarak ayarlandı
cmd-view-set-xy-grid-enabled = XY ızgarası etkin = { $enabled }
cmd-view-zoom-extents-preserving-angle = Kapsama yakınlaştır (açıyı koru)

## Common strings

common-add-product = Ürün Ekle
common-background = Arka plan
common-block-model = Blok model
common-block-models = Blok Modeller
common-cancelled = İptal edildi
common-chamfer = Pah
common-choose = Seç...
common-circle = Daire
common-click-point-fix-centre-rotation = Dönüş merkezini sabitlemek için bir noktaya tıklayın
common-clip-surface-polyline = Yüzeyi Çoklu Çizgiyle Kırp...
common-closed = Kapalı
common-colour = Renk
common-confirm-omf-rewrite = OMF Üzerine Yazmayı Onayla
common-could-not-replace-current-project = Geçerli proje değiştirilemedi: { $error }
common-count-object-s = { $count } nesne
common-create = Oluştur
common-create-batter-berm = Şev-Berm Oluştur
common-create-bezier-curve = Bezier Eğrisi Oluştur
common-create-block-model = Blok Model Oluştur
common-create-block-model-ellipsis = Blok Model Oluştur...
common-create-circle = Çember Oluştur
common-create-drill-pattern = Delme Deseni Oluştur
common-create-layer = Katman Oluştur
common-create-line = Çizgi Oluştur
common-create-ore-triangulation = Cevher Üçgenlemesi Oluştur
common-create-ore-triangulation-ellipsis = Cevher Üçgenlemesi Oluştur...
common-create-point = Nokta Oluştur
common-create-polyline = Çoklu Çizgi Oluştur
common-create-triangulation = Üçgenleme Oluştur...
common-crosses = Artılar
common-cut = Kes
common-cut-topology-pit-shell = Topolojiyi Ocak Kabuğuyla Kes...
common-delete-layer = Katmanı Sil
common-delete-product = Ürünü Sil
common-delete-selection = Seçimi Sil
common-designs = Tasarımlar
common-discard-layer-changes = Katman Değişikliklerini At
common-down = Aşağı
common-drape-topology = Topolojiye Ör
common-easting = Doğu değeri
common-edit-object = Nesneyi Düzenle
common-edit-text = Metni Düzenle
common-elevation = Kot
common-exit-without-saving = Kaydetmeden Çık
common-export-engineering-drawing = Mühendislik Çizimini Dışa Aktar
common-filter = Filtre
common-fly-mode = Uçuş Modu
common-generate-contour-lines = Kontur Çizgileri Oluştur...
common-hide-all = Tümünü Gizle
common-hide-selection = Seçimi Gizle
common-ignore = Yok say
common-import-csv-block-model = CSV Blok Model İçe Aktar
common-import-dxf = DXF İçe Aktar
common-incline-design-project = Incline Design projesi
common-layer = Katman
common-legend = Gösterge
common-line = Çizgi
common-line-weight = Çizgi kalınlığı
common-lock-all = Tümünü Kilitle
common-lock-selection = Seçimi Kilitle
common-m = m
common-max = Maks
common-merge-shell-into-topology = Kabuğu Topolojiyle Birleştir
common-merge-shell-into-topology-ellipsis = Kabuğu Topolojiyle Birleştir...
common-move-collar = Ağzı Taşı
common-move-design = Tasarımı Taşı
common-move-selection = Seçimi Taşı
common-new-product = Yeni Ürün
common-no-block-models = Blok model yok
common-no-design-layers = Tasarım katmanı yok
common-no-drill-holes = Sondaj deliği yok
common-no-file-chosen = Dosya seçilmedi
common-no-open-project = Açık proje yok
common-no-point-clouds = Nokta bulutu yok
common-no-triangulations = Üçgenleme yok
common-none = Yok
common-northing = Kuzey değeri
common-offset = Ofset
common-open = Aç
common-orientation = Yönlendirme
common-point = Nokta
common-point-cloud = Nokta bulutu
common-point-clouds = Nokta Bulutları
common-polyline = Çoklu çizgi
common-polyline-layer = '{ $layer }' üzerindeki çoklu çizgi
common-project = Proje
common-rasters = Rasterler
common-redo = Yinele
common-relimit-line = Çizgiyi Yeniden Sınırla
common-remove-project = Projeyi Kaldır
common-reset-view = Görünümü Sıfırla
common-reveal-all = Tümünü Göster
common-reveal-finder = Finder'da Göster
common-rotate-collar = Ağzı Döndür
common-save-exit = Kaydet ve Çık
common-scale-bar = Ölçek çubuğu
common-set-initiation-point = Ateşleme Noktasını Ayarla
common-shape = Şekil
common-shell = Kabukla
common-slashes = Eğik çizgiler
common-slice = Kesit
common-slice-triangulation-z-range = Üçgenlemeyi Z Aralığına Göre Kes...
common-surface-contours = Yüzey Konturları
common-text = Metin
common-degree-suffix = °
common-tie-holes = Delikleri Bağla
common-triangulations = Üçgenlemeler
common-trim-topology = Topolojiye Kırp...
common-undo = Geri Al
common-undrape-all = Tüm Örtüleri Kaldır
common-uniform-white = Tek renk beyaz
common-unlock-all = Tümünün Kilidini Aç
common-untitled = Adsız
common-up = Yukarı
common-vertical-exaggeration = Dikey Abartma
common-x = x
common-zoom-extents = Kapsama Yakınlaştır

## Confirmations strings

confirmations-close-project-unsaved-changes = Projeyi Kapat: Kaydedilmemiş Değişiklikler
confirmations-close-without-saving = Kaydetmeden Kapat
confirmations-delete = Sil
confirmations-delete-objects = Nesneleri Sil
confirmations-discard = At
confirmations-discard-all-unsaved-changes-layer =
    '{ $name }' katmanı için kaydedilmemiş tüm değişiklikler atılsın mı?
    Kaydedilen katman diskten yeniden yüklenirken diğer katmanlardaki değişiklikler korunur. Bu işlem geri alınamaz.
confirmations-discard-all-unsaved-changes-name =
    '{ $name }' için kaydedilmemiş tüm değişiklikler atılsın mı?
    Son kaydedilen sürüm diskten yeniden yüklenir. Bu işlem geri alınamaz.
confirmations-discard-changes = Değişiklikleri At
confirmations-exit-unsaved-changes = Çıkış: Kaydedilmemiş Değişiklikler
confirmations-incline-design-cannot-reproduce-all = Incline Design, orijinal OMF'deki tüm içeriği yeniden oluşturamaz. Kaydetme işlemi aşağıdaki içeriği atlayacaktır:
confirmations-product = Ürün
confirmations-project = bu proje
confirmations-remove-name-delete-its-browser = '{ $name }' kaldırılsın ve tarayıcıda saklanan kopyası silinsin mi? Kaydedilmemiş değişiklikler kaybolacak.
confirmations-remove-project-unsaved-changes = Projeyi Kaldır: Kaydedilmemiş Değişiklikler
confirmations-remove-without-saving = Kaydetmeden Kaldır
confirmations-replace-project-unsaved-changes = Projeyi Değiştir: Kaydedilmemiş Değişiklikler
confirmations-save = Kaydet
confirmations-save-anyway = Yine de Kaydet
confirmations-save-changes-current-project-before = Değiştirmeden önce mevcut projedeki değişiklikler kaydedilsin mi?
confirmations-save-changes-name-before-closing = Kapatmadan önce '{ $name }' içindeki değişiklikler kaydedilsin mi?
confirmations-save-changes-name-before-removing = Incline Design'dan kaldırmadan önce '{ $name }' içindeki değişiklikler kaydedilsin mi?
confirmations-save-close = Kaydet ve Kapat
confirmations-save-modified-project-before-exiting = Çıkmadan önce değiştirilmiş proje kaydedilsin mi?
confirmations-save-to-browser-before-exit = Çıkmadan önce değiştirilmiş proje tarayıcı deposuna kaydedilsin mi?
confirmations-save-remove = Kaydet ve Kaldır

## Console strings

console-copy-all = Tümünü kopyala
console-copy-message = Mesajı kopyala
console-error = HATA
console-info = BİLGİ
console-no-console-activity-yet = Henüz konsol etkinliği yok
console-pending = BEKLİYOR
console-progress-summary = Devam ediyor · { $summary }
console-success = BAŞARILI
console-warn = UYARI

## Csv strings

csv-block-model-category = Kategori
csv-block-model-value = Değer

## Drill strings

drill-hole-add-stop = Durak ekle
drill-hole-all-rendered-intervals-opaque-white = Çizilen tüm aralıklar opak beyazdır.
drill-hole-burden-spacing-must-greater-than = Burden ve aralık sıfırdan büyük olmalıdır
drill-hole-choose-valid-closed-polyline = Geçerli kapalı bir çoklu çizgi seçin
drill-hole-colour-scale = Renk ölçeği
drill-hole-field = Alan
drill-hole-grayscale = Gri tonlama
drill-hole-green-yellow-red = Yeşil–Sarı–Kırmızı
drill-hole-heat = Isı
drill-hole-no-holes-fit-inside-boundary = Geçerli burden ve aralıkta bu sınırın içine hiçbir delik sığmıyor
drill-hole-pattern-too-many-holes = Desen, maksimum { $maximum } delik sınırını aşıyor; burden veya aralığı artırın
drill-hole-preset = Ön ayar
drill-hole-px = px
drill-hole-rainbow = Gökkuşağı
drill-hole-reset-preset = Ön ayarı sıfırla
drill-hole-rotation-offsets-must-contain-valid = Döndürme ve ofsetler geçerli sayılar içermelidir
drill-hole-selected-polyline-has-no-usable = Seçili çoklu çizginin kullanılabilir bir XY alanı yok
drill-hole-smooth-interpolation = Yumuşak enterpolasyon
drill-hole-spacing-would-scan-too-many = Bu aralık çok fazla ızgara hücresini tarar; burden veya aralığı artırın (maksimum { $maximum } delik)
drill-hole-square = Kare
drill-hole-staggered = Şaşırtmalı
drill-hole-stepped-bands = Kademeli bantlar
common-times-sign = ×
common-minus-sign = −
drill-hole-unsupported-drillhole-source = Desteklenmeyen sondaj deliği kaynağı
drill-hole-width = Genişlik
drill-pattern-arrangement = Düzen
drill-pattern-axis-offset = { $axis } ofseti
drill-pattern-blast-shape = Patlatma şekli
drill-pattern-burden = Burden
drill-pattern-choose-closed-blast-boundary-then = Kapalı bir patlatma sınırı seçin, ardından ızgarayı ayarlayın. Sondaj delikleri görüntü alanında canlı olarak güncellenir.
drill-pattern-closed-design-polyline-whose-xy = XY izdüşümü deliklerle doldurulacak kapalı tasarım çoklu çizgisi.
drill-pattern-rotation-help = Küresel { $axis } eksenine göre desenin saat yönünün tersine döndürülmesi.
drill-pattern-distance-between-holes-along-each = Her desen sırası boyunca delikler arasındaki mesafe.
drill-pattern-name-hint = örn. Bati Kesim 03
drill-pattern-diameter-help = Bitmiş delik çapı. Milimetre cinsinden girilir ve oluşturulan her delikle birlikte saklanır.
drill-pattern-hole-depth = Delik derinliği
drill-pattern-hole-diameter = Delik çapı
drill-pattern-move-over-closed-polyline-then = İmleci kapalı bir çoklu çizginin üzerine getirin, ardından görüntü alanında ona tıklayın. Esc, seçimi iptal eder.
drill-pattern-name-help = Projede oluşturulan sondaj deliği veri kümesinin adı.
drill-pattern-none-picked = Hiçbiri seçilmedi
drill-pattern-pattern-name = Desen adı
drill-pattern-spacing-help = Desen sıraları arasındaki dik mesafe.
drill-pattern-pick = Seç
drill-pattern-preview-count-hole-s-diameter = Önizleme: { $count } delik · { $diameter } mm çap · { $depth } m derinlik
drill-pattern-rotation = Döndürme
drill-pattern-shift-pattern-grid-along-global = Patlatma şekline kırpılmış halde kalırken desen ızgarasını küresel { $axis } ekseni boyunca kaydırır.
drill-pattern-spacing = Aralık
drill-pattern-staggered-offsets-every-second-row = Şaşırtmalı, her ikinci sırayı aralığın yarısı kadar kaydırır.
drill-pattern-vertical-depth-below-each-collar = Her ağzın altındaki dikey derinlik.

## Dxf strings

dxf-block-nesting-too-deep = DXF blok iç içe geçmesi maksimum derinliği ({ $depth }) aşıyor, '{ $name }' atlanıyor
dxf-circular-block-reference = DXF döngüsel blok referansı tespit edildi: '{ $name }'
dxf-undefined-layer = DXF varlığı tanımsız '{ $name }' katmanına başvurdu, '{ $fallback }' olarak içe aktarıldı
dxf-import-budget-exceeded = DXF içe aktarımı { $what } bütçesini ({ $limit }) aşıyor; kalan geometri atlanıyor
dxf-insert-unknown-block = DXF INSERT bilinmeyen '{ $name }' bloğuna başvuruyor

## Edit strings

edit-absolute-length = Mutlak uzunluk
edit-absolute-rl = Mutlak RL
edit-action = Eylem
edit-angle = Açı
edit-dip-help = Yataydan açı, aşağı yönde negatif: -90 dikey bir deliktir.
edit-app-web-not-recommended-production = { $app } Web, üretim kullanımı için önerilmez. Yalnızca demo olarak kullanın.
edit-application = Uygulama
edit-apply = Uygula
edit-apply-pick-target = Uygula ve Hedef Seç
edit-axis-value = { $axis } değeri
edit-azimuth = Azimut
edit-batter-angle = Şev açısı (°)
edit-azimuth-help = Deliklerin delindiği yön, ızgara kuzeyinden saat yönünde derece cinsinden.
edit-bench-height = Basamak yüksekliği
edit-benches = Basamaklar
edit-berm-width = Berm genişliği
edit-bezier-curve = Bezier Eğrisi
edit-choose-layer = Bir katman seçin
edit-measure-help = Girilen değerin şev boyunca mesafe mi, yatay genişlik mi yoksa dikey yükseklik mi olduğunu seçin.
edit-choose-which-two-polyline-paths = Seçili köşeler arasındaki iki çoklu çizgi yolundan hangisinin değiştirileceğini seçin. Uzunluk, kotu ve eğrisel kenarları içerir.
edit-click-corner-closed-polyline = Kapalı bir çoklu çizgide bir köşeye tıklayın.
edit-click-open-closed-polyline-begin = Başlamak için açık veya kapalı bir çoklu çizgiye tıklayın.
edit-click-second-vertex-replacement-span = Değiştirilecek aralığın ikinci köşesine tıklayın.
edit-click-vertex-start-replacement-span = Değiştirilecek aralığı başlatmak için bir köşeye tıklayın.
edit-collide-triangulation = Üçgenlemeyle Çarpış
edit-confirm-selection = Seçimi Onayla
edit-control-point-1 = Kontrol noktası 1
edit-control-point-2 = Kontrol noktası 2
edit-copy = Kopyala
edit-corner-radius-limited-so-replacement = Köşe yarıçapı; değişiklik komşu köşeleri geçemeyecek şekilde sınırlıdır.
edit-create-new-layer = Yeni bir katman oluştur
edit-create-new-project = Yeni bir proje oluştur
edit-create-project = Proje oluştur
edit-delta-length-m-use = Uzunluk değişimi (m, + veya - kullanın)
edit-dip = Eğim
edit-direction = Yön
edit-distance = Mesafe
edit-distance-along-slope = Şev boyunca mesafe
edit-download-free-native-version-our = Ücretsiz masaüstü sürümünü web sitemizden indirin ↗
edit-drill-hole = Sondaj Deliği
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = Bitiş
edit-enter-valid-elevation = Geçerli bir kot girin.
edit-exit-slice = Kesitten çık
edit-finish-polyline = Çoklu Çizgiyi Bitir
edit-generate-batter-berms = Şev-Bermleri Oluştur
edit-height = Yükseklik
edit-height-change = Yükseklik değişimi
edit-height-mode = Yükseklik modu
edit-horizontal-distance = Yatay mesafe
edit-horizontal-width-each-flat-berm = Ardışık şevler arasındaki her düz berm'in yatay genişliği.
edit-hover-choose-which-end-move = Taşınacak ucu seçmek için üzerine gelin, ardından onaylamak için tıklayın.
edit-insert-point-elevation = Kotta Nokta Ekle
edit-intersect = Kesiştir
edit-kind-properties = { $kind } { $properties }
edit-layer-name = Katman adı
edit-load-project = Proje Yükle
edit-longest = En uzun
edit-m-s = m/sn
edit-measure = Ölçü
edit-mit-license = MIT Lisansı
edit-mode = Mod
edit-move = Taşı
edit-move-layer = Katmana Taşı
edit-move-which-end = Hangi ucu taşı
edit-movement-speed-slice-when-using = Yön tuşları kullanılırken kesitin hareket hızı.
edit-moving-end-endpoint = Taşınıyor: Bitiş ucu
edit-moving-start-endpoint = Taşınıyor: Başlangıç ucu
edit-new-length-m = Yeni uzunluk (m)
edit-new-project = Yeni Proje
edit-number-complete-batter-berm-levels = Tam şev ve berm seviyesi sayısı. Maksimum, belirtilen geometriyi koruyan en derin seviyeyle sınırlıdır.
edit-bezier-segments-help = Seçili iki köşe arasındaki eğriyi yaklaştırmak için kullanılan çizgi segmenti sayısı.
edit-chamfer-segments-help = Yuvarlatılmış köşeyi yaklaştırmak için kullanılan düz segment sayısı. Düz bir pah için 1 kullanın.
edit-object = Nesne
edit-offset-element = Ofset Elemanı
edit-pick-side = Taraf Seç
edit-pit = Ocak
edit-project-name = Proje adı
edit-properties = Özellikler
edit-radius = Yarıçap
edit-recent = Son
edit-relative = Göreceli (+/-)
edit-elevation-mode-help = Göreceli, her noktaya dikey bir değişiklik uygular. Mutlak RL, her noktayı tek bir hedef kota izdüşürür.
edit-remove-from-list = Listeden Kaldır
edit-replace-path = Yolu değiştir
edit-rotate = Döndür
edit-rotation-speed-slice-when-using = Q ve E kullanılırken kesitin döndürme hızı.
edit-s = °/sn
edit-segments = Segmentler
edit-segments-lying-elevation-ignored = Bu kotta bulunan segmentler yok sayılır.
edit-endpoint-help = Değişecek uç noktayı seçin; diğer uç sabit kalır.
edit-selected-holes-point-different-ways = Seçili delikler farklı yönleri gösteriyor. Uygula, hepsini bu açılara ayarlar.
edit-selected-start-end-point-moves = Seçili başlangıç veya bitiş noktası çizgi yönü boyunca hareket eder; karşı uç sabit kalır.
edit-set-axis = { $axis } Ayarla
edit-shortest = En kısa
edit-slice-view = Kesit Görünümü
edit-slope-angle-each-batter-face = Yatay düzlemden ölçülen her şev yüzeyinin eğim açısı.
edit-slope-angle-offset-positive-negative = Ofsetin eğim açısı. Pozitif ve negatif açılar, kopya yana doğru hareket ederken onu kaynağın üstüne veya altına taşır.
edit-speed = Hız
edit-start = Başlangıç
edit-stockpile = Stok sahası
edit-stop-generated-offset-where-its = Oluşturulan ofseti, yolunun görünen bir üçgenlemeyle ilk karşılaştığı yerde durdur.
edit-target-rl = Hedef RL
edit-text-colour-opacity = Metin rengi ve saydamlığı.
edit-thickness-visible-slice-slab-centred = Genel bakış göstergesi ortalı görünen kesit dilimi kalınlığı.
edit-translation-axis-help = Dünya { $axis } ekseni boyunca öteleme mesafesi.
edit-type = Tür
edit-type-direction-together-set-offset = Tür ve Yön birlikte ofset tarafını belirler. Ocak + Yukarı ve Stok Sahası + Aşağı dışa doğru adımlar; Ocak + Aşağı ve Stok Sahası + Yukarı içe doğru adımlar.
edit-bench-direction-help = Yukarı, her basamağı basamak yüksekliği kadar yükseltir; Aşağı onu alçaltır. Bu ayrıca ofset tarafını da ters çevirir - bkz. Tür.
edit-value-help = Değer, seçilen Ölçü ve Yükseklik moduna göre yorumlanır.
edit-vertical-rise-fall-each-bench = Bir sonraki berm oluşturulmadan önce her basamağın dikey yükselişi veya alçalışı.
edit-bezier-control-point-1-help = İlk Bezier kontrol noktasının dünya X, Y ve Z koordinatları.
edit-bezier-control-point-2-help = İkinci Bezier kontrol noktasının dünya X, Y ve Z koordinatları.

## Events strings

events-couldn-t-exit-error = Çıkılamadı: { $error }
events-couldn-t-save-error = Kaydedilemedi: { $error }
events-set-elevation = Kotu Ayarla
events-set-elevation-from-cursor-hit = İmleç isabetinden kot Z { $z } olarak ayarlandı
events-tool-not-available-section-view = Bu araç kesit görünümünde kullanılamaz

## Explorer strings

explorer-clear-active-triangulation-texture = Etkin Üçgenleme Dokusunu Temizle
explorer-delete-from-project = Projeden Sil
explorer-discard-changes = Değişiklikleri At...
explorer-download = İndir
explorer-drape-over-surface = Yüzeye Ör
explorer-draped-over-surface = Bir yüzey üzerine örtülü
explorer-duplicate = Çoğalt
explorer-face-colour = Yüzey rengi
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    { $count } renk değişkeni
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    { $holes } delik
    { $fields } renk alanı
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    { $count } nokta
explorer-raster-id =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = Yükle
explorer-lock = Kilitle
explorer-select-all-objects = Tüm Nesneleri Seç
explorer-source-name = Kaynak: { $name }
explorer-unload = Kaldır
explorer-unlock = Kilidi Aç

## Files strings

files-automatic-colour = Otomatik renk
files-automatic-rl-spacing = Otomatik kot aralığı
files-axis-scale-ratio = { $axis } ölçek oranı
files-ok = Tamam
files-reset-scale = 1×'e sıfırla
files-rl-grid-options = Kot Izgarası Seçenekleri
files-rl-spacing = Kot aralığı
files-scales-z-distances-visually-without = Saklanan koordinatları değiştirmeden Z mesafelerini görsel olarak ölçekler.
files-thickness = Kalınlık
files-xy-grid-options = XY Izgara Seçenekleri

## Gpu strings

gpu-cache-block-model-surface-build-failed = Blok model yüzeyi oluşturulamadı: { $error }
gpu-cache-block-model-surface-build-worker = Blok model yüzey oluşturma işçisinin bağlantısı kesildi
gpu-cache-block-model-surface-chunk-rejected = Blok model yüzey yığını GPU ayırmadan önce reddedildi: örnekler={ $instances } bayt, sınır={ $limit } bayt
gpu-cache-block-volume-worker-disconnected = Blok hacim hazırlama işçisinin bağlantısı kesildi
gpu-cache-translucent-volume-could-not-built = Yarı saydam hacim oluşturulamadı ({ $error }); bu blok model bunun yerine küpler olarak gösteriliyor.
gpu-cache-edge-chunk-rejected = Üçgenleme kenar yığını GPU ayırmadan önce reddedildi: örnekler={ $instances } bayt, sınır={ $limit } bayt
gpu-cache-triangulation-chunk-rejected = Üçgenleme GPU yığını ayırmadan önce reddedildi: köşeler={ $vertices } bayt, indeksler={ $indices } bayt, sınır={ $limit } bayt
gpu-cache-triangulation-too-many-vertices = '{ $name }' üçgenlemesinin { $count } köşesi var (> u32::MAX); GPU için yığınlanamıyor
gpu-cache-triangulation-uploaded = '{ $name }' üçgenlemesi { $chunks } konumsal yığında yüklendi ({ $faces } yüzey)
i18n-active-language = Etkin dil { $language } (paket dahil: { $bundled })
i18n-could-not-select-language-error = Bir dil seçilemedi: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = GPU bağdaştırıcı: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = GPU sürücüsü: { $driver } { $driver_info }
init-gpu-supports-maximum-buffer-size = GPU en fazla { $size } MiB arabellek boyutunu destekliyor; büyük sahneler tam olarak görüntülenmeyebilir
init-surface-present-mode = Yüzey sunum modu: { $mode }
init-wgpu-error-continuing-error = wgpu hatası (devam ediliyor): { $error }

## Io strings

io-ascii-points-xyz-pts = ASCII Noktaları (.xyz, .pts)
io-attribute = Öznitelik
io-blank-header = (boş başlık)
io-block-model = Blok model:
io-choose-file-purpose-map-its = Sütunlarını eşlemek için bir dosya amacı seçin.
io-choose-loaded-block-model = Yüklü bir blok model seçin
io-choose-loaded-layer = Yüklü bir katman seçin
io-choose-loaded-triangulation = Yüklü bir üçgenleme seçin
io-choose-purpose = Amaç seçin…
io-choose-source-file-files-import = İçe aktarılacak kaynak dosyayı veya dosyaları seçin.
io-collar = Ağız
io-column-mapping = Sütun eşleme
io-comma-separated-values-csv = Virgülle Ayrılmış Değerler (.csv)
io-csv-files = CSV dosyaları
io-default = Varsayılan
io-depth = Derinlik
io-diameter = Çap
io-drawing-exchange-format-dxf = Drawing Exchange Format (.dxf)
io-drill-holes = Sondaj delikleri
io-east-x = Doğu / X
io-elevation-z = Kot / Z
io-end-x = Bitiş X
io-end-y = Bitiş Y
io-end-z = Bitiş Z
io-explicit-segments = Açık segmentler
io-export = Dışa Aktar
io-export-csv-block-model = CSV Blok Model Dışa Aktar
io-export-dxf = DXF Dışa Aktar
io-export-one-layer = Bir katmanı dışa aktar
io-export-ply = PLY Dışa Aktar
io-export-stl = STL Dışa Aktar
io-export-wavefront-obj = Wavefront OBJ Dışa Aktar
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = İçe Aktar
io-import-ascii-point-cloud = ASCII Nokta Bulutu İçe Aktar
io-import-drillhole-csv-bundle = Sondaj Deliği CSV Paketi İçe Aktar
io-import-geotiff = GeoTIFF İçe Aktar
io-import-las-laz-point-cloud = LAS/LAZ Nokta Bulutu İçe Aktar
io-import-pcd-point-cloud = PCD Nokta Bulutu İçe Aktar
io-import-ply = PLY İçe Aktar
io-import-stl = STL İçe Aktar
io-import-wavefront-obj = Wavefront OBJ İçe Aktar
io-interval = Aralık
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-mapped-csv-bundle-csv = Eşlenmiş CSV paketi (.csv)
io-model-file = Model dosyası
io-name-count-files = { $name } + { $count } dosya
io-no-csv-chosen = .csv seçilmedi
io-no-csv-files-chosen = CSV dosyası seçilmedi
io-no-dxf-chosen = .dxf seçilmedi
io-no-omf-chosen = .omf seçilmedi
io-north-y = Kuzey / Y
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-source-file = Kaynak dosya
io-start-x = Başlangıç X
io-start-y = Başlangıç Y
io-start-z = Başlangıç Z
io-stl = STL (.stl)
io-triangulation = Üçgenleme:
io-unmapped = Eşlenmemiş
io-wavefront-obj = Wavefront OBJ (.obj)

## Jobs strings

jobs-background-task-poll-label-ended = '{ $poll_label }' arka plan görevi sonuç döndürmeden sona erdi
jobs-discarded-stale-result = Bir kaynak değiştiği veya kapandığı için '{ $poll_label }' için eski arka plan sonucu atıldı

## Logging strings

logging-activity-completed = Etkinlik tamamlandı
logging-activity-started = Etkinlik başladı
logging-application-id-id = Uygulama Kimliği: { $id }
logging-application-name = Uygulama adı: { $name }
logging-application-startup = Uygulama Başlatma
logging-build-target-os-architecture = Derleme hedefi: { $os }-{ $architecture }
logging-completed = Tamamlandı
logging-count-messages = { $count } mesaj
logging-desktop-session-xdg-session-type = Masaüstü oturumu: XDG_SESSION_TYPE={ $type }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Incline Design başlatılıyor
logging-locale-environment = Yerel ortam: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session = macOS oturumu: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = İşletim sistemi: GNU / Linux
logging-operating-system-macos = İşletim sistemi: macOS
logging-operating-system-microsoft-windows = İşletim sistemi: Microsoft Windows
logging-pointer-width = İşaretçi genişliği: { $width }-bit
logging-process-id-id = İşlem Kimliği: { $id }
logging-release-version = Sürüm: { $version }
logging-renderer = İşleyici
logging-rust-compiler-host = Rust derleyici sunucusu: { $host }
logging-system = Sistem
logging-system-error = Sistem Hatası
logging-unknown = bilinmiyor
logging-windows-session-sessionname-session = Windows oturumu: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = Çalışıyor…

## Mac strings

mac-cannot-install-macos-menu-bar = macOS menü çubuğu ana iş parçacığı dışında kurulamaz
mac-quit-app = { $app } Uygulamasından Çık

## Main strings

main-incline-design-web-startup-failed = Incline Design Web başlatma işlemi başarısız oldu: { $error }

## Menu strings

menu-count-files-selected = { $count } dosya seçildi

## Object strings

object-edit-appearance = Görünüm
object-edit-arc-circle = Yay ve Daire
object-edit-arc-segments = Yay segmentleri
object-edit-bulge = Bombe
object-edit-bulge-arcs-horizontal-data-model = Bombeli yaylar veri modeline göre yataydır: yay planda döner ve kot bir köşeden diğerine düz bir çizgide değişir.
object-edit-centre-x = Merkez X
object-edit-centre-y = Merkez Y
object-edit-centre-z = Merkez Z
object-edit-chord = Kiriş
object-edit-colour-layer = Katmana göre renk
object-edit-enter-number = Bir sayı girin
object-edit-follow-owning-layer-s-colour = Bu nesneye sabitlenmiş bir renk yerine sahip katmanın rengini kullan.
object-edit-id = ID
object-edit-identity = Özdeşlik
object-edit-insert-after = Sonrasına ekle
object-edit-join-last-vertex-back-first = Son köşeyi tekrar ilk köşeye bağlar.
object-edit-length = Uzunluk { $length } m
object-edit-move-down = Aşağı taşı
object-edit-move-up = Yukarı taşı
object-edit-object-has-no-arc-segments = Bu nesnenin yay segmenti yok.
object-edit-object-has-single-position = Bu nesnenin tek bir konumu var.
object-edit-object-needs-least-required-vertices = Bu nesne en az { $required } köşe gerektirir
object-edit-one-more-properties-not-valid = Bir veya daha fazla özellik geçerli bir sayı değil
object-edit-perimeter-area = Çevre { $length } m, alan { $area } m²
object-edit-reverse = Ters çevir
object-edit-row-invalid-number = Satır { $row }: konum veya bombe geçerli bir sayı değil
object-edit-sweep = Süpürme
object-edit-text-not-number = "{ $text }" bir sayı değil
object-edit-vertices = Köşeler

## Omf strings

omf-element-name-has-count-tie = '{ $name }' elemanının artık içermediği delikleri adlandıran { $count } bağlantısı var
omf-ignoring-colour-map-omf-attribute = OMF '{ $attribute }' özniteliğindeki renk haritası yok sayılıyor: { $error }
omf-mining-data-exported-incline = Incline tarafından dışa aktarılan maden verisi
omf-import = OMF içe aktarma
omf-texture = OMF dokusu
omf-validation-warnings = OMF doğrulama uyarıları: { $warnings }
omf-application-metadata-dropped = Proje uygulama meta verisi '{ $application }' korunmaz
omf-project-author-not-retained = Proje yazarı korunmaz
omf-project-description-not-retained = Proje açıklaması korunmaz
omf-unsupported-metadata-keys = Proje desteklenmeyen meta veri anahtarları içeriyor: { $keys }

## Plot strings

plot-1-1000-one-millimetre-sheet = 1:1000 ölçekte, sayfadaki bir milimetre arazide bir metredir.
plot-1-scale-covers-width-height = 1:{ $scale } · { $width } × { $height } m kaplar
plot-all-visible-data = Görünen tüm veriler
plot-automatic-grid-interval = Otomatik ızgara aralığı
plot-border = Kenarlık
plot-centre = Şuna ortala
plot-fit-scale-help = Görünen her şeyi sayfaya sığdıran en küçük standart ölçeği seçin.
plot-coordinate-grid = Koordinat ızgarası
plot-current-view-centre = Geçerli görünüm merkezi
plot-date-caps = TARİH
plot-date = Tarih
plot-dots-per-inch-paper-size = İnç başına nokta sayısı. Bu kağıt boyutu { $max_dpi } dpi'ye kadar rasterleştirilebilir; 300 dpi normal bir baskı kalitesidir.
plot-dpi = dpi
plot-drawing-no = ÇİZİM No.
plot-drawing-number = Çizim numarası
plot-drawn-by-caps = ÇİZEN
plot-drawn-by = Çizen
plot-e-g-example-gold-project = örn. Örnek Altın Projesi
plot-entered-coordinates = Girilen koordinatlar
plot-export-png = PNG Dışa Aktar...
plot-fit-scale-visible-data = Ölçeği görünen verilere sığdır
plot-grid-interval = Izgara aralığı
plot-landscape = Yatay
plot-lists-visible-surfaces-design-layers = Görünen yüzeyleri ve tasarım katmanlarını renkleriyle listeler.
plot-margin = Kenar boşluğu
plot-margins-leave-no-room-map = Kenar boşlukları harita için yer bırakmıyor
plot-metres-scale-1-scale = metre    Ölçek 1:{ $scale }
plot-mm = mm
plot-north-arrow = Kuzey oku
plot-nothing-visible-draw = Çizilecek görünür bir şey yok
plot-paper = Kağıt
plot-paper-orientation-width-height-mm = { $paper } { $orientation } · { $width } × { $height } mm
plot-paper-size = Kağıt boyutu
plot-pick-interval-reads-roughly-every = Basılı sayfada yaklaşık her 50 mm'de bir okunacak bir aralık seçin.
plot-plan = Plan
plot-scale-must-be-positive = Çizim ölçeği pozitif bir sayı olmalıdır
plot-png-written-sheet-s-exact = PNG, sayfanın tam kağıt boyutunda yazılır ve dpi'sini kaydeder, böylece gerçek ölçekte basılır.
plot-portrait = Dikey
plot-resolution = Çözünürlük
plot-rev = REV
plot-revision = Revizyon
plot-scale = ÖLÇEK
plot-scale-ratio = Ölçek  1:
plot-scale-framing = Ölçek ve çerçeveleme
plot-sheet-furniture = Sayfa donanımı
plot-size-width-height-mm = { $size } ({ $width } × { $height } mm)
plot-subtitle = Alt başlık
plot-title = Başlık
plot-title-block = Başlık bloğu
plot-today = bugün

## Products strings

products-add-initiation = Ateşleme Ekle
products-delay = Gecikme
products-delay-palette = Gecikme Paleti
products-how-long-after-shot-fired = Atış ateşlendikten ne kadar süre sonra bu ağzın turu başlattığı.
products-initiation-name = Ateşleme · { $name }
products-milliseconds-between-one-hole-firing = Bir deliğin ateşlenmesi ile bir sonrakinin ateşlenmesi arasındaki milisaniye.
products-ms = ms
products-no-products = Ürün yok
products-remove = Kaldır
products-update = Güncelle

## Progress strings

progress-percent-done-total = { $percent } ({ $total } içinden { $done })
progress-task-finished = { $task }: Tamamlandı

## Project strings

project-item = Öğe

## Properties strings

properties-adds-view-dependent-rim-highlight = Blok ve malzeme sınırlarına görünüme bağlı bir kenar vurgusu ekler. Bunu kapalı bırakmak hacim çizim işini biraz azaltır.
properties-block-model-downscale = Blok model küçültme
properties-camera = Kamera
properties-camera-clip-planes = Kamera kırpma düzlemleri
properties-cap-while-resizing = Yeniden boyutlandırırken sınırla
properties-dark-mode = Koyu mod
properties-developer = Geliştirici
properties-downscale-rasters = Rasterleri küçült
properties-edit-object = Nesneyi Düzenle...
properties-field-view = Görüş açısı
properties-fps = FPS
properties-frame-counter = Kare sayacı
properties-frame-rate-cap = Kare hızı sınırı
properties-hz = Hz
properties-interface = Arayüz
properties-invert-horizontal = Yatayı ters çevir
properties-invert-vertical = Dikeyi ters çevir
properties-limits-newly-loaded-geotiff-previews = Yeni yüklenen GeoTIFF önizlemelerini en uzun kenarlarında 4096 piksel ile sınırlar. GPU'nun doku sınırına kadar tam çözünürlük kullanmak için devre dışı bırakın; bu daha fazla bellek kullanır.
properties-line-colour = Çizgi rengi
properties-look-sensitivity = Bakış duyarlılığı
properties-max-clip-span = Maks kırpma aralığı
properties-move-layer = Katmana Taşı...
properties-near-clip-limit = Yakın kırpma sınırı
properties-orbit-sensitivity = Yörünge duyarlılığı
properties-panel-chrome = Panel çerçevesi
properties-performance = Performans
properties-plan-mode = Plan Modu
properties-presents-step-display-no-tearing = Ekranla senkronize sunar: yırtılma olmaz ve kare hızını ekran belirler. Kapalıyken kareler çizilir çizilmez sunulur ve aşağıdaki sınır uygulanır.
properties-reflective-block-edges = Yansıtıcı blok kenarları
properties-restore-defaults = Varsayılanları Geri Yükle
properties-show-console = Konsolu göster
properties-shows-live-near-far-projection = Durum çubuğunda canlı yakın ve uzak izdüşüm mesafelerini gösterir.
properties-snap-polling = Yapışma sorgusu
properties-vertical-sync = Dikey eşitleme
properties-world-axis-gizmo = Dünya ekseni göstergesi
properties-zoom-cursor = İmleçe yakınlaştır
properties-zoom-sensitivity = Yakınlaştırma duyarlılığı

## Screenshot strings

screenshot-could-not-encode-viewport-image = Görüntü alanı resmi kodlanamadı: { $error }
screenshot-could-not-map-viewport-screenshot = Görüntü alanı ekran görüntüsü eşlenemedi: { $error }
screenshot-could-not-save-viewport-image = Görüntü alanı resmi { $path } kaydedilemedi: { $error }
screenshot-downloaded-viewport-image-file-name = Görüntü alanı resmi indirildi: { $file_name }
screenshot-saved-viewport-image-path = Görüntü alanı resmi kaydedildi: { $path }
screenshot-viewport-image-download-failed-error = Görüntü alanı resmi indirme işlemi başarısız oldu: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = BVH yüzey indeksi { $index } ağ için aralık dışında; dejenere üçgen ile değiştiriliyor

## State strings

state-above = şuna eşit veya üstünde
state-activate-project = Projeyi Etkinleştir
state-all-open-incline-design-data = Açık tüm Incline Design verileri
state-apply-generated-rings = Oluşturulan halkaları uygula
state-apply-selection = Seçime uygula
state-rotate-by-azimuth-dip = { $azimuth }° azimut, { $dip }° eğim ile
state-rotate-to-azimuth-dip = { $azimuth }° azimut, { $dip }° eğime
state-below = şuna eşit veya altında
state-centre-rotation = Dönüş Merkezi
state-checking-unsaved-work = Kaydedilmemiş çalışma kontrol ediliyor
state-choose-destination = Bir hedef seçin
state-choose-one-more-files = Bir veya daha fazla dosya seçin
state-clear-raster = Rasteri Temizle
state-click-pit-shell-viewport = Görüntü alanında ocak kabuğuna tıklayın.
state-click-pit-stockpile-solid-viewport = Görüntü alanında ocak veya stok sahası katısına tıklayın.
state-click-surface-viewport = Görüntü alanında yüzeye tıklayın.
state-click-topology-viewport = Görüntü alanında topolojiye tıklayın.
state-close-project = Projeyi Kapat
state-colour-drillholes = Sondaj Deliklerini Renklendir
state-copy-objects-layer = Nesneleri Katmana Kopyala
state-count-file-s = { $count } dosya
state-count-object-s-axis-value = { $count } nesne · { $axis } { $value }
state-count-object-s-closed = { $count } nesne · { $closed }
state-count-object-s-layer = { $count } nesne · { $layer }
state-count-object-s-weight = { $count } nesne · { $weight }
state-count-object-s-z-elevation = { $count } nesne · Z { $elevation }
state-create-point-cloud-tin = Nokta Bulutu TIN Oluştur
state-create-project = Proje Oluştur
state-current-project = Geçerli proje
state-cut-topology-pit-shell = Topolojiyi Ocak Kabuğuna Kes
state-cut-triangulation-polyline = Üçgenlemeyi Çoklu Çizgiyle Kes
state-cut-triangulation-z = Üçgenlemeyi Z'ye Göre Kes
state-dark-mode = Koyu Mod
state-detached = Ayrılmış
state-disabled = Devre dışı
state-discard-project-changes = Proje Değişikliklerini At
state-discard-replace-project = At ve Projeyi Değiştir
state-discarding-unsaved-changes = Kaydedilmemiş değişiklikler atılıyor
state-docked = Sabitlenmiş
state-drape-raster = Rasteri Ör
state-drill-pattern = Delme Deseni
state-duplicate-layer = Katmanı Çoğalt
state-east = Doğu
state-enabled = Etkin
state-exit-incline-design = Incline Design'dan Çık
state-export-block-model-csv = Blok Model CSV Dışa Aktar
state-export-layer-dxf = Katmanı DXF'e Dışa Aktar
state-export-omf = OMF Dışa Aktar
state-export-project-dxf = Projeyi DXF'e Dışa Aktar
state-export-triangulation = Üçgenlemeyi Dışa Aktar
state-export-viewport-image = Görüntü Alanı Resmini Dışa Aktar
state-finish-closed-polyline = Kapalı çoklu çizgiyi bitir
state-finish-open-polyline = Açık çoklu çizgiyi bitir
state-fit-extents = Kapsama sığdır
state-fix-release-centre-both-views = Her iki görünümün de etrafında döndüğü merkezi sabitler veya serbest bırakır
state-generate-contours = Konturları Oluştur
state-hidden = Gizli
state-import-drillholes = Sondaj Deliklerini İçe Aktar
state-import-omf = OMF İçe Aktar
state-import-point-cloud = Nokta Bulutu İçe Aktar
state-import-raster = Raster İçe Aktar
state-import-triangulation = Üçgenleme İçe Aktar
state-insert-intersection-points = Kesişim Noktaları Ekle
state-insert-points-elevation = Kotta Noktalar Ekle
state-keep-inside = İçeride tut
state-keep-outside = Dışarıda tut
state-kriged-block-model = Kriging Blok Modeli
state-load-block-model = Blok Model Yükle
state-load-drillholes = Sondaj Deliklerini Yükle
state-load-layer = Katman Yükle
state-load-point-cloud = Nokta Bulutu Yükle
state-load-raster = Raster Yükle
state-load-triangulation = Üçgenleme Yükle
state-locked-count-object-s = { $count } nesne kilitlendi
state-major-minor = Ana { $major } · ara { $minor }
state-move-axis-value = Eksen Değerine Taşı
state-move-objects-layer = Nesneleri Katmana Taşı
state-name-count-holes = { $name } · { $count } delik
state-name-count-object-s = { $name } · { $count } nesne
state-name-z-min-z-max = { $name } · { $z_min } - { $z_max }
state-next-edit = Sonraki düzenleme
state-north = Kuzey
state-open-containing-folder = İçeren klasörü aç
state-open-project = Proje Aç
state-preserve-view-angle = Görünüm açısını koru
state-previous-edit = Önceki düzenleme
state-project-id = Proje { $id }
state-remove-block-model = Blok Modeli Kaldır
state-remove-drillholes = Sondaj Deliklerini Kaldır
state-remove-point-cloud = Nokta Bulutunu Kaldır
state-remove-raster = Rasteri Kaldır
state-remove-triangulation = Üçgenlemeyi Kaldır
state-removed-from-active-triangulation = Etkin üçgenlemeden kaldırıldı
state-removed-from-every-triangulation = Tüm üçgenlemelerden kaldırıldı
state-rename-kind = { $kind } Yeniden Adlandır
state-save-close-project = Projeyi Kaydet ve Kapat
state-save-despite-unsupported-content = Desteklenmeyen içeriğe rağmen kaydet
state-save-project = Projeyi Farklı Kaydet
state-save-replace-project = Kaydet ve Projeyi Değiştir
state-saving-current-project = Geçerli proje kaydediliyor
state-section-name = { $section } bölümü
state-select-layer-objects = Katman Nesnelerini Seç
state-selected-objects = Seçili nesneler
state-selected-polylines = Seçili çoklu çizgiler
state-selected-scene-elements = Seçili sahne öğeleri
state-set-block-model-variable = Blok Model Değişkenini Ayarla
state-set-drillhole-colour-preset = Sondaj Deliği Renk Ön Ayarını Belirle
state-set-entity-lock = Varlık Kilidini Ayarla
state-set-grid = Izgarayı Ayarla
state-set-layer-lock = Katman Kilidini Ayarla
state-set-line-weight = Çizgi Kalınlığını Ayarla
state-set-object-colour = Nesne Rengini Ayarla
state-set-object-fill = Nesne Dolgusunu Ayarla
state-set-point-visibility = Nokta Görünürlüğünü Ayarla
state-set-polyline-closed = Çoklu Çizgiyi Kapalı Yap
state-set-raster-lock = Raster Kilidini Ayarla
state-set-standard-view = Standart Görünümü Ayarla
state-set-topology-wireframes = Topoloji Tel Kafeslerini Ayarla
state-set-triangulation-colour = Üçgenleme Rengini Ayarla
state-show-console = Konsolu Göster
state-show-project = Projeyi Göster
state-shown = Gösteriliyor
state-slice-mode = Kesit Modu
state-slice-preview = Kesit Önizlemesi
state-south = Güney
state-stem-contours = { $stem } Konturları
state-target-new-name = { $target } → "{ $new_name }"
state-trim-above = Üstünü kırp
state-trim-below = Altını kırp
state-trim-triangulation-surface = Üçgenlemeyi Yüzeye Kırp
state-undrape-raster = Raster Örtüsünü Kaldır
state-undrape-rasters = Raster Örtülerini Kaldır
state-unload-block-model = Blok Modeli Kaldır
state-unload-drillholes = Sondaj Deliklerini Kaldır
state-unload-layer = Katmanı Kaldır
state-unload-point-cloud = Nokta Bulutunu Kaldır
state-unload-raster = Rasteri Kaldır
state-unload-triangulation = Üçgenlemeyi Kaldır
state-untitled-project = Adsız proje
state-use-typed-radius = Girilen yarıçapı kullan
state-west = Batı

## Status strings

status-clip-near-far = Kırpma yakın/uzak/Δ: -- / -- / --
status-frame-rate = Kare hızı

## Text strings

text-could-not-build-vector-mesh = { $font } yazı tipi, { $glyph } glifi için vektör ağı oluşturulamadı: { $error }
text-document-text-mesh-exceeded-its = Belge metin ağı u32 indeks aralığını aştı

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = Önce bağlanacak sondaj deliği veri kümesini seçin
tie-in-count-connector-s = { $count } bağlantı
tie-in-delete-tie-ins = Bağlantıları Sil
tie-in-deleted-count-selected-tie-connector = Seçili { $count } bağlantı silindi
tie-in-hole = delik
tie-in-initiation-point-lifted-from-name = Ateşleme noktası { $name } üzerinden kaldırıldı
tie-in-initiation-point-set-name-delay = Ateşleme noktası { $name } üzerinde { $delay } ms olarak ayarlandı
tie-in-select-delay-product-palette-before = Delikleri bağlamadan önce palette bir gecikme ürünü seçin
tie-in-tied-connectors = { $product } ile { $delay } ms'de { $count } bağlantı yapıldı
tie-in-tied-connectors-replacing = { $product } ile { $delay } ms'de { $count } bağlantı yapıldı, { $replaced } değiştirildi

## Toolbar strings

toolbar-fill-type = Dolgu türü

## Toolbars strings

toolbars-auto-bench = Otomatik Basamak
toolbars-bezier-polyline = Bezier Çoklu Çizgisi
toolbars-chamfer-polyline-corners = Çoklu Çizgi Köşelerini Pahla
toolbars-create-text = Metin Oluştur
toolbars-cursor-regular = İmleç: Normal
toolbars-cursor-snap-line = İmleç: Çizgiye Yapış
toolbars-cursor-snap-point = İmleç: Noktaya Yapış
toolbars-cursor-snap-surface = İmleç: Yüzeye Yapış
toolbars-delete-points = Noktaları Sil
toolbars-explode-polyline-lines = Çoklu Çizgiyi Çizgilere Ayır
toolbars-fuse-polylines = Çoklu Çizgileri Kaynaştır
toolbars-measure-distance = Mesafe Ölç
toolbars-new-layer = Yeni Katman
toolbars-split-polyline-points = Çoklu Çizgiyi Noktalarda Böl
toolbars-strike-dip = Doğrultu ve Eğim
toolbars-tool-not-available-section-view = { $tool } - kesit görünümünde kullanılamaz

## Tri strings

tri-sampling-method-help = Uyarlanabilir, düzlem uyum hatası aracılığıyla köşeleri karmaşık arazide yoğunlaştırır; tekdüze bunları eşit şekilde dağıtır. Gelecekte daha fazla yöntem eklenebilir.
tri-adaptive-quadtree = Uyarlanabilir (quadtree)
tri-axis-range = { $axis } aralığı
tri-base-topology-will-receive-pit = Ocak veya stok sahası şeklini alacak taban topolojisi.
tri-boundary-polyline = Sınır çoklu çizgisi
tri-bridge-gaps-help = Yüzey boyunca bundan daha dar boşlukları ve sınır girintilerini köprüler. 0 değeri bile yaklaşık örnekleme hücresi boyutuna kadar boşlukları köprüler; daha büyük değerler daha büyük delikleri doldurur ve sınır girintilerini aşındırır.
tri-budget = Bütçe türü
tri-cancel-pick = Seçimi İptal Et
tri-candidate-detail = Aday detayı
tri-candidate-fine-cells-per-budgeted = Bütçelenen köşe başına aday ince hücre sayısı. Daha yüksek değer, uyarlanabilir örnekleyiciye detay yerleştirmede daha fazla özgürlük tanır ancak oluşturması daha yavaştır.
tri-cap-surface-share-source-points = Yüzeyi kaynak noktaların bir oranıyla veya kesin bir köşe sayısıyla sınırlayın.
tri-choose-input-clicking-loaded-surface = Görüntü alanında yüklü bir yüzeye tıklayarak bu girdiyi seçin
tri-choose-which-side-reference-topology = Ortak XY alanları içinde, referans topolojinin hangi tarafının yüzeyden kaldırılacağını seçin.
tri-clip = Kırp
tri-clip-creates-new-triangulation-name = Kırpma işlemi bu adla yeni bir üçgenleme oluşturur; kaynak yüzey değiştirilmez.
tri-clip-surface-polyline = Yüzeyi Çoklu Çizgiyle Kırp
tri-closed-pit-stockpile-solid-whose = Açıkta kalan sınırı sonuca dahil edilecek kapalı bir ocak veya stok sahası katısı.
tri-create-new-layer-contours-append = Konturlar için yeni bir katman oluşturun veya etkin projede mevcut bir katmana ekleyin.
tri-cut-topology-pit-shell = Topolojiyi Ocak Kabuğuyla Kes
tri-e-g-design-trimmed = örn. tasarim_kirpildi
tri-e-g-mysurf-cut = örn. yuzeyim_kesildi
tri-e-g-mysurf-slice = örn. yuzeyim_kesit
tri-e-g-surface-contour = örn. yuzey_kontur
tri-e-g-topo-cut = örn. topo_kesildi
tri-e-g-topo-pit = örn. topo_ocakli
tri-exact-number-surface-vertices-target = Hedeflenecek kesin yüzey köşesi sayısı. Çok büyük değerler yavaş oluşturulur ve önemli miktarda bellek kullanır.
tri-existing-ground-topology-will-cut = Ocak kabuğu tarafından kesilecek mevcut arazi topolojisi.
tri-fill-holes-up = Şu boyuta kadar delikleri doldur
tri-generate = Oluştur
tri-generate-contour-lines = Kontur Çizgileri Oluştur
tri-generate-upper-surface = Üst Yüzeyi Oluştur
tri-hide-unload-sources = Kaynakları gizle ve kaldır
tri-higher-edge-will-enforced-each = Her çakışmada daha yüksek kenar zorlanacak. Daha alçak çakışan segmentler kırılma çizgisi olarak yok sayılacak ve yüzey bu alanlarda enterpolasyon yapacaktır. Kaynak çoklu çizgiler değişmez.
tri-breaklines-cross = Vurgulanan kırılma çizgisi kenarları planda farklı kotlarda kesişiyor veya çakışıyor. Tek bir arazi yüzeyi ikisini birden takip edemez.
tri-intervals-colours = Aralıklar ve renkler
tri-keep-clipped-topology-included-shape = Kırpılmış topolojiyi ve dahil edilen şekli tek bir varlıkta birleştirmek yerine ayrı üçgenlemeler olarak tutun.
tri-keep-inside-discards-surface-outside = İçeride tut, çoklu çizginin dışındaki yüzeyi atar. Dışarıda tut, yüzeyden çoklu çizgi şeklinde bir delik keser.
tri-keeps-only-surface-within-polyline = Yalnızca çoklu çizgi sınırı içindeki yüzeyi tutar.
tri-keep-surface-relation-help = Yüzeyi, XY kapsamı içinde topoloji ile { $relation } ilişkisinde tutar.
tri-layer-already-exists-select-above = Bu katman zaten mevcut; yukarıdan seçin veya başka bir ad girin.
tri-limit-z-range = Z aralığını sınırla
tri-major = Ana
tri-max-edge-length = Maks kenar uzunluğu
tri-merge = Birleştir
tri-method = Yöntem
tri-min = Min
tri-minimum-maximum-elevations-retained = Çıktı yüzeyinde korunacak minimum ve maksimum kotlar. Minimum, maksimumdan küçük olmalıdır.
tri-minor = Ara
tri-contour-interval-help = Ara, sıradan konturları kontrol eder. Ana, vurgulanan konturları kontrol eder ve en az Ara kadar büyük bir aralık kullanmalıdır.
tri-move-cursor-over-loaded-surface = İmleci yüklü bir yüzeyin üzerine getirin.
tri-slice-output-name-help = Kot ile kırpılmış çıktı yüzeyine atanan ad.
tri-name-assigned-merged-topology-pit = Birleştirilmiş topoloji ve ocak/stok sahası sonucuna atanan ad.
tri-name-assigned-newly-created-contour = Yeni oluşturulan kontur katmanına atanan ad.
tri-reconstruct-output-name-help = Yeniden oluşturulan üçgenlemeye atanan ad.
tri-name-assigned-topology-after-pit = Ocak kabuğu kesildikten sonra topolojiye atanan ad.
tri-name-assigned-trimmed-output-surface = Kırpılmış çıktı yüzeyine atanan ad.
tri-nearby-breakline-vertices-do-not = Yakındaki kırılma çizgisi köşeleri tam olarak aynı konumda buluşmuyor, bu yüzden yüzey üçgenlenemiyor.
tri-new-layer = Yeni katman
tri-new-layer-name = Yeni katman adı
tri-once-merge-succeeds-unload-source = Birleştirme başarılı olduktan sonra, sahnede yalnızca birleştirilmiş sonucun kalması için kaynak topolojiyi ve katıyı kaldırın.
tri-only-loaded-pickable = Yalnızca yüklü üçgenlemeler seçilebilir.
tri-operation = İşlem
tri-output-layer = Çıktı katmanı
tri-percentage = Yüzde
tri-percentage-cloud = Bulutun yüzdesi
tri-pick-from-view = Görünümden Seç
tri-pit-design-surface-only-areas = Ocak tasarım yüzeyi. Kesim için yalnızca topolojinin altına indiği alanlar kullanılır.
tri-pit-shell = Ocak kabuğu
tri-pit-stockpile-solid = Ocak/stok sahası katısı
tri-recommended-weld-retry = Önerilen: Kaynakla ve Yeniden Dene
tri-reconstruct-help = Bir nokta bulutundan üçgenlenmiş bir arazi yüzeyi yeniden oluşturur. Uyarlanabilir örnekleyici, köşe bütçesini arazinin en karmaşık olduğu yerlerde kullanır ve düz alanları seyrek tutar.
tri-reduce-budget-candidate-detail-if = Bilgisayarınızda daha az RAM varsa bütçeyi veya aday detayını azaltın.
tri-reference-topology-help = Diğer yüzeyin nerede kırpılacağını tanımlayan referans topoloji.
tri-reject-reconstructed-triangle-edges = Bu mesafeden uzun yeniden oluşturulmuş üçgen kenarlarını reddedin. Kenar uzunluğu sınırı olmaması için 0 kullanın.
tri-remove-inside-help = Çoklu çizgi sınırı içindeki yüzeyi kaldırır ve geri kalanını tutar.
tri-removes-topology-where-pit-shell = Ocak kabuğunun altına indiği yerlerde topolojiyi kaldırır, böylece kabuk boşluğu doldurur. Kesim çizgisi yüzeyler arasındaki gerçek 3B temas çizgisini takip eder; kabuğun arazinin üzerinde kaldığı bölgelerdeki topoloji korunur.
tri-result = Sonuç
tri-save-two-entities = İki varlık olarak kaydet
tri-select = Seç…
tri-share-source-points-keep-fractions = Korunacak kaynak noktalarının oranı. 0,125% gibi kesirlere izin verilir.
tri-slice-triangulation-z-range = Üçgenlemeyi Z Aralığına Göre Kes
tri-solution-generate-upper-surface = Çözüm: Üst Yüzeyi Oluştur
tri-surface-trim = Kırpılacak Yüzey
tri-target-surface-help = Değiştirilecek yüzey; seçili topoloji olduğu gibi bırakılır.
common-percent-suffix = %
tri-topology = Topoloji
tri-triangulation-failed = Üçgenleme Başarısız Oldu
tri-trim = Kırp
tri-trim-topology = Topolojiye Kırp
tri-uniform-grid = Tekdüze ızgara
tri-up-target-point-count-points = { $point_count } noktadan { $target } tanesine kadarı yüzey köşesi olacak ({ $percent }%).
tri-use-full-surface-elevation-range = Yüzeyin tam kot aralığını kullan
tri-vertex-count = Köşe sayısı
tri-vertices-within-5-cm-xy = XY ve Z'de 5 cm içindeki köşeler bu üçgenleme için tek bir konumu paylaşacak. Bu, oluşturulan yüzeyi yerel olarak 5 cm'ye kadar kaydırabilir; kaynak çoklu çizgiler değişmez.
tri-weld-retry = Kaynakla ve Yeniden Dene
tri-when-enabled-generate-contours-only = Etkinleştirildiğinde, yalnızca belirtilen minimum ve maksimum kotlar arasında kontur oluşturur.

## Ui strings

ui-choose-offset-side = Ofset tarafını seçin
ui-choose-relimit-side = Yeniden sınırlama tarafını seçin
ui-click-circle-centre = Çember merkezine tıklayın
ui-click-closed-polyline-use-blast = Patlatma şekli olarak kullanılacak kapalı bir çoklu çizgiye tıklayın
ui-click-collar-add-edit-initiation = Bir ateşleme noktası eklemek veya düzenlemek için bir ağza tıklayın
ui-click-first-point-slice-line = Kesit çizgisinin ilk noktasına tıklayın
ui-click-first-vertex = İlk köşeye tıklayın
ui-click-perimeter-point-type-radius = Bir çevre noktasına tıklayın veya bir yarıçap girin
ui-click-second-point-slice-line = Kesit çizgisinin ikinci noktasına tıklayın
ui-click-second-vertex = İkinci köşeye tıklayın
ui-click-use-pointer-radius = veya işaretçi yarıçapını kullanmak için tıklayın
ui-could-not-copy-text-browser = Metin tarayıcı panosuna kopyalanamadı: { $error }
ui-dip-horizontal-no-strike = { $dip } (yatay, doğrultu yok)
ui-distance-meters = { $distance } metre
ui-drag-ring-type-azimuth-dip = Bir halkayı sürükleyin veya bir azimut ve eğim girin
ui-each-hole-turns-about-its = her delik kendi ağzı etrafında döner
ui-enter-positive-decimal-radius = Pozitif ondalık bir yarıçap girin
ui-esc-cancels = Esc iptal eder
ui-no-delay-product-tie = Bağlanacak gecikme ürünü yok
ui-press-enter-use-typed-radius = Girilen yarıçapı kullanmak için Enter'a basın
ui-right-click-delay-palette-heading = eklemek için Gecikme Paleti başlığına sağ tıklayın
ui-select-designs = Tasarımları seçin
ui-select-drill-hole = Bir sondaj deliği seçin
ui-select-endpoint-join = Birleştirilecek uç noktayı seçin
ui-select-first-crest-toe-point = İlk tepe/taban noktasını seçin
ui-select-item = Bir öğe seçin
ui-select-line-fuse = Kaynaştırılacak bir çizgi seçin
ui-select-line-polyline = Bir çizgi veya çoklu çizgi seçin
ui-select-line-relimit = Yeniden sınırlanacak çizgiyi seçin
ui-select-next-line-fuse = Kaynaştırılacak sonraki çizgiyi seçin
ui-select-opposite-berm-point = Karşı berm noktasını seçin
ui-select-point = Bir nokta seçin
ui-select-polyline = Bir çoklu çizgi seçin
ui-select-polyline-open-line = Bir çoklu çizgi veya açık çizgi seçin
ui-select-polyline-vertex = Bir çoklu çizgi köşesi seçin
ui-select-second-crest-toe-point = İkinci tepe/taban noktasını seçin
ui-select-second-split-point = İkinci bölme noktasını seçin
ui-select-split-point = Bir bölme noktası seçin
ui-select-topologies = Topolojileri seçin
ui-slice-view = Kesit görünümü
ui-strike-dip = { $strike }° doğrultu · { $dip }
ui-value-dip = { $value }° eğim

## Viewport strings

viewport-all-total-categories-keep-their = { $total } kategorinin tümü rengini korur; yalnızca ilk { $shown } tanesi ayırt edici şekilde çizilir
viewport-axis-maximum = { $axis } maksimum
viewport-axis-minimum = { $axis } minimum
viewport-bar-blast-timeline-placeholder = Patlatma Zaman Çizelgesi [YER TUTUCU]
viewport-bar-burden-relief-heatmap-placeholder = Burden Rahatlama Isı Haritası [YER TUTUCU]
viewport-bar-color = Renk:
viewport-bar-contours-equal-time-placeholder = Eş Zaman Konturları [YER TUTUCU]
viewport-bar-disable-flying-mode = Uçuş Modunu Devre Dışı Bırak
viewport-bar-disable-x-ray-vision = X-Ray Görüşünü Devre Dışı Bırak
viewport-bar-drill-holes = Sondaj Delikleri:
viewport-bar-enable-flying-mode = Uçuş Modunu Etkinleştir
viewport-bar-enable-x-ray-vision = X-Ray Görüşünü Etkinleştir
viewport-bar-exit-slice-view = Kesit Görünümünden Çık
viewport-bar-fill = Dolgu:
viewport-bar-fix-centre-rotation = Dönüş Merkezini Sabitle
viewport-bar-hide-points = Noktaları Gizle
viewport-bar-hide-rl-grid = Kot Izgarasını Gizle
viewport-bar-hide-wireframes = Tel Kafesleri Gizle
viewport-bar-hide-xy-grid = XY Izgarasını Gizle
viewport-bar-release-centre-rotation = Dönüş Merkezini Serbest Bırak
viewport-bar-show-points = Noktaları Göster
viewport-bar-show-rl-grid = Kot Izgarasını Göster
viewport-bar-show-wireframes = Tel Kafesleri Göster
viewport-bar-show-xy-grid = XY Izgarasını Göster
viewport-bar-vertical-slice-view = Dikey Kesit Görünümü
viewport-blank = (boş)
viewport-choose-active-block-model-variable = Etkin blok model değişkenini seçin
viewport-choose-variable = Bir değişken seçin
viewport-click-edit-color-right-click = Düzenlemek için tıklayın; kaldırmak için sağ tıklayın
viewport-click-type-boundary-s-value = Bu sınırın değerini girmek için tıklayın
viewport-colour-mapping = Renk eşleme
viewport-count-categories = { $count } kategori
viewport-count-category = { $count } kategori
viewport-double-click-add-boundary-here = Buraya bir sınır eklemek için çift tıklayın
viewport-drag-move-middle-click-toggles = Taşımak için sürükleyin · Orta tıklama ≤'yi değiştirir
viewport-drag-move-right-click-remove = Taşımak için sürükleyin · Kaldırmak için sağ tıklayın · Orta tıklama ≤'yi değiştirir
viewport-e = D
viewport-edit-category-colour = Bu kategori rengini düzenle
viewport-edit-colour-used-empty-values = Boş değerler için kullanılan rengi düzenle
viewport-empty = (boş)
viewport-empty-hidden = (boş · gizli)
viewport-filter-variables = Değişkenleri filtrele
viewport-navigation-hint = Kaydırmak için orta düğmeyle sürükleyin · Yakınlaştırmak için kaydırın
viewport-navigation-hint-detach = Kaydırmak için orta düğmeyle sürükleyin · Yakınlaştırmak için kaydırın · Ayırmak için tıklayın
viewport-n = K
viewport-no-data-variable = Bu değişken için veri yok
viewport-no-matches = Eşleşme yok
viewport-no-usable-range = (kullanılabilir aralık yok)
viewport-rebuild-variable-s-colours-from = Bu değişkenin renklerini verilerinden yeniden oluştur
viewport-reset = Sıfırla
viewport-restore-full-model-range = Tam model aralığını geri yükle
