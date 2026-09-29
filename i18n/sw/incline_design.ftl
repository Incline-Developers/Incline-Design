# Kiswahili. Maandishi yasiyopatikana hutumia katalogi ya Kiingereza kama akiba.

## Shared

common-cancel = Ghairi
common-clear = Futa
common-close = Funga
common-color = Rangi
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
tri-create-type-label = Aina ya utatuzi
tri-create-type-help =
    Uso wazi huunda karatasi ya mtindo wa ardhi. Kigumu huunda mfumo
    uliofungwa kabisa na kinahitaji chanzo kinachoweza kuunda mpaka usiovuja maji.
tri-create-output-name = Jina la matokeo
tri-create-output-name-help = Jina litakalopewa utatuzi utakaozalishwa.
tri-create-output-name-hint = jina la utatuzi
tri-create-run = Tatua (Triangulate)

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
drill-hole-colour-title = Rangi za Mashimo ya Uchimbaji: { $name }
drill-hole-colour-stop = Kituo { $index }
properties-restore-defaults = Rejesha mipangilio ya { $heading } kwa chaguo-msingi

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
status-fps = FPS: { $fps }
status-clip = Kata karibu/mbali/Δ: { $near } / { $far } / { $delta } m

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
survey-unsupported = Rasta haziwezi kubadilishwa na chombo hiki. Hazichaguliki kwenye mwonekano, kwa hivyo hakuna chochote kwenye uteuzi kinachofikia hapa.
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
survey-system-section = Fasili ya gridi ya mgodi
survey-reference-note = Fremu ambayo kila fasili imeandikwa dhidi yake: kuratibu ambazo data yako tayari inazo data inapoingizwa. Haina vigezo vyake vyenyewe. Bofya kulia mfumo ili kuufanya mfumo wa kuratibu wa mgodi, au eneo tupu hapa chini ili kufasili mmoja.
survey-system-name = Jina
survey-reference-system = Mfumo wa marejeo
survey-reference-origin = Kidokezo kinachojulikana — kuratibu za marejeo
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
survey-axis-help = Majina ambayo mfumo huu huita mihimili yake, kama si X, Y na Z — "E", "N", "RL" kwa gridi ya mgodi. Hutumika kila mahali kuratibu zinapoonyeshwa, lakini tu wakati huu ndio mfumo wa kuratibu wa mgodi. Taja mihimili yote mitatu au isitaje hata mmoja.
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

## About strings

about-copyright-c-2026-leo-timmins =
    Hakimiliki (c) 2026 Leo Timmins, Lucas Timmins na wachangiaji wa Incline Design. Ruhusa inatolewa hapa, bila malipo, kwa mtu yeyote anayepata nakala ya programu hii kuishughulikia bila kizuizi, kulingana na masharti ya Leseni ya MIT.

    Incline Design inatolewa "JINSI ILIVYO", BILA DHAMANA YA AINA YOYOTE, WAZI AU YA KUDOKEZWA, ikiwemo lakini bila kikomo kwa dhamana za UUZAJI, UFAAO KWA KUSUDI MAALUM na KUTOKIUKA.
about-free-open-source-mine-design = Muundo wa Migodi Huru na Chanzo Wazi
about-licensed-under-mit-license = Imeidhinishwa chini ya Leseni ya MIT

## App strings

app-activated-browser-project-name = Mradi wa kivinjari '{ $name }' umeamilishwa.
app-browser-project-deletion-failed-erro = Ufutaji wa mradi wa kivinjari umeshindwa: { $error }
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
app-failed-load-browser-preferences-erro = Imeshindwa kupakia mapendeleo ya kivinjari: { $error }
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
block-model-minimum-nearby-samples-required-esti = Sampuli za karibu za chini zinazohitajika kukadiria kitalu. Vitalu vyenye sampuli chache ndani ya radiasi ya utafutaji vinaachwa tupu.
block-model-minimum-samples = Sampuli za chini
block-model-nugget = Nugget
block-model-numeric-interval-fields-interpolate = Sehemu za vipindi vya nambari za kuingilianisha. Kila sehemu iliyochaguliwa inakuwa kigezo kimoja cha mfano wa vitalu.
block-model-ordinary-kriging-estimates-numeric-d = Kriging ya Kawaida hukadiria vipindi vya nambari vya mashimo ya uchimbaji kwenye kitovu cha kila kitalu kwa kutumia variogramu ya duara.
block-model-partial-sill = Sill ya kiasi
block-model-range-search-radius = Wigo / radiasi ya utafutaji
block-model-samples-farther-than-distance-exclud = Sampuli zilizo mbali zaidi ya umbali huu hazijumuishwi; ushirikiano hufikia sifuri kwenye wigo huu.
block-model-select-all = Chagua yote
block-model-spatially-correlated-variance-contri = Utofauti unaohusiana kwa anga unaotolewa na mfano wa duara. Pamoja na nugget, huweka ushirikiano kwenye umbali sifuri.
block-model-spherical-variogram-search = Variogramu ya duara na utafutaji
block-model-threshold = <= kizingiti
block-model-threshold-2 = >= kizingiti
block-model-threshold-min = Kizingiti / kima cha chini
block-model-upper-x-y-z-extent = Wigo wa juu wa X, Y na Z wa kufunika. Kitalu cha mwisho kinaweza kupita wigo huu wakati urefu si kizidishi kamili cha ukubwa wa kitalu.
block-model-variable = Kigezo
block-model-variance-effectively-zero-separation = Utofauti kwenye tofauti ya karibu sifuri unaosababishwa na hitilafu ya kipimo au tofauti chini ya kipimo cha sampuli. Tumia sifuri wakati hakuna athari ya nugget inayokusudiwa.
block-model-volume-cache-block-volume-usage-feedback-readback = Usomaji wa mrejesho wa matumizi ya ujazo wa vitalu umekatika
block-model-volume-cache-block-volume-usage-feedback-readback-2 = Usomaji wa mrejesho wa matumizi ya ujazo wa vitalu umeshindwa: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z

## Canvas strings

canvas-not-selectable-choose-closed-polylin = Haiwezi kuchaguliwa | Chagua mstari wa pointi nyingi uliofungwa
canvas-polyline-layer-layer-count-vertices = Mstari wa Pointi Nyingi | Tabaka: { $layer } | vipeo { $count }
canvas-surface-name = Uso | { $name }
canvas-trimmed = Imepunguzwa

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
cmd-block-model-loading-name-2 = Inapakia { $name }…
cmd-chamfer-chamfered-corner-corner-radius-radiu = Kona { $corner } imepunguzwa kwa radiasi { $radius } na sehemu { $segments }
cmd-chamfer-radius-radius = Radiasi { $radius }
cmd-commands-clipped = Imekatwa
cmd-commands-command-failed-error = Amri imeshindwa: { $error }
cmd-commands-select-one-more-objects-before = Chagua kitu kimoja au zaidi kabla ya kuweka { $axis }
cmd-commands-sliced = Imekatwa (Sliced)
cmd-contours-contour-generation-failed-error = Uzalishaji wa mistari ya mwinuko umeshindwa: { $error }
cmd-contours-contours-name-were-discarded-layer = Mistari ya mwinuko ya '{ $name }' imeondolewa: tabaka '{ $layer_name }' sasa lipo
cmd-contours-contours-name-were-discarded-project = Mistari ya mwinuko ya '{ $name }' imeondolewa: mradi umefungwa
cmd-contours-contours-name-were-discarded-selecte = Mistari ya mwinuko ya '{ $name }' imeondolewa: tabaka la matokeo lililochaguliwa limefutwa
cmd-contours-generated-line-count-contour-polylin = Mistari { $line_count } ya mwinuko imezalishwa kwa utatuzi '{ $name }' kwenye tabaka '{ $layer_name }'
cmd-creation-assembled-assembled-count-closed-bou = Miduara { $assembled_count } ya mpaka iliyofungwa imekusanywa kutoka mifuatano iliyokatika iliyo wazi
cmd-creation-created-triangulation-from-boundary = Utatuzi umeundwa kutoka miduara { $boundary_count } ya mpaka na vizuizi { $constraint_count } vilivyo wazi, aina ya uso { $surface_type }
cmd-creation-creating-triangulation = Inaunda utatuzi…
cmd-creation-generate-upper-surface-ignored-count = Zalisha uso wa juu: sehemu { $count } za mstari wa mvunjiko zenye mgongano wa chini zimepuuzwa; vitu vya chanzo havijabadilika
cmd-creation-ignored-rejected-non-polyline-degene = Vitu { $rejected } visivyo mistari ya pointi nyingi au vilivyoharibika vimepuuzwa wakati wa utatuzi
cmd-creation-weld-retry-moved-coarse-welded = Unga na ujaribu tena: vipeo { $coarse_welded } vimehamishwa hadi nafasi za pamoja (hadi m{ $coarse_weld_tol }); vitu vya chanzo havijabadilika
cmd-creation-welded-welded-breakline-vertex-verti = Vipeo { $welded } vya mstari wa mvunjiko vilivyopatana ndani ya kikomo vimeungwa
cmd-cuts-clipped-surface-name-polyline-mode = Uso '{ $name }' umekatwa kwa mstari wa pointi nyingi ({ $mode })
cmd-cuts-clipping-surface-polyline = Inakata uso kwa mstari wa pointi nyingi…
cmd-cuts-cut-topology-name-pit-shell = Tofolojia '{ $name }' imekatwa kwa ganda la shimo
cmd-cuts-cut-triangulation-name-z-band = Utatuzi '{ $name }' umekatwa kwa mkanda wa Z [{ $min }, { $max }]
cmd-cuts-cutting-topology-pit-shell = Inakata tofolojia kwa ganda la shimo…
cmd-cuts-cutting-triangulation-z = Inakata utatuzi kwa Z…
cmd-cuts-ignored-count-vertical-degenerate-re = Nyuso { $count } za tofolojia ya rejeleo zilizo wima au zilizoharibika bila eneo la XY zimepuuzwa
cmd-cuts-site-skipped-constraint-from-x = { $site }: kizuizi kimerukwa ({ $from_x }, { $from_y }) -> ({ $to_x }, { $to_y }) kitatuzi hakikuweza kugawanya
cmd-cuts-site-skipped-skipped-near-degenerate = { $site }: kingo { $skipped } za vizuizi zilizo karibu kuharibika zimerukwa; mpaka wa ukataji unaweza kupotoka kidogo karibu nazo
cmd-cuts-trimmed-surface-surface-topology-top = Uso '{ $surface }' umepunguzwa kwa tofolojia '{ $topology }' ({ $mode })
cmd-cuts-trimming-surface-topology = Inapunguza uso kwa tofolojia…
cmd-drape-draped-intersected-vertices-changed = Vipeo { $intersected } vimefunikwa; { $changed } vimebadilisha kimo
cmd-drape-none-selected-design-vertices-inters = Hakuna kipeo cha muundo kilichochaguliwa kinachokatiza tofolojia zilizochaguliwa
cmd-drape-objects-changed-object-s-changed = vitu { $objects } vimebadilika · vipeo { $changed } kati ya { $intersected } vinavyokatiza vimehamishwa
cmd-drape-select-one-more-design-objects = Chagua kitu kimoja au zaidi cha muundo cha kufunika
cmd-drape-select-one-more-topologies-drape = Chagua tofolojia moja au zaidi ya kufunikia
cmd-drape-selected-topologies-no-longer-loaded = Tofolojia zilizochaguliwa hazijapakiwa tena
cmd-drill-hole-drill-pattern-too-large-contains = Mfumo wa uchimbaji ni mkubwa mno au una kuratibu za kola zisizo sahihi
cmd-drill-hole-enter-name-drill-pattern = Weka jina la mfumo wa uchimbaji
cmd-drill-hole-failed-load-drillholes-error = Imeshindwa kupakia mashimo ya uchimbaji: { $error }
cmd-drill-hole-hole-depth-must-greater-than = Kina cha shimo lazima kiwe zaidi ya sifuri
cmd-drill-hole-hole-diameter-must-greater-than = Kipenyo cha shimo lazima kiwe zaidi ya sifuri
cmd-drill-hole-loaded-drillhole-dataset-name-holes = Data ya mashimo ya uchimbaji '{ $name }' imepakiwa: mashimo { $holes }, sehemu za rangi { $fields }
cmd-drill-hole-pattern-contains-no-holes = Mfumo hauna mashimo
cmd-explode-count-line-s = mistari { $count }
cmd-explode-explode-polyline = Vunja Mstari wa Pointi Nyingi
cmd-explode-exploded-polyline-into-count-line = Mstari wa pointi nyingi umevunjwa kuwa sehemu { $count } za mstari
cmd-file-block-model-csv-encoding-failed = Usimbaji wa CSV wa mfano wa vitalu umeshindwa: { $error }
cmd-file-block-model-csv-export-failed = Uhamishaji wa CSV ya mfano wa vitalu umeshindwa: { $error }
cmd-file-browser-recovery-files-unavailable-s = Faili za kurejesha za kivinjari hazipatikani; miradi iliyohifadhiwa inabaki kwenye IndexedDB
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
cmd-file-discard-was-cancelled-because-projec = Kuondoa kumeghairiwa kwa sababu mradi ulibadilika wakati OMF ilikuwa ikipakiwa upya
cmd-file-discarded-changes-layer-target-name = Mabadiliko kwenye tabaka '{ $target_name }' yameondolewa
cmd-file-discarded-changes-reloaded-path = Mabadiliko yameondolewa: { $path } imepakiwa upya
cmd-file-downloaded-description-file-name = Umepakua { $description }: { $file_name }
cmd-file-dxf-download-encoding-failed-error = Usimbaji wa kupakua DXF umeshindwa: { $error }
cmd-file-dxf-import-failed-error = Kuingiza DXF kumeshindwa: { $error }
cmd-file-encoding-block-model-csv-download = Inasimba upakuaji wa CSV ya mfano wa vitalu…
cmd-file-encoding-dxf-download = Inasimba upakuaji wa DXF…
cmd-file-encoding-triangulation-download = Inasimba upakuaji wa utatuzi…
cmd-file-exit-deferred-until-background-expor = Kutoka kumeahirishwa hadi uhamishaji wa nyuma ukamilike
cmd-file-exit-requested-no-unsaved-changes = Kutoka kumeombwa bila mabadiliko yasiyohifadhiwa
cmd-file-exported-block-model-csv-path = Umehamisha CSV ya mfano wa vitalu kwenda { $path }
cmd-file-exported-description-dxf-path = Umehamisha { $description } kwenda DXF: { $path }
cmd-file-exported-triangulation-name-path = Umehamisha utatuzi '{ $name }' kwenda { $path }
cmd-file-exporting-name = Inahamisha { $name }…
cmd-file-exporting-triangulation-name-path = Inahamisha utatuzi '{ $name }' kwenda { $path }
cmd-file-fatal-renderer-failure-reason = Hitilafu kubwa ya kichoraji: { $reason }
cmd-file-file-dialog-action-failed-msg = Kitendo cha dirisha la faili kimeshindwa: { $msg }
cmd-file-imported-added-object-s-from = Umeingiza vitu { $added } kutoka { $name }
cmd-file-imported-total-dxf-object-s = Umeingiza vitu { $total } vya DXF
cmd-file-layer-discard-was-cancelled-because = Uondoaji wa tabaka umeghairiwa kwa sababu mradi ulibadilika wakati mradi ulikuwa ukipakiwa upya
cmd-file-no-recovery-directory-available-erro = Hakuna saraka ya kurejesha inayopatikana: { $error }
cmd-file-no-unsaved-project-content-nothing = Hakuna maudhui ya mradi yasiyohifadhiwa; hakuna cha kurejesha
cmd-file-parsing-browser-dxf-import = Inachanganua uingizaji wa DXF wa kivinjari…
cmd-file-parsing-dxf-import = Inachanganua uingizaji wa DXF…
cmd-file-project-will-close-after-its = Mradi utafungwa baada ya uhifadhi wake wa sasa kukamilika
cmd-file-project-will-close-after-its-2 = Mradi utafungwa baada ya uhifadhi wake wa sasa kukamilika
cmd-file-queued-count-triangulation-file-s = Umeweka foleni faili { $count } za utatuzi za kuingiza
cmd-file-recovery-copies-path-reopen-them = Nakala za kurejesha ziko { $path }; zifungue tena baada ya kuanzisha upya
cmd-file-recovery-copy-failed-error = Nakala ya kurejesha imeshindwa: { $error }
cmd-file-recovery-copy-failed-failure = Nakala ya kurejesha imeshindwa: { $failure }
cmd-file-recovery-copy-written-path = Nakala ya kurejesha imeandikwa: { $path }
cmd-file-reverting-layer = Inarejesha tabaka…
cmd-file-reverting-project = Inarejesha mradi…
cmd-file-save-failed-message = Kuhifadhi kumeshindwa: { $message }
cmd-file-save-worker-ended-without-result = Kazi ya kuhifadhi imeisha bila matokeo
cmd-file-saved-project-path = Umehifadhi mradi kama: { $path }
cmd-file-saved-project-path-2 = Umehifadhi mradi: { $path }
cmd-file-selected-block-model-no-longer = Mfano wa vitalu uliochaguliwa hauja pakiwa tena
cmd-file-switching-project = Inabadilisha mradi…
cmd-file-triangulation-download-encoding-fail = Usimbaji wa kupakua utatuzi umeshindwa: { $error }
cmd-file-user-chose-exit-without-saving = Mtumiaji amechagua kutoka bila kuhifadhi
cmd-file-user-requested-exit-project-export = Mtumiaji ameomba kutoka (uthibitisho wa uhamishaji wa mradi au kazi isiyohifadhiwa unahitajika)
cmd-file-viewport = Mwonekano
cmd-file-wait-current-project-save-finish = Subiri uhifadhi wa mradi wa sasa ukamilike
cmd-file-wait-current-project-switch-finish = Subiri ubadilishaji wa mradi wa sasa ukamilike
cmd-file-wait-project-operation-finish-before = Subiri operesheni ya mradi ikamilike kabla ya kuondoa mabadiliko
cmd-file-wait-project-revert-finish-before = Subiri urejeshaji wa mradi ukamilike kabla ya kuhifadhi
cmd-fuse-closed-polyline = Mstari wa pointi nyingi uliofungwa
cmd-fuse-count-source-line-s = mistari { $count } ya chanzo
cmd-fuse-created-shape-object-id-vertices = { $shape } { $object_id } imeundwa yenye vipeo { $vertices } kutoka mistari { $sources } ya chanzo
cmd-fuse-fuse-click-did-not-hit = Unganisha: kubofya hakukugusa kitu chochote (hakuna kilicho chini ya kishale)
cmd-fuse-fuse-click-was-not-close = Unganisha: kubofya hakukuwa karibu vya kutosha na ncha yoyote ya mstari uliochaguliwa
cmd-fuse-fuse-clicked-object-object-id = Unganisha: kitu kilichobofywa { $object_id } ni mstari wa pointi nyingi uliofungwa, kuunganisha hufanya kazi kwenye mistari iliyo wazi tu
cmd-fuse-fuse-clicked-object-object-id-2 = Unganisha: kitu kilichobofywa { $object_id } si mstari wa pointi nyingi ulio wazi (ni { $kind })
cmd-fuse-fuse-clicked-object-object-id-3 = Unganisha: kitu kilichobofywa { $object_id } hakipo tena
cmd-fuse-fuse-clicked-polyline-object-id = Unganisha: mstari wa pointi nyingi uliobofywa { $object_id } una vipeo { $count } pekee, vinahitajika angalau 2
cmd-fuse-fuse-endpoint-marker-marker-index = Unganisha: alama ya ncha { $marker_index } haipo tena
cmd-fuse-fuse-line-needs-least-3 = Unganisha: mstari unahitaji angalau vipeo 3 tofauti ili kufungwa kuwa mstari wa pointi nyingi (una { $count })
cmd-fuse-fuse-lines = Unganisha Mistari
cmd-fuse-fuse-need-least-2-segments = Unganisha: inahitajika angalau sehemu 2 kukamilisha (kuna { $count })
cmd-fuse-fuse-no-active-layer-place = Unganisha: hakuna tabaka hai la kuweka mstari ulioungwa
cmd-fuse-fuse-no-active-project-cannot = Unganisha: hakuna mradi hai, haiwezekani kukamilisha
cmd-fuse-fuse-no-source-line-close = Unganisha: hakuna mstari wa chanzo wa kufunga kuwa mstari wa pointi nyingi
cmd-fuse-fuse-object-awaiting-id-no = Unganisha: kitu { $awaiting_id } si mstari halali wa pointi nyingi tena
cmd-fuse-fuse-object-object-id-already = Unganisha: kitu { $object_id } tayari ni sehemu ya mnyororo wa kuunganisha, bofya mstari mwingine
cmd-fuse-fuse-result-has-too-few = Unganisha: matokeo yana vipeo vichache mno ({ $count }), inasitisha
cmd-fuse-fuse-segment-object-object-id = Unganisha: sehemu ya kitu { $object_id } si mstari halali wa pointi nyingi tena, inasitisha
cmd-fuse-fuse-source-object-object-id = Unganisha: kitu cha chanzo { $object_id } si mstari halali wa pointi nyingi ulio wazi tena
cmd-fuse-fuse-source-object-object-id-2 = Unganisha: kitu cha chanzo { $object_id } hakipo tena
cmd-fuse-open-polyline = Mstari wa pointi nyingi ulio wazi
cmd-include-include-failed-message = Kujumuisha kumeshindwa: { $message }
cmd-include-included-solid-shape-name-topology = Kigumu '{ $shape_name }' kimejumuishwa kwenye tofolojia '{ $topology_name }' (nyuso { $retained } za tofolojia zimehifadhiwa, nyuso { $skipped } za kifuniko zimerukwa)
cmd-include-including-pit-stockpile-solid = Inajumuisha kigumu cha shimo/rundo la hifadhi…
cmd-insert-point-count-operation-point-s = vidokezo { $count } vya { $operation }
cmd-insert-point-insert-point-elevation-requires-fini = Kuingiza Kidokezo kwenye Kimo kunahitaji kimo dhahiri
cmd-insert-point-insert-points = Ingiza Vidokezo
cmd-insert-point-inserted-count-operation-point-s = Vidokezo { $count } vya { $operation } vimeingizwa
cmd-insert-point-intersection = Makutano
cmd-insert-point-no-new-operation-points-were = Hakuna vidokezo vipya vya { $operation } vilivyopatikana
cmd-insert-point-select-least-two-polylines-before = Chagua angalau mistari miwili ya pointi nyingi kabla ya kuingiza vidokezo vya makutano
cmd-insert-point-select-one-more-polylines-before = Chagua mstari mmoja au zaidi wa pointi nyingi kabla ya kuingiza kidokezo kwenye kimo
cmd-layer-created-layer-name = Umeunda tabaka '{ $name }'
cmd-layer-deleted-layer-layer-id-all = Tabaka { $layer_id } limefutwa (pamoja na vitu vyote vilivyomo)
cmd-layer-duplicated-layer-duplicate-name = Umenakili tabaka '{ $duplicate_name }'
cmd-layer-locked = Imefungwa
cmd-layer-name-copy = nakala ya { $name }
cmd-layer-selected-count-object-s-layer = Vitu { $count } vimechaguliwa kwenye tabaka { $layer_id }
cmd-layer-state-layer-name = { $state } tabaka '{ $name }'
cmd-layer-unlocked = Haijafungwa
cmd-move-tool-applied-move-delta-delta-count = Mabadiliko ya kuhamisha ({ $delta }) yametumika kwa kola { $count } za mashimo ya uchimbaji
cmd-move-tool-applied-move-delta-delta-count-2 = Mabadiliko ya kuhamisha ({ $delta }) yametumika kwa vitu { $count }
cmd-move-tool-count-hole-s = mashimo { $count }
cmd-object-edit-edited-kind = { $kind } imehaririwa
cmd-object-edit-edited-kind-count-vertices = { $kind } imehaririwa (vipeo { $count })
cmd-object-edit-no-changes-apply = Hakuna mabadiliko ya kutumia
cmd-object-edit-object-changed-since-editor-opened = Kitu hiki kimebadilika tangu kihariri kufunguliwa; kifungue tena ili kuhariri toleo la sasa
cmd-object-edit-object-edit-target-changed-discardin = Lengo la uhariri wa kitu limebadilika; uhariri unaondolewa
cmd-object-edit-object-no-longer-exists-document = Kitu hicho hakipo tena kwenye hati
cmd-object-edit-select-single-design-object-edit = Chagua kitu kimoja cha muundo ili kukihariri
cmd-object-edit-unassigned = Haijawekwa
cmd-offset-create-offset = Unda Mkabala
cmd-offset-created-offset-count-object-s = Mkabala wa vitu { $count } umeundwa
cmd-offset-offset-distance-must-greater-than = Umbali wa mkabala lazima uwe zaidi ya sifuri
cmd-omf-could-not-open-project-source = Imeshindwa kufungua mradi { $source_name }: { $error }
cmd-omf-create-open-project-before-merging = Unda au fungua mradi kabla ya kuunganisha data
cmd-omf-encoding-project = Inasimba mradi…
cmd-omf-exported-project-path = Umehamisha mradi kwenda { $path }
cmd-omf-imported-project-project-name-from = Mradi '{ $project_name }' umeingizwa kutoka { $source_name }: seti za data { $count } za ngazi ya juu
cmd-omf-importing-project = Inaingiza mradi…
cmd-omf-omf-export-failed-error = Kuhamisha OMF kumeshindwa: { $error }
cmd-omf-omf-import-failed-error = Kuingiza OMF kumeshindwa: { $error }
cmd-omf-opened-project-project-name-from = Mradi '{ $project_name }' umefunguliwa kutoka { $source_name }
cmd-omf-project-source-name-contains-no = Mradi '{ $source_name }' hauna vipengele vya data vinavyotumika
cmd-omf-source-name-applied-project-origin = { $source_name }: chanzo cha mradi { $origin } kilitumika kabla ya kuunganisha
cmd-omf-source-name-coordinate-reference-sys = { $source_name }: mfumo wa marejeleo wa kuratibu '{ $source_crs }' unatofautiana na CRS ya mradi '{ $target_crs }'; kuratibu ziliunganishwa bila kuoanisha upya makadirio
cmd-omf-source-name-units-source-units = { $source_name }: vipimo '{ $source_units }' vinatofautiana na vipimo vya mradi '{ $target_units }'; kuratibu ziliunganishwa bila kubadilisha
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = Hakuna data ya Incline Design iliyofunguliwa ya kuhamisha
cmd-placement-2-vertices = vipeo 2
cmd-placement-count-vertices = vipeo { $count }
cmd-placement-created-circle-radius-radius-m = Duara limeundwa lenye radiasi m{ $radius }
cmd-placement-created-closed-polyline-count-vertic = Mstari wa pointi nyingi uliofungwa umeundwa wenye vipeo { $count }
cmd-placement-created-line-segment-2-vertices = Sehemu ya mstari yenye vipeo 2 imeundwa
cmd-placement-created-open-polyline-count-vertices = Mstari wa pointi nyingi ulio wazi umeundwa wenye vipeo { $count }
cmd-placement-placed-point-x-y-z = Kidokezo kimewekwa kwenye { $x }, { $y }, { $z }
cmd-placement-radius-radius-m = Radiasi m{ $radius }
cmd-plot-composing-engineering-drawing = Inatunga mchoro wa uhandisi…
cmd-plot-could-not-write-engineering-drawing = Imeshindwa kuandika mchoro wa uhandisi: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Kipimo cha mchoro kimeoanishwa na data inayoonekana: 1:{ $scale }
cmd-plot-plot = Mchoro
cmd-plot-saved-engineering-drawing-descriptio = Mchoro wa uhandisi umehifadhiwa: { $description } ({ $width } × { $height } px kwa { $dpi } dpi)
cmd-point-cloud-failed-load-point-cloud-error = Imeshindwa kupakia wingu la vidokezo: { $error }
cmd-point-cloud-loaded-point-cloud-name-count = Umepakia wingu la vidokezo { $name } (vidokezo { $count })
cmd-point-cloud-point-cloud-loader-disconnected-path = Kipakiaji cha wingu la vidokezo kimekatika kwa { $path }
cmd-point-cloud-tin-max-edge-disabled = (ukingo wa juu umezimwa)
cmd-point-cloud-tin-max-edge-max-edge = (ukingo wa juu { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = TIN ya wingu la vidokezo imeshindwa: { $error }
cmd-point-cloud-tin-terrain-tin-spatially-subsampled-sam = TIN ya ardhi: sampuli za anga { $sampled } kati ya vidokezo { $total }
cmd-point-cloud-tin-terrain-tin-triangulated-vertex-coun = TIN ya ardhi: vidokezo { $vertex_count } vya kipekee vya XY vimetatuliwa kuwa nyuso { $face_count }{ $suffix }
cmd-products-added-product-delay-ms-ms = Umeongeza bidhaa { $delay_ms } ms { $name }
cmd-products-deleted-product-delay-ms-ms = Umefuta bidhaa { $delay_ms } ms { $name }
cmd-products-failed-save-products-error = Imeshindwa kuhifadhi bidhaa: { $error }
cmd-products-product-no-longer-palette = Bidhaa hiyo haipo tena kwenye pala
cmd-raster-draped-raster-raster-over-triangulat = Rasta { $raster } imefunikwa juu ya utatuzi { $triangulation } (wigo unaopishana)
cmd-raster-failed-load-raster-name-error = Imeshindwa kupakia rasta { $name }: { $error }
cmd-raster-failed-load-raster-path-error = Imeshindwa kupakia rasta { $path }: { $error }
cmd-raster-loaded-raster-name-via-driver = Rasta { $name } imepakiwa kupitia { $driver } ({ $srcx }x{ $srcy }, hakikisho { $prevx }x{ $prevy })
cmd-raster-no-loaded-triangulation-overlaps-ext = Hakuna utatuzi uliopakiwa unaopishana na wigo wa { $name }
cmd-raster-raster-loader-disconnected-path = Kipakiaji cha rasta kimekatika kwa { $path }
cmd-raster-undraped-rasters-from-count-triangul = Umeondoa ufunikaji wa rasta kutoka utatuzi { $count }
cmd-relimit-relimit-click-did-not-hit = Weka Upya Kikomo: kubofya hakukugusa kitu chochote (hakuna kilicho chini ya kishale)
cmd-relimit-relimit-click-ignored-tool-not = Weka Upya Kikomo: kubofya kumepuuzwa, chombo hakingoji uchaguzi wa lengo kwa sasa
cmd-relimit-relimit-clicked-source-line-itself = Weka Upya Kikomo: umebofya mstari wa chanzo wenyewe, chagua mstari mwingine
cmd-relimit-relimit-no-source-line-set = Weka Upya Kikomo: hakuna mstari wa chanzo uliowekwa, inasitisha uchaguzi
cmd-relimit-relimited-line-source-id-selected = Mstari { $source_id } umewekwa upya kikomo kwa lengo lililochaguliwa
cmd-relimit-resized-line-source-id-using = Mstari { $source_id } umebadilishwa ukubwa kwa kutumia { $mode } thamani { $value }
cmd-rename-item-no-longer-belongs-active = Kipengele hicho hakihusiani tena na mradi hai
cmd-rename-renamed-before-name = '{ $before }' imebadilishwa jina kuwa '{ $name }'
cmd-rename-renamed-before-name-requested-alread = '{ $before }' imebadilishwa jina kuwa '{ $name }' ('{ $requested }' tayari inatumika)
cmd-rotate-collar-turned-count-drillhole-collar-s = Kola { $count } za mashimo ya uchimbaji zimezungushwa { $rotation }
cmd-section-verb-count-item-s-section = { $verb } vipengele { $count } kwenye { $section }
cmd-selection-delete-vertex = Futa Kipeo
cmd-selection-deleted-count-selected-object-s = Vitu { $count } vilivyochaguliwa vimefutwa
cmd-selection-deleted-vertex-vertex-from-polyline = Kipeo { $vertex } kimefutwa kutoka mstari wa pointi nyingi { $object_id }
cmd-selection-duplicate-selection = Nakili Uteuzi
cmd-selection-duplicated-count-object-s = Vitu { $count } vimenakiliwa
cmd-session-created-triangulation-name-vertex-co = Utatuzi '{ $name }' umeundwa (vipeo { $vertex_count }, nyuso { $face_count }) kutoka aina ya uso { $surface_type }
cmd-session-deleted-triangulation-name-from-proj = Utatuzi '{ $name }' umefutwa kutoka kwenye mradi
cmd-session-failed-load-triangulation-error = Imeshindwa kupakia utatuzi: { $error }
cmd-session-failed-load-triangulation-message = Imeshindwa kupakia utatuzi: { $message }
cmd-session-loaded-triangulation-name-path-verte = Utatuzi '{ $name }' umepakiwa ({ $path }, vipeo { $vertex_count }, nyuso { $face_count })
cmd-session-set-triangulation-tri-id-color = Rangi ya utatuzi { $tri_id } imewekwa kuwa { $color }
cmd-session-triangulation-load-path-ended-withou = Upakiaji wa utatuzi wa { $path } umeisha bila matokeo
cmd-session-triangulation-operation-failed-messa = Operesheni ya utatuzi imeshindwa: { $message }
cmd-session-unloaded-triangulation-name = Utatuzi '{ $name }' umeondolewa
cmd-slice-entered-slice-view-cx-cy = Umeingia kwenye mwonekano wa ukataji @ { $cx }, { $cy }, { $cz } kando ya { $dx }, { $dy } (mstari wa m{ $length })
cmd-slice-exited-slice-view = Umetoka kwenye mwonekano wa ukataji
cmd-slice-reset-section-view-fit-extents = Rejesha mwonekano wa sehemu (oanisha na wigo)
cmd-slice-set-section-grid-enabled = Weka gridi ya sehemu = { $enabled }
cmd-split-created-2-open-polylines = Mistari 2 ya pointi nyingi iliyo wazi imeundwa
cmd-split-split-line = Gawanya Mstari
cmd-split-split-points-choose-interior-vertex = Gawanya kwa Vidokezo: chagua kipeo cha ndani cha mstari ulio wazi
cmd-split-split-points-choose-two-non = Gawanya kwa Vidokezo: chagua vipeo viwili visivyo jirani vya mstari wa pointi nyingi
cmd-split-split-source-polyline-into-two = Mstari wa chanzo wa pointi nyingi umegawanywa kuwa mistari miwili iliyo wazi ya pointi nyingi
cmd-text-finished-text-edit-object-object = Umemaliza kuhariri maandishi ya kitu { $object_id }
cmd-text-updated-text-object-object-id = Umesasisha maandishi kwenye kitu { $object_id }
cmd-view-centre-rotation-not-available-flying = Kitovu cha mzunguko hakipatikani katika Hali ya Kuruka
cmd-view-fixed-centre-rotation-x-y = Kitovu cha mzunguko kimefungwa kwenye { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = Hakuna kidokezo chini ya kishale cha kufungia kitovu cha mzunguko
cmd-view-released-centre-rotation = Kitovu cha mzunguko kimeachiliwa
cmd-view-reset-view-fit-extents = Rejesha mwonekano (oanisha na wigo)
cmd-view-set-topology-wireframes-enabled = Nyaya za tofolojia zimewekwa = { $enabled }
cmd-view-set-view-points-enabled = Mwonekano wa vidokezo umewekwa = { $enabled }
cmd-view-set-xy-grid-enabled = Weka gridi ya XY = { $enabled }
cmd-view-zoom-extents-preserving-angle = Kuza kwa wigo wote (ukihifadhi pembe)

## Common strings

common-add-product = Ongeza Bidhaa
common-background = Mandhari nyuma
common-block-model = Mfano wa vitalu
common-block-models = Mifano ya Vitalu
common-cancelled = Imeghairiwa
common-chamfer = Kingo iliyopunguzwa (Chamfer)
common-choose = Chagua...
common-circle = Duara
common-click-point-fix-centre-rotation = Bofya kidokezo ili kufunga kitovu cha mzunguko
common-clip-surface-polyline = Kata Uso kwa Mstari wa Pointi Nyingi...
common-closed = Imefungwa
common-colour = Rangi
common-confirm-omf-rewrite = Thibitisha Uandikaji Upya wa OMF
common-could-not-replace-current-project = Imeshindwa kubadilisha mradi wa sasa: { $error }
common-count-object-s = vitu { $count }
common-create = Unda
common-create-batter-berm = Unda Mteremko na Lundo la Kinga
common-create-bezier-curve = Unda Mviringo wa Bezier
common-create-block-model = Unda Mfano wa Vitalu
common-create-block-model-2 = Unda Mfano wa Vitalu...
common-create-circle = Unda Duara
common-create-drill-pattern = Unda Mfumo wa Uchimbaji
common-create-layer = Unda Tabaka
common-create-line = Unda Mstari
common-create-ore-triangulation = Unda Utatuzi wa Madini
common-create-ore-triangulation-2 = Unda Utatuzi wa Madini...
common-create-point = Unda Kidokezo
common-create-polyline = Unda Mstari wa Pointi Nyingi
common-create-triangulation = Unda Utatuzi...
common-crosses = Misalaba
common-cut = Kata
common-cut-topology-pit-shell = Kata Tofolojia kwa Ganda la Shimo...
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
common-filter = Chuja
common-fly-mode = Hali ya Kuruka
common-generate-contour-lines = Zalisha Mistari ya Mwinuko...
common-hide-all = Ficha Yote
common-hide-selection = Ficha Uteuzi
common-ignore = Puuza
common-import-csv-block-model = Ingiza CSV Mfano wa Vitalu
common-import-dxf = Ingiza DXF
common-incline-design-project = Mradi wa Incline Design
common-layer = Tabaka
common-legend = Kificho
common-line = Mstari
common-line-weight = Uzito wa mstari
common-lock-all = Funga Yote
common-lock-selection = Funga Uteuzi
common-m = m
common-max = Kiwango cha juu
common-merge-shell-into-topology = Unganisha Ganda kwenye Tofolojia
common-merge-shell-into-topology-2 = Unganisha Ganda kwenye Tofolojia...
common-move-collar = Hamisha Kola
common-move-design = Hamisha Muundo
common-move-selection = Hamisha Uteuzi
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
common-text-2 = °
common-tie-holes = Unganisha Mashimo
common-triangulations = Utatuzi
common-trim-topology = Punguza kwa Tofolojia...
common-undo = Tendua
common-undrape-all = Ondoa Ufunikaji Yote
common-uniform-white = Nyeupe sawia
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
confirmations-save-modified-project-browser-storag = Hifadhi mradi ulioboreshwa kwenye hifadhi ya kivinjari kabla ya kutoka?
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

## Drill strings

drill-hole-add-stop = Ongeza kituo
drill-hole-all-rendered-intervals-opaque-white = Vipindi vyote vilivyochorwa ni nyeupe isiyopenyeza.
drill-hole-burden-spacing-must-greater-than = Burden na nafasi lazima ziwe zaidi ya sifuri
drill-hole-choose-valid-closed-polyline = Chagua mstari halali wa pointi nyingi uliofungwa
drill-hole-colour-scale = Kipimo cha rangi
drill-hole-field = Sehemu
drill-hole-grayscale = Kijivu
drill-hole-green-yellow-red = Kijani–Njano–Nyekundu
drill-hole-heat = Joto
drill-hole-no-holes-fit-inside-boundary = Hakuna mashimo yanayotoshea ndani ya mpaka huu kwa burden na nafasi ya sasa
drill-hole-pattern-exceeds-maximum-maximum-hole = Mfumo unazidi kiwango cha juu cha mashimo { $maximum }; ongeza burden au nafasi
drill-hole-preset = Mpangilio
drill-hole-px = px
drill-hole-rainbow = Upinde wa mvua
drill-hole-reset-preset = Rejesha mpangilio
drill-hole-rotation-offsets-must-contain-valid = Mzunguko na mkabala lazima ziwe na nambari sahihi
drill-hole-selected-polyline-has-no-usable = Mstari wa pointi nyingi uliochaguliwa hauna eneo la XY linalotumika
drill-hole-smooth-interpolation = Uingiliano laini
drill-hole-spacing-would-scan-too-many = Nafasi hii ingechunguza seli nyingi mno za gridi; ongeza burden au nafasi (kiwango cha juu mashimo { $maximum })
drill-hole-square = Mraba
drill-hole-staggered = Kwa zamu
drill-hole-stepped-bands = Mikanda yenye ngazi
drill-hole-text = ×
drill-hole-text-2 = −
drill-hole-unsupported-drillhole-source = Chanzo cha shimo la uchimbaji kisichotumika
drill-hole-width = Upana
drill-pattern-arrangement = Mpangilio
drill-pattern-axis-offset = Mkabala wa { $axis }
drill-pattern-blast-shape = Umbo la ulipuaji
drill-pattern-burden = Burden
drill-pattern-choose-closed-blast-boundary-then = Chagua mpaka uliofungwa wa ulipuaji, kisha rekebisha gridi. Mashimo ya uchimbaji husasishwa moja kwa moja kwenye mwonekano.
drill-pattern-closed-design-polyline-whose-xy = Mstari wa muundo wa pointi nyingi uliofungwa ambao eneo lake la XY litajazwa mashimo.
drill-pattern-counter-clockwise-pattern-rotation-f = Mzunguko wa mfumo kinyume cha saa kutoka mhimili wa { $axis } wa dunia.
drill-pattern-distance-between-holes-along-each = Umbali kati ya mashimo kando ya kila safu ya mfumo.
drill-pattern-e-g-west-cut-03 = mfano: West Cut 03
drill-pattern-finished-hole-diameter-entered-milli = Kipenyo cha shimo lililokamilika. Kimewekwa kwa milimita na kuhifadhiwa na kila shimo linalozalishwa.
drill-pattern-hole-depth = Kina cha shimo
drill-pattern-hole-diameter = Kipenyo cha shimo
drill-pattern-move-over-closed-polyline-then = Sogeza juu ya mstari wa pointi nyingi uliofungwa, kisha ubofye kwenye mwonekano. Esc hughairi uchaguzi.
drill-pattern-name-drillhole-dataset-created-proje = Jina la data ya mashimo ya uchimbaji iliyoundwa kwenye mradi.
drill-pattern-none-picked = Hakuna kilichochaguliwa
drill-pattern-pattern-name = Jina la mfumo
drill-pattern-perpendicular-distance-between-patte = Umbali wa pembeni kati ya safu za mfumo.
drill-pattern-pick = Chagua
drill-pattern-preview-count-hole-s-diameter = Hakikisho: mashimo { $count } · kipenyo mm{ $diameter } · kina m{ $depth }
drill-pattern-rotation = Mzunguko
drill-pattern-shift-pattern-grid-along-global = Hamisha gridi ya mfumo kando ya mhimili wa { $axis } wa dunia huku ukiiweka ndani ya umbo la ulipuaji.
drill-pattern-spacing = Nafasi
drill-pattern-staggered-offsets-every-second-row = Kwa zamu hukabalisha kila safu ya pili kwa nusu ya nafasi.
drill-pattern-vertical-depth-below-each-collar = Kina wima chini ya kila kola.

## Dxf strings

dxf-dxf-block-nesting-exceeds-maximum = Uwekaji ndani wa vitalu vya DXF unazidi kina cha juu ({ $depth }), inaruka '{ $name }'
dxf-dxf-circular-block-reference-detecte = Rejeleo la mviringo la kitalu cha DXF limegunduliwa: '{ $name }'
dxf-dxf-entity-referenced-undefined-laye = Kitu cha DXF kimerejelea tabaka lisilofafanuliwa '{ $name }', kimeingizwa kama '{ $fallback }'
dxf-dxf-import-exceeds-what-budget = Uingizaji wa DXF unazidi kikomo cha bajeti ya { $what } ({ $limit }); jiometri iliyobaki imerukwa
dxf-dxf-insert-references-unknown-block = INSERT ya DXF inarejelea kitalu kisichojulikana '{ $name }'

## Edit strings

edit-absolute-length = Urefu kamili
edit-absolute-rl = RL Kamili
edit-action = Kitendo
edit-angle = Pembe
edit-angle-from-horizontal-negative-downw = Pembe kutoka mlalo, hasi chini: -90 ni shimo wima.
edit-app-web-not-recommended-production = { $app } Web haipendekezwi kwa matumizi ya uzalishaji. Itumie kama onyesho tu.
edit-application = Programu
edit-apply = Tumia
edit-apply-pick-target = Tumia na Chagua Lengo
edit-axis-value = thamani ya { $axis }
edit-azimuth = Azimuth
edit-batter-angle = Pembe ya mteremko (°)
edit-bearing-holes-drilled-degrees-clockw = Mwelekeo ambao mashimo yanachimbwa, kwa digrii kwa mwendo wa saa kutoka kaskazini ya gridi.
edit-bench-height = Kimo cha ngazi
edit-benches = Ngazi
edit-berm-width = Upana wa lundo la kinga
edit-bezier-curve = Mviringo wa Bezier
edit-choose-layer = Chagua tabaka
edit-choose-whether-entered-value-distanc = Chagua kama thamani iliyowekwa ni umbali kwenye mteremko, upana wa mlalo, au kimo wima.
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
edit-number-line-segments-used-approximat = Idadi ya sehemu za mstari zinazotumika kukadiria mviringo kati ya vipeo viwili vilivyochaguliwa.
edit-number-straight-segments-used-approx = Idadi ya sehemu za moja kwa moja zinazotumika kukadiria kona iliyoviringika. Tumia 1 kwa kingo iliyopunguzwa iliyonyooka.
edit-object = Kitu
edit-offset-element = Kipengele cha Mkabala
edit-pick-side = Chagua Upande
edit-pit = Shimo
edit-project-name = Jina la mradi
edit-properties = Sifa
edit-radius = Radiasi
edit-recent = Hivi Karibuni
edit-relative = Jamaa (+/-)
edit-relative-applies-vertical-change-eve = Jamaa hutumia mabadiliko ya wima kwa kila kidokezo. RL Kamili huweka kila kidokezo kwenye kimo kimoja lengwa.
edit-remove-from-list = Ondoa kwenye Orodha
edit-replace-path = Badilisha njia
edit-rotate = Zungusha
edit-rotation-speed-slice-when-using = Kasi ya mzunguko wa ukataji unapotumia Q na E.
edit-s = °/s
edit-segments = Sehemu
edit-segments-lying-elevation-ignored = Sehemu zilizo kwenye kimo hiki hazizingatiwi.
edit-select-endpoint-changes-other-endpoi = Chagua ncha inayobadilika; ncha nyingine inabaki thabiti.
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
edit-translation-distance-along-world-axi = Umbali wa mhamisho kando ya mhimili wa { $axis } wa dunia.
edit-type = Aina
edit-type-direction-together-set-offset = Aina na Mwelekeo pamoja huweka upande wa mkabala. Shimo + Juu na Rundo la Hifadhi + Chini husogea nje; Shimo + Chini na Rundo la Hifadhi + Juu husogea ndani.
edit-up-raises-each-bench-bench = Juu huinua kila ngazi kwa kimo cha ngazi; Chini huishusha. Hii pia hubadilisha upande wa mkabala - angalia Aina.
edit-value-interpreted-using-selected-mea = Thamani hufasiriwa kwa kutumia Kipimo na Hali ya Kimo iliyochaguliwa.
edit-vertical-rise-fall-each-bench = Kupanda au kushuka wima kwa kila ngazi kabla lundo linalofuata halijaundwa.
edit-world-x-y-z-coordinates = Kuratibu za X, Y na Z za dunia za kidokezo cha kwanza cha udhibiti cha Bezier.
edit-world-x-y-z-coordinates-2 = Kuratibu za X, Y na Z za dunia za kidokezo cha pili cha udhibiti cha Bezier.

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
explorer-id-raster-id-source-driver =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = Pakia
explorer-lock = Funga
explorer-select-all-objects = Chagua Vitu Vyote
explorer-source-name = Chanzo: { $name }
explorer-unload = Ondoa
explorer-unlock = Fungua

## Files strings

files-automatic-colour = Rangi kiotomatiki
files-automatic-rl-spacing = Nafasi ya RL kiotomatiki
files-axis-scale-ratio = Uwiano wa kipimo wa { $axis }
files-ok = Sawa
files-reset-1 = Rejesha kwa 1×
files-rl-grid-options = Chaguzi za Gridi ya RL
files-rl-spacing = Nafasi ya RL
files-scales-z-distances-visually-without = Hupima umbali wa Z kwa kuonekana bila kubadilisha kuratibu zilizohifadhiwa.
files-thickness = Unene
files-xy-grid-options = Chaguzi za Gridi ya XY

## Gpu strings

gpu-cache-block-model-surface-build-failed = Ujenzi wa uso wa mfano wa vitalu umeshindwa: { $error }
gpu-cache-block-model-surface-build-worker = Kifanyakazi cha ujenzi wa uso wa mfano wa vitalu kimekatika
gpu-cache-block-model-surface-chunk-rejected = Kipande cha uso wa mfano wa vitalu kimekataliwa kabla ya ugawaji wa GPU: instances={ $instances } bytes, limit={ $limit } bytes
gpu-cache-block-volume-preparation-worker-disc = Kifanyakazi cha kuandaa ujazo wa vitalu kimekatika
gpu-cache-translucent-volume-could-not-built = Ujazo wa uwazi haukuweza kujengwa ({ $error }); mfano huu wa vitalu unaonyeshwa kama vijicubu badala yake.
gpu-cache-triangulation-edge-chunk-rejected-be = Kipande cha ukingo wa utatuzi kimekataliwa kabla ya ugawaji wa GPU: instances={ $instances } bytes, limit={ $limit } bytes
gpu-cache-triangulation-gpu-chunk-rejected-bef = Kipande cha GPU cha utatuzi kimekataliwa kabla ya ugawaji: vertices={ $vertices } bytes, indices={ $indices } bytes, limit={ $limit } bytes
gpu-cache-triangulation-name-has-count-vertice = Utatuzi '{ $name }' una vipeo { $count } (> u32::MAX); haiwezekani kugawanya kwa GPU
gpu-cache-triangulation-name-uploaded-chunks-s = Utatuzi '{ $name }' umepakiwa katika vipande { $chunks } vya anga (nyuso { $faces })

## Init strings

init-gpu-adapter-vendor-name-backend = Kibadilishaji cha GPU: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver-driver-driver-info = Kiendeshi cha GPU: { $driver } { $driver_info }
init-gpu-supports-maximum-buffer-size = GPU inatumia ukubwa wa juu wa hifadhi ya { $size } MiB; maeneo makubwa yanaweza yasionekane kikamilifu
init-surface-presentation-mode-mode = Hali ya uwasilishaji wa uso: { $mode }
init-wgpu-error-continuing-error = Hitilafu ya wgpu (inaendelea): { $error }

## Io strings

io-ascii-points-xyz-pts = Vidokezo vya ASCII (.xyz, .pts)
io-attribute = Sifa
io-blank-header = (kichwa tupu)
io-block-model = Mfano wa vitalu:
io-choose-file-purpose-map-its = Chagua kusudi la faili ili kuoanisha safu wima zake.
io-choose-loaded-block-model = Chagua mfano wa vitalu uliopakiwa
io-choose-loaded-layer = Chagua tabaka lililopakiwa
io-choose-loaded-triangulation = Chagua utatuzi uliopakiwa
io-choose-purpose = Chagua kusudi…
io-choose-source-file-files-import = Chagua faili au faili za chanzo za kuingiza.
io-collar = Kola
io-column-mapping = Uoanishaji wa safu wima
io-comma-separated-values-csv = Comma-Separated Values (.csv)
io-csv-files = Faili za CSV
io-default = Chaguo-msingi
io-depth = Kina
io-diameter = Kipenyo
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
io-export-dxf = Hamisha DXF
io-export-one-layer = Hamisha tabaka moja
io-export-ply = Hamisha PLY
io-export-stl = Hamisha STL
io-export-wavefront-obj = Hamisha Wavefront OBJ
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Ingiza
io-import-ascii-point-cloud = Ingiza Wingu la Vidokezo la ASCII
io-import-drillhole-csv-bundle = Ingiza Kifurushi cha CSV cha Mashimo ya Uchimbaji
io-import-geotiff = Ingiza GeoTIFF
io-import-las-laz-point-cloud = Ingiza Wingu la Vidokezo la LAS/LAZ
io-import-pcd-point-cloud = Ingiza Wingu la Vidokezo la PCD
io-import-ply = Ingiza PLY
io-import-stl = Ingiza STL
io-import-wavefront-obj = Ingiza Wavefront OBJ
io-interval = Kipindi
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-mapped-csv-bundle-csv = Kifurushi cha CSV kilichooanishwa (.csv)
io-model-file = Faili ya mfano
io-name-count-files = { $name } + faili { $count }
io-no-csv-chosen = Hakuna .csv iliyochaguliwa
io-no-csv-files-chosen = Hakuna faili za CSV zilizochaguliwa
io-no-dxf-chosen = Hakuna .dxf iliyochaguliwa
io-no-omf-chosen = Hakuna .omf iliyochaguliwa
io-north-y = Kaskazini / Y
io-ply-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-source-file = Faili ya chanzo
io-start-x = Mwanzo X
io-start-y = Mwanzo Y
io-start-z = Mwanzo Z
io-stl-stl = STL (.stl)
io-triangulation = Utatuzi:
io-unmapped = Haijaoanishwa
io-wavefront-obj-obj = Wavefront OBJ (.obj)

## Jobs strings

jobs-background-task-poll-label-ended = Kazi ya nyuma '{ $poll_label }' imeisha bila matokeo
jobs-discarded-stale-background-result-po = Matokeo ya nyuma yaliyopitwa na wakati ya '{ $poll_label }' yameondolewa kwa sababu chanzo kimebadilika au kimefungwa

## Logging strings

logging-activity-completed = Shughuli imekamilika
logging-activity-started = Shughuli imeanza
logging-application-id-id = Kitambulisho cha programu: { $id }
logging-application-name-name = Jina la programu: { $name }
logging-application-startup = Kuanzisha Programu
logging-build-target-os-architecture = Lengo la ujenzi: { $os }-{ $architecture }
logging-completed = Imekamilika
logging-count-messages = Ujumbe { $count }
logging-desktop-session-xdg-session-type = Kipindi cha eneo-kazi: XDG_SESSION_TYPE={ $type }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Inaanzisha Incline Design
logging-locale-environment-lang-lang-lc = Mazingira ya lugha: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session-user-user-shell = Kipindi cha macOS: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = Mfumo wa uendeshaji: GNU / Linux
logging-operating-system-macos = Mfumo wa uendeshaji: macOS
logging-operating-system-microsoft-windows = Mfumo wa uendeshaji: Microsoft Windows
logging-pointer-width-width-bit = Upana wa kielekezi: { $width }-bit
logging-process-id-id = Kitambulisho cha mchakato: { $id }
logging-release-version-version = Toleo la matoleo: { $version }
logging-renderer = Kichoraji
logging-rust-compiler-host-host = Mwenyeji wa kikusanyaji cha Rust: { $host }
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
object-edit-length-length-m = Urefu m{ $length }
object-edit-move-down = Hamisha chini
object-edit-move-up = Hamisha juu
object-edit-object-has-no-arc-segments = Kitu hiki hakina sehemu za mviringo.
object-edit-object-has-single-position = Kitu hiki kina nafasi moja.
object-edit-object-needs-least-required-vertices = Kitu hiki kinahitaji angalau vipeo { $required }
object-edit-one-more-properties-not-valid = Sifa moja au zaidi si nambari sahihi
object-edit-perimeter-length-m-area-area = Mzingo m{ $length }, eneo m²{ $area }
object-edit-reverse = Geuza
object-edit-row-row-position-bulge-not = Safu { $row }: nafasi au uvimbe si nambari sahihi
object-edit-sweep = Mfagio
object-edit-text-not-number = "{ $text }" si nambari
object-edit-vertices = Vipeo

## Omf strings

omf-element-name-has-count-tie = Kipengele '{ $name }' kina viunganishi { $count } vinavyotaja mashimo ambayo hakina tena
omf-ignoring-colour-map-omf-attribute = Inapuuza ramani ya rangi kwenye sifa ya OMF '{ $attribute }': { $error }
omf-mining-data-exported-incline = Data ya madini iliyohamishwa na Incline
omf-omf-import = Uingizaji wa OMF
omf-omf-texture = Umbile la OMF
omf-omf-validation-warnings-warnings = Maonyo ya uthibitishaji wa OMF: { $warnings }
omf-project-application-metadata-applica = Metadata ya programu ya mradi '{ $application }' haihifadhiwi
omf-project-author-not-retained = Mwandishi wa mradi hahifadhiwi
omf-project-description-not-retained = Maelezo ya mradi hayahifadhiwi
omf-project-has-unsupported-metadata-key = Mradi una funguo za metadata zisizotumika: { $keys }

## Plot strings

plot-1-1000-one-millimetre-sheet = Kwa 1:1000, milimita moja kwenye karatasi ni mita moja ardhini.
plot-1-scale-covers-width-height = 1:{ $scale } · inafunika { $width } × { $height } m
plot-all-visible-data = Data zote zinazoonekana
plot-automatic-grid-interval = Muda wa gridi kiotomatiki
plot-border = Mpaka
plot-centre = Katikati kwenye
plot-choose-smallest-conventional-scale-f = Chagua kipimo kidogo kinachokubalika kinachotosheleza kila kitu kinachoonekana kwenye karatasi.
plot-coordinate-grid = Gridi ya kuratibu
plot-current-view-centre = Kitovu cha mwonekano wa sasa
plot-date = TAREHE
plot-date-2 = Tarehe
plot-dots-per-inch-paper-size = Nukta kwa inchi. Ukubwa huu wa karatasi unaweza kubadilishwa kuwa rasta hadi dpi { $max_dpi }; dpi 300 ni ubora wa kawaida wa uchapishaji.
plot-dpi = dpi
plot-drawing-no = NAMBARI YA MCHORO
plot-drawing-number = Nambari ya mchoro
plot-drawn = IMECHORWA NA
plot-drawn-2 = Imechorwa na
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
plot-plot-scale-must-positive-number = Kipimo cha mchoro lazima kiwe nambari chanya
plot-png-written-sheet-s-exact = PNG imeandikwa kwa ukubwa halisi wa karatasi ya sheet na kurekodi dpi yake, hivyo huchapisha kwa kipimo halisi.
plot-portrait = Wima
plot-resolution = Ubora
plot-rev = MAREKEBISHO
plot-revision = Marekebisho
plot-scale = KIPIMO
plot-scale-1 = Kipimo  1:
plot-scale-framing = Kipimo na uwekaji fremu
plot-sheet-furniture = Mapambo ya karatasi
plot-size-width-height-mm = { $size } ({ $width } × { $height } mm)
plot-subtitle = Kichwa kidogo
plot-title = Kichwa
plot-title-block = Kizuizi cha kichwa
plot-today = leo

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
properties-dark-mode = Hali ya giza
properties-developer = Msanidi
properties-downscale-rasters = Punguza ukubwa wa rasta
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
properties-orbit-sensitivity = Uelekevu wa mzunguko
properties-panel-chrome = Mapambo ya paneli
properties-performance = Utendaji
properties-plan-mode = Hali ya Mpango
properties-presents-step-display-no-tearing = Huonyesha kwa mpangilio wa skrini: hakuna uraruaji, na skrini huweka kasi ya fremu. Ikizimwa, fremu huonyeshwa mara zinapochorwa na kikomo hapa chini kinatumika.
properties-reflective-block-edges = Kingo za vitalu zenye mng'ao
properties-restore-defaults-2 = Rejesha Chaguo-msingi
properties-show-console = Onyesha dashibodi
properties-shows-live-near-far-projection = Huonyesha umbali wa makadirio wa karibu na mbali wa moja kwa moja kwenye pau ya hali.
properties-snap-polling = Ukaguzi wa kubandika
properties-vertical-sync = Usawazishaji wima
properties-world-axis-gizmo = Kifaa cha mhimili wa dunia
properties-zoom-cursor = Kuza kwa kishale
properties-zoom-sensitivity = Uelekevu wa kukuza

## Screenshot strings

screenshot-could-not-encode-viewport-image = Imeshindwa kusimba picha ya mwonekano: { $error }
screenshot-could-not-map-viewport-screenshot = Imeshindwa kuoanisha picha ya skrini ya mwonekano: { $error }
screenshot-could-not-save-viewport-image = Imeshindwa kuhifadhi picha ya mwonekano { $path }: { $error }
screenshot-downloaded-viewport-image-file-name = Picha ya mwonekano imepakuliwa: { $file_name }
screenshot-saved-viewport-image-path = Picha ya mwonekano imehifadhiwa: { $path }
screenshot-viewport-image-download-failed-error = Upakuaji wa picha ya mwonekano umeshindwa: { $error }

## Spatial strings

spatial-bvh-face-index-index-out = Kielezo cha uso { $index } cha BVH kiko nje ya wigo kwa mfumo; kinabadilishwa na pembetatu iliyoharibika

## State strings

state-above = kwenye au juu ya
state-activate-project = Amilisha Mradi
state-all-open-incline-design-data = Data yote iliyofunguliwa ya Incline Design
state-apply-generated-rings = Tumia miduara iliyozalishwa
state-apply-selection = Tumia kwa uteuzi
state-azimuth-azimuth-dip-dip = kwa azimuth { $azimuth }°, mteremko { $dip }°
state-azimuth-azimuth-dip-dip-2 = hadi azimuth { $azimuth }°, mteremko { $dip }°
state-below = kwenye au chini ya
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
state-copy-objects-layer = Nakili Vitu kwenye Tabaka
state-count-file-s = faili { $count }
state-count-object-s-axis-value = vitu { $count } · { $axis } { $value }
state-count-object-s-closed = vitu { $count } · { $closed }
state-count-object-s-layer = vitu { $count } · { $layer }
state-count-object-s-weight = vitu { $count } · { $weight }
state-count-object-s-z-elevation = vitu { $count } · Z { $elevation }
state-create-point-cloud-tin = Unda TIN ya Wingu la Vidokezo
state-create-project = Unda Mradi
state-current-project = Mradi wa sasa
state-cut-topology-pit-shell = Kata Tofolojia kwa Ganda la Shimo
state-cut-triangulation-polyline = Kata Utatuzi kwa Mstari wa Pointi Nyingi
state-cut-triangulation-z = Kata Utatuzi kwa Z
state-dark-mode = Hali ya Giza
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
state-export-layer-dxf = Hamisha Tabaka kwenda DXF
state-export-omf = Hamisha OMF
state-export-project-dxf = Hamisha Mradi kwenda DXF
state-export-triangulation = Hamisha Utatuzi
state-export-viewport-image = Hamisha Picha ya Mwonekano
state-finish-closed-polyline = Maliza mstari wa pointi nyingi uliofungwa
state-finish-open-polyline = Maliza mstari wa pointi nyingi ulio wazi
state-fit-extents = Oanisha na wigo
state-fix-release-centre-both-views = Funga au achilia kitovu ambacho mionekano yote miwili inazunguka
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
state-major-major-minor-minor = Kuu { $major } · ndogo { $minor }
state-move-axis-value = Hamishia kwa Thamani ya Mhimili
state-move-objects-layer = Hamisha Vitu kwenye Tabaka
state-name-count-holes = { $name } · mashimo { $count }
state-name-count-object-s = { $name } · vitu { $count }
state-name-z-min-z-max = { $name } · { $z_min } hadi { $z_max }
state-next-edit = Hariri inayofuata
state-north = Kaskazini
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
state-section-section = sehemu { $section }
state-select-layer-objects = Chagua Vitu vya Tabaka
state-selected-objects = Vitu vilivyochaguliwa
state-selected-polylines = Mistari ya pointi nyingi iliyochaguliwa
state-selected-scene-elements = Vipengele vya eneo vilivyochaguliwa
state-set-block-model-variable = Weka Kigezo cha Mfano wa Vitalu
state-set-drillhole-colour-preset = Weka Mpangilio wa Rangi za Mashimo ya Uchimbaji
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
status-frame-rate = Kasi ya fremu

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
tie-in-tied-count-connector-s-delay = Viunganishi { $count } vimeunganishwa kwa ms { $delay } na { $product }
tie-in-tied-count-connector-s-delay-2 = Viunganishi { $count } vimeunganishwa kwa ms { $delay } na { $product }, vikibadilisha { $replaced }

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

tri-adaptive-concentrates-vertices-compl = Kinachojirekebisha hukusanya vipeo kwenye ardhi ngumu kupitia hitilafu ya kuoanisha ndege; sawia huvisambaza kwa usawa. Mbinu zaidi zinaweza kuongezwa baadaye.
tri-adaptive-quadtree = Kinachojirekebisha (quadtree)
tri-axis-range = Wigo wa { $axis }
tri-base-topology-will-receive-pit = Tofolojia ya msingi itakayopokea umbo la shimo au rundo la hifadhi.
tri-boundary-polyline = Mstari wa pointi nyingi wa mpaka
tri-bridge-gaps-boundary-concavities-nar = Unganisha mianya na mabonde ya mpaka membamba zaidi ya hii kwenye uso. 0 bado huunganisha mianya hadi kiasi cha ukubwa wa seli ya sampuli; thamani kubwa hujaza mashimo makubwa na kuondoa mabonde ya mpaka.
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
tri-hide-unload-sources = Ficha na ondoa vyanzo
tri-higher-edge-will-enforced-each = Ukingo wa juu utatumika katika kila mgongano. Sehemu za chini zenye mgongano hazitazingatiwa kama mistari ya mvunjiko na uso utaingiliana kupitia maeneo hayo. Mistari ya chanzo ya pointi nyingi haibadiliki.
tri-highlighted-breakline-edges-cross-ov = Kingo za mstari wa mvunjiko zilizoangaziwa zinavuka au kupishana kwenye mpango kwenye vimo tofauti. Uso mmoja wa ardhi hauwezi kufuata zote mbili.
tri-intervals-colours = Vipindi na rangi
tri-keep-clipped-topology-included-shape = Weka tofolojia iliyokatwa na umbo lililojumuishwa kama utatuzi tofauti badala ya kuviunganisha kuwa kitu kimoja.
tri-keep-inside-discards-surface-outside = Weka ndani huondoa uso ulio nje ya mstari wa pointi nyingi. Weka nje hukata shimo lenye umbo la mstari wa pointi nyingi kutoka kwenye uso.
tri-keeps-only-surface-within-polyline = Huweka uso ulio ndani ya mpaka wa mstari wa pointi nyingi pekee.
tri-keeps-surface-relation-topology-with = Huweka uso { $relation } tofolojia ndani ya wigo wake wa XY.
tri-layer-already-exists-select-above = Tabaka hilo tayari lipo; lichague hapo juu au chagua jina lingine.
tri-limit-z-range = Punguza wigo wa Z
tri-major = Kuu
tri-max-edge-length = Urefu wa juu wa ukingo
tri-merge = Unganisha
tri-method = Mbinu
tri-min = Kima cha chini
tri-minimum-maximum-elevations-retained = Vimo vya chini na vya juu vitakavyobaki kwenye uso wa matokeo. Kima cha chini lazima kiwe chini ya kima cha juu.
tri-minor = Ndogo
tri-minor-controls-ordinary-contours-maj = Ndogo hudhibiti mistari ya kawaida ya mwinuko. Kuu hudhibiti mistari iliyosisitizwa na lazima itumie muda usiopungua ule wa Ndogo.
tri-move-cursor-over-loaded-surface = Sogeza kishale juu ya uso uliopakiwa.
tri-name-assigned-elevation-clipped-outp = Jina litakalopewa uso wa matokeo uliokatwa kwa kimo.
tri-name-assigned-merged-topology-pit = Jina litakalopewa matokeo ya tofolojia iliyounganishwa na shimo/rundo la hifadhi.
tri-name-assigned-newly-created-contour = Jina litakalopewa tabaka jipya la mistari ya mwinuko.
tri-name-assigned-reconstructed-triangul = Jina litakalopewa utatuzi uliojengwa upya.
tri-name-assigned-topology-after-pit = Jina litakalopewa tofolojia baada ya ganda la shimo kukatwa kutoka humo.
tri-name-assigned-trimmed-output-surface = Jina litakalopewa uso wa matokeo uliopunguzwa.
tri-nearby-breakline-vertices-do-not = Vipeo vya karibu vya mstari wa mvunjiko havikutani katika sehemu moja kamili, hivyo uso hauwezi kutatuliwa.
tri-new-layer = Tabaka jipya
tri-new-layer-name = Jina jipya la tabaka
tri-once-merge-succeeds-unload-source = Baada ya kuunganishwa kufanikiwa, ondoa tofolojia ya chanzo na kigumu ili matokeo yaliyounganishwa pekee yabaki kwenye eneo.
tri-only-loaded-triangulations-can-picke = Utatuzi uliopakiwa tu ndio unaweza kuchaguliwa.
tri-operation = Operesheni
tri-output-layer = Tabaka la matokeo
tri-percentage = Asilimia
tri-percentage-cloud = Asilimia ya wingu
tri-pick-from-view = Chagua kutoka kwa Mwonekano
tri-pit-design-surface-only-areas = Uso wa muundo wa shimo. Maeneo tu ambapo unachimba chini ya tofolojia ndiyo yanayotumika kwa ukataji.
tri-pit-shell = Ganda la shimo
tri-pit-stockpile-solid = Kigumu cha shimo/rundo la hifadhi
tri-recommended-weld-retry = Inapendekezwa: Unga na Ujaribu Tena
tri-reconstruct-triangulated-terrain-sur = Jenga upya uso wa ardhi uliotatuliwa kutoka kwa wingu la vidokezo. Kipima kinachojirekebisha hutumia bajeti ya vipeo mahali ardhi ilipo ngumu zaidi na kuacha maeneo tambarare na vipeo vichache.
tri-reduce-budget-candidate-detail-if = Punguza bajeti au maelezo ya mgombea ikiwa kompyuta yako ina RAM kidogo.
tri-reference-topology-defines-where-oth = Tofolojia ya rejeleo inayoelezea mahali uso mwingine unapopunguzwa.
tri-reject-reconstructed-triangle-edges = Kataa kingo za pembetatu zilizojengwa upya zenye urefu zaidi ya umbali huu. Tumia 0 kwa kutokuwa na kikomo cha urefu wa ukingo.
tri-removes-surface-within-polyline-boun = Huondoa uso ulio ndani ya mpaka wa mstari wa pointi nyingi na kubaki na uliobaki.
tri-removes-topology-where-pit-shell = Huondoa tofolojia mahali ganda la shimo linapochimba chini yake ili ganda lijaze shimo hilo. Mshono hufuata mstari halisi wa 3D wa mgusano kati ya nyuso; tofolojia iliyo chini ya sehemu za ganda zinazosimama juu ya ardhi inabaki.
tri-result = Matokeo
tri-save-two-entities = Hifadhi kama vitu viwili
tri-select = Chagua…
tri-share-source-points-keep-fractions = Sehemu ya vidokezo vya chanzo vya kubaki. Sehemu kama 0.125% zinaruhusiwa.
tri-slice-triangulation-z-range = Kata Utatuzi kwa Wigo wa Z
tri-solution-generate-upper-surface = Suluhisho: Zalisha Uso wa Juu
tri-surface-trim = Uso wa Kupunguza
tri-surface-will-changed-selected-topolo = Uso utakaobadilishwa; tofolojia iliyochaguliwa inabaki kama ilivyo.
tri-text = %
tri-topology = Tofolojia
tri-triangulation-failed = Utatuzi Umeshindwa
tri-trim = Punguza
tri-trim-topology = Punguza kwa Tofolojia
tri-uniform-grid = Gridi sawia
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
ui-strike-strike-dip = mwelekeo { $strike }° · { $dip }
ui-value-dip = mteremko { $value }°

## Viewport strings

viewport-all-total-categories-keep-their = Kategoria zote { $total } zinabaki na rangi yake; za kwanza { $shown } pekee ndizo zinazochorwa kwa kutofautisha
viewport-axis-maximum = kima cha juu cha { $axis }
viewport-axis-minimum = kima cha chini cha { $axis }
viewport-bar-blast-timeline-placeholder = Ratiba ya Ulipuaji [NAFASI YA MUDA]
viewport-bar-burden-relief-heatmap-placeholder = Ramani-joto ya Uwiano wa Mzigo [NAFASI YA MUDA]
viewport-bar-color = Rangi:
viewport-bar-contours-equal-time-placeholder = Mistari ya Mwinuko Sawa kwa Wakati [NAFASI YA MUDA]
viewport-bar-disable-flying-mode = Zima Hali ya Kuruka
viewport-bar-disable-x-ray-vision = Zima Mwono wa X-Ray
viewport-bar-drill-holes = Mashimo ya Uchimbaji:
viewport-bar-enable-flying-mode = Washa Hali ya Kuruka
viewport-bar-enable-x-ray-vision = Washa Mwono wa X-Ray
viewport-bar-exit-slice-view = Toka kwenye Mwonekano wa Ukataji
viewport-bar-fill = Jaza:
viewport-bar-fix-centre-rotation = Funga Kitovu cha Mzunguko
viewport-bar-hide-points = Ficha Vidokezo
viewport-bar-hide-rl-grid = Ficha Gridi ya RL
viewport-bar-hide-wireframes = Ficha Nyaya
viewport-bar-hide-xy-grid = Ficha Gridi ya XY
viewport-bar-release-centre-rotation = Achilia Kitovu cha Mzunguko
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
viewport-double-click-add-boundary-here = Bofya mara mbili ili kuongeza mpaka hapa
viewport-drag-move-middle-click-toggles = Buruta kuhamisha · Bofya kati hubadilisha ≤
viewport-drag-move-right-click-remove = Buruta kuhamisha · Bofya kulia kuondoa · Bofya kati hubadilisha ≤
viewport-e = E
viewport-edit-category-colour = Hariri rangi ya kategoria hii
viewport-edit-colour-used-empty-values = Hariri rangi inayotumika kwa thamani tupu
viewport-empty = (tupu)
viewport-empty-hidden = (tupu · imefichwa)
viewport-filter-variables = Chuja vigezo
viewport-middle-drag-pan-scroll-zoom = Buruta kwa kitufe cha kati kusogeza · Sogeza gurudumu kukuza
viewport-middle-drag-pan-scroll-zoom-2 = Buruta kwa kitufe cha kati kusogeza · Sogeza gurudumu kukuza · Bofya kutenganisha
viewport-n = N
viewport-no-data-variable = Hakuna data kwa kigezo hiki
viewport-no-matches = Hakuna vinavyolingana
viewport-no-usable-range = (hakuna wigo unaotumika)
viewport-rebuild-variable-s-colours-from = Jenga upya rangi za kigezo hiki kutoka kwa data yake
viewport-reset = Rejesha
viewport-restore-full-model-range = Rejesha wigo kamili wa mfano
