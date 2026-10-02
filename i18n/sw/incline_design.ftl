# Kiswahili. Maandishi yasiyopatikana hutumia katalogi ya Kiingereza kama akiba.

## Shared

common-cancel = Ghairi
common-clear = Futa
common-close = Funga
common-fill = Jaza
common-set = Weka

## Status bar

status-language = Lugha

## Menu bar — File

menu-file = Faili
menu-file-save-project = Hifadhi Mradi
menu-file-save-project-as = Hifadhi Mradi Kama...
menu-file-new-project = Mradi Mpya...
menu-file-open-project = Fungua Mradi...
menu-file-open-recent = Fungua wa Hivi Karibuni
menu-file-show-in-explorer = Onyesha kwenye Kivinjari cha Faili
menu-file-show-in-folder = Fungua Folda Inayohusika
menu-file-import = Ingiza...
menu-file-export = Hamisha...
menu-file-export-viewport-image = Hamisha Picha ya Mwonekano...
menu-file-export-engineering-drawing = Hamisha Mchoro wa Uhandisi...
menu-file-about = Kuhusu { $app }...
menu-file-exit = Toka Programu

## Menu bar — View

menu-view = Mwonekano

## Workspaces

ws-production = Uzalishaji
ws-drill-and-blast = Uchimbaji na Ulipuaji
ws-geology = Jiolojia
ws-planning = Upangaji

## Menubars

ws-menubar-design = Muundo
ws-menubar-triangulation = Utatuzi (Triangulation)
ws-menubar-raster = Rasta
ws-menubar-point-cloud = Wingu la Vidokezo
ws-menubar-block-model = Mfano wa Vitalu
ws-menubar-drillholes = Mashimo ya Uchimbaji
ws-menubar-active-layer = Tabaka:

## Menubars functions

ws-menubar-design-insert-point = Ingiza Kidokezo
ws-menubar-design-insert-point-at-intersection = Kwenye makutano
ws-menubar-design-insert-point-at-elevation = Kwenye kimo
ws-menubar-design-move-to = Hamishia
ws-menubar-design-create-triangulation = Unda Utatuzi

## Rename / delete item dialogs

# { $kind } is a workspace noun from the ws-production-* set above.
dialog-rename-title = Badilisha Jina la { $kind }
dialog-rename-field = Jina jipya
dialog-rename-field-hint = Inahitajika
dialog-rename-submit = Badilisha Jina
dialog-delete-title = Futa { $kind }
dialog-delete-confirm =
    Futa '{ $name }' kutoka kwenye mradi?
    Hatua hii haiwezi kutenduliwa.
confirm-delete-product =
    Futa bidhaa '{ $name }' kutoka kwenye pala?
    Hatua hii haiwezi kutenduliwa.

## Create Triangulation dialog

tri-create-title = Unda Utatuzi
tri-create-help = Inatatua vitu vilivyochaguliwa wakati dirisha hili lilipofunguliwa. Lifunge ili kubadilisha uteuzi.
tri-create-type-label = Aina ya utatuzi
tri-create-type-help =
    Uso wazi huunda karatasi ya mtindo wa ardhi. Kigumu huunda mfumo
    uliofungwa kabisa na kinahitaji chanzo kinachoweza kuunda mpaka usiovuja maji.
tri-create-output-name = Jina la matokeo
tri-create-output-name-help = Jina litakalopewa utatuzi utakaozalishwa.
tri-create-output-name-hint = jina la utatuzi
tri-create-run = Tatua (Triangulate)
tri-selection-none = Vitu vilivyochaguliwa havipatikani tena.

tri-selection-selected = { $summary } vimechaguliwa

tri-type-open-surface = Uso
tri-type-solid-closed = Kigumu

# Selection summary pieces, e.g. "3 polylines, 1 point". Each noun is pluralised
# by its own count so languages with more than two plural forms read correctly.
tri-count-polylines =
    { $count ->
        [one] mstari { $count } wa pointi nyingi
       *[other] mistari { $count } ya pointi nyingi
    }
tri-count-strings =
    { $count ->
        [one] mfuatano { $count }
       *[other] mifuatano { $count }
    }
tri-count-circles =
    { $count ->
        [one] duara { $count }
       *[other] miduara { $count }
    }
tri-count-points =
    { $count ->
        [one] kidokezo { $count }
       *[other] vidokezo { $count }
    }
tri-count-texts =
    { $count ->
        [one] kitu { $count } cha maandishi
       *[other] vitu { $count } vya maandishi
    }
tri-count-objects =
    { $count ->
        [one] kitu { $count }
       *[other] vitu { $count }
    }

about-read-full-licence = Soma leseni kamili ↗
about-source-code = Msimbo Chanzi
about-website = Tovuti
about-title = Kuhusu { $app }
drill-hole-colour-stop = Kituo { $index }
properties-restore-defaults-tooltip = Rejesha mipangilio ya { $heading } kwa chaguo-msingi

## Dynamic UI messages

ui-selected-count = { $count } vimechaguliwa
ui-selected-objects = Vitu { $count } vimechaguliwa
ui-selected-polylines = Mistari { $count } ya pointi nyingi imechaguliwa
ui-invalid-axis-value = Weka thamani sahihi ya mhimili { $axis }.
ui-selection-spans = Uteuzi unaanzia { $min } hadi { $max }.
confirm-delete-count = Una uhakika unataka kufuta vitu { $count } vilivyochaguliwa?
confirm-delete-layer = Futa tabaka '{ $name }' na vitu vyote vilivyomo?
    Hatua hii haiwezi kutenduliwa.
plot-preview-pixels = { $width } × { $height } px kwa { $dpi } dpi
tri-estimated-memory = Kumbukumbu ya juu inayokadiriwa ~{ $estimate }. { $detail }
block-grid-summary = Gridi: { $x } × { $y } × { $z } = vitalu { $count }
status-selected = Vilivyochaguliwa: { $count }
status-faces = Nyuso: { $drawn } / { $total } (vipande { $drawn_chunks }/{ $total_chunks })
status-clip = Kata karibu/mbali/Δ: { $near } / { $far } / { $delta } m
status-points = Vidokezo: { $drawn } / { $target } kati ya { $total } (vipande { $drawn_chunks }/{ $total_chunks })

explorer-no-rasters = Hakuna rasta
slice-viewport-gestures = buruta-katikati sogeza · buruta-kulia zunguka · Shift+gurudumu tembea · W/S hamisha bamba · Q/E zungusha · Esc toka

## Startup environment details

## Renderer startup diagnostics

color-aci = ACI
color-aci-value = ACI { $index }
color-index = Kielezo
color-rgb = RGB
color-opacity = Uwazi
color-edit = Bofya kuhariri rangi
color-saturation-value = Ukolezi na mng'ao
color-hue = Rangi msingi
asset-loading = Inapakia data ya rasilimali
asset-unloading = Inaondoa data ya rasilimali
asset-load-failed = Imeshindwa kupakia data ya rasilimali
asset-unload-failed = Imeshindwa kuondoa data ya rasilimali
preferences-title = Mapendeleo
context-text-colour = Rangi ya maandishi
context-polylines = Mistari ya Pointi Nyingi
context-points = Vidokezo
crs-unknown-ellipsoid = Umbo la dunia "{ $name }" halijulikani katika ufafanuzi huu wa mfumo wa kuratibu.
crs-no-ellipsoid = Ufafanuzi huu wa mfumo wa kuratibu haueleze umbo la dunia linalotumika.
crs-unknown-code = EPSG:{ $code } haipo kwenye orodha ya mifumo ya kuratibu.
crs-transform-failed = Kuratibu haikuweza kubadilishwa; matokeo hayakuwa nafasi kamili.
crs-no-datum-path = Hakuna mabadiliko yaliyochapishwa kati ya fremu za marejeo za { $from } na { $to } (mifumo ya EPSG { $source } na { $target }). Kubadilisha hata hivyo kungekuwa na kosa la kiasi kisichojulikana, kwa hivyo hakuna kilichobadilishwa.
crs-unknown-datum = Fremu ya marejeo ya { $from } au { $to } haiwezi kutambuliwa, na hizo mbili zinatumia miundo tofauti ya dunia. Kubadilisha kati yao kungekuwa na kosa la kiasi kisichojulikana.
ws-survey = Upimaji
survey-count-designs = { $count } { $count ->
    [one] muundo
   *[other] miundo
  }
survey-count-meshes = { $count } { $count ->
    [one] utatuzi
   *[other] utatuzi
  }
survey-count-models = { $count } { $count ->
    [one] mfano wa vitalu
   *[other] mifano ya vitalu
  }
survey-count-clouds = { $count } { $count ->
    [one] wingu la vidokezo
   *[other] mawingu ya vidokezo
  }
survey-count-holes = { $count } { $count ->
    [one] seti ya data ya shimo la uchimbaji
   *[other] seti za data za mashimo ya uchimbaji
  }
survey-count-rasters = { $count } { $count ->
    [one] rasta
   *[other] rasta
  }
survey-angle = Mzunguko kuzunguka Z (kinyume cha saa)
survey-scale = Kigezo cha kipimo sawia cha XYZ
survey-invalid-transform = Vitovu, pembe na kuratibu zinazotokana lazima ziwe kamili.
survey-invalid-scale = Kipimo lazima kiwe nambari chanya kamili yenye kigezo tishio kamili.
survey-empty-selection = Chagua angalau kitu kimoja kinachotumika ili kubadilisha.
survey-unavailable = Kitu kilichochaguliwa hakipo au hakijapakiwa. Kipakie kabla ya kubadilisha.
survey-wrong-project = Chagua miundo kutoka mradi hai tu.
survey-name-required = Weka jina la mfumo wa kuratibu.
survey-working = Inabadilisha data iliyochaguliwa…
survey-completed = { $items } imebadilishwa mahali pale pale. Kutendua kunairejesha.
survey-failed = Ubadilishaji umeshindwa: { $error }
survey-stale = Ubadilishaji umeondolewa kwa sababu mradi hai au data ya chanzo imebadilika. Chagua data ya chanzo na ujaribu tena.
survey-coordinates-menu = Kuratibu
survey-definitions-action = Fasili…
survey-transform-action = Badilisha…
survey-definitions-title = Fasili za Kuratibu
survey-transform-title = Badilisha Kuratibu
survey-new-system = Mfumo Mpya wa Kuratibu
survey-new-system-name = Mfumo wa kuratibu
survey-set-local = Weka kama Mfumo wa Kuratibu wa Mgodi
survey-delete-system = Futa Mfumo wa Kuratibu
survey-systems-empty = Hakuna mifumo ya kuratibu
survey-system-name = Jina
survey-system-origin = Kidokezo hicho hicho — kuratibu za mfumo
survey-angle-help = Kinyume cha saa kutoka X ya marejeo kuelekea Y ya marejeo, ukitazama kutoka juu.
survey-scale-help = Kipimo sawia cha XYZ kutoka fremu ya marejeo hadi mfumo huu. Tumia 1 ili kudumisha vipimo.
survey-close = Funga
survey-from = Kutoka
survey-to = Kwenda
survey-transform-button = Badilisha
survey-swap = Badilishana
survey-drape-note = Picha zilizofunikwa huondolewa kwenye nyuso zilizobadilishwa na lazima zifunikwe upya.
survey-needs-grid-block-model = Mfano wa vitalu ni gridi ya kawaida ya seli, na mabadiliko ya makadirio au fremu ya marejeo hayaidumishi ikiwa ya kawaida. Kuibadilisha kungemaanisha kuchukua sampuli upya ya kila seli kwenye gridi mpya na kupoteza thamani inazobeba, kwa hivyo iliachwa bila kubadilishwa.
survey-needs-grid-raster = Rasta huwekwa kwenye dunia kwa ramani ya affine, ambayo mabadiliko ya makadirio au fremu ya marejeo hayawezi kudumisha. Kuibadilisha kungemaanisha kuchukua sampuli upya ya picha, kwa hivyo iliachwa bila kubadilishwa.
survey-conversion-exact = Halisi: mabadiliko ya gridi tu, hakuna uwekaji makadirio upya.
survey-conversion-accuracy = Usahihi uliotajwa m { $accuracy }.
survey-kind = Aina
survey-axis-names = Majina ya mihimili
survey-kind-registry-short = Mfumo wa orodha
survey-kind-grid-short = Gridi juu ya mfumo mwingine
survey-registry-search = Tafuta
survey-registry-hint = Jina au msimbo wa EPSG, mfano "mga zone 56"
survey-registry-none = Hakuna kitu kwenye orodha kinacholingana na kila neno.
survey-parent = Umefasiliwa dhidi ya
survey-parent-origin = Kidokezo kinachojulikana — kuratibu za mzazi
survey-pick-registry = Tafuta mfumo na uuchague kutoka kwenye matokeo.
survey-pick-parent = Chagua mfumo ambao gridi hii imefasiliwa dhidi yake.
survey-pick-system = Chagua mfumo
survey-pick-systems = Chagua mfumo wa kubadilisha kutoka na ule wa kubadilisha kwenda.
survey-no-selection = Chagua mfumo wa kuratibu upande wa kushoto, au bofya kulia ili kuongeza mmoja.
survey-kind-grid = Gridi juu ya { $parent }
survey-system-in-use = "{ $name }" haiwezi kufutwa: { $dependants } { $dependants ->
    [one] imefasiliwa
   *[other] zimefasiliwa
  } dhidi yake. Zielekeze mahali pengine kwanza.
survey-system-cycle = "{ $name }" imefasiliwa dhidi yake yenyewe, moja kwa moja au kupitia wazazi wake.
survey-system-missing = Mfumo huo wa kuratibu haupo tena. Chagua fasili nyingine.
survey-same-system = Chagua mifumo tofauti ya chanzo na lengo.
survey-name-exists = Mfumo wa kuratibu wenye jina hilo tayari upo. Kichague ili kukihariri, au chagua jina lingine.
preferences-ui-size = Ukubwa wa kiolesura
preferences-ui-size-help = Hurekebisha maandishi na vidhibiti kulingana na ukubwa wa kawaida wa onyesho la kifaa chako. 100% hutumia ukubwa wa chaguo-msingi. Azimio la skrini na ukubwa wa dirisha havipunguzi kiolesura.
relimit-select-boundary = Chagua mstari wa pointi nyingi au duara la kuwekea kikomo
relimit-click-boundary = Bofya mstari wa pointi nyingi au duara la kukatiza nalo…
relimit-mode-help = Kukatiza huhamisha ncha moja hadi mstari wa pointi nyingi au duara. Kamili huweka urefu wa mwisho wa mstari. Linganishi huongeza au kupunguza urefu.
browser-graphics-device-lost = Kivinjari kimepoteza kifaa chake cha michoro. Fungua ukurasa huu tena kwenye kichupo kipya. Maelezo ya GPU: { $message }

## About strings

about-copyright-c-2026-leo-timmins =
    Hakimiliki (c) 2026 Leo Timmins, Lucas Timmins na wachangiaji wa Incline Design. Ruhusa inatolewa hapa, bila malipo, kwa mtu yeyote anayepata nakala ya programu hii kuishughulikia bila kizuizi, kulingana na masharti ya Leseni ya MIT.

    Incline Design inatolewa "JINSI ILIVYO", BILA DHAMANA YA AINA YOYOTE, WAZI AU YA KUDOKEZWA, ikiwemo lakini bila kikomo kwa dhamana za UUZAJI, UFAAO KWA KUSUDI MAALUM na KUTOKIUKA.
about-free-open-source-mine-design = Muundo wa Migodi Huru na Chanzo Wazi
about-licensed-under-mit-license = Imeidhinishwa chini ya Leseni ya MIT

## App strings

app-activated-browser-project-name = Mradi wa kivinjari '{ $name }' umeamilishwa.
app-browser-project-delete-failed = Ufutaji wa mradi wa kivinjari umeshindwa: { $error }
app-browser-project-no-longer-exists = Mradi huo wa kivinjari haupo tena
app-browser-save-failed-error = Kuhifadhi kwa kivinjari kumeshindwa: { $error }
app-could-not-activate-browser-project = Imeshindwa kuamilisha mradi wa kivinjari: { $error }
app-could-not-delete-browser-project = Imeshindwa kufuta mradi wa kivinjari: { $error }
app-could-not-load-browser-project = Imeshindwa kupakia mradi wa kivinjari: { $error }
app-could-not-restore-browser-project = Imeshindwa kurejesha mradi wa kivinjari: { $error }
app-deleted-browser-project = Mradi wa kivinjari umefutwa
app-failed-create-window-error = Imeshindwa kuunda dirisha: { $error }
app-failed-create-window-icon-error = Imeshindwa kuunda aikoni ya dirisha: { $error }
app-failed-detach-top-down-preview = Imeshindwa kutenganisha hakikisho la juu-chini: { $error }
app-failed-initialize-graphics-error = Imeshindwa kuanzisha michoro: { $error }
app-browser-preferences-load-failed = Imeshindwa kupakia mapendeleo ya kivinjari: { $error }
app-failed-load-config-file-error = Imeshindwa kupakia faili ya usanidi: { $error }
app-failed-load-session-file-error = Imeshindwa kupakia faili ya kipindi: { $error }
app-failed-rasterize-window-icon-error = Imeshindwa kubadilisha aikoni ya dirisha kuwa rasta: { $error }
app-failed-save-browser-session-error = Imeshindwa kuhifadhi kipindi cha kivinjari: { $error }
app-failed-save-session-error = Imeshindwa kuhifadhi kipindi: { $error }
app-saved-name-browser-storage = '{ $name }' imehifadhiwa kwenye hifadhi ya kivinjari

## Block strings

block-model-between = Kati ya
block-model-block-grid = Gridi ya vitalu
block-model-block-size = Ukubwa wa kitalu
block-model-choose-numeric-variable = Chagua kigezo cha nambari
block-model-choose-numeric-variables = Chagua vigezo vya nambari
block-model-count-variables-selected = Vigezo { $count } vimechaguliwa
block-model-estimate-variables = Kadiria vigezo
block-model-full-x-y-z-dimensions = Vipimo kamili vya X, Y na Z vya kila kitalu. Vitalu vidogo huongeza maelezo, muda wa hesabu na matumizi ya kumbukumbu.
block-model-grid-bounds-block-sizes-invalid = Mipaka ya gridi au ukubwa wa vitalu si sahihi.
block-model-lower-x-y-z-edges = Kingo za chini za X, Y na Z za ujazo wa mfano wa vitalu. Vitovu vya vitalu huanzia nusu kitalu ndani ya mipaka hii.
block-model-maximum = Kima cha juu
block-model-maximum-nearest-samples-used-each = Sampuli za karibu za juu zinazotumika kwa kila kitalu. Thamani za chini huendesha haraka; thamani za juu zinaweza kulainisha makadirio na kuongeza muda wa hesabu.
block-model-maximum-samples = Sampuli za juu
block-model-minimum = Kima cha chini
block-model-min-samples-help = Sampuli za karibu za chini zinazohitajika kukadiria kitalu. Vitalu vyenye sampuli chache ndani ya radiasi ya utafutaji vinaachwa tupu.
block-model-minimum-samples = Sampuli za chini
block-model-no-block-model-selected = Hakuna mfano wa vitalu uliochaguliwa
block-model-no-drill-holes-selected = Hakuna mashimo ya uchimbaji yaliyochaguliwa
block-model-nugget = Nugget
block-model-numeric-interval-fields-interpolate = Sehemu za vipindi vya nambari za kuingilianisha. Kila sehemu iliyochaguliwa inakuwa kigezo kimoja cha mfano wa vitalu.
block-model-kriging-help = Kriging ya Kawaida hukadiria vipindi vya nambari vya mashimo ya uchimbaji kwenye kitovu cha kila kitalu kwa kutumia variogramu ya duara.
block-model-partial-sill = Sill ya kiasi
block-model-range-search-radius = Wigo / radiasi ya utafutaji
block-model-range-help = Sampuli zilizo mbali zaidi ya umbali huu hazijumuishwi; ushirikiano hufikia sifuri kwenye wigo huu.
block-model-select-all = Chagua yote
block-model-selected-block-model-whose-blocks = Mfano wa vitalu uliochaguliwa, ambao vitalu vyake vinabadilishwa kuwa kigumu kwa kizingiti. Funga dirisha ili kuweka kizingiti kwa mwingine.
block-model-source-drill-holes-help = Mkusanyiko wa Mashimo ya Uchimbaji uliochaguliwa, ambao vipindi vyake vya nambari vinakadiriwa kuwa vitalu. Funga dirisha ili kukadiria kutoka kwa mwingine.
block-model-sill-help = Utofauti unaohusiana kwa anga unaotolewa na mfano wa duara. Pamoja na nugget, huweka ushirikiano kwenye umbali sifuri.
block-model-spherical-variogram-search = Variogramu ya duara na utafutaji
block-model-threshold-at-most = <= kizingiti
block-model-threshold-at-least = >= kizingiti
block-model-threshold-min = Kizingiti / kima cha chini
block-model-upper-x-y-z-extent = Wigo wa juu wa X, Y na Z wa kufunika. Kitalu cha mwisho kinaweza kupita wigo huu wakati urefu si kizidishi kamili cha ukubwa wa kitalu.
block-model-variable = Kigezo
block-model-variance-effectively-zero-separation = Utofauti kwenye tofauti ya karibu sifuri unaosababishwa na hitilafu ya kipimo au tofauti chini ya kipimo cha sampuli. Tumia sifuri wakati hakuna athari ya nugget inayokusudiwa.
block-model-volume-feedback-disconnected = Usomaji wa mrejesho wa matumizi ya ujazo wa vitalu umekatika
block-model-volume-feedback-failed = Usomaji wa mrejesho wa matumizi ya ujazo wa vitalu umeshindwa: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z
borehole-inspector-checking-linked-geophysics-file = Inakagua faili ya jiofizikia iliyounganishwa...
borehole-inspector-close-inspector = Funga kikaguzi
borehole-inspector-data = Data
borehole-inspector-display = Onyesho
borehole-inspector-file-not-where-was-linked = { $file } haipo mahali ilipounganishwa kutoka.
borehole-inspector-guessed-name = Imekisiwa kwa jina
borehole-inspector-hold-hole-while-you-work = Shikilia shimo hili unapofanya kazi kwenye yale yanayolizunguka.
borehole-inspector-holding-hole-click-follow-selection = Shimo hili limeshikiliwa. Bofya ili kufuata uteuzi tena.
borehole-inspector-log = Logi
borehole-inspector-no-hole-inspected = Hakuna shimo linalokaguliwa
borehole-inspector-pick-file = Chagua { $file }...
borehole-inspector-pick-file-again-show-its = Chagua { $file } tena ili kuonyesha jiofizikia yake: ukurasa wa kivinjari hauwezi kufungua faili tena wenyewe.
borehole-inspector-reading-geophysics-file-its-index = Inasoma faili ya jiofizikia kwa ajili ya kielezo chake; pau ya hali unaonyesha maendeleo.
borehole-inspector-strat = Stratigrafia
borehole-inspector-summary = Muhtasari
canvas-circle-summary = Duara | Tabaka: { $layer } | radiasi { $radius }

## Canvas strings

canvas-not-selectable-closed-polyline = Haiwezi kuchaguliwa | Chagua mstari wa pointi nyingi uliofungwa
canvas-polyline-summary = Mstari wa Pointi Nyingi | Tabaka: { $layer } | vipeo { $count }
canvas-surface-name = Uso | { $name }
canvas-trimmed = Imepunguzwa
cinematic-shadows-method = Vivuli vya mwonekano wa sinema: { $method }

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = Mteremko na lundo la kinga vimeundwa kutoka kitu { $object_id }
cmd-bezier-replaced-polyline-span-first-last = Sehemu ya mstari wa pointi nyingi { $first }→{ $last } imebadilishwa na vidokezo { $count } vya kati vilivyopimwa
cmd-bezier-vertices-first-last = Vipeo { $first } hadi { $last }
cmd-block-model-block-model-loader-disconnected-path = Kipakiaji cha mfano wa vitalu kimekatika kwa { $path }
cmd-block-model-block-model-path-has-count = Mfano wa vitalu { $path } una vigezo { $count } vya aina isiyotumika ambavyo havitasomeka: { $names }
cmd-block-model-building-ore-mesh = Inajenga mfumo wa madini…
cmd-block-model-could-not-create-block-model = Imeshindwa kuunda mfano wa vitalu: { $error }
cmd-block-model-could-not-decode-block-model = Imeshindwa kusimbua kigezo cha rangi cha mfano wa vitalu '{ $variable }': { $error }
cmd-block-model-created-block-model-name-ordinary = Umeunda mfano wa vitalu '{ $name }' kwa Kriging ya Kawaida
cmd-block-model-failed-load-block-model-error = Imeshindwa kupakia mfano wa vitalu: { $error }
cmd-block-model-generated-ore-mesh-from-block = Mfumo wa madini umezalishwa kutoka mfano wa vitalu '{ $name }'
cmd-block-model-imported-block-model-source-path = Chanzo cha mfano wa vitalu kilichoingizwa { $path }
cmd-block-model-loaded-block-model-name-blocks = Mfano wa vitalu '{ $name }' umepakiwa: vitalu { $blocks } ({ $renderable } vinavyoweza kuchorwa), gridi { $dimx }x{ $dimy }x{ $dimz }, vigezo { $variables }
cmd-block-model-loading-name = Inapakia { $name }
cmd-block-model-loading-name-ellipsis = Inapakia { $name }…
cmd-chamfer-applied = Kona { $corner } imepunguzwa kwa radiasi { $radius } na sehemu { $segments }
cmd-chamfer-radius = Radiasi { $radius }
cmd-commands-clipped = Imekatwa
cmd-commands-command-failed-error = Amri imeshindwa: { $error }
cmd-commands-count-control-string-s = mifuatano { $count } ya udhibiti
cmd-commands-count-control-string-s-layer = mifuatano { $count } ya udhibiti kwenye '{ $layer }'
cmd-commands-count-point-s-across-layers = vidokezo { $count } katika tabaka { $layers }
cmd-commands-count-point-s-layer = vidokezo { $count } kwenye '{ $layer }'
cmd-commands-kind-layer = { $kind } kwenye '{ $layer }'
cmd-commands-no-control-strings = Hakuna mifuatano ya udhibiti
cmd-commands-no-extent = Hakuna wigo
cmd-commands-select-holes-place-reference-points = Chagua mashimo ya kuwekea vidokezo vya marejeo
cmd-triangulate-needs-selection = Chagua vitu vya kutatua kabla ya kuendesha Unda Utatuzi
cmd-commands-select-one-loaded-block-model = Chagua mfano mmoja wa vitalu uliopakiwa kabla ya kuunda utatuzi wa madini kutoka kwake
cmd-commands-select-one-loaded-drill-hole = Chagua mkusanyiko mmoja wa mashimo ya uchimbaji uliopakiwa kabla ya kuunda mfano wa vitalu kutoka kwake
cmd-commands-select-one-loaded-point-cloud = Chagua wingu moja la vidokezo lililopakiwa kabla ya kuunda utatuzi kutoka kwake
cmd-contours-needs-triangulation = Chagua utatuzi mmoja uliopakiwa kabla ya kuzalisha mistari ya mwinuko kutoka kwake
cmd-slice-needs-triangulation = Chagua utatuzi mmoja uliopakiwa kabla ya kuukata kwa wigo wa Z
cmd-commands-select-one-loaded-triangulation-one = Chagua utatuzi mmoja uliopakiwa na mstari mmoja wa pointi nyingi uliofungwa kabla ya kukata
cmd-commands-select-one-more-objects-before = Chagua kitu kimoja au zaidi kabla ya kuweka { $axis }
cmd-commands-sliced = Imekatwa (Sliced)
cmd-contours-contour-generation-failed-error = Uzalishaji wa mistari ya mwinuko umeshindwa: { $error }
cmd-contours-discarded-layer-exists = Mistari ya mwinuko ya '{ $name }' imeondolewa: tabaka '{ $layer_name }' sasa lipo
cmd-contours-discarded-project-closed = Mistari ya mwinuko ya '{ $name }' imeondolewa: mradi umefungwa
cmd-contours-discarded-layer-deleted = Mistari ya mwinuko ya '{ $name }' imeondolewa: tabaka la matokeo lililochaguliwa limefutwa
cmd-contours-generated = Mistari { $line_count } ya mwinuko imezalishwa kwa utatuzi '{ $name }' kwenye tabaka '{ $layer_name }'
cmd-creation-assembled-boundary-rings = Miduara { $assembled_count } ya mpaka iliyofungwa imekusanywa kutoka mifuatano iliyokatika iliyo wazi
cmd-creation-created-triangulation-from-boundary = Utatuzi umeundwa kutoka miduara { $boundary_count } ya mpaka na vizuizi { $constraint_count } vilivyo wazi, aina ya uso { $surface_type }
cmd-creation-creating-triangulation = Inaunda utatuzi…
cmd-creation-generate-upper-surface-ignored-count = Zalisha uso wa juu: sehemu { $count } za mstari wa mvunjiko zenye mgongano wa chini zimepuuzwa; vitu vya chanzo havijabadilika
cmd-creation-ignored-objects = Vitu { $rejected } visivyo mistari ya pointi nyingi au vilivyoharibika vimepuuzwa wakati wa utatuzi
cmd-creation-weld-retry-moved-coarse-welded = Unga na ujaribu tena: vipeo { $coarse_welded } vimehamishwa hadi nafasi za pamoja (hadi m{ $coarse_weld_tol }); vitu vya chanzo havijabadilika
cmd-creation-welded-breakline-vertices = Vipeo { $welded } vya mstari wa mvunjiko vilivyopatana ndani ya kikomo vimeungwa
cmd-cuts-clipped-surface-name-polyline-mode = Uso '{ $name }' umekatwa kwa mstari wa pointi nyingi ({ $mode })
cmd-cuts-clipping-surface-polyline = Inakata uso kwa mstari wa pointi nyingi…
cmd-cuts-cut-topology-name-pit-shell = Tofolojia '{ $name }' imekatwa kwa ganda la shimo
cmd-cuts-cut-triangulation-name-z-band = Utatuzi '{ $name }' umekatwa kwa mkanda wa Z [{ $min }, { $max }]
cmd-cuts-cutting-topology-pit-shell = Inakata tofolojia kwa ganda la shimo…
cmd-cuts-cutting-triangulation-z = Inakata utatuzi kwa Z…
cmd-cuts-ignored-vertical-faces = Nyuso { $count } za tofolojia ya rejeleo zilizo wima au zilizoharibika bila eneo la XY zimepuuzwa
cmd-cuts-site-skipped-constraint-from-x = { $site }: kizuizi kimerukwa ({ $from_x }, { $from_y }) -> ({ $to_x }, { $to_y }) kitatuzi hakikuweza kugawanya
cmd-cuts-skipped-degenerate-edges = { $site }: kingo { $skipped } za vizuizi zilizo karibu kuharibika zimerukwa; mpaka wa ukataji unaweza kupotoka kidogo karibu nazo
cmd-cuts-trimmed-surface = Uso '{ $surface }' umepunguzwa kwa tofolojia '{ $topology }' ({ $mode })
cmd-cuts-trimming-surface-topology = Inapunguza uso kwa tofolojia…
cmd-drape-draped-intersected-vertices-changed = Vipeo { $intersected } vimefunikwa; { $changed } vimebadilisha kimo
cmd-drape-no-intersections = Hakuna kipeo cha muundo kilichochaguliwa kinachokatiza tofolojia zilizochaguliwa
cmd-drape-objects-changed-object-s-changed = vitu { $objects } vimebadilika · vipeo { $changed } kati ya { $intersected } vinavyokatiza vimehamishwa
cmd-drape-select-one-more-design-objects = Chagua kitu kimoja au zaidi cha muundo cha kufunika
cmd-drape-select-one-more-topologies-drape = Chagua tofolojia moja au zaidi ya kufunikia
cmd-drape-selected-topologies-no-longer-loaded = Tofolojia zilizochaguliwa hazijapakiwa tena
cmd-drill-hole-choose-drillhole-source-files-again = Chagua faili za chanzo za mashimo ya uchimbaji tena
cmd-drill-hole-drill-pattern-too-large-contains = Mfumo wa uchimbaji ni mkubwa mno au una kuratibu za kola zisizo sahihi
cmd-drill-hole-drillhole-field-label-has-count = Sehemu ya shimo la uchimbaji '{ $label }' ina misimbo { $count } tofauti, zaidi ya ile ambayo sehemu ya misimbo huwa nayo kwa kawaida; inaonekana kama maandishi huru badala ya sehemu ya kategoria, lakini misimbo yote imehifadhiwa na kupakwa rangi
cmd-drill-hole-enter-name-drill-pattern = Weka jina la mfumo wa uchimbaji
cmd-drill-hole-failed-load-drillholes-error = Imeshindwa kupakia mashimo ya uchimbaji: { $error }
cmd-drill-hole-depth-must-be-positive = Kina cha shimo lazima kiwe zaidi ya sifuri
cmd-drill-hole-diameter-must-be-positive = Kipenyo cha shimo lazima kiwe zaidi ya sifuri
cmd-drill-hole-loaded-drillhole-dataset-name-holes = Data ya mashimo ya uchimbaji '{ $name }' imepakiwa: mashimo { $holes }, sehemu za rangi { $fields }
cmd-drill-hole-name-already-loading = '{ $name }' tayari inapakiwa
cmd-drill-hole-name-reason = '{ $name }': { $reason }
cmd-drill-hole-name-side = { $name } { $side }
cmd-drill-hole-name-working-section-side = { $name } sehemu ya kazi { $side }
cmd-drill-hole-no-hole-holds-value-field = Hakuna shimo lenye '{ $value }' kwenye sehemu hiyo
cmd-drill-hole-only-mapped-csv-bundles-imported = Ni vifurushi vya CSV vilivyooanishwa pekee vinaingizwa kwenye kivinjari
cmd-drill-hole-pattern-contains-no-holes = Mfumo hauna mashimo
cmd-drill-hole-reading-name = Inasoma { $name }
cmd-drill-hole-reference-points-used-holes-placed = Vidokezo vya marejeo: mashimo { $used } yamewekwa, { $absent } bila '{ $value }', { $flagged } yamealamishwa kama marudio yanayowezekana ya mpasuko
cmd-drill-hole-uppermost-run-used-flagged-holes = Mfululizo wa juu kabisa umetumika, yaliyoalamishwa: { $holes }
cmd-drill-hole-working-section-name-not-same = Sehemu ya kazi '{ $name }' si ile ile katika kila seti ya data iliyochaguliwa; ya kila seti ya data imetumika.
cmd-drill-hole-working-sections-not-kept-dataset = Sehemu za kazi hazikuhifadhiwa kwenye '{ $dataset }'. { $reasons }
cmd-explode-count-line-s = mistari { $count }
cmd-explode-polyline = Vunja Mstari wa Pointi Nyingi
cmd-explode-exploded-polyline-into-count-line = Mstari wa pointi nyingi umevunjwa kuwa sehemu { $count } za mstari
cmd-file-block-model-csv-encoding-failed = Usimbaji wa CSV wa mfano wa vitalu umeshindwa: { $error }
cmd-file-block-model-csv-export-failed = Uhamishaji wa CSV ya mfano wa vitalu umeshindwa: { $error }
cmd-file-browser-recovery-unavailable = Faili za kurejesha za kivinjari hazipatikani; miradi iliyohifadhiwa inabaki kwenye IndexedDB
cmd-file-closed-project-runtime-id-runtime = Mradi umefungwa runtime id { $runtime_id }
cmd-file-could-not-create-new-project = Imeshindwa kuunda mradi mpya: { $error }
cmd-file-could-not-finish-pending-project = Imeshindwa kukamilisha kitendo cha mradi kinachosubiri: { $error }
cmd-file-could-not-finish-saving-before = Imeshindwa kukamilisha kuhifadhi kabla ya kutoka: { $error }
cmd-file-could-not-open-browser-project = Imeshindwa kufungua mradi wa kivinjari: { $error }
cmd-file-could-not-open-path-error = Imeshindwa kufungua { $path }: { $error }
cmd-file-could-not-read-selected-file = Imeshindwa kusoma faili iliyochaguliwa: { $error }
cmd-file-could-not-reload-layer-from = Imeshindwa kupakia upya tabaka kutoka kwenye diski: { $error }
cmd-file-could-not-reload-project-from = Imeshindwa kupakia upya mradi kutoka kwenye diski: { $error }
cmd-file-could-not-remove-browser-project = Imeshindwa kuondoa mradi wa kivinjari: { $error }
cmd-file-could-not-restore-layer-from = Imeshindwa kurejesha tabaka kutoka kwenye mradi: { $error }
cmd-file-could-not-snapshot-dirty-project = Imeshindwa kupiga picha ya mradi usiohifadhiwa kwa ajili ya kurejesha: { $error }
cmd-file-could-not-start-browser-export = Imeshindwa kuanzisha uhamishaji wa kivinjari: { $error }
cmd-file-could-not-write-recovery-copies = Imeshindwa kuandika nakala za kurejesha: { $error }
cmd-file-created-new-browser-project = Umeunda mradi mpya wa kivinjari
cmd-file-created-new-project = Umeunda mradi mpya
cmd-file-description-download-failed-error = Upakuaji wa { $description } umeshindwa: { $error }
cmd-file-discard-cancelled-project-changed = Kuondoa kumeghairiwa kwa sababu mradi ulibadilika wakati OMF ilikuwa ikipakiwa upya
cmd-file-discarded-changes-layer-target-name = Mabadiliko kwenye tabaka '{ $target_name }' yameondolewa
cmd-file-discarded-changes-reloaded-path = Mabadiliko yameondolewa: { $path } imepakiwa upya
cmd-file-downhole-geophysics-csv = CSV ya jiofizikia ya ndani ya shimo
cmd-file-downloaded-description-file-name = Umepakua { $description }: { $file_name }
cmd-file-drillhole-csv-export-failed-error = Kuhamisha CSV ya mashimo ya uchimbaji kumeshindwa: { $error }
cmd-file-dxf-download-encoding-failed-error = Usimbaji wa kupakua DXF umeshindwa: { $error }
cmd-file-dxf-import-failed-error = Kuingiza DXF kumeshindwa: { $error }
cmd-file-encoding-block-model-csv-download = Inasimba upakuaji wa CSV ya mfano wa vitalu…
cmd-file-encoding-dxf-download = Inasimba upakuaji wa DXF…
cmd-file-encoding-triangulation-download = Inasimba upakuaji wa utatuzi…
cmd-file-exit-deferred-exports = Kutoka kumeahirishwa hadi uhamishaji wa nyuma ukamilike
cmd-file-exit-requested-no-unsaved-changes = Kutoka kumeombwa bila mabadiliko yasiyohifadhiwa
cmd-file-exported-block-model-csv-path = Umehamisha CSV ya mfano wa vitalu kwenda { $path }
cmd-file-exported-description-dxf-path = Umehamisha { $description } kwenda DXF: { $path }
cmd-file-exported-three-drillhole-csvs-path = Umehamisha CSV tatu za mashimo ya uchimbaji kwenda { $path }
cmd-file-exported-triangulation-name-path = Umehamisha utatuzi '{ $name }' kwenda { $path }
cmd-file-exporting-name = Inahamisha { $name }…
cmd-file-exporting-triangulation-name-path = Inahamisha utatuzi '{ $name }' kwenda { $path }
cmd-file-fatal-renderer-failure-reason = Hitilafu kubwa ya kichoraji: { $reason }
cmd-file-dialog-action-failed = Kitendo cha dirisha la faili kimeshindwa: { $msg }
cmd-file-imported-added-object-s-from = Umeingiza vitu { $added } kutoka { $name }
cmd-file-imported-total-dxf-object-s = Umeingiza vitu { $total } vya DXF
cmd-file-layer-discard-was-cancelled-because = Uondoaji wa tabaka umeghairiwa kwa sababu mradi ulibadilika wakati mradi ulikuwa ukipakiwa upya
cmd-file-no-recovery-directory = Hakuna saraka ya kurejesha inayopatikana: { $error }
cmd-file-no-unsaved-project-content-nothing = Hakuna maudhui ya mradi yasiyohifadhiwa; hakuna cha kurejesha
cmd-file-parsing-browser-dxf-import = Inachanganua uingizaji wa DXF wa kivinjari…
cmd-file-parsing-dxf-import = Inachanganua uingizaji wa DXF…
cmd-file-project-closes-after-save = Mradi utafungwa baada ya uhifadhi wake wa sasa kukamilika
cmd-file-the-project-closes-after-save = Mradi utafungwa baada ya uhifadhi wake wa sasa kukamilika
cmd-file-queued-count-triangulation-file-s = Umeweka foleni faili { $count } za utatuzi za kuingiza
cmd-file-recovery-copies-path-reopen-them = Nakala za kurejesha ziko { $path }; zifungue tena baada ya kuanzisha upya
cmd-file-recovery-copy-failed-error = Nakala ya kurejesha imeshindwa: { $error }
cmd-file-recovery-copy-failed-failure = Nakala ya kurejesha imeshindwa: { $failure }
cmd-file-recovery-copy-written-path = Nakala ya kurejesha imeandikwa: { $path }
cmd-file-reverting-layer = Inarejesha tabaka…
cmd-file-reverting-project = Inarejesha mradi…
cmd-file-save-failed-message = Kuhifadhi kumeshindwa: { $message }
cmd-file-save-project-already-running-save = Uhifadhi wa mradi huu tayari unaendelea; hifadhi tena utakapokamilika
cmd-file-save-worker-ended-without-result = Kazi ya kuhifadhi imeisha bila matokeo
cmd-file-saved-project-as = Umehifadhi mradi kama: { $path }
cmd-file-saved-project = Umehifadhi mradi: { $path }
cmd-file-saving-browser-storage = Inahifadhi kwenye hifadhi ya kivinjari…
cmd-file-selected-block-model-no-longer = Mfano wa vitalu uliochaguliwa hauja pakiwa tena
cmd-file-selected-drillhole-dataset-no-longer = Seti ya data ya mashimo ya uchimbaji iliyochaguliwa haijapakiwa tena
cmd-file-switching-project = Inabadilisha mradi…
cmd-file-triangulation-download-encoding-failed = Usimbaji wa kupakua utatuzi umeshindwa: { $error }
cmd-file-user-chose-exit-without-saving = Mtumiaji amechagua kutoka bila kuhifadhi
cmd-file-user-requested-exit-project-export = Mtumiaji ameomba kutoka (uthibitisho wa uhamishaji wa mradi au kazi isiyohifadhiwa unahitajika)
cmd-file-viewport = Mwonekano
cmd-file-wait-current-project-save-finish = Subiri uhifadhi wa mradi wa sasa ukamilike
cmd-file-wait-current-project-switch-finish = Subiri ubadilishaji wa mradi wa sasa ukamilike
cmd-file-wait-project-operation-finish-before = Subiri operesheni ya mradi ikamilike kabla ya kuondoa mabadiliko
cmd-file-wait-project-revert-finish-before = Subiri urejeshaji wa mradi ukamilike kabla ya kuhifadhi
cmd-folder-collection-named-name-already-exists = Mkusanyiko uitwao '{ $name }' tayari upo
cmd-folder-collection-no-longer-exists = Mkusanyiko huo haupo tena
cmd-folder-created-collection-name = Umeunda mkusanyiko '{ $name }'
cmd-folder-deleted-collection-name = Umefuta mkusanyiko '{ $name }'
cmd-folder-moved-item-into-collection-name = Umehamisha kipengele kwenye mkusanyiko '{ $name }'
cmd-folder-moved-item-root-section = Umehamisha kipengele hadi mzizi wa { $section }
cmd-folder-renamed-collection-before-after = Umebadilisha jina la mkusanyiko '{ $before }' kuwa '{ $after }'
cmd-folder-section-cannot-hold-item = Sehemu hiyo haiwezi kuhifadhi kipengele hiki
cmd-fuse-closed-polyline = Mstari wa pointi nyingi uliofungwa
cmd-fuse-count-source-line-s = mistari { $count } ya chanzo
cmd-fuse-created-shape-object-id-vertices = { $shape } { $object_id } imeundwa yenye vipeo { $vertices } kutoka mistari { $sources } ya chanzo
cmd-fuse-click-missed = Unganisha: kubofya hakukugusa kitu chochote (hakuna kilicho chini ya kishale)
cmd-fuse-click-not-near-endpoint = Unganisha: kubofya hakukuwa karibu vya kutosha na ncha yoyote ya mstari uliochaguliwa
cmd-fuse-clicked-closed-polyline = Unganisha: kitu kilichobofywa { $object_id } ni mstari wa pointi nyingi uliofungwa, kuunganisha hufanya kazi kwenye mistari iliyo wazi tu
cmd-fuse-clicked-not-open-polyline = Unganisha: kitu kilichobofywa { $object_id } si mstari wa pointi nyingi ulio wazi (ni { $kind })
cmd-fuse-clicked-object-missing = Unganisha: kitu kilichobofywa { $object_id } hakipo tena
cmd-fuse-clicked-too-few-vertices = Unganisha: mstari wa pointi nyingi uliobofywa { $object_id } una vipeo { $count } pekee, vinahitajika angalau 2
cmd-fuse-endpoint-marker-missing = Unganisha: alama ya ncha { $marker_index } haipo tena
cmd-fuse-close-needs-three-vertices = Unganisha: mstari unahitaji angalau vipeo 3 tofauti ili kufungwa kuwa mstari wa pointi nyingi (una { $count })
cmd-fuse-lines = Unganisha Mistari
cmd-fuse-needs-two-segments = Unganisha: inahitajika angalau sehemu 2 kukamilisha (kuna { $count })
cmd-fuse-no-active-layer = Unganisha: hakuna tabaka hai la kuweka mstari ulioungwa
cmd-fuse-no-active-project = Unganisha: hakuna mradi hai, haiwezekani kukamilisha
cmd-fuse-no-source-line = Unganisha: hakuna mstari wa chanzo wa kufunga kuwa mstari wa pointi nyingi
cmd-fuse-awaiting-object-invalid = Unganisha: kitu { $awaiting_id } si mstari halali wa pointi nyingi tena
cmd-fuse-object-already-in-chain = Unganisha: kitu { $object_id } tayari ni sehemu ya mnyororo wa kuunganisha, bofya mstari mwingine
cmd-fuse-result-too-few-vertices = Unganisha: matokeo yana vipeo vichache mno ({ $count }), inasitisha
cmd-fuse-segment-object-invalid = Unganisha: sehemu ya kitu { $object_id } si mstari halali wa pointi nyingi tena, inasitisha
cmd-fuse-source-object-invalid = Unganisha: kitu cha chanzo { $object_id } si mstari halali wa pointi nyingi ulio wazi tena
cmd-fuse-source-object-missing = Unganisha: kitu cha chanzo { $object_id } hakipo tena
cmd-fuse-open-polyline = Mstari wa pointi nyingi ulio wazi
cmd-include-failed = Kujumuisha kumeshindwa: { $message }
cmd-include-included-solid-shape-name-topology = Kigumu '{ $shape_name }' kimejumuishwa kwenye tofolojia '{ $topology_name }' (nyuso { $retained } za tofolojia zimehifadhiwa, nyuso { $skipped } za kifuniko zimerukwa)
cmd-include-including-pit-stockpile-solid = Inajumuisha kigumu cha shimo/rundo la hifadhi…
cmd-insert-point-count-operation-point-s = vidokezo { $count } vya { $operation }
cmd-insert-point-elevation-must-be-finite = Kuingiza Kidokezo kwenye Kimo kunahitaji kimo dhahiri
cmd-insert-point-insert-points = Ingiza Vidokezo
cmd-insert-point-inserted-count-operation-point-s = Vidokezo { $count } vya { $operation } vimeingizwa
cmd-insert-point-intersection = Makutano
cmd-insert-point-no-new-operation-points-were = Hakuna vidokezo vipya vya { $operation } vilivyopatikana
cmd-insert-point-select-least-two-polylines-before = Chagua angalau mistari miwili ya pointi nyingi kabla ya kuingiza vidokezo vya makutano
cmd-insert-point-select-one-more-polylines-before = Chagua mstari mmoja au zaidi wa pointi nyingi kabla ya kuingiza kidokezo kwenye kimo
cmd-layer-created-layer-name = Umeunda tabaka '{ $name }'
cmd-layer-deleted-with-objects = Tabaka { $layer_id } limefutwa (pamoja na vitu vyote vilivyomo)
cmd-layer-duplicated-layer-duplicate-name = Umenakili tabaka '{ $duplicate_name }'
cmd-layer-locked = Imefungwa
cmd-layer-name-copy = nakala ya { $name }
cmd-layer-selected-count-object-s-layer = Vitu { $count } vimechaguliwa kwenye tabaka { $layer_id }
cmd-layer-state-layer-name = { $state } tabaka '{ $name }'
cmd-layer-unlocked = Haijafungwa
cmd-move-tool-moved-collars = Mabadiliko ya kuhamisha ({ $delta }) yametumika kwa kola { $count } za mashimo ya uchimbaji
cmd-move-tool-moved-objects = Mabadiliko ya kuhamisha ({ $delta }) yametumika kwa vitu { $count }
cmd-move-tool-count-hole-s = mashimo { $count }
cmd-object-edit-edited-kind = { $kind } imehaririwa
cmd-object-edit-edited-kind-count-vertices = { $kind } imehaririwa (vipeo { $count })
cmd-object-edit-no-changes-apply = Hakuna mabadiliko ya kutumia
cmd-object-edit-object-changed-since-editor-opened = Kitu hiki kimebadilika tangu kihariri kufunguliwa; kifungue tena ili kuhariri toleo la sasa
cmd-object-edit-target-changed = Lengo la uhariri wa kitu limebadilika; uhariri unaondolewa
cmd-object-edit-object-no-longer-exists-document = Kitu hicho hakipo tena kwenye hati
cmd-object-edit-select-single-design-object-edit = Chagua kitu kimoja cha muundo ili kukihariri
cmd-object-edit-unassigned = Haijawekwa
cmd-offset-create-offset = Unda Mkabala
cmd-offset-created-offset-count-object-s = Mkabala wa vitu { $count } umeundwa
cmd-offset-distance-must-be-positive = Umbali wa mkabala lazima uwe zaidi ya sifuri
cmd-offset-skipped-count-circle-s-offset = Miduara { $count } imerukwa: umbali wa mkabala ni mkubwa kuliko radiasi
cmd-omf-could-not-open-project-source = Imeshindwa kufungua mradi { $source_name }: { $error }
cmd-omf-create-open-project-before-merging = Unda au fungua mradi kabla ya kuunganisha data
cmd-omf-dataset-name-count-working-section = Seti ya data '{ $name }': sehemu { $count } za kazi hazikuweza kurejeshwa: { $details }
cmd-omf-field-codes-partly-coloured = Seti ya data '{ $name }': sehemu '{ $field }' ilihifadhiwa na misimbo { $saved } kati ya { $total } iliyopakwa rangi; iliyobaki ilipewa rangi zilizozalishwa.
cmd-omf-encoding-project = Inasimba mradi…
cmd-omf-exported-project-path = Umehamisha mradi kwenda { $path }
cmd-omf-imported-project = Mradi '{ $project_name }' umeingizwa kutoka { $source_name }: seti za data { $count } za ngazi ya juu
cmd-omf-importing-project = Inaingiza mradi…
cmd-omf-export-failed = Kuhamisha OMF kumeshindwa: { $error }
cmd-omf-import-failed = Kuingiza OMF kumeshindwa: { $error }
cmd-omf-opened-project = Mradi '{ $project_name }' umefunguliwa kutoka { $source_name }
cmd-omf-project-source-name-contains-no = Mradi '{ $source_name }' hauna vipengele vya data vinavyotumika
cmd-omf-source-name-applied-project-origin = { $source_name }: chanzo cha mradi { $origin } kilitumika kabla ya kuunganisha
cmd-omf-crs-differs = { $source_name }: mfumo wa marejeleo wa kuratibu '{ $source_crs }' unatofautiana na CRS ya mradi '{ $target_crs }'; kuratibu ziliunganishwa bila kuoanisha upya makadirio
cmd-omf-source-name-units-source-units = { $source_name }: vipimo '{ $source_units }' vinatofautiana na vipimo vya mradi '{ $target_units }'; kuratibu ziliunganishwa bila kubadilisha
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = Hakuna data ya Incline Design iliyofunguliwa ya kuhamisha
cmd-placement-2-vertices = vipeo 2
cmd-placement-count-vertices = vipeo { $count }
cmd-placement-created-circle = Duara limeundwa lenye radiasi m{ $radius }
cmd-placement-created-closed-polyline = Mstari wa pointi nyingi uliofungwa umeundwa wenye vipeo { $count }
cmd-placement-created-line-segment-2-vertices = Sehemu ya mstari yenye vipeo 2 imeundwa
cmd-placement-created-open-polyline-count-vertices = Mstari wa pointi nyingi ulio wazi umeundwa wenye vipeo { $count }
cmd-placement-placed-point-x-y-z = Kidokezo kimewekwa kwenye { $x }, { $y }, { $z }
cmd-placement-radius = Radiasi m{ $radius }
cmd-plot-composing-engineering-drawing = Inatunga mchoro wa uhandisi…
cmd-plot-could-not-write-engineering-drawing = Imeshindwa kuandika mchoro wa uhandisi: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Kipimo cha mchoro kimeoanishwa na data inayoonekana: 1:{ $scale }
cmd-plot = Mchoro
cmd-plot-saved-drawing = Mchoro wa uhandisi umehifadhiwa: { $description } ({ $width } × { $height } px kwa { $dpi } dpi)
cmd-point-cloud-classified = { $name } imeainishwa: ardhi { $ground }, uoto { $vegetation } na kelele { $noise } kati ya vidokezo { $count }
cmd-point-cloud-classifying-point-clouds = Inaainisha mawingu ya vidokezo
cmd-point-cloud-join-dropped-classifications = Uainishaji wa vidokezo umeachwa: baadhi ya mawingu yaliyounganishwa hayajaainishwa, na wingu lililoainishwa kwa sehemu haliwezi kuchujwa hadi ardhi.
cmd-point-cloud-failed-classify-point-clouds-error = Imeshindwa kuainisha mawingu ya vidokezo: { $error }
cmd-point-cloud-failed-join-point-clouds-error = Imeshindwa kuunganisha mawingu ya vidokezo: { $error }
cmd-point-cloud-failed-load-point-cloud-error = Imeshindwa kupakia wingu la vidokezo: { $error }
cmd-point-cloud-joined-count-clouds-into-name = Umeunganisha mawingu { $count } kuwa { $name } (vidokezo { $points })
cmd-point-cloud-joining-name = Inaunganisha { $name }
cmd-point-cloud-loaded-point-cloud-name-count = Umepakia wingu la vidokezo { $name } (vidokezo { $count })
cmd-point-cloud-point-cloud-classification-discarded = Uainishaji wa wingu la vidokezo umeondolewa: wingu limebadilika wakati ukiendeshwa. Uendeshe tena.
cmd-point-cloud-point-cloud-loader-disconnected-path = Kipakiaji cha wingu la vidokezo kimekatika kwa { $path }
cmd-point-cloud-select-one-more-loaded-point = Chagua wingu moja au zaidi la vidokezo lililopakiwa kabla ya kuyaainisha
cmd-point-cloud-select-two-more-loaded-point = Chagua mawingu mawili au zaidi ya vidokezo yaliyopakiwa kabla ya kuyaunganisha
cmd-point-cloud-tin-max-edge-disabled = (ukingo wa juu umezimwa)
cmd-point-cloud-tin-max-edge-max-edge = (ukingo wa juu { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = TIN ya wingu la vidokezo imeshindwa: { $error }
cmd-point-cloud-tin-filtered-ground = TIN ya ardhi: imechujwa hadi vidokezo { $ground } vya ardhi kati ya { $total }
cmd-point-cloud-tin-subsampled = TIN ya ardhi: sampuli za anga { $sampled } kati ya vidokezo { $total }
cmd-point-cloud-tin-triangulated = TIN ya ardhi: vidokezo { $vertex_count } vya kipekee vya XY vimetatuliwa kuwa nyuso { $face_count }{ $suffix }
cmd-products-added-product-delay-ms-ms = Umeongeza bidhaa { $delay_ms } ms { $name }
cmd-products-deleted-product-delay-ms-ms = Umefuta bidhaa { $delay_ms } ms { $name }
cmd-products-failed-save-products-error = Imeshindwa kuhifadhi bidhaa: { $error }
cmd-products-product-no-longer-palette = Bidhaa hiyo haipo tena kwenye pala
cmd-property-action-count-object-s-layer = { $action } vitu { $count } kwenye tabaka { $layer }
cmd-property-batch-set-axis-value-count = Weka thamani ya mhimili { $axis } kwa kundi la vitu { $count }
cmd-property-batch-set-closed-count-polyline = Weka hali ya kufungwa kwa kundi la mistari { $count } ya pointi nyingi
cmd-property-batch-set-color-count-object = Weka rangi kwa kundi la vitu { $count }
cmd-property-batch-set-fill-style-count = Weka mtindo wa kujaza kwa kundi la vitu { $count }
cmd-property-batch-set-line-weight-count = Weka uzito wa mstari kwa kundi la mistari { $count } ya pointi nyingi
cmd-property-copied = Imenakiliwa
cmd-property-moved = Imehamishwa
cmd-raster-draped = Rasta { $raster } imefunikwa juu ya utatuzi { $triangulation } (wigo unaopishana)
cmd-raster-failed-load-raster-name-error = Imeshindwa kupakia rasta { $name }: { $error }
cmd-raster-failed-load-raster-path-error = Imeshindwa kupakia rasta { $path }: { $error }
cmd-raster-loaded-raster-name-via-driver = Rasta { $name } imepakiwa kupitia { $driver } ({ $srcx }x{ $srcy }, hakikisho { $prevx }x{ $prevy })
cmd-raster-no-overlapping-triangulation = Hakuna utatuzi uliopakiwa unaopishana na wigo wa { $name }
cmd-raster-loader-disconnected = Kipakiaji cha rasta kimekatika kwa { $path }
cmd-raster-undraped = Umeondoa ufunikaji wa rasta kutoka utatuzi { $count }
cmd-reference-surface-build-surface-failed-error = Kujenga Uso kumeshindwa: { $error }
cmd-reference-surface-building-surface = Inajenga uso…
cmd-reference-surface-built-surface-name-from-vertex = Umejenga uso { $name } kutoka vidokezo { $vertex_count } hadi nyuso { $face_count }, kisanduku z { $low } hadi { $high }{ $support }{ $coincident }{ $controls }
cmd-reference-surface-control-string-index-could-not = Mfuatano wa udhibiti { $index } haukuweza kuongezwa kwenye mfumo
cmd-reference-surface-control-string-index-crosses-itself = Mfuatano wa udhibiti { $index } unajikatiza wenyewe kwenye mpango
cmd-reference-surface-control-string-index-doubles-back = Mfuatano wa udhibiti { $index } unajirudia wenyewe kwenye mpango
cmd-reference-surface-control-string-index-ends-where = Mfuatano wa udhibiti { $index } unaisha unapoanzia; ufunge ili kuutumia kama kinyago
cmd-reference-surface-control-string-index-has-count = Mfuatano wa udhibiti { $index } una vipeo { $count } tofauti; udhibiti unahitaji angalau { $minimum }
cmd-reference-surface-control-string-index-has-non = Mfuatano wa udhibiti { $index } una kuratibu zisizo kamili
cmd-reference-surface-control-string-index-no-longer = Mfuatano wa udhibiti { $index } haupatikani tena
cmd-reference-surface-control-string-index-runs-along = Mfuatano wa udhibiti { $index } unapita kando ya mfuatano wa wigo kwenye mpango; hilo halitumiki bado
cmd-reference-surface-control-string-overrides-pick-x = Mfuatano wa udhibiti unabatilisha uchaguzi kwenye ({ $x }, { $y }): uchaguzi m { $pick }, udhibiti m { $control }, tofauti m { $difference }
cmd-reference-surface-control-strings-b-disagree-x = Mifuatano ya udhibiti { $a } na { $b } hailingani kwenye ({ $x }, { $y }): m { $za } dhidi ya m { $zb }, tofauti ya m { $difference }
cmd-reference-surface-control-strings-b-run-along = Mifuatano ya udhibiti { $a } na { $b } inapita kando ya kila mmoja kwenye mpango; hilo halitumiki bado
cmd-reference-surface-count-control-string-s-vertices = ; mifuatano { $count } ya udhibiti yenye vipeo { $vertices }{ $crossings }
cmd-reference-surface-count-point-s-inside-extent = vidokezo { $count } ndani ya wigo; uso unahitaji angalau { $minimum }
cmd-reference-surface-count-point-s-outside-extent = ; vidokezo { $count } nje ya wigo viliumba uso kama tegemeo
cmd-reference-surface-count-point-s-selected-surface = vidokezo { $count } vimechaguliwa; uso unahitaji angalau { $minimum }
cmd-reference-surface-count-point-s-shared-plan = ; vidokezo { $count } vilishiriki nafasi ya mpango na vilihifadhiwa mara moja
cmd-reference-surface-delaunay-insert-failed-error = Kuingiza Delaunay kumeshindwa: { $error }
cmd-reference-surface-extent-must-closed-string = Wigo lazima uwe mfuatano uliofungwa
cmd-reference-surface-extent-string-crosses-itself-plan = Mfuatano wa wigo unajikatiza wenyewe kwenye mpango
cmd-reference-surface-extent-string-has-non-finite = Mfuatano wa wigo una kuratibu zisizo kamili
cmd-reference-surface-extent-string-needs-least-three = Mfuatano wa wigo unahitaji angalau vipeo vitatu tofauti
cmd-reference-surface-extent-string-no-longer-available = Mfuatano wa wigo haupatikani tena
cmd-reference-surface-meeting-count-crossing-s = zinakutana kwenye makutano { $count }
cmd-reference-surface-and-more = , … na { $more } zaidi
cmd-reference-surface-no-mask-selected-surface-unclipped = Hakuna kinyago kilichochaguliwa; uso haujakatwa
cmd-reference-surface-no-part-surface-falls-inside = Hakuna sehemu ya uso iliyo ndani ya wigo
cmd-reference-surface-open-project-before-building-surface = Fungua mradi kabla ya kujenga uso
cmd-reference-surface-points-collinear-plan-surface-needs = Vidokezo viko kwenye mstari mmoja kwenye mpango; uso unahitaji vitatu visivyo hivyo
cmd-reference-surface-select-exactly-one-closed-string = Chagua mfuatano mmoja tu uliofungwa wa kukata uso
cmd-reference-surface-selected-point-has-non-finite = Kidokezo kilichochaguliwa kina kuratibu zisizo kamili
cmd-reference-surface-selected-points-span-count-layers = Vidokezo vilivyochaguliwa vinahusu tabaka { $count }; uso umewekwa chini ya { $section }
cmd-relimit-click-missed = Weka Upya Kikomo: kubofya hakukugusa kitu chochote (hakuna kilicho chini ya kishale)
cmd-relimit-click-ignored = Weka Upya Kikomo: kubofya kumepuuzwa, chombo hakingoji uchaguzi wa lengo kwa sasa
cmd-relimit-clicked-source-line = Weka Upya Kikomo: umebofya mstari wa chanzo wenyewe, chagua mstari mwingine
cmd-relimit-no-source-line = Weka Upya Kikomo: hakuna mstari wa chanzo uliowekwa, inasitisha uchaguzi
cmd-relimit-relimited-line-source-id-selected = Mstari { $source_id } umewekwa upya kikomo kwa lengo lililochaguliwa
cmd-relimit-resized-line-source-id-using = Mstari { $source_id } umebadilishwa ukubwa kwa kutumia { $mode } thamani { $value }
cmd-rename-item-no-longer-belongs-active = Kipengele hicho hakihusiani tena na mradi hai
cmd-rename-renamed-before-name = '{ $before }' imebadilishwa jina kuwa '{ $name }'
cmd-rename-renamed-name-taken = '{ $before }' imebadilishwa jina kuwa '{ $name }' ('{ $requested }' tayari inatumika)
cmd-rotate-collar-turned-count-drillhole-collar-s = Kola { $count } za mashimo ya uchimbaji zimezungushwa { $rotation }
cmd-section-verb-count-item-s-section = { $verb } vipengele { $count } kwenye { $section }
cmd-selection-delete-vertex = Futa Kipeo
cmd-selection-deleted-count-selected-object-s = Vitu { $count } vilivyochaguliwa vimefutwa
cmd-selection-deleted-vertex = Kipeo { $vertex } kimefutwa kutoka mstari wa pointi nyingi { $object_id }
cmd-selection-duplicate-selection = Nakili Uteuzi
cmd-selection-duplicated-count-object-s = Vitu { $count } vimenakiliwa
cmd-session-created-triangulation = Utatuzi '{ $name }' umeundwa (vipeo { $vertex_count }, nyuso { $face_count }) kutoka aina ya uso { $surface_type }
cmd-session-deleted-triangulation = Utatuzi '{ $name }' umefutwa kutoka kwenye mradi
cmd-session-failed-load-triangulation-error = Imeshindwa kupakia utatuzi: { $error }
cmd-session-failed-load-triangulation-message = Imeshindwa kupakia utatuzi: { $message }
cmd-session-loaded-triangulation = Utatuzi '{ $name }' umepakiwa ({ $path }, vipeo { $vertex_count }, nyuso { $face_count })
cmd-session-set-triangulation-tri-id-color = Rangi ya utatuzi { $tri_id } imewekwa kuwa { $color }
cmd-session-triangulation-load-no-result = Upakiaji wa utatuzi wa { $path } umeisha bila matokeo
cmd-session-triangulation-failed = Operesheni ya utatuzi imeshindwa: { $message }
cmd-session-unloaded-triangulation-name = Utatuzi '{ $name }' umeondolewa
cmd-slice-entered-slice-view-cx-cy = Umeingia kwenye mwonekano wa ukataji @ { $cx }, { $cy }, { $cz } kando ya { $dx }, { $dy } (mstari wa m{ $length })
cmd-slice-exited-slice-view = Umetoka kwenye mwonekano wa ukataji
cmd-slice-reset-section-view-fit-extents = Rejesha mwonekano wa sehemu (oanisha na wigo)
cmd-slice-set-section-grid-enabled = Weka gridi ya sehemu = { $enabled }
cmd-split-created-2-open-polylines = Mistari 2 ya pointi nyingi iliyo wazi imeundwa
cmd-split-line = Gawanya Mstari
cmd-split-points-needs-interior-vertex = Gawanya kwa Vidokezo: chagua kipeo cha ndani cha mstari ulio wazi
cmd-split-polyline-into-two = Mstari wa chanzo wa pointi nyingi umegawanywa kuwa mistari miwili iliyo wazi ya pointi nyingi
cmd-text-edit-finished = Umemaliza kuhariri maandishi ya kitu { $object_id }
cmd-text-updated = Umesasisha maandishi kwenye kitu { $object_id }
cmd-view-centre-rotation-not-available-flying = Kitovu cha mzunguko hakipatikani katika Hali ya Kuruka
cmd-view-fixed-centre-rotation-x-y = Kitovu cha mzunguko kimefungwa kwenye { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = Hakuna kidokezo chini ya kishale cha kufungia kitovu cha mzunguko
cmd-view-released-centre-rotation = Kitovu cha mzunguko kimeachiliwa
cmd-view-reset-view-fit-extents = Rejesha mwonekano (oanisha na wigo)
cmd-view-set-cinematic-view-enabled = Weka mwonekano wa sinema = { $enabled }
cmd-view-set-topology-wireframes-enabled = Nyaya za tofolojia zimewekwa = { $enabled }
cmd-view-set-view-points-enabled = Mwonekano wa vidokezo umewekwa = { $enabled }
cmd-view-set-xy-grid-enabled = Weka gridi ya XY = { $enabled }
cmd-view-zoom-extents-preserving-angle = Kuza kwa wigo wote (ukihifadhi pembe)

## Common strings

common-add-product = Ongeza Bidhaa
common-appearance = Mwonekano wa Nje...
common-background = Mandhari nyuma
common-block-model = Mfano wa vitalu
common-block-models = Mifano ya Vitalu
common-borehole-inspector = Kikaguzi cha Shimo la Uchimbaji
common-build-surface = Jenga Uso
common-build-surface-ellipsis = Jenga Uso...
common-cancelled = Imeghairiwa
common-chamfer = Kingo iliyopunguzwa (Chamfer)
common-choose = Chagua...
common-circle = Duara
common-classify = Ainisha
common-classify-point-clouds = Ainisha Mawingu ya Vidokezo
common-click-point-fix-centre-rotation = Bofya kidokezo ili kufunga kitovu cha mzunguko
common-clip-surface-polyline = Kata Uso kwa Mstari wa Pointi Nyingi...
common-closed = Imefungwa
common-collection = Mkusanyiko
common-colour = Rangi
common-confirm-omf-rewrite = Thibitisha Uandikaji Upya wa OMF
common-could-not-replace-current-project = Imeshindwa kubadilisha mradi wa sasa: { $error }
common-count-object-s = vitu { $count }
common-create = Unda
common-create-batter-berm = Unda Mteremko na Lundo la Kinga
common-create-bezier-curve = Unda Mviringo wa Bezier
common-create-block-model = Unda Mfano wa Vitalu
common-create-block-model-ellipsis = Unda Mfano wa Vitalu...
common-create-circle = Unda Duara
common-create-drill-pattern = Unda Mfumo wa Uchimbaji
common-create-layer = Unda Tabaka
common-create-line = Unda Mstari
common-create-ore-triangulation = Unda Utatuzi wa Madini
common-create-ore-triangulation-ellipsis = Unda Utatuzi wa Madini...
common-create-point = Unda Kidokezo
common-create-polyline = Unda Mstari wa Pointi Nyingi
common-create-triangulation = Unda Utatuzi...
common-crosses = Misalaba
common-cut = Kata
common-cut-topology-pit-shell = Kata Tofolojia kwa Ganda la Shimo...
common-delete-collection = Futa Mkusanyiko
common-delete-layer = Futa Tabaka
common-delete-product = Futa Bidhaa
common-delete-selection = Futa Uteuzi
common-designs = Miundo
common-discard-layer-changes = Ondoa Mabadiliko ya Tabaka
common-down = Chini
common-drape-topology = Funika kwa Tofolojia
common-easting = Easting
common-edit-object = Hariri Kitu
common-edit-text = Hariri Maandishi
common-elevation = Kimo
common-exit-without-saving = Toka Bila Kuhifadhi
common-export-engineering-drawing = Hamisha Mchoro wa Uhandisi
common-file-was-left-out-downhole = { $file } iliachwa kwenye jiofizikia ya ndani ya shimo: { $error }
common-filter = Chuja
common-fly-mode = Hali ya Kuruka
common-generate-contour-lines = Zalisha Mistari ya Mwinuko...
common-hide-all = Ficha Yote
common-hide-selection = Ficha Uteuzi
common-hole-id = Kitambulisho cha Shimo
common-ignore = Puuza
common-import-csv-block-model = Ingiza CSV Mfano wa Vitalu
common-import-dxf = Ingiza DXF
common-incline-design-project = Mradi wa Incline Design
common-join = Unganisha...
common-join-point-clouds = Unganisha Mawingu ya Vidokezo
common-joined-cloud = Wingu Lililounganishwa
common-layer = Tabaka
common-legend = Kificho
common-line = Mstari
common-line-weight = Uzito wa mstari
common-link-geophysics = Unganisha Jiofizikia...
common-load-drillholes-before-linking-geophysics = Pakia seti ya data ya mashimo ya uchimbaji kabla ya kuunganisha jiofizikia nayo
common-lock-all = Funga Yote
common-lock-selection = Funga Uteuzi
common-m = m
common-max = Kiwango cha juu
common-merge-shell-into-topology = Unganisha Ganda kwenye Tofolojia
common-merge-shell-into-topology-ellipsis = Unganisha Ganda kwenye Tofolojia...
common-modelling = Uundaji wa Mifano
common-move-collar = Hamisha Kola
common-move-collection = Hamisha hadi Mkusanyiko
common-move-design = Hamisha Muundo
common-move-selection = Hamisha Uteuzi
common-name-has-no-readable-size = { $name } haina ukubwa unaosomeka
common-new-product = Bidhaa Mpya
common-no-block-models = Hakuna mifano ya vitalu
common-no-design-layers = Hakuna tabaka za muundo
common-no-drill-holes = Hakuna mashimo ya uchimbaji
common-no-file-chosen = Hakuna faili iliyochaguliwa
common-no-open-project = Hakuna mradi uliofunguliwa
common-no-point-clouds = Hakuna mawingu ya vidokezo
common-no-triangulations = Hakuna utatuzi
common-none = Hakuna
common-northing = Northing
common-offset = Mkabala
common-open = Fungua
common-orientation = Mwelekeo
common-point = Kidokezo
common-point-cloud = Wingu la vidokezo
common-point-clouds = Mawingu ya Vidokezo
common-polyline = Mstari wa Pointi Nyingi
common-polyline-layer = Mstari wa pointi nyingi kwenye '{ $layer }'
common-project = Mradi
common-rasters = Rasta
common-redo = Rudia
common-reference-points = Vidokezo vya Marejeo...
common-relimit-line = Weka Upya Kikomo cha Mstari
common-remove-project = Ondoa Mradi
common-reset-view = Rejesha Mwonekano
common-reveal-all = Onyesha Yote
common-reveal-finder = Onyesha kwenye Finder
common-rotate-collar = Zungusha Kola
common-save-exit = Hifadhi na Toka
common-scale-bar = Pau ya kipimo
common-set-initiation-point = Weka Kidokezo cha Kuanzia
common-shape = Umbo
common-shell = Na Ganda
common-slashes = Mikwaju
common-slice = Kata
common-slice-triangulation-z-range = Kata Utatuzi kwa Wigo wa Z...
common-surface-contours = Mistari ya Mwinuko ya Uso
common-text = Maandishi
common-degree-suffix = °
common-tie-holes = Unganisha Mashimo
common-triangulations = Utatuzi
common-trim-topology = Punguza kwa Tofolojia...
common-undo = Tendua
common-undrape-all = Ondoa Ufunikaji Yote
common-uniform-white = Nyeupe sawia
common-unknown = Haijulikani
common-unlock-all = Fungua Yote
common-untitled = Bila Jina
common-up = Juu
common-vertical-exaggeration = Kuzidisha Wima
common-x = x
common-zoom-extents = Kuza kwa Wigo Wote

## Confirmations strings

confirmations-close-project-unsaved-changes = Funga Mradi: Mabadiliko Hayajahifadhiwa
confirmations-close-without-saving = Funga Bila Kuhifadhi
confirmations-delete = Futa
confirmations-delete-objects = Futa Vitu
confirmations-discard = Ondoa
confirmations-discard-all-unsaved-changes-layer =
    Ondoa mabadiliko yote yasiyohifadhiwa kwa tabaka '{ $name }'?
    Tabaka lililohifadhiwa linapakiwa upya kutoka kwenye diski wakati mabadiliko kwa tabaka mengine yanahifadhiwa. Hatua hii haiwezi kutenduliwa.
confirmations-discard-all-unsaved-changes-name =
    Ondoa mabadiliko yote yasiyohifadhiwa kwa '{ $name }'?
    Toleo la mwisho lililohifadhiwa linapakiwa upya kutoka kwenye diski. Hatua hii haiwezi kutenduliwa.
confirmations-discard-changes = Ondoa Mabadiliko
confirmations-exit-unsaved-changes = Toka: Mabadiliko Hayajahifadhiwa
confirmations-incline-design-cannot-reproduce-all = Incline Design haiwezi kuzalisha upya maudhui yote kutoka OMF asili. Kuhifadhi kutaacha maudhui yafuatayo:
confirmations-product = Bidhaa
confirmations-project = mradi huu
confirmations-remove-name-delete-its-browser = Ondoa '{ $name }' na ufute nakala yake iliyohifadhiwa kwenye kivinjari? Mabadiliko yasiyohifadhiwa yatapotea.
confirmations-remove-project-unsaved-changes = Ondoa Mradi: Mabadiliko Hayajahifadhiwa
confirmations-remove-without-saving = Ondoa Bila Kuhifadhi
confirmations-replace-project-unsaved-changes = Badilisha Mradi: Mabadiliko Hayajahifadhiwa
confirmations-save = Hifadhi
confirmations-save-anyway = Hifadhi Hata Hivyo
confirmations-save-changes-current-project-before = Hifadhi mabadiliko kwenye mradi wa sasa kabla ya kuubadilisha?
confirmations-save-changes-name-before-closing = Hifadhi mabadiliko kwa '{ $name }' kabla ya kuyafunga?
confirmations-save-changes-name-before-removing = Hifadhi mabadiliko kwa '{ $name }' kabla ya kuyaondoa kutoka Incline Design?
confirmations-save-close = Hifadhi na Funga
confirmations-save-modified-project-before-exiting = Hifadhi mradi ulioboreshwa kabla ya kutoka?
confirmations-save-to-browser-before-exit = Hifadhi mradi ulioboreshwa kwenye hifadhi ya kivinjari kabla ya kutoka?
confirmations-save-remove = Hifadhi na Ondoa

## Console strings

console-copy-all = Nakili yote
console-copy-message = Nakili ujumbe
console-error = HITILAFU
console-info = TAARIFA
console-no-console-activity-yet = Hakuna shughuli ya dashibodi bado
console-pending = INASUBIRI
console-progress-summary = Inaendelea · { $summary }
console-success = MAFANIKIO
console-warn = ONYO

## Csv strings

csv-block-model-category = Kategoria
csv-block-model-value = Thamani
csv-drill-hole-rows-for-undefined-holes = safu { $count } zilikuwa za shimo ambalo jiometri ya kifurushi haliainishi
csv-drill-hole-count-rows-were-skipped-total = safu { $count } zilirukwa kwa jumla
csv-drill-hole-csv-file-empty = Faili ya CSV ni tupu
csv-drill-hole-csv-has-too-many-unreadable = CSV ina baiti nyingi mno zisizosomeka kurekebishwa; huenda iko katika usimbaji wa zamani, kwa hivyo ihifadhi kama UTF-8 kisha uiingize tena
csv-drill-hole-csv-header-has-no-columns = Kichwa cha CSV hakina safu wima
csv-drill-hole-csv-headers-must-nonblank-unique = Vichwa vya CSV lazima visiwe tupu na viwe vya kipekee
csv-drill-hole-geophysics-needs-geometry = Jiofizikia ya ndani ya shimo inahitaji faili ya kola au ya sehemu dhahiri kwenye kifurushi, ambayo mashimo yake inayaambatisha
csv-drill-hole-azimuth-out-of-range = { $file } ina safu { $count } ambazo azimuth yake haiko kati ya 0 na 360
csv-drill-hole-dip-out-of-range = { $file } ina safu { $count } ambazo mteremko wake haupo kati ya -90 na 90; safu hizo zilisomwa bila mwelekeo
csv-drill-hole-file-inclination-values-could-angle = Thamani za mwinuko katika { $file } ambazo zingeweza kuwa pembe zote ni sifuri au chini, kwa hivyo safu wima ilisomwa kama mteremko, hasi kuelekea chini
csv-drill-hole-file-maps-gamma-density-column = { $file } inaoanisha safu wima ya gamma au msongamano mara mbili
csv-drill-hole-invalid-utf8 = { $file } si UTF-8 halali; baiti { $count } zisizosomeka zilibadilishwa katika seli { $cells }; seli iliyoharibika haisomwi kama data
csv-drill-hole-file-requires-gamma-density-column = { $file } inahitaji safu wima ya gamma au msongamano
csv-drill-hole-row-undefined-hole = { $file } safu { $row } ni ya DHID '{ $dhid }', shimo ambalo jiometri ya kifurushi haliainishi
csv-drill-hole-holes-hole-s-carry-overlapping = mashimo { $holes } yana vipindi vinavyopishana, kama mshipa uliorekodiwa pamoja na sehemu zake: { $summary }
csv-drill-hole-skipped-row-reason = Safu imerukwa: { $reason }
csv-geophysics-above-5 = juu ya 5
csv-geophysics-below-0-5 = chini ya 0.5
csv-geophysics-count-more = (+{ $count } zaidi)
csv-geophysics-count-rows-were-skipped-total = safu { $count } zilirukwa kwa jumla katika { $file }
csv-geophysics-csv-has-record-longer-than = CSV ina rekodi ndefu kuliko MiB { $limit }: faili haina viishilio vya mstari mahali CSV inapovitarajia, au si maandishi
csv-geophysics-csv-has-unterminated-quoted-field = CSV ina sehemu iliyo ndani ya nukuu isiyofungwa
csv-geophysics-curve-file-was-left-out = { $curve } katika { $file } iliachwa: usomaji wake mwingi ni { $side }, kwa hivyo wastani wake uko nje ya 0.5 hadi 5 g/cc na kipimo chake kinaonekana si sahihi (g/cc inatarajiwa). Incline haibadilishi vipimo; sahihisha uhamishaji kisha uunganishe tena
csv-geophysics-file-empty = { $file } ni tupu
csv-geophysics-file-has-no-curve-no = { $file } haina mkunjo: hakuna safu wima mbali na kitambulisho cha shimo na kina yenye nambari
csv-geophysics-file-mapping-has-mapped-columns = Uoanishaji wa { $file } una safu wima { $mapped }, CSV ina { $found }
csv-geophysics-file-no-longer-matches-its = { $file } hailingani tena na kielezo chake: iunganishe tena
csv-geophysics-file-not-grouped-hole-its = { $file } haijapangwa kwa shimo: safu za mashimo yake zimegawanywa katika mfululizo mwingi mno. Ipange kwa kitambulisho cha shimo, kisha kina, na uiunganishe tena
csv-geophysics-file-requires-one-dhid-one = { $file } inahitaji safu wima moja ya DHID na moja ya kina
csv-geophysics-row-blank-hole-id = { $file } safu { $row } ina kitambulisho cha shimo tupu
csv-geophysics-row-column-count = { $file } safu { $row } ina safu wima { $found }; zinatarajiwa { $expected }
csv-geophysics-row-negative-depth = { $file } safu { $row } ina kina hasi
csv-geophysics-row-no-depth = { $file } safu { $row } haina kina kinachosomeka
csv-geophysics-file-s-path-not-valid = njia ya faili si UTF-8 halali, jambo ambalo mradi hauwezi kuhifadhi: badilisha jina la faili au folda yake kisha uiunganishe tena
csv-geophysics-rows-skipped = { $file }: safu { $skipped } kati ya { $rows } hazikuweza kusomwa; sababu ziko kwenye dashibodi
csv-geophysics-runs-not-grouped = Jiofizikia ya mashimo { $count } inakuja katika zaidi ya mfululizo mmoja, haijapangwa kwa shimo; kila mfululizo unaofuata huongeza kina ambacho shimo lake halina usomaji pekee: { $holes }
csv-geophysics-linked-downhole-geophysics-from-file = Umeunganisha jiofizikia ya ndani ya shimo kutoka { $file }: mashimo { $holes }, mikunjo { $curves }; safu { $rows } zimesomwa, { $skipped } zimerukwa. Usomaji unabaki kwenye faili na husomwa shimo moja kwa wakati
csv-geophysics-no-readings = hakuna usomaji
csv-geophysics-no-usable-depth-step = hakuna hatua ya kina inayoweza kutumika
csv-geophysics-run-count-mismatch = Mfululizo { $read } wa { $hole } umesomwa, kiungo kina { $runs }
csv-geophysics-rows-geophysics-row-s-count = safu { $rows } za jiofizikia za mashimo { $count } ambayo seti ya data haiyaainishi hazijaunganishwa: { $holes }
csv-geophysics-rows-readings-would-need-samples = usomaji { $rows } ungehitaji sampuli { $samples }
csv-geophysics-run-hole-curve-was-not = Mfululizo wa { $hole } { $curve } haukuhifadhiwa ({ $reason })
data-table-copy-selection = Nakili uteuzi
data-table-copy-table = Nakili jedwali
drill-hole-add = Ongeza
drill-hole-add-all = Ongeza yote

## Drill strings

drill-hole-add-stop = Ongeza kituo
drill-hole-add-working-section = Ongeza sehemu ya kazi
drill-hole-all-rendered-intervals-opaque-white = Vipindi vyote vilivyochorwa ni nyeupe isiyopenyeza.
drill-hole-another-working-section-field-has = Sehemu nyingine ya kazi ya sehemu hii ina jina hilo.
drill-hole-assumed = Inadhaniwa
drill-hole-burden-spacing-must-greater-than = Burden na nafasi lazima ziwe zaidi ya sifuri
drill-hole-cache-drill-hole-set-name-has = Seti ya mashimo ya uchimbaji { $name } ina mashimo na viunganishi { $count }, zaidi ya { $capacity } ambavyo mwangaza wa uteuzi unaweza kubeba: kuchagua seti nzima bado kunaiangaza, kuchagua mashimo mmoja mmoja hakutaangaza
drill-hole-cache-drill-hole-set-name-stations = Seti ya mashimo ya uchimbaji { $name }: vituo { $stations }, sehemu { $before } zimeunganishwa hadi { $after }, seli { $cells }
drill-hole-choose-valid-closed-polyline = Chagua mstari halali wa pointi nyingi uliofungwa
drill-hole-clear-filter = Futa kichujio
drill-hole-code-already-in-section = { $code } tayari iko kwenye sehemu ya kazi { $section }.
drill-hole-code-outside-section-has-name = Msimbo ulio nje ya sehemu hii una jina hilo. Sehemu inaweza kushiriki jina lake na msimbo inaoushikilia tu.
drill-hole-colour-scale = Kipimo cha rangi
drill-hole-count-codes = misimbo { $count }
drill-hole-count-codes-interval-no-logged = misimbo { $count }. Kipindi kisicho na thamani iliyorekodiwa kinabaki cheupe.
drill-hole-disc-diameter = Kipenyo cha diski
drill-hole-appearance-title = Mwonekano wa Shimo la Uchimbaji: { $name }
drill-hole-drilled-diameter = Cha kipenyo kilichochimbwa
drill-hole-every-code-lists-already-another = Kila msimbo inaoorodhesha tayari uko kwenye sehemu nyingine ya kazi.
drill-hole-every-interval-value-colour-field = Kila kipindi chenye thamani kwenye sehemu ya rangi huchorwa kama diski ya upana huu kwenye mfuatano. Kwa mbali hakiwi chembamba kuliko pikseli chache.
drill-hole-field = Sehemu
drill-hole-field-working-section = { $field } kwa sehemu ya kazi
drill-hole-floor = Sakafu
drill-hole-grayscale = Kijivu
drill-hole-green-yellow-red = Kijani–Njano–Nyekundu
drill-hole-heat = Joto
drill-hole-drilled-width-help = Shimo kwa upana wake uliochimbwa linaonekana kama bomba kando ya jiolojia; seti ya maelfu inaonekana kama mkeka.
drill-hole-line-width-help = Shimo lenyewe huchorwa kama mstari wa upana huu kwa kila kiwango cha kukuza.
drill-hole-however-far-eye-hole-drawn = Jicho likiwa mbali kiasi chochote, shimo huchorwa kwa angalau upana huu.
drill-hole-measured = Imepimwa
drill-hole-name-working-section = { $name } (sehemu ya kazi)
drill-hole-never-thinner-than = Kamwe si chembamba kuliko
drill-hole-new-section-name = Jina la sehemu mpya
drill-hole-new-working-section = Sehemu mpya ya kazi
drill-hole-no-holes-fit-inside-boundary = Hakuna mashimo yanayotoshea ndani ya mpaka huu kwa burden na nafasi ya sasa
drill-hole-part-code = Sehemu ya msimbo
drill-hole-pattern-too-many-holes = Mfumo unazidi kiwango cha juu cha mashimo { $maximum }; ongeza burden au nafasi
drill-hole-preset = Mpangilio
drill-hole-px = px
drill-hole-rainbow = Upinde wa mvua
drill-hole-reset-colours = Rejesha rangi
drill-hole-reset-preset = Rejesha mpangilio
drill-hole-reset-shown-colours = Rejesha rangi zinazoonyeshwa
drill-hole-roof = Paa
drill-hole-rotation-offsets-must-contain-valid = Mzunguko na mkabala lazima ziwe na nambari sahihi
drill-hole-selected-polyline-has-no-usable = Mstari wa pointi nyingi uliochaguliwa hauna eneo la XY linalotumika
drill-hole-shown-total-codes-shown = misimbo { $shown } kati ya { $total } inaonyeshwa
drill-hole-shown-total-rows-shown = safu { $shown } kati ya { $total } zinaonyeshwa
drill-hole-smooth-interpolation = Uingiliano laini
drill-hole-spacing-would-scan-too-many = Nafasi hii ingechunguza seli nyingi mno za gridi; ongeza burden au nafasi (kiwango cha juu mashimo { $maximum })
drill-hole-square = Mraba
drill-hole-staggered = Kwa zamu
drill-hole-stepped-bands = Mikanda yenye ngazi
drill-hole-string-discs = Mfuatano na diski
drill-hole-string-discs-where-intervals-overlap = Kama mfuatano na diski, vipindi vinapopishana kifupi zaidi huchorwa kama diski.
drill-hole-string-width = Upana wa mfuatano
drill-hole-style = Mtindo
drill-hole-suggested-from-code-names-count = Inapendekezwa kutoka majina ya misimbo ({ $count })
common-times-sign = ×
common-minus-sign = −
drill-hole-ticked-but-hidden-filter-count = Imetiwa alama lakini imefichwa na kichujio: { $count }
drill-hole-true-diameter = Kipenyo halisi
drill-hole-unsupported-drillhole-source = Chanzo cha shimo la uchimbaji kisichotumika
drill-hole-width = Upana
drill-hole-working-section-needs-name = Sehemu ya kazi inahitaji jina.
drill-hole-working-section-set-seams-plies = Sehemu ya kazi ni seti ya mishipa au tabaka zinazochimbwa kama kitengo kimoja. Kupaka rangi kwa sehemu hiyo hupa seti nzima rangi moja.
drill-hole-working-sections = Sehemu za kazi
drill-pattern-arrangement = Mpangilio
drill-pattern-axis-offset = Mkabala wa { $axis }
drill-pattern-blast-shape = Umbo la ulipuaji
drill-pattern-burden = Burden
drill-pattern-choose-closed-blast-boundary-then = Chagua mpaka uliofungwa wa ulipuaji, kisha rekebisha gridi. Mashimo ya uchimbaji husasishwa moja kwa moja kwenye mwonekano.
drill-pattern-closed-design-polyline-whose-xy = Mstari wa muundo wa pointi nyingi uliofungwa ambao eneo lake la XY litajazwa mashimo.
drill-pattern-rotation-help = Mzunguko wa mfumo kinyume cha saa kutoka mhimili wa { $axis } wa dunia.
drill-pattern-distance-between-holes-along-each = Umbali kati ya mashimo kando ya kila safu ya mfumo.
drill-pattern-name-hint = mfano: West Cut 03
drill-pattern-diameter-help = Kipenyo cha shimo lililokamilika. Kimewekwa kwa milimita na kuhifadhiwa na kila shimo linalozalishwa.
drill-pattern-hole-depth = Kina cha shimo
drill-pattern-hole-diameter = Kipenyo cha shimo
drill-pattern-move-over-closed-polyline-then = Sogeza juu ya mstari wa pointi nyingi uliofungwa, kisha ubofye kwenye mwonekano. Esc hughairi uchaguzi.
drill-pattern-name-help = Jina la data ya mashimo ya uchimbaji iliyoundwa kwenye mradi.
drill-pattern-none-picked = Hakuna kilichochaguliwa
drill-pattern-pattern-name = Jina la mfumo
drill-pattern-spacing-help = Umbali wa pembeni kati ya safu za mfumo.
drill-pattern-pick = Chagua
drill-pattern-preview-count-hole-s-diameter = Hakikisho: mashimo { $count } · kipenyo mm{ $diameter } · kina m{ $depth }
drill-pattern-rotation = Mzunguko
drill-pattern-shift-pattern-grid-along-global = Hamisha gridi ya mfumo kando ya mhimili wa { $axis } wa dunia huku ukiiweka ndani ya umbo la ulipuaji.
drill-pattern-spacing = Nafasi
drill-pattern-staggered-offsets-every-second-row = Kwa zamu hukabalisha kila safu ya pili kwa nusu ya nafasi.
drill-pattern-vertical-depth-below-each-collar = Kina wima chini ya kila kola.

## Dxf strings

dxf-block-nesting-too-deep = Uwekaji ndani wa vitalu vya DXF unazidi kina cha juu ({ $depth }), inaruka '{ $name }'
dxf-circular-block-reference = Rejeleo la mviringo la kitalu cha DXF limegunduliwa: '{ $name }'
dxf-undefined-layer = Kitu cha DXF kimerejelea tabaka lisilofafanuliwa '{ $name }', kimeingizwa kama '{ $fallback }'
dxf-import-budget-exceeded = Uingizaji wa DXF unazidi kikomo cha bajeti ya { $what } ({ $limit }); jiometri iliyobaki imerukwa
dxf-insert-unknown-block = INSERT ya DXF inarejelea kitalu kisichojulikana '{ $name }'

## Edit strings

edit-absolute-length = Urefu kamili
edit-absolute-rl = RL Kamili
edit-action = Kitendo
edit-angle = Pembe
edit-dip-help = Pembe kutoka mlalo, hasi chini: -90 ni shimo wima.
edit-app-web-not-recommended-production = { $app } Web haipendekezwi kwa matumizi ya uzalishaji. Itumie kama onyesho tu.
edit-application = Programu
edit-apply = Tumia
edit-apply-pick-target = Tumia na Chagua Lengo
edit-axis-value = thamani ya { $axis }
edit-azimuth = Azimuth
edit-batter-angle = Pembe ya mteremko (°)
edit-azimuth-help = Mwelekeo ambao mashimo yanachimbwa, kwa digrii kwa mwendo wa saa kutoka kaskazini ya gridi.
edit-bench-height = Kimo cha ngazi
edit-benches = Ngazi
edit-berm-width = Upana wa lundo la kinga
edit-bezier-curve = Mviringo wa Bezier
edit-choose-layer = Chagua tabaka
edit-measure-help = Chagua kama thamani iliyowekwa ni umbali kwenye mteremko, upana wa mlalo, au kimo wima.
edit-choose-which-two-polyline-paths = Chagua ni njia gani kati ya njia mbili za mstari wa pointi nyingi kati ya vipeo vilivyochaguliwa itakayobadilishwa. Urefu unajumuisha kimo na kingo zilizopinda.
edit-click-corner-closed-polyline = Bofya kona kwenye mstari wa pointi nyingi uliofungwa.
edit-click-open-closed-polyline-begin = Bofya mstari wa pointi nyingi ulio wazi au uliofungwa ili kuanza.
edit-click-second-vertex-replacement-span = Bofya kipeo cha pili cha sehemu ya ubadilishaji.
edit-click-vertex-start-replacement-span = Bofya kipeo ili kuanza sehemu ya ubadilishaji.
edit-collide-triangulation = Gongana na Utatuzi
edit-confirm-selection = Thibitisha Uteuzi
edit-control-point-1 = Kidokezo cha udhibiti 1
edit-control-point-2 = Kidokezo cha udhibiti 2
edit-copy = Nakili
edit-corner-radius-limited-so-replacement = Radiasi ya kona, imepunguzwa ili ubadilishaji usipite vipeo vya jirani.
edit-create-new-layer = Unda tabaka jipya
edit-create-new-project = Unda mradi mpya
edit-create-project = Unda mradi
edit-delta-length-m-use = Tofauti ya urefu (m, tumia + au -)
edit-dip = Mteremko (Dip)
edit-direction = Mwelekeo
edit-distance = Umbali
edit-distance-along-slope = Umbali kwenye mteremko
edit-download-free-native-version-our = Pakua toleo asili la bure kwenye tovuti yetu ↗
edit-drill-hole = Shimo la Uchimbaji
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = Mwisho
edit-enter-valid-elevation = Weka kimo sahihi.
edit-exit-slice = Toka kwenye Ukataji
edit-finish-polyline = Maliza Mstari wa Pointi Nyingi
edit-generate-batter-berms = Zalisha Miteremko na Malundo ya Kinga
edit-height = Kimo
edit-height-change = Mabadiliko ya kimo
edit-height-mode = Hali ya kimo
edit-horizontal-distance = Umbali wa mlalo
edit-horizontal-width-each-flat-berm = Upana wa mlalo wa kila lundo tambarare kati ya miteremko inayofuatana.
edit-hover-choose-which-end-move = Elekeza kipanya ili kuchagua ncha ya kuhamisha, kisha bofya ili kuthibitisha.
edit-insert-point-elevation = Ingiza Kidokezo kwenye Kimo
edit-intersect = Katiza
edit-kind-properties = { $kind } { $properties }
edit-layer-name = Jina la tabaka
edit-load-project = Pakia Mradi
edit-longest = Ndefu zaidi
edit-m-s = m/s
edit-measure = Pima
edit-mit-license = Leseni ya MIT
edit-mode = Hali
edit-move = Hamisha
edit-move-layer = Hamishia kwenye Tabaka
edit-move-which-end = Hamisha ncha ipi
edit-movement-speed-slice-when-using = Kasi ya mwendo wa ukataji unapotumia vitufe vya usogezaji.
edit-moving-end-endpoint = Inahamisha: Ncha ya Mwisho
edit-moving-start-endpoint = Inahamisha: Ncha ya Mwanzo
edit-new-length-m = Urefu mpya (m)
edit-new-project = Mradi Mpya
edit-number-complete-batter-berm-levels = Idadi ya ngazi kamili za mteremko na lundo. Kiwango cha juu kimepunguzwa kwa ngazi ya kina zaidi inayohifadhi jiometri iliyoainishwa.
edit-bezier-segments-help = Idadi ya sehemu za mstari zinazotumika kukadiria mviringo kati ya vipeo viwili vilivyochaguliwa.
edit-chamfer-segments-help = Idadi ya sehemu za moja kwa moja zinazotumika kukadiria kona iliyoviringika. Tumia 1 kwa kingo iliyopunguzwa iliyonyooka.
edit-object = Kitu
edit-offset-element = Kipengele cha Mkabala
edit-pick-side = Chagua Upande
edit-pit = Shimo
edit-project-name = Jina la mradi
edit-properties = Sifa
edit-radius = Radiasi
edit-recent = Hivi Karibuni
edit-relative = Jamaa (+/-)
edit-elevation-mode-help = Jamaa hutumia mabadiliko ya wima kwa kila kidokezo. RL Kamili huweka kila kidokezo kwenye kimo kimoja lengwa.
edit-remove-from-list = Ondoa kwenye Orodha
edit-replace-path = Badilisha njia
edit-rotate = Zungusha
edit-rotation-speed-slice-when-using = Kasi ya mzunguko wa ukataji unapotumia Q na E.
edit-s = °/s
edit-segments = Sehemu
edit-segments-lying-elevation-ignored = Sehemu zilizo kwenye kimo hiki hazizingatiwi.
edit-endpoint-help = Chagua ncha inayobadilika; ncha nyingine inabaki thabiti.
edit-selected-holes-point-different-ways = Mashimo yaliyochaguliwa yanaelekea njia tofauti. Tumia huyaweka yote kwenye pembe hizi.
edit-selected-start-end-point-moves = Kidokezo cha mwanzo au mwisho kilichochaguliwa husogea kando ya mwelekeo wa mstari; ncha nyingine inabaki thabiti.
edit-set-axis = Weka { $axis }
edit-shortest = Fupi zaidi
edit-slice-view = Mwonekano wa Ukataji
edit-slope-angle-each-batter-face = Pembe ya mteremko wa kila uso wa mteremko, iliyopimwa kutoka mlalo.
edit-slope-angle-offset-positive-negative = Pembe ya mteremko wa mkabala. Pembe chanya na hasi huhamisha nakala juu au chini ya chanzo inaposogea kando.
edit-speed = Kasi
edit-start = Mwanzo
edit-stockpile = Rundo la Hifadhi
edit-stop-generated-offset-where-its = Simamisha mkabala uliozalishwa pale njia yake inapokutana kwa mara ya kwanza na utatuzi unaoonekana.
edit-target-rl = RL Lengwa
edit-text-colour-opacity = Rangi na uwazi wa maandishi.
edit-thickness-visible-slice-slab-centred = Unene wa bamba la ukataji linaloonekana, likiwa katikati ya kiashiria cha muhtasari.
edit-translation-axis-help = Umbali wa mhamisho kando ya mhimili wa { $axis } wa dunia.
edit-type = Aina
edit-type-direction-together-set-offset = Aina na Mwelekeo pamoja huweka upande wa mkabala. Shimo + Juu na Rundo la Hifadhi + Chini husogea nje; Shimo + Chini na Rundo la Hifadhi + Juu husogea ndani.
edit-bench-direction-help = Juu huinua kila ngazi kwa kimo cha ngazi; Chini huishusha. Hii pia hubadilisha upande wa mkabala - angalia Aina.
edit-value-help = Thamani hufasiriwa kwa kutumia Kipimo na Hali ya Kimo iliyochaguliwa.
edit-vertical-rise-fall-each-bench = Kupanda au kushuka wima kwa kila ngazi kabla lundo linalofuata halijaundwa.
edit-bezier-control-point-1-help = Kuratibu za X, Y na Z za dunia za kidokezo cha kwanza cha udhibiti cha Bezier.
edit-bezier-control-point-2-help = Kuratibu za X, Y na Z za dunia za kidokezo cha pili cha udhibiti cha Bezier.

## Events strings

events-couldn-t-exit-error = Imeshindwa kutoka: { $error }
events-couldn-t-save-error = Imeshindwa kuhifadhi: { $error }
events-set-elevation = Weka Kimo
events-set-elevation-from-cursor-hit = Kimo kimewekwa kutoka mgusano wa kishale hadi Z { $z }
events-tool-not-available-section-view = Chombo hicho hakipatikani kwenye mwonekano wa sehemu

## Explorer strings

explorer-clear-active-triangulation-texture = Futa Umbile la Utatuzi Hai
explorer-delete-from-project = Futa kutoka kwenye Mradi
explorer-discard-changes = Ondoa Mabadiliko...
explorer-download = Pakua
explorer-drape-over-surface = Funika juu ya Uso
explorer-draped-over-surface = Umefunikwa juu ya uso
explorer-duplicate = Nakala
explorer-empty-collection = Mkusanyiko mtupu
explorer-face-colour = Rangi ya uso
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    vigezo { $count } vya rangi
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    mashimo { $holes }
    sehemu { $fields } za rangi
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    vidokezo { $count }
explorer-raster-id =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = Pakia
explorer-lock = Funga
explorer-new-collection = Mkusanyiko Mpya
explorer-no-collection = Hakuna Mkusanyiko
explorer-select-all-objects = Chagua Vitu Vyote
explorer-source-name = Chanzo: { $name }
explorer-unload = Ondoa
explorer-unlock = Fungua

## Files strings

files-automatic-colour = Rangi kiotomatiki
files-automatic-rl-spacing = Nafasi ya RL kiotomatiki
files-axis-scale-ratio = Uwiano wa kipimo wa { $axis }
files-ok = Sawa
files-reset-scale = Rejesha kwa 1×
files-rl-grid-options = Chaguzi za Gridi ya RL
files-rl-spacing = Nafasi ya RL
files-scales-z-distances-visually-without = Hupima umbali wa Z kwa kuonekana bila kubadilisha kuratibu zilizohifadhiwa.
files-thickness = Unene
files-xy-grid-options = Chaguzi za Gridi ya XY
geophysics-checking-geophysics-files = Inakagua faili za jiofizikia
geophysics-downhole-geophysics-name-could-not = Jiofizikia ya ndani ya shimo ya '{ $name }' haikuweza kuunganishwa: { $error }
geophysics-file-changed = Faili ya jiofizikia imebadilika tangu kielezo chake kilipotengenezwa
geophysics-file-unreadable = Faili ya jiofizikia iliyounganishwa na '{ $name }' haiwezi kusomwa kwenye { $path } ({ $error }); iunganishe tena kutoka menyu ya kubofya kulia ya seti ya data
geophysics-linked-changed-rereading = Jiofizikia iliyounganishwa na '{ $name }' imebadilika tangu kielezo chake kilipotengenezwa; inaisoma tena
geophysics-hole-has-size-mib-geophysics = { $hole } ina MiB { $size } za safu za jiofizikia, zaidi ya kiasi ambacho shimo husomwa
geophysics-hole-needs-size-mib-its = { $hole } inahitaji MiB { $size } kwa jiofizikia yake, zaidi ya kilichobaki kwenye kivinjari: ondoa vipengele vingine, kisha ondoa na upakie seti hii ya data tena
geophysics-linking-geophysics-name = Inaunganisha jiofizikia na { $name }
geophysics-reading-geophysics-hole = Inasoma jiofizikia ya { $hole }
geophysics-web-could-not-read-name-error = Imeshindwa kusoma '{ $name }': { $error }
geophysics-web-name-used-session-s-downhole = '{ $name }' inatumika kwa jiofizikia ya ndani ya shimo ya kipindi hiki

## Gpu strings

gpu-cache-block-model-surface-build-failed = Ujenzi wa uso wa mfano wa vitalu umeshindwa: { $error }
gpu-cache-block-model-surface-build-worker = Kifanyakazi cha ujenzi wa uso wa mfano wa vitalu kimekatika
gpu-cache-block-model-surface-chunk-rejected = Kipande cha uso wa mfano wa vitalu kimekataliwa kabla ya ugawaji wa GPU: instances={ $instances } bytes, limit={ $limit } bytes
gpu-cache-block-volume-worker-disconnected = Kifanyakazi cha kuandaa ujazo wa vitalu kimekatika
gpu-cache-translucent-volume-could-not-built = Ujazo wa uwazi haukuweza kujengwa ({ $error }); mfano huu wa vitalu unaonyeshwa kama vijicubu badala yake.
gpu-cache-edge-chunk-rejected = Kipande cha ukingo wa utatuzi kimekataliwa kabla ya ugawaji wa GPU: instances={ $instances } bytes, limit={ $limit } bytes
gpu-cache-triangulation-chunk-rejected = Kipande cha GPU cha utatuzi kimekataliwa kabla ya ugawaji: vertices={ $vertices } bytes, indices={ $indices } bytes, limit={ $limit } bytes
gpu-cache-triangulation-too-many-vertices = Utatuzi '{ $name }' una vipeo { $count } (> u32::MAX); haiwezekani kugawanya kwa GPU
gpu-cache-triangulation-uploaded = Utatuzi '{ $name }' umepakiwa katika vipande { $chunks } vya anga (nyuso { $faces })
i18n-active-language = Lugha hai ni { $language } (imejumuishwa: { $bundled })
i18n-could-not-select-language-error = Imeshindwa kuchagua lugha: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = Kibadilishaji cha GPU: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = Kiendeshi cha GPU: { $driver } { $driver_info }
init-gpu-limits-max-buffer-size = Vikomo vya GPU: max_buffer_size={ $max_buffer_size } MiB, max_storage_buffer_binding_size={ $max_storage_buffer_binding_size } MiB, max_storage_buffers_per_shader_stage={ $max_storage_buffers_per_shader_stage }, max_uniform_buffer_binding_size={ $max_uniform_buffer_binding_size } KiB, max_texture_dimension_2d={ $max_texture_dimension_2d }, max_bind_groups={ $max_bind_groups }
init-gpu-supports-maximum-buffer-size = GPU inatumia ukubwa wa juu wa hifadhi ya { $size } MiB; maeneo makubwa yanaweza yasionekane kikamilifu
init-surface-present-mode = Hali ya uwasilishaji wa uso: { $mode }
init-wgpu-error-continuing-error = Hitilafu ya wgpu (inaendelea): { $error }
input-could-not-read-name-error = imeshindwa kusoma { $name }: { $error }
input-could-not-slice-name-error = imeshindwa kukata { $name }: { $error }
io-add-collar-file-explicit-segments = Ongeza faili ya kola (au faili ya sehemu dhahiri): jiofizikia ya ndani ya shimo huambatishwa kwenye mashimo inayoyaainisha.

## Io strings

io-ascii-points-xyz-pts = Vidokezo vya ASCII (.xyz, .pts)
io-attribute = Sifa
io-blank-header = (kichwa tupu)
io-block-model = Mfano wa vitalu:
io-choose-file-purpose-map-its = Chagua kusudi la faili ili kuoanisha safu wima zake.
io-choose-loaded-block-model = Chagua mfano wa vitalu uliopakiwa
io-choose-loaded-dataset = Chagua seti ya data iliyopakiwa
io-choose-loaded-layer = Chagua tabaka lililopakiwa
io-choose-loaded-triangulation = Chagua utatuzi uliopakiwa
io-choose-purpose = Chagua kusudi…
io-choose-source-file-files-import = Chagua faili au faili za chanzo za kuingiza.
io-collar = Kola
io-column-mapping = Uoanishaji wa safu wima
io-comma-separated-values-csv = Comma-Separated Values (.csv)
io-csv-files = Faili za CSV
io-dataset = Seti ya data:
io-default = Chaguo-msingi
io-density-read-g-cc-exported = Msongamano, unasomwa kama g/cc, kama ulivyohamishwa. Mkunjo ambao wastani wake hauko kati ya 0.5 na 5 g/cc huachwa kwenye uingizaji kwa onyo, kwa sababu kipimo chake kinaonekana si sahihi.
io-depth = Kina
io-diameter = Kipenyo
io-downhole-geophysics = Jiofizikia ya ndani ya shimo
io-drawing-exchange-format-dxf = Drawing Exchange Format (.dxf)
io-drill-holes = Mashimo ya uchimbaji
io-east-x = Mashariki / X
io-elevation-z = Kimo / Z
io-end-x = Mwisho X
io-end-y = Mwisho Y
io-end-z = Mwisho Z
io-explicit-segments = Sehemu dhahiri
io-export = Hamisha
io-export-csv-block-model = Hamisha CSV Mfano wa Vitalu
io-export-csv-drillholes = Hamisha CSV ya Mashimo ya Uchimbaji
io-export-dxf = Hamisha DXF
io-export-one-layer = Hamisha tabaka moja
io-export-open-mining-format-2 = Hamisha Open Mining Format 2
io-export-ply = Hamisha PLY
io-export-stl = Hamisha STL
io-export-wavefront-obj = Hamisha Wavefront OBJ
io-gamma-api = Gamma (API)
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Ingiza
io-import-ascii-point-cloud = Ingiza Wingu la Vidokezo la ASCII
io-import-drillhole-csv-bundle = Ingiza Kifurushi cha CSV cha Mashimo ya Uchimbaji
io-import-geotiff = Ingiza GeoTIFF
io-import-las-laz-point-cloud = Ingiza Wingu la Vidokezo la LAS/LAZ
io-import-open-mining-format-2 = Ingiza Open Mining Format 2
io-import-pcd-point-cloud = Ingiza Wingu la Vidokezo la PCD
io-import-ply = Ingiza PLY
io-import-stl = Ingiza STL
io-import-wavefront-obj = Ingiza Wavefront OBJ
io-inclination = Mwinuko
io-interval = Kipindi
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-long-spaced-density-g-cc = Msongamano wa nafasi ndefu (g/cc)
io-mapped-csv-bundle-csv = Kifurushi cha CSV kilichooanishwa (.csv)
io-measured-depth-down-hole-read = Kina kilichopimwa kuelekea chini ya shimo, kinasomwa kama mita. Incline haibadilishi vipimo: hifadhidata iliyohamisha faili huvipanga.
io-model-file = Faili ya mfano
io-name-count-files = { $name } + faili { $count }
io-natural-gamma-read-api-units = Gamma asilia, inasomwa kama vizio vya API, kama ilivyohamishwa.
io-no-csv-chosen = Hakuna .csv iliyochaguliwa
io-no-csv-files-chosen = Hakuna faili za CSV zilizochaguliwa
io-no-dxf-chosen = Hakuna .dxf iliyochaguliwa
io-no-omf-chosen = Hakuna .omf iliyochaguliwa
io-north-y = Kaskazini / Y
io-open-mining-format-2-omf = Open Mining Format 2 (.omf)
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-projects = Miradi
io-short-spaced-density-g-cc = Msongamano wa nafasi fupi (g/cc)
io-source-file = Faili ya chanzo
io-start-x = Mwanzo X
io-start-y = Mwanzo Y
io-start-z = Mwanzo Z
io-stl = STL (.stl)
io-triangulation = Utatuzi:
io-unmapped = Haijaoanishwa
io-wavefront-obj = Wavefront OBJ (.obj)
io-writes-three-files-beside-name = Huandika faili tatu kando ya jina unalochagua: kola, uchunguzi na vipindi, katika safu wima ambazo dirisha hili huingiza.

## Jobs strings

jobs-background-task-poll-label-ended = Kazi ya nyuma '{ $poll_label }' imeisha bila matokeo
jobs-cancelled-label-its-project-no = '{ $label }' imeghairiwa: mradi wake si hai tena
jobs-discarded-stale-result = Matokeo ya nyuma yaliyopitwa na wakati ya '{ $poll_label }' yameondolewa kwa sababu chanzo kimebadilika au kimefungwa
jobs-drillhole-import = uingizaji wa mashimo ya uchimbaji
log-traces-auto-from-hole = Otomatiki, kutoka shimo hili
log-traces-curve-no-reading = { $curve }: hakuna usomaji
log-traces-curve-value-unit = { $curve }: { $value } { $unit }
log-traces-custom-range = Wigo maalum
log-traces-default-colour = Rangi ya chaguo-msingi
log-traces-density-scale = Kipimo cha msongamano
log-traces-depth-m = { $depth } m
log-traces-gamma = Gamma
log-traces-gamma-colour = Rangi ya gamma
log-traces-gamma-scale = Kipimo cha gamma
log-traces-percentile-range-no-data = Asilimia ya 1 hadi 99 ya shimo, imekadirwa kwenda nje. Shimo hili halina data kwa hilo bado.
log-traces-percentile-range = Asilimia ya 1 hadi 99 ya shimo, imekadirwa kwenda nje: { $range }.
log-traces-long-density = Msongamano mrefu
log-traces-long-density-colour = Rangi ya msongamano mrefu
log-traces-min-max-unit = { $min } hadi { $max } { $unit }
log-traces-reading = Inasoma...
log-traces-short-density = Msongamano mfupi
log-traces-short-density-colour = Rangi ya msongamano mfupi

## Logging strings

logging-activity-completed = Shughuli imekamilika
logging-activity-started = Shughuli imeanza
logging-application-id-id = Kitambulisho cha programu: { $id }
logging-application-name = Jina la programu: { $name }
logging-application-startup = Kuanzisha Programu
logging-build-target-os-architecture = Lengo la ujenzi: { $os }-{ $architecture }
logging-completed = Imekamilika
logging-count-messages = Ujumbe { $count }
logging-desktop-session-xdg-session-type = Kipindi cha eneo-kazi: XDG_SESSION_TYPE={ $session }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Inaanzisha Incline Design
logging-locale-environment = Mazingira ya lugha: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session = Kipindi cha macOS: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = Mfumo wa uendeshaji: GNU / Linux
logging-operating-system-macos = Mfumo wa uendeshaji: macOS
logging-operating-system-microsoft-windows = Mfumo wa uendeshaji: Microsoft Windows
logging-pointer-width = Upana wa kielekezi: { $width }-bit
logging-process-id-id = Kitambulisho cha mchakato: { $id }
logging-release-version = Toleo la matoleo: { $version }
logging-renderer = Kichoraji
logging-rust-compiler-host = Mwenyeji wa kikusanyaji cha Rust: { $host }
logging-system = Mfumo
logging-system-error = Hitilafu ya Mfumo
logging-unknown = haijulikani
logging-windows-session-sessionname-session = Kipindi cha Windows: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = Inafanya kazi…

## Mac strings

mac-cannot-install-macos-menu-bar = Haiwezekani kusakinisha pau ya menyu ya macOS mbali na uzi mkuu
mac-quit-app = Toka { $app }

## Main strings

main-incline-design-web-startup-failed = Uanzishaji wa Incline Design Web umeshindwa: { $error }

## Menu strings

menu-count-files-selected = faili { $count } zimechaguliwa

## Object strings

object-edit-appearance = Muonekano
object-edit-arc-circle = Mviringo na Duara
object-edit-arc-segments = Sehemu za mviringo
object-edit-bulge = Uvimbe
object-edit-bulge-arcs-horizontal-data-model = Mviringo wa uvimbe ni mlalo kwa muundo wa data: mviringo hugeuka kwenye ramani-ardhi wakati kimo kinaenda moja kwa moja kutoka kipeo kimoja hadi kingine.
object-edit-centre-x = Kitovu X
object-edit-centre-y = Kitovu Y
object-edit-centre-z = Kitovu Z
object-edit-chord = Kigunda
object-edit-colour-layer = Rangi kwa tabaka
object-edit-enter-number = Weka nambari
object-edit-follow-owning-layer-s-colour = Fuata rangi ya tabaka linalomiliki badala ya rangi iliyobandikwa kwenye kitu hiki.
object-edit-id = Kitambulisho
object-edit-identity = Utambulisho
object-edit-insert-after = Ingiza baada
object-edit-join-last-vertex-back-first = Unganisha kipeo cha mwisho kurudi kwa cha kwanza.
object-edit-length = Urefu m{ $length }
object-edit-move-down = Hamisha chini
object-edit-move-up = Hamisha juu
object-edit-object-has-no-arc-segments = Kitu hiki hakina sehemu za mviringo.
object-edit-object-has-single-position = Kitu hiki kina nafasi moja.
object-edit-object-needs-least-required-vertices = Kitu hiki kinahitaji angalau vipeo { $required }
object-edit-one-more-properties-not-valid = Sifa moja au zaidi si nambari sahihi
object-edit-perimeter-area = Mzingo m{ $length }, eneo m²{ $area }
object-edit-reverse = Geuza
object-edit-row-invalid-number = Safu { $row }: nafasi au uvimbe si nambari sahihi
object-edit-sweep = Mfagio
object-edit-text-not-number = "{ $text }" si nambari
object-edit-vertices = Vipeo

## Omf strings

omf-element-name-has-count-tie = Kipengele '{ $name }' kina viunganishi { $count } vinavyotaja mashimo ambayo hakina tena
omf-element-name-has-count-unreadable = Kipengele '{ $name }' kina sehemu { $count } za kazi zisizosomeka; ziliachwa
omf-element-unsupported-section = Kipengele '{ $name }' kinataja sehemu '{ $section }' ambayo haiwezi kuonyesha aina hii ya kipengele katika toleo hili
omf-element-name-names-unknown-section = Kipengele '{ $name }' kinataja sehemu isiyojulikana '{ $section }'
omf-ignoring-colour-map-omf-attribute = Inapuuza ramani ya rangi kwenye sifa ya OMF '{ $attribute }': { $error }
omf-mining-data-exported-incline = Data ya madini iliyohamishwa na Incline
omf-import = Uingizaji wa OMF
omf-texture = Umbile la OMF
omf-validation-warnings = Maonyo ya uthibitishaji wa OMF: { $warnings }
omf-application-metadata-dropped = Metadata ya programu ya mradi '{ $application }' haihifadhiwi
omf-project-author-not-retained = Mwandishi wa mradi hahifadhiwi
omf-project-description-not-retained = Maelezo ya mradi hayahifadhiwi
omf-unsupported-metadata-keys = Mradi una funguo za metadata zisizotumika: { $keys }
omf-skipped-drillhole-data-saved-older = Data ya mashimo ya uchimbaji iliyohifadhiwa katika mpangilio wa zamani imerukwa ({ $names }); iingize tena kutoka faili zake za chanzo

## Plot strings

plot-1-1000-one-millimetre-sheet = Kwa 1:1000, milimita moja kwenye karatasi ni mita moja ardhini.
plot-1-scale-covers-width-height = 1:{ $scale } · inafunika { $width } × { $height } m
plot-all-visible-data = Data zote zinazoonekana
plot-automatic-grid-interval = Muda wa gridi kiotomatiki
plot-border = Mpaka
plot-centre = Katikati kwenye
plot-fit-scale-help = Chagua kipimo kidogo kinachokubalika kinachotosheleza kila kitu kinachoonekana kwenye karatasi.
plot-coordinate-grid = Gridi ya kuratibu
plot-current-view-centre = Kitovu cha mwonekano wa sasa
plot-date-caps = TAREHE
plot-date = Tarehe
plot-dots-per-inch-paper-size = Nukta kwa inchi. Ukubwa huu wa karatasi unaweza kubadilishwa kuwa rasta hadi dpi { $max_dpi }; dpi 300 ni ubora wa kawaida wa uchapishaji.
plot-dpi = dpi
plot-drawing-no = NAMBARI YA MCHORO
plot-drawing-number = Nambari ya mchoro
plot-drawn-by-caps = IMECHORWA NA
plot-drawn-by = Imechorwa na
plot-e-g-example-gold-project = mfano: Mradi wa Mfano wa Dhahabu
plot-entered-coordinates = Kuratibu zilizowekwa
plot-export-png = Hamisha PNG...
plot-fit-scale-visible-data = Oanisha kipimo na data inayoonekana
plot-grid-interval = Muda wa gridi
plot-landscape = Mlalo
plot-lists-visible-surfaces-design-layers = Huorodhesha nyuso zinazoonekana na tabaka za muundo pamoja na rangi zake.
plot-margin = Ukingo
plot-margins-leave-no-room-map = Kingo hazibaki nafasi kwa ramani
plot-metres-scale-1-scale = mita    Kipimo 1:{ $scale }
plot-mm = mm
plot-north-arrow = Mshale wa kaskazini
plot-nothing-visible-draw = Hakuna kinachoonekana cha kuchora
plot-paper = Karatasi
plot-paper-orientation-width-height-mm = { $paper } { $orientation } · { $width } × { $height } mm
plot-paper-size = Ukubwa wa karatasi
plot-pick-interval-reads-roughly-every = Chagua muda unaosomeka takriban kila milimita 50 kwenye karatasi iliyochapishwa.
plot-plan = Mpango
plot-scale-must-be-positive = Kipimo cha mchoro lazima kiwe nambari chanya
plot-png-written-sheet-s-exact = PNG imeandikwa kwa ukubwa halisi wa karatasi ya sheet na kurekodi dpi yake, hivyo huchapisha kwa kipimo halisi.
plot-portrait = Wima
plot-resolution = Ubora
plot-rev = MAREKEBISHO
plot-revision = Marekebisho
plot-scale = KIPIMO
plot-scale-ratio = Kipimo  1:
plot-scale-framing = Kipimo na uwekaji fremu
plot-sheet-furniture = Mapambo ya karatasi
plot-size-width-height-mm = { $size } ({ $width } × { $height } mm)
plot-subtitle = Kichwa kidogo
plot-title = Kichwa
plot-title-block = Kizuizi cha kichwa
plot-today = leo
point-cloud-classify = Ainisha
point-cloud-classify-vegetation = Ainisha uoto
point-cloud-cloth-resolution = Azimio la kitambaa
point-cloud-cloth-resolution-about-one-half = Azimio la kitambaa la takriban mara moja na nusu ya nafasi ya vidokezo vya wingu lenye mtawanyiko mdogo zaidi kati ya yaliyochaguliwa, ili kila chembe iwe na marejeo chini yake.
point-cloud-combine-selected-point-clouds-into = Unganisha mawingu ya vidokezo yaliyochaguliwa kuwa wingu moja jipya, ili utatuzi mmoja uweze kujengwa katika yote. Rangi za kila kidokezo huhifadhiwa; wingu lisilo nazo huchangia rangi yake ya onyesho.
point-cloud-selected-count = { $count } vimechaguliwa · vidokezo { $points }
point-cloud-delete-selected-clouds-from-project = Futa mawingu yaliyochaguliwa kutoka kwenye mradi baada ya kuunganisha kukamilika, ili kuachilia kumbukumbu ambayo nakala yao ya ziada ingeshikilia.
point-cloud-flat-pads-structures = Tambarare (majukwaa, miundo)
point-cloud-ground-cloud-covers-steep-follows = Ardhi ambayo wingu inafunika. Mwinuko hufuata kuta kutoka kwenye vilele vyake; Tambarare hutumia kitambaa kigumu zaidi kinachovuka majengo makubwa na mitambo lakini hulainisha mivunjiko mikali.
point-cloud-ground-threshold = Kizingiti cha ardhi
point-cloud-how-far-around-each-point = Umbali unaozunguka kila kidokezo wa kuhesabu majirani.
point-cloud-join = Unganisha
point-cloud-let-cloth-follow-walls-down = Acha kitambaa kifuate kuta kutoka kwenye vilele vyake, ambapo ugumu wake ungekishikilia mbali na uso. Zima tu kwenye ardhi laini iliyojaa mitambo.
point-cloud-mark-each-point-ground-noise = Weka alama kila kidokezo kama ardhi, kelele au kisichoainishwa. Kitambaa hubanwa juu chini ya wingu na hutulia kwenye uso wa ardhi; vidokezo vilivyo ndani ya kizingiti cha ardhi kutoka kwake ni ardhi. Madaraja yoyote yaliyopo hubadilishwa; kutendua huyarejesha.
point-cloud-mark-isolated-returns-birds-dust = Weka alama marejeo yaliyotengwa - ndege, vumbi, makosa ya njia nyingi - kama kelele kabla ya ardhi kupatikana, ili kidokezo cha chini kilichopotea kisiweze kuvuta kitambaa chini.
point-cloud-mark-noise = Weka alama kelele
point-cloud-minimum-neighbours = Majirani wa chini
point-cloud-name-assigned-joined-point-cloud = Jina litakalopewa wingu la vidokezo lililounganishwa.
point-cloud-name-count-points = { $name } (vidokezo { $count })
point-cloud-noise-radius = Radiasi ya kelele
point-cloud-point-clouds = Mawingu ya vidokezo
point-cloud-points-closer-than-settled-cloth = Vidokezo vilivyo karibu zaidi ya hii na kitambaa kilichotulia, vinavyopimwa kwenye uso wake, ni ardhi.
point-cloud-points-fewer-neighbours-than-within = Vidokezo vyenye majirani wachache kuliko idadi hii ndani ya radiasi ya kelele ni kelele.
point-cloud-raise-cloth-resolution-if-your = Ongeza azimio la kitambaa ikiwa mashine yako ina RAM kidogo.
point-cloud-recommended = Inapendekezwa
point-cloud-recover-steep-slopes = Rejesha miteremko mikali
point-cloud-relief-dumps-rolling-ground = Mwinuko (jalala, ardhi ya vilima)
point-cloud-remove-sources = Ondoa vyanzo
point-cloud-resolution-m-points-spacing-m = { $resolution } m (vidokezo ~{ $spacing } m kati yake)
point-cloud-selected-clouds-copied-into-joined = Mawingu yaliyochaguliwa, yaliyonakiliwa kwenye wingu lililounganishwa. Funga dirisha ili kuunganisha seti tofauti.
point-cloud-selected-clouds-each-classified-its = Mawingu yaliyochaguliwa, kila moja linaainishwa lenyewe. Funga dirisha ili kuainisha seti tofauti.
point-cloud-classify-help = Panga marejeo kwa kiainishi kilichofunzwa kinachosoma umbo la vidokezo vinavyozunguka kila kimoja: ardhi, uoto - kwa vikundi vya chini (chini ya m 1), wastani (chini ya m 3) au juu kwa urefu - na vingine vyote, kama majengo na mitambo, vinaachwa bila kuainishwa. Zima hii ili kutumia kitambaa pekee.
point-cloud-spacing-cloth-s-particles-around = Nafasi ya chembe za kitambaa. Karibu na nafasi ya vidokezo vya wingu ni mwanzo mzuri; ndogo zaidi hufuata ardhi kwa karibu zaidi lakini inahitaji vidokezo vingi zaidi.
point-cloud-steep-pit-walls-benches = Mwinuko (kuta za shimo, ngazi)
point-cloud-terrain = Ardhi
point-cloud-use = Tumia

## Products strings

products-add-initiation = Ongeza Uanzishaji
products-delay = Muda wa kuchelewesha
products-delay-palette = Pala ya Muda wa Kuchelewesha
products-how-long-after-shot-fired = Muda gani baada ya ulipuaji kufyatuliwa kola hii huanzisha raundi.
products-initiation-name = Uanzishaji · { $name }
products-milliseconds-between-one-hole-firing = Milisekunde kati ya ulipuaji wa shimo moja na lililofuata.
products-ms = ms
products-no-products = Hakuna bidhaa
products-remove = Ondoa
products-update = Sasisha

## Progress strings

progress-percent-done-total = { $percent } ({ $done } kati ya { $total })
progress-task-finished = { $task }: Imekamilika

## Project strings

project-item = Kipengele

## Properties strings

properties-adds-view-dependent-rim-highlight = Huongeza mwangaza wa ukingo unaotegemea mwonekano kwenye mipaka ya vitalu na nyenzo. Kuacha hii ikizimwa hupunguza kidogo kazi ya kuchora ujazo.
properties-block-model-downscale = Upunguzaji wa mfano wa vitalu
properties-camera = Kamera
properties-camera-clip-planes = Ndege za kukata za kamera
properties-cap-while-resizing = Punguza wakati wa kubadilisha ukubwa
properties-colours-each-point-cloud-chunk = Hupaka rangi kila kipande cha wingu la vidokezo, huchora mstari kuzunguka kisanduku kinachokatwa kwa frustum, na huonyesha vidokezo vilivyochorwa fremu ya mwisho dhidi ya lengo la kiwango cha undani na jumla inayoonekana kwenye pau ya hali.
properties-colours-each-surface-chunk-outlines = Hupaka rangi kila kipande cha uso, huchora mstari kuzunguka kisanduku kinachokatwa kwa frustum, na huonyesha nyuso zilizochorwa fremu ya mwisho dhidi ya jumla inayoonekana kwenye pau ya hali.
properties-dark-mode = Hali ya giza
properties-dataset = Seti ya data
properties-developer = Msanidi
properties-downscale-rasters = Punguza ukubwa wa rasta
properties-drillholes = Mashimo ya Uchimbaji
properties-edit-object = Hariri Kitu...
properties-field-view = Uwanja wa mtazamo
properties-fps = FPS
properties-frame-counter = Kihesabu fremu
properties-frame-rate-cap = Kikomo cha kasi ya fremu
properties-hz = Hz
properties-interface = Kiolesura
properties-invert-horizontal = Geuza mlalo
properties-invert-vertical = Geuza wima
properties-limits-newly-loaded-geotiff-previews = Hupunguza hakikisho za GeoTIFF mpya zilizopakiwa hadi pikseli 4096 kwenye upande wake mrefu zaidi. Zima ili kutumia ubora kamili hadi kikomo cha umbile la GPU, ambayo hutumia kumbukumbu zaidi.
properties-line-colour = Rangi ya mstari
properties-look-sensitivity = Uelekevu wa mtazamo
properties-max-clip-span = Wigo wa juu wa kukata
properties-move-layer = Hamishia kwenye Tabaka...
properties-near-clip-limit = Kikomo cha kukata karibu
properties-no-drillhole-datasets-open = Hakuna seti za data za mashimo ya uchimbaji zilizofunguliwa.
properties-orbit-sensitivity = Uelekevu wa mzunguko
properties-panel-chrome = Mapambo ya paneli
properties-performance = Utendaji
properties-plan-mode = Hali ya Mpango
properties-point-cloud-chunk-debug-view = Mwonekano wa utatuzi wa vipande vya wingu la vidokezo
properties-presents-step-display-no-tearing = Huonyesha kwa mpangilio wa skrini: hakuna uraruaji, na skrini huweka kasi ya fremu. Ikizimwa, fremu huonyeshwa mara zinapochorwa na kikomo hapa chini kinatumika.
properties-reflective-block-edges = Kingo za vitalu zenye mng'ao
properties-restore-defaults = Rejesha Chaguo-msingi
properties-show-console = Onyesha dashibodi
properties-shows-live-near-far-projection = Huonyesha umbali wa makadirio wa karibu na mbali wa moja kwa moja kwenye pau ya hali.
properties-snap-polling = Ukaguzi wa kubandika
properties-surface-chunk-debug-view = Mwonekano wa utatuzi wa vipande vya uso
properties-vertical-sync = Usawazishaji wima
properties-world-axis-gizmo = Kifaa cha mhimili wa dunia
properties-zoom-cursor = Kuza kwa kishale
properties-zoom-sensitivity = Uelekevu wa kukuza
reference-points-count-holes-from-dataset = mashimo { $count } kutoka '{ $dataset }'
reference-points-holes-from-datasets = mashimo { $count } kutoka seti { $datasets } za data
reference-points-holes = Mashimo
reference-points-holes-points-placed-selected-when = Mashimo ambayo vidokezo vimewekwa juu yake, kama yalivyochaguliwa dirisha lilipofunguliwa. Funga dirisha ili kuchagua tofauti.
reference-points-make = Tengeneza
reference-points-no-categorical-field = Hakuna sehemu ya kategoria
reference-points-no-values = Hakuna thamani
reference-points-one-point-per-hole-boundary = Kidokezo kimoja kwa kila shimo kwenye mpaka huo, kama tabaka jipya. Shimo lenye sehemu hiyo mara mbili hutoa la juu zaidi na huwekewa alama.
reference-points-reference-points = Vidokezo vya Marejeo
reference-points-side = Upande
reference-points-working-section = Sehemu ya kazi
reference-points-working-section-field = Sehemu ya sehemu ya kazi
reference-surface-controls = Vidhibiti
reference-surface-extent = Wigo
reference-surface-points-outside-extent-still-shape = Vidokezo vilivyo nje ya wigo bado huumba uso; uso pekee ndio hukatwa kwa wigo.
reference-surface-points-surface-built-from-selected = Vidokezo ambavyo uso unajengwa kutoka kwavyo, kama vilivyochaguliwa dirisha lilipofunguliwa. Funga dirisha ili kuchagua tofauti.
reference-surface-extent-help = Mfuatano uliofungwa uliochaguliwa ambao uso uliokamilika hukatwa kwake; vidokezo vilivyo nje yake bado huumba uso.
reference-surface-selected-open-strings-surface-made = Mifuatano iliyo wazi iliyochaguliwa ambayo uso unafanywa kupita ndani yake, kama ilivyochaguliwa dirisha lilipofunguliwa. Funga dirisha ili kuchagua tofauti.
reference-surface-triangulates-selected-points-plan-in = Hutatua vidokezo vilivyochaguliwa kwenye mpango kuwa uso mpya. Kila ujenzi huongeza uso.

## Screenshot strings

screenshot-could-not-encode-viewport-image = Imeshindwa kusimba picha ya mwonekano: { $error }
screenshot-could-not-map-viewport-screenshot = Imeshindwa kuoanisha picha ya skrini ya mwonekano: { $error }
screenshot-could-not-save-viewport-image = Imeshindwa kuhifadhi picha ya mwonekano { $path }: { $error }
screenshot-downloaded-viewport-image-file-name = Picha ya mwonekano imepakuliwa: { $file_name }
screenshot-saved-viewport-image-path = Picha ya mwonekano imehifadhiwa: { $path }
screenshot-viewport-image-download-failed-error = Upakuaji wa picha ya mwonekano umeshindwa: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = Kielezo cha uso { $index } cha BVH kiko nje ya wigo kwa mfumo; kinabadilishwa na pembetatu iliyoharibika

## State strings

state-above = kwenye au juu ya
state-activate-project = Amilisha Mradi
state-all-open-incline-design-data = Data yote iliyofunguliwa ya Incline Design
state-apply-generated-rings = Tumia miduara iliyozalishwa
state-apply-selection = Tumia kwa uteuzi
state-rotate-by-azimuth-dip = kwa azimuth { $azimuth }°, mteremko { $dip }°
state-rotate-to-azimuth-dip = hadi azimuth { $azimuth }°, mteremko { $dip }°
state-below = kwenye au chini ya
state-build-reference-points = Jenga Vidokezo vya Marejeo
state-centre-rotation = Kitovu cha Mzunguko
state-checking-unsaved-work = Inakagua kazi isiyohifadhiwa
state-choose-destination = Chagua marudio
state-choose-one-more-files = Chagua faili moja au zaidi
state-clear-raster = Futa Rasta
state-click-pit-shell-viewport = Bofya ganda la shimo kwenye mwonekano.
state-click-pit-stockpile-solid-viewport = Bofya kigumu cha shimo au rundo la hifadhi kwenye mwonekano.
state-click-surface-viewport = Bofya uso kwenye mwonekano.
state-click-topology-viewport = Bofya tofolojia kwenye mwonekano.
state-close-project = Funga Mradi
state-colour-drillholes = Weka Rangi za Mashimo ya Uchimbaji
state-colour-drillholes-working-section = Paka Rangi Mashimo ya Uchimbaji kwa Sehemu ya Kazi
state-colour-points-classification = Paka Rangi Vidokezo kwa Uainishaji
state-copy-objects-layer = Nakili Vitu kwenye Tabaka
state-count-cloud-s = mawingu { $count }
state-count-file-s = faili { $count }
state-count-object-s-axis-value = vitu { $count } · { $axis } { $value }
state-count-object-s-closed = vitu { $count } · { $closed }
state-count-object-s-layer = vitu { $count } · { $layer }
state-count-object-s-weight = vitu { $count } · { $weight }
state-count-object-s-z-elevation = vitu { $count } · Z { $elevation }
state-points-controls-clipped = vidokezo { $count } · mifuatano { $controls } ya udhibiti · imekatwa kwa mfuatano wa wigo
state-points-controls-unclipped = vidokezo { $count } · mifuatano { $controls } ya udhibiti · haijakatwa
state-create-collection = Unda Mkusanyiko
state-create-point-cloud-tin = Unda TIN ya Wingu la Vidokezo
state-create-project = Unda Mradi
state-current-project = Mradi wa sasa
state-cut-topology-pit-shell = Kata Tofolojia kwa Ganda la Shimo
state-cut-triangulation-polyline = Kata Utatuzi kwa Mstari wa Pointi Nyingi
state-cut-triangulation-z = Kata Utatuzi kwa Z
state-dark-mode = Hali ya Giza
state-data-ticked-export-checklist = Data iliyotiwa alama kwenye orodha ya uhamishaji
state-detached = Imetenganishwa
state-disabled = Imezimwa
state-discard-project-changes = Ondoa Mabadiliko ya Mradi
state-discard-replace-project = Ondoa na Badilisha Mradi
state-discarding-unsaved-changes = Inaondoa mabadiliko yasiyohifadhiwa
state-docked = Imefungiwa
state-drape-raster = Funika Rasta
state-drill-pattern = Mfumo wa Uchimbaji
state-duplicate-layer = Nakili Tabaka
state-east = Mashariki
state-enabled = Imewashwa
state-exit-incline-design = Toka Incline Design
state-export-block-model-csv = Hamisha CSV ya Mfano wa Vitalu
state-export-drillhole-csv = Hamisha CSV ya Mashimo ya Uchimbaji
state-export-layer-dxf = Hamisha Tabaka kwenda DXF
state-export-omf = Hamisha OMF
state-export-project-dxf = Hamisha Mradi kwenda DXF
state-export-triangulation = Hamisha Utatuzi
state-export-viewport-image = Hamisha Picha ya Mwonekano
state-finish-closed-polyline = Maliza mstari wa pointi nyingi uliofungwa
state-finish-open-polyline = Maliza mstari wa pointi nyingi ulio wazi
state-fit-extents = Oanisha na wigo
state-fix-release-centre-both-views = Funga au achilia kitovu ambacho mionekano yote miwili inazunguka
state-folder-section = { $folder } katika { $section }
state-generate-contours = Zalisha Mistari ya Mwinuko
state-hidden = Imefichwa
state-import-drillholes = Ingiza Mashimo ya Uchimbaji
state-import-omf = Ingiza OMF
state-import-point-cloud = Ingiza Wingu la Vidokezo
state-import-raster = Ingiza Rasta
state-import-triangulation = Ingiza Utatuzi
state-insert-intersection-points = Ingiza Vidokezo vya Makutano
state-insert-points-elevation = Ingiza Vidokezo kwenye Kimo
state-keep-inside = Weka ndani
state-keep-outside = Weka nje
state-kriged-block-model = Mfano wa Vitalu wa Kriging
state-load-block-model = Pakia Mfano wa Vitalu
state-load-drillholes = Pakia Mashimo ya Uchimbaji
state-load-layer = Pakia Tabaka
state-load-point-cloud = Pakia Wingu la Vidokezo
state-load-raster = Pakia Rasta
state-load-triangulation = Pakia Utatuzi
state-locked-count-object-s = Vitu { $count } vimefungwa
state-major-minor = Kuu { $major } · ndogo { $minor }
state-member-into-folder-section = { $member } kwenda { $folder } katika { $section }
state-member-root-section = { $member } hadi mzizi wa { $section }
state-move-axis-value = Hamishia kwa Thamani ya Mhimili
state-move-objects-layer = Hamisha Vitu kwenye Tabaka
state-name-count-cloud-s = { $name } · mawingu { $count }
state-name-count-holes = { $name } · mashimo { $count }
state-name-count-object-s = { $name } · vitu { $count }
state-name-z-min-z-max = { $name } · { $z_min } hadi { $z_max }
state-new-collection-under-section = Mkusanyiko mpya chini ya { $section }
state-next-edit = Hariri inayofuata
state-north = Kaskazini
state-off = Imezimwa
state-on = Imewashwa
state-open-containing-folder = Fungua folda inayohusika
state-open-project = Fungua Mradi
state-preserve-view-angle = Hifadhi pembe ya mwonekano
state-previous-edit = Hariri iliyotangulia
state-project-id = Mradi { $id }
state-remove-block-model = Ondoa Mfano wa Vitalu
state-remove-drillholes = Ondoa Mashimo ya Uchimbaji
state-remove-point-cloud = Ondoa Wingu la Vidokezo
state-remove-raster = Ondoa Rasta
state-remove-triangulation = Ondoa Utatuzi
state-removed-from-active-triangulation = Imeondolewa kwenye utatuzi hai
state-removed-from-every-triangulation = Imeondolewa kwenye utatuzi wote
state-rename-kind = Badilisha Jina la { $kind }
state-save-close-project = Hifadhi na Funga Mradi
state-save-despite-unsupported-content = Hifadhi licha ya maudhui yasiyotumika
state-save-project = Hifadhi Mradi Kama
state-save-replace-project = Hifadhi na Badilisha Mradi
state-saving-current-project = Inahifadhi mradi wa sasa
state-section-name = sehemu { $section }
state-select-layer-objects = Chagua Vitu vya Tabaka
state-selected-objects = Vitu vilivyochaguliwa
state-selected-polylines = Mistari ya pointi nyingi iliyochaguliwa
state-selected-scene-elements = Vipengele vya eneo vilivyochaguliwa
state-set-block-model-variable = Weka Kigezo cha Mfano wa Vitalu
state-set-cinematic-view = Weka Mwonekano wa Sinema
state-set-drillhole-colour-preset = Weka Mpangilio wa Rangi za Mashimo ya Uchimbaji
state-set-drillhole-discs = Weka Diski za Mashimo ya Uchimbaji
state-set-drillhole-style = Weka Mtindo wa Mashimo ya Uchimbaji
state-set-drillhole-width = Weka Upana wa Mashimo ya Uchimbaji
state-set-entity-lock = Weka Ufungaji wa Kitu
state-set-grid = Weka Gridi
state-set-layer-lock = Weka Ufungaji wa Tabaka
state-set-line-weight = Weka Uzito wa Mstari
state-set-object-colour = Weka Rangi ya Kitu
state-set-object-fill = Weka Ujazaji wa Kitu
state-set-point-visibility = Weka Mwonekano wa Kidokezo
state-set-polyline-closed = Weka Mstari wa Pointi Nyingi Kufungwa
state-set-raster-lock = Weka Ufungaji wa Rasta
state-set-standard-view = Weka Mwonekano wa Kawaida
state-set-topology-wireframes = Weka Nyaya za Tofolojia
state-set-triangulation-colour = Weka Rangi ya Utatuzi
state-show-console = Onyesha Dashibodi
state-show-project = Onyesha Mradi
state-shown = Inaonyeshwa
state-slice-mode = Hali ya Ukataji
state-slice-preview = Hakikisho la Ukataji
state-south = Kusini
state-stem-contours = Mistari ya Mwinuko ya { $stem }
state-target-new-name = { $target } kuwa "{ $new_name }"
state-trim-above = Punguza juu
state-trim-below = Punguza chini
state-trim-triangulation-surface = Punguza Utatuzi kwa Uso
state-undrape-raster = Ondoa Ufunikaji wa Rasta
state-undrape-rasters = Ondoa Ufunikaji wa Rasta Zote
state-unload-block-model = Ondoa Mfano wa Vitalu
state-unload-drillholes = Ondoa Mashimo ya Uchimbaji
state-unload-layer = Ondoa Tabaka
state-unload-point-cloud = Ondoa Wingu la Vidokezo
state-unload-raster = Ondoa Rasta
state-unload-triangulation = Ondoa Utatuzi
state-untitled-project = Mradi usio na jina
state-use-typed-radius = Tumia radiasi iliyoandikwa
state-west = Magharibi

## Status strings

status-clip-near-far = Kata karibu/mbali/Δ: -- / -- / --
status-faces-chunks = Nyuso: -- / -- (vipande --/--)
status-frame-rate = Kasi ya fremu
status-points-chunks = Vidokezo: -- / -- kati ya -- (vipande --/--)

## Text strings

text-could-not-build-vector-mesh = Imeshindwa kujenga mfumo wa vector kwa fonti { $font }, alama { $glyph }: { $error }
text-document-text-mesh-exceeded-its = Mfumo wa maandishi ya hati umezidi wigo wake wa kielezo cha u32

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = Chagua data ya mashimo ya uchimbaji ya kuunganisha kwanza
tie-in-count-connector-s = viunganishi { $count }
tie-in-delete-tie-ins = Futa Viunganishi
tie-in-deleted-count-selected-tie-connector = Viunganishi { $count } vilivyochaguliwa vya kuunganisha vimefutwa
tie-in-hole = shimo
tie-in-initiation-point-lifted-from-name = Kidokezo cha kuanzia kimeondolewa kutoka { $name }
tie-in-initiation-point-set-name-delay = Kidokezo cha kuanzia kimewekwa kwenye { $name } kwa ms { $delay }
tie-in-select-delay-product-palette-before = Chagua bidhaa ya muda wa kuchelewesha kwenye pala kabla ya kuunganisha mashimo
tie-in-tied-connectors = Viunganishi { $count } vimeunganishwa kwa ms { $delay } na { $product }
tie-in-tied-connectors-replacing = Viunganishi { $count } vimeunganishwa kwa ms { $delay } na { $product }, vikibadilisha { $replaced }

## Toolbar strings

toolbar-fill-type = Aina ya kujaza

## Toolbars strings

toolbars-auto-bench = Ngazi-Kiotomatiki
toolbars-bezier-polyline = Mstari wa Pointi Nyingi wa Bezier
toolbars-chamfer-polyline-corners = Punguza Kingo za Kona za Mstari wa Pointi Nyingi
toolbars-create-text = Unda Maandishi
toolbars-cursor-regular = Kishale: Kawaida
toolbars-cursor-snap-line = Kishale: Bandika kwenye Mstari
toolbars-cursor-snap-point = Kishale: Bandika kwenye Kidokezo
toolbars-cursor-snap-surface = Kishale: Bandika kwenye Uso
toolbars-delete-points = Futa Vidokezo
toolbars-explode-polyline-lines = Vunja Mstari wa Pointi Nyingi kuwa Mistari
toolbars-fuse-polylines = Unganisha Mistari ya Pointi Nyingi
toolbars-measure-distance = Pima Umbali
toolbars-new-layer = Tabaka Jipya
toolbars-split-polyline-points = Gawanya Mstari wa Pointi Nyingi kwa Vidokezo
toolbars-strike-dip = Mwelekeo na Mteremko
toolbars-tool-not-available-section-view = { $tool } - hakipatikani kwenye mwonekano wa sehemu

## Tri strings

tri-sampling-method-help = Kinachojirekebisha hukusanya vipeo kwenye ardhi ngumu kupitia hitilafu ya kuoanisha ndege; sawia huvisambaza kwa usawa. Mbinu zaidi zinaweza kuongezwa baadaye.
tri-adaptive-quadtree = Kinachojirekebisha (quadtree)
tri-axis-range = Wigo wa { $axis }
tri-base-topology-will-receive-pit = Tofolojia ya msingi itakayopokea umbo la shimo au rundo la hifadhi.
tri-boundary-polyline = Mstari wa pointi nyingi wa mpaka
tri-bridge-gaps-help = Unganisha mianya na mabonde ya mpaka membamba zaidi ya hii kwenye uso. 0 bado huunganisha mianya hadi kiasi cha ukubwa wa seli ya sampuli; thamani kubwa hujaza mashimo makubwa na kuondoa mabonde ya mpaka.
tri-budget = Bajeti kwa
tri-cancel-pick = Ghairi Uchaguzi
tri-candidate-detail = Maelezo ya mgombea
tri-candidate-fine-cells-per-budgeted = Seli laini za mgombea kwa kila kipeo cha bajeti. Thamani ya juu humpa kipima kinachojirekebisha uhuru zaidi wa kuweka maelezo, lakini hujengwa polepole zaidi.
tri-cap-surface-share-source-points = Punguza uso kwa asilimia ya vidokezo vya chanzo au kwa idadi kamili ya vipeo.
tri-choose-input-clicking-loaded-surface = Chagua chanzo hiki kwa kubofya uso uliopakiwa kwenye mwonekano
tri-choose-which-side-reference-topology = Chagua upande upi wa tofolojia ya rejeleo wa kuondoa kutoka kwenye uso ndani ya eneo lao la pamoja la XY.
tri-clip = Kata
tri-clip-creates-new-triangulation-name = Ukataji huunda utatuzi mpya wenye jina hili; uso wa chanzo haubadiliki.
tri-clip-surface-polyline = Kata Uso kwa Mstari wa Pointi Nyingi
tri-closed-pit-stockpile-solid-whose = Kigumu cha shimo au rundo la hifadhi kilichofungwa ambacho mpaka wake ulio wazi utajumuishwa kwenye matokeo.
tri-cloud-carries-no-classifications-so = Wingu hili halina uainishaji, kwa hivyo kila kidokezo kinatengenezewa uso. Ingiza faili ya LAS/LAZ iliyopitia kichujio cha ardhi ili kujenga upya ardhi tupu.
tri-create-new-layer-contours-append = Unda tabaka jipya kwa mistari ya mwinuko au uiongeze kwenye tabaka lililopo kwenye mradi hai.
tri-cut-topology-pit-shell = Kata Tofolojia kwa Ganda la Shimo
tri-e-g-design-trimmed = mfano: design_trimmed
tri-e-g-mysurf-cut = mfano: mysurf_cut
tri-e-g-mysurf-slice = mfano: mysurf_slice
tri-e-g-surface-contour = mfano: surface_contour
tri-e-g-topo-cut = mfano: topo_cut
tri-e-g-topo-pit = mfano: topo_with_pit
tri-exact-number-surface-vertices-target = Idadi kamili ya vipeo vya uso vinavyolengwa. Thamani kubwa sana hujengwa polepole na kutumia kumbukumbu kubwa.
tri-existing-ground-topology-will-cut = Tofolojia iliyopo ya ardhi itakayokatwa na ganda la shimo.
tri-fill-holes-up = Jaza mashimo hadi
tri-generate = Zalisha
tri-generate-contour-lines = Zalisha Mistari ya Mwinuko
tri-generate-upper-surface = Zalisha Uso wa Juu
tri-ground-points-only = Vidokezo vya ardhi pekee
tri-hide-unload-sources = Ficha na ondoa vyanzo
tri-higher-edge-will-enforced-each = Ukingo wa juu utatumika katika kila mgongano. Sehemu za chini zenye mgongano hazitazingatiwa kama mistari ya mvunjiko na uso utaingiliana kupitia maeneo hayo. Mistari ya chanzo ya pointi nyingi haibadiliki.
tri-breaklines-cross = Kingo za mstari wa mvunjiko zilizoangaziwa zinavuka au kupishana kwenye mpango kwenye vimo tofauti. Uso mmoja wa ardhi hauwezi kufuata zote mbili.
tri-intervals-colours = Vipindi na rangi
tri-keep-clipped-topology-included-shape = Weka tofolojia iliyokatwa na umbo lililojumuishwa kama utatuzi tofauti badala ya kuviunganisha kuwa kitu kimoja.
tri-keep-inside-discards-surface-outside = Weka ndani huondoa uso ulio nje ya mstari wa pointi nyingi. Weka nje hukata shimo lenye umbo la mstari wa pointi nyingi kutoka kwenye uso.
tri-keeps-only-surface-within-polyline = Huweka uso ulio ndani ya mpaka wa mstari wa pointi nyingi pekee.
tri-keep-surface-relation-help = Huweka uso { $relation } tofolojia ndani ya wigo wake wa XY.
tri-layer-already-exists-select-above = Tabaka hilo tayari lipo; lichague hapo juu au chagua jina lingine.
tri-limit-z-range = Punguza wigo wa Z
tri-major = Kuu
tri-max-edge-length = Urefu wa juu wa ukingo
tri-merge = Unganisha
tri-method = Mbinu
tri-min = Kima cha chini
tri-minimum-maximum-elevations-retained = Vimo vya chini na vya juu vitakavyobaki kwenye uso wa matokeo. Kima cha chini lazima kiwe chini ya kima cha juu.
tri-minor = Ndogo
tri-contour-interval-help = Ndogo hudhibiti mistari ya kawaida ya mwinuko. Kuu hudhibiti mistari iliyosisitizwa na lazima itumie muda usiopungua ule wa Ndogo.
tri-move-cursor-over-loaded-surface = Sogeza kishale juu ya uso uliopakiwa.
tri-slice-output-name-help = Jina litakalopewa uso wa matokeo uliokatwa kwa kimo.
tri-name-assigned-merged-topology-pit = Jina litakalopewa matokeo ya tofolojia iliyounganishwa na shimo/rundo la hifadhi.
tri-name-assigned-newly-created-contour = Jina litakalopewa tabaka jipya la mistari ya mwinuko.
tri-reconstruct-output-name-help = Jina litakalopewa utatuzi uliojengwa upya.
tri-name-assigned-topology-after-pit = Jina litakalopewa tofolojia baada ya ganda la shimo kukatwa kutoka humo.
tri-name-assigned-trimmed-output-surface = Jina litakalopewa uso wa matokeo uliopunguzwa.
tri-nearby-breakline-vertices-do-not = Vipeo vya karibu vya mstari wa mvunjiko havikutani katika sehemu moja kamili, hivyo uso hauwezi kutatuliwa.
tri-new-layer = Tabaka jipya
tri-new-layer-name = Jina jipya la tabaka
tri-no-boundary-selected = Hakuna mpaka uliochaguliwa
tri-no-point-cloud-selected = Hakuna wingu la vidokezo lililochaguliwa
tri-no-surface-selected = Hakuna uso uliochaguliwa
tri-once-clip-succeeds-unload-source = Ukataji ukifanikiwa, ondoa uso wa chanzo ili matokeo yaliyokatwa pekee yabaki kwenye tukio.
tri-once-cut-succeeds-unload-original = Ukataji ukifanikiwa, ondoa tofolojia ya awali ili matokeo ya ukataji pekee yabaki kwenye tukio. Ganda la shimo linabaki limepakiwa.
tri-once-merge-succeeds-unload-source = Baada ya kuunganishwa kufanikiwa, ondoa tofolojia ya chanzo na kigumu ili matokeo yaliyounganishwa pekee yabaki kwenye eneo.
tri-once-slice-succeeds-unload-source = Ukataji wa kipande ukifanikiwa, ondoa uso wa chanzo ili matokeo yaliyokatwa pekee yabaki kwenye tukio.
tri-once-trim-succeeds-unload-surface = Upunguzaji ukifanikiwa, ondoa uso uliopunguzwa ili matokeo pekee yabaki kwenye tukio. Tofolojia inabaki imepakiwa.
tri-only-loaded-pickable = Utatuzi uliopakiwa tu ndio unaweza kuchaguliwa.
tri-operation = Operesheni
tri-output-layer = Tabaka la matokeo
tri-percentage = Asilimia
tri-percentage-cloud = Asilimia ya wingu
tri-pick-from-view = Chagua kutoka kwa Mwonekano
tri-pit-design-surface-only-areas = Uso wa muundo wa shimo. Maeneo tu ambapo unachimba chini ya tofolojia ndiyo yanayotumika kwa ukataji.
tri-pit-shell = Ganda la shimo
tri-pit-stockpile-solid = Kigumu cha shimo/rundo la hifadhi
tri-recommended-weld-retry = Inapendekezwa: Unga na Ujaribu Tena
tri-reconstruct-ground-only-help = Jenga upya kutoka vidokezo vilivyoainishwa kama ardhi tupu, ukiacha uoto, majengo, mitambo na kelele. Zima hii ili kutengenezea uso kila kidokezo kwenye wingu.
tri-reconstruct-help = Jenga upya uso wa ardhi uliotatuliwa kutoka kwa wingu la vidokezo. Kipima kinachojirekebisha hutumia bajeti ya vipeo mahali ardhi ilipo ngumu zaidi na kuacha maeneo tambarare na vipeo vichache.
tri-reduce-budget-candidate-detail-if = Punguza bajeti au maelezo ya mgombea ikiwa kompyuta yako ina RAM kidogo.
tri-reference-topology-help = Tofolojia ya rejeleo inayoelezea mahali uso mwingine unapopunguzwa.
tri-reject-reconstructed-triangle-edges = Kataa kingo za pembetatu zilizojengwa upya zenye urefu zaidi ya umbali huu. Tumia 0 kwa kutokuwa na kikomo cha urefu wa ukingo.
tri-remove-inside-help = Huondoa uso ulio ndani ya mpaka wa mstari wa pointi nyingi na kubaki na uliobaki.
tri-removes-topology-where-pit-shell = Huondoa tofolojia mahali ganda la shimo linapochimba chini yake ili ganda lijaze shimo hilo. Mshono hufuata mstari halisi wa 3D wa mgusano kati ya nyuso; tofolojia iliyo chini ya sehemu za ganda zinazosimama juu ya ardhi inabaki.
tri-result = Matokeo
tri-save-two-entities = Hifadhi kama vitu viwili
tri-select = Chagua…
tri-selected-closed-polyline-whose-xy = Mstari wa pointi nyingi uliofungwa uliochaguliwa, ambao mpaka wake wa XY unafafanua eneo la kukata.
tri-selected-point-cloud-whose-points = Wingu la vidokezo lililochaguliwa, ambalo vidokezo vyake vitajengwa upya kuwa uso wa ardhi. Funga dirisha ili kujenga upya tofauti.
tri-selected-surface-from-which-contour = Uso uliochaguliwa, ambao mistari ya mwinuko itazalishwa kutoka kwake. Funga dirisha ili kuzalisha kutoka tofauti.
tri-selected-surface-which-will-clipped = Uso uliochaguliwa, utakaokatwa. Funga dirisha ili kukata tofauti.
tri-slice-source-help = Uso uliochaguliwa, ambao wigo wake wa kimo utakatwa. Funga dirisha ili kukata tofauti.
tri-share-source-points-keep-fractions = Sehemu ya vidokezo vya chanzo vya kubaki. Sehemu kama 0.125% zinaruhusiwa.
tri-slice-triangulation-z-range = Kata Utatuzi kwa Wigo wa Z
tri-solution-generate-upper-surface = Suluhisho: Zalisha Uso wa Juu
tri-surface-trim = Uso wa Kupunguza
tri-target-surface-help = Uso utakaobadilishwa; tofolojia iliyochaguliwa inabaki kama ilivyo.
common-percent-suffix = %
tri-topology = Tofolojia
tri-triangulation-failed = Utatuzi Umeshindwa
tri-trim = Punguza
tri-trim-topology = Punguza kwa Tofolojia
tri-uniform-grid = Gridi sawia
tri-unload-source-surface = Ondoa uso wa chanzo
tri-unload-source-topology = Ondoa tofolojia ya chanzo
tri-up-target-point-count-points = Hadi vidokezo { $target } kati ya { $point_count } vitakuwa vipeo vya uso ({ $percent }%).
tri-use-full-surface-elevation-range = Tumia wigo kamili wa kimo cha uso
tri-vertex-count = Idadi ya vipeo
tri-vertices-within-5-cm-xy = Vipeo vilivyo ndani ya sentimita 5 kwenye XY na Z vitashiriki nafasi moja kwa utatuzi huu. Hii inaweza kuhamisha uso uliozalishwa kwa kiasi cha hadi sentimita 5 kwa eneo hilo; mistari ya chanzo ya pointi nyingi haibadiliki.
tri-weld-retry = Unga na Ujaribu Tena
tri-when-enabled-generate-contours-only = Ikiwashwa, zalisha mistari ya mwinuko kati ya vimo vya chini na vya juu vilivyoainishwa tu.

## Ui strings

ui-choose-offset-side = Chagua upande wa mkabala
ui-choose-relimit-side = Chagua upande wa kikomo kipya
ui-click-circle-centre = Bofya kitovu cha duara
ui-click-closed-polyline-use-blast = Bofya mstari wa pointi nyingi uliofungwa wa kutumia kama umbo la ulipuaji
ui-click-collar-add-edit-initiation = Bofya kola ili kuongeza au kuhariri kidokezo cha kuanzia
ui-click-first-point-slice-line = Bofya kidokezo cha kwanza cha mstari wa kukata
ui-click-first-vertex = Bofya kipeo cha kwanza
ui-click-perimeter-point-type-radius = Bofya kidokezo cha mzunguko au andika radiasi
ui-click-second-point-slice-line = Bofya kidokezo cha pili cha mstari wa kukata
ui-click-second-vertex = Bofya kipeo cha pili
ui-click-use-pointer-radius = au bofya kutumia radiasi ya kishale
ui-could-not-copy-text-browser = Imeshindwa kunakili maandishi kwenye ubao wa kunakili wa kivinjari: { $error }
ui-dip-horizontal-no-strike = { $dip } (mlalo, hakuna mwelekeo)
ui-distance-meters = mita { $distance }
ui-drag-ring-type-azimuth-dip = Buruta duara au andika azimuth na mteremko
ui-each-hole-turns-about-its = kila shimo hugeuka kwenye kola yake
ui-enter-positive-decimal-radius = Weka radiasi chanya ya desimali
ui-esc-cancels = Esc kughairi
ui-no-delay-product-tie = Hakuna bidhaa ya muda wa kuchelewesha ya kuunganisha nayo
ui-press-enter-use-typed-radius = Bonyeza Enter kutumia radiasi iliyoandikwa
ui-right-click-delay-palette-heading = bofya kulia kichwa cha Pala ya Muda wa Kuchelewesha ili kuongeza moja
ui-select-designs = Chagua miundo
ui-select-drill-hole = Chagua shimo la uchimbaji
ui-select-endpoint-join = Chagua ncha ya kuunganisha
ui-select-first-crest-toe-point = Chagua kidokezo cha kwanza cha kilele/mguu
ui-select-item = Chagua kipengele
ui-select-line-fuse = Chagua mstari wa kuunganisha
ui-select-line-polyline = Chagua mstari au mstari wa pointi nyingi
ui-select-line-relimit = Chagua mstari wa kuweka kikomo kipya
ui-select-next-line-fuse = Chagua mstari unaofuata wa kuunganisha
ui-select-opposite-berm-point = Chagua kidokezo kinyume cha lundo
ui-select-point = Chagua kidokezo
ui-select-polyline = Chagua mstari wa pointi nyingi
ui-select-polyline-open-line = Chagua mstari wa pointi nyingi au mstari wazi
ui-select-polyline-vertex = Chagua kipeo cha mstari wa pointi nyingi
ui-select-second-crest-toe-point = Chagua kidokezo cha pili cha kilele/mguu
ui-select-second-split-point = Chagua kidokezo cha pili cha kugawanya
ui-select-split-point = Chagua kidokezo cha kugawanya
ui-select-topologies = Chagua tofolojia
ui-slice-view = Mwonekano wa ukataji
ui-strike-dip = mwelekeo { $strike }° · { $dip }
ui-value-dip = mteremko { $value }°
viewport-1-1-true-shape = 1:1, umbo halisi
viewport-1-ratio = 1:{ $ratio }

## Viewport strings

viewport-all-total-categories-keep-their = Kategoria zote { $total } zinabaki na rangi yake; za kwanza { $shown } pekee ndizo zinazochorwa kwa kutofautisha
viewport-axis-maximum = kima cha juu cha { $axis }
viewport-axis-minimum = kima cha chini cha { $axis }
viewport-azimuth-dip = Azimuth { $azimuth }, mteremko { $dip }
viewport-back-whole-log = Rudi kwenye logi nzima.
viewport-bar-blast-timeline-placeholder = Ratiba ya Ulipuaji [NAFASI YA MUDA]
viewport-bar-burden-relief-heatmap-placeholder = Ramani-joto ya Uwiano wa Mzigo [NAFASI YA MUDA]
viewport-bar-cinematic-view = Mwonekano wa Sinema
viewport-bar-color = Rangi:
viewport-bar-contours-equal-time-placeholder = Mistari ya Mwinuko Sawa kwa Wakati [NAFASI YA MUDA]
viewport-bar-disable-cinematic-view = Zima Mwonekano wa Sinema
viewport-bar-disable-flying-mode = Zima Hali ya Kuruka
viewport-bar-disable-x-ray-vision = Zima Mwono wa X-Ray
viewport-bar-drill-holes = Mashimo ya Uchimbaji:
viewport-bar-enable-flying-mode = Washa Hali ya Kuruka
viewport-bar-enable-x-ray-vision = Washa Mwono wa X-Ray
viewport-bar-exit-slice-view = Toka kwenye Mwonekano wa Ukataji
viewport-bar-fill = Jaza:
viewport-bar-fix-centre-rotation = Funga Kitovu cha Mzunguko
viewport-bar-hide-borehole-inspector = Ficha Kikaguzi cha Shimo la Uchimbaji
viewport-bar-hide-classification = Ficha Uainishaji
viewport-bar-hide-points = Ficha Vidokezo
viewport-bar-hide-rl-grid = Ficha Gridi ya RL
viewport-bar-hide-wireframes = Ficha Nyaya
viewport-bar-hide-xy-grid = Ficha Gridi ya XY
viewport-bar-release-centre-rotation = Achilia Kitovu cha Mzunguko
viewport-bar-show-borehole-inspector = Onyesha Kikaguzi cha Shimo la Uchimbaji
viewport-bar-show-classification = Onyesha Uainishaji
viewport-bar-show-points = Onyesha Vidokezo
viewport-bar-show-rl-grid = Onyesha Gridi ya RL
viewport-bar-show-wireframes = Onyesha Nyaya
viewport-bar-show-xy-grid = Onyesha Gridi ya XY
viewport-bar-vertical-slice-view = Mwonekano wa Ukataji Wima
viewport-blank = (tupu)
viewport-choose-active-block-model-variable = Chagua kigezo hai cha mfano wa vitalu
viewport-choose-variable = Chagua kigezo
viewport-click-edit-color-right-click = Bofya ili kuhariri rangi; bofya kulia ili kuondoa
viewport-click-type-boundary-s-value = Bofya ili kuandika thamani ya mpaka huu
viewport-colour-mapping = Uoanishaji wa rangi
viewport-count-categories = kategoria { $count }
viewport-count-category = kategoria { $count }
viewport-depth-m-hole-end = m { $depth } mwisho wa shimo
viewport-double-click-add-boundary-here = Bofya mara mbili ili kuongeza mpaka hapa
viewport-drag-move-middle-click-toggles = Buruta kuhamisha · Bofya kati hubadilisha ≤
viewport-drag-move-right-click-remove = Buruta kuhamisha · Bofya kulia kuondoa · Bofya kati hubadilisha ≤
viewport-drag-spin-view-around-hole = Buruta ili kuzungusha mwonekano kuzunguka shimo. Bofya mara mbili ili kuelekea kaskazini.
viewport-e = E
viewport-edit-category-colour = Hariri rangi ya kategoria hii
viewport-edit-colour-used-empty-values = Hariri rangi inayotumika kwa thamani tupu
viewport-empty = (tupu)
viewport-empty-hidden = (tupu · imefichwa)
viewport-filter-variables = Chuja vigezo
viewport-fit-hole-track = Oanisha shimo na njia
viewport-from = { $from } hadi { $to }
viewport-from-m = { $from } hadi { $to } m
viewport-h-1-ratio = H 1:{ $ratio }
viewport-hole-has-no-trace-draw = Shimo hili halina njia ya kuchora.
viewport-interval-data = Data ya vipindi
viewport-intervals = Vipindi
viewport-m-from-collar-toward-bearing = m kutoka kola, kuelekea { $bearing }°
viewport-navigation-hint = Buruta kwa kitufe cha kati kusogeza · Sogeza gurudumu kukuza
viewport-navigation-hint-detach = Buruta kwa kitufe cha kati kusogeza · Sogeza gurudumu kukuza · Bofya kutenganisha
viewport-n = N
viewport-no-data-variable = Hakuna data kwa kigezo hiki
viewport-no-density-log-hole = Hakuna logi ya msongamano ya shimo hili
viewport-no-downhole-geophysics-hole = Hakuna jiofizikia ya ndani ya shimo ya shimo hili
viewport-no-gamma-log-hole = Hakuna logi ya gamma ya shimo hili
viewport-no-matches = Hakuna vinavyolingana
viewport-no-trace = Hakuna njia
viewport-no-usable-range = (hakuna wigo unaotumika)
viewport-not-logged = Haijarekodiwa
viewport-orientation-source = Chanzo cha mwelekeo
viewport-rebuild-variable-s-colours-from = Jenga upya rangi za kigezo hiki kutoka kwa data yake
viewport-reset = Rejesha
viewport-restore-full-model-range = Rejesha wigo kamili wa mfano
viewport-roll-wheel-over-log-zoom = Zungusha gurudumu juu ya logi ili kukuza mshipa. Buruta logi ili kuzungusha shimo na kutembea chini yake.
viewport-s = S
viewport-sideways-scale = Kipimo cha pembeni
viewport-squeeze-sideways-just-enough-keep = Bana pembeni kwa kiasi tu cha kuweka shimo kwenye mwonekano. Hainyooshi kamwe.
viewport-trace-extent = Wigo wa njia
viewport-w = W
viewport-widen-panel-show-density = Panua paneli ili kuonyesha msongamano
viewport-widen-panel-show-density-gamma = Panua paneli ili kuonyesha msongamano na gamma
viewport-widen-panel-show-gamma = Panua paneli ili kuonyesha gamma
charging-edit-charge-product = Hariri Bidhaa ya Chaji
charging-new-charge-product = Bidhaa Mpya ya Chaji
charging-explosive-decks-add-mass-primed-stemming = Sehemu za vilipuzi huongeza uzito na huwekewa kiwashio; sehemu za stemming na hewa huchukua urefu pekee.
charging-density = Msongamano
charging-density-hint = Msongamano ndani ya shimo. Uzito kwa mita ni huu mara eneo la sehemu mtambuka ya shimo.
charging-another-product-already-has-name = Bidhaa nyingine tayari ina jina hili
charging-edit-charge-rule = Hariri Kanuni ya Chaji
charging-new-charge-rule = Kanuni Mpya ya Chaji
charging-decks-collar-toe = Sehemu, kutoka kola hadi toe
charging-priming = Kuweka kiwashio
charging-preview = Hakikisho
charging-preview-use-pattern-hole = Tumia shimo la wastani la mfumo
charging-preview-active-pattern-median-hole = Hakikisho kwenye shimo la wastani la mfumo hai
charging-fixed-decks-longer-than-hole = Sehemu zisizobadilika ni ndefu kuliko shimo hili
charging-mass-kg-explosive = kg { $mass } za kilipuzi
charging-rate-kg-m = { $rate } kg/m
charging-count-primer = viwashio { $count }
charging-another-rule-already-has-name = Kanuni nyingine tayari ina jina hili
charging-save-reload-count-hole = Hifadhi na Pakia Upya Mashimo { $count }
charging-length = Urefu
charging-rest-length-m = salio · { $length } m
charging-rest = salio
charging-deck-takes-whatever-length-fixed-decks = Sehemu hii huchukua urefu wowote unaoachwa na sehemu zisizobadilika. Sehemu moja kwa kila kanuni hujaza.
charging-remove-deck = Ondoa sehemu
charging-add-deck = Ongeza Sehemu
charging-downhole-delay = Ucheleweshaji ndani ya shimo
charging-hole-detonator-hole-fires-long-after = Kilipuzi cha ndani ya shimo. Shimo hulipuka kwa muda huu baada ya ishara yake ya juu kufika.
charging-primer-height = Urefu wa kiwashio
charging-how-far-above-base-each-explosive = Umbali juu ya msingi wa kila sehemu ya kilipuzi ambapo kiwashio chake kinakaa.
charging-booster = Kiimarishaji
charging-cast-booster-mass-each-primer = Uzito wa kiimarishaji kilichoyeyushwa kwenye kila kiwashio.
charging-count-rule-load-product-will-need = kanuni { $count } hupakia bidhaa hii na zitahitaji nyingine ichaguliwe.
charging-rule = Kanuni
charging-holes-already-loaded-keep-their-charge = Mashimo yaliyokwisha pakiwa nayo yanabaki na chaji yake.
blast-burden-relief = Uwiano wa Mzigo
blast-ms-per-metre-last-neighbour-fire = ms kwa mita hadi jirani wa mwisho kulipuka
blast-below-hole-fires-before-rock-front = Chini ya hii shimo hulipuka kabla mwamba ulio mbele yake haujasogea: bana.
blast-above-rock-front-has-long-gone = Juu ya hii mwamba ulio mbele umeshaondoka zamani: legevu, na hatari ya kukatika na mwamba kurushwa.
blast-tight = bana
blast-good = nzuri
blast-slack = legevu
blast-free-face = uso huru
blast-fires-at = Hulipuka saa
blast-empty-won-t-detonate = tupu, halitalipuka
blast-not-reached = halijafikiwa
blast-value-ms-m-from-hole = { $value } ms/m kutoka { $hole }
blast-fires-first-free-face = hulipuka kwanza: uso huru
blast-relief = Ahueni
blast-explosive = Kilipuzi
blast-powder-factor = Kipengele cha baruti
blast-not-loaded = Halijapakiwa
blast-count-primer-delay-ms-downhole = viwashio { $count } · ms { $delay } ndani ya shimo
blast-set-initiation-point-tie-holes-play = Weka kidokezo cha kuanzia na uunganishe mashimo ili kucheza mlipuko
blast-pause = Sitisha
blast-play = Cheza
blast-back-start = Rudi mwanzo
blast-duration-ms = ya ms { $duration }
blast-real-time = Wakati halisi
blast-mic-limit = Kikomo cha MIC
blast-most-explosive-allowed-detonate-any-8 = Kilipuzi cha juu zaidi kinachoruhusiwa kulipuka katika ms 8 zozote kwenye eneo hili. Madirisha yanayozidi huwekewa alama.
blast-no-holes-loaded-surface-signal-plays = Hakuna mashimo yaliyopakiwa: ishara ya juu inachezwa, lakini hakuna kinacholipuka. Pakia mashimo kwa zana ya Chaji Mashimo.
blast-now-holes-hole = Sasa: mashimo { $holes }
blast-in-8-ms = ndani ya ms 8
blast-peak-mass-kg-time-ms = Kilele kg { $mass } saa ms { $time }
blast-peak-holes-hole-time-ms = Kilele mashimo { $holes } saa ms { $time }
blast-peak-over-limit = , kg { $over } juu ya kikomo
blast-peak-within-limit = , ndani ya kikomo
blast-top-surface-signal-lighting-each-downline = Juu: ishara ya juu inayowasha kila downline. Chini: milipuko. Bofya au buruta ili kusogeza kichwa cha uchezaji.
products-charge-rules = Kanuni za Chaji
products-new-rule = Kanuni Mpya
products-charge-products = Bidhaa za Chaji
products-new-rule-default-name = Kanuni mpya
products-no-rules = Hakuna kanuni
products-load-selected-holes-count = Pakia Mashimo Yaliyochaguliwa ({ $count })
products-unload-selected-holes-count = Ondoa Mzigo kwenye Mashimo Yaliyochaguliwa ({ $count })
products-edit-rule = Hariri Kanuni
products-duplicate-rule = Nakili Kanuni
products-delete-rule = Futa Kanuni
products-fill-product = jaza  { $product }
products-primer-offset-m-off-each-explosive = Kiwashio m { $offset } juu ya msingi wa kila sehemu ya kilipuzi, kiimarishaji kg { $booster }, ms { $delay } ndani ya shimo
products-double-click-edit = Bofya mara mbili ili kuhariri
products-edit-product = Hariri Bidhaa
blast-log-updated-charge-product-name = Umesasisha bidhaa ya chaji { $name }
blast-log-added-charge-product-name = Umeongeza bidhaa ya chaji { $name }
blast-log-updated-charge-rule-name = Umesasisha kanuni ya chaji { $name }
blast-log-added-charge-rule-name = Umeongeza kanuni ya chaji { $name }
blast-log-entry-no-longer-charge-library = Kipengee hicho hakipo tena kwenye maktaba ya chaji
blast-log-deleted-name-from-charge-library = Umefuta { $name } kutoka maktaba ya chaji
blast-log-failed-save-charge-library-error = Imeshindwa kuhifadhi maktaba ya chaji: { $error }
blast-log-cannot-load-rule-problem = Haiwezi kupakia kwa kanuni hii: { $problem }
blast-log-count-hole-too-short-fixed-decks = mashimo { $count } ni mafupi mno kwa sehemu zisizobadilika za kanuni hii na yaliachwa kama yalivyokuwa
blast-log-count-hole-have-no-depth-load = mashimo { $count } hayana kina cha kupakia
blast-log-count-loaded-hole-have-no-diameter = mashimo { $count } yaliyopakiwa hayana kipenyo, kwa hivyo uzito wa kilipuzi chake haujulikani
common-charge-holes = Chaji Mashimo
blast-log-loaded-count-hole-rule = Umepakia mashimo { $count } kwa { $rule }
blast-log-unload-holes = Ondoa Mzigo kwenye Mashimo
blast-log-unloaded-count-hole = Umeondoa mzigo kwenye mashimo { $count }
blast-log-select-holes-active-pattern-first = Chagua kwanza mashimo ya mfumo hai
blast-log-rule-no-longer-charge-library = Kanuni hiyo haipo tena kwenye maktaba ya chaji
blast-log-there-no-charge-rule-load-add = Hakuna kanuni ya chaji ya kupakia nayo: ongeza moja kwenye paneli ya bidhaa
blast-rule-stemming = Stemming
blast-rule-air-deck = Sehemu ya hewa
blast-rule-give-rule-name = Ipe kanuni jina
blast-rule-add-least-one-deck = Ongeza angalau sehemu moja
blast-rule-only-one-deck-can-fill-rest = Sehemu moja pekee inaweza kujaza salio la shimo
blast-rule-deck-lengths-must-greater-than-zero = Urefu wa sehemu lazima uwe zaidi ya sifuri
blast-rule-no-product-named-name = Hakuna bidhaa iitwayo '{ $name }'
blast-rule-rule-needs-least-one-explosive-deck = Kanuni inahitaji angalau sehemu moja ya kilipuzi
state-save-charge-product = Hifadhi Bidhaa ya Chaji
state-save-charge-rule = Hifadhi Kanuni ya Chaji
state-delete-charge-library-entry = Futa Kipengee cha Maktaba ya Chaji
ui-click-drag-over-holes-load-them = Bofya au buruta juu ya mashimo ili kuyapakia kwa { $rule }
ui-hold-shift-unload = shikilia Shift ili kuondoa mzigo
ui-no-charge-rule-load = Hakuna kanuni ya chaji ya kupakia nayo
ui-right-click-charge-rules-heading-add = bofya kulia kichwa cha Kanuni za Chaji ili kuongeza moja
omf-element-name-has-count-charge-naming = Kipengele '{ $name }' kina chaji { $count } zinazotaja mashimo ambayo hakina tena
