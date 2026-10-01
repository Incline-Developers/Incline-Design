# Incline — Danh mục thông báo tiếng Việt.
#
# Tệp này có thể chưa đầy đủ; các id còn thiếu sẽ dùng bản tiếng Anh
# (`i18n/en/incline_design.ftl`) làm phương án dự phòng.
#
# Không thay đổi id (phần bên trái dấu `=`) hoặc tên tham số bên trong
# `{ $... }` — chỉ dịch phần giá trị bên phải dấu `=`.

## Shared

common-cancel = Hủy
common-clear = Xóa
common-close = Đóng
common-fill = Tô
common-set = Đặt

## Status bar

# Tiêu đề của menu ngôn ngữ trên thanh trạng thái. Tên các ngôn ngữ không
# được dịch: mỗi ngôn ngữ tự hiển thị bằng chữ viết của mình, từ `LanguageChoice`.
status-language = Ngôn ngữ

## Menu bar — File

menu-file = Tệp
menu-file-save-project = Lưu dự án
menu-file-save-project-as = Lưu dự án thành...
menu-file-new-project = Dự án mới...
menu-file-open-project = Mở dự án...
menu-file-open-recent = Mở gần đây
menu-file-show-in-explorer = Hiển thị trong Explorer
menu-file-show-in-folder = Mở thư mục chứa
menu-file-import = Nhập...
menu-file-export = Xuất...
menu-file-export-viewport-image = Xuất ảnh khung nhìn...
menu-file-export-engineering-drawing = Xuất bản vẽ kỹ thuật...
menu-file-about = Giới thiệu về { $app }...
menu-file-exit = Thoát ứng dụng

## Menu bar — View

menu-view = Xem

## Workspaces

ws-production = Sản xuất
ws-drill-and-blast = Khoan & Nổ mìn
ws-geology = Địa chất
ws-planning = Lập kế hoạch

## Menubars

ws-menubar-design = Thiết kế
ws-menubar-triangulation = Lưới tam giác
ws-menubar-raster = Ảnh raster
ws-menubar-point-cloud = Đám mây điểm
ws-menubar-block-model = Mô hình khối
ws-menubar-drillholes = Lỗ khoan
ws-menubar-active-layer = Lớp:

## Menubars functions

ws-menubar-design-insert-point = Chèn điểm
ws-menubar-design-insert-point-at-intersection = Tại giao điểm
ws-menubar-design-insert-point-at-elevation = Tại cao độ
ws-menubar-design-move-to = Di chuyển đến
ws-menubar-design-create-triangulation = Tạo lưới tam giác

## Rename / delete item dialogs

# { $kind } là danh từ không gian làm việc từ nhóm ws-production-* ở trên.
dialog-rename-title = Đổi tên { $kind }
dialog-rename-field = Tên mới
dialog-rename-field-hint = Bắt buộc
dialog-rename-submit = Đổi tên
dialog-delete-title = Xóa { $kind }
dialog-delete-confirm =
    Xóa '{ $name }' khỏi dự án?
    Không thể hoàn tác thao tác này.
confirm-delete-product =
    Xóa sản phẩm '{ $name }' khỏi bảng?
    Không thể hoàn tác thao tác này.

## Create Triangulation dialog

tri-create-title = Tạo lưới tam giác
tri-create-type-label = Loại lưới tam giác
tri-create-type-help =
    Mặt hở tạo ra một tấm bề mặt kiểu địa hình. Khối đặc tạo ra một lưới
    khép kín hoàn toàn và yêu cầu dữ liệu đầu vào có thể tạo thành ranh giới kín nước.
tri-create-output-name = Tên đầu ra
tri-create-output-name-help = Tên gán cho lưới tam giác được tạo ra.
tri-create-output-name-hint = tên lưới tam giác
tri-create-run = Tạo lưới tam giác

tri-selection-selected = Đã chọn { $summary }

tri-type-open-surface = Mặt
tri-type-solid-closed = Khối đặc

# Các thành phần tóm tắt lựa chọn, ví dụ: "3 đường đa tuyến, 1 điểm".
tri-count-polylines =
    { $count ->
       *[other] { $count } đường đa tuyến
    }
tri-count-strings =
    { $count ->
       *[other] { $count } đường
    }
tri-count-points =
    { $count ->
       *[other] { $count } điểm
    }
tri-count-texts =
    { $count ->
       *[other] { $count } đối tượng văn bản
    }
tri-count-objects =
    { $count ->
       *[other] { $count } đối tượng
    }

about-read-full-licence = Đọc toàn văn giấy phép ↗
about-source-code = Mã nguồn
about-website = Trang web
about-title = Giới thiệu về { $app }
drill-hole-colour-stop = Điểm dừng { $index }
properties-restore-defaults-tooltip = Đặt lại thiết lập { $heading } về mặc định

## Dynamic UI messages

ui-selected-count = Đã chọn { $count }
ui-selected-objects = Đã chọn { $count } đối tượng
ui-selected-polylines = Đã chọn { $count } đường đa tuyến
ui-invalid-axis-value = Nhập giá trị { $axis } hợp lệ.
ui-selection-spans = Vùng chọn kéo dài từ { $min } đến { $max }.
confirm-delete-count = Bạn có chắc muốn xóa { $count } mục đã chọn không?
confirm-delete-layer = Xóa lớp '{ $name }' cùng tất cả đối tượng trên đó?
    Không thể hoàn tác thao tác này.
plot-preview-pixels = { $width } × { $height } px ở { $dpi } dpi
tri-estimated-memory = Bộ nhớ đỉnh ước tính ~{ $estimate }. { $detail }
block-grid-summary = Lưới: { $x } × { $y } × { $z } = { $count } khối
status-selected = Đã chọn: { $count }
status-clip = Cắt gần/xa/Δ: { $near } / { $far } / { $delta } m

explorer-no-rasters = Không có ảnh raster
slice-viewport-gestures = kéo nút giữa: di chuyển · kéo nút phải: xoay quỹ đạo · Shift+lăn chuột: đi bộ · W/S: di chuyển lớp cắt · Q/E: xoay · Esc: thoát

## Startup environment details

## Renderer startup diagnostics

color-aci = ACI
color-aci-value = ACI { $index }
color-index = Chỉ số
color-rgb = RGB
color-opacity = Độ mờ
color-edit = Nhấp để sửa màu
color-saturation-value = Độ bão hòa và độ sáng
color-hue = Tông màu
asset-loading = Đang tải dữ liệu tài sản
asset-unloading = Đang gỡ dữ liệu tài sản
asset-load-failed = Không thể tải dữ liệu tài sản
asset-unload-failed = Không thể gỡ dữ liệu tài sản
preferences-title = Tùy chọn
context-text-colour = Màu văn bản
context-polylines = Đường đa tuyến
context-points = Điểm
crs-unknown-ellipsoid = Mô hình Trái Đất không được nhận dạng "{ $name }" trong định nghĩa hệ tọa độ này.
crs-no-ellipsoid = Định nghĩa hệ tọa độ này không cho biết mô hình Trái Đất mà nó sử dụng.
crs-unknown-code = EPSG:{ $code } không có trong sổ đăng ký hệ tọa độ.
crs-transform-failed = Không thể chuyển đổi một tọa độ; kết quả không phải là một vị trí hữu hạn.
crs-no-datum-path = Không có phép chuyển đổi nào được công bố giữa các hệ quy chiếu của { $from } và { $to } (mốc EPSG { $source } và { $target }). Việc chuyển đổi dù sao cũng sẽ sai lệch một lượng không xác định, vì vậy không có gì được thay đổi.
crs-unknown-datum = Không thể xác định hệ quy chiếu của { $from } hoặc { $to }, và cả hai sử dụng các mô hình Trái Đất khác nhau. Việc chuyển đổi giữa chúng sẽ sai lệch một lượng không xác định.
ws-survey = Trắc địa
survey-count-designs = { $count } { $count ->
   *[other] thiết kế
  }
survey-count-meshes = { $count } { $count ->
   *[other] lưới tam giác
  }
survey-count-models = { $count } { $count ->
   *[other] mô hình khối
  }
survey-count-clouds = { $count } { $count ->
   *[other] đám mây điểm
  }
survey-count-holes = { $count } { $count ->
   *[other] bộ dữ liệu lỗ khoan
  }
survey-count-rasters = { $count } { $count ->
   *[other] raster
  }
survey-angle = Xoay quanh Z (ngược chiều kim đồng hồ)
survey-scale = Hệ số tỷ lệ XYZ đồng nhất
survey-invalid-transform = Gốc tọa độ, góc và tọa độ kết quả phải là hữu hạn.
survey-invalid-scale = Tỷ lệ phải là một số dương hữu hạn có nghịch đảo hữu hạn.
survey-empty-selection = Chọn ít nhất một mục được hỗ trợ để chuyển đổi.
survey-unavailable = Một mục đã chọn bị thiếu hoặc chưa được tải. Hãy tải nó trước khi chuyển đổi.
survey-wrong-project = Chỉ chọn thiết kế từ dự án đang hoạt động.
survey-name-required = Nhập tên hệ tọa độ.
survey-working = Đang chuyển đổi dữ liệu đã chọn…
survey-completed = Đã chuyển đổi tại chỗ { $items }. Hoàn tác sẽ khôi phục chúng.
survey-failed = Chuyển đổi thất bại: { $error }
survey-stale = Phép chuyển đổi bị hủy vì dự án đang hoạt động hoặc dữ liệu nguồn đã thay đổi. Hãy chọn dữ liệu nguồn và thử lại.
survey-coordinates-menu = Tọa độ
survey-definitions-action = Định nghĩa…
survey-transform-action = Chuyển đổi…
survey-definitions-title = Định nghĩa tọa độ
survey-transform-title = Chuyển đổi tọa độ
survey-new-system = Hệ tọa độ mới
survey-new-system-name = Hệ tọa độ
survey-set-local = Đặt làm Hệ tọa độ mỏ
survey-delete-system = Xóa hệ tọa độ
survey-systems-empty = Không có hệ tọa độ nào
survey-system-name = Tên
survey-system-origin = Cùng điểm — tọa độ hệ
survey-angle-help = Ngược chiều kim đồng hồ từ X tham chiếu hướng đến Y tham chiếu, nhìn từ trên xuống.
survey-scale-help = Tỷ lệ XYZ đồng nhất từ hệ tham chiếu đến hệ này. Dùng 1 để giữ nguyên kích thước.
survey-close = Đóng
survey-from = Từ
survey-to = Đến
survey-transform-button = Chuyển đổi
survey-swap = Hoán đổi
survey-drape-note = Ảnh phủ bị loại bỏ khỏi các bề mặt đã chuyển đổi và phải được phủ lại.
survey-needs-grid-block-model = Mô hình khối là một lưới ô đều đặn, và việc thay đổi phép chiếu hay hệ quy chiếu không giữ được sự đều đặn đó. Chuyển đổi nó sẽ đồng nghĩa với việc lấy mẫu lại từng ô vào một lưới mới và mất đi các giá trị mà nó mang theo, vì vậy nó được giữ nguyên.
survey-needs-grid-raster = Một raster được đặt vào thế giới bằng một phép ánh xạ afin, điều mà việc thay đổi phép chiếu hay hệ quy chiếu không thể bảo toàn. Chuyển đổi nó sẽ đồng nghĩa với việc lấy mẫu lại hình ảnh, vì vậy nó được giữ nguyên.
survey-conversion-exact = Chính xác: chỉ thay đổi lưới, không chiếu lại.
survey-conversion-accuracy = Độ chính xác đã nêu { $accuracy } m.
survey-kind = Loại
survey-axis-names = Tên trục
survey-kind-registry-short = Hệ trong sổ đăng ký
survey-kind-grid-short = Lưới trên một hệ khác
survey-registry-search = Tìm kiếm
survey-registry-hint = Tên hoặc mã EPSG, vd. "mga zone 56"
survey-registry-none = Không có gì trong sổ đăng ký khớp với tất cả các từ.
survey-parent = Được định nghĩa dựa trên
survey-parent-origin = Điểm đã biết — tọa độ hệ cha
survey-pick-registry = Tìm kiếm hệ và chọn từ kết quả.
survey-pick-parent = Chọn hệ mà lưới này được định nghĩa dựa trên.
survey-pick-system = Chọn một hệ
survey-pick-systems = Chọn hệ nguồn và hệ đích để chuyển đổi.
survey-no-selection = Chọn một hệ tọa độ ở bên trái, hoặc nhấp chuột phải để thêm một hệ mới.
survey-kind-grid = Lưới trên { $parent }
survey-system-in-use = Không thể xóa "{ $name }": { $dependants } { $dependants ->
   *[other] hệ
  } được định nghĩa dựa trên nó. Hãy chuyển hướng chúng sang nơi khác trước.
survey-system-cycle = "{ $name }" được định nghĩa dựa trên chính nó, trực tiếp hoặc thông qua các hệ cha của nó.
survey-system-missing = Hệ tọa độ đó không còn tồn tại. Hãy chọn một định nghĩa khác.
survey-same-system = Hãy chọn hệ nguồn và hệ đích khác nhau.
survey-name-exists = Đã tồn tại một hệ tọa độ với tên đó. Hãy chọn nó để sửa, hoặc chọn tên khác.

## About strings

about-copyright-c-2026-leo-timmins =
    Bản quyền (c) 2026 Leo Timmins, Lucas Timmins và những người đóng góp cho Incline Design. Theo đây, quyền được cấp miễn phí cho bất kỳ ai có được bản sao của phần mềm này để sử dụng nó mà không bị hạn chế, tuân theo các điều kiện của Giấy phép MIT.

    Incline Design được cung cấp "NGUYÊN TRẠNG", KHÔNG CÓ BẤT KỲ BẢO ĐẢM NÀO, RÕ RÀNG HAY NGỤ Ý, bao gồm nhưng không giới hạn ở các bảo đảm về KHẢ NĂNG THƯƠNG MẠI, SỰ PHÙ HỢP CHO MỘT MỤC ĐÍCH CỤ THỂ và KHÔNG VI PHẠM.
about-free-open-source-mine-design = Thiết kế mỏ mã nguồn mở, miễn phí
about-licensed-under-mit-license = Được cấp phép theo Giấy phép MIT

## App strings

app-activated-browser-project-name = Đã kích hoạt dự án trình duyệt '{ $name }'.
app-browser-project-delete-failed = Xóa dự án trình duyệt thất bại: { $error }
app-browser-project-no-longer-exists = Dự án trình duyệt đó không còn tồn tại
app-browser-save-failed-error = Lưu trong trình duyệt thất bại: { $error }
app-could-not-activate-browser-project = Không thể kích hoạt dự án trình duyệt: { $error }
app-could-not-delete-browser-project = Không thể xóa dự án trình duyệt: { $error }
app-could-not-load-browser-project = Không thể tải dự án trình duyệt: { $error }
app-could-not-restore-browser-project = Không thể khôi phục dự án trình duyệt: { $error }
app-deleted-browser-project = Đã xóa dự án trình duyệt
app-failed-create-window-error = Tạo cửa sổ thất bại: { $error }
app-failed-create-window-icon-error = Tạo biểu tượng cửa sổ thất bại: { $error }
app-failed-detach-top-down-preview = Tách xem trước từ trên xuống thất bại: { $error }
app-failed-initialize-graphics-error = Khởi tạo đồ họa thất bại: { $error }
app-browser-preferences-load-failed = Tải tùy chọn trình duyệt thất bại: { $error }
app-failed-load-config-file-error = Tải tệp cấu hình thất bại: { $error }
app-failed-load-session-file-error = Tải tệp phiên làm việc thất bại: { $error }
app-failed-rasterize-window-icon-error = Rasterize biểu tượng cửa sổ thất bại: { $error }
app-failed-save-browser-session-error = Lưu phiên trình duyệt thất bại: { $error }
app-failed-save-session-error = Lưu phiên làm việc thất bại: { $error }
app-saved-name-browser-storage = Đã lưu '{ $name }' vào bộ nhớ trình duyệt

## Block strings

block-model-between = Nằm giữa
block-model-block-grid = Lưới khối
block-model-block-size = Kích thước khối
block-model-choose-numeric-variable = Chọn một biến số
block-model-choose-numeric-variables = Chọn biến số
block-model-count-variables-selected = Đã chọn { $count } biến
block-model-estimate-variables = Ước tính biến
block-model-full-x-y-z-dimensions = Kích thước đầy đủ theo X, Y và Z của mỗi khối. Khối nhỏ hơn tăng độ chi tiết, thời gian tính toán và bộ nhớ sử dụng.
block-model-grid-bounds-block-sizes-invalid = Giới hạn lưới hoặc kích thước khối không hợp lệ.
block-model-lower-x-y-z-edges = Cạnh X, Y và Z dưới của khối mô hình. Tâm khối bắt đầu từ nửa khối bên trong các giới hạn này.
block-model-maximum = Tối đa
block-model-maximum-nearest-samples-used-each = Số mẫu gần nhất tối đa dùng cho mỗi khối. Giá trị thấp hơn chạy nhanh hơn; giá trị cao hơn có thể làm mượt ước tính và tăng thời gian tính toán.
block-model-maximum-samples = Số mẫu tối đa
block-model-minimum = Tối thiểu
block-model-min-samples-help = Số mẫu lân cận tối thiểu cần thiết để ước tính một khối. Các khối có ít mẫu hơn trong bán kính tìm kiếm sẽ để trống.
block-model-minimum-samples = Số mẫu tối thiểu
block-model-nugget = Nugget
block-model-numeric-interval-fields-interpolate = Các trường khoảng dạng số cần nội suy. Mỗi trường đã chọn trở thành một biến mô hình khối.
block-model-kriging-help = Kriging thông thường ước tính các khoảng lỗ khoan dạng số tại mỗi tâm khối bằng variogram hình cầu.
block-model-partial-sill = Sill một phần
block-model-range-search-radius = Phạm vi / bán kính tìm kiếm
block-model-range-help = Các mẫu xa hơn khoảng cách này sẽ bị loại; hiệp phương sai đạt về 0 tại phạm vi này.
block-model-select-all = Chọn tất cả
block-model-sill-help = Phương sai tương quan không gian do mô hình hình cầu đóng góp. Cùng với nugget, nó xác định hiệp phương sai tại khoảng cách bằng 0.
block-model-spherical-variogram-search = Variogram hình cầu và tìm kiếm
block-model-threshold-at-most = <= ngưỡng
block-model-threshold-at-least = >= ngưỡng
block-model-threshold-min = Ngưỡng / tối thiểu
block-model-upper-x-y-z-extent = Phạm vi X, Y và Z trên cần bao phủ. Khối cuối cùng có thể vượt quá phạm vi này khi khoảng cách không phải là bội số chính xác của kích thước khối.
block-model-variable = Biến
block-model-variance-effectively-zero-separation = Phương sai tại khoảng cách gần như bằng 0, do sai số đo lường hoặc biến động dưới quy mô lấy mẫu. Dùng 0 khi không muốn có hiệu ứng nugget.
block-model-volume-feedback-disconnected = Đọc phản hồi sử dụng khối thể tích bị ngắt kết nối
block-model-volume-feedback-failed = Đọc phản hồi sử dụng khối thể tích thất bại: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z

## Canvas strings

canvas-not-selectable-closed-polyline = Không thể chọn | Chọn một đường đa tuyến khép kín
canvas-polyline-summary = Đường đa tuyến | Lớp: { $layer } | { $count } đỉnh
canvas-surface-name = Bề mặt | { $name }
canvas-trimmed = Đã xén

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = Đã tạo taluy - cơ từ đối tượng { $object_id }
cmd-bezier-replaced-polyline-span-first-last = Đã thay thế đoạn đường đa tuyến { $first }→{ $last } bằng { $count } điểm trung gian lấy mẫu
cmd-bezier-vertices-first-last = Đỉnh { $first } đến { $last }
cmd-block-model-block-model-loader-disconnected-path = Bộ tải mô hình khối bị ngắt kết nối với { $path }
cmd-block-model-block-model-path-has-count = Mô hình khối { $path } có { $count } biến thuộc loại không được hỗ trợ và sẽ không đọc được: { $names }
cmd-block-model-building-ore-mesh = Đang dựng lưới quặng…
cmd-block-model-could-not-create-block-model = Không thể tạo mô hình khối: { $error }
cmd-block-model-could-not-decode-block-model = Không thể giải mã biến màu mô hình khối '{ $variable }': { $error }
cmd-block-model-created-block-model-name-ordinary = Đã tạo mô hình khối '{ $name }' bằng Kriging thông thường
cmd-block-model-failed-load-block-model-error = Tải mô hình khối thất bại: { $error }
cmd-block-model-generated-ore-mesh-from-block = Đã tạo lưới quặng từ mô hình khối '{ $name }'
cmd-block-model-imported-block-model-source-path = Đã nhập nguồn mô hình khối { $path }
cmd-block-model-loaded-block-model-name-blocks = Đã tải mô hình khối '{ $name }': { $blocks } khối ({ $renderable } có thể dựng hình), lưới { $dimx }x{ $dimy }x{ $dimz }, { $variables } biến
cmd-block-model-loading-name = Đang tải { $name }
cmd-block-model-loading-name-ellipsis = Đang tải { $name }…
cmd-chamfer-applied = Đã vát góc { $corner } với bán kính { $radius } và { $segments } đoạn
cmd-chamfer-radius = Bán kính { $radius }
cmd-commands-clipped = Đã cắt
cmd-commands-command-failed-error = Lệnh thất bại: { $error }
cmd-commands-select-one-more-objects-before = Chọn một hoặc nhiều đối tượng trước khi đặt { $axis }
cmd-commands-sliced = Đã cắt lát
cmd-contours-contour-generation-failed-error = Tạo đường đồng mức thất bại: { $error }
cmd-contours-discarded-layer-exists = Đường đồng mức cho '{ $name }' đã bị bỏ qua: lớp '{ $layer_name }' hiện đã tồn tại
cmd-contours-discarded-project-closed = Đường đồng mức cho '{ $name }' đã bị bỏ qua: dự án đã đóng
cmd-contours-discarded-layer-deleted = Đường đồng mức cho '{ $name }' đã bị bỏ qua: lớp đầu ra đã chọn đã bị xóa
cmd-contours-generated = Đã tạo { $line_count } đường đa tuyến đồng mức cho lưới tam giác '{ $name }' trong lớp '{ $layer_name }'
cmd-creation-assembled-boundary-rings = Đã ghép { $assembled_count } vòng ranh giới khép kín từ các đường hở bị phân mảnh
cmd-creation-created-triangulation-from-boundary = Đã tạo lưới tam giác từ { $boundary_count } vòng ranh giới và { $constraint_count } ràng buộc hở, loại bề mặt { $surface_type }
cmd-creation-creating-triangulation = Đang tạo lưới tam giác…
cmd-creation-generate-upper-surface-ignored-count = Tạo bề mặt trên: đã bỏ qua { $count } đoạn đường gãy xung đột thấp hơn; các đối tượng nguồn không thay đổi
cmd-creation-ignored-objects = Đã bỏ qua { $rejected } đối tượng không phải đường đa tuyến hoặc suy biến trong quá trình tạo lưới tam giác
cmd-creation-weld-retry-moved-coarse-welded = Hàn & thử lại: đã di chuyển { $coarse_welded } đỉnh vào các vị trí chung (tới { $coarse_weld_tol } m); các đối tượng nguồn không thay đổi
cmd-creation-welded-breakline-vertices = Đã hàn { $welded } đỉnh đường gãy trùng nhau trong phạm vi sai số
cmd-cuts-clipped-surface-name-polyline-mode = Đã cắt bề mặt '{ $name }' theo đường đa tuyến ({ $mode })
cmd-cuts-clipping-surface-polyline = Đang cắt bề mặt theo đường đa tuyến…
cmd-cuts-cut-topology-name-pit-shell = Đã cắt bề mặt địa hình '{ $name }' theo vỏ moong
cmd-cuts-cut-triangulation-name-z-band = Đã cắt lưới tam giác '{ $name }' theo dải Z [{ $min }, { $max }]
cmd-cuts-cutting-topology-pit-shell = Đang cắt bề mặt địa hình theo vỏ moong…
cmd-cuts-cutting-triangulation-z = Đang cắt lưới tam giác theo Z…
cmd-cuts-ignored-vertical-faces = Đã bỏ qua { $count } mặt bề mặt địa hình tham chiếu thẳng đứng hoặc suy biến không có diện tích XY
cmd-cuts-site-skipped-constraint-from-x = { $site }: đã bỏ qua ràng buộc ({ $from_x }, { $from_y }) -> ({ $to_x }, { $to_y }) do bộ tạo lưới tam giác không thể chia tách
cmd-cuts-skipped-degenerate-edges = { $site }: đã bỏ qua { $skipped } cạnh ràng buộc gần như suy biến; ranh giới cắt có thể lệch một chút gần các cạnh đó
cmd-cuts-trimmed-surface = Đã xén bề mặt '{ $surface }' theo bề mặt địa hình '{ $topology }' ({ $mode })
cmd-cuts-trimming-surface-topology = Đang xén bề mặt theo bề mặt địa hình…
cmd-drape-draped-intersected-vertices-changed = Đã phủ { $intersected } đỉnh; { $changed } đã thay đổi cao độ
cmd-drape-no-intersections = Không có đỉnh thiết kế nào đã chọn giao với các bề mặt địa hình đã chọn
cmd-drape-objects-changed-object-s-changed = { $objects } đối tượng đã thay đổi · { $changed } trong { $intersected } đỉnh giao đã di chuyển
cmd-drape-select-one-more-design-objects = Chọn một hoặc nhiều đối tượng thiết kế để phủ
cmd-drape-select-one-more-topologies-drape = Chọn một hoặc nhiều bề mặt địa hình để phủ lên
cmd-drape-selected-topologies-no-longer-loaded = Các bề mặt địa hình đã chọn không còn được tải
cmd-drill-hole-drill-pattern-too-large-contains = Mẫu khoan quá lớn hoặc chứa tọa độ miệng lỗ không hợp lệ
cmd-drill-hole-enter-name-drill-pattern = Nhập tên cho mẫu khoan
cmd-drill-hole-failed-load-drillholes-error = Tải lỗ khoan thất bại: { $error }
cmd-drill-hole-depth-must-be-positive = Độ sâu lỗ khoan phải lớn hơn 0
cmd-drill-hole-diameter-must-be-positive = Đường kính lỗ khoan phải lớn hơn 0
cmd-drill-hole-loaded-drillhole-dataset-name-holes = Đã tải bộ dữ liệu lỗ khoan '{ $name }': { $holes } lỗ, { $fields } trường màu
cmd-drill-hole-pattern-contains-no-holes = Mẫu khoan không chứa lỗ khoan nào
cmd-explode-count-line-s = { $count } đường
cmd-explode-polyline = Phân rã đường đa tuyến
cmd-explode-exploded-polyline-into-count-line = Đã phân rã đường đa tuyến thành { $count } đoạn thẳng
cmd-file-block-model-csv-encoding-failed = Mã hóa CSV mô hình khối thất bại: { $error }
cmd-file-block-model-csv-export-failed = Xuất CSV mô hình khối thất bại: { $error }
cmd-file-browser-recovery-unavailable = Không có tệp khôi phục trình duyệt; các dự án đã lưu vẫn còn trong IndexedDB
cmd-file-closed-project-runtime-id-runtime = Đã đóng dự án với id thời gian chạy { $runtime_id }
cmd-file-could-not-create-new-project = Không thể tạo dự án mới: { $error }
cmd-file-could-not-finish-pending-project = Không thể hoàn tất thao tác dự án đang chờ: { $error }
cmd-file-could-not-finish-saving-before = Không thể hoàn tất lưu trước khi thoát: { $error }
cmd-file-could-not-open-browser-project = Không thể mở dự án trình duyệt: { $error }
cmd-file-could-not-open-path-error = Không thể mở { $path }: { $error }
cmd-file-could-not-read-selected-file = Không thể đọc tệp đã chọn: { $error }
cmd-file-could-not-reload-layer-from = Không thể tải lại lớp từ đĩa: { $error }
cmd-file-could-not-reload-project-from = Không thể tải lại dự án từ đĩa: { $error }
cmd-file-could-not-remove-browser-project = Không thể gỡ dự án trình duyệt: { $error }
cmd-file-could-not-restore-layer-from = Không thể khôi phục lớp từ dự án: { $error }
cmd-file-could-not-snapshot-dirty-project = Không thể chụp nhanh dự án chưa lưu để khôi phục: { $error }
cmd-file-could-not-start-browser-export = Không thể bắt đầu xuất trong trình duyệt: { $error }
cmd-file-could-not-write-recovery-copies = Không thể ghi các bản khôi phục: { $error }
cmd-file-created-new-browser-project = Đã tạo dự án trình duyệt mới
cmd-file-created-new-project = Đã tạo dự án mới
cmd-file-description-download-failed-error = Tải xuống { $description } thất bại: { $error }
cmd-file-discard-cancelled-project-changed = Đã hủy thao tác bỏ qua vì dự án đã thay đổi trong khi tệp OMF đang được tải lại
cmd-file-discarded-changes-layer-target-name = Đã bỏ qua thay đổi của lớp '{ $target_name }'
cmd-file-discarded-changes-reloaded-path = Đã bỏ qua thay đổi: đã tải lại { $path }
cmd-file-downloaded-description-file-name = Đã tải xuống { $description }: { $file_name }
cmd-file-dxf-download-encoding-failed-error = Mã hóa tệp DXF tải xuống thất bại: { $error }
cmd-file-dxf-import-failed-error = Nhập DXF thất bại: { $error }
cmd-file-encoding-block-model-csv-download = Đang mã hóa CSV mô hình khối tải xuống…
cmd-file-encoding-dxf-download = Đang mã hóa tệp DXF tải xuống…
cmd-file-encoding-triangulation-download = Đang mã hóa lưới tam giác tải xuống…
cmd-file-exit-deferred-exports = Hoãn thoát cho đến khi các tác vụ xuất nền hoàn tất
cmd-file-exit-requested-no-unsaved-changes = Yêu cầu thoát, không có thay đổi chưa lưu
cmd-file-exported-block-model-csv-path = Đã xuất CSV mô hình khối sang { $path }
cmd-file-exported-description-dxf-path = Đã xuất { $description } sang DXF: { $path }
cmd-file-exported-triangulation-name-path = Đã xuất lưới tam giác '{ $name }' sang { $path }
cmd-file-exporting-name = Đang xuất { $name }…
cmd-file-exporting-triangulation-name-path = Đang xuất lưới tam giác '{ $name }' sang { $path }
cmd-file-fatal-renderer-failure-reason = Lỗi nghiêm trọng của bộ dựng hình: { $reason }
cmd-file-dialog-action-failed = Thao tác hộp thoại tệp thất bại: { $msg }
cmd-file-imported-added-object-s-from = Đã nhập { $added } đối tượng từ { $name }
cmd-file-imported-total-dxf-object-s = Đã nhập { $total } đối tượng DXF
cmd-file-layer-discard-was-cancelled-because = Đã hủy bỏ qua thay đổi lớp vì dự án đã thay đổi trong khi dự án đang được tải lại
cmd-file-no-recovery-directory = Không có thư mục khôi phục: { $error }
cmd-file-no-unsaved-project-content-nothing = Không có nội dung dự án chưa lưu; không có gì để khôi phục
cmd-file-parsing-browser-dxf-import = Đang phân tích tệp nhập DXF từ trình duyệt…
cmd-file-parsing-dxf-import = Đang phân tích tệp nhập DXF…
cmd-file-project-closes-after-save = Dự án sẽ đóng sau khi lưu hiện tại hoàn tất
cmd-file-the-project-closes-after-save = Dự án sẽ đóng sau khi lưu hiện tại hoàn tất
cmd-file-queued-count-triangulation-file-s = Đã xếp hàng { $count } tệp lưới tam giác để nhập
cmd-file-recovery-copies-path-reopen-them = Các bản khôi phục nằm trong { $path }; hãy mở lại chúng sau khi khởi động lại
cmd-file-recovery-copy-failed-error = Tạo bản khôi phục thất bại: { $error }
cmd-file-recovery-copy-failed-failure = Tạo bản khôi phục thất bại: { $failure }
cmd-file-recovery-copy-written-path = Đã ghi bản khôi phục: { $path }
cmd-file-reverting-layer = Đang khôi phục lớp…
cmd-file-reverting-project = Đang khôi phục dự án…
cmd-file-save-failed-message = Lưu thất bại: { $message }
cmd-file-save-worker-ended-without-result = Tiến trình lưu đã kết thúc mà không có kết quả
cmd-file-saved-project-as = Đã lưu dự án thành: { $path }
cmd-file-saved-project = Đã lưu dự án: { $path }
cmd-file-selected-block-model-no-longer = Mô hình khối đã chọn không còn được tải
cmd-file-switching-project = Đang chuyển dự án…
cmd-file-triangulation-download-encoding-failed = Mã hóa lưới tam giác tải xuống thất bại: { $error }
cmd-file-user-chose-exit-without-saving = Người dùng chọn thoát mà không lưu
cmd-file-user-requested-exit-project-export = Người dùng yêu cầu thoát (cần xác nhận xuất dự án hoặc công việc chưa lưu)
cmd-file-viewport = Khung nhìn
cmd-file-wait-current-project-save-finish = Chờ lưu dự án hiện tại hoàn tất
cmd-file-wait-current-project-switch-finish = Chờ chuyển dự án hiện tại hoàn tất
cmd-file-wait-project-operation-finish-before = Chờ thao tác dự án hoàn tất trước khi bỏ qua thay đổi
cmd-file-wait-project-revert-finish-before = Chờ khôi phục dự án hoàn tất trước khi lưu
cmd-fuse-closed-polyline = Đường đa tuyến khép kín
cmd-fuse-count-source-line-s = { $count } đường nguồn
cmd-fuse-created-shape-object-id-vertices = Đã tạo { $shape } { $object_id } với { $vertices } đỉnh từ { $sources } đường nguồn
cmd-fuse-click-missed = Hợp nhất: cú nhấp không chạm vào đối tượng nào (không có gì dưới con trỏ)
cmd-fuse-click-not-near-endpoint = Hợp nhất: cú nhấp không đủ gần với đầu nào của đường đã chọn
cmd-fuse-clicked-closed-polyline = Hợp nhất: đối tượng { $object_id } được nhấp là đường đa tuyến khép kín, hợp nhất chỉ hoạt động trên đường đa tuyến hở
cmd-fuse-clicked-not-open-polyline = Hợp nhất: đối tượng { $object_id } được nhấp không phải đường đa tuyến hở (đó là { $kind })
cmd-fuse-clicked-object-missing = Hợp nhất: đối tượng { $object_id } được nhấp không còn tồn tại
cmd-fuse-clicked-too-few-vertices = Hợp nhất: đường đa tuyến { $object_id } được nhấp chỉ có { $count } đỉnh, cần ít nhất 2
cmd-fuse-endpoint-marker-missing = Hợp nhất: điểm đánh dấu đầu mút { $marker_index } không còn tồn tại
cmd-fuse-close-needs-three-vertices = Hợp nhất: đường cần ít nhất 3 đỉnh khác nhau để khép thành đường đa tuyến (hiện có { $count })
cmd-fuse-lines = Hợp nhất đường
cmd-fuse-needs-two-segments = Hợp nhất: cần ít nhất 2 đoạn để hoàn tất (hiện có { $count })
cmd-fuse-no-active-layer = Hợp nhất: không có lớp đang hoạt động để đặt đường đã hợp nhất
cmd-fuse-no-active-project = Hợp nhất: không có dự án đang hoạt động, không thể hoàn tất
cmd-fuse-no-source-line = Hợp nhất: không có đường nguồn để khép thành đường đa tuyến
cmd-fuse-awaiting-object-invalid = Hợp nhất: đối tượng { $awaiting_id } không còn là đường đa tuyến hợp lệ
cmd-fuse-object-already-in-chain = Hợp nhất: đối tượng { $object_id } đã là một phần của chuỗi hợp nhất, hãy nhấp vào đường khác
cmd-fuse-result-too-few-vertices = Hợp nhất: kết quả có quá ít đỉnh ({ $count }), đã hủy
cmd-fuse-segment-object-invalid = Hợp nhất: đối tượng đoạn { $object_id } không còn là đường đa tuyến hợp lệ, đã hủy
cmd-fuse-source-object-invalid = Hợp nhất: đối tượng nguồn { $object_id } không còn là đường đa tuyến hở hợp lệ
cmd-fuse-source-object-missing = Hợp nhất: đối tượng nguồn { $object_id } không còn tồn tại
cmd-fuse-open-polyline = Đường đa tuyến hở
cmd-include-failed = Bao gồm thất bại: { $message }
cmd-include-included-solid-shape-name-topology = Đã đưa khối đặc '{ $shape_name }' vào bề mặt địa hình '{ $topology_name }' (giữ lại { $retained } mặt địa hình, bỏ qua { $skipped } mặt nắp đóng)
cmd-include-including-pit-stockpile-solid = Đang đưa vào khối moong/bãi chứa…
cmd-insert-point-count-operation-point-s = { $count } điểm { $operation }
cmd-insert-point-elevation-must-be-finite = Chèn điểm tại cao độ yêu cầu một giá trị cao độ hữu hạn
cmd-insert-point-insert-points = Chèn điểm
cmd-insert-point-inserted-count-operation-point-s = Đã chèn { $count } điểm { $operation }
cmd-insert-point-intersection = Giao điểm
cmd-insert-point-no-new-operation-points-were = Không tìm thấy điểm { $operation } mới nào
cmd-insert-point-select-least-two-polylines-before = Chọn ít nhất hai đường đa tuyến trước khi chèn điểm giao
cmd-insert-point-select-one-more-polylines-before = Chọn một hoặc nhiều đường đa tuyến trước khi chèn điểm tại cao độ
cmd-layer-created-layer-name = Đã tạo lớp '{ $name }'
cmd-layer-deleted-with-objects = Đã xóa lớp { $layer_id } (và tất cả đối tượng trên đó)
cmd-layer-duplicated-layer-duplicate-name = Đã nhân bản lớp '{ $duplicate_name }'
cmd-layer-locked = Đã khóa
cmd-layer-name-copy = { $name } bản sao
cmd-layer-selected-count-object-s-layer = Đã chọn { $count } đối tượng trong lớp { $layer_id }
cmd-layer-state-layer-name = { $state } lớp '{ $name }'
cmd-layer-unlocked = Đã mở khóa
cmd-move-tool-moved-collars = Đã áp dụng độ dịch chuyển ({ $delta }) cho { $count } miệng lỗ khoan
cmd-move-tool-moved-objects = Đã áp dụng độ dịch chuyển ({ $delta }) cho { $count } đối tượng
cmd-move-tool-count-hole-s = { $count } lỗ khoan
cmd-object-edit-edited-kind = Đã sửa { $kind }
cmd-object-edit-edited-kind-count-vertices = Đã sửa { $kind } ({ $count } đỉnh)
cmd-object-edit-no-changes-apply = Không có thay đổi nào để áp dụng
cmd-object-edit-object-changed-since-editor-opened = Đối tượng này đã thay đổi kể từ khi mở trình chỉnh sửa; hãy mở lại để sửa phiên bản hiện tại
cmd-object-edit-target-changed = Đối tượng đang sửa đã thay đổi; chỉnh sửa bị hủy bỏ
cmd-object-edit-object-no-longer-exists-document = Đối tượng đó không còn tồn tại trong tài liệu
cmd-object-edit-select-single-design-object-edit = Chọn một đối tượng thiết kế duy nhất để sửa
cmd-object-edit-unassigned = Chưa gán
cmd-offset-create-offset = Tạo dịch chuyển
cmd-offset-created-offset-count-object-s = Đã tạo dịch chuyển của { $count } đối tượng
cmd-offset-distance-must-be-positive = Khoảng cách dịch chuyển phải lớn hơn 0
cmd-omf-could-not-open-project-source = Không thể mở dự án { $source_name }: { $error }
cmd-omf-create-open-project-before-merging = Tạo hoặc mở một dự án trước khi gộp dữ liệu
cmd-omf-encoding-project = Đang mã hóa dự án…
cmd-omf-exported-project-path = Đã xuất dự án sang { $path }
cmd-omf-imported-project = Đã nhập dự án '{ $project_name }' từ { $source_name }: { $count } bộ dữ liệu cấp cao nhất
cmd-omf-importing-project = Đang nhập dự án…
cmd-omf-export-failed = Xuất OMF thất bại: { $error }
cmd-omf-import-failed = Nhập OMF thất bại: { $error }
cmd-omf-opened-project = Đã mở dự án '{ $project_name }' từ { $source_name }
cmd-omf-project-source-name-contains-no = Dự án '{ $source_name }' không chứa thành phần dữ liệu nào được hỗ trợ
cmd-omf-source-name-applied-project-origin = { $source_name }: đã áp dụng gốc tọa độ dự án { $origin } trước khi gộp
cmd-omf-crs-differs = { $source_name }: hệ quy chiếu tọa độ '{ $source_crs }' khác với CRS của dự án '{ $target_crs }'; tọa độ đã được gộp mà không chiếu lại
cmd-omf-source-name-units-source-units = { $source_name }: đơn vị '{ $source_units }' khác với đơn vị của dự án '{ $target_units }'; tọa độ đã được gộp mà không chuyển đổi
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = Không có dữ liệu Incline Design nào đang mở để xuất
cmd-placement-2-vertices = 2 đỉnh
cmd-placement-count-vertices = { $count } đỉnh
cmd-placement-created-circle = Đã tạo đường tròn với bán kính { $radius } m
cmd-placement-created-closed-polyline = Đã tạo đường đa tuyến khép kín với { $count } đỉnh
cmd-placement-created-line-segment-2-vertices = Đã tạo đoạn thẳng với 2 đỉnh
cmd-placement-created-open-polyline-count-vertices = Đã tạo đường đa tuyến hở với { $count } đỉnh
cmd-placement-placed-point-x-y-z = Đã đặt điểm tại { $x }, { $y }, { $z }
cmd-placement-radius = Bán kính { $radius } m
cmd-plot-composing-engineering-drawing = Đang soạn bản vẽ kỹ thuật…
cmd-plot-could-not-write-engineering-drawing = Không thể ghi bản vẽ kỹ thuật: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Tỉ lệ bản vẽ vừa khít dữ liệu hiển thị: 1:{ $scale }
cmd-plot = Bản vẽ
cmd-plot-saved-drawing = Đã lưu bản vẽ kỹ thuật: { $description } ({ $width } × { $height } px ở { $dpi } dpi)
cmd-point-cloud-failed-load-point-cloud-error = Tải đám mây điểm thất bại: { $error }
cmd-point-cloud-loaded-point-cloud-name-count = Đã tải đám mây điểm { $name } ({ $count } điểm)
cmd-point-cloud-point-cloud-loader-disconnected-path = Bộ tải đám mây điểm bị ngắt kết nối với { $path }
cmd-point-cloud-tin-max-edge-disabled = (đã tắt cạnh tối đa)
cmd-point-cloud-tin-max-edge-max-edge = (cạnh tối đa { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = Tạo TIN đám mây điểm thất bại: { $error }
cmd-point-cloud-tin-subsampled = TIN địa hình: đã lấy mẫu không gian { $sampled } trong tổng { $total } điểm
cmd-point-cloud-tin-triangulated = TIN địa hình: đã tạo lưới tam giác từ { $vertex_count } điểm XY duy nhất thành { $face_count } mặt{ $suffix }
cmd-products-added-product-delay-ms-ms = Đã thêm sản phẩm { $delay_ms } ms { $name }
cmd-products-deleted-product-delay-ms-ms = Đã xóa sản phẩm { $delay_ms } ms { $name }
cmd-products-failed-save-products-error = Lưu sản phẩm thất bại: { $error }
cmd-products-product-no-longer-palette = Sản phẩm đó không còn trong bảng
cmd-property-action-count-object-s-layer = { $action } { $count } đối tượng sang lớp { $layer }
cmd-property-batch-set-axis-value-count = Đặt hàng loạt giá trị { $axis } cho { $count } đối tượng
cmd-property-batch-set-closed-count-polyline = Đặt hàng loạt thuộc tính khép kín cho { $count } đường đa tuyến
cmd-property-batch-set-color-count-object = Đặt hàng loạt màu cho { $count } đối tượng
cmd-property-batch-set-fill-style-count = Đặt hàng loạt kiểu tô cho { $count } đối tượng
cmd-property-batch-set-line-weight-count = Đặt hàng loạt độ dày nét cho { $count } đường đa tuyến
cmd-property-copied = Đã sao chép
cmd-property-moved = Đã di chuyển
cmd-raster-draped = Đã phủ ảnh raster { $raster } lên lưới tam giác { $triangulation } (phạm vi chồng lấp)
cmd-raster-failed-load-raster-name-error = Tải ảnh raster { $name } thất bại: { $error }
cmd-raster-failed-load-raster-path-error = Tải ảnh raster { $path } thất bại: { $error }
cmd-raster-loaded-raster-name-via-driver = Đã tải ảnh raster { $name } qua { $driver } ({ $srcx }x{ $srcy }, xem trước { $prevx }x{ $prevy })
cmd-raster-no-overlapping-triangulation = Không có lưới tam giác đã tải nào chồng lên phạm vi của { $name }
cmd-raster-loader-disconnected = Bộ tải ảnh raster bị ngắt kết nối với { $path }
cmd-raster-undraped = Đã bỏ phủ ảnh raster khỏi { $count } lưới tam giác
cmd-relimit-click-missed = Giới hạn lại: cú nhấp không chạm vào đối tượng nào (không có gì dưới con trỏ)
cmd-relimit-click-ignored = Giới hạn lại: bỏ qua cú nhấp, công cụ hiện không chờ chọn mục tiêu
cmd-relimit-clicked-source-line = Giới hạn lại: đã nhấp vào chính đường nguồn, hãy chọn một đường khác
cmd-relimit-no-source-line = Giới hạn lại: chưa đặt đường nguồn, đã hủy chọn
cmd-relimit-relimited-line-source-id-selected = Đã giới hạn lại đường { $source_id } theo mục tiêu đã chọn
cmd-relimit-resized-line-source-id-using = Đã thay đổi kích thước đường { $source_id } dùng { $mode } giá trị { $value }
cmd-rename-item-no-longer-belongs-active = Mục đó không còn thuộc dự án đang hoạt động
cmd-rename-renamed-before-name = Đã đổi tên '{ $before }' thành '{ $name }'
cmd-rename-renamed-name-taken = Đã đổi tên '{ $before }' thành '{ $name }' ('{ $requested }' đã được sử dụng)
cmd-rotate-collar-turned-count-drillhole-collar-s = Đã xoay { $count } miệng lỗ khoan { $rotation }
cmd-section-verb-count-item-s-section = { $verb } { $count } mục trong { $section }
cmd-selection-delete-vertex = Xóa đỉnh
cmd-selection-deleted-count-selected-object-s = Đã xóa { $count } đối tượng đã chọn
cmd-selection-deleted-vertex = Đã xóa đỉnh { $vertex } khỏi đường đa tuyến { $object_id }
cmd-selection-duplicate-selection = Nhân bản vùng chọn
cmd-selection-duplicated-count-object-s = Đã nhân bản { $count } đối tượng
cmd-session-created-triangulation = Đã tạo lưới tam giác '{ $name }' ({ $vertex_count } đỉnh, { $face_count } mặt) từ loại bề mặt { $surface_type }
cmd-session-deleted-triangulation = Đã xóa lưới tam giác '{ $name }' khỏi dự án
cmd-session-failed-load-triangulation-error = Tải lưới tam giác thất bại: { $error }
cmd-session-failed-load-triangulation-message = Tải lưới tam giác thất bại: { $message }
cmd-session-loaded-triangulation = Đã tải lưới tam giác '{ $name }' ({ $path }, { $vertex_count } đỉnh, { $face_count } mặt)
cmd-session-set-triangulation-tri-id-color = Đã đặt màu lưới tam giác { $tri_id } thành { $color }
cmd-session-triangulation-load-no-result = Tải lưới tam giác cho { $path } đã kết thúc mà không có kết quả
cmd-session-triangulation-failed = Thao tác lưới tam giác thất bại: { $message }
cmd-session-unloaded-triangulation-name = Đã gỡ tải lưới tam giác '{ $name }'
cmd-slice-entered-slice-view-cx-cy = Đã vào khung nhìn cắt lát @ { $cx }, { $cy }, { $cz } theo hướng { $dx }, { $dy } (đường { $length }m)
cmd-slice-exited-slice-view = Đã thoát khung nhìn cắt lát
cmd-slice-reset-section-view-fit-extents = Đặt lại khung nhìn mặt cắt (vừa khít phạm vi)
cmd-slice-set-section-grid-enabled = Bật lưới mặt cắt = { $enabled }
cmd-split-created-2-open-polylines = Đã tạo 2 đường đa tuyến hở
cmd-split-line = Chia tách đường
cmd-split-points-needs-interior-vertex = Chia tách tại điểm: chọn một đỉnh bên trong của đường hở
cmd-split-polyline-into-two = Đã chia tách đường đa tuyến nguồn thành hai đường đa tuyến hở
cmd-text-edit-finished = Đã hoàn tất sửa văn bản cho đối tượng { $object_id }
cmd-text-updated = Đã cập nhật văn bản trên đối tượng { $object_id }
cmd-view-centre-rotation-not-available-flying = Tâm xoay không khả dụng trong chế độ bay
cmd-view-fixed-centre-rotation-x-y = Đã cố định tâm xoay tại { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = Không có điểm nào dưới con trỏ để cố định tâm xoay
cmd-view-released-centre-rotation = Đã giải phóng tâm xoay
cmd-view-reset-view-fit-extents = Đặt lại khung nhìn (vừa khung)
cmd-view-set-topology-wireframes-enabled = Đặt khung dây bề mặt địa hình = { $enabled }
cmd-view-set-view-points-enabled = Đặt hiển thị điểm = { $enabled }
cmd-view-set-xy-grid-enabled = Bật lưới XY = { $enabled }
cmd-view-zoom-extents-preserving-angle = Thu phóng vừa khung (giữ nguyên góc nhìn)

## Common strings

common-add-product = Thêm sản phẩm
common-background = Nền
common-block-model = Mô hình khối
common-block-models = Mô hình khối
common-cancelled = Đã hủy
common-chamfer = Vát góc
common-choose = Chọn...
common-circle = Vòng tròn
common-click-point-fix-centre-rotation = Nhấp vào một điểm để cố định tâm xoay
common-clip-surface-polyline = Cắt bề mặt theo đường đa tuyến...
common-closed = Khép kín
common-colour = Màu
common-confirm-omf-rewrite = Xác nhận ghi lại OMF
common-could-not-replace-current-project = Không thể thay thế dự án hiện tại: { $error }
common-count-object-s = { $count } đối tượng
common-create = Tạo
common-create-batter-berm = Tạo taluy - cơ
common-create-bezier-curve = Tạo đường cong Bezier
common-create-block-model = Tạo mô hình khối
common-create-block-model-ellipsis = Tạo mô hình khối...
common-create-circle = Tạo đường tròn
common-create-drill-pattern = Tạo mẫu khoan
common-create-layer = Tạo lớp
common-create-line = Tạo đường thẳng
common-create-ore-triangulation = Tạo lưới tam giác quặng
common-create-ore-triangulation-ellipsis = Tạo lưới tam giác quặng...
common-create-point = Tạo điểm
common-create-polyline = Tạo đường đa tuyến
common-create-triangulation = Tạo lưới tam giác...
common-crosses = Dấu cộng
common-cut = Cắt
common-cut-topology-pit-shell = Cắt bề mặt địa hình bằng vỏ moong...
common-delete-layer = Xóa lớp
common-delete-product = Xóa sản phẩm
common-delete-selection = Xóa vùng chọn
common-designs = Bản thiết kế
common-discard-layer-changes = Bỏ qua thay đổi của lớp
common-down = Xuống
common-drape-topology = Phủ lên bề mặt địa hình
common-easting = Tọa độ Đông
common-edit-object = Sửa đối tượng
common-edit-text = Sửa văn bản
common-elevation = Cao độ
common-exit-without-saving = Thoát mà không lưu
common-export-engineering-drawing = Xuất bản vẽ kỹ thuật
common-filter = Bộ lọc
common-fly-mode = Chế độ bay
common-generate-contour-lines = Tạo đường đồng mức...
common-hide-all = Ẩn tất cả
common-hide-selection = Ẩn vùng chọn
common-ignore = Bỏ qua
common-import-csv-block-model = Nhập mô hình khối CSV
common-import-dxf = Nhập DXF
common-incline-design-project = Dự án Incline Design
common-layer = Lớp
common-legend = Chú giải
common-line = Đường thẳng
common-line-weight = Độ dày nét
common-lock-all = Khóa tất cả
common-lock-selection = Khóa vùng chọn
common-m = m
common-max = Tối đa
common-merge-shell-into-topology = Gộp vỏ vào bề mặt địa hình
common-merge-shell-into-topology-ellipsis = Gộp vỏ vào bề mặt địa hình...
common-move-collar = Di chuyển miệng lỗ
common-move-design = Di chuyển thiết kế
common-move-selection = Di chuyển vùng chọn
common-new-product = Sản phẩm mới
common-no-block-models = Không có mô hình khối
common-no-design-layers = Không có lớp thiết kế
common-no-drill-holes = Không có lỗ khoan
common-no-file-chosen = Chưa chọn tệp
common-no-open-project = Không có dự án đang mở
common-no-point-clouds = Không có đám mây điểm
common-no-triangulations = Không có lưới tam giác
common-none = Không có
common-northing = Tọa độ Bắc
common-offset = Dịch chuyển
common-open = Mở
common-orientation = Hướng giấy
common-point = Điểm
common-point-cloud = Đám mây điểm
common-point-clouds = Đám mây điểm
common-polyline = Đường đa tuyến
common-polyline-layer = Đường đa tuyến trên '{ $layer }'
common-project = Dự án
common-rasters = Ảnh raster
common-redo = Làm lại
common-relimit-line = Giới hạn lại đường
common-remove-project = Gỡ dự án
common-reset-view = Đặt lại khung nhìn
common-reveal-all = Hiện tất cả
common-reveal-finder = Hiện trong Finder
common-rotate-collar = Xoay miệng lỗ
common-save-exit = Lưu và thoát
common-scale-bar = Thanh tỉ lệ
common-set-initiation-point = Đặt điểm kích nổ
common-shape = Hình dạng
common-shell = Với vỏ
common-slashes = Gạch chéo
common-slice = Cắt lát
common-slice-triangulation-z-range = Cắt lưới tam giác theo khoảng Z...
common-surface-contours = Đường đồng mức bề mặt
common-text = Văn bản
common-degree-suffix = °
common-tie-holes = Đấu nối lỗ khoan
common-triangulations = Lưới tam giác
common-trim-topology = Xén theo bề mặt địa hình...
common-undo = Hoàn tác
common-undrape-all = Bỏ phủ tất cả
common-uniform-white = Trắng đồng nhất
common-unlock-all = Mở khóa tất cả
common-untitled = Chưa đặt tên
common-up = Lên
common-vertical-exaggeration = Phóng đại đứng
common-x = x
common-zoom-extents = Thu phóng vừa khung

## Confirmations strings

confirmations-close-project-unsaved-changes = Đóng dự án: Có thay đổi chưa lưu
confirmations-close-without-saving = Đóng mà không lưu
confirmations-delete = Xóa
confirmations-delete-objects = Xóa đối tượng
confirmations-discard = Bỏ qua
confirmations-discard-all-unsaved-changes-layer =
    Bỏ qua tất cả thay đổi chưa lưu cho lớp '{ $name }'?
    Lớp đã lưu sẽ được tải lại từ đĩa trong khi thay đổi của các lớp khác vẫn được giữ. Không thể hoàn tác thao tác này.
confirmations-discard-all-unsaved-changes-name =
    Bỏ qua tất cả thay đổi chưa lưu cho '{ $name }'?
    Phiên bản đã lưu gần nhất sẽ được tải lại từ đĩa. Không thể hoàn tác thao tác này.
confirmations-discard-changes = Bỏ qua thay đổi
confirmations-exit-unsaved-changes = Thoát: Có thay đổi chưa lưu
confirmations-incline-design-cannot-reproduce-all = Incline Design không thể tái tạo toàn bộ nội dung từ tệp OMF gốc. Việc lưu sẽ bỏ qua các nội dung sau:
confirmations-product = Sản phẩm
confirmations-project = dự án này
confirmations-remove-name-delete-its-browser = Gỡ '{ $name }' và xóa bản lưu trong trình duyệt của nó? Các thay đổi chưa lưu sẽ bị mất.
confirmations-remove-project-unsaved-changes = Gỡ dự án: Có thay đổi chưa lưu
confirmations-remove-without-saving = Gỡ mà không lưu
confirmations-replace-project-unsaved-changes = Thay thế dự án: Có thay đổi chưa lưu
confirmations-save = Lưu
confirmations-save-anyway = Vẫn lưu
confirmations-save-changes-current-project-before = Lưu thay đổi của dự án hiện tại trước khi thay thế nó?
confirmations-save-changes-name-before-closing = Lưu thay đổi cho '{ $name }' trước khi đóng?
confirmations-save-changes-name-before-removing = Lưu thay đổi cho '{ $name }' trước khi gỡ khỏi Incline Design?
confirmations-save-close = Lưu và đóng
confirmations-save-modified-project-before-exiting = Lưu dự án đã sửa đổi trước khi thoát?
confirmations-save-to-browser-before-exit = Lưu dự án đã sửa đổi vào bộ nhớ trình duyệt trước khi thoát?
confirmations-save-remove = Lưu và gỡ

## Console strings

console-copy-all = Sao chép tất cả
console-copy-message = Sao chép thông báo
console-error = LỖI
console-info = THÔNG TIN
console-no-console-activity-yet = Chưa có hoạt động nào trong bảng điều khiển
console-pending = ĐANG CHỜ
console-progress-summary = Đang thực hiện · { $summary }
console-success = THÀNH CÔNG
console-warn = CẢNH BÁO

## Csv strings

csv-block-model-category = Danh mục
csv-block-model-value = Giá trị

## Drill strings

drill-hole-add-stop = Thêm điểm dừng
drill-hole-all-rendered-intervals-opaque-white = Tất cả các khoảng được hiển thị đều là màu trắng đục.
drill-hole-burden-spacing-must-greater-than = Burden và khoảng cách hàng phải lớn hơn 0
drill-hole-choose-valid-closed-polyline = Chọn một đường đa tuyến khép kín hợp lệ
drill-hole-colour-scale = Thang màu
drill-hole-field = Trường
drill-hole-grayscale = Thang xám
drill-hole-green-yellow-red = Lục–Vàng–Đỏ
drill-hole-heat = Nhiệt
drill-hole-no-holes-fit-inside-boundary = Không có lỗ khoan nào vừa bên trong ranh giới này với burden và khoảng cách hàng hiện tại
drill-hole-pattern-too-many-holes = Mẫu khoan vượt quá tối đa { $maximum } lỗ khoan; hãy tăng burden hoặc khoảng cách hàng
drill-hole-preset = Thiết lập sẵn
drill-hole-px = px
drill-hole-rainbow = Cầu vồng
drill-hole-reset-preset = Đặt lại thiết lập sẵn
drill-hole-rotation-offsets-must-contain-valid = Góc xoay và độ dịch chuyển phải chứa các số hợp lệ
drill-hole-selected-polyline-has-no-usable = Đường đa tuyến đã chọn không có diện tích XY sử dụng được
drill-hole-smooth-interpolation = Nội suy mượt
drill-hole-spacing-would-scan-too-many = Khoảng cách hàng này sẽ quét quá nhiều ô lưới; hãy tăng burden hoặc khoảng cách hàng (tối đa { $maximum } lỗ khoan)
drill-hole-square = Vuông
drill-hole-staggered = So le
drill-hole-stepped-bands = Dải bậc thang
common-times-sign = ×
common-minus-sign = −
drill-hole-unsupported-drillhole-source = Nguồn lỗ khoan không được hỗ trợ
drill-hole-width = Chiều rộng
drill-pattern-arrangement = Cách sắp xếp
drill-pattern-axis-offset = Độ lệch trục { $axis }
drill-pattern-blast-shape = Hình dạng bãi nổ
drill-pattern-burden = Burden
drill-pattern-choose-closed-blast-boundary-then = Chọn một ranh giới bãi nổ khép kín, sau đó tinh chỉnh lưới. Các lỗ khoan cập nhật trực tiếp trong khung nhìn.
drill-pattern-closed-design-polyline-whose-xy = Đường đa tuyến thiết kế khép kín có phạm vi XY sẽ được lấp đầy bằng các lỗ khoan.
drill-pattern-rotation-help = Xoay hoa văn ngược chiều kim đồng hồ từ trục toàn cục { $axis }.
drill-pattern-distance-between-holes-along-each = Khoảng cách giữa các lỗ khoan dọc theo mỗi hàng của mẫu.
drill-pattern-name-hint = ví dụ: West Cut 03
drill-pattern-diameter-help = Đường kính lỗ khoan hoàn thiện. Nhập theo milimét và lưu cùng mỗi lỗ khoan được tạo.
drill-pattern-hole-depth = Độ sâu lỗ khoan
drill-pattern-hole-diameter = Đường kính lỗ khoan
drill-pattern-move-over-closed-polyline-then = Di chuột lên một đường đa tuyến khép kín, sau đó nhấp vào nó trong khung nhìn. Esc để hủy chọn.
drill-pattern-name-help = Tên của bộ dữ liệu lỗ khoan được tạo trong dự án.
drill-pattern-none-picked = Chưa chọn
drill-pattern-pattern-name = Tên mẫu khoan
drill-pattern-spacing-help = Khoảng cách vuông góc giữa các hàng của mẫu khoan.
drill-pattern-pick = Chọn
drill-pattern-preview-count-hole-s-diameter = Xem trước: { $count } lỗ khoan · đường kính { $diameter } mm · sâu { $depth } m
drill-pattern-rotation = Xoay
drill-pattern-shift-pattern-grid-along-global = Dịch lưới hoa văn dọc theo trục toàn cục { $axis } trong khi vẫn giữ nó được cắt theo hình dạng khối nổ.
drill-pattern-spacing = Khoảng cách hàng
drill-pattern-staggered-offsets-every-second-row = So le dịch mỗi hàng thứ hai đi một nửa khoảng cách hàng.
drill-pattern-vertical-depth-below-each-collar = Độ sâu thẳng đứng bên dưới mỗi miệng lỗ.

## Dxf strings

dxf-block-nesting-too-deep = Lồng khối DXF vượt quá độ sâu tối đa ({ $depth }), bỏ qua '{ $name }'
dxf-circular-block-reference = Phát hiện tham chiếu khối vòng lặp trong DXF: '{ $name }'
dxf-undefined-layer = Đối tượng DXF tham chiếu đến lớp chưa xác định '{ $name }', đã nhập với tên '{ $fallback }'
dxf-import-budget-exceeded = Nhập DXF vượt quá ngân sách { $what } ({ $limit }); phần hình học còn lại bị bỏ qua
dxf-insert-unknown-block = DXF INSERT tham chiếu đến khối không xác định '{ $name }'

## Edit strings

edit-absolute-length = Chiều dài tuyệt đối
edit-absolute-rl = RL tuyệt đối
edit-action = Hành động
edit-angle = Góc
edit-dip-help = Góc so với phương ngang, âm là hướng xuống: -90 là lỗ khoan thẳng đứng.
edit-app-web-not-recommended-production = { $app } Web không được khuyến nghị dùng cho môi trường sản xuất. Chỉ dùng như bản demo.
edit-application = Ứng dụng
edit-apply = Áp dụng
edit-apply-pick-target = Áp dụng và chọn mục tiêu
edit-axis-value = Giá trị { $axis }
edit-azimuth = Phương vị
edit-batter-angle = Góc taluy (°)
edit-azimuth-help = Phương vị khoan của các lỗ, tính bằng độ theo chiều kim đồng hồ từ hướng bắc lưới.
edit-bench-height = Chiều cao tầng
edit-benches = Tầng
edit-berm-width = Chiều rộng cơ
edit-bezier-curve = Đường cong Bezier
edit-choose-layer = Chọn một lớp
edit-measure-help = Chọn xem giá trị nhập vào là khoảng cách dọc theo mái dốc, chiều rộng ngang, hay chiều cao đứng.
edit-choose-which-two-polyline-paths = Chọn đường nào trong hai đường đa tuyến giữa các đỉnh đã chọn sẽ được thay thế. Chiều dài bao gồm cao độ và các cạnh cong.
edit-click-corner-closed-polyline = Nhấp vào một góc trên đường đa tuyến khép kín.
edit-click-open-closed-polyline-begin = Nhấp vào một đường đa tuyến hở hoặc khép kín để bắt đầu.
edit-click-second-vertex-replacement-span = Nhấp vào đỉnh thứ hai của đoạn thay thế.
edit-click-vertex-start-replacement-span = Nhấp vào một đỉnh để bắt đầu đoạn thay thế.
edit-collide-triangulation = Va chạm với lưới tam giác
edit-confirm-selection = Xác nhận lựa chọn
edit-control-point-1 = Điểm điều khiển 1
edit-control-point-2 = Điểm điều khiển 2
edit-copy = Sao chép
edit-corner-radius-limited-so-replacement = Bán kính góc, giới hạn để phần thay thế không vượt qua các đỉnh lân cận.
edit-create-new-layer = Tạo một lớp mới
edit-create-new-project = Tạo một dự án mới
edit-create-project = Tạo dự án
edit-delta-length-m-use = Chênh lệch chiều dài (m, dùng + hoặc -)
edit-dip = Góc dốc
edit-direction = Hướng
edit-distance = Khoảng cách
edit-distance-along-slope = Khoảng cách dọc theo mái dốc
edit-download-free-native-version-our = Tải phiên bản native miễn phí tại trang web của chúng tôi ↗
edit-drill-hole = Lỗ khoan
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = Cuối
edit-enter-valid-elevation = Nhập cao độ hợp lệ.
edit-exit-slice = Thoát chế độ cắt lát
edit-finish-polyline = Hoàn tất đường đa tuyến
edit-generate-batter-berms = Tạo taluy - cơ
edit-height = Chiều cao
edit-height-change = Thay đổi chiều cao
edit-height-mode = Chế độ chiều cao
edit-horizontal-distance = Khoảng cách ngang
edit-horizontal-width-each-flat-berm = Chiều rộng ngang của mỗi cơ phẳng giữa các taluy liên tiếp.
edit-hover-choose-which-end-move = Di chuột để chọn đầu cần di chuyển, sau đó nhấp để xác nhận.
edit-insert-point-elevation = Chèn điểm tại cao độ
edit-intersect = Giao điểm
edit-kind-properties = { $kind } { $properties }
edit-layer-name = Tên lớp
edit-load-project = Tải dự án
edit-longest = Dài nhất
edit-m-s = m/s
edit-measure = Đo lường
edit-mit-license = Giấy phép MIT
edit-mode = Chế độ
edit-move = Di chuyển
edit-move-layer = Di chuyển đến lớp
edit-move-which-end = Di chuyển đầu nào
edit-movement-speed-slice-when-using = Tốc độ di chuyển của lát cắt khi dùng các phím điều hướng.
edit-moving-end-endpoint = Đang di chuyển: điểm cuối
edit-moving-start-endpoint = Đang di chuyển: điểm đầu
edit-new-length-m = Chiều dài mới (m)
edit-new-project = Dự án mới
edit-number-complete-batter-berm-levels = Số cấp taluy - cơ hoàn chỉnh. Giá trị tối đa bị giới hạn ở cấp sâu nhất giữ được hình học đã chỉ định.
edit-bezier-segments-help = Số đoạn thẳng dùng để xấp xỉ đường cong giữa hai đỉnh đã chọn.
edit-chamfer-segments-help = Số đoạn thẳng dùng để xấp xỉ góc bo tròn. Dùng 1 để vát góc thẳng.
edit-object = Đối tượng
edit-offset-element = Dịch chuyển phần tử
edit-pick-side = Chọn phía
edit-pit = Moong
edit-project-name = Tên dự án
edit-properties = Thuộc tính
edit-radius = Bán kính
edit-recent = Gần đây
edit-relative = Tương đối (+/-)
edit-elevation-mode-help = Tương đối áp dụng một thay đổi theo phương đứng cho mọi điểm. RL tuyệt đối chiếu mọi điểm lên một cao độ mục tiêu.
edit-remove-from-list = Gỡ khỏi danh sách
edit-replace-path = Thay thế đường dẫn
edit-rotate = Xoay
edit-rotation-speed-slice-when-using = Tốc độ xoay của lát cắt khi dùng phím Q và E.
edit-s = °/s
edit-segments = Đoạn
edit-segments-lying-elevation-ignored = Các đoạn nằm ở cao độ này sẽ bị bỏ qua.
edit-endpoint-help = Chọn điểm cuối sẽ thay đổi; điểm cuối còn lại giữ nguyên.
edit-selected-holes-point-different-ways = Các lỗ khoan đã chọn hướng theo nhiều cách khác nhau. Áp dụng sẽ đặt tất cả về các góc này.
edit-selected-start-end-point-moves = Điểm đầu hoặc cuối đã chọn di chuyển dọc theo hướng đường; đầu mút đối diện giữ nguyên.
edit-set-axis = Đặt { $axis }
edit-shortest = Ngắn nhất
edit-slice-view = Khung nhìn cắt lát
edit-slope-angle-each-batter-face = Góc dốc của mỗi mặt taluy, đo từ phương ngang.
edit-slope-angle-offset-positive-negative = Góc dốc của bản dịch chuyển. Góc dương và âm di chuyển bản sao lên trên hoặc xuống dưới so với nguồn khi nó di chuyển sang bên.
edit-speed = Tốc độ
edit-start = Đầu
edit-stockpile = Bãi chứa
edit-stop-generated-offset-where-its = Dừng đường dịch chuyển được tạo ra tại nơi nó chạm vào lưới tam giác hiển thị đầu tiên.
edit-target-rl = RL mục tiêu
edit-text-colour-opacity = Màu và độ mờ của văn bản.
edit-thickness-visible-slice-slab-centred = Độ dày của khối lát cắt hiển thị, lấy tâm tại chỉ báo tổng quan.
edit-translation-axis-help = Khoảng cách tịnh tiến dọc theo trục thế giới { $axis }.
edit-type = Loại
edit-type-direction-together-set-offset = Loại và Hướng cùng nhau xác định phía dịch chuyển. Moong + Lên và Bãi chứa + Xuống bước ra ngoài; Moong + Xuống và Bãi chứa + Lên bước vào trong.
edit-bench-direction-help = Lên nâng mỗi tầng lên theo chiều cao tầng; Xuống hạ thấp nó. Điều này cũng đảo phía dịch chuyển - xem Loại.
edit-value-help = Giá trị được diễn giải theo Đơn vị đo và Chế độ chiều cao đã chọn.
edit-vertical-rise-fall-each-bench = Độ tăng hoặc giảm cao độ của mỗi tầng trước khi tạo cơ tiếp theo.
edit-bezier-control-point-1-help = Tọa độ X, Y và Z toàn cục của điểm điều khiển Bezier thứ nhất.
edit-bezier-control-point-2-help = Tọa độ X, Y và Z toàn cục của điểm điều khiển Bezier thứ hai.

## Events strings

events-couldn-t-exit-error = Không thể thoát: { $error }
events-couldn-t-save-error = Không thể lưu: { $error }
events-set-elevation = Đặt cao độ
events-set-elevation-from-cursor-hit = Đã đặt cao độ từ điểm chạm con trỏ thành Z { $z }
events-tool-not-available-section-view = Công cụ đó không khả dụng trong khung nhìn mặt cắt

## Explorer strings

explorer-clear-active-triangulation-texture = Xóa kết cấu lưới tam giác đang hoạt động
explorer-delete-from-project = Xóa khỏi dự án
explorer-discard-changes = Bỏ qua thay đổi...
explorer-download = Tải xuống
explorer-drape-over-surface = Phủ lên bề mặt
explorer-draped-over-surface = Được phủ lên một bề mặt
explorer-duplicate = Nhân bản
explorer-face-colour = Màu mặt
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    { $count } biến màu
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    { $holes } lỗ
    { $fields } trường màu
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    { $count } điểm
explorer-raster-id =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = Tải
explorer-lock = Khóa
explorer-select-all-objects = Chọn tất cả đối tượng
explorer-source-name = Nguồn: { $name }
explorer-unload = Gỡ tải
explorer-unlock = Mở khóa

## Files strings

files-automatic-colour = Màu tự động
files-automatic-rl-spacing = Khoảng cách cao độ tự động
files-axis-scale-ratio = Tỷ lệ trục { $axis }
files-ok = OK
files-reset-scale = Đặt lại về 1×
files-rl-grid-options = Tùy chọn lưới cao độ
files-rl-spacing = Khoảng cách cao độ
files-scales-z-distances-visually-without = Phóng đại khoảng cách Z về mặt hiển thị mà không thay đổi tọa độ lưu trữ.
files-thickness = Độ dày
files-xy-grid-options = Tùy chọn lưới XY

## Gpu strings

gpu-cache-block-model-surface-build-failed = Dựng bề mặt mô hình khối thất bại: { $error }
gpu-cache-block-model-surface-build-worker = Tiến trình dựng bề mặt mô hình khối bị ngắt kết nối
gpu-cache-block-model-surface-chunk-rejected = Chunk bề mặt mô hình khối bị từ chối trước khi cấp phát GPU: instances={ $instances } byte, limit={ $limit } byte
gpu-cache-block-volume-worker-disconnected = Tiến trình chuẩn bị khối thể tích bị ngắt kết nối
gpu-cache-translucent-volume-could-not-built = Không thể dựng khối thể tích trong mờ ({ $error }); hiển thị mô hình khối này dưới dạng khối lập phương thay thế.
gpu-cache-edge-chunk-rejected = Chunk cạnh lưới tam giác bị từ chối trước khi cấp phát GPU: instances={ $instances } byte, limit={ $limit } byte
gpu-cache-triangulation-chunk-rejected = Chunk GPU lưới tam giác bị từ chối trước khi cấp phát: vertices={ $vertices } byte, indices={ $indices } byte, limit={ $limit } byte
gpu-cache-triangulation-too-many-vertices = Lưới tam giác '{ $name }' có { $count } đỉnh (> u32::MAX); không thể chia chunk cho GPU
gpu-cache-triangulation-uploaded = Lưới tam giác '{ $name }' đã được tải lên trong { $chunks } chunk không gian ({ $faces } mặt)
i18n-active-language = Ngôn ngữ đang hoạt động là { $language } (đóng gói sẵn: { $bundled })
i18n-could-not-select-language-error = Không thể chọn ngôn ngữ: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = Bộ điều hợp GPU: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = Driver GPU: { $driver } { $driver_info }
init-gpu-supports-maximum-buffer-size = GPU hỗ trợ kích thước bộ đệm tối đa { $size } MiB; các cảnh lớn có thể không hiển thị đầy đủ
init-surface-present-mode = Chế độ hiển thị bề mặt: { $mode }
init-wgpu-error-continuing-error = Lỗi wgpu (tiếp tục): { $error }

## Io strings

io-ascii-points-xyz-pts = ASCII Points (.xyz, .pts)
io-attribute = Thuộc tính
io-blank-header = (tiêu đề trống)
io-block-model = Mô hình khối:
io-choose-file-purpose-map-its = Chọn công dụng của tệp để ánh xạ các cột của nó.
io-choose-loaded-block-model = Chọn một mô hình khối đã tải
io-choose-loaded-layer = Chọn một lớp đã tải
io-choose-loaded-triangulation = Chọn một lưới tam giác đã tải
io-choose-purpose = Chọn công dụng…
io-choose-source-file-files-import = Chọn (các) tệp nguồn cần nhập.
io-collar = Miệng lỗ
io-column-mapping = Ánh xạ cột
io-comma-separated-values-csv = Comma-Separated Values (.csv)
io-csv-files = Tệp CSV
io-default = Mặc định
io-depth = Độ sâu
io-diameter = Đường kính
io-drawing-exchange-format-dxf = Drawing Exchange Format (.dxf)
io-drill-holes = Lỗ khoan
io-east-x = Đông / X
io-elevation-z = Cao độ / Z
io-end-x = X cuối
io-end-y = Y cuối
io-end-z = Z cuối
io-explicit-segments = Đoạn tường minh
io-export = Xuất
io-export-csv-block-model = Xuất mô hình khối CSV
io-export-dxf = Xuất DXF
io-export-one-layer = Xuất một lớp
io-export-ply = Xuất PLY
io-export-stl = Xuất STL
io-export-wavefront-obj = Xuất Wavefront OBJ
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Nhập
io-import-ascii-point-cloud = Nhập đám mây điểm ASCII
io-import-drillhole-csv-bundle = Nhập gói CSV lỗ khoan
io-import-geotiff = Nhập GeoTIFF
io-import-las-laz-point-cloud = Nhập đám mây điểm LAS/LAZ
io-import-pcd-point-cloud = Nhập đám mây điểm PCD
io-import-ply = Nhập PLY
io-import-stl = Nhập STL
io-import-wavefront-obj = Nhập Wavefront OBJ
io-interval = Khoảng
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-mapped-csv-bundle-csv = Gói CSV đã ánh xạ (.csv)
io-model-file = Tệp mô hình
io-name-count-files = { $name } + { $count } tệp
io-no-csv-chosen = Chưa chọn tệp .csv
io-no-csv-files-chosen = Chưa chọn tệp CSV
io-no-dxf-chosen = Chưa chọn tệp .dxf
io-no-omf-chosen = Chưa chọn tệp .omf
io-north-y = Bắc / Y
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-source-file = Tệp nguồn
io-start-x = X đầu
io-start-y = Y đầu
io-start-z = Z đầu
io-stl = STL (.stl)
io-triangulation = Lưới tam giác:
io-unmapped = Chưa ánh xạ
io-wavefront-obj = Wavefront OBJ (.obj)

## Jobs strings

jobs-background-task-poll-label-ended = Tác vụ nền '{ $poll_label }' đã kết thúc mà không có kết quả
jobs-discarded-stale-result = Đã bỏ qua kết quả nền cũ cho '{ $poll_label }' vì nguồn đã thay đổi hoặc đóng

## Logging strings

logging-activity-completed = Hoạt động đã hoàn tất
logging-activity-started = Hoạt động đã bắt đầu
logging-application-id-id = ID ứng dụng: { $id }
logging-application-name = Tên ứng dụng: { $name }
logging-application-startup = Khởi động ứng dụng
logging-build-target-os-architecture = Build cho: { $os }-{ $architecture }
logging-completed = Hoàn tất
logging-count-messages = { $count } thông báo
logging-desktop-session-xdg-session-type = Phiên desktop: XDG_SESSION_TYPE={ $session }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Đang khởi tạo Incline Design
logging-locale-environment = Môi trường ngôn ngữ: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session = Phiên macOS: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = Hệ điều hành: GNU/Linux
logging-operating-system-macos = Hệ điều hành: macOS
logging-operating-system-microsoft-windows = Hệ điều hành: Microsoft Windows
logging-pointer-width = Độ rộng con trỏ: { $width }-bit
logging-process-id-id = ID tiến trình: { $id }
logging-release-version = Phiên bản phát hành: { $version }
logging-renderer = Bộ dựng hình
logging-rust-compiler-host = Máy chủ trình biên dịch Rust: { $host }
logging-system = Hệ thống
logging-system-error = Lỗi hệ thống
logging-unknown = không xác định
logging-windows-session-sessionname-session = Phiên Windows: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = Đang xử lý…

## Mac strings

mac-cannot-install-macos-menu-bar = Không thể cài đặt thanh menu macOS ngoài luồng chính
mac-quit-app = Thoát { $app }

## Main strings

main-incline-design-web-startup-failed = Khởi động Incline Design Web thất bại: { $error }

## Menu strings

menu-count-files-selected = Đã chọn { $count } tệp

## Object strings

object-edit-appearance = Giao diện
object-edit-arc-circle = Cung & Vòng tròn
object-edit-arc-segments = Đoạn cung
object-edit-bulge = Độ phình
object-edit-bulge-arcs-horizontal-data-model = Theo mô hình dữ liệu, các cung phình nằm ngang: cung uốn cong trên mặt bằng, còn cao độ thay đổi theo đường thẳng từ đỉnh này sang đỉnh tiếp theo.
object-edit-centre-x = Tâm X
object-edit-centre-y = Tâm Y
object-edit-centre-z = Tâm Z
object-edit-chord = Dây cung
object-edit-colour-layer = Màu theo lớp
object-edit-enter-number = Nhập một số
object-edit-follow-owning-layer-s-colour = Dùng màu của lớp sở hữu thay vì màu được gán cố định cho đối tượng này.
object-edit-id = ID
object-edit-identity = Đồng nhất
object-edit-insert-after = Chèn sau
object-edit-join-last-vertex-back-first = Nối đỉnh cuối cùng trở lại đỉnh đầu tiên.
object-edit-length = Chiều dài { $length } m
object-edit-move-down = Chuyển xuống
object-edit-move-up = Chuyển lên
object-edit-object-has-no-arc-segments = Đối tượng này không có đoạn cung nào.
object-edit-object-has-single-position = Đối tượng này chỉ có một vị trí.
object-edit-object-needs-least-required-vertices = Đối tượng này cần ít nhất { $required } đỉnh
object-edit-one-more-properties-not-valid = Một hoặc nhiều thuộc tính không phải là số hợp lệ
object-edit-perimeter-area = Chu vi { $length } m, diện tích { $area } m²
object-edit-reverse = Đảo ngược
object-edit-row-invalid-number = Hàng { $row }: vị trí hoặc độ phình không phải là số hợp lệ
object-edit-sweep = Góc quét
object-edit-text-not-number = "{ $text }" không phải là số
object-edit-vertices = Đỉnh

## Omf strings

omf-element-name-has-count-tie = Phần tử '{ $name }' có { $count } đấu nối đặt tên các lỗ khoan mà nó không còn chứa
omf-ignoring-colour-map-omf-attribute = Bỏ qua bảng màu trên thuộc tính OMF '{ $attribute }': { $error }
omf-mining-data-exported-incline = Dữ liệu khai thác được xuất bởi Incline
omf-import = Nhập OMF
omf-texture = Kết cấu OMF
omf-validation-warnings = Cảnh báo kiểm tra OMF: { $warnings }
omf-application-metadata-dropped = Siêu dữ liệu ứng dụng dự án '{ $application }' không được giữ lại
omf-project-author-not-retained = Tác giả dự án không được giữ lại
omf-project-description-not-retained = Mô tả dự án không được giữ lại
omf-unsupported-metadata-keys = Dự án có các khóa siêu dữ liệu không được hỗ trợ: { $keys }

## Plot strings

plot-1-1000-one-millimetre-sheet = Ở tỉ lệ 1:1000, một milimét trên bản vẽ tương ứng một mét ngoài thực địa.
plot-1-scale-covers-width-height = 1:{ $scale } · bao phủ { $width } × { $height } m
plot-all-visible-data = Tất cả dữ liệu hiển thị
plot-automatic-grid-interval = Khoảng lưới tự động
plot-border = Viền
plot-centre = Lấy tâm tại
plot-fit-scale-help = Chọn tỉ lệ chuẩn nhỏ nhất vừa đủ để hiển thị mọi thứ trên bản vẽ.
plot-coordinate-grid = Lưới tọa độ
plot-current-view-centre = Tâm khung nhìn hiện tại
plot-date-caps = NGÀY
plot-date = Ngày
plot-dots-per-inch-paper-size = Số điểm ảnh trên inch. Khổ giấy này có thể được rasterize tối đa { $max_dpi } dpi; 300 dpi là chất lượng in thông thường.
plot-dpi = dpi
plot-drawing-no = SỐ BẢN VẼ
plot-drawing-number = Số hiệu bản vẽ
plot-drawn-by-caps = NGƯỜI VẼ
plot-drawn-by = Người vẽ
plot-e-g-example-gold-project = ví dụ: Example Gold Project
plot-entered-coordinates = Tọa độ đã nhập
plot-export-png = Xuất PNG...
plot-fit-scale-visible-data = Vừa khít tỉ lệ với dữ liệu hiển thị
plot-grid-interval = Khoảng lưới
plot-landscape = Ngang
plot-lists-visible-surfaces-design-layers = Liệt kê các bề mặt và lớp thiết kế đang hiển thị cùng màu của chúng.
plot-margin = Lề
plot-margins-leave-no-room-map = Lề không để lại không gian cho bản đồ
plot-metres-scale-1-scale = mét    Tỉ lệ 1:{ $scale }
plot-mm = mm
plot-north-arrow = Mũi tên chỉ Bắc
plot-nothing-visible-draw = Không có gì hiển thị để vẽ
plot-paper = Giấy
plot-paper-orientation-width-height-mm = { $paper } { $orientation } · { $width } × { $height } mm
plot-paper-size = Kích thước giấy
plot-pick-interval-reads-roughly-every = Chọn khoảng cách đọc được xấp xỉ mỗi 50 mm trên bản vẽ in.
plot-plan = Mặt bằng
plot-scale-must-be-positive = Tỉ lệ bản vẽ phải là số dương
plot-png-written-sheet-s-exact = Tệp PNG được ghi đúng kích thước giấy của bản vẽ và ghi lại DPI của nó, để in đúng tỉ lệ.
plot-portrait = Dọc
plot-resolution = Độ phân giải
plot-rev = SỬA ĐỔI
plot-revision = Bản sửa đổi
plot-scale = TỈ LỆ
plot-scale-ratio = Tỉ lệ  1:
plot-scale-framing = Tỉ lệ và bố cục
plot-sheet-furniture = Khung tên bản vẽ
plot-size-width-height-mm = { $size } ({ $width } × { $height } mm)
plot-subtitle = Phụ đề
plot-title = Tiêu đề
plot-title-block = Khung tên
plot-today = hôm nay

## Products strings

products-add-initiation = Thêm điểm kích nổ
products-delay = Độ trễ
products-delay-palette = Bảng độ trễ
products-how-long-after-shot-fired = Khoảng thời gian sau khi phát nổ mà miệng lỗ này kích nổ đợt bắn.
products-initiation-name = Kích nổ · { $name }
products-milliseconds-between-one-hole-firing = Số mili-giây giữa lần nổ của lỗ này và lỗ tiếp theo.
products-ms = ms
products-no-products = Không có sản phẩm
products-remove = Gỡ
products-update = Cập nhật

## Progress strings

progress-percent-done-total = { $percent } ({ $done } trong { $total })
progress-task-finished = { $task }: Hoàn tất

## Project strings

project-item = Mục

## Properties strings

properties-adds-view-dependent-rim-highlight = Thêm hiệu ứng viền sáng phụ thuộc góc nhìn tại ranh giới khối và vật liệu. Tắt tùy chọn này sẽ giảm nhẹ khối lượng công việc dựng hình khối.
properties-block-model-downscale = Giảm phân giải mô hình khối
properties-camera = Camera
properties-camera-clip-planes = Mặt phẳng cắt camera
properties-cap-while-resizing = Giới hạn khi thay đổi kích thước
properties-dark-mode = Chế độ tối
properties-developer = Nhà phát triển
properties-downscale-rasters = Giảm phân giải ảnh raster
properties-edit-object = Sửa đối tượng...
properties-field-view = Góc nhìn
properties-fps = FPS
properties-frame-counter = Bộ đếm khung hình
properties-frame-rate-cap = Giới hạn tốc độ khung hình
properties-hz = Hz
properties-interface = Giao diện
properties-invert-horizontal = Đảo chiều ngang
properties-invert-vertical = Đảo chiều dọc
properties-limits-newly-loaded-geotiff-previews = Giới hạn ảnh xem trước GeoTIFF mới tải ở mức 4096 pixel trên cạnh dài nhất. Tắt để dùng độ phân giải đầy đủ tới giới hạn kết cấu của GPU, sẽ dùng nhiều bộ nhớ hơn.
properties-line-colour = Màu đường
properties-look-sensitivity = Độ nhạy quan sát
properties-max-clip-span = Phạm vi cắt tối đa
properties-move-layer = Di chuyển đến lớp...
properties-near-clip-limit = Giới hạn cắt gần
properties-orbit-sensitivity = Độ nhạy xoay quỹ đạo
properties-panel-chrome = Khung bảng điều khiển
properties-performance = Hiệu năng
properties-plan-mode = Chế độ mặt bằng
properties-presents-step-display-no-tearing = Hiển thị đồng bộ với màn hình: không xé hình, và màn hình quyết định tốc độ khung hình. Khi tắt, khung hình được hiển thị ngay khi vẽ xong và giới hạn bên dưới sẽ áp dụng.
properties-reflective-block-edges = Cạnh khối phản chiếu
properties-restore-defaults = Khôi phục mặc định
properties-show-console = Hiện bảng điều khiển
properties-shows-live-near-far-projection = Hiển thị khoảng cách chiếu gần và xa theo thời gian thực trên thanh trạng thái.
properties-snap-polling = Thăm dò bắt điểm
properties-vertical-sync = Đồng bộ dọc
properties-world-axis-gizmo = Gizmo trục tọa độ
properties-zoom-cursor = Thu phóng theo con trỏ
properties-zoom-sensitivity = Độ nhạy thu phóng

## Screenshot strings

screenshot-could-not-encode-viewport-image = Không thể mã hóa ảnh khung nhìn: { $error }
screenshot-could-not-map-viewport-screenshot = Không thể ánh xạ ảnh chụp khung nhìn: { $error }
screenshot-could-not-save-viewport-image = Không thể lưu ảnh khung nhìn { $path }: { $error }
screenshot-downloaded-viewport-image-file-name = Đã tải xuống ảnh khung nhìn: { $file_name }
screenshot-saved-viewport-image-path = Đã lưu ảnh khung nhìn: { $path }
screenshot-viewport-image-download-failed-error = Tải xuống ảnh khung nhìn thất bại: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = Chỉ số mặt BVH { $index } ngoài phạm vi của lưới; đang thay bằng tam giác suy biến

## State strings

state-above = tại hoặc trên
state-activate-project = Kích hoạt dự án
state-all-open-incline-design-data = Tất cả dữ liệu Incline Design đang mở
state-apply-generated-rings = Áp dụng các vòng đã tạo
state-apply-selection = Áp dụng cho vùng chọn
state-rotate-by-azimuth-dip = theo phương vị { $azimuth }°, góc dốc { $dip }°
state-rotate-to-azimuth-dip = đến phương vị { $azimuth }°, góc dốc { $dip }°
state-below = tại hoặc dưới
state-centre-rotation = Tâm xoay
state-checking-unsaved-work = Đang kiểm tra công việc chưa lưu
state-choose-destination = Chọn nơi lưu
state-choose-one-more-files = Chọn một hoặc nhiều tệp
state-clear-raster = Xóa ảnh raster
state-click-pit-shell-viewport = Nhấp vào vỏ moong trong khung nhìn.
state-click-pit-stockpile-solid-viewport = Nhấp vào khối moong hoặc bãi chứa trong khung nhìn.
state-click-surface-viewport = Nhấp vào bề mặt trong khung nhìn.
state-click-topology-viewport = Nhấp vào bề mặt địa hình trong khung nhìn.
state-close-project = Đóng dự án
state-colour-drillholes = Tô màu lỗ khoan
state-copy-objects-layer = Sao chép đối tượng sang lớp
state-count-file-s = { $count } tệp
state-count-object-s-axis-value = { $count } đối tượng · { $axis } { $value }
state-count-object-s-closed = { $count } đối tượng · { $closed }
state-count-object-s-layer = { $count } đối tượng · { $layer }
state-count-object-s-weight = { $count } đối tượng · { $weight }
state-count-object-s-z-elevation = { $count } đối tượng · Z { $elevation }
state-create-point-cloud-tin = Tạo TIN đám mây điểm
state-create-project = Tạo dự án
state-current-project = Dự án hiện tại
state-cut-topology-pit-shell = Cắt bề mặt địa hình theo vỏ moong
state-cut-triangulation-polyline = Cắt lưới tam giác theo đường đa tuyến
state-cut-triangulation-z = Cắt lưới tam giác theo Z
state-dark-mode = Chế độ tối
state-detached = Đã tách rời
state-disabled = Đã tắt
state-discard-project-changes = Bỏ qua thay đổi của dự án
state-discard-replace-project = Bỏ qua và thay thế dự án
state-discarding-unsaved-changes = Đang bỏ qua thay đổi chưa lưu
state-docked = Đã ghép
state-drape-raster = Phủ ảnh raster
state-drill-pattern = Mẫu khoan
state-duplicate-layer = Nhân bản lớp
state-east = Đông
state-enabled = Đã bật
state-exit-incline-design = Thoát Incline Design
state-export-block-model-csv = Xuất CSV mô hình khối
state-export-layer-dxf = Xuất lớp sang DXF
state-export-omf = Xuất OMF
state-export-project-dxf = Xuất dự án sang DXF
state-export-triangulation = Xuất lưới tam giác
state-export-viewport-image = Xuất ảnh khung nhìn
state-finish-closed-polyline = Hoàn tất đường đa tuyến khép kín
state-finish-open-polyline = Hoàn tất đường đa tuyến hở
state-fit-extents = Vừa khung
state-fix-release-centre-both-views = Cố định hoặc giải phóng tâm mà cả hai khung nhìn xoay quanh
state-generate-contours = Tạo đường đồng mức
state-hidden = Đã ẩn
state-import-drillholes = Nhập lỗ khoan
state-import-omf = Nhập OMF
state-import-point-cloud = Nhập đám mây điểm
state-import-raster = Nhập ảnh raster
state-import-triangulation = Nhập lưới tam giác
state-insert-intersection-points = Chèn điểm giao
state-insert-points-elevation = Chèn các điểm tại cao độ
state-keep-inside = Giữ bên trong
state-keep-outside = Giữ bên ngoài
state-kriged-block-model = Mô hình khối Kriging
state-load-block-model = Tải mô hình khối
state-load-drillholes = Tải lỗ khoan
state-load-layer = Tải lớp
state-load-point-cloud = Tải đám mây điểm
state-load-raster = Tải ảnh raster
state-load-triangulation = Tải lưới tam giác
state-locked-count-object-s = Đã khóa { $count } đối tượng
state-major-minor = Chính { $major } · phụ { $minor }
state-move-axis-value = Di chuyển đến giá trị trục
state-move-objects-layer = Di chuyển đối tượng sang lớp
state-name-count-holes = { $name } · { $count } lỗ khoan
state-name-count-object-s = { $name } · { $count } đối tượng
state-name-z-min-z-max = { $name } · { $z_min } đến { $z_max }
state-next-edit = Chỉnh sửa sau
state-north = Bắc
state-open-containing-folder = Mở thư mục chứa
state-open-project = Mở dự án
state-preserve-view-angle = Giữ nguyên góc nhìn
state-previous-edit = Chỉnh sửa trước
state-project-id = Dự án { $id }
state-remove-block-model = Gỡ mô hình khối
state-remove-drillholes = Gỡ lỗ khoan
state-remove-point-cloud = Gỡ đám mây điểm
state-remove-raster = Gỡ ảnh raster
state-remove-triangulation = Gỡ lưới tam giác
state-removed-from-active-triangulation = Đã gỡ khỏi lưới tam giác đang hoạt động
state-removed-from-every-triangulation = Đã gỡ khỏi mọi lưới tam giác
state-rename-kind = Đổi tên { $kind }
state-save-close-project = Lưu và đóng dự án
state-save-despite-unsupported-content = Vẫn lưu dù có nội dung không được hỗ trợ
state-save-project = Lưu dự án thành
state-save-replace-project = Lưu và thay thế dự án
state-saving-current-project = Đang lưu dự án hiện tại
state-section-name = phần { $section }
state-select-layer-objects = Chọn đối tượng của lớp
state-selected-objects = Đối tượng đã chọn
state-selected-polylines = Đường đa tuyến đã chọn
state-selected-scene-elements = Các phần tử cảnh đã chọn
state-set-block-model-variable = Đặt biến mô hình khối
state-set-drillhole-colour-preset = Đặt thiết lập màu lỗ khoan
state-set-entity-lock = Đặt khóa đối tượng
state-set-grid = Đặt lưới
state-set-layer-lock = Đặt khóa lớp
state-set-line-weight = Đặt độ dày nét
state-set-object-colour = Đặt màu đối tượng
state-set-object-fill = Đặt tô đối tượng
state-set-point-visibility = Đặt hiển thị điểm
state-set-polyline-closed = Đặt đường đa tuyến khép kín
state-set-raster-lock = Đặt khóa ảnh raster
state-set-standard-view = Đặt khung nhìn chuẩn
state-set-topology-wireframes = Đặt khung dây bề mặt địa hình
state-set-triangulation-colour = Đặt màu lưới tam giác
state-show-console = Hiện bảng điều khiển
state-show-project = Hiện dự án
state-shown = Đang hiện
state-slice-mode = Chế độ cắt lát
state-slice-preview = Xem trước lát cắt
state-south = Nam
state-stem-contours = Đường đồng mức { $stem }
state-target-new-name = { $target } thành “{ $new_name }”
state-trim-above = Xén phía trên
state-trim-below = Xén phía dưới
state-trim-triangulation-surface = Xén lưới tam giác theo bề mặt
state-undrape-raster = Bỏ phủ ảnh raster
state-undrape-rasters = Bỏ phủ ảnh raster
state-unload-block-model = Gỡ tải mô hình khối
state-unload-drillholes = Gỡ lỗ khoan
state-unload-layer = Gỡ lớp
state-unload-point-cloud = Gỡ đám mây điểm
state-unload-raster = Gỡ tải ảnh raster
state-unload-triangulation = Gỡ tải lưới tam giác
state-untitled-project = Dự án chưa đặt tên
state-use-typed-radius = Dùng bán kính đã nhập
state-west = Tây

## Status strings

status-clip-near-far = Cắt gần/xa/Δ: -- / -- / --
status-frame-rate = Tốc độ khung hình

## Text strings

text-could-not-build-vector-mesh = Không thể dựng lưới vector cho phông { $font }, ký tự { $glyph }: { $error }
text-document-text-mesh-exceeded-its = Lưới văn bản tài liệu vượt quá phạm vi chỉ số u32

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = Chọn bộ dữ liệu lỗ khoan cần đấu nối trước
tie-in-count-connector-s = { $count } đầu nối
tie-in-delete-tie-ins = Xóa đấu nối
tie-in-deleted-count-selected-tie-connector = Đã xóa { $count } đầu nối đấu nối đã chọn
tie-in-hole = lỗ khoan
tie-in-initiation-point-lifted-from-name = Điểm kích nổ được gỡ khỏi { $name }
tie-in-initiation-point-set-name-delay = Điểm kích nổ được đặt trên { $name } tại { $delay } ms
tie-in-select-delay-product-palette-before = Chọn một sản phẩm độ trễ trong bảng trước khi đấu nối các lỗ khoan
tie-in-tied-connectors = Đã đấu nối { $count } đầu nối tại { $delay } ms với { $product }
tie-in-tied-connectors-replacing = Đã đấu nối { $count } đầu nối tại { $delay } ms với { $product }, thay thế { $replaced }

## Toolbar strings

toolbar-fill-type = Kiểu tô

## Toolbars strings

toolbars-auto-bench = Tự động tạo tầng
toolbars-bezier-polyline = Đường đa tuyến Bezier
toolbars-chamfer-polyline-corners = Vát góc đường đa tuyến
toolbars-create-text = Tạo văn bản
toolbars-cursor-regular = Con trỏ: Thông thường
toolbars-cursor-snap-line = Con trỏ: Bắt vào đường
toolbars-cursor-snap-point = Con trỏ: Bắt vào điểm
toolbars-cursor-snap-surface = Con trỏ: Bắt vào bề mặt
toolbars-delete-points = Xóa điểm
toolbars-explode-polyline-lines = Phân rã đường đa tuyến thành các đoạn
toolbars-fuse-polylines = Hợp nhất đường đa tuyến
toolbars-measure-distance = Đo khoảng cách
toolbars-new-layer = Lớp mới
toolbars-split-polyline-points = Chia tách đường đa tuyến tại điểm
toolbars-strike-dip = Hướng dốc và góc dốc
toolbars-tool-not-available-section-view = { $tool } - không khả dụng trong khung nhìn mặt cắt

## Tri strings

tri-sampling-method-help = Thích ứng tập trung đỉnh vào địa hình phức tạp dựa trên sai số khớp mặt phẳng; đều phân bố đỉnh đồng đều. Có thể bổ sung thêm phương pháp trong tương lai.
tri-adaptive-quadtree = Thích ứng (quadtree)
tri-axis-range = Phạm vi trục { $axis }
tri-base-topology-will-receive-pit = Bề mặt địa hình cơ sở sẽ tiếp nhận hình dạng moong hoặc bãi chứa.
tri-boundary-polyline = Đường đa tuyến ranh giới
tri-bridge-gaps-help = Bắc cầu qua các khoảng trống và chỗ lõm ranh giới hẹp hơn giá trị này trên bề mặt. Giá trị 0 vẫn bắc cầu qua các khoảng trống xấp xỉ kích thước ô lấy mẫu; giá trị lớn hơn lấp lỗ lớn hơn và làm mòn các chỗ lõm ranh giới.
tri-budget = Ngân sách theo
tri-cancel-pick = Hủy chọn
tri-candidate-detail = Chi tiết ứng viên
tri-candidate-fine-cells-per-budgeted = Số ô mịn ứng viên trên mỗi đỉnh trong ngân sách. Giá trị cao hơn cho bộ lấy mẫu thích ứng nhiều tự do hơn để đặt chi tiết, nhưng dựng chậm hơn.
tri-cap-surface-share-source-points = Giới hạn bề mặt theo tỷ lệ điểm nguồn hoặc theo số đỉnh chính xác.
tri-choose-input-clicking-loaded-surface = Chọn dữ liệu đầu vào này bằng cách nhấp vào một bề mặt đã tải trong khung nhìn
tri-choose-which-side-reference-topology = Chọn phía nào của bề mặt địa hình tham chiếu sẽ bị loại bỏ khỏi bề mặt, trong vùng XY chung của chúng.
tri-clip = Cắt
tri-clip-creates-new-triangulation-name = Thao tác cắt sẽ tạo một lưới tam giác mới với tên này; bề mặt nguồn không bị thay đổi.
tri-clip-surface-polyline = Cắt bề mặt theo đường đa tuyến
tri-closed-pit-stockpile-solid-whose = Một khối moong hoặc bãi chứa khép kín có ranh giới lộ ra sẽ được đưa vào kết quả.
tri-create-new-layer-contours-append = Tạo một lớp mới cho các đường đồng mức hoặc thêm chúng vào một lớp có sẵn trong dự án đang hoạt động.
tri-cut-topology-pit-shell = Cắt bề mặt địa hình bằng vỏ moong
tri-e-g-design-trimmed = ví dụ: design_trimmed
tri-e-g-mysurf-cut = ví dụ: mysurf_cut
tri-e-g-mysurf-slice = ví dụ: mysurf_slice
tri-e-g-surface-contour = ví dụ: surface_contour
tri-e-g-topo-cut = ví dụ: topo_cut
tri-e-g-topo-pit = ví dụ: topo_with_pit
tri-exact-number-surface-vertices-target = Số đỉnh bề mặt chính xác cần đạt được. Giá trị rất lớn sẽ dựng chậm và dùng nhiều bộ nhớ.
tri-existing-ground-topology-will-cut = Bề mặt địa hình hiện có sẽ bị cắt bởi vỏ moong.
tri-fill-holes-up = Lấp lỗ trống đến
tri-generate = Tạo
tri-generate-contour-lines = Tạo đường đồng mức
tri-generate-upper-surface = Tạo bề mặt trên
tri-hide-unload-sources = Ẩn và gỡ nguồn
tri-higher-edge-will-enforced-each = Cạnh cao hơn sẽ được áp dụng tại mỗi xung đột. Các đoạn xung đột thấp hơn sẽ bị bỏ qua như đường gãy, và bề mặt sẽ nội suy qua các khu vực đó. Các đường đa tuyến nguồn không thay đổi.
tri-breaklines-cross = Các cạnh đường gãy được tô sáng cắt hoặc chồng lên nhau trên mặt bằng ở các cao độ khác nhau. Một bề mặt địa hình không thể theo cả hai.
tri-intervals-colours = Khoảng & màu sắc
tri-keep-clipped-topology-included-shape = Giữ bề mặt địa hình đã cắt và hình dạng được đưa vào dưới dạng các lưới tam giác riêng biệt thay vì gộp chúng thành một đối tượng.
tri-keep-inside-discards-surface-outside = Giữ bên trong sẽ loại bỏ phần bề mặt bên ngoài đường đa tuyến. Giữ bên ngoài sẽ cắt một lỗ hình đường đa tuyến khỏi bề mặt.
tri-keeps-only-surface-within-polyline = Chỉ giữ lại phần bề mặt bên trong ranh giới đường đa tuyến.
tri-keep-surface-relation-help = Giữ bề mặt { $relation } bề mặt địa hình trong phạm vi phủ XY của nó.
tri-layer-already-exists-select-above = Lớp đó đã tồn tại; chọn nó ở trên hoặc chọn tên khác.
tri-limit-z-range = Giới hạn khoảng Z
tri-major = Chính
tri-max-edge-length = Chiều dài cạnh tối đa
tri-merge = Gộp
tri-method = Phương pháp
tri-min = Tối thiểu
tri-minimum-maximum-elevations-retained = Cao độ tối thiểu và tối đa được giữ lại trong bề mặt đầu ra. Giá trị tối thiểu phải nhỏ hơn giá trị tối đa.
tri-minor = Phụ
tri-contour-interval-help = Phụ điều khiển các đường đồng mức thông thường. Chính điều khiển các đường đồng mức nhấn mạnh và phải dùng khoảng lớn hơn hoặc bằng Phụ.
tri-move-cursor-over-loaded-surface = Di chuột lên một bề mặt đã tải.
tri-slice-output-name-help = Tên gán cho bề mặt đầu ra đã cắt theo cao độ.
tri-name-assigned-merged-topology-pit = Tên gán cho kết quả gộp giữa bề mặt địa hình và moong/bãi chứa.
tri-name-assigned-newly-created-contour = Tên gán cho lớp đường đồng mức mới tạo.
tri-reconstruct-output-name-help = Tên gán cho lưới tam giác được tái tạo.
tri-name-assigned-topology-after-pit = Tên gán cho bề mặt địa hình sau khi vỏ moong được cắt khỏi đó.
tri-name-assigned-trimmed-output-surface = Tên gán cho bề mặt đầu ra đã được xén.
tri-nearby-breakline-vertices-do-not = Các đỉnh đường gãy lân cận không trùng chính xác cùng một vị trí, nên bề mặt không thể tạo lưới tam giác.
tri-new-layer = Lớp mới
tri-new-layer-name = Tên lớp mới
tri-once-merge-succeeds-unload-source = Sau khi gộp thành công, hãy gỡ tải bề mặt địa hình và khối đặc nguồn để chỉ kết quả đã gộp còn lại trong cảnh.
tri-only-loaded-pickable = Chỉ có thể chọn các lưới tam giác đã tải.
tri-operation = Thao tác
tri-output-layer = Lớp đầu ra
tri-percentage = Phần trăm
tri-percentage-cloud = Phần trăm đám mây
tri-pick-from-view = Chọn từ khung nhìn
tri-pit-design-surface-only-areas = Bề mặt thiết kế moong. Chỉ những khu vực nó đào sâu hơn bề mặt địa hình mới được dùng để cắt.
tri-pit-shell = Vỏ moong
tri-pit-stockpile-solid = Khối moong/bãi chứa
tri-recommended-weld-retry = Khuyến nghị: Hàn & Thử lại
tri-reconstruct-help = Tái tạo bề mặt địa hình dạng lưới tam giác từ đám mây điểm. Bộ lấy mẫu thích ứng dùng ngân sách đỉnh ở những nơi địa hình phức tạp nhất và giữ thưa ở các vùng phẳng.
tri-reduce-budget-candidate-detail-if = Giảm ngân sách hoặc chi tiết ứng viên nếu máy của bạn có ít RAM hơn.
tri-reference-topology-help = Bề mặt địa hình tham chiếu xác định nơi bề mặt kia bị xén.
tri-reject-reconstructed-triangle-edges = Loại bỏ các cạnh tam giác được tái tạo dài hơn khoảng cách này. Dùng 0 để không giới hạn chiều dài cạnh.
tri-remove-inside-help = Loại bỏ phần bề mặt bên trong ranh giới đường đa tuyến và giữ lại phần còn lại.
tri-removes-topology-where-pit-shell = Loại bỏ bề mặt địa hình ở nơi vỏ moong đào sâu hơn nó, để vỏ lấp đầy khoảng trống. Đường nối theo đúng đường tiếp xúc 3D thực giữa các bề mặt; bề mặt địa hình bên dưới các phần của vỏ nhô cao hơn mặt đất được giữ nguyên.
tri-result = Kết quả
tri-save-two-entities = Lưu thành hai đối tượng
tri-select = Chọn…
tri-share-source-points-keep-fractions = Tỷ lệ điểm nguồn cần giữ lại. Cho phép các giá trị phân số như 0,125%.
tri-slice-triangulation-z-range = Cắt lưới tam giác theo khoảng Z
tri-solution-generate-upper-surface = Giải pháp: Tạo bề mặt trên
tri-surface-trim = Bề mặt cần xén
tri-target-surface-help = Bề mặt sẽ bị thay đổi; bề mặt địa hình đã chọn được giữ nguyên.
common-percent-suffix = %
tri-topology = Bề mặt địa hình
tri-triangulation-failed = Tạo lưới tam giác thất bại
tri-trim = Xén
tri-trim-topology = Xén theo bề mặt địa hình
tri-uniform-grid = Lưới đều
tri-up-target-point-count-points = Tối đa { $target } trong { $point_count } điểm sẽ trở thành đỉnh bề mặt ({ $percent }%).
tri-use-full-surface-elevation-range = Dùng toàn bộ khoảng cao độ bề mặt
tri-vertex-count = Số đỉnh
tri-vertices-within-5-cm-xy = Các đỉnh trong phạm vi 5 cm theo XY và Z sẽ dùng chung một vị trí cho lưới tam giác này. Điều này có thể làm dịch bề mặt được tạo ra cục bộ tới 5 cm; các đường đa tuyến nguồn không thay đổi.
tri-weld-retry = Hàn & Thử lại
tri-when-enabled-generate-contours-only = Khi bật, chỉ tạo đường đồng mức giữa các cao độ tối thiểu và tối đa đã chỉ định.

## Ui strings

ui-choose-offset-side = Chọn phía dịch chuyển
ui-choose-relimit-side = Chọn phía giới hạn lại
ui-click-circle-centre = Nhấp vào tâm đường tròn
ui-click-closed-polyline-use-blast = Nhấp vào một đường đa tuyến khép kín để dùng làm hình dạng bãi nổ
ui-click-collar-add-edit-initiation = Nhấp vào một miệng lỗ để thêm hoặc sửa điểm kích nổ
ui-click-first-point-slice-line = Nhấp vào điểm đầu tiên của đường cắt lát
ui-click-first-vertex = Nhấp vào đỉnh đầu tiên
ui-click-perimeter-point-type-radius = Nhấp vào một điểm trên chu vi hoặc nhập bán kính
ui-click-second-point-slice-line = Nhấp vào điểm thứ hai của đường cắt lát
ui-click-second-vertex = Nhấp vào đỉnh thứ hai
ui-click-use-pointer-radius = hoặc nhấp để dùng bán kính theo con trỏ
ui-could-not-copy-text-browser = Không thể sao chép văn bản vào bảng nhớ tạm của trình duyệt: { $error }
ui-dip-horizontal-no-strike = { $dip } (nằm ngang, không có hướng dốc)
ui-distance-meters = { $distance } mét
ui-drag-ring-type-azimuth-dip = Kéo một vòng hoặc nhập phương vị và góc dốc
ui-each-hole-turns-about-its = mỗi lỗ khoan xoay quanh miệng lỗ của chính nó
ui-enter-positive-decimal-radius = Nhập bán kính thập phân dương
ui-esc-cancels = Esc để hủy
ui-no-delay-product-tie = Không có kíp vi sai nào để nối
ui-press-enter-use-typed-radius = Nhấn Enter để dùng bán kính đã nhập
ui-right-click-delay-palette-heading = nhấp chuột phải vào tiêu đề bảng độ trễ để thêm một cái
ui-select-designs = Chọn bản thiết kế
ui-select-drill-hole = Chọn một lỗ khoan
ui-select-endpoint-join = Chọn điểm cuối để nối
ui-select-first-crest-toe-point = Chọn điểm đỉnh/chân đầu tiên
ui-select-item = Chọn một mục
ui-select-line-fuse = Chọn một đường để hợp nhất
ui-select-line-polyline = Chọn một đường thẳng hoặc đường đa tuyến
ui-select-line-relimit = Chọn đường cần giới hạn lại
ui-select-next-line-fuse = Chọn đường tiếp theo để hợp nhất
ui-select-opposite-berm-point = Chọn điểm cơ đối diện
ui-select-point = Chọn một điểm
ui-select-polyline = Chọn một đường đa tuyến
ui-select-polyline-open-line = Chọn một đường đa tuyến hoặc đường hở
ui-select-polyline-vertex = Chọn một đỉnh của đường đa tuyến
ui-select-second-crest-toe-point = Chọn điểm đỉnh/chân thứ hai
ui-select-second-split-point = Chọn điểm chia tách thứ hai
ui-select-split-point = Chọn một điểm chia tách
ui-select-topologies = Chọn bề mặt địa hình
ui-slice-view = Khung nhìn cắt lát
ui-strike-dip = hướng dốc { $strike }° · { $dip }
ui-value-dip = góc dốc { $value }°

## Viewport strings

viewport-all-total-categories-keep-their = Tất cả { $total } danh mục giữ nguyên màu của mình; chỉ { $shown } danh mục đầu tiên được vẽ khác biệt
viewport-axis-maximum = { $axis } tối đa
viewport-axis-minimum = { $axis } tối thiểu
viewport-bar-blast-timeline-placeholder = Dòng thời gian nổ mìn [CHỖ GIỮ]
viewport-bar-burden-relief-heatmap-placeholder = Bản đồ nhiệt giải phóng burden [CHỖ GIỮ]
viewport-bar-color = Màu:
viewport-bar-contours-equal-time-placeholder = Đường đồng thời gian [CHỖ GIỮ]
viewport-bar-disable-flying-mode = Tắt chế độ bay
viewport-bar-disable-x-ray-vision = Tắt chế độ nhìn xuyên thấu
viewport-bar-drill-holes = Lỗ khoan:
viewport-bar-enable-flying-mode = Bật chế độ bay
viewport-bar-enable-x-ray-vision = Bật chế độ nhìn xuyên thấu
viewport-bar-exit-slice-view = Thoát khung nhìn cắt lát
viewport-bar-fill = Tô:
viewport-bar-fix-centre-rotation = Cố định tâm xoay
viewport-bar-hide-points = Ẩn điểm
viewport-bar-hide-rl-grid = Ẩn lưới cao độ
viewport-bar-hide-wireframes = Ẩn khung dây
viewport-bar-hide-xy-grid = Ẩn lưới XY
viewport-bar-release-centre-rotation = Giải phóng tâm xoay
viewport-bar-show-points = Hiện điểm
viewport-bar-show-rl-grid = Hiện lưới cao độ
viewport-bar-show-wireframes = Hiện khung dây
viewport-bar-show-xy-grid = Hiện lưới XY
viewport-bar-vertical-slice-view = Khung nhìn cắt lát đứng
viewport-blank = (trống)
viewport-choose-active-block-model-variable = Chọn biến mô hình khối đang hoạt động
viewport-choose-variable = Chọn một biến
viewport-click-edit-color-right-click = Nhấp để sửa màu; nhấp chuột phải để xóa
viewport-click-type-boundary-s-value = Nhấp để nhập giá trị của ranh giới này
viewport-colour-mapping = Ánh xạ màu
viewport-count-categories = { $count } danh mục
viewport-count-category = { $count } danh mục
viewport-double-click-add-boundary-here = Nhấp đúp để thêm ranh giới tại đây
viewport-drag-move-middle-click-toggles = Kéo để di chuyển · Nhấp giữa để chuyển đổi ≤
viewport-drag-move-right-click-remove = Kéo để di chuyển · Nhấp phải để xóa · Nhấp giữa để chuyển đổi ≤
viewport-e = Đ
viewport-edit-category-colour = Sửa màu danh mục này
viewport-edit-colour-used-empty-values = Sửa màu dùng cho giá trị trống
viewport-empty = (trống)
viewport-empty-hidden = (trống · ẩn)
viewport-filter-variables = Lọc biến
viewport-navigation-hint = Kéo chuột giữa để lia · Cuộn để thu phóng
viewport-navigation-hint-detach = Kéo chuột giữa để lia · Cuộn để thu phóng · Nhấp để tách rời
viewport-n = B
viewport-no-data-variable = Không có dữ liệu cho biến này
viewport-no-matches = Không có kết quả trùng khớp
viewport-no-usable-range = (không có khoảng sử dụng được)
viewport-rebuild-variable-s-colours-from = Xây dựng lại màu của biến này từ dữ liệu của nó
viewport-reset = Đặt lại
viewport-restore-full-model-range = Khôi phục toàn bộ khoảng mô hình
