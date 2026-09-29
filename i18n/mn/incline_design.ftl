# Incline — Монгол хэл дээрх мессежийн каталог.
#
# Энэ файл дутуу байж болно: дутуу мессежүүд англи хэлнээс авагдана
# (`i18n/en/incline_design.ftl`). `=` тэмдгийн зүүн талд байгаа ID-ууд
# болон `{ $... }` хэлбэрийн аргументын нэрсийг өөрчлөхийг хориглоно —
# зөвхөн тэмдгийн баруун талын текстийг орчуулна.

## Ерөнхий

common-cancel = Цуцлах
common-clear = Цэвэрлэх
common-close = Хаах
common-color = Өнгө
common-fill = Дүүргэлт
common-set = Тохируулах

## Төлөв байдлын мөр

# Төлөв байдлын мөрөн дэх хэлний цэсний гарчиг. Хэл бүрийг хэзээ ч
# орчуулдаггүй: тус бүр өөрийн бичгээр өөрийгөө нэрлэдэг (LanguageChoice-с).
status-language = Хэл

## Цэс — Файл

menu-file = Файл
menu-file-save-project = Төслийг хадгалах
menu-file-save-project-as = Төслийг өөр нэрээр хадгалах...
menu-file-new-project = Шинэ төсөл...
menu-file-open-project = Төсөл нээх...
menu-file-open-recent = Сүүлд нээсэн
menu-file-show-in-explorer = Explorer дээр харуулах
menu-file-show-in-folder = Агуулсан хавтсыг нээх
menu-file-import = Импортлох...
menu-file-export = Экспортлох...
menu-file-export-viewport-image = Харагдах цонхны зургийг экспортлох...
menu-file-export-engineering-drawing = Инженерийн зургийг экспортлох...
menu-file-about = { $app }-ийн тухай...
menu-file-exit = Программаас гарах

## Цэс — Харагдац

menu-view = Харагдац

## Ажлын орчинууд

ws-production = Үйлдвэрлэл
ws-drill-and-blast = Өрөмдлөг ба тэсэлгээ
ws-geology = Геологи
ws-planning = Төлөвлөлт

## Цэсний мөрүүд

ws-menubar-design = Зураг төсөл
ws-menubar-triangulation = Триангуляц
ws-menubar-raster = Растер
ws-menubar-point-cloud = Цэгэн үүл
ws-menubar-block-model = Блокийн загвар
ws-menubar-drillholes = Цооногууд
ws-menubar-active-layer = Давхарга:

## Цэсний мөрийн функцууд

ws-menubar-design-insert-point = Цэг оруулах
ws-menubar-design-insert-point-at-intersection = Огтлолцол дээр
ws-menubar-design-insert-point-at-elevation = Өндөрлөг дээр
ws-menubar-design-move-to = Шилжүүлэх
ws-menubar-design-create-triangulation = Триангуляц үүсгэх

## Нэр өөрчлөх / устгах диалогууд

# { $kind } нь дээрх ws-production-* багцын ажлын орчны нэр үг.
dialog-rename-title = { $kind }-ийг нэр өөрчлөх
dialog-rename-field = Шинэ нэр
dialog-rename-field-hint = Заавал бөглөнө
dialog-rename-submit = Нэр өөрчлөх
dialog-delete-title = { $kind }-ийг устгах
dialog-delete-confirm =
    «{ $name }»-ийг төслөөс устгах уу?
    Энэ үйлдлийг буцаах боломжгүй.
confirm-delete-product =
    «{ $name }» бүтээгдэхүүнийг палитраас устгах уу?
    Энэ үйлдлийг буцаах боломжгүй.

## «Триангуляц үүсгэх» диалог

tri-create-title = Триангуляц үүсгэх
tri-create-type-label = Триангуляцын төрөл
tri-create-type-help =
    Задгай гадаргуу нь рельефийн маягийн хавтгай үүсгэнэ. Хатуу бие нь бүрэн
    хаалттай тор үүсгэх бөгөөд усны нэвтэрхий бус хилийг бүрдүүлж чадах
    оролтын өгөгдөл шаардана.
tri-create-output-name = Гаралтын нэр
tri-create-output-name-help = Үүсгэсэн триангуляцад оноох нэр.
tri-create-output-name-hint = триангуляцын нэр
tri-create-run = Триангуляц хийх

tri-selection-selected = { $summary } сонгогдсон

tri-type-open-surface = Гадаргуу
tri-type-solid-closed = Хатуу бие

# Сонголтын тоймын хэсгүүд, ж: "3 полилиниа, 1 цэг". Нэр үг бүр өөрийн
# тоогоор олонлогжсон тул хоёроос олон олонлогийн хэлбэртэй хэлүүдэд ч
# зөв харагдана.
tri-count-polylines =
    { $count ->
        [one] { $count } полилиниа
       *[other] { $count } полилиниа
    }
tri-count-strings =
    { $count ->
        [one] { $count } шугаман өгөгдөл
       *[other] { $count } шугаман өгөгдөл
    }
tri-count-points =
    { $count ->
        [one] { $count } цэг
       *[other] { $count } цэг
    }
tri-count-texts =
    { $count ->
        [one] { $count } текст объект
       *[other] { $count } текст объект
    }
tri-count-objects =
    { $count ->
        [one] { $count } объект
       *[other] { $count } объект
    }

about-read-full-licence = Бүрэн лицензийг унших ↗
about-source-code = Эх код
about-website = Вэбсайт
about-title = { $app }-ийн тухай
drill-hole-colour-title = Цооногийн өнгө: { $name }
drill-hole-colour-stop = Зогсоол { $index }
properties-restore-defaults = { $heading } тохиргоог үндсэн утга руу нь буцаах

## Динамик интерфейсийн мессежүүд

ui-selected-count = { $count } сонгогдсон
ui-selected-objects = { $count } объект сонгогдсон
ui-selected-polylines = { $count } полилиниа сонгогдсон
ui-invalid-axis-value = { $axis }-д зөв утга оруулна уу.
ui-selection-spans = Сонголт { $min }-с { $max } хүртэл үргэлжилнэ.
confirm-delete-count = Сонгосон { $count } зүйлийг устгахдаа итгэлтэй байна уу?
confirm-delete-layer = «{ $name }» давхаргыг түүн дээрх бүх объектын хамт устгах уу?
    Энэ үйлдлийг буцаах боломжгүй.
plot-preview-pixels = { $width } × { $height } px, { $dpi } dpi
tri-estimated-memory = Тооцоолсон дээд санах ой ~{ $estimate }. { $detail }
block-grid-summary = Тор: { $x } × { $y } × { $z } = { $count } блок
status-selected = Сонгогдсон: { $count }
status-fps = FPS: { $fps }
status-clip = Огтлолын ойр/хол/Δ: { $near } / { $far } / { $delta } м

## Их давтамжтай эх кодын шууд мөрүүд

## Эх кодын шууд мөрүүд

## Нэмэлт эх кодын шууд мөрүүд

explorer-no-rasters = Растер алга
slice-viewport-gestures = дунд товч чирж зөөх · баруун товч чирж тойрох · Shift+хулганы дугуй алхах · W/S давхарга зөөх · Q/E эргүүлэх · Esc гарах

## Эхлэлийн орчны дэлгэрэнгүй

## Хамрах хүрээний шалгалтаар илэрсэн эх кодын шууд мөрүүд

## Дүрслэлийн эхлэлийн онош

## Өрөмдлөг ба тэсэлгээ болон бусад үлдсэн каталогийн бичлэгүүд

color-aci = ACI
color-aci-value = ACI { $index }
color-index = Индекс
color-rgb = RGB
color-opacity = Тунгалагжилт
color-edit = Товшиж өнгийг засах
color-saturation-value = Ханалт ба гэрэлтэлт
color-hue = Өнгөлөг
asset-loading = Хөрөнгийн өгөгдлийг ачаалж байна
asset-unloading = Хөрөнгийн өгөгдлийг буулгаж байна
asset-load-failed = Хөрөнгийн өгөгдлийг ачаалж чадсангүй
asset-unload-failed = Хөрөнгийн өгөгдлийг буулгаж чадсангүй
preferences-title = Тохиргоо
context-text-colour = Текстийн өнгө
context-polylines = Полилиниа
context-points = Цэгүүд
crs-unknown-ellipsoid = Энэ координатын системийн тодорхойлолт дахь "{ $name }" дэлхийн загварыг таньсангүй.
crs-no-ellipsoid = Энэ координатын системийн тодорхойлолт ямар дэлхийн загвар ашиглаж байгааг заагаагүй байна.
crs-unknown-code = EPSG:{ $code } нь координатын системийн бүртгэлд алга.
crs-transform-failed = Координатыг хувиргаж чадсангүй; үр дүн төгсгөлөг байрлал болсонгүй.
crs-no-datum-path = { $from } болон { $to }-ийн лавлагаа системүүдийн (EPSG датум { $source } ба { $target }) хооронд нийтлэгдсэн хувиргалт олдсонгүй. Ямар ч байсан хувиргах нь тодорхойгүй хэмжээгээр буруу байх тул юу ч өөрчлөгдсөнгүй.
crs-unknown-datum = { $from } эсвэл { $to }-ийн лавлагаа системийг тодорхойлж чадахгүй байна, мөн энэ хоёр өөр дэлхийн загвар ашигладаг тул тэдгээрийн хооронд хувиргах нь тодорхойгүй хэмжээгээр буруу байх болно.
ws-survey = Геодези
survey-count-designs = { $count } { $count ->
    [one] зураг төсөл
   *[other] зураг төсөл
  }
survey-count-meshes = { $count } { $count ->
    [one] триангуляц
   *[other] триангуляц
  }
survey-count-models = { $count } { $count ->
    [one] блокийн загвар
   *[other] блокийн загвар
  }
survey-count-clouds = { $count } { $count ->
    [one] цэгэн үүл
   *[other] цэгэн үүл
  }
survey-count-holes = { $count } { $count ->
    [one] цооногийн өгөгдлийн сан
   *[other] цооногийн өгөгдлийн сан
  }
survey-count-rasters = { $count } { $count ->
    [one] растер
   *[other] растер
  }
survey-unsupported = Растерыг энэ хувиргалтаар хувиргах боломжгүй. Тэдгээрийг харагдацад сонгох боломжгүй тул сонголтод юу ч нөлөөлөхгүй.
survey-angle = Z тэнхлэгийг тойрсон эргэлт (цагийн зүүний эсрэг)
survey-scale = Нэгдсэн XYZ масштабын коэффициент
survey-invalid-transform = Эх цэг, өнцөг болон гарсан координатууд төгсгөлөг байх ёстой.
survey-invalid-scale = Масштаб нь төгсгөлөг эерэг тоо байх ёстой бөгөөд түүний урвуу тоо ч төгсгөлөг байх ёстой.
survey-empty-selection = Хувиргах дор хаяж нэг дэмжигдсэн зүйлийг сонгоно уу.
survey-unavailable = Сонгосон зүйл алга байна эсвэл ачаалагдаагүй байна. Хувиргахаас өмнө ачаална уу.
survey-wrong-project = Зөвхөн идэвхтэй төслөөс зураг төсөл сонгоно уу.
survey-name-required = Координатын системийн нэрийг оруулна уу.
survey-working = Сонгосон өгөгдлийг хувиргаж байна…
survey-completed = { $items }-г байгаа газарт нь хувиргалаа. Буцаах нь тэдгээрийг сэргээнэ.
survey-failed = Хувиргалт амжилтгүй боллоо: { $error }
survey-stale = Идэвхтэй төсөл эсвэл эх өгөгдөл өөрчлөгдсөн тул хувиргалтыг хаялаа. Эх өгөгдлийг сонгоод дахин оролдоно уу.
survey-coordinates-menu = Координат
survey-definitions-action = Тодорхойлолтууд…
survey-transform-action = Хувиргах…
survey-definitions-title = Координатын тодорхойлолтууд
survey-transform-title = Координат хувиргах
survey-new-system = Шинэ координатын систем
survey-new-system-name = Координатын систем
survey-set-local = Уурхайн координатын систем болгож тохируулах
survey-delete-system = Координатын системийг устгах
survey-systems-empty = Координатын систем алга
survey-system-section = Уурхайн торны тодорхойлолт
survey-reference-note = Тодорхойлолт бүрийг эсрэг нь бичдэг систем: таны өгөгдлийг импортлох үед аль хэдийн зөөж яваа координат. Түүнд өөрийн параметр байдаггүй. Системийг уурхайн координатын систем болгохын тулд хулганы баруун товчоор товшино уу, эсвэл доорх хоосон орон зай дээр товшиж шинэ систем тодорхойлно уу.
survey-system-name = Нэр
survey-reference-system = Лавлагаа систем
survey-reference-origin = Мэдэгдэж буй цэг — лавлагаа системийн координат
survey-system-origin = Ижил цэг — системийн координат
survey-angle-help = Дээрээс харахад лавлагаа X тэнхлэгээс лавлагаа Y тэнхлэг рүү цагийн зүүний эсрэг чиглэлээр.
survey-scale-help = Лавлагаа системээс энэ систем рүү шилжих нэгдсэн XYZ масштаб. Хэмжээг хадгалахын тулд 1 ашиглана уу.
survey-close = Хаах
survey-from = Эх
survey-to = Хүрэх
survey-transform-button = Хувиргах
survey-swap = Сэлгэх
survey-drape-note = Дараасан зураг хувиргасан гадаргуугаас хасагдах бөгөөд дахин дараах шаардлагатай.
survey-needs-grid-block-model = Блокийн загвар бол эсийн тогтмол тор бөгөөд проекц эсвэл лавлагаа системийн өөрчлөлт энэ тогтмол байдлыг хадгалдаггүй. Үүнийг хувиргах гэдэг нь эс бүрийг шинэ тор руу дахин дээж авч, агуулж буй утгуудаа алдана гэсэн үг тул өөрчлөлгүй орхилоо.
survey-needs-grid-raster = Растер нь дэлхий дээр аффин зураглалаар байрладаг бөгөөд үүнийг проекц эсвэл лавлагаа системийн өөрчлөлт хадгалж чадахгүй. Үүнийг хувиргах гэдэг нь зургийг дахин дээж авна гэсэн үг тул өөрчлөлгүй орхилоо.
survey-conversion-exact = Яг таг: зөвхөн торны өөрчлөлт, дахин проекцлолгүй.
survey-conversion-accuracy = Заасан нарийвчлал { $accuracy } м.
survey-kind = Төрөл
survey-axis-names = Тэнхлэгийн нэрс
survey-axis-help = Хэрэв X, Y, Z биш бол энэ систем өөрийн тэнхлэгүүдийг юу гэж нэрлэдэг вэ — уурхайн торны хувьд "E", "N", "RL" гэх мэт. Энэ нь координат харуулах бүх газарт хэрэглэгддэг ч зөвхөн энэ нь уурхайн координатын систем байх үед л хэрэгжинэ. Гурвыг нь бүгдийг нь эсвэл алийг нь ч бүү нэрлэ.
survey-kind-registry-short = Бүртгэлийн систем
survey-kind-grid-short = Өөр систем дээрх тор
survey-registry-search = Хайх
survey-registry-hint = Нэр эсвэл EPSG код, ж: "mga zone 56"
survey-registry-none = Бүх үгтэй тохирох зүйл бүртгэлд алга.
survey-parent = Тодорхойлогдсон эсрэг
survey-parent-origin = Мэдэгдэж буй цэг — эцэг системийн координат
survey-pick-registry = Системийг хайж, үр дүнгээс сонгоно уу.
survey-pick-parent = Энэ торыг ямар системийн эсрэг тодорхойлохыг сонгоно уу.
survey-pick-system = Систем сонгоно уу
survey-pick-systems = Хувиргах эх системийг болон хүрэх системийг сонгоно уу.
survey-no-selection = Зүүн талаас координатын систем сонгох эсвэл нэмэхийн тулд доорх хоосон орон зайг хулганы баруун товчоор товшино уу.
survey-kind-grid = { $parent } дээрх тор
survey-system-in-use = "{ $name }"-г устгах боломжгүй: үүний эсрэг { $dependants } { $dependants ->
    [one] систем тодорхойлогдсон байна
   *[other] систем тодорхойлогдсон байна
  }. Эхлээд тэдгээрийг өөр газар руу чиглүүлнэ үү.
survey-system-cycle = "{ $name }" нь өөрийнхөө эсрэг, шууд эсвэл эцэг системүүдээрээ дамжуулан тодорхойлогдсон байна.
survey-system-missing = Тэр координатын систем цаашид байхгүй байна. Өөр тодорхойлолт сонгоно уу.
survey-same-system = Өөр өөр эх ба хүрэх систем сонгоно уу.
survey-name-exists = Ийм нэртэй координатын систем аль хэдийн байна. Засварлахын тулд түүнийг сонгох эсвэл өөр нэр сонгоно уу.

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins, and Incline Design contributors. Permission is hereby granted, free of charge, to any person obtaining a copy of this software to deal in it without restriction, subject to the conditions of the MIT License.

    Incline Design is provided "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, including but not limited to the warranties of MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE and NONINFRINGEMENT.
about-free-open-source-mine-design = Уул уурхайн үнэгүй нээлттэй эх зураг төсөл
about-licensed-under-mit-license = MIT лицензийн дагуу лицензлэгдсэн

## App strings

app-activated-browser-project-name = '{ $name }' хөтчийн төслийг идэвхжүүллээ.
app-browser-project-deletion-failed-erro = Хөтчийн төслийг устгах амжилтгүй боллоо: { $error }
app-browser-project-no-longer-exists = Тэр хөтчийн төсөл цаашид байхгүй болжээ
app-browser-save-failed-error = Хөтчийн хадгалалт амжилтгүй боллоо: { $error }
app-could-not-activate-browser-project = Хөтчийн төслийг идэвхжүүлж чадсангүй: { $error }
app-could-not-delete-browser-project = Хөтчийн төслийг устгаж чадсангүй: { $error }
app-could-not-load-browser-project = Хөтчийн төслийг ачаалж чадсангүй: { $error }
app-could-not-restore-browser-project = Хөтчийн төслийг сэргээж чадсангүй: { $error }
app-deleted-browser-project = Хөтчийн төслийг устгалаа
app-failed-create-window-error = Цонхыг үүсгэж чадсангүй: { $error }
app-failed-create-window-icon-error = Цонхны дүрсийг үүсгэж чадсангүй: { $error }
app-failed-detach-top-down-preview = Дээрээс харах урьдчилан харагдацыг салгаж чадсангүй: { $error }
app-failed-initialize-graphics-error = Дүрслэлийг эхлүүлж чадсангүй: { $error }
app-failed-load-browser-preferences-erro = Хөтчийн тохиргоог ачаалж чадсангүй: { $error }
app-failed-load-config-file-error = Тохиргооны файлыг ачаалж чадсангүй: { $error }
app-failed-load-session-file-error = Сешний файлыг ачаалж чадсангүй: { $error }
app-failed-rasterize-window-icon-error = Цонхны дүрсийг растержуулж чадсангүй: { $error }
app-failed-save-browser-session-error = Хөтчийн сешнийг хадгалж чадсангүй: { $error }
app-failed-save-session-error = Сешнийг хадгалж чадсангүй: { $error }
app-saved-name-browser-storage = '{ $name }'-г хөтчийн санах ойд хадгаллаа

## Block strings

block-model-between = Хооронд
block-model-block-grid = Блокийн тор
block-model-block-size = Блокийн хэмжээ
block-model-choose-numeric-variable = Тоон хувьсагч сонгох
block-model-choose-numeric-variables = Тоон хувьсагч сонгох
block-model-count-variables-selected = { $count } хувьсагч сонгогдсон
block-model-estimate-variables = Тооцоолох хувьсагчид
block-model-full-x-y-z-dimensions = Блок бүрийн бүрэн X, Y, Z хэмжээс. Жижиг блок нарийвчлал, тооцооллын хугацаа, санах ойн хэрэглээг нэмэгдүүлнэ.
block-model-grid-bounds-block-sizes-invalid = Торны хил хязгаар эсвэл блокийн хэмжээ буруу байна.
block-model-lower-x-y-z-edges = Блокийн загварын эзэлхүүний доод X, Y, Z ирмэг. Блокийн төв энэ хязгаараас нэг блокийн хагасын зайд эхэлнэ.
block-model-maximum = Дээд
block-model-maximum-nearest-samples-used-each = Блок бүрт ашиглах хамгийн ойрын дээд түүврийн тоо. Бага утга хурдан ажиллана; өндөр утга тооцооллыг гөлгөрүүлж, тооцооллын хугацааг нэмэгдүүлж болно.
block-model-maximum-samples = Дээд түүврийн тоо
block-model-minimum = Доод
block-model-minimum-nearby-samples-required-esti = Блок тооцоолоход шаардагдах ойролцоох доод түүврийн тоо. Хайлтын радиус дотор энэ тооноос цөөн түүвэртэй блок хоосон үлдэнэ.
block-model-minimum-samples = Доод түүврийн тоо
block-model-nugget = Наггет
block-model-numeric-interval-fields-interpolate = Интерполяцлах тоон интервалын талбарууд. Сонгосон талбар бүр блокийн загварын нэг хувьсагч болно.
block-model-ordinary-kriging-estimates-numeric-d = Энгийн Кригинг бөмбөрцөг вариограм ашиглан блок бүрийн төвд цооногийн тоон интервалыг тооцоолно.
block-model-partial-sill = Хэсэгчилсэн доод хязгаар
block-model-range-search-radius = Мужийн / хайлтын радиус
block-model-samples-farther-than-distance-exclud = Энэ зайнаас хол байгаа түүврийг хасна; коварианс энэ мужид тэгтэй тэнцэнэ.
block-model-select-all = Бүгдийг сонгох
block-model-spatially-correlated-variance-contri = Бөмбөрцөг загвараас гарах орон зайн хамааралтай дисперс. Наггеттэй хамт энэ нь тэг зайд коварианс тогтооно.
block-model-spherical-variogram-search = Бөмбөрцөг вариограм ба хайлт
block-model-threshold = <= босго
block-model-threshold-2 = >= босго
block-model-threshold-min = Босго / доод
block-model-upper-x-y-z-extent = Хамрах дээд X, Y, Z хэмжээ. Хамрах муж блокийн хэмжээнд яг тэнцүү үрждэггүй бол сүүлчийн блок энэ хэмжээнээс давж болно.
block-model-variable = Хувьсагч
block-model-variance-effectively-zero-separation = Хэмжилтийн алдаа эсвэл түүврийн масштабаас доогуур хэлбэлзлээс үүдэлтэй, бодит байдал дээр тэг зайд гарах дисперс. Наггет эффект хэрэггүй бол тэгийг ашиглана уу.
block-model-volume-cache-block-volume-usage-feedback-readback = Блокийн эзлэхүүний ашиглалтын мэдээллийн уншилт холболтоо тасалдлаа
block-model-volume-cache-block-volume-usage-feedback-readback-2 = Блокийн эзлэхүүний ашиглалтын мэдээллийн уншилт амжилтгүй боллоо: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z

## Canvas strings

canvas-not-selectable-choose-closed-polylin = Сонгох боломжгүй | Хаалттай полилиниа сонгоно уу
canvas-polyline-layer-layer-count-vertices = Полилиниа | Давхарга: { $layer } | { $count } орой цэг
canvas-surface-name = Гадаргуу | { $name }
canvas-trimmed = Тайрсан

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = { $object_id } объектоос уступ-берм үүсгэлээ
cmd-bezier-replaced-polyline-span-first-last = { $first }→{ $last } полилиниагийн хэсгийг { $count } дундын түүврийн цэгээр сольсон
cmd-bezier-vertices-first-last = { $first }-с { $last } хүртэлх орой цэг
cmd-block-model-block-model-loader-disconnected-path = { $path }-н блокийн загвар ачаалагч холболтоо тасалдлаа
cmd-block-model-block-model-path-has-count = { $path } блокийн загварт уншигдахгүй дэмжигдээгүй төрлийн { $count } хувьсагч бий: { $names }
cmd-block-model-building-ore-mesh = Хүдрийн тор бүтээж байна…
cmd-block-model-could-not-create-block-model = Блокийн загвар үүсгэж чадсангүй: { $error }
cmd-block-model-could-not-decode-block-model = Блокийн загварын '{ $variable }' өнгийн хувьсагчийг тайлбарлаж чадсангүй: { $error }
cmd-block-model-created-block-model-name-ordinary = Энгийн Кригингээр '{ $name }' блокийн загварыг үүсгэлээ
cmd-block-model-failed-load-block-model-error = Блокийн загварыг ачаалж чадсангүй: { $error }
cmd-block-model-generated-ore-mesh-from-block = '{ $name }' блокийн загвараас хүдрийн тор үүсгэлээ
cmd-block-model-imported-block-model-source-path = Импортолсон блокийн загварын эх сурвалж { $path }
cmd-block-model-loaded-block-model-name-blocks = '{ $name }' блокийн загварыг ачааллаа: { $blocks } блок ({ $renderable } дүрслэгдэх), тор { $dimx }x{ $dimy }x{ $dimz }, { $variables } хувьсагч
cmd-block-model-loading-name = { $name }-г ачаалж байна
cmd-block-model-loading-name-2 = { $name }-г ачаалж байна…
cmd-chamfer-chamfered-corner-corner-radius-radiu = { $corner } буланг { $radius } радиус, { $segments } сегментээр фасклав
cmd-chamfer-radius-radius = Радиус { $radius }
cmd-commands-clipped = Огтолсон
cmd-commands-command-failed-error = Тушаал амжилтгүй боллоо: { $error }
cmd-commands-select-one-more-objects-before = { $axis }-ыг тохируулахаас өмнө нэг буюу түүнээс олон объект сонгоно уу
cmd-commands-sliced = Огтлосон (Slice)
cmd-contours-contour-generation-failed-error = Изолиниа үүсгэлт амжилтгүй боллоо: { $error }
cmd-contours-contours-name-were-discarded-layer = '{ $name }'-н изолиниа хаягдлаа: '{ $layer_name }' давхарга аль хэдийн байна
cmd-contours-contours-name-were-discarded-project = '{ $name }'-н изолиниа хаягдлаа: төсөл хаагдсан байна
cmd-contours-contours-name-were-discarded-selecte = '{ $name }'-н изолиниа хаягдлаа: сонгосон гаралтын давхарга устгагдсан байна
cmd-contours-generated-line-count-contour-polylin = '{ $name }' триангуляцын { $line_count } изолиниа полилиниаг '{ $layer_name }' давхаргад үүсгэлээ
cmd-creation-assembled-assembled-count-closed-bou = Хуваагдсан задгай шугамуудаас { $assembled_count } хаалттай хилийн цагираг угсарлаа
cmd-creation-created-triangulation-from-boundary = { $boundary_count } хилийн цагираг ба { $constraint_count } задгай хязгаарлалтаас триангуляц үүсгэлээ, гадаргуугийн төрөл { $surface_type }
cmd-creation-creating-triangulation = Триангуляц үүсгэж байна…
cmd-creation-generate-upper-surface-ignored-count = Дээд гадаргуу үүсгэх: { $count } доод зөрчилдсөн эвдрэлийн шугамын сегментийг үл хэрэгсэв; эх объект өөрчлөгдөхгүй
cmd-creation-ignored-rejected-non-polyline-degene = Триангуляцын явцад полилиниа бус эсвэл доройтсон { $rejected } объектыг үл хэрэгсэв
cmd-creation-weld-retry-moved-coarse-welded = Гагнаад дахин оролдох: { $coarse_welded } орой цэгийг хамтын байрлал руу шилжүүлэв (хамгийн ихдээ { $coarse_weld_tol } м); эх объект өөрчлөгдөхгүй
cmd-creation-welded-welded-breakline-vertex-verti = Хязгаарын дотор давхцсан { $welded } эвдрэлийн шугамын орой цэгийг гагналаа
cmd-cuts-clipped-surface-name-polyline-mode = '{ $name }' гадаргууг полилиниагаар огтолов ({ $mode })
cmd-cuts-clipping-surface-polyline = Гадаргууг полилиниагаар огтолж байна…
cmd-cuts-cut-topology-name-pit-shell = '{ $name }' топологийг карьерийн бүрхүүлээр тайрлаа
cmd-cuts-cut-triangulation-name-z-band = '{ $name }' триангуляцыг Z зурвасаар [{ $min }, { $max }] тайрлаа
cmd-cuts-cutting-topology-pit-shell = Топологийг карьерийн бүрхүүлээр тайрж байна…
cmd-cuts-cutting-triangulation-z = Триангуляцыг Z-ээр тайрж байна…
cmd-cuts-ignored-count-vertical-degenerate-re = XY талбайгүй { $count } босоо эсвэл доройтсон лавлагаа топологийн талыг үл хэрэгсэв
cmd-cuts-site-skipped-constraint-from-x = { $site }: ({ $from_x }, { $from_y }) -> ({ $to_x }, { $to_y }) хязгаарлалтыг алгасав, триангулятор хуваах боломжгүй байлаа
cmd-cuts-site-skipped-skipped-near-degenerate = { $site }: доройтолд ойрхон { $skipped } хязгаарлалтын ирмэгийг алгасав; тэдгээрийн ойролцоох огтлолын хил үсний зузаанаар зөрсөн байж болно
cmd-cuts-trimmed-surface-surface-topology-top = '{ $surface }' гадаргууг '{ $topology }' топологиор тайрлаа ({ $mode })
cmd-cuts-trimming-surface-topology = Гадаргууг топологиор тайрж байна…
cmd-drape-draped-intersected-vertices-changed = { $intersected } орой цэгийг дараалаа; { $changed } нь өндөрлөгөө өөрчиллөө
cmd-drape-none-selected-design-vertices-inters = Сонгосон зураг төслийн орой цэгүүдийн аль нь ч сонгосон топологитой огтлолцохгүй байна
cmd-drape-objects-changed-object-s-changed = { $objects } объект өөрчлөгдсөн · { $intersected }-с { $changed } огтлолцсон орой цэг шилжсэн
cmd-drape-select-one-more-design-objects = Дараах нэг буюу түүнээс олон зураг төслийн объектыг сонгоно уу
cmd-drape-select-one-more-topologies-drape = Дараах топологи болгон нэг буюу түүнээс олныг сонгоно уу
cmd-drape-selected-topologies-no-longer-loaded = Сонгосон топологи цаашид ачаалагдаагүй байна
cmd-drill-hole-drill-pattern-too-large-contains = Өрмийн сүлжээ хэт том эсвэл буруу амсрын координат агуулж байна
cmd-drill-hole-enter-name-drill-pattern = Өрмийн сүлжээний нэрийг оруулна уу
cmd-drill-hole-failed-load-drillholes-error = Цооногийг ачаалж чадсангүй: { $error }
cmd-drill-hole-hole-depth-must-greater-than = Цооногийн гүн тэгээс их байх ёстой
cmd-drill-hole-hole-diameter-must-greater-than = Цооногийн диаметр тэгээс их байх ёстой
cmd-drill-hole-loaded-drillhole-dataset-name-holes = '{ $name }' цооногийн өгөгдлийн багцыг ачааллаа: { $holes } цооног, { $fields } өнгөний талбар
cmd-drill-hole-pattern-contains-no-holes = Сүлжээнд цооног алга
cmd-explode-count-line-s = { $count } шугам
cmd-explode-explode-polyline = Полилиниаг задлах
cmd-explode-exploded-polyline-into-count-line = Полилиниаг { $count } шугамын сегмент болгож задлав
cmd-file-block-model-csv-encoding-failed = Блокийн загварын CSV кодчилол амжилтгүй боллоо: { $error }
cmd-file-block-model-csv-export-failed = Блокийн загварын CSV экспорт амжилтгүй боллоо: { $error }
cmd-file-browser-recovery-files-unavailable-s = Хөтчийн сэргээх файл боломжгүй байна; хадгалагдсан төслүүд IndexedDB-д хэвээр байна
cmd-file-closed-project-runtime-id-runtime = { $runtime_id } ажиллагааны ID-тай төслийг хаалаа
cmd-file-could-not-create-new-project = Шинэ төсөл үүсгэж чадсангүй: { $error }
cmd-file-could-not-finish-pending-project = Хүлээгдэж буй төслийн үйлдлийг дуусгаж чадсангүй: { $error }
cmd-file-could-not-finish-saving-before = Гарахаас өмнө хадгалалтыг дуусгаж чадсангүй: { $error }
cmd-file-could-not-open-browser-project = Хөтчийн төслийг нээж чадсангүй: { $error }
cmd-file-could-not-open-path-error = { $path }-ыг нээж чадсангүй: { $error }
cmd-file-could-not-read-selected-file = Сонгосон файлыг уншиж чадсангүй: { $error }
cmd-file-could-not-reload-layer-from = Давхаргыг дискнээс дахин ачаалж чадсангүй: { $error }
cmd-file-could-not-reload-project-from = Төслийг дискнээс дахин ачаалж чадсангүй: { $error }
cmd-file-could-not-remove-browser-project = Хөтчийн төслийг устгаж чадсангүй: { $error }
cmd-file-could-not-restore-layer-from = Давхаргыг төслөөс сэргээж чадсангүй: { $error }
cmd-file-could-not-snapshot-dirty-project = Сэргээлтэд зориулж өөрчлөгдсөн төслийн агшинг авч чадсангүй: { $error }
cmd-file-could-not-start-browser-export = Хөтчийн экспортыг эхлүүлж чадсангүй: { $error }
cmd-file-could-not-write-recovery-copies = Сэргээх хуулбарыг бичиж чадсангүй: { $error }
cmd-file-created-new-browser-project = Шинэ хөтчийн төсөл үүсгэлээ
cmd-file-created-new-project = Шинэ төсөл үүсгэлээ
cmd-file-description-download-failed-error = { $description } татах явдал амжилтгүй боллоо: { $error }
cmd-file-discard-was-cancelled-because-projec = OMF дахин ачаалж байх зуур төсөл өөрчлөгдсөн тул үл хэрэгсэх үйлдэл цуцлагдлаа
cmd-file-discarded-changes-layer-target-name = '{ $target_name }' давхаргын өөрчлөлтийг үл хэрэгсэв
cmd-file-discarded-changes-reloaded-path = Өөрчлөлтийг үл хэрэгсэв: { $path }-г дахин ачааллаа
cmd-file-downloaded-description-file-name = { $description } татагдлаа: { $file_name }
cmd-file-dxf-download-encoding-failed-error = DXF татаж авах кодчилол амжилтгүй боллоо: { $error }
cmd-file-dxf-import-failed-error = DXF импорт амжилтгүй боллоо: { $error }
cmd-file-encoding-block-model-csv-download = Блокийн загварын CSV татаж авалтыг кодчилж байна…
cmd-file-encoding-dxf-download = DXF татаж авалтыг кодчилж байна…
cmd-file-encoding-triangulation-download = Триангуляц татаж авалтыг кодчилж байна…
cmd-file-exit-deferred-until-background-expor = Далд экспорт дуустал гарахыг хойшлуулав
cmd-file-exit-requested-no-unsaved-changes = Хадгалаагүй өөрчлөлт байхгүй тул гарахыг хүслээ
cmd-file-exported-block-model-csv-path = Блокийн загварын CSV-г { $path } руу экспортоллоо
cmd-file-exported-description-dxf-path = { $description }-г DXF болгон экспортоллоо: { $path }
cmd-file-exported-triangulation-name-path = '{ $name }' триангуляцыг { $path } руу экспортоллоо
cmd-file-exporting-name = { $name }-г экспортолж байна…
cmd-file-exporting-triangulation-name-path = '{ $name }' триангуляцыг { $path } руу экспортолж байна
cmd-file-fatal-renderer-failure-reason = Дүрслэгчийн ноцтой алдаа: { $reason }
cmd-file-file-dialog-action-failed-msg = Файлын харилцах цонхны үйлдэл амжилтгүй боллоо: { $msg }
cmd-file-imported-added-object-s-from = { $name }-с { $added } объект импортлож нэмлээ
cmd-file-imported-total-dxf-object-s = { $total } DXF объектыг импортоллоо
cmd-file-layer-discard-was-cancelled-because = Төсөл дахин ачаалагдаж байх зуур төсөл өөрчлөгдсөн тул давхаргыг үл хэрэгсэх үйлдэл цуцлагдлаа
cmd-file-no-recovery-directory-available-erro = Сэргээх хавтас байхгүй байна: { $error }
cmd-file-no-unsaved-project-content-nothing = Хадгалаагүй төслийн агуулга алга; сэргээх зүйл алга
cmd-file-parsing-browser-dxf-import = Хөтчийн DXF импортыг задалж байна…
cmd-file-parsing-dxf-import = DXF импортыг задалж байна…
cmd-file-project-will-close-after-its = Одоогийн хадгалалт дуусмагц төсөл хаагдана
cmd-file-project-will-close-after-its-2 = Одоогийн хадгалалт дуусмагц төсөл хаагдана
cmd-file-queued-count-triangulation-file-s = Импортлохоор { $count } триангуляцын файлыг дараалалд оруулав
cmd-file-recovery-copies-path-reopen-them = Сэргээх хуулбарууд { $path }-д байна; дахин эхлүүлсний дараа тэдгээрийг нээнэ үү
cmd-file-recovery-copy-failed-error = Сэргээх хуулбар амжилтгүй боллоо: { $error }
cmd-file-recovery-copy-failed-failure = Сэргээх хуулбар амжилтгүй боллоо: { $failure }
cmd-file-recovery-copy-written-path = Сэргээх хуулбар бичигдлээ: { $path }
cmd-file-reverting-layer = Давхаргыг буцааж байна…
cmd-file-reverting-project = Төслийг буцааж байна…
cmd-file-save-failed-message = Хадгалах явдал амжилтгүй боллоо: { $message }
cmd-file-save-worker-ended-without-result = Хадгалах ажлын процесс үр дүнгүй дуусав
cmd-file-saved-project-path = Төслийг дараах нэрээр хадгаллаа: { $path }
cmd-file-saved-project-path-2 = Төслийг хадгаллаа: { $path }
cmd-file-selected-block-model-no-longer = Сонгосон блокийн загвар цаашид ачаалагдаагүй байна
cmd-file-switching-project = Төслийг сольж байна…
cmd-file-triangulation-download-encoding-fail = Триангуляц татаж авах кодчилол амжилтгүй боллоо: { $error }
cmd-file-user-chose-exit-without-saving = Хэрэглэгч хадгалахгүйгээр гарахыг сонгов
cmd-file-user-requested-exit-project-export = Хэрэглэгч гарахыг хүслээ (төслийн экспорт эсвэл хадгалаагүй ажлыг батлах шаардлагатай)
cmd-file-viewport = Харагдах цонх
cmd-file-wait-current-project-save-finish = Одоогийн төслийн хадгалалт дуусахыг хүлээнэ үү
cmd-file-wait-current-project-switch-finish = Одоогийн төслийн шилжилт дуусахыг хүлээнэ үү
cmd-file-wait-project-operation-finish-before = Өөрчлөлтийг үл хэрэгсэхээс өмнө төслийн үйлдэл дуусахыг хүлээнэ үү
cmd-file-wait-project-revert-finish-before = Хадгалахаас өмнө төслийн буцаалт дуусахыг хүлээнэ үү
cmd-fuse-closed-polyline = Хаалттай полилиниа
cmd-fuse-count-source-line-s = { $count } эх шугам
cmd-fuse-created-shape-object-id-vertices = { $sources } эх шугамаас { $vertices } орой цэгтэй { $shape } { $object_id }-г үүсгэлээ
cmd-fuse-fuse-click-did-not-hit = Нэгтгэх: товшилт ямар ч объектод тусаагүй (заагчийн доор юу ч байхгүй)
cmd-fuse-fuse-click-was-not-close = Нэгтгэх: товшилт сонгосон шугамын аль ч үзүүрт хангалттай ойрхон биш байна
cmd-fuse-fuse-clicked-object-object-id = Нэгтгэх: товшсон { $object_id } объект хаалттай полилиниа байна, нэгтгэх нь зөвхөн задгай полилиниад ажиллана
cmd-fuse-fuse-clicked-object-object-id-2 = Нэгтгэх: товшсон { $object_id } объект задгай полилиниа биш (энэ нь { $kind })
cmd-fuse-fuse-clicked-object-object-id-3 = Нэгтгэх: товшсон { $object_id } объект цаашид байхгүй болжээ
cmd-fuse-fuse-clicked-polyline-object-id = Нэгтгэх: товшсон { $object_id } полилиниад ердөө { $count } орой цэг байна, дор хаяж 2 хэрэгтэй
cmd-fuse-fuse-endpoint-marker-marker-index = Нэгтгэх: үзүүрийн { $marker_index } тэмдэг цаашид байхгүй болжээ
cmd-fuse-fuse-line-needs-least-3 = Нэгтгэх: полилиниа болгож хаахад дор хаяж 3 өөр орой цэг хэрэгтэй (одоо { $count })
cmd-fuse-fuse-lines = Шугамуудыг нэгтгэх
cmd-fuse-fuse-need-least-2-segments = Нэгтгэх: батлахын тулд дор хаяж 2 сегмент хэрэгтэй (одоо { $count })
cmd-fuse-fuse-no-active-layer-place = Нэгтгэх: нэгтгэсэн шугамыг байрлуулах идэвхтэй давхарга алга
cmd-fuse-fuse-no-active-project-cannot = Нэгтгэх: идэвхтэй төсөл байхгүй тул батлах боломжгүй
cmd-fuse-fuse-no-source-line-close = Нэгтгэх: полилиниа болгож хаах эх шугам алга
cmd-fuse-fuse-object-awaiting-id-no = Нэгтгэх: { $awaiting_id } объект цаашид зөв полилиниа биш болжээ
cmd-fuse-fuse-object-object-id-already = Нэгтгэх: { $object_id } объект аль хэдийн нэгтгэлийн гинжинд орсон байна, өөр шугам товшино уу
cmd-fuse-fuse-result-has-too-few = Нэгтгэх: үр дүнд орой цэг хэт цөөн байна ({ $count }), таслав
cmd-fuse-fuse-segment-object-object-id = Нэгтгэх: { $object_id } сегмент объект цаашид зөв полилиниа биш болжээ, таслав
cmd-fuse-fuse-source-object-object-id = Нэгтгэх: эх { $object_id } объект цаашид зөв задгай полилиниа биш болжээ
cmd-fuse-fuse-source-object-object-id-2 = Нэгтгэх: эх { $object_id } объект цаашид байхгүй болжээ
cmd-fuse-open-polyline = Задгай полилиниа
cmd-include-include-failed-message = Оруулах явдал амжилтгүй боллоо: { $message }
cmd-include-included-solid-shape-name-topology = '{ $shape_name }' хатуу биеийг '{ $topology_name }' топологид оруулав ({ $retained } топологийн тал хадгалж, { $skipped } хаах таг талыг алгаслаа)
cmd-include-including-pit-stockpile-solid = Карьер/овоолгын хатуу биеийг оруулж байна…
cmd-insert-point-count-operation-point-s = { $count } { $operation } цэг
cmd-insert-point-insert-point-elevation-requires-fini = Өндөрлөг дээр цэг оруулахад тодорхой хязгаартай өндөрлөг шаардлагатай
cmd-insert-point-insert-points = Цэгүүд оруулах
cmd-insert-point-inserted-count-operation-point-s = { $count } { $operation } цэг оруулав
cmd-insert-point-intersection = Огтлолцол
cmd-insert-point-no-new-operation-points-were = Шинэ { $operation } цэг олдсонгүй
cmd-insert-point-select-least-two-polylines-before = Огтлолцлын цэг оруулахаас өмнө дор хаяж хоёр полилиниа сонгоно уу
cmd-insert-point-select-one-more-polylines-before = Өндөрлөг дээр цэг оруулахаас өмнө нэг буюу түүнээс олон полилиниа сонгоно уу
cmd-layer-created-layer-name = '{ $name }' давхаргыг үүсгэлээ
cmd-layer-deleted-layer-layer-id-all = { $layer_id } давхаргыг (бүх объектын хамт) устгалаа
cmd-layer-duplicated-layer-duplicate-name = '{ $duplicate_name }' давхаргыг хувиллаа
cmd-layer-locked = Түгжигдсэн
cmd-layer-name-copy = { $name } хуулбар
cmd-layer-selected-count-object-s-layer = { $layer_id } давхаргад { $count } объект сонгов
cmd-layer-state-layer-name = '{ $name }' давхарга { $state }
cmd-layer-unlocked = Түгжээгүй
cmd-move-tool-applied-move-delta-delta-count = Шилжилт ({ $delta })-г { $count } цооногийн амсарт хэрэглэлээ
cmd-move-tool-applied-move-delta-delta-count-2 = Шилжилт ({ $delta })-г { $count } объектод хэрэглэлээ
cmd-move-tool-count-hole-s = { $count } цооног
cmd-object-edit-edited-kind = { $kind } засварлалаа
cmd-object-edit-edited-kind-count-vertices = { $kind } засварлалаа ({ $count } орой цэг)
cmd-object-edit-no-changes-apply = Хэрэглэх өөрчлөлт алга
cmd-object-edit-object-changed-since-editor-opened = Энэ объект засварлагч нээгдсэнээс хойш өөрчлөгдсөн байна; одоогийн хувилбарыг засварлахын тулд дахин нээнэ үү
cmd-object-edit-object-edit-target-changed-discardin = Заслын объект өөрчлөгдсөн тул заслыг хаялаа
cmd-object-edit-object-no-longer-exists-document = Тэр объект баримт бичигт цаашид байхгүй байна
cmd-object-edit-select-single-design-object-edit = Засварлах ганц зураг төслийн объект сонгоно уу
cmd-object-edit-unassigned = Оноогдоогүй
cmd-offset-create-offset = Шилжилт үүсгэх
cmd-offset-created-offset-count-object-s = { $count } объектын шилжилт үүсгэлээ
cmd-offset-offset-distance-must-greater-than = Шилжилтийн зай тэгээс их байх ёстой
cmd-omf-could-not-open-project-source = { $source_name } төслийг нээж чадсангүй: { $error }
cmd-omf-create-open-project-before-merging = Өгөгдөл нэгтгэхээс өмнө төсөл үүсгэх эсвэл нээнэ үү
cmd-omf-encoding-project = Төслийг кодчилж байна…
cmd-omf-exported-project-path = Төслийг { $path } руу экспортоллоо
cmd-omf-imported-project-project-name-from = '{ $project_name }' төслийг { $source_name }-с импортоллоо: { $count } дээд түвшний өгөгдлийн багц
cmd-omf-importing-project = Төслийг импортолж байна…
cmd-omf-omf-export-failed-error = OMF экспорт амжилтгүй боллоо: { $error }
cmd-omf-omf-import-failed-error = OMF импорт амжилтгүй боллоо: { $error }
cmd-omf-opened-project-project-name-from = '{ $project_name }' төслийг { $source_name }-с нээлээ
cmd-omf-project-source-name-contains-no = '{ $source_name }' төсөл дэмжигдсэн өгөгдлийн элемент агуулаагүй байна
cmd-omf-source-name-applied-project-origin = { $source_name }: нэгтгэхийн өмнө төслийн { $origin } эх цэгийг хэрэглэлээ
cmd-omf-source-name-coordinate-reference-sys = { $source_name }: координатын лавлагаа систем '{ $source_crs }' нь төслийн '{ $target_crs }' системээс ялгаатай байна; координатыг дахин проекцлолгүй нэгтгэлээ
cmd-omf-source-name-units-source-units = { $source_name }: нэгж '{ $source_units }' нь төслийн '{ $target_units }' нэгжээс ялгаатай байна; координатыг хөрвүүлэлгүй нэгтгэлээ
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = Экспортлох Incline Design өгөгдөл нээлттэй байхгүй байна
cmd-placement-2-vertices = 2 орой цэг
cmd-placement-count-vertices = { $count } орой цэг
cmd-placement-created-circle-radius-radius-m = { $radius } м радиустай тойрог үүсгэлээ
cmd-placement-created-closed-polyline-count-vertic = { $count } орой цэгтэй хаалттай полилиниа үүсгэлээ
cmd-placement-created-line-segment-2-vertices = 2 орой цэгтэй шугамын сегмент үүсгэлээ
cmd-placement-created-open-polyline-count-vertices = { $count } орой цэгтэй задгай полилиниа үүсгэлээ
cmd-placement-placed-point-x-y-z = { $x }, { $y }, { $z }-д цэг байрлуулав
cmd-placement-radius-radius-m = Радиус { $radius } м
cmd-plot-composing-engineering-drawing = Инженерийн зургийг бүтээж байна…
cmd-plot-could-not-write-engineering-drawing = Инженерийн зургийг бичиж чадсангүй: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Зургийн масштабыг харагдаж буй өгөгдөлд тааруулав: 1:{ $scale }
cmd-plot-plot = Зураг
cmd-plot-saved-engineering-drawing-descriptio = Инженерийн зургийг хадгаллаа: { $description } ({ $width } × { $height } px, { $dpi } dpi)
cmd-point-cloud-failed-load-point-cloud-error = Цэгэн үүлийг ачаалж чадсангүй: { $error }
cmd-point-cloud-loaded-point-cloud-name-count = { $name } цэгэн үүлийг ачааллаа ({ $count } цэг)
cmd-point-cloud-point-cloud-loader-disconnected-path = { $path }-н цэгэн үүл ачаалагч холболтоо тасалдлаа
cmd-point-cloud-tin-max-edge-disabled = (дээд ирмэг идэвхгүй)
cmd-point-cloud-tin-max-edge-max-edge = (дээд ирмэг { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = Цэгэн үүлийн TIN амжилтгүй боллоо: { $error }
cmd-point-cloud-tin-terrain-tin-spatially-subsampled-sam = Рельефийн TIN: орон зайгаар дэд-түүвэрлэсэн { $total } цэгээс { $sampled }
cmd-point-cloud-tin-terrain-tin-triangulated-vertex-coun = Рельефийн TIN: { $vertex_count } давхцаагүй XY цэгийг { $face_count } тал болгож триангуляцлав{ $suffix }
cmd-products-added-product-delay-ms-ms = { $delay_ms } мс { $name } бүтээгдэхүүнийг нэмлээ
cmd-products-deleted-product-delay-ms-ms = { $delay_ms } мс { $name } бүтээгдэхүүнийг устгалаа
cmd-products-failed-save-products-error = Бүтээгдэхүүнийг хадгалж чадсангүй: { $error }
cmd-products-product-no-longer-palette = Тэр бүтээгдэхүүн цаашид палитрт байхгүй байна
cmd-raster-draped-raster-raster-over-triangulat = { $raster } растерыг { $triangulation } триангуляц дээр дараалаа (давхцах хэмжээ)
cmd-raster-failed-load-raster-name-error = { $name } растерыг ачаалж чадсангүй: { $error }
cmd-raster-failed-load-raster-path-error = { $path } растерыг ачаалж чадсангүй: { $error }
cmd-raster-loaded-raster-name-via-driver = { $name } растерыг { $driver } драйверээр ачааллаа ({ $srcx }x{ $srcy }, урьдчилан харах { $prevx }x{ $prevy })
cmd-raster-no-loaded-triangulation-overlaps-ext = { $name }-н хэмжээтэй давхцах ачаалагдсан триангуляц алга
cmd-raster-raster-loader-disconnected-path = { $path }-н растер ачаалагч холболтоо тасалдлаа
cmd-raster-undraped-rasters-from-count-triangul = { $count } триангуляцаас растерыг дараахаас цуцаллаа
cmd-relimit-relimit-click-did-not-hit = Хязгаарлах: товшилт ямар ч объектод тусаагүй (заагчийн доор юу ч байхгүй)
cmd-relimit-relimit-click-ignored-tool-not = Хязгаарлах: товшилтыг үл хэрэгсэв, багаж одоогоор бай сонгохыг хүлээгээгүй байна
cmd-relimit-relimit-clicked-source-line-itself = Хязгаарлах: та эх шугам дээрээ л товшлоо, өөр шугам сонгоно уу
cmd-relimit-relimit-no-source-line-set = Хязгаарлах: эх шугам тохируулаагүй байна, сонголтыг таслав
cmd-relimit-relimited-line-source-id-selected = { $source_id } шугамыг сонгосон бай руу хязгаарлав
cmd-relimit-resized-line-source-id-using = { $source_id } шугамыг { $mode } горимоор { $value } утга ашиглан хэмжээг өөрчлөв
cmd-rename-item-no-longer-belongs-active = Тэр зүйл цаашид идэвхтэй төсөлд харьяалагдахгүй байна
cmd-rename-renamed-before-name = '{ $before }'-г '{ $name }' болгож нэрлэв
cmd-rename-renamed-before-name-requested-alread = '{ $before }'-г '{ $name }' болгож нэрлэв ('{ $requested }' нэр аль хэдийн эзэлгдсэн байна)
cmd-rotate-collar-turned-count-drillhole-collar-s = { $count } цооногийн амсрыг эргүүлэв ({ $rotation })
cmd-section-verb-count-item-s-section = { $section }-д { $count } зүйлийг { $verb }
cmd-selection-delete-vertex = Орой цэгийг устгах
cmd-selection-deleted-count-selected-object-s = Сонгосон { $count } объектыг устгалаа
cmd-selection-deleted-vertex-vertex-from-polyline = { $object_id } полилиниагаас { $vertex } орой цэгийг устгалаа
cmd-selection-duplicate-selection = Сонголтыг хувилах
cmd-selection-duplicated-count-object-s = { $count } объектыг хувилав
cmd-session-created-triangulation-name-vertex-co = '{ $name }' триангуляцыг үүсгэлээ ({ $vertex_count } орой цэг, { $face_count } тал), гадаргуугийн төрөл { $surface_type }
cmd-session-deleted-triangulation-name-from-proj = '{ $name }' триангуляцыг төслөөс устгалаа
cmd-session-failed-load-triangulation-error = Триангуляцыг ачаалж чадсангүй: { $error }
cmd-session-failed-load-triangulation-message = Триангуляцыг ачаалж чадсангүй: { $message }
cmd-session-loaded-triangulation-name-path-verte = '{ $name }' триангуляцыг ачааллаа ({ $path }, { $vertex_count } орой цэг, { $face_count } тал)
cmd-session-set-triangulation-tri-id-color = { $tri_id } триангуляцын өнгийг { $color } болгож тохируулав
cmd-session-triangulation-load-path-ended-withou = { $path }-н триангуляц ачаалалт үр дүнгүй дуусав
cmd-session-triangulation-operation-failed-messa = Триангуляцын үйлдэл амжилтгүй боллоо: { $message }
cmd-session-unloaded-triangulation-name = '{ $name }' триангуляцыг буулгалаа
cmd-slice-entered-slice-view-cx-cy = Огтлолын харагдац руу орлоо @ { $cx }, { $cy }, { $cz }, чиглэл { $dx }, { $dy } ({ $length }м шугам)
cmd-slice-exited-slice-view = Огтлолын харагдацаас гарлаа
cmd-slice-reset-section-view-fit-extents = Огтлолын харагдацыг сэргээх (хэмжээнд тааруулах)
cmd-slice-set-section-grid-enabled = Огтлолын торыг тохируулав = { $enabled }
cmd-split-created-2-open-polylines = 2 задгай полилиниа үүсгэлээ
cmd-split-split-line = Шугамыг хуваах
cmd-split-split-points-choose-interior-vertex = Цэгээр хуваах: задгай шугамын дотоод орой цэгийг сонгоно уу
cmd-split-split-points-choose-two-non = Цэгээр хуваах: зэргэлдээ бус хоёр полилиниагийн орой цэгийг сонгоно уу
cmd-split-split-source-polyline-into-two = Эх полилиниаг хоёр задгай полилиниа болгож хуваав
cmd-text-finished-text-edit-object-object = { $object_id } объектын текст засварыг дуусгалаа
cmd-text-updated-text-object-object-id = { $object_id } объект дээрх текстийг шинэчиллээ
cmd-view-centre-rotation-not-available-flying = Нисэх горимд эргэлтийн төв боломжгүй байна
cmd-view-fixed-centre-rotation-x-y = Эргэлтийн төвийг { $x }, { $y }, { $z }-д тогтоов
cmd-view-no-point-under-cursor-fix = Заагчийн доор эргэлтийн төвийг тогтоох цэг алга
cmd-view-released-centre-rotation = Эргэлтийн төвийг суллалаа
cmd-view-reset-view-fit-extents = Харагдацыг сэргээх (хэмжээнд тааруулах)
cmd-view-set-topology-wireframes-enabled = Топологийн торон дүрсийг тохируулав = { $enabled }
cmd-view-set-view-points-enabled = Цэгийн харагдацыг тохируулав = { $enabled }
cmd-view-set-xy-grid-enabled = XY торыг тохируулав = { $enabled }
cmd-view-zoom-extents-preserving-angle = Хэмжээнд тааруулж томруулах (өнцгийг хадгалах)

## Common strings

common-add-product = Бүтээгдэхүүн нэмэх
common-background = Дэвсгэр
common-block-model = Блокийн загвар
common-block-models = Блокийн загварууд
common-cancelled = Цуцлагдсан
common-chamfer = Фаск
common-choose = Сонгох...
common-circle = Тойрог
common-click-point-fix-centre-rotation = Эргэлтийн төвийг тогтооход цэг дээр товшино уу
common-clip-surface-polyline = Гадаргууг полилиниагаар огтлох...
common-closed = Хаалттай
common-colour = Өнгө
common-confirm-omf-rewrite = OMF дахин бичихийг батлах
common-could-not-replace-current-project = Одоогийн төслийг солиж чадсангүй: { $error }
common-count-object-s = { $count } объект
common-create = Үүсгэх
common-create-batter-berm = Уступ-берм үүсгэх
common-create-bezier-curve = Безье муруй үүсгэх
common-create-block-model = Блокийн загвар үүсгэх
common-create-block-model-2 = Блокийн загвар үүсгэх...
common-create-circle = Тойрог үүсгэх
common-create-drill-pattern = Өрмийн сүлжээ үүсгэх
common-create-layer = Давхарга үүсгэх
common-create-line = Шугам үүсгэх
common-create-ore-triangulation = Хүдрийн триангуляц үүсгэх
common-create-ore-triangulation-2 = Хүдрийн триангуляц үүсгэх...
common-create-point = Цэг үүсгэх
common-create-polyline = Полилиниа үүсгэх
common-create-triangulation = Триангуляц үүсгэх...
common-crosses = Хөндлөвч тэмдэг
common-cut = Тайрах
common-cut-topology-pit-shell = Топологийг карьерийн бүрхүүлээр тайрах...
common-delete-layer = Давхарга устгах
common-delete-product = Бүтээгдэхүүн устгах
common-delete-selection = Сонголтыг устгах
common-designs = Зураг төслүүд
common-discard-layer-changes = Давхаргын өөрчлөлтийг үл хэрэгсэх
common-down = Доош
common-drape-topology = Топологи дээр дараах
common-easting = Зүүн тийш (X)
common-edit-object = Объект засах
common-edit-text = Текст засах
common-elevation = Өндөрлөг
common-exit-without-saving = Хадгалахгүйгээр гарах
common-export-engineering-drawing = Инженерийн зургийг экспортлох
common-filter = Шүүлтүүр
common-fly-mode = Нисэх горим
common-generate-contour-lines = Изолиниа үүсгэх...
common-hide-all = Бүгдийг нуух
common-hide-selection = Сонголтыг нуух
common-ignore = Үл хэрэгсэх
common-import-csv-block-model = CSV блокийн загвар импортлох
common-import-dxf = DXF импортлох
common-incline-design-project = Incline Design төсөл
common-layer = Давхарга
common-legend = Тайлбар
common-line = Шугам
common-line-weight = Шугамын зузаан
common-lock-all = Бүгдийг түгжих
common-lock-selection = Сонголтыг түгжих
common-m = м
common-max = Дээд
common-merge-shell-into-topology = Бүрхүүлийг топологид нэгтгэх
common-merge-shell-into-topology-2 = Бүрхүүлийг топологид нэгтгэх...
common-move-collar = Амсрыг шилжүүлэх
common-move-design = Объектыг шилжүүлэх
common-move-selection = Сонголтыг шилжүүлэх
common-new-product = Шинэ бүтээгдэхүүн
common-no-block-models = Блокийн загвар алга
common-no-design-layers = Зураг төслийн давхарга алга
common-no-drill-holes = Цооног алга
common-no-file-chosen = Файл сонгоогүй
common-no-open-project = Нээлттэй төсөл алга
common-no-point-clouds = Цэгэн үүл алга
common-no-triangulations = Триангуляц алга
common-none = Байхгүй
common-northing = Хойд тийш (Y)
common-offset = Шилжилт
common-open = Нээх
common-orientation = Чиглэл
common-point = Цэг
common-point-cloud = Цэгэн үүл
common-point-clouds = Цэгэн үүлүүд
common-polyline = Полилиниа
common-polyline-layer = '{ $layer }' дээрх полилиниа
common-project = Төсөл
common-rasters = Растерууд
common-redo = Дахин хийх
common-relimit-line = Шугамыг хязгаарлах
common-remove-project = Төсөл устгах
common-reset-view = Харагдацыг сэргээх
common-reveal-all = Бүгдийг харуулах
common-reveal-finder = Finder-т харуулах
common-rotate-collar = Амсрыг эргүүлэх
common-save-exit = Хадгалаад гарах
common-scale-bar = Масштабын мөр
common-set-initiation-point = Дэлбэлгээ эхлүүлэх цэгийг тохируулах
common-shape = Хэлбэр
common-shell = Бүрхүүлтэй
common-slashes = Ташуу зураас
common-slice = Огтлол
common-slice-triangulation-z-range = Триангуляцыг Z мужаар огтлох...
common-surface-contours = Гадаргуугийн изолиниа
common-text = Текст
common-text-2 = °
common-tie-holes = Цооногуудыг холбох
common-triangulations = Триангуляцууд
common-trim-topology = Топологиор тайрах...
common-undo = Буцаах
common-undrape-all = Бүгдийг дараахаас цуцлах
common-uniform-white = Жигд цагаан
common-unlock-all = Бүгдийн түгжээг тайлах
common-untitled = Нэргүй
common-up = Дээш
common-vertical-exaggeration = Босоо хэтрүүлэлт
common-x = x
common-zoom-extents = Хэмжээнд тааруулж томруулах

## Confirmations strings

confirmations-close-project-unsaved-changes = Төслийг хаах: Хадгалаагүй өөрчлөлт
confirmations-close-without-saving = Хадгалахгүйгээр хаах
confirmations-delete = Устгах
confirmations-delete-objects = Объект устгах
confirmations-discard = Үл хэрэгсэх
confirmations-discard-all-unsaved-changes-layer =
    '{ $name }' давхаргын хадгалаагүй бүх өөрчлөлтийг үл хэрэгсэх үү?
    Хадгалсан давхарга дискнээс дахин ачаалагдах бол бусад давхаргын өөрчлөлт хэвээр хадгалагдана. Энэ үйлдлийг буцаах боломжгүй.
confirmations-discard-all-unsaved-changes-name =
    '{ $name }'-н хадгалаагүй бүх өөрчлөлтийг үл хэрэгсэх үү?
    Сүүлд хадгалсан хувилбар дискнээс дахин ачаалагдана. Энэ үйлдлийг буцаах боломжгүй.
confirmations-discard-changes = Өөрчлөлтийг үл хэрэгсэх
confirmations-exit-unsaved-changes = Гарах: Хадгалаагүй өөрчлөлт
confirmations-incline-design-cannot-reproduce-all = Incline Design эх OMF-н бүх агуулгыг дахин үүсгэж чадахгүй. Хадгалахад дараах агуулга орхигдоно:
confirmations-product = Бүтээгдэхүүн
confirmations-project = энэ төсөл
confirmations-remove-name-delete-its-browser = '{ $name }'-г устгаад хөтчид хадгалагдсан хуулбарыг нь устгах уу? Хадгалаагүй өөрчлөлт алдагдана.
confirmations-remove-project-unsaved-changes = Төслийг устгах: Хадгалаагүй өөрчлөлт
confirmations-remove-without-saving = Хадгалахгүйгээр устгах
confirmations-replace-project-unsaved-changes = Төслийг солих: Хадгалаагүй өөрчлөлт
confirmations-save = Хадгалах
confirmations-save-anyway = Тэй тэй хадгалах
confirmations-save-changes-current-project-before = Одоогийн төслийг солихоос өмнө өөрчлөлтийг хадгалах уу?
confirmations-save-changes-name-before-closing = '{ $name }'-г хаахаас өмнө өөрчлөлтийг хадгалах уу?
confirmations-save-changes-name-before-removing = Incline Design-с устгахаас өмнө '{ $name }'-н өөрчлөлтийг хадгалах уу?
confirmations-save-close = Хадгалаад хаах
confirmations-save-modified-project-before-exiting = Гарахаас өмнө өөрчилсөн төслийг хадгалах уу?
confirmations-save-modified-project-browser-storag = Гарахаас өмнө өөрчилсөн төслийг хөтчийн санах ойд хадгалах уу?
confirmations-save-remove = Хадгалаад устгах

## Console strings

console-copy-all = Бүгдийг хуулах
console-copy-message = Мессежийг хуулах
console-error = АЛДАА
console-info = МЭДЭЭЛЭЛ
console-no-console-activity-yet = Одоогоор консолын үйл ажиллагаа алга
console-pending = ХҮЛЭЭГДЭЖ БАЙНА
console-progress-summary = Явагдаж байна · { $summary }
console-success = АМЖИЛТТАЙ
console-warn = АНХААРУУЛГА

## Csv strings

csv-block-model-category = Ангилал
csv-block-model-value = Утга

## Drill strings

drill-hole-add-stop = Зогсоол нэмэх
drill-hole-all-rendered-intervals-opaque-white = Бүх дүрслэгдсэн интервал бүрэн цагаан өнгөтэй.
drill-hole-burden-spacing-must-greater-than = Бурден ба зай тэгээс их байх ёстой
drill-hole-choose-valid-closed-polyline = Зөв хаалттай полилиниа сонгоно уу
drill-hole-colour-scale = Өнгийн масштаб
drill-hole-field = Талбар
drill-hole-grayscale = Саарал өнгийн масштаб
drill-hole-green-yellow-red = Ногоон–Шар–Улаан
drill-hole-heat = Дулаан
drill-hole-no-holes-fit-inside-boundary = Одоогийн бурден ба зайгаар энэ хил дотор ямар ч цооног багтахгүй байна
drill-hole-pattern-exceeds-maximum-maximum-hole = Сүлжээ { $maximum } цооногийн дээд хэмжээнээс хэтэрлээ; бурден эсвэл зайг нэмэгдүүлнэ үү
drill-hole-preset = Бэлэн тохиргоо
drill-hole-px = px
drill-hole-rainbow = Солонго
drill-hole-reset-preset = Тохиргоог сэргээх
drill-hole-rotation-offsets-must-contain-valid = Эргэлт ба шилжилт зөв тоо агуулах ёстой
drill-hole-selected-polyline-has-no-usable = Сонгосон полилиниа ашиглах боломжтой XY талбайгүй байна
drill-hole-smooth-interpolation = Гөлгөр интерполяци
drill-hole-spacing-would-scan-too-many = Энэ зайгаар хэт олон торны эсийг шалгах шаардлагатай болно; бурден эсвэл зайг нэмэгдүүлнэ үү (дээд хэмжээ { $maximum } цооног)
drill-hole-square = Дөрвөлжин
drill-hole-staggered = Шаталсан
drill-hole-stepped-bands = Шаталсан зурвас
drill-hole-text = ×
drill-hole-text-2 = −
drill-hole-unsupported-drillhole-source = Дэмжигдээгүй цооногийн эх сурвалж
drill-hole-width = Өргөн
drill-pattern-arrangement = Байршуулалт
drill-pattern-axis-offset = { $axis } шилжилт
drill-pattern-blast-shape = Тэсэлгээний хэлбэр
drill-pattern-burden = Бурден
drill-pattern-choose-closed-blast-boundary-then = Хаалттай тэсэлгээний хилийг сонгоод, торыг тохируулна уу. Цооногууд харагдах цонхонд амьд шинэчлэгдэнэ.
drill-pattern-closed-design-polyline-whose-xy = XY проекц нь цооногоор дүүргэгдэх хаалттай зураг төслийн полилиниа.
drill-pattern-counter-clockwise-pattern-rotation-f = Глобал { $axis } тэнхлэгээс цагийн зүүний эсрэг сүлжээний эргэлт.
drill-pattern-distance-between-holes-along-each = Сүлжээний мөр бүрийн дагуух цооногуудын хоорондох зай.
drill-pattern-e-g-west-cut-03 = ж: Баруун Огтлол 03
drill-pattern-finished-hole-diameter-entered-milli = Дуусгасан цооногийн диаметр. Миллиметрээр оруулж, үүсгэсэн цооног бүрт хадгалагдана.
drill-pattern-hole-depth = Цооногийн гүн
drill-pattern-hole-diameter = Цооногийн диаметр
drill-pattern-move-over-closed-polyline-then = Заагчийг хаалттай полилиниа дээгүүр аваачаад, харагдах цонхонд түүн дээр товшино уу. Esc сонголтыг цуцална.
drill-pattern-name-drillhole-dataset-created-proje = Төсөлд үүсгэгдэх цооногийн өгөгдлийн багцын нэр.
drill-pattern-none-picked = Юу ч сонгоогүй
drill-pattern-pattern-name = Сүлжээний нэр
drill-pattern-perpendicular-distance-between-patte = Сүлжээний мөрүүдийн хоорондох перпендикуляр зай.
drill-pattern-pick = Сонгох
drill-pattern-preview-count-hole-s-diameter = Урьдчилан харах: { $count } цооног · { $diameter } мм диаметр · { $depth } м гүн
drill-pattern-rotation = Эргэлт
drill-pattern-shift-pattern-grid-along-global = Сүлжээний торыг тэсэлгээний хэлбэрт огтлогдсон хэвээр глобал { $axis } тэнхлэгийн дагуу шилжүүлэх.
drill-pattern-spacing = Зай
drill-pattern-staggered-offsets-every-second-row = Шаталсан байршил хоёр дахь мөр бүрийг зайн хагасаар шилжүүлнэ.
drill-pattern-vertical-depth-below-each-collar = Амсар бүрийн доорхи босоо гүн.

## Dxf strings

dxf-dxf-block-nesting-exceeds-maximum = DXF блокийн үүрлэлт дээд гүнээс ({ $depth }) хэтэрсэн тул '{ $name }'-г алгасав
dxf-dxf-circular-block-reference-detecte = DXF тойрог хэлбэрийн блокийн лавлагаа илэрлээ: '{ $name }'
dxf-dxf-entity-referenced-undefined-laye = DXF объект тодорхойлогдоогүй '{ $name }' давхаргыг ашигласан тул '{ $fallback }' нэрээр импортлов
dxf-dxf-import-exceeds-what-budget = DXF импорт { $what } төсвөөс ({ $limit }) хэтэрлээ; үлдсэн геометрийг алгасав
dxf-dxf-insert-references-unknown-block = DXF INSERT үл мэдэгдэх '{ $name }' блокийг ашиглаж байна

## Edit strings

edit-absolute-length = Үнэмлэхүй урт
edit-absolute-rl = Үнэмлэхүй RL
edit-action = Үйлдэл
edit-angle = Өнцөг
edit-angle-from-horizontal-negative-downw = Хэвтээ шугамаас хэмжсэн өнцөг, доош нь сөрөг: −90° нь босоо цооног.
edit-app-web-not-recommended-production = { $app } Web-г бодит ашиглалтад санал болгохгүй. Зөвхөн демо болгон ашиглана уу.
edit-application = Программ
edit-apply = Хэрэглэх
edit-apply-pick-target = Хэрэглээд бай сонгох
edit-axis-value = { $axis } утга
edit-azimuth = Азимут
edit-batter-angle = Налуугийн өнцөг (°)
edit-bearing-holes-drilled-degrees-clockw = Цооногууд өрөмдөгдөх чиглэл, торны хойд зүгээс цагийн зүүний дагуу градусаар.
edit-bench-height = Уступын өндөр
edit-benches = Уступууд
edit-berm-width = Бермийн өргөн
edit-bezier-curve = Безье муруй
edit-choose-layer = Давхарга сонгох
edit-choose-whether-entered-value-distanc = Оруулсан утга нь налуугийн дагуух зай, хэвтээ өргөн, эсвэл босоо өндөр эсэхийг сонгоно уу.
edit-choose-which-two-polyline-paths = Сонгосон орой цэгүүдийн хоорондох хоёр полилиниа замын алийг нь солихыг сонгоно уу. Урт нь өндөрлөг ба муруй ирмэгийг оролцуулна.
edit-click-corner-closed-polyline = Хаалттай полилиниагийн булан дээр товшино уу.
edit-click-open-closed-polyline-begin = Эхлэхийн тулд задгай эсвэл хаалттай полилиниа дээр товшино уу.
edit-click-second-vertex-replacement-span = Солих хэсгийн хоёр дахь орой цэгийг товшино уу.
edit-click-vertex-start-replacement-span = Солих хэсгийг эхлүүлэхийн тулд орой цэг дээр товшино уу.
edit-collide-triangulation = Триангуляцтай мөргөлдөх
edit-confirm-selection = Сонголтыг батлах
edit-control-point-1 = Хяналтын цэг 1
edit-control-point-2 = Хяналтын цэг 2
edit-copy = Хуулах
edit-corner-radius-limited-so-replacement = Буланг тойруулах радиус, зэргэлдээх орой цэгийг давахгүйгээр хязгаарлагдана.
edit-create-new-layer = Шинэ давхарга үүсгэх
edit-create-new-project = Шинэ төсөл үүсгэх
edit-create-project = Төсөл үүсгэх
edit-delta-length-m-use = Уртын өөрчлөлт (м, + эсвэл - ашиглана уу)
edit-dip = Уналт
edit-direction = Чиглэл
edit-distance = Зай
edit-distance-along-slope = Налуугийн дагуух зай
edit-download-free-native-version-our = Манай вэбсайтаас үнэгүй суурин хувилбарыг татаж авах ↗
edit-drill-hole = Цооног
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = Төгсгөл
edit-enter-valid-elevation = Зөв өндөрлөгийн утга оруулна уу.
edit-exit-slice = Огтлолоос гарах
edit-finish-polyline = Полилиниаг дуусгах
edit-generate-batter-berms = Уступ-берм үүсгэх
edit-height = Өндөр
edit-height-change = Өндрийн өөрчлөлт
edit-height-mode = Өндрийн горим
edit-horizontal-distance = Хэвтээ зай
edit-horizontal-width-each-flat-berm = Дараалсан налуу талуудын хоорондох тэгш бермийн хэвтээ өргөн.
edit-hover-choose-which-end-move = Аль үзүүрийг шилжүүлэхээ сонгохын тулд заагчийг байрлуулаад, батлахын тулд товшино уу.
edit-insert-point-elevation = Өндөрлөг дээр цэг оруулах
edit-intersect = Огтлолцуулах
edit-kind-properties = { $kind } { $properties }
edit-layer-name = Давхаргын нэр
edit-load-project = Төсөл ачаалах
edit-longest = Хамгийн урт
edit-m-s = м/с
edit-measure = Хэмжих
edit-mit-license = MIT лиценз
edit-mode = Горим
edit-move = Шилжүүлэх
edit-move-layer = Давхарга руу шилжүүлэх
edit-move-which-end = Аль үзүүрийг шилжүүлэх
edit-movement-speed-slice-when-using = Чиглүүлэх товчлуур ашиглах үеийн огтлолын хөдөлгөөний хурд.
edit-moving-end-endpoint = Шилжиж байна: Төгсгөлийн үзүүр
edit-moving-start-endpoint = Шилжиж байна: Эхлэлийн үзүүр
edit-new-length-m = Шинэ урт (м)
edit-new-project = Шинэ төсөл
edit-number-complete-batter-berm-levels = Бүрэн налуу-бермийн түвшний тоо. Дээд хязгаар нь заасан геометрийг хадгалах хамгийн гүн түвшнээр хязгаарлагдана.
edit-number-line-segments-used-approximat = Сонгосон хоёр орой цэгийн хоорондох муруйг ойролцоолон дүрслэхэд ашиглах шугаман сегментийн тоо.
edit-number-straight-segments-used-approx = Дугуйрсан буланг ойролцоолон дүрслэхэд ашиглах шулуун сегментийн тоо. Шулуун фаскийн хувьд 1-ийг ашиглана уу.
edit-object = Объект
edit-offset-element = Элементийг шилжүүлэх
edit-pick-side = Талыг сонгох
edit-pit = Карьер
edit-project-name = Төслийн нэр
edit-properties = Шинж чанар
edit-radius = Радиус
edit-recent = Сүүлд
edit-relative = Харьцангуй (+/-)
edit-relative-applies-vertical-change-eve = «Харьцангуй» цэг бүрт босоо өөрчлөлт хэрэглэнэ. «Үнэмлэхүй RL» цэг бүрийг нэг байх өндөрлөг рүү проекцлоно.
edit-remove-from-list = Жагсаалтаас устгах
edit-replace-path = Замыг солих
edit-rotate = Эргүүлэх
edit-rotation-speed-slice-when-using = Q, E товч ашиглах үеийн огтлолын эргэлтийн хурд.
edit-s = °/с
edit-segments = Сегментүүд
edit-segments-lying-elevation-ignored = Энэ өндөрлөгт байрлах сегментүүдийг үл хэрэгсэнэ.
edit-select-endpoint-changes-other-endpoi = Өөрчлөгдөх үзүүрийг сонгоно уу; нөгөө үзүүр тогтмол хэвээр байна.
edit-selected-holes-point-different-ways = Сонгосон цооногууд өөр өөр чиглэж байна. Хэрэглэх нь бүгдийг эдгээр өнцөгт тохируулна.
edit-selected-start-end-point-moves = Сонгосон эхлэл эсвэл төгсгөлийн цэг шугамын чиглэлийн дагуу шилжинэ; эсрэг үзүүр тогтмол хэвээр байна.
edit-set-axis = { $axis }-ыг тохируулах
edit-shortest = Хамгийн богино
edit-slice-view = Огтлолын харагдац
edit-slope-angle-each-batter-face = Хэвтээ шугамаас хэмжсэн налуу талын бүр өнцөг.
edit-slope-angle-offset-positive-negative = Шилжилтийн налуугийн өнцөг. Эерэг ба сөрөг өнцөг нь хажуу тийш шилжихдээ хуулбарыг эхээс дээш эсвэл доош шилжүүлнэ.
edit-speed = Хурд
edit-start = Эхлэл
edit-stockpile = Овоолго
edit-stop-generated-offset-where-its = Үүсгэсэн шилжилтийн зам харагдаж буй триангуляцтай анх уулзсан газар зогсоох.
edit-target-rl = Байх RL
edit-text-colour-opacity = Текстийн өнгө ба тунгалагжилт.
edit-thickness-visible-slice-slab-centred = Тоймын заагч дээр төвлөрсөн харагдах огтлолын давхаргын зузаан.
edit-translation-distance-along-world-axi = Дэлхийн { $axis } тэнхлэгийн дагуух шилжилтийн зай.
edit-type = Төрөл
edit-type-direction-together-set-offset = Төрөл болон Чиглэл хамтдаа шилжилтийн талыг тодорхойлно. Карьер + Дээш ба Овоолго + Доош нь гадагш алхана; Карьер + Доош ба Овоолго + Дээш нь дотогш алхана.
edit-up-raises-each-bench-bench = «Дээш» уступ бүрийг уступын өндрөөр өргөнө; «Доош» доошлуулна. Энэ нь мөн шилжилтийн талыг эргүүлнэ - Төрөлийг үзнэ үү.
edit-value-interpreted-using-selected-mea = Утга нь сонгосон хэмжигдэхүүн ба өндрийн горимын дагуу тайлбарлагдана.
edit-vertical-rise-fall-each-bench = Дараагийн берм үүсэхээс өмнөх уступ бүрийн босоо өсөлт эсвэл уналт.
edit-world-x-y-z-coordinates = Эхний Безье хяналтын цэгийн дэлхийн X, Y, Z координат.
edit-world-x-y-z-coordinates-2 = Хоёр дахь Безье хяналтын цэгийн дэлхийн X, Y, Z координат.

## Events strings

events-couldn-t-exit-error = Гарч чадсангүй: { $error }
events-couldn-t-save-error = Хадгалж чадсангүй: { $error }
events-set-elevation = Өндөрлөг тохируулах
events-set-elevation-from-cursor-hit = Заагчийн тусалтаас өндөрлөгийг Z { $z } болгож тохируулав
events-tool-not-available-section-view = Энэ хэрэгсэл огтлолын харагдацад боломжгүй байна

## Explorer strings

explorer-clear-active-triangulation-texture = Идэвхтэй триангуляцын текстурыг цэвэрлэх
explorer-delete-from-project = Төслөөс устгах
explorer-discard-changes = Өөрчлөлтийг үл хэрэгсэх...
explorer-download = Татаж авах
explorer-drape-over-surface = Гадаргуу дээр дараах
explorer-draped-over-surface = Гадаргуу дээр дараасан
explorer-duplicate = Хувилах
explorer-face-colour = Талын өнгө
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    { $count } өнгөний хувьсагч
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    { $holes } цооног
    { $fields } өнгөний талбар
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    { $count } цэг
explorer-id-raster-id-source-driver =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = Ачаалах
explorer-lock = Түгжих
explorer-select-all-objects = Бүх объектыг сонгох
explorer-source-name = Эх сурвалж: { $name }
explorer-unload = Буулгах
explorer-unlock = Түгжээг тайлах

## Files strings

files-automatic-colour = Автомат өнгө
files-automatic-rl-spacing = Автомат RL зай
files-axis-scale-ratio = { $axis } масштабын харьцаа
files-ok = OK
files-reset-1 = 1×-т сэргээх
files-rl-grid-options = RL торны сонголтууд
files-rl-spacing = RL зай
files-scales-z-distances-visually-without = Хадгалагдсан координатыг өөрчлөхгүйгээр Z зайг харагдах байдлаар масштаблана.
files-thickness = Зузаан
files-xy-grid-options = XY торны сонголтууд

## Gpu strings

gpu-cache-block-model-surface-build-failed = Блокийн загварын гадаргуу бүтээх амжилтгүй боллоо: { $error }
gpu-cache-block-model-surface-build-worker = Блокийн загварын гадаргуу бүтээх ажлын процесс холболтоо тасалдлаа
gpu-cache-block-model-surface-chunk-rejected = GPU хуваарилалтаас өмнө блокийн загварын гадаргуугийн хэсэг татгалзагдав: instances={ $instances } байт, limit={ $limit } байт
gpu-cache-block-volume-preparation-worker-disc = Блокийн эзлэхүүн бэлтгэх ажлын процесс холболтоо тасалдлаа
gpu-cache-translucent-volume-could-not-built = Тунгалаг эзлэхүүнийг бүтээж чадсангүй ({ $error }); энэ блокийн загварыг оронд нь шоо хэлбэрээр харуулж байна.
gpu-cache-triangulation-edge-chunk-rejected-be = GPU хуваарилалтаас өмнө триангуляцын ирмэгийн хэсэг татгалзагдав: instances={ $instances } байт, limit={ $limit } байт
gpu-cache-triangulation-gpu-chunk-rejected-bef = Хуваарилалтаас өмнө триангуляцын GPU хэсэг татгалзагдав: vertices={ $vertices } байт, indices={ $indices } байт, limit={ $limit } байт
gpu-cache-triangulation-name-has-count-vertice = '{ $name }' триангуляц { $count } орой цэгтэй (> u32::MAX); GPU-д зориулж хэсэглэх боломжгүй
gpu-cache-triangulation-name-uploaded-chunks-s = '{ $name }' триангуляцыг { $chunks } орон зайн хэсэг ({ $faces } тал)-т байршуулав

## Init strings

init-gpu-adapter-vendor-name-backend = GPU адаптер: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver-driver-driver-info = GPU драйвер: { $driver } { $driver_info }
init-gpu-supports-maximum-buffer-size = GPU { $size } MiB хүртэлх дээд буфферийн хэмжээг дэмждэг; том дүрслэл бүрэн харагдахгүй байж болно
init-surface-presentation-mode-mode = Гадаргуу үзүүлэх горим: { $mode }
init-wgpu-error-continuing-error = wgpu алдаа (үргэлжлүүлж байна): { $error }

## Io strings

io-ascii-points-xyz-pts = ASCII цэгүүд (.xyz, .pts)
io-attribute = Аттрибут
io-blank-header = (гарчиггүй)
io-block-model = Блокийн загвар:
io-choose-file-purpose-map-its = Баганыг холбохын тулд файлын зориулалтыг сонгоно уу.
io-choose-loaded-block-model = Ачаалагдсан блокийн загвар сонгох
io-choose-loaded-layer = Ачаалагдсан давхарга сонгох
io-choose-loaded-triangulation = Ачаалагдсан триангуляц сонгох
io-choose-purpose = Зориулалт сонгох…
io-choose-source-file-files-import = Импортлох эх файл(ууд)-ыг сонгоно уу.
io-collar = Амсар
io-column-mapping = Баганы харгалзаа
io-comma-separated-values-csv = Comma-Separated Values (.csv)
io-csv-files = CSV файлууд
io-default = Үндсэн
io-depth = Гүн
io-diameter = Диаметр
io-drawing-exchange-format-dxf = Drawing Exchange Format (.dxf)
io-drill-holes = Цооногууд
io-east-x = Зүүн / X
io-elevation-z = Өндөрлөг / Z
io-end-x = Төгсгөл X
io-end-y = Төгсгөл Y
io-end-z = Төгсгөл Z
io-explicit-segments = Тодорхой сегментүүд
io-export = Экспортлох
io-export-csv-block-model = CSV блокийн загвар экспортлох
io-export-dxf = DXF экспортлох
io-export-one-layer = Нэг давхарга экспортлох
io-export-ply = PLY экспортлох
io-export-stl = STL экспортлох
io-export-wavefront-obj = Wavefront OBJ экспортлох
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Импортлох
io-import-ascii-point-cloud = ASCII цэгэн үүл импортлох
io-import-drillhole-csv-bundle = Цооногийн CSV багц импортлох
io-import-geotiff = GeoTIFF импортлох
io-import-las-laz-point-cloud = LAS/LAZ цэгэн үүл импортлох
io-import-pcd-point-cloud = PCD цэгэн үүл импортлох
io-import-ply = PLY импортлох
io-import-stl = STL импортлох
io-import-wavefront-obj = Wavefront OBJ импортлох
io-interval = Интервал
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-mapped-csv-bundle-csv = Холбогдсон CSV багц (.csv)
io-model-file = Загварын файл
io-name-count-files = { $name } + { $count } файл
io-no-csv-chosen = .csv сонгоогүй
io-no-csv-files-chosen = CSV файл сонгоогүй
io-no-dxf-chosen = .dxf сонгоогүй
io-no-omf-chosen = .omf сонгоогүй
io-north-y = Хойд / Y
io-ply-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-source-file = Эх файл
io-start-x = Эхлэл X
io-start-y = Эхлэл Y
io-start-z = Эхлэл Z
io-stl-stl = STL (.stl)
io-triangulation = Триангуляц:
io-unmapped = Холбогдоогүй
io-wavefront-obj-obj = Wavefront OBJ (.obj)

## Jobs strings

jobs-background-task-poll-label-ended = '{ $poll_label }' далд ажил үр дүнгүй дуусав
jobs-discarded-stale-background-result-po = '{ $poll_label }'-н хуучирсан далд үр дүнг хаяв, учир нь эх сурвалж өөрчлөгдсөн эсвэл хаагдсан

## Logging strings

logging-activity-completed = Үйл ажиллагаа дууслаа
logging-activity-started = Үйл ажиллагаа эхэллээ
logging-application-id-id = Программын ID: { $id }
logging-application-name-name = Программын нэр: { $name }
logging-application-startup = Программын эхлэл
logging-build-target-os-architecture = Билд бай: { $os }-{ $architecture }
logging-completed = Дууссан
logging-count-messages = { $count } мессеж
logging-desktop-session-xdg-session-type = Ажлын талбарын сешн: XDG_SESSION_TYPE={ $type }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Incline Design-г эхлүүлж байна
logging-locale-environment-lang-lang-lc = Локал орчин: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session-user-user-shell = macOS сешн: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = Үйлдлийн систем: GNU / Linux
logging-operating-system-macos = Үйлдлийн систем: macOS
logging-operating-system-microsoft-windows = Үйлдлийн систем: Microsoft Windows
logging-pointer-width-width-bit = Заагчийн өргөн: { $width }-бит
logging-process-id-id = Процессын ID: { $id }
logging-release-version-version = Хувилбарын дугаар: { $version }
logging-renderer = Дүрслэгч
logging-rust-compiler-host-host = Rust хөрвүүлэгчийн хост: { $host }
logging-system = Систем
logging-system-error = Системийн алдаа
logging-unknown = үл мэдэгдэх
logging-windows-session-sessionname-session = Windows сешн: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = Ажиллаж байна…

## Mac strings

mac-cannot-install-macos-menu-bar = macOS цэсний мөрийг үндсэн урсгалаас гадуур суулгах боломжгүй
mac-quit-app = { $app }-аас гарах

## Main strings

main-incline-design-web-startup-failed = Incline Design Web-н эхлэл амжилтгүй боллоо: { $error }

## Menu strings

menu-count-files-selected = { $count } файл сонгогдсон

## Object strings

object-edit-appearance = Дүр төрх
object-edit-arc-circle = Нум ба тойрог
object-edit-arc-segments = Нумын хэсгүүд
object-edit-bulge = Гүдгэр
object-edit-bulge-arcs-horizontal-data-model = Өгөгдлийн загварын дагуу гүдгэр нум нь хэвтээ байна: нум нь хэвтээ хавтгайд эргэдэг бөгөөд өндөрлөг нь орой цэг бүрээс дараагийнх руу шулуунаар өөрчлөгддөг.
object-edit-centre-x = Төв X
object-edit-centre-y = Төв Y
object-edit-centre-z = Төв Z
object-edit-chord = Хорд
object-edit-colour-layer = Давхаргын өнгө
object-edit-enter-number = Тоо оруулна уу
object-edit-follow-owning-layer-s-colour = Энэ объектод бэхлэгдсэн өнгөний оронд эзэмшигч давхаргын өнгийг дагах.
object-edit-id = ID
object-edit-identity = Ижилт
object-edit-insert-after = Дараа нь оруулах
object-edit-join-last-vertex-back-first = Сүүлийн орой цэгийг эхнийхтэй нь дахин холбоно.
object-edit-length-length-m = Урт { $length } м
object-edit-move-down = Доош шилжүүлэх
object-edit-move-up = Дээш шилжүүлэх
object-edit-object-has-no-arc-segments = Энэ объект нумын хэсэггүй байна.
object-edit-object-has-single-position = Энэ объект ганц байрлалтай.
object-edit-object-needs-least-required-vertices = Энэ объектод дор хаяж { $required } орой цэг шаардлагатай
object-edit-one-more-properties-not-valid = Нэг буюу түүнээс олон шинж чанар хүчинтэй тоо биш байна
object-edit-perimeter-length-m-area-area = Периметр { $length } м, талбай { $area } м²
object-edit-reverse = Урвуулах
object-edit-row-row-position-bulge-not = { $row } мөр: байрлал эсвэл гүдгэр хүчинтэй тоо биш байна
object-edit-sweep = Хамрах өнцөг
object-edit-text-not-number = "{ $text }" тоо биш байна
object-edit-vertices = Орой цэгүүд

## Omf strings

omf-element-name-has-count-tie = '{ $name }' элемент цаашид агуулаагүй цооногуудыг нэрлэсэн { $count } холболт агуулж байна
omf-ignoring-colour-map-omf-attribute = OMF-н '{ $attribute }' аттрибут дээрх өнгийн зургийг үл хэрэгсэж байна: { $error }
omf-mining-data-exported-incline = Incline-с экспортолсон уул уурхайн өгөгдөл
omf-omf-import = OMF импорт
omf-omf-texture = OMF текстур
omf-omf-validation-warnings-warnings = OMF баталгаажуулалтын анхааруулга: { $warnings }
omf-project-application-metadata-applica = Төслийн '{ $application }' программын метаөгөгдөл хадгалагдахгүй
omf-project-author-not-retained = Төслийн зохиогч хадгалагдахгүй
omf-project-description-not-retained = Төслийн тайлбар хадгалагдахгүй
omf-project-has-unsupported-metadata-key = Төсөл дэмжигдээгүй метаөгөгдлийн түлхүүр агуулж байна: { $keys }

## Plot strings

plot-1-1000-one-millimetre-sheet = 1:1000 масштабтай үед хуудсан дээрх 1 мм нь газар дээрх 1 метртэй тэнцүү.
plot-1-scale-covers-width-height = 1:{ $scale } · { $width } × { $height } м хамарна
plot-all-visible-data = Бүх харагдаж буй өгөгдөл
plot-automatic-grid-interval = Торны автомат интервал
plot-border = Хүрээ
plot-centre = Төвлөрүүлэх
plot-choose-smallest-conventional-scale-f = Харагдаж буй бүх зүйлийг хуудсанд багтаах хамгийн бага стандарт масштабыг сонгоно уу.
plot-coordinate-grid = Координатын тор
plot-current-view-centre = Одоогийн харагдацын төв
plot-date = ОГНОО
plot-date-2 = Огноо
plot-dots-per-inch-paper-size = Инч тутмын цэг. Энэ цаасны хэмжээг { $max_dpi } dpi хүртэл растержуулж болно; 300 dpi нь ердийн хэвлэлийн чанар юм.
plot-dpi = dpi
plot-drawing-no = ЗУРГИЙН №
plot-drawing-number = Зургийн дугаар
plot-drawn = ЗУРСАН
plot-drawn-2 = Зурсан
plot-e-g-example-gold-project = ж: Жишээ Алтны Төсөл
plot-entered-coordinates = Оруулсан координат
plot-export-png = PNG экспортлох...
plot-fit-scale-visible-data = Масштабыг харагдаж буй өгөгдөлд тааруулах
plot-grid-interval = Торны интервал
plot-landscape = Хэвтээ
plot-lists-visible-surfaces-design-layers = Харагдаж буй гадаргуу болон зураг төслийн давхаргуудыг тэдгээрийн өнгөтэй нь жагсаана.
plot-margin = Захын зай
plot-margins-leave-no-room-map = Захын зай нь газрын зурагт зай үлдээхгүй байна
plot-metres-scale-1-scale = метр    Масштаб 1:{ $scale }
plot-mm = мм
plot-north-arrow = Хойд зүг заагч
plot-nothing-visible-draw = Зурах юу ч харагдахгүй байна
plot-paper = Цаас
plot-paper-orientation-width-height-mm = { $paper } { $orientation } · { $width } × { $height } мм
plot-paper-size = Цаасны хэмжээ
plot-pick-interval-reads-roughly-every = Хэвлэсэн хуудсан дээр ойролцоогоор 50 мм тутамд уншигдах интервал сонгоно уу.
plot-plan = Төлөвлөгөө
plot-plot-scale-must-positive-number = Зургийн масштаб эерэг тоо байх ёстой
plot-png-written-sheet-s-exact = PNG нь хуудасны яг цаасны хэмжээгээр бичигдэж, DPI-г нь тэмдэглэдэг тул бодит масштабаар хэвлэгдэнэ.
plot-portrait = Босоо
plot-resolution = Нарийвчлал
plot-rev = ХУВИЛБАР
plot-revision = Хувилбар
plot-scale = МАСШТАБ
plot-scale-1 = Масштаб  1:
plot-scale-framing = Масштаб ба хүрээ
plot-sheet-furniture = Хуудасны хүрээлэн орчин
plot-size-width-height-mm = { $size } ({ $width } × { $height } мм)
plot-subtitle = Дэд гарчиг
plot-title = Гарчиг
plot-title-block = Гарчгийн хайрцаг
plot-today = өнөөдөр

## Products strings

products-add-initiation = Дэлбэлгээ эхлүүлэх цэг нэмэх
products-delay = Саатал
products-delay-palette = Саатлын палитр
products-how-long-after-shot-fired = Тэсэлгээ эхэлснээс хойш энэ амсар хэдий хугацааны дараа дэлбэрэхийг заана.
products-initiation-name = Дэлбэлгээ эхлүүлэх · { $name }
products-milliseconds-between-one-hole-firing = Нэг цооног дэлбэрснээс дараагийнх хүртэлх миллисекундийн хугацаа.
products-ms = мс
products-no-products = Бүтээгдэхүүн алга
products-remove = Устгах
products-update = Шинэчлэх

## Progress strings

progress-percent-done-total = { $percent } ({ $total }-с { $done })
progress-task-finished = { $task }: Дууссан

## Project strings

project-item = Зүйл

## Properties strings

properties-adds-view-dependent-rim-highlight = Блок болон материалын хилд харагдацаас хамаарсан ирмэгийн тодруулга нэмнэ. Үүнийг унтраавал эзлэхүүн дүрслэлийн ажлыг бага зэрэг хөнгөвчилнө.
properties-block-model-downscale = Блокийн загварыг багасгах
properties-camera = Камер
properties-camera-clip-planes = Камерын огтлолын хавтгайнууд
properties-cap-while-resizing = Хэмжээ өөрчлөхөд хязгаарлах
properties-dark-mode = Харанхуй горим
properties-developer = Хөгжүүлэгч
properties-downscale-rasters = Растерыг багасгах
properties-edit-object = Объект засах...
properties-field-view = Харах өнцөг
properties-fps = FPS
properties-frame-counter = Фрэймийн тоолуур
properties-frame-rate-cap = Фрэйм хурдны дээд хязгаар
properties-hz = Гц
properties-interface = Интерфейс
properties-invert-horizontal = Хэвтээгээр урвуулах
properties-invert-vertical = Босоогоор урвуулах
properties-limits-newly-loaded-geotiff-previews = Шинээр ачаалагдсан GeoTIFF урьдчилан харах зургийг хамгийн урт талдаа 4096 пиксел хүртэл хязгаарлана. GPU-н текстурын хязгаар хүртэл бүрэн нарийвчлал ашиглахын тулд унтраана уу, энэ нь илүү санах ой ашиглана.
properties-line-colour = Шугамын өнгө
properties-look-sensitivity = Харцны мэдрэмж
properties-max-clip-span = Огтлолын дээд урт
properties-move-layer = Давхарга руу шилжүүлэх...
properties-near-clip-limit = Ойрын огтлолын хязгаар
properties-orbit-sensitivity = Эргэлтийн мэдрэмж
properties-panel-chrome = Самбарын дизайн
properties-performance = Гүйцэтгэл
properties-plan-mode = Төлөвлөгөөний горим
properties-presents-step-display-no-tearing = Дэлгэцтэй нийцүүлэн үзүүлнэ: хагарал үгүй бөгөөд дэлгэц фрэймийн хурдыг тогтооно. Унтраасан үед фрэймүүд зурагдмагцаа шууд харагдах бөгөөд доорх хязгаарлалт хэрэгжинэ.
properties-reflective-block-edges = Тусгалтай блокийн ирмэг
properties-restore-defaults-2 = Үндсэн тохиргоог сэргээх
properties-show-console = Консол харуулах
properties-shows-live-near-far-projection = Төлөв байдлын мөрөнд шууд ойр ба хол проекцийн зайг харуулна.
properties-snap-polling = Наалдацыг шалгах
properties-vertical-sync = Босоо синхрончлол
properties-world-axis-gizmo = Дэлхийн тэнхлэгийн гизмо
properties-zoom-cursor = Заагч руу томруулах
properties-zoom-sensitivity = Томруулах мэдрэмж

## Screenshot strings

screenshot-could-not-encode-viewport-image = Харагдах цонхны зургийг кодчилж чадсангүй: { $error }
screenshot-could-not-map-viewport-screenshot = Харагдах цонхны дэлгэцийн зургийг зурагдуулж чадсангүй: { $error }
screenshot-could-not-save-viewport-image = { $path } харагдах цонхны зургийг хадгалж чадсангүй: { $error }
screenshot-downloaded-viewport-image-file-name = Харагдах цонхны зургийг татаж авлаа: { $file_name }
screenshot-saved-viewport-image-path = Харагдах цонхны зургийг хадгаллаа: { $path }
screenshot-viewport-image-download-failed-error = Харагдах цонхны зургийг татаж авах амжилтгүй боллоо: { $error }

## Spatial strings

spatial-bvh-face-index-index-out = BVH-н { $index } талын индекс тор доторх хязгаараас гарсан байна; доройтсон гурвалжинг орлуулж байна

## State strings

state-above = дээш буюу тэнцүү
state-activate-project = Төслийг идэвхжүүлэх
state-all-open-incline-design-data = Incline Design-д нээлттэй байгаа бүх өгөгдөл
state-apply-generated-rings = Үүсгэсэн цагирагийг хэрэглэх
state-apply-selection = Сонголтод хэрэглэх
state-azimuth-azimuth-dip-dip = { $azimuth }° азимут, { $dip }° уналттай
state-azimuth-azimuth-dip-dip-2 = { $azimuth }° азимут, { $dip }° уналт руу
state-below = доош буюу тэнцүү
state-centre-rotation = Эргэлтийн төв
state-checking-unsaved-work = Хадгалаагүй ажлыг шалгаж байна
state-choose-destination = Хүлээн авагчийг сонгох
state-choose-one-more-files = Нэг буюу түүнээс олон файл сонгох
state-clear-raster = Растерыг цэвэрлэх
state-click-pit-shell-viewport = Харагдах цонхон дахь карьерийн бүрхүүлийг товшино уу.
state-click-pit-stockpile-solid-viewport = Харагдах цонхон дахь карьер эсвэл овоолгын хатуу биеийг товшино уу.
state-click-surface-viewport = Харагдах цонхон дахь гадаргууг товшино уу.
state-click-topology-viewport = Харагдах цонхон дахь топологийг товшино уу.
state-close-project = Төслийг хаах
state-colour-drillholes = Цооногийг өнгөөр ялгах
state-copy-objects-layer = Объектыг давхарга руу хуулах
state-count-file-s = { $count } файл
state-count-object-s-axis-value = { $count } объект · { $axis } { $value }
state-count-object-s-closed = { $count } объект · { $closed }
state-count-object-s-layer = { $count } объект · { $layer }
state-count-object-s-weight = { $count } объект · { $weight }
state-count-object-s-z-elevation = { $count } объект · Z { $elevation }
state-create-point-cloud-tin = Цэгэн үүлээс TIN үүсгэх
state-create-project = Төсөл үүсгэх
state-current-project = Одоогийн төсөл
state-cut-topology-pit-shell = Топологийг карьерийн бүрхүүлээр тайрах
state-cut-triangulation-polyline = Триангуляцыг полилиниагаар тайрах
state-cut-triangulation-z = Триангуляцыг Z-ээр тайрах
state-dark-mode = Харанхуй горим
state-detached = Салгасан
state-disabled = Идэвхгүй
state-discard-project-changes = Төслийн өөрчлөлтийг үл хэрэгсэх
state-discard-replace-project = Үл хэрэгсэн, төслийг солих
state-discarding-unsaved-changes = Хадгалаагүй өөрчлөлтийг үл хэрэгсэж байна
state-docked = Тогтоосон
state-drape-raster = Растерыг дараах
state-drill-pattern = Өрмийн сүлжээ
state-duplicate-layer = Давхарга хувилах
state-east = Зүүн
state-enabled = Идэвхтэй
state-exit-incline-design = Incline Design-аас гарах
state-export-block-model-csv = Блокийн загварыг CSV болгон экспортлох
state-export-layer-dxf = Давхаргыг DXF болгон экспортлох
state-export-omf = OMF экспортлох
state-export-project-dxf = Төслийг DXF болгон экспортлох
state-export-triangulation = Триангуляц экспортлох
state-export-viewport-image = Харагдах цонхны зургийг экспортлох
state-finish-closed-polyline = Хаалттай полилиниаг дуусгах
state-finish-open-polyline = Задгай полилиниаг дуусгах
state-fit-extents = Хэмжээнд тааруулах
state-fix-release-centre-both-views = Хоёр харагдац эргэдэг төвийг тогтоох эсвэл суллах
state-generate-contours = Изолиниа үүсгэх
state-hidden = Нуугдсан
state-import-drillholes = Цооног импортлох
state-import-omf = OMF импортлох
state-import-point-cloud = Цэгэн үүл импортлох
state-import-raster = Растер импортлох
state-import-triangulation = Триангуляц импортлох
state-insert-intersection-points = Огтлолцлын цэгүүдийг оруулах
state-insert-points-elevation = Өндөрлөг дээр цэгүүд оруулах
state-keep-inside = Дотор талыг хадгалах
state-keep-outside = Гадна талыг хадгалах
state-kriged-block-model = Кригинг хийсэн блокийн загвар
state-load-block-model = Блокийн загвар ачаалах
state-load-drillholes = Цооног ачаалах
state-load-layer = Давхарга ачаалах
state-load-point-cloud = Цэгэн үүл ачаалах
state-load-raster = Растер ачаалах
state-load-triangulation = Триангуляц ачаалах
state-locked-count-object-s = { $count } объектыг түгжлээ
state-major-major-minor-minor = Гол { $major } · туслах { $minor }
state-move-axis-value = Тэнхлэгийн утга руу шилжүүлэх
state-move-objects-layer = Объектыг давхарга руу шилжүүлэх
state-name-count-holes = { $name } · { $count } цооног
state-name-count-object-s = { $name } · { $count } объект
state-name-z-min-z-max = { $name } · { $z_min }-с { $z_max } хүртэл
state-next-edit = Дараагийн засвар
state-north = Хойд
state-open-containing-folder = Агуулсан хавтсыг нээх
state-open-project = Төсөл нээх
state-preserve-view-angle = Харагдацын өнцгийг хадгалах
state-previous-edit = Өмнөх засвар
state-project-id = Төсөл { $id }
state-remove-block-model = Блокийн загвар устгах
state-remove-drillholes = Цооног устгах
state-remove-point-cloud = Цэгэн үүл устгах
state-remove-raster = Растер устгах
state-remove-triangulation = Триангуляц устгах
state-removed-from-active-triangulation = Идэвхтэй триангуляцаас устгагдсан
state-removed-from-every-triangulation = Бүх триангуляцаас устгагдсан
state-rename-kind = { $kind }-ийг нэр өөрчлөх
state-save-close-project = Хадгалаад төслийг хаах
state-save-despite-unsupported-content = Дэмжигдээгүй агуулгыг үл харгалзан хадгалах
state-save-project = Төслийг өөр нэрээр хадгалах
state-save-replace-project = Хадгалаад төслийг солих
state-saving-current-project = Одоогийн төслийг хадгалж байна
state-section-section = { $section } хэсэг
state-select-layer-objects = Давхаргын объектуудыг сонгох
state-selected-objects = Сонгосон объектууд
state-selected-polylines = Сонгосон полилиниа
state-selected-scene-elements = Сонгосон дүрслэлийн элементүүд
state-set-block-model-variable = Блокийн загварын хувьсагчийг тохируулах
state-set-drillhole-colour-preset = Цооногийн өнгөний бэлэн тохиргоог тохируулах
state-set-entity-lock = Объектын түгжээг тохируулах
state-set-grid = Тор тохируулах
state-set-layer-lock = Давхаргын түгжээг тохируулах
state-set-line-weight = Шугамын зузааныг тохируулах
state-set-object-colour = Объектын өнгийг тохируулах
state-set-object-fill = Объектын дүүргэлтийг тохируулах
state-set-point-visibility = Цэгийн харагдацыг тохируулах
state-set-polyline-closed = Полилиниаг хаалттай болгож тохируулах
state-set-raster-lock = Растерын түгжээг тохируулах
state-set-standard-view = Стандарт харагдацыг тохируулах
state-set-topology-wireframes = Топологийн торон дүрсийг тохируулах
state-set-triangulation-colour = Триангуляцын өнгийг тохируулах
state-show-console = Консол харуулах
state-show-project = Төслийг харуулах
state-shown = Харагдаж байна
state-slice-mode = Огтлолын горим
state-slice-preview = Огтлолын урьдчилсан харагдац
state-south = Өмнөд
state-stem-contours = { $stem } изолиниа
state-target-new-name = { $target }-с «{ $new_name }» болгох
state-trim-above = Дээрхийг тайрах
state-trim-below = Доорхыг тайрах
state-trim-triangulation-surface = Триангуляцыг гадаргуугаар тайрах
state-undrape-raster = Растерыг дараахаас цуцлах
state-undrape-rasters = Растеруудыг дараахаас цуцлах
state-unload-block-model = Блокийн загварыг буулгах
state-unload-drillholes = Цооногуудыг буулгах
state-unload-layer = Давхаргыг буулгах
state-unload-point-cloud = Цэгэн үүлийг буулгах
state-unload-raster = Растерыг буулгах
state-unload-triangulation = Триангуляцыг буулгах
state-untitled-project = Нэргүй төсөл
state-use-typed-radius = Бичсэн радиусыг ашиглах
state-west = Баруун

## Status strings

status-clip-near-far = Огтлолын ойр/хол/Δ: -- / -- / --
status-frame-rate = Фрэйм хурд

## Text strings

text-could-not-build-vector-mesh = { $font } фонт, { $glyph } тэмдэгтийн вектор тор бүтээж чадсангүй: { $error }
text-document-text-mesh-exceeded-its = Баримтын текст тор u32 индексийн мужаас хэтэрлээ

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = Эхлээд холбох цооногийн өгөгдлийн багцыг сонгоно уу
tie-in-count-connector-s = { $count } холболт
tie-in-delete-tie-ins = Холболтуудыг устгах
tie-in-deleted-count-selected-tie-connector = Сонгосон { $count } холболтыг устгалаа
tie-in-hole = цооног
tie-in-initiation-point-lifted-from-name = Дэлбэлгээ эхлүүлэх цэгийг { $name }-с авлаа
tie-in-initiation-point-set-name-delay = Дэлбэлгээ эхлүүлэх цэгийг { $name } дээр { $delay } мс саатлаар тохируулав
tie-in-select-delay-product-palette-before = Цооногуудыг холбохоос өмнө палитраас саатлын бүтээгдэхүүн сонгоно уу
tie-in-tied-count-connector-s-delay = { $product }-р { $delay } мс саатлаар { $count } холболт үүсгэлээ
tie-in-tied-count-connector-s-delay-2 = { $product }-р { $delay } мс саатлаар { $count } холболт үүсгэлээ, { $replaced }-г сольсон

## Toolbar strings

toolbar-fill-type = Дүүргэлтийн төрөл

## Toolbars strings

toolbars-auto-bench = Авто-уступ
toolbars-bezier-polyline = Безье полилиниа
toolbars-chamfer-polyline-corners = Полилиниагийн буланг фасклах
toolbars-create-text = Текст үүсгэх
toolbars-cursor-regular = Заагч: Энгийн
toolbars-cursor-snap-line = Заагч: Шугамд наах
toolbars-cursor-snap-point = Заагч: Цэгт наах
toolbars-cursor-snap-surface = Заагч: Гадаргуунд наах
toolbars-delete-points = Цэгүүдийг устгах
toolbars-explode-polyline-lines = Полилиниаг шугам болгон задлах
toolbars-fuse-polylines = Полилиниаг нэгтгэх
toolbars-measure-distance = Зай хэмжих
toolbars-new-layer = Шинэ давхарга
toolbars-split-polyline-points = Полилиниаг цэгээр хуваах
toolbars-strike-dip = Чиг ба уналт
toolbars-tool-not-available-section-view = { $tool } - огтлолын харагдацад боломжгүй

## Tri strings

tri-adaptive-concentrates-vertices-compl = Уян хатан арга нь хавтгайд тохируулах алдаагаар нарийн төвөгтэй рельеф дээр орой цэгийг төвлөрүүлдэг; жигд арга тэдгээрийг жигд тараана. Ирээдүйд илүү олон арга нэмэгдэж болно.
tri-adaptive-quadtree = Уян хатан (quadtree)
tri-axis-range = { $axis } хязгаар
tri-base-topology-will-receive-pit = Карьер эсвэл овоолгын хэлбэрийг хүлээн авах суурь топологи.
tri-boundary-polyline = Хилийн полилиниа
tri-bridge-gaps-boundary-concavities-nar = Гадаргуугийн хөндлөн энэ хэмжээнээс нарийн завсар болон хилийн хотгорыг холбоно. 0 нь ойролцоогоор түүврийн эсийн хэмжээ хүртэлх завсрыг холбосон хэвээр байна; том утга илүү том нүхийг дүүргэж хилийн хотгорыг элэгдүүлнэ.
tri-budget = Төсвийг дараах байдлаар
tri-cancel-pick = Сонголтыг цуцлах
tri-candidate-detail = Нэр дэвшигчийн нарийвчлал
tri-candidate-fine-cells-per-budgeted = Төсөвлөгдсөн орой цэг бүрт ногдох нэр дэвшигч нарийн эсийн тоо. Өндөр утга уян хатан түүврлэгчид нарийвчлал байрлуулах илүү эрх чөлөө өгдөг ч бүтэхэд удаан байдаг.
tri-cap-surface-share-source-points = Гадаргууг эх цэгүүдийн эзлэх хувиар эсвэл орой цэгийн тодорхой тоогоор хязгаарлана.
tri-choose-input-clicking-loaded-surface = Харагдах цонхон дахь ачаалагдсан гадаргуу дээр товшиж энэ оролтыг сонгоно уу
tri-choose-which-side-reference-topology = Хамтын XY талбайн доторх лавлагаа топологийн аль талыг гадаргуунаас хасахыг сонгоно уу.
tri-clip = Огтлох
tri-clip-creates-new-triangulation-name = Огтлолт нь энэ нэртэй шинэ триангуляц үүсгэнэ; эх гадаргуу өөрчлөгдөхгүй.
tri-clip-surface-polyline = Гадаргууг полилиниагаар огтлох
tri-closed-pit-stockpile-solid-whose = Ил гарсан хил нь үр дүнд орох хаалттай карьер эсвэл овоолгын хатуу бие.
tri-create-new-layer-contours-append = Изолиниад зориулж шинэ давхарга үүсгэх, эсвэл идэвхтэй төслийн байгаа давхаргад нэмэх.
tri-cut-topology-pit-shell = Топологийг карьерийн бүрхүүлээр тайрах
tri-e-g-design-trimmed = ж: design_trimmed
tri-e-g-mysurf-cut = ж: mysurf_cut
tri-e-g-mysurf-slice = ж: mysurf_slice
tri-e-g-surface-contour = ж: surface_contour
tri-e-g-topo-cut = ж: topo_cut
tri-e-g-topo-pit = ж: topo_with_pit
tri-exact-number-surface-vertices-target = Зорьж буй гадаргуугийн орой цэгийн тодорхой тоо. Хэт өндөр утга удаан бүтэж, ихээхэн санах ой ашиглана.
tri-existing-ground-topology-will-cut = Карьерийн бүрхүүлээр тайрагдах одоо байгаа газрын гадаргын топологи.
tri-fill-holes-up = Дараах хүртэлх нүхийг дүүргэх
tri-generate = Үүсгэх
tri-generate-contour-lines = Изолиниа үүсгэх
tri-generate-upper-surface = Дээд гадаргуу үүсгэх
tri-hide-unload-sources = Нуугаад эх сурвалжийг буулгах
tri-higher-edge-will-enforced-each = Зөрчил бүрт өндөр ирмэг давамгайлна. Доод зөрчилдсөн сегментүүдийг эвдрэлийн шугам болгон үл хэрэгсэх бөгөөд гадаргуу тэдгээр хэсгээр интерполяцлана. Эх полилиниа өөрчлөгдөхгүй.
tri-highlighted-breakline-edges-cross-ov = Тодруулсан эвдрэлийн шугамын ирмэгүүд төлөвлөгөөнд өөр өөр өндөрлөгт огтлолцож эсвэл давхцаж байна. Нэг рельефийн гадаргуу хоёуланг нь дагаж чадахгүй.
tri-intervals-colours = Интервал ба өнгө
tri-keep-clipped-topology-included-shape = Огтолсон топологи болон оруулсан хэлбэрийг нэг объект болгож нэгтгэхийн оронд тусдаа триангуляц болгож хадгалах.
tri-keep-inside-discards-surface-outside = «Дотор талыг хадгалах» нь полилиниагийн гадна талын гадаргууг хаяна. «Гадна талыг хадгалах» нь гадаргуунаас полилиниа хэлбэртэй нүх огтолно.
tri-keeps-only-surface-within-polyline = Зөвхөн полилиниагийн хилийн доторх гадаргууг хадгална.
tri-keeps-surface-relation-topology-with = Гадаргууг топологитой харьцах байдлаар ({ $relation }) түүний XY хамрах хүрээнд хадгална.
tri-layer-already-exists-select-above = Тэр давхарга аль хэдийн байна; дээрээс сонгох эсвэл өөр нэр сонгоно уу.
tri-limit-z-range = Z-ийн мужийг хязгаарлах
tri-major = Гол
tri-max-edge-length = Дээд ирмэгийн урт
tri-merge = Нэгтгэх
tri-method = Арга
tri-min = Доод
tri-minimum-maximum-elevations-retained = Гаралтын гадаргуунд хадгалагдах доод болон дээд өндөрлөг. Доод хэмжээ дээд хэмжээнээс бага байх ёстой.
tri-minor = Туслах
tri-minor-controls-ordinary-contours-maj = «Туслах» энгийн изолиниаг удирдана. «Гол» онцлон харуулах изолиниаг удирдах бөгөөд Туслахаас багагүй интервал ашиглах ёстой.
tri-move-cursor-over-loaded-surface = Заагчийг ачаалагдсан гадаргуу дээгүүр аваачна уу.
tri-name-assigned-elevation-clipped-outp = Өндөрлөгөөр огтлогдсон гаралтын гадаргуунд оноох нэр.
tri-name-assigned-merged-topology-pit = Нэгтгэсэн топологи ба карьер/овоолгын үр дүнд оноох нэр.
tri-name-assigned-newly-created-contour = Шинээр үүсгэсэн изолиниагийн давхаргад оноох нэр.
tri-name-assigned-reconstructed-triangul = Дахин байгуулсан триангуляцад оноох нэр.
tri-name-assigned-topology-after-pit = Карьерийн бүрхүүлийг тайрсны дараах топологид оноох нэр.
tri-name-assigned-trimmed-output-surface = Тайрагдсан гаралтын гадаргуунд оноох нэр.
tri-nearby-breakline-vertices-do-not = Ойролцоох эвдрэлийн шугамын орой цэгүүд яг ижил байрлалд тохирохгүй байгаа тул гадаргууг триангуляцлах боломжгүй.
tri-new-layer = Шинэ давхарга
tri-new-layer-name = Давхаргын шинэ нэр
tri-once-merge-succeeds-unload-source = Нэгтгэлт амжилттай болмогц эх топологи болон хатуу биеийг буулгаж, зөвхөн нэгтгэсэн үр дүнг дүрслэлд үлдээх.
tri-only-loaded-triangulations-can-picke = Зөвхөн ачаалагдсан триангуляцыг сонгож болно.
tri-operation = Үйлдэл
tri-output-layer = Гаралтын давхарга
tri-percentage = Хувь
tri-percentage-cloud = Үүлний хувь
tri-pick-from-view = Харагдацаас сонгох
tri-pit-design-surface-only-areas = Карьерийн зураг төслийн гадаргуу. Топологиос доогуур ухагдах хэсгүүдийг л огтлолтод ашиглана.
tri-pit-shell = Карьерийн бүрхүүл
tri-pit-stockpile-solid = Карьер/овоолгын хатуу бие
tri-recommended-weld-retry = Санал болгох: Гагнаад дахин оролдох
tri-reconstruct-triangulated-terrain-sur = Цэгэн үүлээс триангуляцлагдсан рельефийн гадаргуу дахин байгуулна. Уян хатан түүврлэгч орой цэгийн төсвийг газар хамгийн нарийн төвөгтэй хэсэгт зарцуулж, тэгш хэсгийг сийрэг хэвээр үлдээнэ.
tri-reduce-budget-candidate-detail-if = Хэрэв таны компьютерт RAM бага байвал төсөв эсвэл нэр дэвшигчийн нарийвчлалыг багасгана уу.
tri-reference-topology-defines-where-oth = Нөгөө гадаргуу хаана тайрагдахыг тодорхойлдог лавлагаа топологи.
tri-reject-reconstructed-triangle-edges = Энэ зайнаас урт дахин байгуулсан гурвалжны ирмэгийг татгалзана. Ирмэгийн уртад хязгаар тавихгүй бол 0 ашиглана уу.
tri-removes-surface-within-polyline-boun = Полилиниагийн хилийн доторх гадаргууг устгаж, үлдсэнийг хадгална.
tri-removes-topology-where-pit-shell = Карьерийн бүрхүүл доогуур ухсан газарт топологийг устгаж, бүрхүүл нүхийг дүүргэнэ. Холбоос нь гадаргуудын хоорондох бодит 3D харьцах шугамыг дагана; бүрхүүлийн газрын түвшнээс дээш гарсан хэсгийн доорх топологи хадгалагдана.
tri-result = Үр дүн
tri-save-two-entities = Хоёр тусдаа объект болгон хадгалах
tri-select = Сонгох…
tri-share-source-points-keep-fractions = Хадгалах эх цэгийн эзлэх хувь. 0.125% гэх мэт бутархай утга зөвшөөрөгдөнө.
tri-slice-triangulation-z-range = Триангуляцыг Z мужаар огтлох
tri-solution-generate-upper-surface = Шийдэл: Дээд гадаргуу үүсгэх
tri-surface-trim = Тайрах гадаргуу
tri-surface-will-changed-selected-topolo = Өөрчлөгдөх гадаргуу; сонгосон топологи өөрчлөгдөлгүй үлдэнэ.
tri-text = %
tri-topology = Топологи
tri-triangulation-failed = Триангуляц амжилтгүй боллоо
tri-trim = Тайрах
tri-trim-topology = Топологиор тайрах
tri-uniform-grid = Жигд тор
tri-up-target-point-count-points = { $point_count } цэгийн дотроос { $target } хүртэл нь гадаргуугийн орой цэг болно ({ $percent }%).
tri-use-full-surface-elevation-range = Гадаргуугийн бүх өндрийн мужийг ашиглах
tri-vertex-count = Орой цэгийн тоо
tri-vertices-within-5-cm-xy = XY болон Z чиглэлд 5 см-ийн дотор байгаа орой цэгүүд энэ триангуляцын хувьд нэг байрлалыг хуваалцана. Энэ нь үүсгэсэн гадаргууг орон нутгийн хэмжээнд 5 см хүртэл шилжүүлж болно; эх полилиниа өөрчлөгдөхгүй.
tri-weld-retry = Гагнаад дахин оролдох
tri-when-enabled-generate-contours-only = Идэвхжүүлсэн үед зөвхөн заасан доод болон дээд өндөрлөгийн хооронд изолиниа үүсгэнэ.

## Ui strings

ui-choose-offset-side = Шилжилтийн талыг сонгох
ui-choose-relimit-side = Хязгаарлах талыг сонгох
ui-click-circle-centre = Тойргийн төвийг товших
ui-click-closed-polyline-use-blast = Тэсэлгээний хэлбэр болгон ашиглах хаалттай полилиниа дээр товшино уу
ui-click-collar-add-edit-initiation = Дэлбэлгээ эхлүүлэх цэгийг нэмэх эсвэл засахын тулд амсар дээр товшино уу
ui-click-first-point-slice-line = Огтлолын шугамын эхний цэгийг товших
ui-click-first-vertex = Эхний орой цэгийг товших
ui-click-perimeter-point-type-radius = Периметрийн цэг дээр товших эсвэл радиус оруулна уу
ui-click-second-point-slice-line = Огтлолын шугамын хоёр дахь цэгийг товших
ui-click-second-vertex = Хоёр дахь орой цэгийг товших
ui-click-use-pointer-radius = эсвэл заагчийн радиусыг ашиглахын тулд товшино уу
ui-could-not-copy-text-browser = Текстийг хөтчийн санах ойд хуулж чадсангүй: { $error }
ui-dip-horizontal-no-strike = { $dip } (хэвтээ, чиггүй)
ui-distance-meters = { $distance } метр
ui-drag-ring-type-azimuth-dip = Цагирагийг чирэх эсвэл азимут ба уналтыг бичих
ui-each-hole-turns-about-its = цооног бүр өөрийн амсрын эргэн тойронд эргэнэ
ui-enter-positive-decimal-radius = Эерэг бутархай радиус оруулна уу
ui-esc-cancels = Esc цуцлана
ui-no-delay-product-tie = Холбох саатлын бүтээгдэхүүн алга
ui-press-enter-use-typed-radius = Бичсэн радиусыг ашиглахын тулд Enter дарах
ui-right-click-delay-palette-heading = саатлын палитрын гарчиг дээр хулганы баруун товчоор товшиж нэмнэ үү
ui-select-designs = Зураг төслүүдийг сонгох
ui-select-drill-hole = Цооног сонгох
ui-select-endpoint-join = Холбох үзүүр цэгийг сонгох
ui-select-first-crest-toe-point = Эхний оргил/ёроолын цэгийг сонгох
ui-select-item = Зүйл сонгох
ui-select-line-fuse = Нэгтгэх шугамыг сонгох
ui-select-line-polyline = Шугам эсвэл полилиниа сонгох
ui-select-line-relimit = Хязгаарлах шугамыг сонгох
ui-select-next-line-fuse = Нэгтгэх дараагийн шугамыг сонгох
ui-select-opposite-berm-point = Бермийн эсрэг цэгийг сонгох
ui-select-point = Цэг сонгох
ui-select-polyline = Полилиниа сонгох
ui-select-polyline-open-line = Полилиниа эсвэл задгай шугам сонгох
ui-select-polyline-vertex = Полилиниагийн орой цэг сонгох
ui-select-second-crest-toe-point = Хоёр дахь оргил/ёроолын цэгийг сонгох
ui-select-second-split-point = Хоёр дахь хуваах цэгийг сонгох
ui-select-split-point = Хуваах цэг сонгох
ui-select-topologies = Топологи сонгох
ui-slice-view = Огтлолын харагдац
ui-strike-strike-dip = { $strike }° чиг · { $dip }
ui-value-dip = { $value }° уналт

## Viewport strings

viewport-all-total-categories-keep-their = Бүх { $total } ангилал өнгөө хадгална; зөвхөн эхний { $shown } нь тодорхой ялгаатай дүрслэгдэнэ
viewport-axis-maximum = { $axis } дээд
viewport-axis-minimum = { $axis } доод
viewport-bar-blast-timeline-placeholder = Тэсэлгээний хугацааны шугам [ТОДОРХОЙГҮЙ]
viewport-bar-burden-relief-heatmap-placeholder = Бурдений хөнгөрөлтийн дулааны зураг [ТОДОРХОЙГҮЙ]
viewport-bar-color = Өнгө:
viewport-bar-contours-equal-time-placeholder = Тэнцүү хугацааны изолиниуд [ТОДОРХОЙГҮЙ]
viewport-bar-disable-flying-mode = Нисэх горимыг унтраах
viewport-bar-disable-x-ray-vision = Рентген харагдацыг унтраах
viewport-bar-drill-holes = Цооногууд:
viewport-bar-enable-flying-mode = Нисэх горимыг асаах
viewport-bar-enable-x-ray-vision = Рентген харагдацыг асаах
viewport-bar-exit-slice-view = Огтлолын харагдацаас гарах
viewport-bar-fill = Дүүргэлт:
viewport-bar-fix-centre-rotation = Эргэлтийн төвийг тогтоох
viewport-bar-hide-points = Цэгүүдийг нуух
viewport-bar-hide-rl-grid = RL торыг нуух
viewport-bar-hide-wireframes = Торон дүрсийг нуух
viewport-bar-hide-xy-grid = XY торыг нуух
viewport-bar-release-centre-rotation = Эргэлтийн төвийг суллах
viewport-bar-show-points = Цэгүүдийг харуулах
viewport-bar-show-rl-grid = RL торыг харуулах
viewport-bar-show-wireframes = Торон дүрсийг харуулах
viewport-bar-show-xy-grid = XY торыг харуулах
viewport-bar-vertical-slice-view = Босоо огтлолын харагдац
viewport-blank = (хоосон)
viewport-choose-active-block-model-variable = Идэвхтэй блокийн загварын хувьсагчийг сонгох
viewport-choose-variable = Хувьсагч сонгох
viewport-click-edit-color-right-click = Товшиж өнгийг засах; баруун товшиж устгах
viewport-click-type-boundary-s-value = Энэ хилийн утгыг бичихийн тулд товшино уу
viewport-colour-mapping = Өнгийн харгалзаа
viewport-count-categories = { $count } ангилал
viewport-count-category = { $count } ангилал
viewport-double-click-add-boundary-here = Энд хил нэмэхийн тулд давхар товшино уу
viewport-drag-move-middle-click-toggles = Чирж зөөх · Дунд товшиж ≤ солих
viewport-drag-move-right-click-remove = Чирж зөөх · Баруун товшиж устгах · Дунд товшиж ≤ солих
viewport-e = E
viewport-edit-category-colour = Энэ ангиллын өнгийг засах
viewport-edit-colour-used-empty-values = Хоосон утганд ашиглах өнгийг засах
viewport-empty = (хоосон)
viewport-empty-hidden = (хоосон · нуугдсан)
viewport-filter-variables = Хувьсагчийг шүүх
viewport-middle-drag-pan-scroll-zoom = Дунд товч чирж зөөх · Гүйлгэж томруулах
viewport-middle-drag-pan-scroll-zoom-2 = Дунд товч чирж зөөх · Гүйлгэж томруулах · Товшиж салгах
viewport-n = N
viewport-no-data-variable = Энэ хувьсагчийн өгөгдөл алга
viewport-no-matches = Тохирол алга
viewport-no-usable-range = (ашиглах боломжтой муж алга)
viewport-rebuild-variable-s-colours-from = Энэ хувьсагчийн өнгийг өгөгдлөөс нь дахин байгуулах
viewport-reset = Сэргээх
viewport-restore-full-model-range = Загварын бүтэн мужийг сэргээх
