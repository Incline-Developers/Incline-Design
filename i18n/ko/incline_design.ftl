# 한국어 — Incline 메시지 카탈로그.
#
# 이 파일은 불완전할 수 있으며, 누락된 항목은 영문 카탈로그(`i18n/en/incline_design.ftl`)로
# 대체됩니다.
#
# `=` 왼쪽의 id와 `{ $... }` 형태의 인자 이름은 절대 변경하지 마세요.

## Shared

common-cancel = 취소
common-clear = 지우기
common-close = 닫기
common-fill = 채우기
common-set = 설정

## Status bar

# 상태 표시줄의 언어 메뉴 제목. 언어 이름 자체는 번역하지 않습니다.
status-language = 언어

## Menu bar — File

menu-file = 파일
menu-file-save-project = 프로젝트 저장
menu-file-save-project-as = 다른 이름으로 프로젝트 저장...
menu-file-new-project = 새 프로젝트...
menu-file-open-project = 프로젝트 열기...
menu-file-open-recent = 최근 항목 열기
menu-file-show-in-explorer = 탐색기에서 표시
menu-file-show-in-folder = 폴더 열기
menu-file-import = 가져오기...
menu-file-export = 내보내기...
menu-file-export-viewport-image = 뷰포트 이미지 내보내기...
menu-file-export-engineering-drawing = 도면 내보내기...
menu-file-about = { $app } 정보...
menu-file-exit = 애플리케이션 종료

## Menu bar — View

menu-view = 보기

## Workspaces

ws-production = 생산
ws-drill-and-blast = 천공 및 발파
ws-geology = 지질
ws-planning = 계획

## Menubars

ws-menubar-design = 설계
ws-menubar-triangulation = 삼각망
ws-menubar-raster = 래스터
ws-menubar-point-cloud = 포인트 클라우드
ws-menubar-block-model = 블록 모델
ws-menubar-drillholes = 시추공
ws-menubar-modelling = 모델링
ws-menubar-active-layer = 레이어:

## Menubars functions

ws-menubar-design-insert-point = 점 삽입
ws-menubar-design-insert-point-at-intersection = 교차점에서
ws-menubar-design-insert-point-at-elevation = 표고에서
ws-menubar-design-move-to = 이동
ws-menubar-design-create-triangulation = 삼각망 생성

## Rename / delete item dialogs

# { $kind } is a workspace noun from the ws-production-* set above.
dialog-rename-title = { $kind } 이름 바꾸기
dialog-rename-field = 새 이름
dialog-rename-field-hint = 필수
dialog-rename-submit = 이름 바꾸기
dialog-delete-title = { $kind } 삭제
dialog-delete-confirm =
    프로젝트에서 '{ $name }'을(를) 삭제하시겠습니까?
    이 작업은 되돌릴 수 없습니다.
confirm-delete-product =
    팔레트에서 제품 '{ $name }'을(를) 삭제하시겠습니까?
    이 작업은 되돌릴 수 없습니다.

## Create Triangulation dialog

tri-create-title = 삼각망 생성
tri-create-help = 이 대화상자를 열 때 선택한 객체를 삼각화합니다. 선택을 변경하려면 대화상자를 닫으세요.
tri-create-type-label = 삼각망 유형
tri-create-type-help =
    개방형 표면은 지형 형태의 표면을 생성합니다. 솔리드는 완전히 밀폐된
    메쉬를 생성하며, 수밀 경계를 이룰 수 있는 입력이 필요합니다.
tri-create-output-name = 출력 이름
tri-create-output-name-help = 생성된 삼각망에 지정할 이름입니다.
tri-create-output-name-hint = 삼각망 이름
tri-create-run = 삼각망 생성
tri-selection-none = 선택한 객체를 더 이상 사용할 수 없습니다.

tri-selection-selected = { $summary } 선택됨

tri-type-open-surface = 표면
tri-type-solid-closed = 솔리드

# Selection summary pieces, e.g. "3 polylines, 1 point". 한국어는 CLDR 복수
# 카테고리가 하나뿐이므로 *[other] 분기만 사용합니다.
tri-count-polylines =
    { $count ->
       *[other] 폴리라인 { $count }개
    }
tri-count-strings =
    { $count ->
       *[other] 선 { $count }개
    }
tri-count-circles =
    { $count ->
       *[other] 원 { $count }개
    }
tri-count-points =
    { $count ->
       *[other] 점 { $count }개
    }
tri-count-texts =
    { $count ->
       *[other] 텍스트 객체 { $count }개
    }
tri-count-objects =
    { $count ->
       *[other] 객체 { $count }개
    }

about-read-full-licence = 전체 라이선스 읽기 ↗
about-source-code = 소스 코드
about-website = 웹사이트
about-title = { $app } 정보
drill-hole-colour-stop = 색상 단계 { $index }
properties-restore-defaults-tooltip = { $heading } 설정을 기본값으로 재설정

## Dynamic UI messages

ui-selected-count = { $count }개 선택됨
ui-selected-objects = 객체 { $count }개 선택됨
ui-selected-polylines = 폴리라인 { $count }개 선택됨
ui-invalid-axis-value = 올바른 { $axis } 값을 입력하세요.
ui-selection-spans = 선택 범위: { $min } ~ { $max }.
confirm-delete-count = 선택한 항목 { $count }개를 삭제하시겠습니까?
confirm-delete-layer =
    레이어 '{ $name }' 및 그 안의 모든 객체를 삭제하시겠습니까?
    이 작업은 되돌릴 수 없습니다.
plot-preview-pixels = { $width } × { $height } px, { $dpi } dpi
tri-estimated-memory = 예상 최대 메모리 사용량 ~{ $estimate }. { $detail }
block-grid-summary = 그리드: { $x } × { $y } × { $z } = 블록 { $count }개
status-selected = 선택됨: { $count }
status-faces = 면: { $drawn } / { $total } (청크 { $drawn_chunks }/{ $total_chunks })
status-clip = 클립 근/원/Δ: { $near } / { $far } / { $delta } m
status-points = 점: { $drawn } / { $target } (전체 { $total }, 청크 { $drawn_chunks }/{ $total_chunks })

explorer-no-rasters = 래스터 없음
slice-viewport-gestures = 가운데 버튼 드래그로 이동 · 오른쪽 버튼 드래그로 궤도 회전 · Shift+휠로 걷기 · W/S로 슬래브 이동 · Q/E로 회전 · Esc로 종료

## Startup environment details

## Renderer startup diagnostics

color-aci = ACI
color-aci-value = ACI { $index }
color-index = 인덱스
color-rgb = RGB
color-opacity = 불투명도
color-edit = 클릭하여 색상 편집
color-saturation-value = 채도 및 명도
color-hue = 색조
asset-loading = 애셋 데이터 불러오는 중
asset-unloading = 애셋 데이터 언로드하는 중
asset-load-failed = 애셋 데이터를 불러올 수 없습니다
asset-unload-failed = 애셋 데이터를 언로드할 수 없습니다
preferences-title = 환경설정
context-text-colour = 텍스트 색상
context-polylines = 폴리라인
context-points = 점
crs-unknown-ellipsoid = 이 좌표계 정의에서 인식할 수 없는 지구 모델 "{ $name }"입니다.
crs-no-ellipsoid = 이 좌표계 정의에는 사용하는 지구 모델이 지정되어 있지 않습니다.
crs-unknown-code = EPSG:{ $code }는 좌표계 등록부에 없습니다.
crs-transform-failed = 좌표를 변환하지 못했습니다. 결과가 유한한 위치 값이 아닙니다.
crs-no-datum-path = { $from }와(과) { $to }의 기준계 사이에는 공개된 변환 경로가 없습니다(EPSG 데이텀 { $source }, { $target }). 그대로 변환하면 알 수 없는 만큼 오차가 발생하므로 아무것도 변경하지 않았습니다.
crs-unknown-datum = { $from } 또는 { $to }의 기준계를 식별할 수 없으며, 두 좌표계는 서로 다른 지구 모델을 사용합니다. 이 둘 사이를 변환하면 알 수 없는 만큼 오차가 발생합니다.
ws-survey = 측량
survey-count-designs =
    { $count ->
       *[other] 설계 { $count }개
    }
survey-count-meshes =
    { $count ->
       *[other] 삼각망 { $count }개
    }
survey-count-models =
    { $count ->
       *[other] 블록 모델 { $count }개
    }
survey-count-clouds =
    { $count ->
       *[other] 포인트 클라우드 { $count }개
    }
survey-count-holes =
    { $count ->
       *[other] 시추공 데이터셋 { $count }개
    }
survey-count-rasters =
    { $count ->
       *[other] 래스터 { $count }개
    }
survey-angle = Z축 기준 회전(반시계 방향)
survey-scale = 균일 XYZ 축척 비율
survey-invalid-transform = 원점, 각도 및 결과 좌표는 모두 유한한 값이어야 합니다.
survey-invalid-scale = 축척은 유한한 양수여야 하며, 그 역수도 유한해야 합니다.
survey-empty-selection = 변환하려면 지원되는 항목을 하나 이상 선택하세요.
survey-unavailable = 선택한 항목이 없거나 언로드되었습니다. 변환하기 전에 불러오세요.
survey-wrong-project = 활성 프로젝트의 설계만 선택하세요.
survey-name-required = 좌표계 이름을 입력하세요.
survey-working = 선택한 데이터를 변환하는 중…
survey-completed = { $items }을(를) 제자리에서 변환했습니다. 실행 취소로 되돌릴 수 있습니다.
survey-failed = 변환에 실패했습니다: { $error }
survey-stale = 활성 프로젝트나 원본 데이터가 변경되어 변환이 취소되었습니다. 원본 데이터를 다시 선택한 후 시도하세요.
survey-coordinates-menu = 좌표
survey-definitions-action = 정의…
survey-transform-action = 변환…
survey-definitions-title = 좌표 정의
survey-transform-title = 좌표 변환
survey-new-system = 새 좌표계
survey-new-system-name = 좌표계
survey-set-local = 광산 좌표계로 설정
survey-delete-system = 좌표계 삭제
survey-systems-empty = 좌표계 없음
survey-system-name = 이름
survey-system-origin = 동일한 점 — 이 좌표계 좌표
survey-angle-help = 위에서 내려다볼 때 기준 X축에서 기준 Y축 방향으로의 반시계 방향 각도입니다.
survey-scale-help = 기준계에서 이 좌표계로의 균일한 XYZ 축척입니다. 치수를 그대로 유지하려면 1을 사용하세요.
survey-close = 닫기
survey-from = 원본
survey-to = 대상
survey-transform-button = 변환
survey-swap = 바꾸기
survey-drape-note = 투영된 이미지는 변환된 표면에서 제거되며 다시 투영해야 합니다.
survey-needs-grid-block-model = 블록 모델은 규칙적인 셀 그리드이며, 투영법이나 기준계를 변경하면 이 규칙성이 유지되지 않습니다. 변환하려면 모든 셀을 새 그리드로 리샘플링해야 하고 그 과정에서 값이 손실되므로 변경하지 않았습니다.
survey-needs-grid-raster = 래스터는 아핀 변환으로 세계 좌표에 배치되며, 투영법이나 기준계를 변경하면 이를 유지할 수 없습니다. 변환하려면 이미지를 리샘플링해야 하므로 변경하지 않았습니다.
survey-conversion-exact = 정확함: 그리드만 변경되며 재투영은 하지 않습니다.
survey-conversion-accuracy = 명시된 정확도 { $accuracy }m입니다.
survey-kind = 종류
survey-axis-names = 축 이름
survey-kind-registry-short = 등록부 좌표계
survey-kind-grid-short = 다른 좌표계 위의 그리드
survey-registry-search = 검색
survey-registry-hint = 이름 또는 EPSG 코드(예: "mga zone 56")
survey-registry-none = 모든 단어와 일치하는 항목이 등록부에 없습니다.
survey-parent = 정의 기준
survey-parent-origin = 알려진 점 — 상위 좌표계 좌표
survey-pick-registry = 좌표계를 검색하여 결과에서 선택하세요.
survey-pick-parent = 이 그리드의 정의 기준이 되는 좌표계를 선택하세요.
survey-pick-system = 좌표계 선택
survey-pick-systems = 변환할 원본 좌표계와 대상 좌표계를 선택하세요.
survey-no-selection = 왼쪽에서 좌표계를 선택하거나 우클릭하여 추가하세요.
survey-kind-grid = { $parent } 위의 그리드
survey-system-in-use =
    "{ $name }"을(를) 삭제할 수 없습니다: 이를 기준으로 정의된 항목 { $dependants }개가 { $dependants ->
       *[other] 있습니다
    }. 먼저 다른 곳을 참조하도록 변경하세요.
survey-system-cycle = "{ $name }"이(가) 직접 또는 상위 좌표계를 통해 자기 자신을 기준으로 정의되어 있습니다.
survey-system-missing = 해당 좌표계가 더 이상 존재하지 않습니다. 다른 정의를 선택하세요.
survey-same-system = 서로 다른 원본 좌표계와 대상 좌표계를 선택하세요.
survey-name-exists = 해당 이름의 좌표계가 이미 있습니다. 편집하려면 선택하거나 다른 이름을 사용하세요.
preferences-ui-size = UI 크기
preferences-ui-size-help = 기기의 기본 디스플레이 배율을 기준으로 텍스트와 컨트롤 크기를 조정합니다. 100%는 기본 크기입니다. 화면 해상도와 창 크기에 따라 인터페이스가 작아지지 않습니다.
relimit-select-boundary = 재한계할 폴리라인 또는 원 선택
relimit-click-boundary = 교차할 폴리라인 또는 원을 클릭하세요…
relimit-mode-help = 교차는 한쪽 끝점을 폴리라인 또는 원으로 이동합니다. 절대값은 최종 선 길이를 설정합니다. 상대값은 길이를 더하거나 뺍니다.
browser-graphics-device-lost = 브라우저가 그래픽 장치를 잃었습니다. 이 페이지를 새 탭에서 다시 여세요. GPU 세부 정보: { $message }

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins, and Incline Design contributors. 이 소프트웨어의 사본을 취득한 모든 사람은 MIT 라이선스 조건에 따라 제한 없이 이를 다룰 수 있는 권한을 무료로 부여받습니다.

    Incline Design은 상품성, 특정 목적 적합성, 비침해에 대한 보증을 포함하되 이에 국한되지 않는 어떠한 종류의 명시적 또는 묵시적 보증 없이 "있는 그대로" 제공됩니다.
about-free-open-source-mine-design = 무료 오픈소스 광산 설계
about-licensed-under-mit-license = MIT 라이선스에 따라 라이선스가 부여됩니다

## App strings

app-activated-browser-project-name = 브라우저 프로젝트 '{ $name }'을(를) 활성화했습니다.
app-browser-project-delete-failed = 브라우저 프로젝트 삭제 실패: { $error }
app-browser-project-no-longer-exists = 해당 브라우저 프로젝트가 더 이상 존재하지 않습니다
app-browser-save-failed-error = 브라우저 저장 실패: { $error }
app-could-not-activate-browser-project = 브라우저 프로젝트를 활성화할 수 없습니다: { $error }
app-could-not-delete-browser-project = 브라우저 프로젝트를 삭제할 수 없습니다: { $error }
app-could-not-load-browser-project = 브라우저 프로젝트를 불러올 수 없습니다: { $error }
app-could-not-restore-browser-project = 브라우저 프로젝트를 복원할 수 없습니다: { $error }
app-deleted-browser-project = 브라우저 프로젝트를 삭제했습니다
app-failed-create-window-error = 창을 생성하지 못했습니다: { $error }
app-failed-create-window-icon-error = 창 아이콘을 생성하지 못했습니다: { $error }
app-failed-detach-top-down-preview = 평면도 미리보기를 분리하지 못했습니다: { $error }
app-failed-initialize-graphics-error = 그래픽을 초기화하지 못했습니다: { $error }
app-browser-preferences-load-failed = 브라우저 환경설정을 불러오지 못했습니다: { $error }
app-failed-load-config-file-error = 구성 파일을 불러오지 못했습니다: { $error }
app-failed-load-session-file-error = 세션 파일을 불러오지 못했습니다: { $error }
app-failed-rasterize-window-icon-error = 창 아이콘을 래스터화하지 못했습니다: { $error }
app-failed-save-browser-session-error = 브라우저 세션을 저장하지 못했습니다: { $error }
app-failed-save-session-error = 세션을 저장하지 못했습니다: { $error }
app-saved-name-browser-storage = '{ $name }'을(를) 브라우저 저장소에 저장했습니다

## Block strings

block-model-between = 사이
block-model-block-grid = 블록 그리드
block-model-block-size = 블록 크기
block-model-choose-numeric-variable = 숫자형 변수 선택
block-model-choose-numeric-variables = 숫자형 변수 선택
block-model-count-variables-selected = { $count }개 변수 선택됨
block-model-estimate-variables = 추정 변수
block-model-full-x-y-z-dimensions = 각 블록의 전체 X, Y, Z 치수입니다. 블록이 작을수록 세부 표현력, 계산 시간, 메모리 사용량이 늘어납니다.
block-model-grid-bounds-block-sizes-invalid = 그리드 경계 또는 블록 크기가 올바르지 않습니다.
block-model-lower-x-y-z-edges = 블록 모델 볼륨의 X, Y, Z 하한 경계입니다. 블록 중심은 이 경계에서 블록 절반만큼 안쪽에서 시작합니다.
block-model-maximum = 최대값
block-model-maximum-nearest-samples-used-each = 각 블록에 사용되는 최대 최근접 샘플 수입니다. 값이 낮을수록 빠르게 실행되고, 값이 높을수록 추정값이 매끄러워지지만 계산 시간이 늘어납니다.
block-model-maximum-samples = 최대 샘플 수
block-model-minimum = 최소값
block-model-min-samples-help = 블록을 추정하는 데 필요한 최소 인접 샘플 수입니다. 검색 반경 내 샘플이 이보다 적은 블록은 비워 둡니다.
block-model-minimum-samples = 최소 샘플 수
block-model-no-block-model-selected = 선택한 블록 모델 없음
block-model-no-drill-holes-selected = 선택한 시추공 없음
block-model-nugget = 너깃 효과
block-model-numeric-interval-fields-interpolate = 보간할 숫자형 구간 필드입니다. 선택한 각 필드는 하나의 블록 모델 변수가 됩니다.
block-model-kriging-help = 정규 크리깅은 구형 배리오그램을 사용하여 각 블록 중심에서 숫자형 시추공 구간 값을 추정합니다.
block-model-partial-sill = 부분 시일
block-model-range-search-radius = 범위 / 검색 반경
block-model-range-help = 이 거리보다 먼 샘플은 제외되며, 이 범위에서 공분산이 0에 도달합니다.
block-model-select-all = 모두 선택
block-model-selected-block-model-whose-blocks = 블록을 임계값으로 나누어 솔리드로 만들 선택한 블록 모델입니다. 다른 모델을 사용하려면 대화상자를 닫으세요.
block-model-source-drill-holes-help = 숫자형 구간을 블록으로 추정할 선택한 시추공 컬렉션입니다. 다른 컬렉션에서 추정하려면 대화상자를 닫으세요.
block-model-sill-help = 구형 모델이 기여하는 공간적으로 상관된 분산입니다. 너깃 효과와 함께 거리 0에서의 공분산을 결정합니다.
block-model-spherical-variogram-search = 구형 배리오그램 및 검색
block-model-threshold-at-most = <= 임계값
block-model-threshold-at-least = >= 임계값
block-model-threshold-min = 임계값 / 최소값
block-model-upper-x-y-z-extent = 덮을 X, Y, Z 상한 범위입니다. 범위가 블록 크기의 정확한 배수가 아닌 경우 마지막 블록이 이 범위를 넘어갈 수 있습니다.
block-model-variable = 변수
block-model-variance-effectively-zero-separation = 측정 오차 또는 샘플링 규모 이하의 변동으로 인한 사실상 거리 0에서의 분산입니다. 너깃 효과가 필요 없으면 0을 사용하세요.
block-model-volume-feedback-disconnected = 블록 볼륨 사용량 피드백 리드백 연결이 끊어졌습니다
block-model-volume-feedback-failed = 블록 볼륨 사용량 피드백 리드백 실패: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z
borehole-inspector-add-every-code = 나열되지 않은 코드 모두 추가
borehole-inspector-add-to-column = 추가
borehole-inspector-check = 검증
borehole-inspector-check-accept = 수락
borehole-inspector-check-column = 검증
borehole-inspector-check-column-changed = 마지막 검증 이후 층서 열이 바뀌었습니다. 어떤 시추공이 층서 열과 맞지 않는지 보려면 다시 검증하세요.
borehole-inspector-check-column-hint = 대부분의 시추공이 이 코드들에 부여한 순서를 구하고, 이와 맞지 않는 시추공을 나열합니다. 비어 있는 층서 열은 채워지고, 다른 순서가 있는 경우에는 수락할 때만 바뀝니다.
borehole-inspector-check-differences-note = 대부분의 시추공이 부여한 순서를 층서 열 옆에 표시합니다. 수락하면 층서 열이 이 순서로 설정되며 실행 취소는 한 단계입니다.
borehole-inspector-check-flagged = 검증: 시추공 { $count }개 표시됨
borehole-inspector-check-moved = 이동됨
borehole-inspector-check-not-run = 아직 검증하지 않았습니다.
borehole-inspector-check-now = 현재
borehole-inspector-check-order-differs = 대부분의 시추공이 이 코드들에 부여한 순서가 층서 열과 다릅니다.
borehole-inspector-check-overruled = 약한 다수 { $count }개가 더 강한 다수에 의해 기각되었습니다.
borehole-inspector-check-place = 순위
borehole-inspector-check-proposed = 제안
borehole-inspector-check-show-differences = 차이 표시
borehole-inspector-check-stale = 마지막 검증 이후 시추공이 바뀌었습니다. 다시 검증하세요.
borehole-inspector-check-summary = 시추공 { $holes }개를 읽었으며, { $flagged }개가 층서 열과 맞지 않습니다.
borehole-inspector-check-too-many-codes = 이 필드에는 순서를 매기기에 코드가 너무 많습니다.
borehole-inspector-checking-linked-geophysics-file = 연결된 물리탐사 파일을 확인하는 중...
borehole-inspector-close-inspector = 검사기 닫기
borehole-inspector-code-not-in-set = 층서 열에는 있지만 이 세트의 어떤 구간에서도 사용되지 않음
borehole-inspector-column = 층서 열
borehole-inspector-column-empty-check = 아직 층서 열이 없습니다. 검증을 눌러 대부분의 시추공이 부여한 순서로 채우거나, 아래에서 코드를 추가하고 직접 순서를 정하세요.
borehole-inspector-data = 데이터
borehole-inspector-display = 표시
borehole-inspector-every-code-placed = 이 필드의 모든 코드가 층서 열에 들어 있습니다.
borehole-inspector-flag-of-groups = { $kind }(그룹)
borehole-inspector-flag-out-of-place = 위치 어긋남
borehole-inspector-flag-overturned = 역전됨
borehole-inspector-flag-repeat = 반복됨
borehole-inspector-flagged-holes = 표시된 시추공 ({ $count })
borehole-inspector-flags-first-shown = 표시 { $count }개 중 처음 { $shown }개를 나열했습니다.
borehole-inspector-file-not-where-was-linked = { $file }이(가) 연결된 위치에 없습니다.
borehole-inspector-guessed-name = 이름으로 추정됨
borehole-inspector-hold-hole-while-you-work = 주변 시추공을 작업하는 동안 이 시추공을 고정합니다.
borehole-inspector-holding-hole-click-follow-selection = 이 시추공을 고정 중입니다. 클릭하면 다시 선택을 따라갑니다.
borehole-inspector-log = 검층
borehole-inspector-inspect-hole = 검사
borehole-inspector-no-holes-flagged = 층서 열과 맞지 않는 시추공이 없습니다.
borehole-inspector-no-categorical-field = 이 세트에는 순서를 매길 범주형 필드가 없습니다.
borehole-inspector-no-hole-inspected = 검사 중인 시추공 없음
borehole-inspector-not-in-column = 층서 열에 없음 ({ $count })
borehole-inspector-pick-file = { $file } 선택...
borehole-inspector-pick-file-again-show-its = 물리탐사 데이터를 표시하려면 { $file }을(를) 다시 선택하세요. 브라우저 페이지는 파일을 스스로 다시 열 수 없습니다.
borehole-inspector-place-codes-note = 데이터에는 있지만 층서 열에 아직 나열되지 않은 코드입니다. 추가한 코드는 맨 아래에 놓이므로 제자리로 옮기세요.
borehole-inspector-reading-geophysics-file-its-index = 인덱스를 위해 물리탐사 파일을 읽는 중입니다. 진행률은 상태 표시줄에 표시됩니다.
borehole-inspector-remove-from-column = 층서 열에서 제거
borehole-inspector-strat = 층서
borehole-inspector-strat-column = 층서 열
borehole-inspector-summary = 요약
canvas-circle-summary = 원 | 레이어: { $layer } | 반지름 { $radius }

## Canvas strings

canvas-not-selectable-closed-polyline = 선택할 수 없음 | 닫힌 폴리라인을 선택하세요
canvas-polyline-summary = 폴리라인 | 레이어: { $layer } | 정점 { $count }개
canvas-surface-name = 표면 | { $name }
canvas-trimmed = 트리밍됨
cinematic-shadows-method = 시네마틱 뷰 그림자: { $method }

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = 객체 { $object_id }로부터 사면-소단을 생성했습니다
cmd-bezier-replaced-polyline-span-first-last = 폴리라인 구간 { $first }→{ $last }을(를) 샘플링된 중간점 { $count }개로 교체했습니다
cmd-bezier-vertices-first-last = 정점 { $first }부터 { $last }까지
cmd-block-model-block-model-loader-disconnected-path = { $path }의 블록 모델 로더 연결이 끊어졌습니다
cmd-block-model-block-model-path-has-count = 블록 모델 { $path }에 읽을 수 없는 지원되지 않는 유형의 변수 { $count }개가 있습니다: { $names }
cmd-block-model-building-ore-mesh = 광체 메쉬 생성 중…
cmd-block-model-could-not-create-block-model = 블록 모델을 생성할 수 없습니다: { $error }
cmd-block-model-could-not-decode-block-model = 블록 모델 색상 변수 '{ $variable }'을(를) 디코딩할 수 없습니다: { $error }
cmd-block-model-created-block-model-name-ordinary = 정규 크리깅으로 블록 모델 '{ $name }'을(를) 생성했습니다
cmd-block-model-failed-load-block-model-error = 블록 모델을 불러오지 못했습니다: { $error }
cmd-block-model-generated-ore-mesh-from-block = 블록 모델 '{ $name }'에서 광체 메쉬를 생성했습니다
cmd-block-model-imported-block-model-source-path = 블록 모델 원본 { $path }을(를) 가져왔습니다
cmd-block-model-loaded-block-model-name-blocks = 블록 모델 '{ $name }'을(를) 불러왔습니다: 블록 { $blocks }개(렌더링 가능 { $renderable }개), 그리드 { $dimx }x{ $dimy }x{ $dimz }, 변수 { $variables }개
cmd-block-model-loading-name = { $name } 불러오는 중
cmd-block-model-loading-name-ellipsis = { $name }을(를) 불러오는 중…
cmd-chamfer-applied = 모서리 { $corner }을(를) 반지름 { $radius }, 세그먼트 { $segments }개로 모따기했습니다
cmd-chamfer-radius = 반지름 { $radius }
cmd-commands-clipped = 클립됨
cmd-commands-command-failed-error = 명령 실패: { $error }
cmd-commands-count-control-string-s = 제어선 { $count }개
cmd-commands-count-control-string-s-layer = '{ $layer }'의 제어선 { $count }개
cmd-commands-count-point-s-across-layers = 레이어 { $layers }개에 걸친 점 { $count }개
cmd-commands-count-point-s-layer = '{ $layer }'의 점 { $count }개
cmd-commands-kind-layer = '{ $layer }'의 { $kind }
cmd-commands-no-control-strings = 제어선 없음
cmd-commands-no-extent = 범위 없음
cmd-commands-select-holes-place-reference-points = 기준점을 배치할 시추공을 선택하세요
cmd-triangulate-needs-selection = 삼각망 생성을 실행하기 전에 삼각화할 객체를 선택하세요
cmd-commands-select-one-loaded-block-model = 광체 삼각망을 생성하기 전에 로드된 블록 모델을 하나 선택하세요
cmd-commands-select-one-loaded-drill-hole = 블록 모델을 생성하기 전에 로드된 시추공 컬렉션을 하나 선택하세요
cmd-commands-select-one-loaded-point-cloud = 삼각망을 생성하기 전에 로드된 포인트 클라우드를 하나 선택하세요
cmd-contours-needs-triangulation = 등고선을 생성하기 전에 로드된 삼각망을 하나 선택하세요
cmd-slice-needs-triangulation = Z 범위로 슬라이스하기 전에 로드된 삼각망을 하나 선택하세요
cmd-commands-select-one-loaded-triangulation-one = 클립하기 전에 로드된 삼각망 하나와 닫힌 폴리라인 하나를 선택하세요
cmd-commands-select-one-more-objects-before = { $axis }을(를) 설정하기 전에 객체를 하나 이상 선택하세요
cmd-commands-sliced = 슬라이스됨
cmd-commands-modelling-settings-set-settings = 모델링 설정을 변경했습니다. { $settings }
cmd-contours-contour-generation-failed-error = 등고선 생성 실패: { $error }
cmd-contours-discarded-layer-exists = '{ $name }'의 등고선이 취소되었습니다: 레이어 '{ $layer_name }'이(가) 이제 존재합니다
cmd-contours-discarded-project-closed = '{ $name }'의 등고선이 취소되었습니다: 프로젝트가 닫혔습니다
cmd-contours-discarded-layer-deleted = '{ $name }'의 등고선이 취소되었습니다: 선택한 출력 레이어가 삭제되었습니다
cmd-contours-generated = 레이어 '{ $layer_name }'에 삼각망 '{ $name }'의 등고선 폴리라인 { $line_count }개를 생성했습니다
cmd-creation-assembled-boundary-rings = 조각난 열린 선들로부터 닫힌 경계 링 { $assembled_count }개를 조립했습니다
cmd-creation-created-triangulation-from-boundary = 경계 링 { $boundary_count }개와 열린 제약 { $constraint_count }개로부터 삼각망을 생성했습니다(표면 유형: { $surface_type })
cmd-creation-creating-triangulation = 삼각망 생성 중…
cmd-creation-generate-upper-surface-ignored-count = 상부 표면 생성: 충돌하는 하부 브레이크라인 세그먼트 { $count }개를 무시했습니다. 원본 객체는 변경되지 않습니다
cmd-creation-ignored-objects = 삼각화 중 폴리라인이 아니거나 퇴화된 객체 { $rejected }개를 무시했습니다
cmd-creation-weld-retry-moved-coarse-welded = 결합 후 재시도: 정점 { $coarse_welded }개를 공유 위치로 이동했습니다(최대 { $coarse_weld_tol } m). 원본 객체는 변경되지 않습니다
cmd-creation-welded-breakline-vertices = 허용 오차 내에서 일치하는 브레이크라인 정점 { $welded }개를 결합했습니다
cmd-cuts-clipped-surface-name-polyline-mode = 폴리라인으로 표면 '{ $name }'을(를) 클립했습니다({ $mode })
cmd-cuts-clipping-surface-polyline = 폴리라인으로 표면을 클립하는 중…
cmd-cuts-cut-topology-name-pit-shell = 지형면 '{ $name }'을(를) 채굴장 쉘로 절단했습니다
cmd-cuts-cut-triangulation-name-z-band = 삼각망 '{ $name }'을(를) Z 밴드 [{ $min }, { $max }]로 절단했습니다
cmd-cuts-cutting-topology-pit-shell = 채굴장 쉘로 지형면 절단 중…
cmd-cuts-cutting-triangulation-z = Z값으로 삼각망 절단 중…
cmd-cuts-ignored-vertical-faces = XY 면적이 없는 수직 또는 퇴화된 기준 지형면 { $count }개를 무시했습니다
cmd-cuts-site-skipped-constraint-from-x = { $site }: 삼각화기가 분할할 수 없는 제약 ({ $from_x }, { $from_y }) → ({ $to_x }, { $to_y })을(를) 건너뛰었습니다
cmd-cuts-skipped-degenerate-edges = { $site }: 거의 퇴화된 제약 모서리 { $skipped }개를 건너뛰었습니다. 근처에서 절단 경계가 미세하게 어긋날 수 있습니다
cmd-cuts-trimmed-surface = 표면 '{ $surface }'을(를) 지형면 '{ $topology }'에 맞춰 트리밍했습니다({ $mode })
cmd-cuts-trimming-surface-topology = 지형면에 맞춰 표면 트리밍 중…
cmd-drape-draped-intersected-vertices-changed = 정점 { $intersected }개를 투영했으며, 그중 { $changed }개의 표고가 변경되었습니다
cmd-drape-no-intersections = 선택한 설계 정점 중 선택한 지형면과 교차하는 것이 없습니다
cmd-drape-objects-changed-object-s-changed = 객체 { $objects }개 변경됨 · 교차 정점 { $intersected }개 중 { $changed }개 이동됨
cmd-drape-select-one-more-design-objects = 투영할 설계 객체를 하나 이상 선택하세요
cmd-drape-select-one-more-topologies-drape = 투영 대상 지형면을 하나 이상 선택하세요
cmd-drape-selected-topologies-no-longer-loaded = 선택한 지형면이 더 이상 로드되어 있지 않습니다
cmd-drill-hole-choose-drillhole-source-files-again = 시추공 원본 파일을 다시 선택하세요
cmd-drill-hole-drill-pattern-too-large-contains = 천공 패턴이 너무 크거나 유효하지 않은 공구 좌표를 포함하고 있습니다
cmd-drill-hole-drillhole-field-label-has-count = 시추공 필드 '{ $label }'에는 고유 코드가 { $count }개 있어 일반적인 코드 필드보다 많습니다. 범주형 필드가 아니라 자유 텍스트로 보이지만, 모든 코드를 유지하고 색상을 지정했습니다
cmd-drill-hole-enter-name-drill-pattern = 천공 패턴의 이름을 입력하세요
cmd-drill-hole-failed-load-drillholes-error = 시추공을 불러오지 못했습니다: { $error }
cmd-drill-hole-depth-must-be-positive = 공심은 0보다 커야 합니다
cmd-drill-hole-diameter-must-be-positive = 공경은 0보다 커야 합니다
cmd-drill-hole-loaded-drillhole-dataset-name-holes = 시추공 데이터 세트 '{ $name }'을(를) 불러왔습니다: 시추공 { $holes }개, 색상 필드 { $fields }개
cmd-drill-hole-field-has-no-strat-column = { $field }에 층서 열이 없어 이름을 이동하지 않았습니다
cmd-drill-hole-name-already-loading = '{ $name }'을(를) 이미 불러오는 중입니다
cmd-drill-hole-name-reason = '{ $name }': { $reason }
cmd-drill-hole-name-side = { $name } { $side }
cmd-drill-hole-name-working-section-side = { $name } 작업 구간 { $side }
cmd-drill-hole-no-hole-holds-value-field = 해당 필드에 '{ $value }' 값을 가진 시추공이 없습니다
cmd-drill-hole-names-shifted-down = { $hole }의 이름을 시추공 아래 방향으로 이동했습니다: 이동 { $moved }개, UNK로 지정 { $unknown }개, 층서 열에 없어 그대로 둠 { $untouched }개
cmd-drill-hole-names-shifted-up = { $hole }의 이름을 시추공 위 방향으로 이동했습니다: 이동 { $moved }개, UNK로 지정 { $unknown }개, 층서 열에 없어 그대로 둠 { $untouched }개
cmd-drill-hole-no-interval-holds-seam = { $name }을(를) 가진 구간이 더 이상 없어 이름을 바꾸지 않았습니다
cmd-drill-hole-only-mapped-csv-bundles-imported = 브라우저에서는 매핑된 CSV 번들만 가져올 수 있습니다
cmd-drill-hole-pattern-contains-no-holes = 패턴에 발파공이 없습니다
cmd-drill-hole-no-interval-names-column-code = { $hole }에는 층서 열의 코드를 이름으로 가진 구간이 없습니다. 층서 열에 없어 그대로 둠 { $untouched }개
cmd-drill-hole-reading-name = { $name } 읽는 중
cmd-drill-hole-reference-points-used-holes-placed = 기준점: 시추공 { $used }개 배치, '{ $value }' 없음 { $absent }개, 단층 반복 가능성 표시 { $flagged }개
cmd-drill-hole-seam-renamed = { $from }을(를) { $to }(으)로 이름을 바꿨습니다. 제안된 정정 기록: { $count }개
cmd-drill-hole-uppermost-run-used-flagged-holes = 최상단 구간 사용, 표시된 시추공: { $holes }
cmd-drill-hole-working-section-name-not-same = 작업 구간 '{ $name }'이(가) 선택한 모든 데이터 세트에서 동일하지 않습니다. 각 데이터 세트 자체의 구간을 사용했습니다.
cmd-drill-hole-working-sections-not-kept-dataset = '{ $dataset }'에 작업 구간을 유지하지 않았습니다. { $reasons }
cmd-explode-count-line-s = 선 { $count }개
cmd-explode-polyline = 폴리라인 분해
cmd-explode-exploded-polyline-into-count-line = 폴리라인을 선분 { $count }개로 분해했습니다
cmd-file-block-model-csv-encoding-failed = 블록 모델 CSV 인코딩 실패: { $error }
cmd-file-block-model-csv-export-failed = 블록 모델 CSV 내보내기 실패: { $error }
cmd-file-browser-recovery-unavailable = 브라우저 복구 파일을 사용할 수 없습니다. 저장된 프로젝트는 IndexedDB에 남아 있습니다
cmd-file-closed-project-runtime-id-runtime = 런타임 ID { $runtime_id }의 프로젝트를 닫았습니다
cmd-file-could-not-create-new-project = 새 프로젝트를 생성할 수 없습니다: { $error }
cmd-file-could-not-finish-pending-project = 대기 중인 프로젝트 작업을 완료할 수 없습니다: { $error }
cmd-file-could-not-finish-saving-before = 종료 전 저장을 완료할 수 없습니다: { $error }
cmd-file-could-not-open-browser-project = 브라우저 프로젝트를 열 수 없습니다: { $error }
cmd-file-could-not-open-path-error = { $path }을(를) 열 수 없습니다: { $error }
cmd-file-could-not-read-selected-file = 선택한 파일을 읽을 수 없습니다: { $error }
cmd-file-could-not-reload-layer-from = 디스크에서 레이어를 다시 불러올 수 없습니다: { $error }
cmd-file-could-not-reload-project-from = 디스크에서 프로젝트를 다시 불러올 수 없습니다: { $error }
cmd-file-could-not-remove-browser-project = 브라우저 프로젝트를 제거할 수 없습니다: { $error }
cmd-file-could-not-restore-layer-from = 프로젝트에서 레이어를 복원할 수 없습니다: { $error }
cmd-file-could-not-snapshot-dirty-project = 복구를 위해 수정된 프로젝트의 스냅샷을 만들 수 없습니다: { $error }
cmd-file-could-not-start-browser-export = 브라우저 내보내기를 시작할 수 없습니다: { $error }
cmd-file-could-not-write-recovery-copies = 복구 사본을 기록할 수 없습니다: { $error }
cmd-file-created-new-browser-project = 새 브라우저 프로젝트를 생성했습니다
cmd-file-created-new-project = 새 프로젝트를 생성했습니다
cmd-file-description-download-failed-error = { $description } 다운로드 실패: { $error }
cmd-file-discard-cancelled-project-changed = OMF를 다시 불러오는 동안 프로젝트가 변경되어 취소가 중단되었습니다
cmd-file-discarded-changes-layer-target-name = 레이어 '{ $target_name }'의 변경 사항을 취소했습니다
cmd-file-discarded-changes-reloaded-path = 변경 사항을 취소했습니다: { $path }을(를) 다시 불러왔습니다
cmd-file-downhole-geophysics-csv = 공내 물리탐사 CSV
cmd-file-downloaded-description-file-name = { $description }을(를) 다운로드했습니다: { $file_name }
cmd-file-drillhole-csv-export-failed-error = 시추공 CSV 내보내기 실패: { $error }
cmd-file-dxf-download-encoding-failed-error = DXF 다운로드 인코딩 실패: { $error }
cmd-file-dxf-import-failed-error = DXF 가져오기 실패: { $error }
cmd-file-encoding-block-model-csv-download = 블록 모델 CSV 다운로드 인코딩 중…
cmd-file-encoding-dxf-download = DXF 다운로드 인코딩 중…
cmd-file-encoding-triangulation-download = 삼각망 다운로드 인코딩 중…
cmd-file-exit-deferred-exports = 백그라운드 내보내기가 완료될 때까지 종료가 지연됩니다
cmd-file-exit-requested-no-unsaved-changes = 저장되지 않은 변경 사항 없이 종료가 요청되었습니다
cmd-file-exported-block-model-csv-path = 블록 모델 CSV를 { $path }로 내보냈습니다
cmd-file-exported-description-dxf-path = { $description }을(를) DXF로 내보냈습니다: { $path }
cmd-file-exported-three-drillhole-csvs-path = 시추공 CSV 3개를 { $path }로 내보냈습니다
cmd-file-exported-triangulation-name-path = 삼각망 '{ $name }'을(를) { $path }로 내보냈습니다
cmd-file-exporting-name = { $name }을(를) 내보내는 중…
cmd-file-exporting-triangulation-name-path = 삼각망 '{ $name }'을(를) { $path }로 내보내는 중입니다
cmd-file-fatal-renderer-failure-reason = 심각한 렌더러 오류: { $reason }
cmd-file-dialog-action-failed = 파일 대화상자 작업이 실패했습니다: { $msg }
cmd-file-imported-added-object-s-from = { $name }에서 객체 { $added }개를 가져왔습니다
cmd-file-imported-total-dxf-object-s = DXF 객체 { $total }개를 가져왔습니다
cmd-file-layer-discard-was-cancelled-because = 프로젝트를 다시 불러오는 동안 프로젝트가 변경되어 레이어 변경 취소가 중단되었습니다
cmd-file-no-recovery-directory = 사용 가능한 복구 디렉터리가 없습니다: { $error }
cmd-file-no-unsaved-project-content-nothing = 저장되지 않은 프로젝트 내용이 없습니다. 복구할 항목 없음
cmd-file-parsing-browser-dxf-import = 브라우저 DXF 가져오기 분석 중…
cmd-file-parsing-dxf-import = DXF 가져오기 분석 중…
cmd-file-project-closes-after-save = 현재 저장이 완료된 후 프로젝트가 닫힙니다
cmd-file-the-project-closes-after-save = 현재 저장이 완료된 후 프로젝트가 닫힙니다
cmd-file-queued-count-triangulation-file-s = 삼각망 파일 { $count }개를 가져오기 대기열에 추가했습니다
cmd-file-recovery-copies-path-reopen-them = 복구 사본이 { $path }에 있습니다. 다시 시작한 후 열어보세요
cmd-file-recovery-copy-failed-error = 복구 사본 작성 실패: { $error }
cmd-file-recovery-copy-failed-failure = 복구 사본 작성 실패: { $failure }
cmd-file-recovery-copy-written-path = 복구 사본을 기록했습니다: { $path }
cmd-file-reverting-layer = 레이어 되돌리는 중…
cmd-file-reverting-project = 프로젝트 되돌리는 중…
cmd-file-save-failed-message = 저장 실패: { $message }
cmd-file-save-project-already-running-save = 이 프로젝트를 이미 저장하는 중입니다. 완료된 후 다시 저장하세요
cmd-file-save-worker-ended-without-result = 저장 작업 스레드가 결과 없이 종료되었습니다
cmd-file-saved-project-as = 다음 이름으로 프로젝트를 저장했습니다: { $path }
cmd-file-saved-project = 프로젝트를 저장했습니다: { $path }
cmd-file-saving-browser-storage = 브라우저 저장소에 저장하는 중…
cmd-file-selected-block-model-no-longer = 선택한 블록 모델이 더 이상 로드되어 있지 않습니다
cmd-file-selected-drillhole-dataset-no-longer = 선택한 시추공 데이터 세트가 더 이상 로드되어 있지 않습니다
cmd-file-switching-project = 프로젝트 전환 중…
cmd-file-triangulation-download-encoding-failed = 삼각망 다운로드 인코딩 실패: { $error }
cmd-file-user-chose-exit-without-saving = 사용자가 저장하지 않고 종료를 선택했습니다
cmd-file-user-requested-exit-project-export = 사용자가 종료를 요청했습니다(프로젝트 내보내기 또는 저장되지 않은 작업 확인 필요)
cmd-file-viewport = 뷰포트
cmd-file-wait-current-project-save-finish = 현재 프로젝트 저장이 완료될 때까지 기다리세요
cmd-file-wait-current-project-switch-finish = 현재 프로젝트 전환이 완료될 때까지 기다리세요
cmd-file-wait-project-operation-finish-before = 변경 사항을 취소하기 전에 프로젝트 작업이 완료될 때까지 기다리세요
cmd-file-wait-project-revert-finish-before = 저장하기 전에 프로젝트 되돌리기가 완료될 때까지 기다리세요
cmd-folder-collection-named-name-already-exists = '{ $name }' 컬렉션이 이미 있습니다
cmd-folder-collection-no-longer-exists = 해당 컬렉션이 더 이상 존재하지 않습니다
cmd-folder-created-collection-name = 컬렉션 '{ $name }'을(를) 생성했습니다
cmd-folder-deleted-collection-name = 컬렉션 '{ $name }'을(를) 삭제했습니다
cmd-folder-moved-item-into-collection-name = 항목을 컬렉션 '{ $name }'(으)로 이동했습니다
cmd-folder-moved-item-root-section = 항목을 { $section }의 최상위로 이동했습니다
cmd-folder-renamed-collection-before-after = 컬렉션 '{ $before }'을(를) '{ $after }'(으)로 이름을 변경했습니다
cmd-folder-section-cannot-hold-item = 해당 섹션에는 이 항목을 둘 수 없습니다
cmd-fuse-closed-polyline = 닫힌 폴리라인
cmd-fuse-count-source-line-s = 원본 선 { $count }개
cmd-fuse-created-shape-object-id-vertices = 원본 선 { $sources }개로부터 정점 { $vertices }개를 가진 { $shape } { $object_id }을(를) 생성했습니다
cmd-fuse-click-missed = 결합: 클릭이 어떤 객체도 맞히지 못했습니다 (커서 아래에 객체 없음)
cmd-fuse-click-not-near-endpoint = 결합: 클릭 위치가 선택한 선의 어느 끝점에도 충분히 가깝지 않습니다
cmd-fuse-clicked-closed-polyline = 결합: 클릭한 객체 { $object_id }은(는) 닫힌 폴리라인입니다. 결합은 열린 폴리라인에서만 작동합니다
cmd-fuse-clicked-not-open-polyline = 결합: 클릭한 객체 { $object_id }은(는) 열린 폴리라인이 아닙니다({ $kind }임)
cmd-fuse-clicked-object-missing = 결합: 클릭한 객체 { $object_id }이(가) 더 이상 존재하지 않습니다
cmd-fuse-clicked-too-few-vertices = 결합: 클릭한 폴리라인 { $object_id }에는 정점이 { $count }개뿐입니다. 최소 2개가 필요합니다
cmd-fuse-endpoint-marker-missing = 결합: 끝점 마커 { $marker_index }이(가) 더 이상 존재하지 않습니다
cmd-fuse-close-needs-three-vertices = 결합: 폴리라인으로 닫으려면 서로 다른 정점이 최소 3개 필요합니다(현재 { $count }개)
cmd-fuse-lines = 선 결합
cmd-fuse-needs-two-segments = 결합: 커밋하려면 세그먼트가 최소 2개 필요합니다(현재 { $count }개)
cmd-fuse-no-active-layer = 결합: 결합된 선을 배치할 활성 레이어가 없습니다
cmd-fuse-no-active-project = 결합: 활성 프로젝트가 없어 커밋할 수 없습니다
cmd-fuse-no-source-line = 결합: 폴리라인으로 닫을 원본 선이 없습니다
cmd-fuse-awaiting-object-invalid = 결합: 객체 { $awaiting_id }은(는) 더 이상 유효한 폴리라인이 아닙니다
cmd-fuse-object-already-in-chain = 결합: 객체 { $object_id }은(는) 이미 결합 체인에 포함되어 있습니다. 다른 선을 클릭하세요
cmd-fuse-result-too-few-vertices = 결합: 결과의 정점이 너무 적어({ $count }개) 중단합니다
cmd-fuse-segment-object-invalid = 결합: 세그먼트 객체 { $object_id }은(는) 더 이상 유효한 폴리라인이 아니어서 중단합니다
cmd-fuse-source-object-invalid = 결합: 원본 객체 { $object_id }은(는) 더 이상 유효한 열린 폴리라인이 아닙니다
cmd-fuse-source-object-missing = 결합: 원본 객체 { $object_id }이(가) 더 이상 존재하지 않습니다
cmd-fuse-open-polyline = 열린 폴리라인
cmd-include-failed = 포함 작업 실패: { $message }
cmd-include-included-solid-shape-name-topology = 솔리드 '{ $shape_name }'을(를) 지형면 '{ $topology_name }'에 포함했습니다(지형면 { $retained }개 유지, 마감 면 { $skipped }개 건너뜀)
cmd-include-including-pit-stockpile-solid = 채굴장/적치장 솔리드 포함 중…
cmd-insert-point-count-operation-point-s = { $operation } 점 { $count }개
cmd-insert-point-elevation-must-be-finite = 표고에 점 삽입은 유한한 표고 값이 필요합니다
cmd-insert-point-insert-points = 점 삽입
cmd-insert-point-inserted-count-operation-point-s = { $operation } 점 { $count }개를 삽입했습니다
cmd-insert-point-intersection = 교차점
cmd-insert-point-no-new-operation-points-were = 새로운 { $operation } 점을 찾지 못했습니다
cmd-insert-point-select-least-two-polylines-before = 교차점을 삽입하기 전에 폴리라인을 최소 두 개 선택하세요
cmd-insert-point-select-one-more-polylines-before = 표고에 점을 삽입하기 전에 폴리라인을 하나 이상 선택하세요
cmd-layer-created-layer-name = 레이어 '{ $name }'을(를) 생성했습니다
cmd-layer-deleted-with-objects = 레이어 { $layer_id }(및 그 안의 모든 객체)를 삭제했습니다
cmd-layer-duplicated-layer-duplicate-name = 레이어 '{ $duplicate_name }'을(를) 복제했습니다
cmd-layer-locked = 잠김
cmd-layer-name-copy = { $name } 사본
cmd-layer-selected-count-object-s-layer = 레이어 { $layer_id }에서 객체 { $count }개를 선택했습니다
cmd-layer-state-layer-name = 레이어 '{ $name }' { $state }
cmd-layer-unlocked = 잠금 해제됨
cmd-move-tool-moved-collars = 이동량({ $delta })을 시추공 공구 { $count }개에 적용했습니다
cmd-move-tool-moved-objects = 이동량({ $delta })을 객체 { $count }개에 적용했습니다
cmd-move-tool-count-hole-s = 발파공 { $count }개
cmd-object-edit-edited-kind = 편집된 { $kind }
cmd-object-edit-edited-kind-count-vertices = 편집된 { $kind }(정점 { $count }개)
cmd-object-edit-no-changes-apply = 적용할 변경 사항이 없습니다
cmd-object-edit-object-changed-since-editor-opened = 편집기를 연 이후 이 객체가 변경되었습니다. 최신 버전을 편집하려면 다시 여세요
cmd-object-edit-target-changed = 편집 대상 객체가 변경되어 편집 내용을 버립니다
cmd-object-edit-object-no-longer-exists-document = 해당 객체가 문서에 더 이상 존재하지 않습니다
cmd-object-edit-select-single-design-object-edit = 편집할 설계 객체를 하나만 선택하세요
cmd-object-edit-unassigned = 미지정
cmd-offset-create-offset = 오프셋 생성
cmd-offset-created-offset-count-object-s = 객체 { $count }개의 오프셋을 생성했습니다
cmd-offset-distance-must-be-positive = 오프셋 거리는 0보다 커야 합니다
cmd-offset-skipped-count-circle-s-offset = 원 { $count }개를 건너뛰었습니다. 오프셋 거리가 반지름보다 큽니다
cmd-omf-could-not-open-project-source = 프로젝트 { $source_name }을(를) 열 수 없습니다: { $error }
cmd-omf-create-open-project-before-merging = 데이터를 병합하기 전에 프로젝트를 생성하거나 여세요
cmd-omf-dataset-name-count-working-section = 데이터 세트 '{ $name }': 작업 구간 { $count }개를 복원할 수 없습니다: { $details }
cmd-omf-field-codes-partly-coloured = 데이터 세트 '{ $name }': 필드 '{ $field }'은(는) 전체 코드 { $total }개 중 { $saved }개의 색상만 저장되었고, 나머지는 자동 생성된 색상이 지정되었습니다.
cmd-omf-encoding-project = 프로젝트 인코딩 중…
cmd-omf-exported-project-path = 프로젝트를 { $path }로 내보냈습니다
cmd-omf-imported-project = { $source_name }에서 프로젝트 '{ $project_name }'을(를) 가져왔습니다: 최상위 데이터 세트 { $count }개
cmd-omf-importing-project = 프로젝트 가져오는 중…
cmd-omf-export-failed = OMF 내보내기 실패: { $error }
cmd-omf-import-failed = OMF 가져오기 실패: { $error }
cmd-omf-opened-project = { $source_name }에서 프로젝트 '{ $project_name }'을(를) 열었습니다
cmd-omf-project-source-name-contains-no = 프로젝트 '{ $source_name }'에 지원되는 데이터 요소가 없습니다
cmd-omf-source-name-applied-project-origin = { $source_name }: 병합 전에 프로젝트 원점 { $origin }을(를) 적용했습니다
cmd-omf-crs-differs = { $source_name }: 좌표 참조계 '{ $source_crs }'이(가) 프로젝트 CRS '{ $target_crs }'와(과) 다릅니다. 좌표를 재투영 없이 병합했습니다
cmd-omf-source-name-units-source-units = { $source_name }: 단위 '{ $source_units }'이(가) 프로젝트 단위 '{ $target_units }'와(과) 다릅니다. 좌표를 변환 없이 병합했습니다
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = 내보낼 수 있는 열린 Incline Design 데이터가 없습니다
cmd-placement-2-vertices = 정점 2개
cmd-placement-count-vertices = 정점 { $count }개
cmd-placement-created-circle = 반지름 { $radius } m인 원을 생성했습니다
cmd-placement-created-closed-polyline = 정점 { $count }개인 닫힌 폴리라인을 생성했습니다
cmd-placement-created-line-segment-2-vertices = 정점 2개로 이루어진 선분을 생성했습니다
cmd-placement-created-open-polyline-count-vertices = 정점 { $count }개인 열린 폴리라인을 생성했습니다
cmd-placement-placed-point-x-y-z = { $x }, { $y }, { $z }에 점을 배치했습니다
cmd-placement-radius = 반지름 { $radius } m
cmd-plot-composing-engineering-drawing = 도면 구성 중…
cmd-plot-could-not-write-engineering-drawing = 도면을 기록할 수 없습니다: { $error }
cmd-plot-drawing-scale-fitted-visible-data = 도면 축척이 보이는 데이터에 맞춰졌습니다: 1:{ $scale }
cmd-plot = 도면
cmd-plot-saved-drawing = 도면을 저장했습니다: { $description }({ $width } × { $height } px, { $dpi } dpi)
cmd-point-cloud-classified = { $name } 분류 완료: 전체 { $count }개 점 중 지면 { $ground }개, 식생 { $vegetation }개, 노이즈 { $noise }개
cmd-point-cloud-classifying-point-clouds = 포인트 클라우드 분류 중
cmd-point-cloud-join-dropped-classifications = 점 분류를 버렸습니다. 결합한 클라우드 중 일부가 분류되지 않았으며, 일부만 분류된 클라우드는 지면으로 필터링할 수 없습니다.
cmd-point-cloud-failed-classify-point-clouds-error = 포인트 클라우드를 분류하지 못했습니다: { $error }
cmd-point-cloud-failed-join-point-clouds-error = 포인트 클라우드를 결합하지 못했습니다: { $error }
cmd-point-cloud-failed-load-point-cloud-error = 포인트 클라우드를 불러오지 못했습니다: { $error }
cmd-point-cloud-joined-count-clouds-into-name = 클라우드 { $count }개를 { $name }(으)로 결합했습니다(점 { $points }개)
cmd-point-cloud-joining-name = { $name } 결합 중
cmd-point-cloud-loaded-point-cloud-name-count = 포인트 클라우드 { $name }을(를) 불러왔습니다({ $count }개 점)
cmd-point-cloud-point-cloud-classification-discarded = 포인트 클라우드 분류가 취소되었습니다. 실행 중에 클라우드가 변경되었습니다. 다시 실행하세요.
cmd-point-cloud-point-cloud-loader-disconnected-path = { $path }의 포인트 클라우드 로더 연결이 끊어졌습니다
cmd-point-cloud-select-one-more-loaded-point = 분류하기 전에 로드된 포인트 클라우드를 하나 이상 선택하세요
cmd-point-cloud-select-two-more-loaded-point = 결합하기 전에 로드된 포인트 클라우드를 두 개 이상 선택하세요
cmd-point-cloud-tin-max-edge-disabled = (최대 변 비활성화됨)
cmd-point-cloud-tin-max-edge-max-edge = (최대 변 { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = 포인트 클라우드 TIN 생성 실패: { $error }
cmd-point-cloud-tin-filtered-ground = 지형 TIN: 전체 { $total }개 중 지면 점 { $ground }개로 필터링했습니다
cmd-point-cloud-tin-subsampled = 지형 TIN: 전체 { $total }개 점 중 { $sampled }개를 공간적으로 서브샘플링했습니다
cmd-point-cloud-tin-triangulated = 지형 TIN: 고유 XY 점 { $vertex_count }개를 면 { $face_count }개로 삼각화했습니다{ $suffix }
cmd-products-added-product-delay-ms-ms = 제품을 추가했습니다: { $delay_ms } ms { $name }
cmd-products-deleted-product-delay-ms-ms = 제품을 삭제했습니다: { $delay_ms } ms { $name }
cmd-products-failed-save-products-error = 제품을 저장하지 못했습니다: { $error }
cmd-products-product-no-longer-palette = 해당 제품은 더 이상 팔레트에 없습니다
cmd-property-action-count-object-s-layer = 객체 { $count }개를 레이어 { $layer }(으)로 { $action }
cmd-property-batch-set-axis-value-count = 객체 { $count }개에 { $axis } 값 일괄 설정
cmd-property-batch-set-closed-count-polyline = 폴리라인 { $count }개를 닫힘으로 일괄 설정
cmd-property-batch-set-color-count-object = 객체 { $count }개에 색상 일괄 설정
cmd-property-batch-set-fill-style-count = 객체 { $count }개에 채우기 스타일 일괄 설정
cmd-property-batch-set-line-weight-count = 폴리라인 { $count }개에 선 두께 일괄 설정
cmd-property-copied = 복사됨
cmd-property-moved = 이동됨
cmd-raster-draped = 래스터 { $raster }을(를) 삼각망 { $triangulation } 위에 투영했습니다(범위 겹침)
cmd-raster-failed-load-raster-name-error = 래스터 { $name }을(를) 불러오지 못했습니다: { $error }
cmd-raster-failed-load-raster-path-error = 래스터 { $path }을(를) 불러오지 못했습니다: { $error }
cmd-raster-loaded-raster-name-via-driver = { $driver }를 통해 래스터 { $name }을(를) 불러왔습니다({ $srcx }x{ $srcy }, 미리보기 { $prevx }x{ $prevy })
cmd-raster-no-overlapping-triangulation = { $name }의 범위와 겹치는 로드된 삼각망이 없습니다
cmd-raster-loader-disconnected = { $path }의 래스터 로더 연결이 끊어졌습니다
cmd-raster-undraped = 삼각망 { $count }개에서 래스터 투영을 해제했습니다
cmd-reference-surface-build-surface-failed-error = 표면 만들기 실패: { $error }
cmd-reference-surface-building-surface = 표면 만드는 중…
cmd-reference-surface-built-surface-name-inside-grid = 범위 안의 그리드 노드 { $inside }개로부터 표면 { $name }을(를) 만들었습니다(노드 { $vertex_count }개, 면 { $face_count }개, 박스 z { $low } ~ { $high }){ $support }{ $controls }
cmd-reference-surface-built-surface-name-from-vertex = 점 { $vertex_count }개로부터 표면 { $name }을(를) 만들었습니다(면 { $face_count }개, 박스 z { $low } ~ { $high }){ $support }{ $coincident }{ $controls }
cmd-reference-surface-control-string-index-could-not = 제어선 { $index }을(를) 메쉬에 추가할 수 없습니다
cmd-reference-surface-control-string-index-crosses-itself = 제어선 { $index }이(가) 평면상 ({ $x }, { $y })에서 자기 자신과 교차합니다
cmd-reference-surface-control-string-index-doubles-back = 제어선 { $index }이(가) 평면상 ({ $x }, { $y })에서 자기 자신으로 되돌아옵니다
cmd-reference-surface-control-string-index-ends-where = 제어선 { $index }의 끝이 시작점과 같습니다. 마스크로 사용하려면 닫으세요
cmd-reference-surface-control-string-index-has-count = 제어선 { $index }에는 고유 정점이 { $count }개 있습니다. 제어선에는 최소 { $minimum }개가 필요합니다
cmd-reference-surface-control-string-index-has-non = 제어선 { $index }에 유한하지 않은 좌표가 있습니다
cmd-reference-surface-control-string-index-no-longer = 제어선 { $index }을(를) 더 이상 사용할 수 없습니다
cmd-reference-surface-control-string-index-runs-along = 제어선 { $index }이(가) 평면상에서 범위선을 따라 지나갑니다. 아직 지원되지 않습니다
cmd-reference-surface-control-string-overrides-pick-x = 제어선이 ({ $x }, { $y })의 선택 점을 덮어씁니다: 선택 { $pick } m, 제어 { $control } m, 차이 { $difference } m
cmd-reference-surface-control-strings-b-disagree-x = 제어선 { $a }과(와) { $b }이(가) ({ $x }, { $y })에서 일치하지 않습니다: { $za } m 대 { $zb } m, { $difference } m 차이
cmd-reference-surface-control-strings-b-run-along = 제어선 { $a }과(와) { $b }이(가) 평면상에서 서로 겹쳐 지나갑니다. 아직 지원되지 않습니다
cmd-reference-surface-count-control-string-s-entered = ; 제어선 { $count }개를 점 { $points }개로 입력함{ $crossings }
cmd-reference-surface-count-other-strings-hidden = 나머지 제어선 { $count }개가 숨겨졌습니다. 보기 도구 모음의 모두 숨기기 해제로 다시 표시할 수 있습니다
cmd-unhide-all-count = 숨겨진 객체 { $count }개를 다시 표시했습니다
cmd-unhide-all-objects-items-count = 숨겨진 객체 { $objects }개와 항목 { $items }개를 다시 표시했습니다
cmd-unhide-all-nothing-hidden = 불러온 레이어에 숨긴 객체가 없습니다
cmd-reference-surface-count-control-string-s-vertices = ; 정점 { $vertices }개의 제어선 { $count }개{ $crossings }
cmd-reference-surface-count-point-s-inside-extent = 범위 안의 점이 { $count }개입니다. 표면에는 최소 { $minimum }개가 필요합니다
cmd-reference-surface-count-point-s-outside-extent = ; 범위 밖의 점 { $count }개가 지지점으로 표면 형태에 영향을 주었습니다
cmd-reference-surface-count-point-s-selected-surface = 선택한 점이 { $count }개입니다. 표면에는 최소 { $minimum }개가 필요합니다
cmd-reference-surface-count-places-stop-build = 제어선의 { $count }곳이 생성을 막고 있으며, 각각 링으로 표시했습니다:
cmd-reference-surface-cleaned-heading = 표면 만들기가 선 정리와 모두 중간 높이로 연결이 하듯이 제어선의 자체 사본을 정리했습니다. 프로젝트의 선은 바뀌지 않았습니다:
cmd-reference-surface-cleaned-repeats = 반복된 점을 하나로 병합한 곳 { $count }곳, 위치 { $places }
cmd-reference-surface-cleaned-spikes = 스파이크 { $count }개 제거, 위치 { $places }
cmd-reference-surface-cleaned-retraces = 선 위로 되돌아가는 구간 { $count }개 잘라냄, 위치 { $places }
cmd-reference-surface-cleaned-loops = 선이 자기 자신과 교차하는 고리 { $count }개 잘라냄, 위치 { $places }
cmd-reference-surface-cleaned-zeros = z = 0인 정점 { $count }개 제거, 위치 { $places }
cmd-reference-surface-cleaned-heights = 이웃과 크게 동떨어진 단일 높이 { $count }개 제거, 위치 { $places }
cmd-reference-surface-cleaned-shared-cut = 두 선이 공유한 구간 { $count }개를 짧은 선에서 잘라냄, 위치 { $places }
cmd-reference-surface-cleaned-removed = 전체 길이에 걸쳐 다른 선을 따라가는 선 { $count }개를 사본에서 제거, 위치 { $places }
cmd-reference-surface-cleaned-joined-small = 차이가 { $limit } m 이하인 교차 { $count }곳을 중간 높이로 연결, 위치 { $places }
cmd-reference-surface-cleaned-joined-on-request = 차이가 { $low } m 초과 { $high } m 이하인 교차 { $count }곳을 중간 높이로 연결, 위치 { $places }
cmd-reference-surface-cleaned-vertex-shared = 선의 차이가 { $limit } m를 넘는 곳에 공유 정점 { $count }개 삽입, 위치 { $places }
cmd-reference-surface-left-out-count = 제어선 { $count }개를 이번 생성에서 제외했으며, 각각 링으로 표시하고 선택했습니다. 표면은 나머지로 만들어집니다:
cmd-reference-surface-left-out-below = 선 { $string } 제외: { $places }에서 { $others }보다 { $amount } m 아래에 있습니다
cmd-reference-surface-left-out-above = 선 { $string } 제외: { $places }에서 { $others }보다 { $amount } m 위에 있습니다
cmd-reference-surface-left-out-above-and-below = 선 { $string } 제외: { $places }에서 { $others }보다 { $amount } m 위와 아래에 있습니다
cmd-reference-surface-left-out-along = 선 { $string } 제외: { $places }에서 { $others }을(를) 따라갑니다
cmd-reference-surface-left-out-range = { $low }~{ $high }
cmd-reference-surface-left-out-other-string = 선 { $string }
cmd-reference-surface-left-out-other-strings = 선 { $strings }
cmd-reference-surface-left-out-too-short = 선 { $string } 제외: 서로 다른 정점이 두 개 미만입니다, ({ $x }, { $y })
cmd-reference-surface-left-out-ends-where-it-starts = 선 { $string } 제외: 시작한 곳에서 끝납니다, ({ $x }, { $y })
cmd-reference-surface-left-out-turns-back = 선 { $string } 제외: ({ $x }, { $y })에서 되돌아갑니다
cmd-reference-surface-left-out-crosses-itself = 선 { $string } 제외: ({ $x }, { $y })에서 자기 자신과 교차합니다
cmd-reference-surface-left-out-points-disagree = 선 { $string } 제외: 평면상 같은 곳의 두 점이 높이로 { $miss } m 떨어져 있습니다, ({ $x }, { $y })
cmd-reference-surface-left-out-none-left = 충돌하거나 모양이 잘못된 선을 제외하면 제어선이 하나도 남지 않으므로 아무것도 만들지 않습니다
cmd-reference-surface-thinned = 제어선의 점이 한 표면에 너무 많아 생성 과정에서 사본을 솎아냈습니다: 끝점, 서로 교차하는 곳, 그리고 그 정점이 없는 선에서 평면이나 높이로 { $tolerance } m 넘게 떨어진 모든 정점에 점 { $kept }개를 남겼고{ $raised }, 선을 따라 { $spacing } m마다 점을 두었습니다. 예산 { $budget } 중 점 { $used }개를 사용했습니다
cmd-reference-surface-thinned-raised = (더 작으면 맞지 않아 { $first } m에서 올림)
cmd-reference-surface-thin-refused = 제어선이 한 표면의 예산 { $budget }점에 맞지 않습니다: 끝점, 교차점, 그리고 그것이 없는 선에서 { $tolerance } m 넘게 떨어진 정점만 남겨도 점 { $kept }개, 선택 점 { $picks }개를 더하면 { $total }개입니다. 아무것도 만들지 않습니다
cmd-reference-surface-count-refused-strings-selected = 거부된 제어선 { $count }개가 이제 선택되었습니다
cmd-reference-surface-count-point-s-shared-plan = ; 점 { $count }개가 같은 평면 위치를 공유하여 한 번만 사용했습니다
cmd-reference-surface-delaunay-insert-failed-error = 들로네 삽입 실패: { $error }
cmd-reference-surface-extent-must-closed-string = 범위선은 닫힌 선이어야 합니다
cmd-reference-surface-extent-string-crosses-itself-plan = 범위선이 평면상에서 자기 자신과 교차합니다
cmd-reference-surface-extent-string-has-non-finite = 범위선에 유한하지 않은 좌표가 있습니다
cmd-reference-surface-extent-string-needs-least-three = 범위선에는 서로 다른 정점이 최소 3개 필요합니다
cmd-reference-surface-extent-string-no-longer-available = 범위선을 더 이상 사용할 수 없습니다
cmd-reference-surface-meeting-count-crossing-s = 교차점 { $count }개에서 만남
cmd-reference-surface-and-more = , … 외 { $more }개
cmd-reference-surface-no-mask-selected-surface-outline = 선택한 마스크 없음. 표면이 점들의 외곽선에 { $buffer } m를 더한 범위로 클립됩니다
cmd-reference-surface-no-mask-selected-surface-unclipped = 선택한 마스크 없음. 표면이 클립되지 않았습니다
cmd-reference-surface-no-part-surface-falls-inside = 표면의 어떤 부분도 범위 안에 들어오지 않습니다
cmd-reference-surface-open-project-before-building-surface = 표면을 만들기 전에 프로젝트를 여세요
cmd-reference-surface-points-collinear-plan-surface-needs = 점들이 평면상에서 일직선입니다. 표면에는 일직선이 아닌 점 3개가 필요합니다
cmd-reference-surface-select-exactly-one-closed-string = 표면을 클립할 닫힌 선을 정확히 하나 선택하세요
cmd-reference-surface-selected-point-has-non-finite = 선택한 점에 유한하지 않은 좌표가 있습니다
cmd-reference-surface-selected-points-span-count-layers = 선택한 점이 레이어 { $count }개에 걸쳐 있습니다. 표면은 { $section } 아래에 배치됩니다
cmd-reference-surface-control-string-index-has-two = 제어선 { $index }에 평면상에서 ({ $x }, { $y })로부터 { $distance } m 이내이면서 높이가 다른 정점이 두 개 있습니다
cmd-reference-surface-run-record-used-point = 실행 기록: 지정한 { $picks }개 점 중 { $used }개 사용, { $merged }개 병합, 제어선 아래 { $left_out }개 제외(그중 { $overridden }개는 다른 높이). { $method }, 간격 { $spacing } m. 작성자 { $author }, 날짜 { $date }
cmd-reference-surface-count-pair-s-points-closer = 평면상 간격이 { $spacing } m보다 가까운 점 쌍 { $count }개의 경사가 { $degrees }도보다 급합니다. 그리드는 물결 현상 없이는 이를 따라갈 수 없습니다:
cmd-reference-surface-steep-pair = ({ $ax }, { $ay }, { $az })와 ({ $bx }, { $by }, { $bz }): 거리 { $distance } m, 높이차 { $rise } m, 경사 { $slope }도
cmd-reference-surface-surface-could-not-cut = ({ $x }, { $y }) 부근에서 범위선을 따라 표면을 자를 수 없습니다
cmd-relimit-click-missed = 재한계: 클릭이 어떤 객체도 맞히지 못했습니다 (커서 아래에 객체 없음)
cmd-relimit-click-ignored = 재한계: 도구가 현재 대상 선택을 기다리고 있지 않아 클릭을 무시했습니다
cmd-relimit-clicked-source-line = 재한계: 원본 선 자체를 클릭했습니다. 다른 선을 선택하세요
cmd-relimit-no-source-line = 재한계: 원본 선이 설정되지 않아 선택을 중단합니다
cmd-relimit-relimited-line-source-id-selected = 선 { $source_id }을(를) 선택한 대상으로 재한계했습니다
cmd-relimit-resized-line-source-id-using = { $mode } 모드와 값 { $value }을(를) 사용하여 선 { $source_id }의 크기를 조정했습니다
cmd-rename-item-no-longer-belongs-active = 해당 항목은 더 이상 활성 프로젝트에 속하지 않습니다
cmd-rename-renamed-before-name = '{ $before }'을(를) '{ $name }'(으)로 이름을 변경했습니다
cmd-rename-renamed-name-taken = '{ $before }'을(를) '{ $name }'(으)로 이름을 변경했습니다('{ $requested }'은(는) 이미 사용 중)
cmd-rotate-collar-turned-count-drillhole-collar-s = 시추공 공구 { $count }개를 { $rotation } 회전했습니다
cmd-section-verb-count-item-s-section = { $section }의 항목 { $count }개를 { $verb }
cmd-selection-delete-vertex = 정점 삭제
cmd-selection-deleted-count-selected-object-s = 선택한 객체 { $count }개를 삭제했습니다
cmd-selection-deleted-vertex = 폴리라인 { $object_id }에서 정점 { $vertex }을(를) 삭제했습니다
cmd-strat-check-checking = { $name }의 층서 열을 검증하는 중
cmd-strat-check-failed = 층서 열 검증 실패: { $error }
cmd-strat-check-summary = { $name }의 { $field }을(를) 검증했습니다: 시추공 { $holes }개, 표시됨 { $flagged }개
cmd-strat-check-too-many-codes = { $name }의 { $field }에는 순서를 매기기에 코드가 너무 많습니다
cmd-strat-import-filled = { $field }의 층서 열을 채웠습니다: 이름 { $names }개, 시추공 { $flagged }개가 맞지 않습니다. 검증을 눌러 확인하세요.
cmd-strat-import-filled-groups = { $field }의 층서 열을 채웠습니다: { $groups }개 그룹에 이름 { $names }개, 시추공 { $flagged }개가 맞지 않습니다. 검증을 눌러 확인하세요.
cmd-string-clean-and = 및
cmd-string-clean-checks-pass = 표면 만들기 검사가 레이어 { $layer }에서 통과합니다
cmd-string-clean-build-would-leave-out = 표면 만들기는 레이어 { $layer }의 선 { $strings }을(를) 제외하고 나머지로 만듭니다
cmd-string-clean-checks-refuse = 표면 만들기 검사가 여전히 레이어 { $layer }을(를) 거부합니다: { $count }곳을 링으로 표시
cmd-string-clean-clean-strings = 선 정리
cmd-string-clean-clean-this-string = 이 선 정리
cmd-string-clean-cleaning-strings = 선 정리 중
cmd-string-clean-hand-along = 수동 수정 필요: 선 { $strings }이(가) ({ $x }, { $y })에서 서로를 따라갑니다
cmd-string-clean-hand-build-refuses = 수동 수정 필요: 표면 만들기가 위에서 언급되지 않은 선을 여전히 거부합니다: { $refusal }
cmd-string-clean-hand-crosses-itself = 수동 수정 필요: 선 { $string }이(가) ({ $x }, { $y })에서 자기 자신과 교차합니다
cmd-string-clean-hand-crossing = 수동 수정 필요: 선 { $strings }이(가) ({ $x }, { $y })에서 { $miss } m 어긋납니다
cmd-string-clean-hand-ends-where-it-starts = 수동 수정 필요: 선 { $string }이(가) ({ $x }, { $y })에서 시작한 곳에서 끝납니다
cmd-string-clean-hand-near-miss = 수동 수정 필요: 선 { $strings }이(가) 만나지 않고 가까이 지나가며 { $miss } m 어긋납니다, ({ $x }, { $y })
cmd-string-clean-hand-points-disagree = 수동 수정 필요: 선 { $string }에 평면상 같은 곳의 두 점이 있으며 높이가 { $miss } m 떨어져 있습니다, ({ $x }, { $y })
cmd-string-clean-hand-too-short = 수동 수정 필요: 선 { $string }의 서로 다른 정점이 두 개 미만입니다, ({ $x }, { $y })
cmd-string-clean-hand-turns-back = 수동 수정 필요: 선 { $string }이(가) ({ $x }, { $y })에서 되돌아갑니다
cmd-string-clean-height-dropped = 선 { $string }: 이웃과 { $offset } m 떨어진 높이를 제거함 ({ $x }, { $y }, { $z })
cmd-string-clean-join-all-at-halfway = 모두 중간 높이로 연결
cmd-string-clean-clear-rings = 링 지우기
cmd-string-clean-join-all-crossing = 모두 중간 높이로 연결 대상: 선 { $strings }이(가) ({ $x }, { $y })에서 { $miss } m 어긋납니다
cmd-string-clean-join-here-at-halfway = 여기를 중간 높이로 연결
cmd-string-clean-joining-strings = 선을 중간 높이로 연결 중
cmd-string-clean-joined = 선 { $strings }: ({ $x }, { $y })에서 중간 높이 { $z }(으)로 연결함, { $miss } m 어긋나 있었음
cmd-string-clean-layer = 레이어 { $layer }: 선 { $strings }개
cmd-string-clean-left-arcs = 선 { $string }을(를) 그린 그대로 둠: 호가 있습니다
cmd-string-clean-left-not-finite = 선 { $string }을(를) 그린 그대로 둠: 유한하지 않은 좌표가 있습니다
cmd-string-clean-loop-cut = 선 { $string }: 자기 교차하는 곳에서 정점 { $count }개짜리 고리를 잘라냄 ({ $x }, { $y }, { $z })
cmd-string-clean-nothing-to-clean = 선택한 선에 정리할 것이 없습니다
cmd-string-clean-odd-above-every = 선 { $string }은(는) 교차하는 모든 선보다 { $low }~{ $high } m 위에 있습니다 (교차 { $total }곳 중 { $count }곳)
cmd-string-clean-odd-above-misses = 선 { $string }은(는) { $limit } m 넘게 어긋나는 모든 선보다 { $low }~{ $high } m 위에 있습니다 (교차 { $total }곳 중 { $count }곳)
cmd-string-clean-odd-below-every = 선 { $string }은(는) 교차하는 모든 선보다 { $low }~{ $high } m 아래에 있습니다 (교차 { $total }곳 중 { $count }곳)
cmd-string-clean-odd-below-misses = 선 { $string }은(는) { $limit } m 넘게 어긋나는 모든 선보다 { $low }~{ $high } m 아래에 있습니다 (교차 { $total }곳 중 { $count }곳)
cmd-string-clean-removed = 선 { $string }: 전체 길이에 걸쳐 선 { $kept }을(를) 따라갔으므로 제거함
cmd-string-clean-repeats-merged = 선 { $string }: 반복된 점 { $count }개를 하나로 병합함 ({ $x }, { $y }, { $z })
cmd-string-clean-retrace-dropped = 선 { $string }: 선 위로 되돌아간 정점 { $count }개를 잘라냄 ({ $x }, { $y }, { $z })
cmd-string-clean-ring-title = 선 { $strings }
cmd-string-clean-ring-title-miss = 선 { $strings }, { $miss } m 차이
cmd-string-clean-rings = 링이 있는 선
cmd-string-clean-run-finished = { $label }: 완료, 수정 { $edits }건, 링 표시 { $rings }곳
cmd-string-clean-run-started = { $label }: 레이어 { $layers }개의 선 { $strings }개
cmd-string-clean-shared-cut = 선 { $string }: 선 { $kept }과(와) 공유한 { $length } m를 잘라냄 ({ $x }, { $y }, { $z })
cmd-string-clean-spike-dropped = 선 { $string }: 스파이크를 제거함 ({ $x }, { $y }, { $z })
cmd-string-clean-vertex-shared = 선 { $strings }: ({ $x }, { $y })에 공유 정점을 삽입함, { $miss } m 어긋남
cmd-string-clean-zero-dropped = 선 { $string }: z = 0인 정점을 제거함 ({ $x }, { $y })
cmd-selection-duplicate-selection = 선택 항목 복제
cmd-selection-duplicated-count-object-s = 객체 { $count }개를 복제했습니다
cmd-session-created-triangulation = { $surface_type } 표면 유형으로부터 삼각망 '{ $name }'을(를) 생성했습니다(정점 { $vertex_count }개, 면 { $face_count }개)
cmd-session-deleted-triangulation = 프로젝트에서 삼각망 '{ $name }'을(를) 삭제했습니다
cmd-session-failed-load-triangulation-error = 삼각망을 불러오지 못했습니다: { $error }
cmd-session-failed-load-triangulation-message = 삼각망을 불러오지 못했습니다: { $message }
cmd-session-loaded-triangulation = 삼각망 '{ $name }'을(를) 불러왔습니다({ $path }, 정점 { $vertex_count }개, 면 { $face_count }개)
cmd-session-set-triangulation-tri-id-color = 삼각망 { $tri_id }의 색상을 { $color }(으)로 설정했습니다
cmd-session-triangulation-load-no-result = { $path }의 삼각망 불러오기가 결과 없이 종료되었습니다
cmd-session-triangulation-failed = 삼각망 작업 실패: { $message }
cmd-session-unloaded-triangulation-name = 삼각망 '{ $name }'을(를) 언로드했습니다
cmd-slice-entered-slice-view-cx-cy = { $cx }, { $cy }, { $cz }에서 { $dx }, { $dy } 방향으로 슬라이스 뷰에 진입했습니다(선 길이 { $length }m)
cmd-slice-exited-slice-view = 슬라이스 뷰를 종료했습니다
cmd-slice-reset-section-view-fit-extents = 슬라이스 뷰 재설정(전체 범위에 맞춤)
cmd-slice-set-section-grid-enabled = 슬라이스 그리드 설정 = { $enabled }
cmd-split-created-2-open-polylines = 열린 폴리라인 2개를 생성했습니다
cmd-split-line = 선 분할
cmd-split-points-needs-interior-vertex = 점에서 분할: 열린 선의 내부 정점을 선택하세요
cmd-split-polyline-into-two = 원본 폴리라인을 열린 폴리라인 두 개로 분할했습니다
cmd-text-edit-finished = 객체 { $object_id }의 텍스트 편집을 완료했습니다
cmd-text-updated = 객체 { $object_id }의 텍스트를 업데이트했습니다
cmd-view-centre-rotation-not-available-flying = 회전 중심은 비행 모드에서 사용할 수 없습니다
cmd-view-fixed-centre-rotation-x-y = 회전 중심을 { $x }, { $y }, { $z }에 고정했습니다
cmd-view-no-point-under-cursor-fix = 커서 아래에 회전 중심을 고정할 점이 없습니다
cmd-view-released-centre-rotation = 회전 중심을 해제했습니다
cmd-view-reset-view-fit-extents = 뷰 재설정(범위에 맞춤)
cmd-view-set-cinematic-view-enabled = 시네마틱 뷰 설정 = { $enabled }
cmd-view-set-topology-wireframes-enabled = 지형면 와이어프레임 설정 = { $enabled }
cmd-view-set-view-points-enabled = 뷰 포인트 설정 = { $enabled }
cmd-view-set-xy-grid-enabled = XY 그리드 설정 = { $enabled }
cmd-view-zoom-extents-preserving-angle = 범위에 맞춰 확대/축소(각도 유지)

## Common strings

common-add-product = 제품 추가
common-appearance = 모양...
common-background = 배경
common-block-model = 블록 모델
common-block-models = 블록 모델
common-borehole-inspector = 시추공 검사기
common-build-surface = 표면 만들기
common-build-surface-ellipsis = 표면 만들기...
common-cancelled = 취소됨
common-chamfer = 모따기
common-choose = 선택...
common-circle = 원
common-classify = 분류
common-classify-point-clouds = 포인트 클라우드 분류
common-click-point-fix-centre-rotation = 점을 클릭하여 회전 중심을 고정하세요
common-clip-surface-polyline = 폴리라인으로 표면 클립...
common-closed = 닫힘
common-collection = 컬렉션
common-colour = 색상
common-confirm-omf-rewrite = OMF 재작성 확인
common-could-not-replace-current-project = 현재 프로젝트를 교체할 수 없습니다: { $error }
common-count-object-s = 객체 { $count }개
common-create = 생성
common-create-batter-berm = 사면-소단 생성
common-create-bezier-curve = 베지어 곡선 생성
common-create-block-model = 블록 모델 생성
common-create-block-model-ellipsis = 블록 모델 생성...
common-create-circle = 원 생성
common-create-drill-pattern = 천공 패턴 생성
common-create-layer = 레이어 생성
common-create-line = 선 생성
common-create-ore-triangulation = 광체 삼각망 생성
common-create-ore-triangulation-ellipsis = 광체 삼각망 생성...
common-create-point = 점 생성
common-create-polyline = 폴리라인 생성
common-create-triangulation = 삼각망 생성...
common-crosses = 십자형
common-cut = 잘라내기
common-cut-topology-pit-shell = 채굴장 쉘로 지형면 절단...
common-delete-collection = 컬렉션 삭제
common-delete-layer = 레이어 삭제
common-delete-product = 제품 삭제
common-delete-selection = 선택 항목 삭제
common-designs = 설계
common-discard-layer-changes = 레이어 변경 사항 취소
common-down = 아래로
common-drape-topology = 지형면에 투영
common-easting = 동거리
common-edit-object = 객체 편집
common-edit-text = 텍스트 편집
common-elevation = 표고
common-exit-without-saving = 저장하지 않고 종료
common-export-engineering-drawing = 도면 내보내기
common-file-was-left-out-downhole = { $file }이(가) 공내 물리탐사에서 제외되었습니다: { $error }
common-filter = 필터
common-fly-mode = 비행 모드
common-generate-contour-lines = 등고선 생성...
common-hide-all = 모두 숨기기
common-hide-selection = 선택 항목 숨기기
common-unhide-all = 모두 숨기기 해제
common-hole-id = 시추공 ID
common-ignore = 무시
common-import-csv-block-model = CSV 블록 모델 가져오기
common-import-dxf = DXF 가져오기
common-incline-design-project = Incline Design 프로젝트
common-join = 결합...
common-join-point-clouds = 포인트 클라우드 결합
common-joined-cloud = 결합된 클라우드
common-layer = 레이어
common-legend = 범례
common-line = 선
common-line-weight = 선 두께
common-link-geophysics = 물리탐사 연결...
common-load-drillholes-before-linking-geophysics = 물리탐사를 연결하기 전에 시추공 데이터 세트를 불러오세요
common-lock-all = 모두 잠금
common-lock-selection = 선택 항목 잠금
common-m = m
common-max = 최대
common-merge-shell-into-topology = 쉘을 지형면에 병합
common-merge-shell-into-topology-ellipsis = 쉘을 지형면에 병합...
common-modelling = 모델링
common-move-collar = 공구 이동
common-move-collection = 컬렉션으로 이동
common-move-design = 설계 이동
common-move-selection = 선택 항목 이동
common-name-has-no-readable-size = { $name }의 크기를 읽을 수 없습니다
common-new-product = 새 제품
common-no-block-models = 블록 모델 없음
common-no-design-layers = 설계 레이어 없음
common-no-drill-holes = 시추공 없음
common-no-file-chosen = 선택된 파일 없음
common-no-open-project = 열린 프로젝트 없음
common-no-point-clouds = 포인트 클라우드 없음
common-no-triangulations = 삼각망 없음
common-none = 없음
common-northing = 북거리
common-offset = 오프셋
common-ok = 확인
common-open = 열기
common-orientation = 방향
common-point = 점
common-point-cloud = 포인트 클라우드
common-point-clouds = 포인트 클라우드
common-polyline = 폴리라인
common-polyline-layer = '{ $layer }'의 폴리라인
common-project = 프로젝트
common-rasters = 래스터
common-redo = 다시 실행
common-reference-points = 기준점...
common-relimit-line = 선 재한계
common-remove-project = 프로젝트 제거
common-reset-view = 뷰 재설정
common-reveal-all = 모두 표시
common-reveal-finder = Finder에서 표시
common-rotate-collar = 공구 회전
common-save-exit = 저장 후 종료
common-scale-bar = 축척 막대
common-set-initiation-point = 기폭점 설정
common-shape = 형태
common-shell = 쉘 포함
common-slashes = 사선
common-slice = 슬라이스
common-slice-triangulation-z-range = Z 범위로 삼각망 슬라이스...
common-surface-contours = 표면 등고선
common-text = 텍스트
common-degree-suffix = °
common-tie-holes = 발파공 연결
common-triangulations = 삼각망
common-trim-topology = 지형면에 맞춰 트리밍...
common-undo = 실행 취소
common-undrape-all = 모든 투영 해제
common-uniform-white = 균일한 흰색
common-unknown = 알 수 없음
common-unlock-all = 모두 잠금 해제
common-untitled = 제목 없음
common-up = 위로
common-vertical-exaggeration = 수직 과장
common-x = x
common-zoom-extents = 범위에 맞춰 확대/축소

## Confirmations strings

confirmations-close-project-unsaved-changes = 프로젝트 닫기: 저장되지 않은 변경 사항
confirmations-close-without-saving = 저장하지 않고 닫기
confirmations-delete = 삭제
confirmations-delete-objects = 객체 삭제
confirmations-discard = 취소
confirmations-discard-all-unsaved-changes-layer =
    레이어 '{ $name }'의 저장되지 않은 모든 변경 사항을 취소하시겠습니까?
    저장된 레이어를 디스크에서 다시 불러오며, 다른 레이어의 변경 사항은 유지됩니다. 이 작업은 되돌릴 수 없습니다.
confirmations-discard-all-unsaved-changes-name =
    '{ $name }'의 저장되지 않은 모든 변경 사항을 취소하시겠습니까?
    마지막으로 저장된 버전을 디스크에서 다시 불러옵니다. 이 작업은 되돌릴 수 없습니다.
confirmations-discard-changes = 변경 사항 취소
confirmations-exit-unsaved-changes = 종료: 저장되지 않은 변경 사항
confirmations-incline-design-cannot-reproduce-all = Incline Design은 원본 OMF의 모든 콘텐츠를 재현할 수 없습니다. 저장 시 다음 콘텐츠는 생략됩니다:
confirmations-product = 제품
confirmations-project = 이 프로젝트
confirmations-remove-name-delete-its-browser = '{ $name }'을(를) 제거하고 브라우저에 저장된 사본을 삭제하시겠습니까? 저장되지 않은 변경 사항은 손실됩니다.
confirmations-remove-project-unsaved-changes = 프로젝트 제거: 저장되지 않은 변경 사항
confirmations-remove-without-saving = 저장하지 않고 제거
confirmations-replace-project-unsaved-changes = 프로젝트 교체: 저장되지 않은 변경 사항
confirmations-save = 저장
confirmations-save-anyway = 그래도 저장
confirmations-save-changes-current-project-before = 교체하기 전에 현재 프로젝트의 변경 사항을 저장하시겠습니까?
confirmations-save-changes-name-before-closing = '{ $name }'을(를) 닫기 전에 변경 사항을 저장하시겠습니까?
confirmations-save-changes-name-before-removing = Incline Design에서 '{ $name }'을(를) 제거하기 전에 변경 사항을 저장하시겠습니까?
confirmations-save-close = 저장 후 닫기
confirmations-save-modified-project-before-exiting = 종료하기 전에 수정된 프로젝트를 저장하시겠습니까?
confirmations-save-to-browser-before-exit = 종료하기 전에 수정된 프로젝트를 브라우저 저장소에 저장하시겠습니까?
confirmations-save-remove = 저장 후 제거

## Console strings

console-copy-all = 모두 복사
console-copy-message = 메시지 복사
console-error = 오류
console-info = 정보
console-no-console-activity-yet = 아직 콘솔 활동 없음
console-pending = 대기 중
console-progress-summary = 진행 중 · { $summary }
console-success = 성공
console-warn = 경고

## Csv strings

csv-block-model-category = 범주
csv-block-model-value = 값
csv-drill-hole-rows-for-undefined-holes = 번들의 지오메트리에 정의되지 않은 시추공에 대한 행이 { $count }개 있습니다
csv-drill-hole-count-rows-were-skipped-total = 총 { $count }개 행을 건너뛰었습니다
csv-drill-hole-csv-file-empty = CSV 파일이 비어 있습니다
csv-drill-hole-csv-has-too-many-unreadable = CSV에 복구할 수 없을 만큼 읽을 수 없는 바이트가 많습니다. 레거시 인코딩일 수 있으니 UTF-8로 저장한 후 다시 가져오세요
csv-drill-hole-csv-header-has-no-columns = CSV 헤더에 열이 없습니다
csv-drill-hole-csv-headers-must-nonblank-unique = CSV 헤더는 비어 있지 않고 고유해야 합니다
csv-drill-hole-geophysics-needs-geometry = 공내 물리탐사에는 번들에 공구 파일 또는 명시적 구간 파일이 필요하며, 물리탐사 데이터는 그 파일의 시추공에 연결됩니다
csv-drill-hole-azimuth-out-of-range = { $file }에 방위각이 0과 360 사이가 아닌 행이 { $count }개 있습니다
csv-drill-hole-dip-out-of-range = { $file }에 경사각이 -90과 90 사이가 아닌 행이 { $count }개 있으며, 해당 행은 방향 없이 읽었습니다
csv-drill-hole-file-inclination-values-could-angle = { $file }의 각도일 수 있는 경사 값이 모두 0 이하이므로, 해당 열을 아래쪽이 음수인 경사각으로 읽었습니다
csv-drill-hole-file-maps-gamma-density-column = { $file }에서 감마 또는 밀도 열이 두 번 매핑되었습니다
csv-drill-hole-invalid-utf8 = { $file }은(는) 올바른 UTF-8이 아닙니다. 읽을 수 없는 바이트 { $count }개를 셀 { $cells }개에서 대체했으며, 손상된 셀은 데이터로 읽지 않습니다
csv-drill-hole-file-requires-gamma-density-column = { $file }에는 감마 또는 밀도 열이 필요합니다
csv-drill-hole-row-undefined-hole = { $file }의 { $row }행은 DHID '{ $dhid }'에 대한 것이지만, 번들의 지오메트리에 정의되지 않은 시추공입니다
csv-drill-hole-holes-hole-s-carry-overlapping = 시추공 { $holes }개에 겹치는 구간이 있습니다(예: 분할 구간과 함께 기록된 탄층): { $summary }
csv-drill-hole-skipped-row-reason = 행을 건너뛰었습니다: { $reason }
csv-drill-hole-row-attribute-not-number = { $file } { $row }행: 숫자 열에 '{ $value }' 값이 있습니다
csv-drill-hole-row-repeats-dhid = { $file } { $row }행: DHID '{ $dhid }'가 반복됩니다
csv-drill-hole-most-rows-unreadable = { $file }: { $count }행 중 { $skipped }행을 읽을 수 없습니다. 원인은 콘솔에 있습니다
csv-drill-hole-file-maps-dip-column-twice = { $file }가 경사 또는 경사각 열을 두 번 지정합니다
csv-drill-hole-row-has-no-geometry = { $file } { $row }행: 완전한 XYZ 또는 방위각/경사 지오메트리가 없습니다
csv-drill-hole-row-invalid-interval = { $file } { $row }행: DHID '{ $dhid }'의 구간 { $from }..{ $to }이(가) 잘못되었습니다
csv-drill-hole-row-zero-length-segment = { $file } { $row }행: DHID '{ $dhid }'의 { $depth } 위치에 길이가 0인 구간이 있습니다
csv-drill-hole-row-unreadable-value = { $file } { $row }행: 읽을 수 없는 값이 있습니다
csv-drill-hole-csv-is-wide-text = CSV가 UTF-16 또는 UTF-32 텍스트입니다. UTF-8로 저장한 후 다시 가져오세요
csv-drill-hole-csv-holds-nul-bytes = CSV 전체에 NUL 바이트가 있어 UTF-8 텍스트가 아닙니다. UTF-16 또는 UTF-32로 작성된 경우 UTF-8로 저장한 후 다시 가져오세요
csv-drill-hole-overlap-field-summary = { $field }: { $count }개 시추공, 예: { $examples }
csv-geophysics-above-5 = 5 초과
csv-geophysics-below-0-5 = 0.5 미만
csv-geophysics-count-more = (+{ $count }개 더)
csv-geophysics-count-rows-were-skipped-total = { $file }에서 총 { $count }개 행을 건너뛰었습니다
csv-geophysics-csv-has-record-longer-than = CSV에 { $limit } MiB보다 긴 레코드가 있습니다. CSV에 있어야 할 줄 바꿈이 파일에 없거나 텍스트 파일이 아닙니다
csv-geophysics-csv-has-unterminated-quoted-field = CSV에 닫히지 않은 따옴표 필드가 있습니다
csv-geophysics-curve-file-was-left-out = { $file }의 { $curve }이(가) 제외되었습니다. 측정값 대부분이 { $side }이어서 중앙값이 0.5~5 g/cc 범위를 벗어나며 단위가 잘못된 것으로 보입니다(g/cc 필요). Incline은 단위를 변환하지 않습니다. 내보내기를 수정한 후 다시 연결하세요
csv-geophysics-file-empty = { $file }이(가) 비어 있습니다
csv-geophysics-file-has-no-curve-no = { $file }에 곡선이 없습니다. 시추공 ID와 심도 외에 숫자가 있는 열이 없습니다
csv-geophysics-file-mapping-has-mapped-columns = { $file } 매핑에는 열이 { $mapped }개 있고, CSV에는 { $found }개 있습니다
csv-geophysics-file-no-longer-matches-its = { $file }이(가) 더 이상 인덱스와 일치하지 않습니다. 다시 연결하세요
csv-geophysics-file-not-grouped-hole-its = { $file }이(가) 시추공별로 그룹화되어 있지 않습니다. 시추공의 행이 너무 많은 구간으로 나뉘어 있습니다. 시추공 ID, 심도 순으로 정렬한 후 다시 연결하세요
csv-geophysics-file-requires-one-dhid-one = { $file }에는 DHID 열 하나와 심도 열 하나가 필요합니다
csv-geophysics-row-blank-hole-id = { $file }의 { $row }행에 시추공 ID가 비어 있습니다
csv-geophysics-row-column-count = { $file }의 { $row }행에 열이 { $found }개 있습니다. 필요한 개수는 { $expected }개입니다
csv-geophysics-row-negative-depth = { $file }의 { $row }행에 음수 심도가 있습니다
csv-geophysics-row-no-depth = { $file }의 { $row }행에 읽을 수 있는 심도가 없습니다
csv-geophysics-file-s-path-not-valid = 파일 경로가 올바른 UTF-8이 아니어서 프로젝트에 저장할 수 없습니다. 파일 또는 폴더 이름을 바꾼 후 다시 연결하세요
csv-geophysics-rows-skipped = { $file }: 행 { $rows }개 중 { $skipped }개를 읽을 수 없습니다. 이유는 콘솔에 있습니다
csv-geophysics-runs-not-grouped = 시추공 { $count }개의 물리탐사 데이터가 시추공별로 그룹화되지 않고 둘 이상의 구간으로 나뉘어 있습니다. 이후 구간은 해당 시추공에 측정값이 없는 심도만 추가합니다: { $holes }
csv-geophysics-linked-downhole-geophysics-from-file = { $file }에서 공내 물리탐사를 연결했습니다: 시추공 { $holes }개, 곡선 { $curves }, 읽은 행 { $rows }개, 건너뛴 행 { $skipped }개. 측정값은 파일에 남아 있으며 시추공 단위로 읽습니다
csv-geophysics-no-readings = 측정값 없음
csv-geophysics-no-usable-depth-step = 사용 가능한 심도 간격 없음
csv-geophysics-run-count-mismatch = { $hole }의 구간 { $read }개를 읽었지만 링크에는 { $runs }개가 있습니다
csv-geophysics-rows-geophysics-row-s-count = 데이터 세트에 정의되지 않은 시추공 { $count }개의 물리탐사 행 { $rows }개는 연결되지 않았습니다: { $holes }
csv-geophysics-rows-readings-would-need-samples = 측정값 { $rows }개에 샘플 { $samples }개가 필요합니다
csv-geophysics-run-hole-curve-was-not = { $hole } { $curve }의 구간이 유지되지 않았습니다({ $reason })
data-table-copy-selection = 선택 항목 복사
data-table-copy-table = 표 복사
drill-hole-add = 추가
drill-hole-add-all = 모두 추가

## Drill strings

drill-hole-add-stop = 색상 단계 추가
drill-hole-add-working-section = 작업 구간 추가
drill-hole-all-rendered-intervals-opaque-white = 렌더링된 모든 구간이 불투명한 흰색입니다.
drill-hole-another-working-section-field-has = 이 필드의 다른 작업 구간이 해당 이름을 사용 중입니다.
drill-hole-assumed = 가정값
drill-hole-burden-spacing-must-greater-than = 저항선과 간격은 0보다 커야 합니다
drill-hole-cache-drill-hole-set-name-has = 시추공 세트 { $name }에 시추공 및 연결이 { $count }개 있어 선택 강조 표시가 처리할 수 있는 { $capacity }개를 초과합니다. 세트 전체를 선택하면 여전히 강조 표시되지만, 개별 시추공을 선택하면 강조 표시되지 않습니다
drill-hole-cache-drill-hole-set-name-stations = 시추공 세트 { $name }: 측점 { $stations }개, 세그먼트 { $before }개를 { $after }개로 병합, 셀 { $cells }개
drill-hole-choose-valid-closed-polyline = 유효한 닫힌 폴리라인을 선택하세요
drill-hole-clear-filter = 필터 지우기
drill-hole-code-already-in-section = { $code }은(는) 이미 작업 구간 { $section }에 있습니다.
drill-hole-code-outside-section-has-name = 이 구간 밖의 코드가 해당 이름을 사용 중입니다. 구간은 자신이 포함한 코드와만 이름을 공유할 수 있습니다.
drill-hole-colour-scale = 색상 스케일
drill-hole-count-codes = 코드 { $count }개
drill-hole-count-codes-interval-no-logged = 코드 { $count }개. 기록된 값이 없는 구간은 흰색으로 유지됩니다.
drill-hole-disc-diameter = 디스크 직경
drill-hole-appearance-title = 시추공 모양: { $name }
drill-hole-drilled-diameter = 천공 직경 대비
drill-hole-every-code-lists-already-another = 나열된 모든 코드가 이미 다른 작업 구간에 있습니다.
drill-hole-every-interval-value-colour-field = 색상 필드에 값이 있는 모든 구간은 시추공 선 위에 이 너비의 디스크로 그려집니다. 멀리서도 몇 픽셀보다 좁아지지 않습니다.
drill-hole-field = 필드
drill-hole-field-working-section = 작업 구간별 { $field }
drill-hole-floor = 하반
drill-hole-grayscale = 그레이스케일
drill-hole-green-yellow-red = 녹색–노랑–빨강
drill-hole-heat = 열지도색
drill-hole-drilled-width-help = 천공 너비로 표시한 시추공은 지질 옆의 파이프처럼 보이고, 수천 개의 집합은 매트처럼 보입니다.
drill-hole-line-width-help = 시추공 자체는 모든 확대/축소 수준에서 이 너비의 선으로 그려집니다.
drill-hole-however-far-eye-hole-drawn = 시점이 얼마나 멀어도 시추공은 최소한 이 너비로 그려집니다.
drill-hole-measured = 측정값
drill-hole-name-working-section = { $name }(작업 구간)
drill-hole-never-thinner-than = 최소 두께
drill-hole-new-section-name = 새 구간 이름
drill-hole-new-working-section = 새 작업 구간
drill-hole-no-holes-fit-inside-boundary = 현재 저항선과 간격으로는 이 경계 안에 발파공이 들어가지 않습니다
drill-hole-part-code = 코드의 일부
drill-hole-pattern-too-many-holes = 패턴이 최대 { $maximum }개 발파공을 초과합니다. 저항선 또는 간격을 늘리세요
drill-hole-preset = 프리셋
drill-hole-px = px
drill-hole-rainbow = 무지개색
drill-hole-rename-out-of-sequence-hole = { $from }을(를) { $to }(으)로 바꾸면 이 시추공에서 층서 열의 순서를 벗어납니다.
drill-hole-rename-out-of-sequence-holes = { $from }을(를) { $to }(으)로 바꾸면 시추공 { $count }개에서 층서 열의 순서를 벗어납니다.
drill-hole-rename-out-of-sequence-note = 역전되거나 반복된 지층은 순서를 벗어나므로 이름 바꾸기는 막지 않습니다. 확인을 누르면 그대로 이름을 바꾸고, 취소를 누르면 이름 바꾸기로 돌아갑니다.
drill-hole-rename-out-of-sequence-title = 순서 벗어남
drill-hole-rename-seam-every-hole-of = 다음 데이터의 모든 시추공
drill-hole-rename-seam-hole = 시추공
drill-hole-rename-seam-holes = 시추공 수
drill-hole-rename-seam-horizon-intervals = 이 층준의 구간
drill-hole-rename-seam-intervals = 구간
drill-hole-rename-seam-logged-name-kept = 기록된 이름은 유지되며, 새 이름은 정정으로 제안됩니다.
drill-hole-rename-seam-reason = 사유
drill-hole-rename-seam-reason-hint = 이름을 바꾸는 이유
drill-hole-rename-seam-seam = 탄층
drill-hole-rename-seam-title = 탄층 이름 바꾸기
drill-hole-reset-colours = 색상 재설정
drill-hole-reset-preset = 프리셋 재설정
drill-hole-reset-shown-colours = 표시된 색상 재설정
drill-hole-roof = 상반
drill-hole-rotation-offsets-must-contain-valid = 회전 및 오프셋에는 올바른 숫자가 있어야 합니다
drill-hole-selected-polyline-has-no-usable = 선택한 폴리라인에 사용 가능한 XY 면적이 없습니다
drill-hole-shift-names-depths-kept = 이름만 이동하고 심도는 바뀌지 않습니다. 기록된 이름은 유지되며, 각 새 이름은 정정으로 제안됩니다.
drill-hole-shift-names-down-from-here-title = 여기서부터 이름 아래로 이동
drill-hole-shift-names-down-title = 이름 아래로 이동
drill-hole-shift-names-field = 필드
drill-hole-shift-names-from-here-note = 클릭한 층준과 그쪽에 있는 이름이 시추공을 따라 한 칸씩 밀리고, 반대쪽 이름은 그대로 남습니다. 클릭한 층준은 이름을 바꿀 때까지 알 수 없음을 뜻하는 UNK로 지정됩니다.
drill-hole-shift-names-moved = 이동한 이름
drill-hole-shift-names-not-in-column = 층서 열에 없어 그대로 둠
drill-hole-shift-names-reason-hint = 이름을 이동하는 이유
drill-hole-shift-names-submit = 이동
drill-hole-shift-names-unknown = UNK로 지정
drill-hole-shift-names-unknown-note = 시추공의 이름이 시추공을 따라 한 칸씩 밀립니다. 밀려 간 자리에 층서 열의 이름이 없으면 그 구간은 이름을 바꿀 때까지 알 수 없음을 뜻하는 UNK로 지정됩니다. 구간은 그대로 남고, 이름은 정정으로 제안됩니다.
drill-hole-shift-names-up-from-here-title = 여기서부터 이름 위로 이동
drill-hole-shift-names-up-title = 이름 위로 이동
drill-hole-shown-total-codes-shown = 코드 { $total }개 중 { $shown }개 표시됨
drill-hole-shown-total-rows-shown = 행 { $total }개 중 { $shown }개 표시됨
drill-hole-smooth-interpolation = 부드러운 보간
drill-hole-spacing-would-scan-too-many = 이 간격은 너무 많은 그리드 셀을 스캔합니다. 저항선 또는 간격을 늘리세요(최대 { $maximum }개 발파공)
drill-hole-square = 정방형
drill-hole-staggered = 엇갈림
drill-hole-stepped-bands = 단계별 색상 밴드
drill-hole-string-discs = 선과 디스크
drill-hole-string-discs-where-intervals-overlap = 선과 디스크로 표시할 때 구간이 겹치는 곳에서는 가장 짧은 구간이 디스크로 그려집니다.
drill-hole-string-width = 선 너비
drill-hole-style = 스타일
drill-hole-suggested-from-code-names-count = 코드 이름에서 제안됨({ $count })
common-times-sign = ×
common-minus-sign = −
drill-hole-ticked-but-hidden-filter-count = 선택했지만 필터로 숨겨짐: { $count }
drill-hole-true-diameter = 실제 직경
drill-hole-unsupported-drillhole-source = 지원되지 않는 시추공 데이터 원본
drill-hole-width = 너비
drill-hole-working-section-needs-name = 작업 구간에는 이름이 필요합니다.
drill-hole-working-section-set-seams-plies = 작업 구간은 하나의 단위로 채굴되는 탄층 또는 층리의 집합입니다. 이 구간으로 색상을 지정하면 전체 집합에 하나의 색상이 지정됩니다.
drill-hole-working-sections = 작업 구간
drill-pattern-arrangement = 배열
drill-pattern-axis-offset = { $axis } 오프셋
drill-pattern-blast-shape = 발파구역 형상
drill-pattern-burden = 저항선
drill-pattern-choose-closed-blast-boundary-then = 닫힌 발파 경계를 선택한 다음 그리드를 조정하세요. 발파공은 뷰포트에서 실시간으로 업데이트됩니다.
drill-pattern-closed-design-polyline-whose-xy = XY 영역이 발파공으로 채워질 닫힌 설계 폴리라인입니다.
drill-pattern-rotation-help = 전역 { $axis } 축을 기준으로 한 반시계 방향 패턴 회전입니다.
drill-pattern-distance-between-holes-along-each = 패턴 각 행을 따른 발파공 간 거리입니다.
drill-pattern-name-hint = 예: West Cut 03
drill-pattern-diameter-help = 최종 공경입니다. 밀리미터 단위로 입력되며 생성되는 모든 발파공에 저장됩니다.
drill-pattern-hole-depth = 공심(발파공 깊이)
drill-pattern-hole-diameter = 공경
drill-pattern-move-over-closed-polyline-then = 닫힌 폴리라인 위로 이동한 다음 뷰포트에서 클릭하세요. Esc를 누르면 선택이 취소됩니다.
drill-pattern-name-help = 프로젝트에 생성될 시추공 데이터 세트의 이름입니다.
drill-pattern-none-picked = 선택되지 않음
drill-pattern-pattern-name = 패턴 이름
drill-pattern-spacing-help = 패턴 행 사이의 수직 거리입니다.
drill-pattern-pick = 선택
drill-pattern-preview-count-hole-s-diameter = 미리보기: 발파공 { $count }개 · 공경 { $diameter }mm · 공심 { $depth }m
drill-pattern-rotation = 회전
drill-pattern-shift-pattern-grid-along-global = 패턴 그리드를 발파 형상에 맞춰 자른 상태로 유지하면서 전역 { $axis } 축을 따라 이동합니다.
drill-pattern-spacing = 간격
drill-pattern-staggered-offsets-every-second-row = 엇갈림 배열은 한 행씩 건너뛰어 간격의 절반만큼 오프셋합니다.
drill-pattern-vertical-depth-below-each-collar = 각 공구 아래의 수직 깊이입니다.

## Dxf strings

dxf-block-nesting-too-deep = DXF 블록 중첩이 최대 깊이({ $depth })를 초과하여 '{ $name }'을(를) 건너뜁니다
dxf-circular-block-reference = DXF 순환 블록 참조가 감지되었습니다: '{ $name }'
dxf-undefined-layer = DXF 엔터티가 정의되지 않은 레이어 '{ $name }'을(를) 참조하여 '{ $fallback }'(으)로 가져왔습니다
dxf-import-budget-exceeded = DXF 가져오기가 { $what } 예산({ $limit })을 초과하여 나머지 형상은 건너뜁니다
dxf-insert-unknown-block = DXF INSERT가 알 수 없는 블록 '{ $name }'을(를) 참조합니다

## Edit strings

edit-absolute-length = 절대 길이
edit-absolute-rl = 절대 표고(RL)
edit-action = 작업
edit-angle = 각도
edit-delete-vertex-number = 정점 { $number } 삭제
edit-dip-help = 수평면 기준 각도이며, 아래쪽이 음수입니다: -90은 수직 발파공을 의미합니다.
edit-app-web-not-recommended-production = { $app } Web은 프로덕션 용도로 권장되지 않습니다. 데모 용도로만 사용하세요.
edit-application = 애플리케이션
edit-apply = 적용
edit-apply-pick-target = 적용 후 대상 선택
edit-axis-value = { $axis } 값
edit-azimuth = 방위각
edit-batter-angle = 사면 경사각 (°)
edit-azimuth-help = 발파공이 천공되는 방위이며, 그리드 북쪽 기준 시계 방향 각도입니다.
edit-bench-height = 벤치 높이
edit-benches = 벤치
edit-berm-width = 소단 폭
edit-bezier-curve = 베지어 곡선
edit-choose-layer = 레이어 선택
edit-measure-help = 입력한 값이 사면을 따른 거리, 수평 폭, 수직 높이 중 무엇을 나타내는지 선택하세요.
edit-choose-which-two-polyline-paths = 선택한 정점 사이에 있는 두 폴리라인 경로 중 교체할 것을 선택하세요. 길이는 표고와 곡선 모서리를 포함합니다.
edit-click-corner-closed-polyline = 닫힌 폴리라인의 모서리를 클릭하세요.
edit-click-open-closed-polyline-begin = 열린 또는 닫힌 폴리라인을 클릭하여 시작하세요.
edit-click-second-vertex-replacement-span = 교체 구간의 두 번째 정점을 클릭하세요.
edit-click-vertex-start-replacement-span = 정점을 클릭하여 교체 구간을 시작하세요.
edit-collide-triangulation = 삼각망과 충돌
edit-confirm-selection = 선택 확인
edit-control-point-1 = 조절점 1
edit-control-point-2 = 조절점 2
edit-copy = 복사
edit-corner-radius-limited-so-replacement = 모서리 반지름은 교체 구간이 인접 정점을 넘지 않도록 제한됩니다.
edit-create-new-layer = 새 레이어 생성
edit-create-new-project = 새 프로젝트 생성
edit-create-project = 프로젝트 생성
edit-delta-length-m-use = 길이 변화량(m, + 또는 - 사용)
edit-dip = 경사각
edit-direction = 방향
edit-distance = 거리
edit-distance-along-slope = 사면을 따른 거리
edit-download-free-native-version-our = 저희 웹사이트에서 무료 네이티브 버전을 다운로드하세요 ↗
edit-drill-hole = 시추공
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = 끝점
edit-enter-valid-elevation = 올바른 표고를 입력하세요.
edit-exit-slice = 슬라이스 종료
edit-finish-polyline = 폴리라인 완료
edit-generate-batter-berms = 사면-소단 생성
edit-height = 높이
edit-height-change = 높이 변화
edit-height-mode = 높이 모드
edit-horizontal-distance = 수평 거리
edit-horizontal-width-each-flat-berm = 연속된 사면 사이에 있는 각 평탄한 소단의 수평 폭입니다.
edit-hover-choose-which-end-move = 마우스를 올려 이동할 끝점을 고른 다음 클릭하여 확정하세요.
edit-insert-point-elevation = 표고에 점 삽입
edit-intersect = 교차
edit-kind-properties = { $kind } { $properties }
edit-layer-name = 레이어 이름
edit-load-project = 프로젝트 불러오기
edit-longest = 가장 긺
edit-m-s = m/s
edit-measure = 측정
edit-mit-license = MIT 라이선스
edit-mode = 모드
edit-move = 이동
edit-move-layer = 레이어로 이동
edit-move-which-end = 이동할 끝점 선택
edit-movement-speed-slice-when-using = 탐색 키를 사용할 때 슬라이스가 움직이는 속도입니다.
edit-moving-end-endpoint = 이동 중: 끝점
edit-moving-start-endpoint = 이동 중: 시작점
edit-new-length-m = 새 길이(m)
edit-new-project = 새 프로젝트
edit-number-complete-batter-berm-levels = 완전한 사면-소단 레벨 수입니다. 최대값은 지정된 형상을 유지할 수 있는 가장 깊은 레벨로 제한됩니다.
edit-bezier-segments-help = 선택한 두 정점 사이의 곡선을 근사하는 데 사용되는 선 세그먼트 수입니다.
edit-chamfer-segments-help = 둥근 모서리를 근사하는 데 사용되는 직선 세그먼트 수입니다. 직선 모따기는 1을 사용하세요.
edit-object = 객체
edit-offset-element = 오프셋 요소
edit-pick-side = 방향 선택
edit-pit = 채굴장
edit-project-name = 프로젝트 이름
edit-properties = 속성
edit-radius = 반지름
edit-recent = 최근
edit-relative = 상대값 (+/-)
edit-elevation-mode-help = 상대값은 모든 점에 수직 변화를 적용합니다. 절대 표고(RL)는 모든 점을 하나의 목표 표고로 투영합니다.
edit-remove-from-list = 목록에서 제거
edit-replace-path = 경로 바꾸기
edit-rotate = 회전
edit-rotation-speed-slice-when-using = Q와 E 키를 사용할 때 슬라이스의 회전 속도입니다.
edit-s = °/s
edit-segments = 세그먼트
edit-segments-lying-elevation-ignored = 이 표고에 있는 세그먼트는 무시됩니다.
edit-endpoint-help = 변경할 끝점을 선택하세요. 다른 끝점은 고정된 상태로 유지됩니다.
edit-selected-holes-point-different-ways = 선택한 발파공이 서로 다른 방향을 향하고 있습니다. 적용하면 모두 이 각도로 설정됩니다.
edit-selected-start-end-point-moves = 선택한 시작점 또는 끝점은 선 방향을 따라 이동합니다. 반대쪽 끝점은 고정된 상태로 유지됩니다.
edit-set-axis = { $axis } 설정
edit-shortest = 가장 짧음
edit-show-vertex-number-in-table = 표에서 정점 { $number } 보기
edit-slice-view = 슬라이스 뷰
edit-slope-angle-each-batter-face = 수평면 기준으로 측정한 각 사면의 경사각입니다.
edit-slope-angle-offset-positive-negative = 오프셋의 경사각입니다. 양수와 음수 각도는 측면으로 이동할 때 복사본을 원본보다 위 또는 아래로 이동시킵니다.
edit-speed = 속도
edit-start = 시작점
edit-stockpile = 적치장
edit-stop-generated-offset-where-its = 생성된 오프셋 경로가 보이는 삼각망과 처음 만나는 지점에서 멈춥니다.
edit-target-rl = 목표 표고(RL)
edit-text-colour-opacity = 텍스트 색상과 불투명도입니다.
edit-thickness-visible-slice-slab-centred = 개요 표시기를 중심으로 한 보이는 슬라이스 판의 두께입니다.
edit-translation-axis-help = 월드 { $axis } 축을 따른 이동 거리입니다.
edit-type = 유형
edit-type-direction-together-set-offset = 유형과 방향이 함께 오프셋 방향을 결정합니다. 채굴장+위쪽 및 적치장+아래쪽은 바깥쪽으로 확장하고, 채굴장+아래쪽 및 적치장+위쪽은 안쪽으로 축소합니다.
edit-bench-direction-help = 위쪽은 각 벤치를 벤치 높이만큼 올리고, 아래쪽은 내립니다. 이 동작은 오프셋 방향도 반전시킵니다(유형 참조).
edit-value-help = 이 값은 선택한 측정 방식과 높이 모드로 해석됩니다.
edit-vertical-rise-fall-each-bench = 다음 소단이 생성되기 전 각 벤치의 수직 상승 또는 하강 높이입니다.
edit-bezier-control-point-1-help = 첫 번째 베지어 조절점의 월드 X, Y, Z 좌표입니다.
edit-bezier-control-point-2-help = 두 번째 베지어 조절점의 월드 X, Y, Z 좌표입니다.

## Events strings

events-couldn-t-exit-error = 종료할 수 없습니다: { $error }
events-couldn-t-save-error = 저장할 수 없습니다: { $error }
events-set-elevation = 표고 설정
events-set-elevation-from-cursor-hit = 커서 히트 지점을 기준으로 표고를 Z { $z }(으)로 설정했습니다
events-tool-not-available-section-view = 이 도구는 슬라이스 뷰에서 사용할 수 없습니다

## Explorer strings

explorer-clear-active-triangulation-texture = 활성 삼각망 텍스처 지우기
explorer-delete-from-project = 프로젝트에서 삭제
explorer-discard-changes = 변경 사항 취소...
explorer-download = 다운로드
explorer-drape-over-surface = 표면에 투영
explorer-draped-over-surface = 표면에 투영됨
explorer-duplicate = 복제
explorer-empty-collection = 빈 컬렉션
explorer-face-colour = 면 색상
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    색상 변수 { $count }개
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    시추공 { $holes }개
    색상 필드 { $fields }개
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    점 { $count }개
explorer-raster-id =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = 불러오기
explorer-lock = 잠금
explorer-new-collection = 새 컬렉션
explorer-no-collection = 컬렉션 없음
explorer-select-all-objects = 모든 객체 선택
explorer-settings = 설정...
explorer-source-name = 출처: { $name }
explorer-unload = 언로드
explorer-unlock = 잠금 해제

## Files strings

files-automatic-colour = 자동 색상
files-automatic-rl-spacing = 자동 RL 간격
files-axis-scale-ratio = { $axis } 축척 비율
files-ok = 확인
files-reset-scale = 1×로 재설정
files-rl-grid-options = RL 그리드 옵션
files-rl-spacing = RL 간격
files-scales-z-distances-visually-without = 저장된 좌표는 변경하지 않고 시각적으로만 Z 거리를 조정합니다.
files-thickness = 두께
files-xy-grid-options = XY 그리드 옵션
geophysics-checking-geophysics-files = 물리탐사 파일 확인 중
geophysics-downhole-geophysics-name-could-not = '{ $name }'의 공내 물리탐사를 연결할 수 없습니다: { $error }
geophysics-file-changed = 인덱스를 만든 이후 물리탐사 파일이 변경되었습니다
geophysics-file-unreadable = '{ $name }'에 연결된 물리탐사 파일을 { $path }에서 읽을 수 없습니다({ $error }). 데이터 세트의 우클릭 메뉴에서 다시 연결하세요
geophysics-linked-changed-rereading = '{ $name }'에 연결된 물리탐사 데이터가 인덱스를 만든 이후 변경되어 다시 읽는 중입니다
geophysics-hole-has-size-mib-geophysics = { $hole }의 물리탐사 행이 { $size } MiB로, 시추공 하나를 읽을 수 있는 한도를 초과합니다
geophysics-hole-needs-size-mib-its = { $hole }의 물리탐사에 { $size } MiB가 필요하지만 브라우저의 남은 메모리가 부족합니다. 다른 항목을 언로드한 후 이 데이터 세트를 언로드했다가 다시 불러오세요
geophysics-linking-geophysics-name = { $name }에 물리탐사 연결 중
geophysics-reading-geophysics-hole = { $hole }의 물리탐사 읽는 중
geophysics-web-could-not-read-name-error = '{ $name }'을(를) 읽을 수 없습니다: { $error }
geophysics-web-name-used-session-s-downhole = '{ $name }'은(는) 이 세션의 공내 물리탐사에 사용됩니다

## Gpu strings

gpu-cache-block-model-surface-build-failed = 블록 모델 표면 생성 실패: { $error }
gpu-cache-block-model-surface-build-worker = 블록 모델 표면 생성 작업 스레드 연결이 끊어졌습니다
gpu-cache-block-model-surface-chunk-rejected = 블록 모델 표면 청크가 GPU 할당 전에 거부되었습니다: 인스턴스={ $instances } 바이트, 한계={ $limit } 바이트
gpu-cache-block-volume-worker-disconnected = 블록 볼륨 준비 작업 스레드 연결이 끊어졌습니다
gpu-cache-translucent-volume-could-not-built = 반투명 볼륨을 생성할 수 없어({ $error }) 이 블록 모델을 대신 큐브로 표시합니다.
gpu-cache-edge-chunk-rejected = 삼각망 모서리 청크가 GPU 할당 전에 거부되었습니다: 인스턴스={ $instances } 바이트, 한계={ $limit } 바이트
gpu-cache-triangulation-chunk-rejected = 삼각망 GPU 청크가 할당 전에 거부되었습니다: 정점={ $vertices } 바이트, 인덱스={ $indices } 바이트, 한계={ $limit } 바이트
gpu-cache-triangulation-too-many-vertices = 삼각망 '{ $name }'에 정점이 { $count }개 있습니다(> u32::MAX). GPU용으로 청크할 수 없습니다
gpu-cache-triangulation-uploaded = 삼각망 '{ $name }'을(를) 공간 청크 { $chunks }개로 업로드했습니다(면 { $faces }개)
i18n-active-language = 현재 언어: { $language }(번들 포함: { $bundled })
i18n-could-not-select-language-error = 언어를 선택할 수 없습니다: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = GPU 어댑터: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = GPU 드라이버: { $driver } { $driver_info }
init-gpu-limits-max-buffer-size = GPU 한도: max_buffer_size={ $max_buffer_size } MiB, max_storage_buffer_binding_size={ $max_storage_buffer_binding_size } MiB, max_storage_buffers_per_shader_stage={ $max_storage_buffers_per_shader_stage }, max_uniform_buffer_binding_size={ $max_uniform_buffer_binding_size } KiB, max_texture_dimension_2d={ $max_texture_dimension_2d }, max_bind_groups={ $max_bind_groups }
init-gpu-supports-maximum-buffer-size = GPU가 지원하는 최대 버퍼 크기는 { $size } MiB입니다. 큰 장면은 완전히 표시되지 않을 수 있습니다
init-surface-present-mode = 표면 표시 모드: { $mode }
init-wgpu-error-continuing-error = wgpu 오류(계속 진행): { $error }
input-could-not-read-name-error = { $name }을(를) 읽을 수 없습니다: { $error }
input-could-not-slice-name-error = { $name }을(를) 슬라이스할 수 없습니다: { $error }
io-add-collar-file-explicit-segments = 공구 파일(또는 명시적 구간 파일)을 추가하세요. 공내 물리탐사는 해당 파일이 정의한 시추공에 연결됩니다.

## Io strings

io-ascii-points-xyz-pts = ASCII 포인트(.xyz, .pts)
io-attribute = 속성
io-blank-header = (빈 헤더)
io-block-model = 블록 모델:
io-choose-file-purpose-map-its = 열을 매핑할 파일 용도를 선택하세요.
io-choose-loaded-block-model = 로드된 블록 모델 선택
io-choose-loaded-dataset = 로드된 데이터 세트 선택
io-choose-loaded-layer = 로드된 레이어 선택
io-choose-loaded-triangulation = 로드된 삼각망 선택
io-choose-purpose = 용도 선택…
io-choose-source-file-files-import = 가져올 원본 파일을 선택하세요.
io-collar = 공구
io-column-mapping = 열 매핑
io-comma-separated-values-csv = Comma-Separated Values (.csv)
io-csv-files = CSV 파일
io-dataset = 데이터 세트:
io-default = 기본값
io-density-read-g-cc-exported = 밀도이며 내보낸 그대로 g/cc로 읽습니다. 중앙값이 0.5~5 g/cc 사이가 아닌 곡선은 단위가 잘못된 것으로 보여 경고와 함께 가져오기에서 제외됩니다.
io-depth = 깊이
io-diameter = 직경
io-downhole-geophysics = 공내 물리탐사
io-drawing-exchange-format-dxf = Drawing Exchange Format (.dxf)
io-drill-holes = 시추공
io-east-x = 동거리 / X
io-elevation-z = 표고 / Z
io-end-x = 끝 X
io-end-y = 끝 Y
io-end-z = 끝 Z
io-explicit-segments = 명시적 세그먼트
io-export = 내보내기
io-export-csv-block-model = CSV 블록 모델 내보내기
io-export-csv-drillholes = CSV 시추공 내보내기
io-export-dxf = DXF 내보내기
io-export-one-layer = 레이어 하나 내보내기
io-export-open-mining-format-2 = Open Mining Format 2 내보내기
io-export-ply = PLY 내보내기
io-export-stl = STL 내보내기
io-export-wavefront-obj = Wavefront OBJ 내보내기
io-gamma-api = 감마 (API)
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = 가져오기
io-import-ascii-point-cloud = ASCII 포인트 클라우드 가져오기
io-import-drillhole-csv-bundle = 시추공 CSV 묶음 가져오기
io-import-geotiff = GeoTIFF 가져오기
io-import-las-laz-point-cloud = LAS/LAZ 포인트 클라우드 가져오기
io-import-open-mining-format-2 = Open Mining Format 2 가져오기
io-import-pcd-point-cloud = PCD 포인트 클라우드 가져오기
io-import-ply = PLY 가져오기
io-import-stl = STL 가져오기
io-import-wavefront-obj = Wavefront OBJ 가져오기
io-inclination = 경사
io-interval = 구간
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-long-spaced-density-g-cc = 장간격 밀도 (g/cc)
io-mapped-csv-bundle-csv = 매핑된 CSV 묶음 (.csv)
io-measured-depth-down-hole-read = 시추공을 따라 측정한 심도이며 미터로 읽습니다. Incline은 단위를 변환하지 않으며, 파일을 내보낸 데이터베이스가 단위를 결정합니다.
io-model-file = 모델 파일
io-name-count-files = { $name } + 파일 { $count }개
io-natural-gamma-read-api-units = 자연 감마이며 내보낸 그대로 API 단위로 읽습니다.
io-no-csv-chosen = 선택된 .csv 파일 없음
io-no-csv-files-chosen = 선택된 CSV 파일 없음
io-no-dxf-chosen = 선택된 .dxf 파일 없음
io-no-omf-chosen = 선택된 .omf 파일 없음
io-north-y = 북거리 / Y
io-open-mining-format-2-omf = Open Mining Format 2 (.omf)
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-projects = 프로젝트
io-short-spaced-density-g-cc = 단간격 밀도 (g/cc)
io-source-file = 원본 파일
io-start-x = 시작 X
io-start-y = 시작 Y
io-start-z = 시작 Z
io-stl = STL (.stl)
io-triangulation = 삼각망:
io-unmapped = 매핑되지 않음
io-wavefront-obj = Wavefront OBJ (.obj)
io-writes-three-files-beside-name = 선택한 이름 옆에 파일 3개를 기록합니다: 이 대화상자에서 가져오는 열 형식의 공구, 측량, 구간 파일입니다.

## Jobs strings

jobs-background-task-poll-label-ended = 백그라운드 작업 '{ $poll_label }'이(가) 결과 없이 종료되었습니다
jobs-cancelled-label-its-project-no = '{ $label }'을(를) 취소했습니다. 해당 프로젝트가 더 이상 활성 상태가 아닙니다
jobs-discarded-stale-result = 원본이 변경되거나 닫혀 '{ $poll_label }'의 오래된 백그라운드 결과를 버렸습니다
jobs-drillhole-import = 시추공 가져오기
log-traces-auto-from-hole = 자동, 이 시추공 기준
log-traces-curve-no-reading = { $curve }: 측정값 없음
log-traces-curve-value-unit = { $curve }: { $value } { $unit }
log-traces-custom-range = 사용자 지정 범위
log-traces-default-colour = 기본 색상
log-traces-density-scale = 밀도 스케일
log-traces-depth-m = { $depth } m
log-traces-gamma = 감마
log-traces-gamma-colour = 감마 색상
log-traces-gamma-scale = 감마 스케일
log-traces-percentile-range-no-data = 이 시추공의 1~99번째 백분위수를 바깥쪽으로 반올림한 값입니다. 이 시추공에는 아직 데이터가 없습니다.
log-traces-percentile-range = 이 시추공의 1~99번째 백분위수를 바깥쪽으로 반올림한 값: { $range }.
log-traces-long-density = 장간격 밀도
log-traces-long-density-colour = 장간격 밀도 색상
log-traces-min-max-unit = { $min } ~ { $max } { $unit }
log-traces-reading = 읽는 중...
log-traces-short-density = 단간격 밀도
log-traces-short-density-colour = 단간격 밀도 색상

## Logging strings

logging-activity-completed = 활동 완료됨
logging-activity-started = 활동 시작됨
logging-application-id-id = 애플리케이션 ID: { $id }
logging-application-name = 애플리케이션 이름: { $name }
logging-application-startup = 애플리케이션 시작
logging-build-target-os-architecture = 빌드 대상: { $os }-{ $architecture }
logging-completed = 완료됨
logging-count-messages = 메시지 { $count }개
logging-desktop-session-xdg-session-type = 데스크톱 세션: XDG_SESSION_TYPE={ $session }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Incline Design 초기화 중
logging-locale-environment = 로케일 환경: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session = macOS 세션: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = 운영체제: GNU / Linux
logging-operating-system-macos = 운영체제: macOS
logging-operating-system-microsoft-windows = 운영체제: Microsoft Windows
logging-pointer-width = 포인터 폭: { $width }비트
logging-process-id-id = 프로세스 ID: { $id }
logging-release-version = 릴리스 버전: { $version }
logging-renderer = 렌더러
logging-rust-compiler-host = Rust 컴파일러 호스트: { $host }
logging-system = 시스템
logging-system-error = 시스템 오류
logging-unknown = 알 수 없음
logging-windows-session-sessionname-session = Windows 세션: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = 처리 중…

## Mac strings

mac-cannot-install-macos-menu-bar = 메인 스레드가 아닌 곳에서는 macOS 메뉴 바를 설치할 수 없습니다
mac-quit-app = { $app } 종료

## Main strings

main-incline-design-web-startup-failed = Incline Design Web 시작 실패: { $error }

## Menu strings

menu-count-files-selected = 파일 { $count }개 선택됨
modelling-settings-modelling-settings = 모델링 설정
modelling-settings-thin-plate-spline-exact = 박판 스플라인, 정확
modelling-settings-anisotropic-spline = 이방성 스플라인
modelling-settings-auto-axis-spline = 자동 축 스플라인
modelling-settings-surface-method = 표면 방법
modelling-settings-hermite-dips = 경사각을 쓰는 에르미트(경사각 측정값 필요)
modelling-settings-steep-pair-distance = 급경사 쌍 거리
modelling-settings-steep-pair-distance-help = 평면상에서 이 거리보다 가깝고 아래 각도보다 급한 점 쌍은 구축이 성공할 때 표시됩니다. 거부하거나 수정하지는 않습니다.
modelling-settings-steep-pair-angle = 급경사 쌍 각도
modelling-settings-surface-help = 표면 만들기가 그리드를 그리는 방식입니다. 프로젝트 수준 설정이며 프로젝트와 함께 저장됩니다.

## Object strings

object-edit-appearance = 외관
object-edit-arc-circle = 호 및 원
object-edit-arc-segments = 호 세그먼트
object-edit-bulge = 벌지
object-edit-bulge-arcs-horizontal-data-model = 벌지 호는 데이터 모델상 수평입니다. 호는 평면상에서 돌지만, 표고는 한 정점에서 다음 정점까지 직선으로 이어집니다.
object-edit-centre-x = 중심 X
object-edit-centre-y = 중심 Y
object-edit-centre-z = 중심 Z
object-edit-chord = 현
object-edit-colour-layer = 레이어별 색상
object-edit-enter-number = 숫자를 입력하세요
object-edit-follow-owning-layer-s-colour = 이 객체에 고정된 색상 대신 소속 레이어의 색상을 따릅니다.
object-edit-id = ID
object-edit-identity = 식별 정보
object-edit-insert-after = 뒤에 삽입
object-edit-join-last-vertex-back-first = 마지막 정점을 첫 번째 정점에 다시 연결합니다.
object-edit-length = 길이 { $length }m
object-edit-move-down = 아래로 이동
object-edit-move-up = 위로 이동
object-edit-object-has-no-arc-segments = 이 객체에는 호 세그먼트가 없습니다.
object-edit-object-has-single-position = 이 객체는 위치가 하나뿐입니다.
object-edit-object-needs-least-required-vertices = 이 객체는 최소 { $required }개의 정점이 필요합니다
object-edit-one-more-properties-not-valid = 속성 중 하나 이상이 올바른 숫자가 아닙니다
object-edit-perimeter-area = 둘레 { $length }m, 면적 { $area }m²
object-edit-reverse = 반전
object-edit-row-invalid-number = 행 { $row }: 위치 또는 벌지가 올바른 숫자가 아닙니다
object-edit-sweep = 스윕
object-edit-text-not-number = "{ $text }"은(는) 숫자가 아닙니다
object-edit-vertices = 정점

## Omf strings

omf-element-name-has-count-tie = 요소 '{ $name }'에 더 이상 포함되지 않는 발파공을 지정하는 연결 { $count }개가 있습니다
omf-element-name-has-count-unreadable = 요소 '{ $name }'에 읽을 수 없는 작업 구간이 { $count }개 있어 제외했습니다
omf-element-unsupported-section = 요소 '{ $name }'이(가) 이 빌드에서 해당 종류의 항목을 표시할 수 없는 섹션 '{ $section }'을(를) 지정합니다
omf-element-name-names-unknown-section = 요소 '{ $name }'이(가) 알 수 없는 섹션 '{ $section }'을(를) 지정합니다
omf-ignoring-colour-map-omf-attribute = OMF 속성 '{ $attribute }'의 색상 맵을 무시합니다: { $error }
omf-mining-data-exported-incline = Incline에서 내보낸 마이닝 데이터
omf-import = OMF 가져오기
omf-texture = OMF 텍스처
omf-validation-warnings = OMF 유효성 검사 경고: { $warnings }
omf-application-metadata-dropped = 프로젝트 애플리케이션 메타데이터 '{ $application }'은(는) 유지되지 않습니다
omf-project-author-not-retained = 프로젝트 작성자는 유지되지 않습니다
omf-project-description-not-retained = 프로젝트 설명은 유지되지 않습니다
omf-unsupported-metadata-keys = 프로젝트에 지원되지 않는 메타데이터 키가 있습니다: { $keys }
omf-skipped-drillhole-data-saved-older = 이전 레이아웃으로 저장된 시추공 데이터({ $names })를 건너뛰었습니다. 원본 파일에서 다시 가져오세요
omf-modelling-settings-unreadable = 프로젝트 모델링 설정을 읽을 수 없어 기본값을 사용합니다

## Plot strings

plot-1-1000-one-millimetre-sheet = 1:1000 축척에서는 도면의 1밀리미터가 실제 지면의 1미터에 해당합니다.
plot-1-scale-covers-width-height = 1:{ $scale } · { $width } × { $height } m 커버
plot-all-visible-data = 보이는 모든 데이터
plot-automatic-grid-interval = 자동 그리드 간격
plot-border = 테두리
plot-centre = 중심 위치
plot-fit-scale-help = 보이는 모든 내용이 도면에 들어가는 가장 작은 표준 축척을 선택하세요.
plot-coordinate-grid = 좌표 그리드
plot-current-view-centre = 현재 뷰 중심
plot-date-caps = 날짜
plot-date = 날짜
plot-dots-per-inch-paper-size = 인치당 도트 수입니다. 이 용지 크기는 최대 { $max_dpi } dpi까지 래스터화할 수 있으며, 300dpi가 일반적인 인쇄 품질입니다.
plot-dpi = dpi
plot-drawing-no = 도면 번호
plot-drawing-number = 도면 번호
plot-drawn-by-caps = 작성자
plot-drawn-by = 작성자
plot-e-g-example-gold-project = 예: Example Gold Project
plot-entered-coordinates = 입력한 좌표
plot-export-png = PNG 내보내기...
plot-fit-scale-visible-data = 보이는 데이터에 축척 맞추기
plot-grid-interval = 그리드 간격
plot-landscape = 가로 방향
plot-lists-visible-surfaces-design-layers = 보이는 표면과 설계 레이어를 색상과 함께 나열합니다.
plot-margin = 여백
plot-margins-leave-no-room-map = 여백이 지도가 들어갈 공간을 남기지 않습니다
plot-metres-scale-1-scale = 미터    축척 1:{ $scale }
plot-mm = mm
plot-north-arrow = 방위 표시(북쪽 화살표)
plot-nothing-visible-draw = 그릴 수 있는 보이는 내용 없음
plot-paper = 용지
plot-paper-orientation-width-height-mm = { $paper } { $orientation } · { $width } × { $height } mm
plot-paper-size = 용지 크기
plot-pick-interval-reads-roughly-every = 인쇄된 도면에서 약 50mm마다 나타나는 간격을 선택하세요.
plot-plan = 평면도
plot-scale-must-be-positive = 도면 축척은 양수여야 합니다
plot-png-written-sheet-s-exact = PNG는 도면의 정확한 용지 크기로 기록되고 DPI를 함께 저장하여 실제 축척으로 인쇄됩니다.
plot-portrait = 세로 방향
plot-resolution = 해상도
plot-rev = 개정
plot-revision = 개정
plot-scale = 축척
plot-scale-ratio = 축척  1:
plot-scale-framing = 축척 및 프레이밍
plot-sheet-furniture = 도면 요소
plot-size-width-height-mm = { $size }({ $width } × { $height } mm)
plot-subtitle = 부제목
plot-title = 제목
plot-title-block = 표제란
plot-today = 오늘
point-cloud-classify = 분류
point-cloud-classify-vegetation = 식생 분류
point-cloud-cloth-resolution = 천 해상도
point-cloud-cloth-resolution-about-one-half = 선택한 클라우드 중 가장 성긴 점 간격의 약 1.5배인 천 해상도로, 모든 입자 아래에 반환점이 있도록 합니다.
point-cloud-combine-selected-point-clouds-into = 선택한 포인트 클라우드를 하나의 새 클라우드로 합쳐, 전체에 걸쳐 하나의 삼각망을 만들 수 있게 합니다. 점별 색상은 유지되며, 색상이 없는 클라우드는 표시 색상을 사용합니다.
point-cloud-selected-count = { $count }개 선택됨 · 점 { $points }개
point-cloud-delete-selected-clouds-from-project = 결합이 완료되면 선택한 클라우드를 프로젝트에서 삭제하여 중복 사본이 차지할 메모리를 확보합니다.
point-cloud-flat-pads-structures = 평탄 (패드, 구조물)
point-cloud-ground-cloud-covers-steep-follows = 클라우드가 덮는 지면입니다. 급경사는 벽을 상단부터 따라 내려가며, 평탄은 더 뻣뻣한 천을 사용해 큰 건물과 설비는 건너뛰지만 급격한 단절은 둥글게 만듭니다.
point-cloud-ground-threshold = 지면 임계값
point-cloud-how-far-around-each-point = 각 점 주변에서 이웃을 셀 거리입니다.
point-cloud-join = 결합
point-cloud-let-cloth-follow-walls-down = 천의 뻣뻣함 때문에 면에서 떠 있는 곳에서도 천이 벽을 상단부터 따라 내려가게 합니다. 설비가 밀집한 완만한 지면에서만 끄세요.
point-cloud-mark-each-point-ground-noise = 각 점을 지면, 노이즈 또는 미분류로 표시합니다. 천을 클라우드 아래에서 위로 눌러 지면에 안착시키며, 지면 임계값 이내의 점은 지면입니다. 기존 분류는 대체되며 실행 취소로 복원할 수 있습니다.
point-cloud-mark-isolated-returns-birds-dust = 지면을 찾기 전에 새, 먼지, 다중 경로 오류 같은 고립된 반환점을 노이즈로 표시하여 떨어진 낮은 점이 천을 끌어내리지 않게 합니다.
point-cloud-mark-noise = 노이즈 표시
point-cloud-minimum-neighbours = 최소 이웃 수
point-cloud-name-assigned-joined-point-cloud = 결합된 포인트 클라우드에 지정할 이름입니다.
point-cloud-name-count-points = { $name }(점 { $count }개)
point-cloud-noise-radius = 노이즈 반경
point-cloud-point-clouds = 포인트 클라우드
point-cloud-points-closer-than-settled-cloth = 안착한 천 표면에서 측정한 거리가 이보다 가까운 점은 지면입니다.
point-cloud-points-fewer-neighbours-than-within = 노이즈 반경 내 이웃이 이보다 적은 점은 노이즈입니다.
point-cloud-raise-cloth-resolution-if-your = 컴퓨터의 RAM이 적으면 천 해상도를 높이세요.
point-cloud-recommended = 권장
point-cloud-recover-steep-slopes = 급경사 복원
point-cloud-relief-dumps-rolling-ground = 기복 (배토장, 구릉지)
point-cloud-remove-sources = 원본 제거
point-cloud-resolution-m-points-spacing-m = { $resolution } m (점 간격 약 { $spacing } m)
point-cloud-selected-clouds-copied-into-joined = 결합된 클라우드로 복사될 선택한 클라우드입니다. 다른 세트를 결합하려면 대화상자를 닫으세요.
point-cloud-selected-clouds-each-classified-its = 각각 개별적으로 분류될 선택한 클라우드입니다. 다른 세트를 분류하려면 대화상자를 닫으세요.
point-cloud-classify-help = 각 점 주변의 점 형태를 읽는 학습된 분류기로 반환점을 분류합니다: 지면, 식생(높이에 따라 낮음(1 m 미만), 중간(3 m 미만), 높음으로 구분), 그 외 건물이나 설비 등은 미분류로 둡니다. 천만 사용하려면 끄세요.
point-cloud-spacing-cloth-s-particles-around = 천 입자의 간격입니다. 클라우드의 점 간격 정도가 적절한 시작점이며, 더 촘촘하면 지면을 더 정확히 따르지만 더 조밀한 점이 필요합니다.
point-cloud-steep-pit-walls-benches = 급경사 (채굴장 벽, 벤치)
point-cloud-terrain = 지형
point-cloud-use = 사용

## Products strings

products-add-initiation = 기폭점 추가
products-delay = 지연시간
products-delay-palette = 지연시간 팔레트
products-how-long-after-shot-fired = 발파가 시작된 후 이 공구가 기폭하기까지 걸리는 시간입니다.
products-initiation-name = 기폭 · { $name }
products-milliseconds-between-one-hole-firing = 한 발파공이 기폭한 후 다음 발파공이 기폭하기까지의 밀리초입니다.
products-ms = ms
products-no-products = 제품 없음
products-remove = 제거
products-update = 업데이트

## Progress strings

progress-percent-done-total = { $percent }({ $total } 중 { $done })
progress-task-finished = { $task }: 완료됨

## Project strings

project-item = 항목
project-steep-pair-distance-positive = 급경사 쌍 거리는 양수(미터)여야 합니다
project-steep-pair-angle-range = 급경사 쌍 각도는 0보다 크고 90도 이하여야 합니다
project-thin-plate-spline-exact = 박판 스플라인, 정확
project-method-steep-pairs-under = 방법: { $method } · { $distance } m 미만에서 { $degrees }도보다 급한 급경사 쌍

## Properties strings

properties-adds-view-dependent-rim-highlight = 블록 및 재질 경계에 시점에 따른 테두리 하이라이트를 추가합니다. 비활성화하면 볼륨 렌더링 작업량이 약간 줄어듭니다.
properties-block-model-downscale = 블록 모델 다운스케일
properties-camera = 카메라
properties-camera-clip-planes = 카메라 클립 평면
properties-cap-while-resizing = 크기 조정 시 제한
properties-colours-each-point-cloud-chunk = 각 포인트 클라우드 청크에 색상을 지정하고, 프러스텀 컬링에 사용되는 박스의 윤곽을 그리며, 지난 프레임에 그린 점 수를 상세 수준(LOD) 목표 및 보이는 전체 수와 함께 상태 표시줄에 표시합니다.
properties-colours-each-surface-chunk-outlines = 각 표면 청크에 색상을 지정하고, 프러스텀 컬링에 사용되는 박스의 윤곽을 그리며, 지난 프레임에 그린 면 수를 보이는 전체 수와 함께 상태 표시줄에 표시합니다.
properties-dark-mode = 다크 모드
properties-dataset = 데이터 세트
properties-developer = 개발자
properties-downscale-rasters = 래스터 다운스케일
properties-drillholes = 시추공
properties-edit-object = 객체 편집...
properties-field-view = 시야각
properties-fps = FPS
properties-frame-counter = 프레임 카운터
properties-frame-rate-cap = 프레임 속도 상한
properties-hz = Hz
properties-interface = 인터페이스
properties-invert-horizontal = 수평 반전
properties-invert-vertical = 수직 반전
properties-limits-newly-loaded-geotiff-previews = 새로 로드된 GeoTIFF 미리보기의 최장 변을 4096픽셀로 제한합니다. 비활성화하면 GPU 텍스처 한계까지 전체 해상도를 사용하지만 메모리를 더 많이 사용합니다.
properties-line-colour = 선 색상
properties-look-sensitivity = 시점 감도
properties-max-clip-span = 최대 클립 범위
properties-move-layer = 레이어로 이동...
properties-near-clip-limit = 근접 클립 한계
properties-no-drillhole-datasets-open = 열린 시추공 데이터 세트가 없습니다.
properties-orbit-sensitivity = 궤도 감도
properties-panel-chrome = 패널 외관
properties-performance = 성능
properties-plan-mode = 평면 모드
properties-point-cloud-chunk-debug-view = 포인트 클라우드 청크 디버그 뷰
properties-presents-step-display-no-tearing = 디스플레이 주사와 동기화되어 표시됩니다. 화면 찢김이 없으며 디스플레이가 프레임 속도를 결정합니다. 끄면 프레임이 그려지는 즉시 표시되며 아래의 상한이 적용됩니다.
properties-reflective-block-edges = 반사형 블록 테두리
properties-restore-defaults = 기본값 복원
properties-show-console = 콘솔 표시
properties-shows-live-near-far-projection = 상태 표시줄에 실시간 근/원 투영 거리를 표시합니다.
properties-snap-polling = 스냅 폴링
properties-surface-chunk-debug-view = 표면 청크 디버그 뷰
properties-vertical-sync = 수직 동기화
properties-world-axis-gizmo = 월드 축 표시기
properties-zoom-cursor = 커서로 확대/축소
properties-zoom-sensitivity = 확대/축소 감도
reference-points-count-holes-from-dataset = '{ $dataset }'의 시추공 { $count }개
reference-points-holes-from-datasets = 데이터 세트 { $datasets }개의 시추공 { $count }개
reference-points-holes = 시추공
reference-points-holes-points-placed-selected-when = 점을 배치할 시추공으로, 대화상자를 열 때 선택한 항목입니다. 다른 시추공을 선택하려면 대화상자를 닫으세요.
reference-points-make = 생성
reference-points-no-categorical-field = 범주형 필드 없음
reference-points-no-values = 값 없음
reference-points-one-point-per-hole-boundary = 해당 경계에서 시추공마다 점 하나를 새 레이어로 만듭니다. 구간을 두 번 가진 시추공은 최상단 점을 사용하며 표시됩니다.
reference-points-reference-points = 기준점
reference-points-side = 면
reference-points-working-section = 작업 구간
reference-points-working-section-field = 작업 구간 필드
reference-surface-controls = 제어선
reference-surface-extent = 범위
reference-surface-points-outside-extent-still-shape = 범위 밖의 점도 표면 형태에 영향을 주며, 표면만 범위에 맞춰 클립됩니다.
reference-surface-points-surface-built-from-selected = 표면을 만드는 데 사용할 점으로, 대화상자를 열 때 선택한 항목입니다. 다른 점을 선택하려면 대화상자를 닫으세요.
reference-surface-extent-help = 완성된 표면을 클립할 선택한 닫힌 선입니다. 그 밖의 점도 표면 형태에 영향을 줍니다.
reference-surface-selected-open-strings-surface-made = 표면이 통과하도록 할 선택한 열린 선으로, 대화상자를 열 때 선택한 항목입니다. 다른 선을 선택하려면 대화상자를 닫으세요.
reference-surface-grids-selected-points-plan-into = 선택한 점을 평면상에서 그리드화하여 새 표면을 만듭니다. 실행할 때마다 표면이 추가됩니다.
reference-surface-change-these-under-modelling-settings = 모델링, 설정에서 변경하세요
reference-surface-triangulates-selected-points-plan-in = 선택한 점을 평면상에서 삼각화하여 새 표면을 만듭니다. 실행할 때마다 표면이 추가됩니다.

## Screenshot strings

screenshot-could-not-encode-viewport-image = 뷰포트 이미지를 인코딩할 수 없습니다: { $error }
screenshot-could-not-map-viewport-screenshot = 뷰포트 스크린샷을 매핑할 수 없습니다: { $error }
screenshot-could-not-save-viewport-image = 뷰포트 이미지 { $path }을(를) 저장할 수 없습니다: { $error }
screenshot-downloaded-viewport-image-file-name = 뷰포트 이미지를 다운로드했습니다: { $file_name }
screenshot-saved-viewport-image-path = 뷰포트 이미지를 저장했습니다: { $path }
screenshot-viewport-image-download-failed-error = 뷰포트 이미지 다운로드 실패: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = BVH 면 인덱스 { $index }이(가) 메쉬 범위를 벗어나 퇴화 삼각형으로 대체합니다

## State strings

state-above = 이상
state-activate-project = 프로젝트 활성화
state-all-open-incline-design-data = 열려 있는 모든 Incline Design 데이터
state-apply-generated-rings = 생성된 링 적용
state-apply-selection = 선택 항목에 적용
state-rotate-by-azimuth-dip = 방위각 { $azimuth }°, 경사각 { $dip }°
state-rotate-to-azimuth-dip = 방위각 { $azimuth }°, 경사각 { $dip }°로
state-below = 이하
state-build-reference-points = 기준점 만들기
state-centre-rotation = 회전 중심
state-checking-unsaved-work = 저장되지 않은 작업 확인 중
state-choose-destination = 대상 위치 선택
state-choose-one-more-files = 파일을 하나 이상 선택하세요
state-clear-raster = 래스터 지우기
state-click-pit-shell-viewport = 뷰포트에서 채굴장 쉘을 클릭하세요.
state-click-pit-stockpile-solid-viewport = 뷰포트에서 채굴장 또는 적치장 솔리드를 클릭하세요.
state-click-surface-viewport = 뷰포트에서 표면을 클릭하세요.
state-click-topology-viewport = 뷰포트에서 지형면을 클릭하세요.
state-close-project = 프로젝트 닫기
state-colour-drillholes = 시추공 색상
state-colour-drillholes-working-section = 작업 구간별 시추공 색상 지정
state-colour-points-classification = 분류별 점 색상 지정
state-copy-objects-layer = 객체를 레이어로 복사
state-count-cloud-s = 클라우드 { $count }개
state-count-file-s = 파일 { $count }개
state-count-object-s-axis-value = 객체 { $count }개 · { $axis } { $value }
state-count-object-s-closed = 객체 { $count }개 · { $closed }
state-count-object-s-layer = 객체 { $count }개 · { $layer }
state-count-object-s-weight = 객체 { $count }개 · { $weight }
state-count-object-s-z-elevation = 객체 { $count }개 · Z { $elevation }
state-points-controls-clipped = 점 { $count }개 · 제어선 { $controls }개 · 범위선으로 클립됨
state-points-controls-outline = 점 { $count }개 · 제어선 { $controls }개 · 점들의 외곽선으로 클립됨
state-points-controls-unclipped = 점 { $count }개 · 제어선 { $controls }개 · 클립 안 됨
state-create-collection = 컬렉션 생성
state-create-point-cloud-tin = 포인트 클라우드 TIN 생성
state-create-project = 프로젝트 생성
state-current-project = 현재 프로젝트
state-cut-topology-pit-shell = 채굴장 쉘로 지형면 절단
state-cut-triangulation-polyline = 폴리라인으로 삼각망 절단
state-cut-triangulation-z = Z값으로 삼각망 절단
state-dark-mode = 다크 모드
state-data-ticked-export-checklist = 내보내기 체크리스트에서 선택한 데이터
state-detached = 분리됨
state-disabled = 비활성화됨
state-discard-project-changes = 프로젝트 변경 사항 취소
state-discard-replace-project = 변경 사항 취소 및 프로젝트 교체
state-discarding-unsaved-changes = 저장되지 않은 변경 사항 취소 중
state-docked = 도킹됨
state-drape-raster = 래스터 투영
state-drill-pattern = 천공 패턴
state-duplicate-layer = 레이어 복제
state-east = 동
state-enabled = 활성화됨
state-exit-incline-design = Incline Design 종료
state-export-block-model-csv = 블록 모델 CSV 내보내기
state-export-drillhole-csv = 시추공 CSV 내보내기
state-export-layer-dxf = 레이어를 DXF로 내보내기
state-export-omf = OMF 내보내기
state-export-project-dxf = 프로젝트를 DXF로 내보내기
state-export-triangulation = 삼각망 내보내기
state-export-viewport-image = 뷰포트 이미지 내보내기
state-finish-closed-polyline = 닫힌 폴리라인 완료
state-finish-open-polyline = 열린 폴리라인 완료
state-fit-extents = 범위에 맞춤
state-fix-release-centre-both-views = 두 뷰가 궤도 회전하는 중심을 고정하거나 해제합니다
state-folder-section = { $section }의 { $folder }
state-generate-contours = 등고선 생성
state-hidden = 숨겨짐
state-import-drillholes = 시추공 가져오기
state-import-omf = OMF 가져오기
state-import-point-cloud = 포인트 클라우드 가져오기
state-import-raster = 래스터 가져오기
state-import-triangulation = 삼각망 가져오기
state-insert-intersection-points = 교차점 삽입
state-insert-points-elevation = 표고에 점 여러 개 삽입
state-keep-inside = 내부 유지
state-keep-outside = 외부 유지
state-kriged-block-model = 크리깅 블록 모델
state-load-block-model = 블록 모델 불러오기
state-load-drillholes = 시추공 불러오기
state-load-layer = 레이어 불러오기
state-load-point-cloud = 포인트 클라우드 불러오기
state-load-raster = 래스터 불러오기
state-load-triangulation = 삼각망 불러오기
state-locked-count-object-s = 객체 { $count }개를 잠갔습니다
state-major-minor = 주 간격 { $major } · 부 간격 { $minor }
state-member-into-folder-section = { $member }을(를) { $section }의 { $folder }(으)로
state-member-root-section = { $member }을(를) { $section }의 최상위로
state-move-axis-value = 축 값으로 이동
state-move-objects-layer = 객체를 레이어로 이동
state-name-count-cloud-s = { $name } · 클라우드 { $count }개
state-name-count-holes = { $name } · 발파공 { $count }개
state-name-count-object-s = { $name } · 객체 { $count }개
state-name-z-min-z-max = { $name } · { $z_min } ~ { $z_max }
state-new-collection-under-section = { $section } 아래의 새 컬렉션
state-next-edit = 다음 편집
state-north = 북
state-off = 끔
state-on = 켬
state-open-containing-folder = 포함된 폴더 열기
state-open-project = 프로젝트 열기
state-preserve-view-angle = 뷰 각도 유지
state-previous-edit = 이전 편집
state-project-id = 프로젝트 { $id }
state-remove-block-model = 블록 모델 제거
state-remove-drillholes = 시추공 제거
state-remove-point-cloud = 포인트 클라우드 제거
state-remove-raster = 래스터 제거
state-remove-triangulation = 삼각망 제거
state-removed-from-active-triangulation = 활성 삼각망에서 제거됨
state-removed-from-every-triangulation = 모든 삼각망에서 제거됨
state-rename-kind = { $kind } 이름 바꾸기
state-rename-seam = 탄층 이름 바꾸기
state-rename-seam-from-to = { $from }에서 { $to }(으)로
state-save-close-project = 저장 후 프로젝트 닫기
state-save-despite-unsupported-content = 지원되지 않는 콘텐츠가 있어도 저장
state-save-project = 다른 이름으로 프로젝트 저장
state-save-replace-project = 저장 후 프로젝트 교체
state-saving-current-project = 현재 프로젝트 저장 중
state-section-name = { $section } 구간
state-select-layer-objects = 레이어 객체 선택
state-selected-objects = 선택한 객체
state-selected-polylines = 선택한 폴리라인
state-selected-scene-elements = 선택한 장면 요소
state-hidden-objects = 숨겨진 모든 객체
state-set-block-model-variable = 블록 모델 변수 설정
state-set-cinematic-view = 시네마틱 뷰 설정
state-set-drillhole-colour-preset = 시추공 색상 프리셋 설정
state-set-drillhole-discs = 시추공 디스크 설정
state-set-drillhole-style = 시추공 스타일 설정
state-set-drillhole-width = 시추공 너비 설정
state-set-entity-lock = 엔터티 잠금 설정
state-set-grid = 그리드 설정
state-set-layer-lock = 레이어 잠금 설정
state-set-line-weight = 선 두께 설정
state-set-modelling-settings = 모델링 설정 변경
state-set-object-colour = 객체 색상 설정
state-set-object-fill = 객체 채우기 설정
state-set-point-visibility = 점 표시 여부 설정
state-set-polyline-closed = 폴리라인 닫힘 설정
state-set-raster-lock = 래스터 잠금 설정
state-set-standard-view = 표준 뷰 설정
state-set-topology-wireframes = 지형면 와이어프레임 설정
state-set-triangulation-colour = 삼각망 색상 설정
state-shift-names = 이름 이동
state-shift-names-down = { $field } 시추공 아래로
state-shift-names-down-from-here = { $field } 층준에서부터 시추공 아래로
state-shift-names-up = { $field } 시추공 위로
state-shift-names-up-from-here = { $field } 층준에서부터 시추공 위로
state-show-console = 콘솔 표시
state-show-project = 프로젝트 표시
state-shown = 표시됨
state-slice-mode = 슬라이스 모드
state-slice-preview = 슬라이스 미리보기
state-south = 남
state-stem-contours = { $stem } 등고선
state-target-new-name = { $target }을(를) “{ $new_name }”(으)로
state-trim-above = 위쪽 트리밍
state-trim-below = 아래쪽 트리밍
state-trim-triangulation-surface = 표면에 맞춰 삼각망 트리밍
state-undrape-raster = 래스터 투영 해제
state-undrape-rasters = 래스터 투영 해제
state-unload-block-model = 블록 모델 언로드
state-unload-drillholes = 시추공 언로드
state-unload-layer = 레이어 언로드
state-unload-point-cloud = 포인트 클라우드 언로드
state-unload-raster = 래스터 언로드
state-unload-triangulation = 삼각망 언로드
state-untitled-project = 제목 없는 프로젝트
state-use-typed-radius = 입력한 반지름 사용
state-west = 서

## Status strings

status-clip-near-far = 클립 근/원/Δ: -- / -- / --
status-faces-chunks = 면: -- / -- (청크 --/--)
status-frame-rate = 프레임 속도
status-points-chunks = 점: -- / -- (전체 --, 청크 --/--)

## Text strings

text-could-not-build-vector-mesh = 글꼴 { $font }, 글리프 { $glyph }에 대한 벡터 메쉬를 생성할 수 없습니다: { $error }
text-document-text-mesh-exceeded-its = 문서 텍스트 메쉬가 u32 인덱스 범위를 초과했습니다

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = 먼저 연결할 시추공 데이터 세트를 선택하세요
tie-in-count-connector-s = 커넥터 { $count }개
tie-in-delete-tie-ins = 연결 삭제
tie-in-deleted-count-selected-tie-connector = 선택한 연결 커넥터 { $count }개를 삭제했습니다
tie-in-hole = 발파공
tie-in-initiation-point-lifted-from-name = { $name }에서 기폭점을 제거했습니다
tie-in-initiation-point-set-name-delay = { $name }에 지연시간 { $delay } ms로 기폭점을 설정했습니다
tie-in-select-delay-product-palette-before = 발파공을 연결하기 전에 팔레트에서 지연 제품을 선택하세요
tie-in-tied-connectors = { $product }(으)로 지연시간 { $delay } ms에 커넥터 { $count }개를 연결했습니다
tie-in-tied-connectors-replacing = { $product }(으)로 지연시간 { $delay } ms에 커넥터 { $count }개를 연결했습니다({ $replaced }개 교체됨)

## Toolbar strings

toolbar-fill-type = 채우기 유형

## Toolbars strings

toolbars-auto-bench = 자동 벤치
toolbars-bezier-polyline = 베지어 폴리라인
toolbars-chamfer-polyline-corners = 폴리라인 모서리 모따기
toolbars-create-text = 텍스트 생성
toolbars-cursor-regular = 커서: 일반
toolbars-cursor-snap-line = 커서: 선에 스냅
toolbars-cursor-snap-point = 커서: 점에 스냅
toolbars-cursor-snap-surface = 커서: 표면에 스냅
toolbars-delete-points = 점 삭제
toolbars-explode-polyline-lines = 폴리라인을 선으로 분해
toolbars-fuse-polylines = 폴리라인 결합
toolbars-measure-distance = 거리 측정
toolbars-new-layer = 새 레이어
toolbars-split-polyline-points = 점에서 폴리라인 분할
toolbars-strike-dip = 주향 및 경사
toolbars-tool-not-available-section-view = { $tool } - 슬라이스 뷰에서 사용할 수 없음

## Tri strings

tri-sampling-method-help = 적응형 방법은 평면 적합 오차를 기반으로 복잡한 지형에 정점을 집중시키고, 균일 방법은 고르게 분산시킵니다. 향후 더 많은 방법이 추가될 수 있습니다.
tri-adaptive-quadtree = 적응형(쿼드트리)
tri-axis-range = { $axis } 범위
tri-base-topology-will-receive-pit = 채굴장 또는 적치장 형태를 받아들일 기준 지형면입니다.
tri-boundary-polyline = 경계 폴리라인
tri-bridge-gaps-help = 표면 전체에서 이 값보다 좁은 간격과 경계 오목부를 연결합니다. 0으로 설정해도 대략 샘플링 셀 크기까지의 간격은 연결됩니다. 값이 클수록 더 큰 구멍을 메우고 경계 오목부를 완화합니다.
tri-budget = 예산 기준
tri-cancel-pick = 선택 취소
tri-candidate-detail = 후보 상세도
tri-candidate-fine-cells-per-budgeted = 예산 정점당 후보 세밀 셀 수입니다. 값이 높을수록 적응형 샘플러가 세부 사항을 배치할 자유도가 커지지만 생성 속도는 느려집니다.
tri-cap-surface-share-source-points = 원본 점의 비율 또는 정확한 정점 수로 표면 규모를 제한합니다.
tri-choose-input-clicking-loaded-surface = 뷰포트에서 로드된 표면을 클릭하여 이 입력을 선택하세요
tri-choose-which-side-reference-topology = 공유하는 XY 영역 내에서 표면으로부터 제거할 기준 지형면의 방향을 선택하세요.
tri-clip = 클립
tri-clip-creates-new-triangulation-name = 클립은 이 이름으로 새 삼각망을 생성합니다. 원본 표면은 수정되지 않습니다.
tri-clip-surface-polyline = 폴리라인으로 표면 클립
tri-closed-pit-stockpile-solid-whose = 노출된 경계가 결과에 포함될 닫힌 채굴장 또는 적치장 솔리드입니다.
tri-cloud-carries-no-classifications-so = 이 클라우드에는 분류가 없으므로 모든 점이 표면화됩니다. 맨땅을 복원하려면 지면 필터를 거친 LAS/LAZ 파일을 가져오세요.
tri-create-new-layer-contours-append = 등고선을 위한 새 레이어를 생성하거나 활성 프로젝트의 기존 레이어에 추가합니다.
tri-cut-topology-pit-shell = 채굴장 쉘로 지형면 절단
tri-e-g-design-trimmed = 예: design_trimmed
tri-e-g-mysurf-cut = 예: mysurf_cut
tri-e-g-mysurf-slice = 예: mysurf_slice
tri-e-g-surface-contour = 예: surface_contour
tri-e-g-topo-cut = 예: topo_cut
tri-e-g-topo-pit = 예: topo_with_pit
tri-exact-number-surface-vertices-target = 목표로 할 정확한 표면 정점 수입니다. 매우 큰 값은 생성이 느리고 메모리를 많이 사용합니다.
tri-existing-ground-topology-will-cut = 채굴장 쉘에 의해 절단될 기존 지반 지형면입니다.
tri-fill-holes-up = 구멍 메우기 한도
tri-generate = 생성
tri-generate-contour-lines = 등고선 생성
tri-generate-upper-surface = 상부 표면 생성
tri-ground-points-only = 지면 점만
tri-hide-unload-sources = 원본 숨기고 언로드
tri-higher-edge-will-enforced-each = 충돌이 발생하는 각 지점에서 더 높은 모서리가 적용됩니다. 낮은 쪽의 충돌 세그먼트는 브레이크라인으로 취급되지 않으며, 표면은 해당 영역에서 보간됩니다. 원본 폴리라인은 변경되지 않습니다.
tri-breaklines-cross = 강조 표시된 브레이크라인 모서리가 서로 다른 표고에서 평면상 교차하거나 겹칩니다. 하나의 지형 표면은 둘 다를 따를 수 없습니다.
tri-intervals-colours = 구간 및 색상
tri-keep-clipped-topology-included-shape = 클립된 지형면과 포함된 형태를 하나의 엔터티로 합치지 않고 별도의 삼각망으로 유지합니다.
tri-keep-inside-discards-surface-outside = 내부 유지는 폴리라인 바깥의 표면을 버립니다. 외부 유지는 표면에서 폴리라인 모양의 구멍을 잘라냅니다.
tri-keeps-only-surface-within-polyline = 폴리라인 경계 내부의 표면만 유지합니다.
tri-keep-surface-relation-help = XY 범위 내에서 표면을 지형면과 { $relation } 관계로 유지합니다.
tri-layer-already-exists-select-above = 해당 레이어가 이미 존재합니다. 위에서 선택하거나 다른 이름을 사용하세요.
tri-limit-z-range = Z 범위 제한
tri-major = 주
tri-max-edge-length = 최대 변 길이
tri-merge = 병합
tri-method = 방법
tri-min = 최소
tri-minimum-maximum-elevations-retained = 출력 표면에 유지되는 최소 및 최대 표고입니다. 최소값은 최대값보다 작아야 합니다.
tri-minor = 부
tri-contour-interval-help = 부 간격은 일반 등고선을 제어합니다. 주 간격은 강조 등고선을 제어하며, 부 간격 이상의 값을 사용해야 합니다.
tri-move-cursor-over-loaded-surface = 로드된 표면 위로 커서를 이동하세요.
tri-slice-output-name-help = 표고로 클립된 출력 표면에 지정할 이름입니다.
tri-name-assigned-merged-topology-pit = 병합된 지형면과 채굴장/적치장 결과에 지정할 이름입니다.
tri-name-assigned-newly-created-contour = 새로 생성된 등고선 레이어에 지정할 이름입니다.
tri-reconstruct-output-name-help = 재구성된 삼각망에 지정할 이름입니다.
tri-name-assigned-topology-after-pit = 채굴장 쉘로 절단된 후의 지형면에 지정할 이름입니다.
tri-name-assigned-trimmed-output-surface = 트리밍된 출력 표면에 지정할 이름입니다.
tri-nearby-breakline-vertices-do-not = 인접한 브레이크라인 정점이 정확히 같은 위치에서 만나지 않아 표면을 삼각화할 수 없습니다.
tri-new-layer = 새 레이어
tri-new-layer-name = 새 레이어 이름
tri-no-boundary-selected = 선택한 경계 없음
tri-no-point-cloud-selected = 선택한 포인트 클라우드 없음
tri-no-surface-selected = 선택한 표면 없음
tri-once-clip-succeeds-unload-source = 클립에 성공하면 원본 표면을 언로드하여 클립된 결과만 장면에 남깁니다.
tri-once-cut-succeeds-unload-original = 절단에 성공하면 원본 지형면을 언로드하여 절단된 결과만 장면에 남깁니다. 채굴장 쉘은 로드된 상태로 유지됩니다.
tri-once-merge-succeeds-unload-source = 병합에 성공하면 원본 지형면과 솔리드를 언로드하여 장면에 병합된 결과만 남깁니다.
tri-once-slice-succeeds-unload-source = 슬라이스에 성공하면 원본 표면을 언로드하여 슬라이스된 결과만 장면에 남깁니다.
tri-once-trim-succeeds-unload-surface = 트리밍에 성공하면 트리밍된 표면을 언로드하여 결과만 장면에 남깁니다. 지형면은 로드된 상태로 유지됩니다.
tri-only-loaded-pickable = 로드된 삼각망만 선택할 수 있습니다.
tri-operation = 작업
tri-output-layer = 출력 레이어
tri-percentage = 백분율
tri-percentage-cloud = 포인트 클라우드 비율
tri-pick-from-view = 뷰에서 선택
tri-pit-design-surface-only-areas = 채굴장 설계 표면입니다. 지형면보다 아래로 굴착하는 영역만 절단에 사용됩니다.
tri-pit-shell = 채굴장 쉘
tri-pit-stockpile-solid = 채굴장/적치장 솔리드
tri-recommended-weld-retry = 권장: 결합 후 재시도
tri-reconstruct-ground-only-help = 맨땅으로 분류된 점으로부터 복원하고, 식생, 건물, 설비 및 노이즈는 제외합니다. 클라우드의 모든 점을 표면화하려면 끄세요.
tri-reconstruct-help = 포인트 클라우드에서 삼각망 지형 표면을 재구성합니다. 적응형 샘플러는 지형이 가장 복잡한 곳에 정점 예산을 사용하고 평탄한 영역은 성기게 유지합니다.
tri-reduce-budget-candidate-detail-if = 컴퓨터의 RAM이 적다면 예산 또는 후보 상세도를 줄이세요.
tri-reference-topology-help = 다른 표면이 트리밍되는 위치를 정의하는 기준 지형면입니다.
tri-reject-reconstructed-triangle-edges = 이 거리보다 긴 재구성된 삼각형 변을 거부합니다. 변 길이 제한이 없으려면 0을 사용하세요.
tri-remove-inside-help = 폴리라인 경계 내부의 표면을 제거하고 나머지는 유지합니다.
tri-removes-topology-where-pit-shell = 채굴장 쉘이 아래로 굴착하는 위치의 지형면을 제거하여 쉘이 그 구멍을 채우도록 합니다. 이음선은 두 표면 사이의 실제 3D 접촉선을 따르며, 지면보다 위에 있는 쉘 부분 아래의 지형면은 유지됩니다.
tri-result = 결과
tri-save-two-entities = 두 개의 엔터티로 저장
tri-select = 선택…
tri-selected-closed-polyline-whose-xy = XY 경계가 클립 영역을 정의하는 선택한 닫힌 폴리라인입니다.
tri-selected-point-cloud-whose-points = 점들이 지형 표면으로 복원될 선택한 포인트 클라우드입니다. 다른 클라우드를 복원하려면 대화상자를 닫으세요.
tri-selected-surface-from-which-contour = 등고선을 생성할 선택한 표면입니다. 다른 표면의 등고선을 만들려면 대화상자를 닫으세요.
tri-selected-surface-which-will-clipped = 클립될 선택한 표면입니다. 다른 표면을 클립하려면 대화상자를 닫으세요.
tri-slice-source-help = 표고 범위가 클립될 선택한 표면입니다. 다른 표면을 슬라이스하려면 대화상자를 닫으세요.
tri-share-source-points-keep-fractions = 유지할 원본 점의 비율입니다. 0.125%와 같은 소수도 허용됩니다.
tri-slice-triangulation-z-range = Z 범위로 삼각망 슬라이스
tri-solution-generate-upper-surface = 해결 방법: 상부 표면 생성
tri-surface-trim = 트리밍할 표면
tri-target-surface-help = 변경될 표면입니다. 선택한 지형면은 그대로 유지됩니다.
common-percent-suffix = %
tri-topology = 지형면
tri-triangulation-failed = 삼각망 생성 실패
tri-trim = 트리밍
tri-trim-topology = 지형면에 맞춰 트리밍
tri-uniform-grid = 균일 그리드
tri-unload-source-surface = 원본 표면 언로드
tri-unload-source-topology = 원본 지형면 언로드
tri-up-target-point-count-points = { $point_count }개 점 중 최대 { $target }개가 표면 정점이 됩니다({ $percent }%).
tri-use-full-surface-elevation-range = 전체 표면 표고 범위 사용
tri-vertex-count = 정점 수
tri-vertices-within-5-cm-xy = 이 삼각망에서는 XY와 Z 방향으로 5cm 이내에 있는 정점들이 하나의 위치를 공유합니다. 이로 인해 생성된 표면이 국소적으로 최대 5cm까지 이동할 수 있습니다. 원본 폴리라인은 변경되지 않습니다.
tri-weld-retry = 결합 후 재시도
tri-when-enabled-generate-contours-only = 활성화하면 지정된 최소 및 최대 표고 사이에서만 등고선을 생성합니다.

## Ui strings

ui-choose-offset-side = 오프셋 방향 선택
ui-choose-relimit-side = 재한계 방향 선택
ui-click-circle-centre = 원의 중심 클릭
ui-click-closed-polyline-use-blast = 발파구역 형상으로 사용할 닫힌 폴리라인을 클릭하세요
ui-click-collar-add-edit-initiation = 공구를 클릭하여 기폭점을 추가하거나 편집하세요
ui-click-first-point-slice-line = 슬라이스 선의 첫 번째 점 클릭
ui-click-first-vertex = 첫 번째 정점 클릭
ui-click-perimeter-point-type-radius = 둘레 점을 클릭하거나 반지름을 입력하세요
ui-click-second-point-slice-line = 슬라이스 선의 두 번째 점 클릭
ui-click-second-vertex = 두 번째 정점 클릭
ui-click-use-pointer-radius = 클릭하여 포인터 반지름을 사용하세요
ui-could-not-copy-text-browser = 텍스트를 브라우저 클립보드에 복사할 수 없습니다: { $error }
ui-dip-horizontal-no-strike = { $dip }(수평, 주향 없음)
ui-distance-meters = { $distance } m
ui-drag-ring-type-azimuth-dip = 링을 드래그하거나 방위각과 경사각을 입력하세요
ui-each-hole-turns-about-its = 각 발파공은 자신의 공구를 중심으로 회전합니다
ui-enter-positive-decimal-radius = 양의 소수 반지름을 입력하세요
ui-esc-cancels = Esc로 취소
ui-no-delay-product-tie = 연결할 지연 제품이 없습니다
ui-press-enter-use-typed-radius = Enter 키를 눌러 입력한 반지름을 사용하거나
ui-right-click-delay-palette-heading = 지연 팔레트 제목을 우클릭하여 추가하세요
ui-select-designs = 설계 선택
ui-select-drill-hole = 시추공 선택
ui-select-endpoint-join = 연결할 끝점 선택
ui-select-first-crest-toe-point = 첫 번째 상단/하단 점 선택
ui-select-item = 항목 선택
ui-select-line-fuse = 결합할 선 선택
ui-select-line-polyline = 선 또는 폴리라인 선택
ui-select-line-relimit = 재한계할 선 선택
ui-select-next-line-fuse = 결합할 다음 선 선택
ui-select-opposite-berm-point = 반대쪽 소단 점 선택
ui-select-point = 점 선택
ui-select-polyline = 폴리라인 선택
ui-select-polyline-open-line = 폴리라인 또는 열린 선 선택
ui-select-polyline-vertex = 폴리라인 정점 선택
ui-select-second-crest-toe-point = 두 번째 상단/하단 점 선택
ui-select-second-split-point = 두 번째 분할점 선택
ui-select-split-point = 분할점 선택
ui-select-topologies = 지형면 선택
ui-slice-view = 슬라이스 뷰
ui-strike-dip = 주향 { $strike }° · { $dip }
ui-value-dip = 경사각 { $value }°
viewport-1-1-true-shape = 1:1, 실제 형태
viewport-1-ratio = 1:{ $ratio }

## Viewport strings

viewport-all-total-categories-keep-their = 전체 { $total }개 범주가 색상을 유지하며, 처음 { $shown }개만 뚜렷하게 그려집니다
viewport-axis-maximum = { $axis } 최대값
viewport-axis-minimum = { $axis } 최소값
viewport-azimuth-dip = 방위각 { $azimuth }, 경사각 { $dip }
viewport-back-whole-log = 전체 검층으로 돌아가기.
viewport-bar-blast-timeline-placeholder = 발파 타임라인 [플레이스홀더]
viewport-bar-burden-relief-heatmap-placeholder = 저항선 이완 히트맵 [플레이스홀더]
viewport-bar-cinematic-view = 시네마틱 뷰
viewport-bar-color = 색상:
viewport-bar-contours-equal-time-placeholder = 등시선 [플레이스홀더]
viewport-bar-disable-cinematic-view = 시네마틱 뷰 끄기
viewport-bar-disable-flying-mode = 비행 모드 비활성화
viewport-bar-disable-x-ray-vision = 투시 모드 비활성화
viewport-bar-drill-holes = 시추공:
viewport-bar-enable-flying-mode = 비행 모드 활성화
viewport-bar-enable-x-ray-vision = 투시 모드 활성화
viewport-bar-unhide-all = 모두 숨기기 해제: 불러온 레이어의 숨긴 객체를 표시
viewport-bar-exit-slice-view = 슬라이스 뷰 종료
viewport-bar-fill = 채우기:
viewport-bar-fix-centre-rotation = 회전 중심 고정
viewport-bar-hide-borehole-inspector = 시추공 검사기 숨기기
viewport-bar-hide-classification = 분류 숨기기
viewport-bar-hide-points = 점 숨기기
viewport-bar-hide-rl-grid = RL 그리드 숨기기
viewport-bar-hide-wireframes = 와이어프레임 숨기기
viewport-bar-hide-xy-grid = XY 그리드 숨기기
viewport-bar-release-centre-rotation = 회전 중심 해제
viewport-bar-show-borehole-inspector = 시추공 검사기 표시
viewport-bar-show-classification = 분류 표시
viewport-bar-show-points = 점 표시
viewport-bar-show-rl-grid = RL 그리드 표시
viewport-bar-show-wireframes = 와이어프레임 표시
viewport-bar-show-xy-grid = XY 그리드 표시
viewport-bar-vertical-slice-view = 수직 슬라이스 뷰
viewport-blank = (빈 값)
viewport-choose-active-block-model-variable = 활성 블록 모델 변수를 선택하세요
viewport-choose-variable = 변수 선택
viewport-click-edit-color-right-click = 클릭하여 색상 편집, 우클릭하여 제거
viewport-click-type-boundary-s-value = 클릭하여 이 경계의 값을 입력하세요
viewport-colour-mapping = 색상 매핑
viewport-count-categories = 범주 { $count }개
viewport-count-category = 범주 { $count }개
viewport-depth-m-hole-end = { $depth } m 시추공 끝
viewport-double-click-add-boundary-here = 여기에 경계를 추가하려면 더블클릭하세요
viewport-drag-move-middle-click-toggles = 드래그하여 이동 · 가운데 클릭으로 전환 ≤
viewport-drag-move-right-click-remove = 드래그하여 이동 · 우클릭하여 제거 · 가운데 클릭으로 전환 ≤
viewport-drag-spin-view-around-hole = 드래그하여 시추공을 중심으로 뷰를 회전하세요. 더블 클릭하면 북쪽을 향합니다.
viewport-e = E
viewport-edit-category-colour = 이 범주 색상 편집
viewport-edit-colour-used-empty-values = 빈 값에 사용되는 색상 편집
viewport-empty = (비어 있음)
viewport-empty-hidden = (비어 있음 · 숨겨짐)
viewport-field-has-no-strat-column = 이 필드에는 아직 층서 열이 없습니다. 검사기의 층서 열 탭에서 만드세요
viewport-filter-variables = 변수 필터링
viewport-fit-hole-track = 시추공을 궤적에 맞춤
viewport-from = { $from } ~ { $to }
viewport-from-m = { $from } ~ { $to } m
viewport-h-1-ratio = H 1:{ $ratio }
viewport-hole-has-no-trace-draw = 이 시추공에는 그릴 궤적이 없습니다.
viewport-interval-data = 구간 데이터
viewport-intervals = 구간
viewport-m-from-collar-toward-bearing = 공구에서 { $bearing }° 방향으로 m
viewport-navigation-hint = 가운데 버튼 드래그로 이동 · 스크롤로 확대/축소
viewport-navigation-hint-detach = 가운데 버튼 드래그로 이동 · 스크롤로 확대/축소 · 클릭으로 분리
viewport-move-all-down = 모두 아래로
viewport-move-all-up = 모두 위로
viewport-move-down-from-here = 여기서부터 아래로
viewport-move-up-from-here = 여기서부터 위로
viewport-n = N
viewport-name-not-in-strat-column = 이 이름은 필드의 층서 열에 없으므로 이동을 시작할 층준이 없습니다
viewport-no-data-variable = 이 변수에 대한 데이터 없음
viewport-no-density-log-hole = 이 시추공에는 밀도 검층이 없습니다
viewport-no-downhole-geophysics-hole = 이 시추공에는 공내 물리탐사가 없습니다
viewport-no-gamma-log-hole = 이 시추공에는 감마 검층이 없습니다
viewport-no-matches = 일치하는 항목 없음
viewport-no-trace = 궤적 없음
viewport-no-usable-range = (사용 가능한 범위 없음)
viewport-not-logged = 기록되지 않음
viewport-orientation-source = 방향 출처
viewport-rebuild-variable-s-colours-from = 데이터를 기반으로 이 변수의 색상 다시 생성
viewport-rename-seam-in-every-hole = 모든 시추공에서 이름 바꾸기
viewport-rename-seam-in-this-hole = 이 시추공에서 이름 바꾸기
viewport-reset = 재설정
viewport-restore-full-model-range = 전체 모델 범위 복원
viewport-roll-wheel-over-log-zoom = 검층 위에서 휠을 굴려 탄층을 확대하세요. 검층을 드래그하면 시추공을 회전하고 따라 내려갈 수 있습니다.
viewport-s = S
viewport-sideways-scale = 가로 축척
viewport-squeeze-sideways-just-enough-keep = 시추공이 보이도록 필요한 만큼만 가로로 압축합니다. 늘리지는 않습니다.
viewport-trace-extent = 궤적 범위
viewport-w = W
viewport-widen-panel-show-density = 밀도를 표시하려면 패널을 넓히세요
viewport-widen-panel-show-density-gamma = 밀도와 감마를 표시하려면 패널을 넓히세요
viewport-widen-panel-show-gamma = 감마를 표시하려면 패널을 넓히세요
charging-edit-charge-product = 장약 제품 편집
charging-new-charge-product = 새 장약 제품
charging-explosive-decks-add-mass-primed-stemming = 폭약 데크는 질량이 더해지고 기폭약이 장착되며, 전색 및 에어 데크는 길이만 사용합니다.
charging-density = 밀도
charging-density-hint = 공내 밀도입니다. 미터당 질량은 이 값에 발파공 단면적을 곱한 값입니다.
charging-another-product-already-has-name = 다른 제품이 이미 이 이름을 사용 중입니다
charging-edit-charge-rule = 장약 규칙 편집
charging-new-charge-rule = 새 장약 규칙
charging-decks-collar-toe = 데크, 공구에서 공저 순
charging-priming = 기폭
charging-preview = 미리보기
charging-preview-use-pattern-hole = 패턴의 중앙값 발파공 사용
charging-preview-active-pattern-median-hole = 활성 패턴의 중앙값 발파공으로 미리보기
charging-fixed-decks-longer-than-hole = 고정 데크가 이 발파공보다 깁니다
charging-mass-kg-explosive = 폭약 { $mass } kg
charging-rate-kg-m = { $rate } kg/m
charging-count-primer = 기폭약 { $count }개
charging-another-rule-already-has-name = 다른 규칙이 이미 이 이름을 사용 중입니다
charging-save-reload-count-hole = 저장 후 발파공 { $count }개 다시 장약
charging-length = 길이
charging-rest-length-m = 나머지 · { $length } m
charging-rest = 나머지
charging-deck-takes-whatever-length-fixed-decks = 이 데크는 고정 데크를 제외한 나머지 길이를 모두 차지합니다. 규칙당 하나의 데크만 나머지를 채웁니다.
charging-remove-deck = 데크 제거
charging-add-deck = 데크 추가
charging-downhole-delay = 공내 지연시간
charging-hole-detonator-hole-fires-long-after = 공내 뇌관입니다. 발파공은 지표 신호가 도달한 후 이 시간이 지나 기폭됩니다.
charging-primer-height = 기폭약 높이
charging-how-far-above-base-each-explosive = 각 폭약 데크의 바닥에서 기폭약이 위치하는 높이입니다.
charging-booster = 부스터
charging-cast-booster-mass-each-primer = 각 기폭약의 캐스트 부스터 질량입니다.
charging-count-rule-load-product-will-need = 이 제품을 장약하는 규칙 { $count }개에 다른 제품을 선택해야 합니다.
charging-rule = 규칙
charging-holes-already-loaded-keep-their-charge = 이미 이 제품으로 장약된 발파공은 장약이 유지됩니다.
blast-burden-relief = 저항선 이완
blast-ms-per-metre-last-neighbour-fire = 마지막으로 기폭하는 인접 공까지 미터당 ms
blast-below-hole-fires-before-rock-front = 이 값 미만에서는 발파공 앞의 암석이 이동하기 전에 기폭됩니다: 타이트.
blast-above-rock-front-has-long-gone = 이 값 초과에서는 앞의 암석이 이미 오래전에 이동했습니다: 느슨함, 절단 및 비석 위험이 있습니다.
blast-tight = 타이트
blast-good = 양호
blast-slack = 느슨함
blast-free-face = 자유면
blast-fires-at = 기폭 시간
blast-empty-won-t-detonate = 비어 있음, 기폭되지 않음
blast-not-reached = 도달하지 않음
blast-value-ms-m-from-hole = { $hole }에서 { $value } ms/m
blast-fires-first-free-face = 가장 먼저 기폭: 자유면
blast-relief = 완화
blast-explosive = 폭약
blast-powder-factor = 비장약량
blast-not-loaded = 장약되지 않음
blast-count-primer-delay-ms-downhole = 기폭약 { $count }개 · 공내 { $delay } ms
blast-set-initiation-point-tie-holes-play = 기폭점을 설정하고 발파공을 연결하여 발파를 재생하세요
blast-pause = 일시 정지
blast-play = 재생
blast-back-start = 처음으로
blast-duration-ms = / { $duration } ms
blast-real-time = 실시간
blast-mic-limit = MIC 한도
blast-most-explosive-allowed-detonate-any-8 = 이 현장에서 임의의 8 ms 안에 기폭할 수 있는 최대 폭약량입니다. 이를 초과하는 구간은 표시됩니다.
blast-no-holes-loaded-surface-signal-plays = 장약된 발파공이 없습니다: 지표 신호는 재생되지만 기폭되는 것은 없습니다. 발파공 장약 도구로 발파공을 장약하세요.
blast-now-holes-hole = 현재: 발파공 { $holes }개
blast-in-8-ms = 8 ms 내
blast-peak-mass-kg-time-ms = 최대 { $mass } kg, { $time } ms
blast-peak-holes-hole-time-ms = 최대 발파공 { $holes }개, { $time } ms
blast-peak-over-limit = , { $over } kg 초과
blast-peak-within-limit = , 한도 이내
blast-top-surface-signal-lighting-each-downline = 위: 각 다운라인에 점화되는 지표 신호. 아래: 기폭. 클릭하거나 드래그하여 재생 위치를 이동하세요.
products-charge-rules = 장약 규칙
products-new-rule = 새 규칙
products-charge-products = 장약 제품
products-new-rule-default-name = 새 규칙
products-no-rules = 규칙 없음
products-load-selected-holes-count = 선택한 발파공 장약 ({ $count })
products-unload-selected-holes-count = 선택한 발파공 장약 해제 ({ $count })
products-edit-rule = 규칙 편집
products-duplicate-rule = 규칙 복제
products-delete-rule = 규칙 삭제
products-fill-product = 채움  { $product }
products-primer-offset-m-off-each-explosive = 각 폭약 데크 바닥에서 { $offset } m 위에 기폭약, 부스터 { $booster } kg, 공내 지연시간 { $delay } ms
products-double-click-edit = 더블 클릭하여 편집
products-edit-product = 제품 편집
blast-log-updated-charge-product-name = 장약 제품 { $name }을(를) 업데이트했습니다
blast-log-added-charge-product-name = 장약 제품 { $name }을(를) 추가했습니다
blast-log-updated-charge-rule-name = 장약 규칙 { $name }을(를) 업데이트했습니다
blast-log-added-charge-rule-name = 장약 규칙 { $name }을(를) 추가했습니다
blast-log-entry-no-longer-charge-library = 해당 항목이 더 이상 장약 라이브러리에 없습니다
blast-log-deleted-name-from-charge-library = 장약 라이브러리에서 { $name }을(를) 삭제했습니다
blast-log-failed-save-charge-library-error = 장약 라이브러리를 저장하지 못했습니다: { $error }
blast-log-cannot-load-rule-problem = 이 규칙으로 장약할 수 없습니다: { $problem }
blast-log-count-hole-too-short-fixed-decks = 발파공 { $count }개가 이 규칙의 고정 데크에 비해 너무 짧아 그대로 두었습니다
blast-log-count-hole-have-no-depth-load = 발파공 { $count }개에 장약할 심도가 없습니다
blast-log-count-loaded-hole-have-no-diameter = 장약된 발파공 { $count }개에 공경이 없어 폭약 질량을 알 수 없습니다
common-charge-holes = 발파공 장약
blast-log-loaded-count-hole-rule = { $rule }(으)로 발파공 { $count }개를 장약했습니다
blast-log-unload-holes = 발파공 장약 해제
blast-log-unloaded-count-hole = 발파공 { $count }개의 장약을 해제했습니다
blast-log-select-holes-active-pattern-first = 먼저 활성 패턴의 발파공을 선택하세요
blast-log-rule-no-longer-charge-library = 해당 규칙이 더 이상 장약 라이브러리에 없습니다
blast-log-there-no-charge-rule-load-add = 장약에 사용할 장약 규칙이 없습니다. 제품 패널에서 추가하세요
blast-rule-stemming = 전색
blast-rule-air-deck = 에어 데크
blast-rule-give-rule-name = 규칙 이름을 입력하세요
blast-rule-add-least-one-deck = 데크를 하나 이상 추가하세요
blast-rule-only-one-deck-can-fill-rest = 나머지 길이를 채울 수 있는 데크는 하나뿐입니다
blast-rule-deck-lengths-must-greater-than-zero = 데크 길이는 0보다 커야 합니다
blast-rule-no-product-named-name = '{ $name }' 제품이 없습니다
blast-rule-rule-needs-least-one-explosive-deck = 규칙에는 폭약 데크가 하나 이상 필요합니다
state-save-charge-product = 장약 제품 저장
state-save-charge-rule = 장약 규칙 저장
state-delete-charge-library-entry = 장약 라이브러리 항목 삭제
ui-click-drag-over-holes-load-them = 발파공 위를 클릭하거나 드래그하여 { $rule }(으)로 장약
ui-hold-shift-unload = Shift를 누르면 장약 해제
ui-no-charge-rule-load = 장약에 사용할 장약 규칙 없음
ui-right-click-charge-rules-heading-add = 장약 규칙 제목을 우클릭하여 추가
omf-element-name-has-count-charge-naming = 요소 '{ $name }'에 더 이상 포함되지 않은 발파공을 지정하는 장약이 { $count }개 있습니다
