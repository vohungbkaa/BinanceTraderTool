# PROMPT LẬP TRÌNH PHASE 0
Xây dựng **PHASE 0 - Data Pipeline & Preprocessing** cho một ứng dụng desktop giao dịch Binance Futures.

## MỤC TIÊU
Triển khai một **pipeline dữ liệu thị trường realtime đáng tin cậy, dạng mô-đun** có khả năng:
- thu thập dữ liệu thị trường công khai từ Binance Futures
- tách biệt **nến đang chạy** và **nến đã đóng**
- kiểm tra và chuẩn hóa dữ liệu đầu vào
- tính toán chỉ báo kỹ thuật
- lưu dữ liệu gần nhất trong hot cache
- lưu dữ liệu cần thiết vào SQLite
- phát các sự kiện nội bộ theo chuẩn thống nhất
- xử lý reconnect, bù dữ liệu thiếu và giám sát trạng thái hệ thống một cách an toàn

## PHẠM VI TRIỂN KHAI
Cần triển khai:
- bộ lọc symbol
- bộ nạp dữ liệu lịch sử để warm-up
- bộ quản lý luồng WebSocket
- hàng đợi REST để bù dữ liệu
- bộ chuẩn hóa dữ liệu thị trường
- bộ quản lý trạng thái nến
- bộ máy tính chỉ báo
- bộ quản lý hot cache
- cơ chế lưu trữ SQLite
- bộ kiểm tra tính hợp lệ dữ liệu
- bộ giám sát trạng thái hệ thống
- bộ phát sự kiện nội bộ

## NGOÀI PHẠM VI
KHÔNG được triển khai:
- tạo tín hiệu giao dịch
- chấm điểm setup
- thực thi lệnh
- quản lý vị thế
- logic giao dịch tài khoản riêng tư
- quản trị danh mục
- tối ưu chiến lược

## RÀNG BUỘC CỨNG
BẠN BẮT BUỘC phải dùng các công nghệ sau:

- **Desktop shell**: Tauri
- **Giao diện frontend**: Vue 3 + TypeScript
- **Runtime lõi và logic pipeline**: Rust
- **Async runtime**: Tokio
- **Serialization**: Serde
- **Lưu trữ bền vững**: SQLite
- **Thư viện SQLite**: SQLx hoặc rusqlite
- **Logging**: tracing
- **Xử lý lỗi**: anyhow hoặc thiserror
- **Cấu hình**: TOML hoặc YAML

BẠN KHÔNG ĐƯỢC:
- dùng Python để triển khai runtime lõi
- dùng Node.js làm backend runtime chính
- dùng Electron làm desktop shell
- đưa logic pipeline sang frontend
- triển khai toàn bộ logic lõi trong một file đơn khối
- bỏ qua các quy tắc kiểm tra dữ liệu
- phát nến chưa xác nhận như dữ liệu đã xác nhận cho downstream

## QUY TẮC THỊ TRƯỜNG
Chỉ theo dõi các symbol thỏa toàn bộ điều kiện sau:
- Binance **USDT-M Perpetual Futures**
- thời gian niêm yết > 30 ngày
- khối lượng giao dịch 24h >= 5.000.000 USDT
- không nằm trong blacklist
- hỗ trợ whitelist để ghi đè khi cần

Làm mới danh sách symbol hoạt động mỗi 24 giờ.

## KHUNG THỜI GIAN
Chỉ hỗ trợ chính xác các khung:
- `1d`
- `4h`
- `15m`

Sử dụng trực tiếp các kline stream của Binance cho các khung thời gian này.
Không tự tổng hợp khung thời gian thủ công ở phiên bản triển khai đầu tiên.

## QUY TẮC NGUỒN DỮ LIỆU
Sử dụng:
- **WebSocket** làm nguồn realtime chính
- **REST API** chỉ cho các mục đích:
  - warm-up dữ liệu lịch sử
  - bù dữ liệu thiếu
  - khôi phục sau reconnect
  - làm mới metadata

Phải phân biệt rõ:
- **Nến đang chạy**
- **Nến đã đóng**
- **Sự kiện thị trường**

Chỉ **Nến đã đóng** mới là dữ liệu xác nhận hợp lệ cho các module downstream.

## CHỈ BÁO
Tối thiểu phải tính:
- EMA20
- EMA50
- EMA200
- ATR14
- ADX14

Quy tắc:
- tính riêng cho từng symbol và từng timeframe
- chỉ báo trên nến đang chạy chỉ là giá trị tạm thời
- chỉ báo trên nến đã đóng là giá trị xác nhận
- nếu chưa đủ dữ liệu warm-up:
  - `is_warmup = true`
  - `indicator_ready = false`

## QUY TẮC KIỂM TRA DỮ LIỆU
Một cây nến chỉ hợp lệ nếu:
- symbol đang hoạt động
- timeframe được hỗ trợ
- timestamp hợp lệ
- `high >= max(open, close, low)`
- `low <= min(open, close, high)`
- `volume >= 0`

Chính sách trùng lặp:
- xác định bản ghi trùng bằng `symbol + timeframe + open_time + event_type`

Chính sách dữ liệu thiếu:
- phát hiện khoảng thời gian bị thiếu
- đánh dấu phạm vi bị ảnh hưởng là `gap-pending`
- tạm dừng phát dữ liệu xác nhận xuống downstream cho phạm vi đó
- đưa tác vụ bù dữ liệu vào hàng đợi REST
- chỉ tiếp tục sau khi đối soát dữ liệu thành công

Đồng bộ thời gian:
- đồng bộ với thời gian máy chủ Binance mỗi 1 giờ

## QUY TẮC LƯU TRỮ
Cần triển khai:
- **Hot Cache**: lưu 200 đến 500 nến gần nhất cho mỗi symbol và mỗi timeframe
- **Persistent Store**: dùng SQLite để lưu:
  - nến đã đóng
  - snapshot chỉ báo
  - sự kiện trạng thái hệ thống
  - sự kiện thiếu dữ liệu
  - sự kiện reconnect
  - metadata của danh sách symbol

Kho lưu trữ dài hạn:
- chỉ cần định nghĩa interface, không cần triển khai đầy đủ

## QUY TẮC SỰ KIỆN
Cần triển khai các sự kiện nội bộ sau:
- `market.candle.updated`
- `market.candle.closed`
- `market.indicator.updated`
- `market.depth.updated`
- `market.funding.updated`
- `system.health.changed`
- `system.data_gap.detected`
- `system.data_gap.resolved`

Quy tắc:
- `market.candle.closed` là trigger downstream chính
- tất cả sự kiện phải có timestamp
- sự kiện thị trường phải có symbol và timeframe khi phù hợp
- payload sự kiện phải có tính xác định, không mơ hồ

## QUY TẮC GIÁM SÁT TRẠNG THÁI
Các trạng thái hệ thống:
- `Healthy`: độ trễ < 1000 ms và không có gap chưa xử lý
- `Degraded`: độ trễ từ 1000 đến 5000 ms hoặc reconnect lặp lại
- `Critical`: độ trễ > 5000 ms, có gap nghiêm trọng chưa xử lý, hoặc stream không khả dụng

Hành động:
- nếu trạng thái là `Critical`, dừng phát dữ liệu xác nhận xuống downstream cho phạm vi bị ảnh hưởng
- phát sự kiện `system.health.changed`
- lưu lại sự chuyển trạng thái hệ thống

## QUY TẮC AN TOÀN
An toàn REST:
- mọi request REST phải đi qua một hàng đợi tập trung
- hỗ trợ retry và exponential backoff
- luôn giữ biên độ an toàn dưới giới hạn rate limit của sàn

An toàn reconnect:
- khi WebSocket bị ngắt:
  - đánh dấu stream là không khỏe
  - chờ 5 giây
  - thử kết nối lại với backoff
  - sau khi reconnect, phát hiện và bù dữ liệu thiếu trước khi tiếp tục phát dữ liệu xác nhận

An toàn khi tắt ứng dụng:
- flush toàn bộ ghi dữ liệu đang chờ
- lưu trạng thái thiết yếu
- đóng stream an toàn

## PHONG CÁCH TRIỂN KHAI
Mã nguồn phải:
- dạng mô-đun
- có kiểu dữ liệu rõ ràng
- có thể kiểm thử
- an toàn khi khởi động lại
- có khả năng quan sát
- điều khiển bằng cấu hình
- độ phụ thuộc thấp

Cần tránh:
- trạng thái toàn cục có thể thay đổi nhưng bị ẩn
- lỗi im lặng không log
- hằng số hardcode không cấu hình
- logic nghiệp vụ nằm trong UI component

## ĐẦU RA BẮT BUỘC
Trả kết quả theo đúng thứ tự sau:
1. tóm tắt kiến trúc
2. cây thư mục module
3. các data model lõi bằng Rust
4. hợp đồng sự kiện
5. mã triển khai
6. schema SQLite
7. ví dụ file cấu hình
8. kế hoạch kiểm thử
9. các giả định đã sử dụng

## TIÊU CHÍ CHẤP NHẬN
Việc triển khai chỉ được coi là đạt nếu:
- bộ lọc symbol hoạt động đúng
- cơ chế warm-up dữ liệu lịch sử hoạt động đúng
- luồng WebSocket hoạt động đúng
- nến đang chạy và nến đã đóng được tách biệt
- chỉ báo được tính đúng
- dữ liệu không hợp lệ bị loại bỏ
- dữ liệu thiếu kích hoạt đúng quy trình phục hồi
- thay đổi trạng thái hệ thống được theo dõi
- sự kiện được phát nhất quán
- lưu trữ SQLite hoạt động đúng
- codebase có tính mô-đun và có thể kiểm thử

## QUY TẮC PHẢN HỒI
Chỉ trả về các đầu ra cụ thể.
Không giải thích triết lý giao dịch.
Không mở rộng ra ngoài PHASE 0.
