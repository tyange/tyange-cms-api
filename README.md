# tyange-cms-api

Rust + Poem + SQLite로 만든 개인용 CMS API 서버입니다.
블로그 콘텐츠를 관리하고, 기간 총예산/지출을 추적하기 위해 만든 백엔드이며 현재는 단일 운영자(개인) 사용 시나리오를 중심으로 구성되어 있습니다.

## 이 프로젝트의 목적

이 API는 크게 두 가지를 해결하기 위해 만들어졌습니다.

- 블로그 운영: 포스트/태그/이미지/포트폴리오 데이터를 CMS 형태로 관리
- 소비 추적: 기간 총예산 설정, 지출 기록, 예산 요약 및 기간별 계산

즉, "공용 SaaS형 CMS"보다는 "개인 콘텐츠 + 개인 소비 관리"에 최적화된 구조입니다.

## Tech Stack

- Rust (Edition 2021)
- Poem
- SQLx (SQLite)
- Tokio
- JWT (`jsonwebtoken`)
- bcrypt

## 빠른 시작

### 1) 환경변수 설정

프로젝트 루트 `.env`에 아래 값을 설정하세요.

```env
# 상대 경로 예시
DATABASE_PATH=./data/database.db
UPLOAD_PATH=.uploads/images

# JWT
JWT_ACCESS_SECRET=replace-with-access-secret
JWT_REFRESH_SECRET=replace-with-refresh-secret
```

`DATABASE_PATH`, `UPLOAD_PATH`는 절대 경로로 직접 지정해도 됩니다.

```env
# 절대 경로 예시
DATABASE_PATH=/Users/yourname/data/tyange/database.db
UPLOAD_PATH=/Users/yourname/data/tyange/uploads/images
```

### 2) 실행

```bash
cargo run -q
```

기본 바인드 주소는 `0.0.0.0:8080` 입니다.

## 인증

### JWT 인증 (`Authorization`)

`Authorization` 헤더에 JWT 액세스 토큰을 넣어 호출합니다.
관리자/작성자 권한이 필요한 CMS 수정 계열 API와 일부 Budget 설정 API에서 사용됩니다.

### API Key 인증 (`X-API-Key`)

API Key는 유저별로 여러 개 발급할 수 있고, 원문은 발급 시 1회만 반환됩니다.
DB에는 bcrypt 해시만 저장하며 `name`, `created_at`, `last_used_at`, `revoked_at`를 관리합니다.

인증 규칙은 다음과 같습니다.

- 대부분의 관리/조회 API: JWT(`Authorization`) 사용
- `POST /budget/spending`: JWT 또는 `X-API-Key` 둘 다 허용
- API Key 관리 API(`POST /api-keys`, `GET /api-keys`, `DELETE /api-keys/:id`): JWT 필요

`POST /budget/spending`은 request body의 `user_id`를 신뢰하지 않고, 항상 인증 컨텍스트의 `user_id`를 사용합니다.

## API 개요

### Health

- `GET /health`
서버 생존 확인용 헬스체크.

- `GET /health-check`
서버 생존 확인용 대체 헬스체크.

- `OPTIONS /*path`
CORS preflight 처리.

### Auth

- `POST /login`
사용자 로그인 후 access/refresh 토큰 발급.

- `POST /admin/add-user` (JWT)
신규 사용자 계정 추가(비밀번호 해시 저장).

- `POST /api-keys` (JWT)
현재 로그인한 유저용 API Key 발급. 원문 API key는 이 응답에서만 반환.

- `GET /api-keys` (JWT)
현재 로그인한 유저가 발급한 API key 목록 조회.

- `DELETE /api-keys/:id` (JWT)
현재 로그인한 유저의 API key 폐기(revoke).

### Posts / Tags (CMS)

- `GET /posts`
공개용 포스트 목록 조회(초안 제외), 작성자 필터 지원.

- `GET /posts/search-with-tags`
포함/제외 태그 조건으로 포스트 검색.

- `GET /post/:post_id`
단일 포스트 상세 조회.

- `POST /post/upload` (JWT)
새 포스트 작성 및 태그 연결.

- `PUT /post/update/:post_id` (JWT)
본인 포스트 내용/태그 수정.

- `DELETE /post/delete/:post_id` (JWT)
본인 포스트 삭제.

- `GET /admin/posts` (JWT)
관리자용 전체 포스트 목록 조회(초안 포함).

- `GET /tags`
태그별 사용 횟수 조회(카테고리 필터 가능).

- `GET /tags-with-category`
카테고리별 태그 묶음 조회.

### Images / Portfolio

- `POST /upload-image` (JWT)
이미지 업로드 후 웹 경로(`/images/...`) 반환.

- `GET /portfolio`
포트폴리오 콘텐츠 조회.

- `PUT /portfolio/update` (JWT)
포트폴리오 콘텐츠 수정.

### Budget

- `GET /budget` (JWT)
현재 활성 기간 예산 요약 조회.
응답 필드:
`budget_id`, `total_budget`, `from_date`, `to_date`, `total_spent`, `remaining_budget`, `usage_rate`, `alert`, `alert_threshold`, `is_overspent`

- `PUT /budget` (JWT)
현재 활성 기간 예산의 총액을 다시 설정한다. `alert_threshold`와 현재까지의 누적 지출 스냅샷(`total_spent`)도 함께 수정할 수 있다.
기간 필드(`from_date`, `to_date`)는 수정할 수 없다.

- `POST /budget/plan` (JWT)
기간 총예산을 생성한다. 현재까지의 누적 지출 스냅샷(`total_spent`)도 함께 저장할 수 있다.

- `POST /budget/card-excel/remaining-weekly-budget` (JWT)
카드 엑셀 업로드 기반 순지출/잔여예산/주간 버킷 계산.

- `GET /budget/spending`
현재 활성 예산 기간의 소비 기록을 조회하고, 응답에서만 ISO week 기준으로 그룹핑한다.

- `POST /budget/spending` (JWT or API Key)
소비 기록 생성 및 기간 누적/남은 예산 계산.
`transacted_at`는 현재 활성 예산 기간 안에 있어야 합니다.

- `POST /budget/spending/import-preview` (JWT)
신한카드 XLS 파일을 업로드해 미리보기 결과를 반환한다. 응답에는 `summary`, `rows`가 포함되며 각 row는 `fingerprint`, `transacted_at`, `amount`, `merchant`, `status`, `reason`을 가진다.

- `POST /budget/spending/import-commit` (JWT)
신한카드 XLS 파일과 `selected_fingerprints`를 함께 보내 선택한 거래만 반영한다. 이미 반영된 imported row는 중복으로 건너뛴다.

- `PUT /budget/spending/:record_id`
소비 기록 수정.

- `DELETE /budget/spending/:record_id`
소비 기록 삭제.

#### Budget request 예시

`POST /budget/plan`

```json
{
  "total_budget": 1500,
  "from_date": "2026-04-01",
  "to_date": "2026-04-30",
  "total_spent": 400,
  "alert_threshold": 0.9
}
```

`PUT /budget`

```json
{
  "total_budget": 1800,
  "total_spent": 400,
  "alert_threshold": 0.9
}
```

`POST /budget/plan`, `PUT /budget`는 `total_spent`를 권장 필드명으로 사용한다.
하위 호환을 위해 `spent_so_far`도 같은 의미의 alias로 허용한다.

#### Budget 계산 규칙

- `remaining_budget = total_budget - total_spent`
- `is_overspent = total_spent > total_budget`
- `usage_rate = total_spent / total_budget` (`total_budget > 0`)
- `alert = usage_rate >= alert_threshold`

#### Budget total_spent 정책

- `total_spent`를 요청 바디에 넣으면, 이 값은 `budget_periods.snapshot_total_spent`에 저장되는 요약 스냅샷으로 취급한다.
- 스냅샷이 저장된 예산은 `GET /budget`, `POST /budget/plan`, `PUT /budget` 응답에서 소비 기록 합계 대신 이 값을 사용한다.
- `total_spent`를 생략하면 기존처럼 해당 기간의 `spending_records` 합계를 사용한다.
- 따라서 스냅샷 값과 소비 기록 합계가 달라도 에러로 막지 않는다. 대시보드 수동 보정값을 우선해야 하는 요구사항에는 이 방식이 권장안이다.

#### Spending import 정책

- import 대상은 신한카드 XLS 거래내역이며 서버는 stateless하게 `preview -> commit` 2단계로 처리한다.
- imported row는 `source_type='shinhancard_xls'`, `source_fingerprint`를 저장해 동일 거래 재업로드를 중복으로 막는다.
- import는 `spending_records`만 갱신하고 `budget_periods.snapshot_total_spent`는 수정하지 않는다.
- 따라서 스냅샷 예산을 사용하는 경우 `GET /budget.total_spent`와 `GET /budget/spending.total_spent`가 import 후에도 다를 수 있다.

## 테스트

```bash
cargo test
```

현재 테스트는 JWT 생성/검증 기본 동작 중심입니다.

## curl 예시

### 로그인 후 JWT 획득

```bash
curl -sS -X POST http://127.0.0.1:8080/login \
  -H 'Content-Type: application/json' \
  -d '{"user_id":"me@example.com","password":"secret"}'
```

### API Key 발급

```bash
curl -sS -X POST http://127.0.0.1:8080/api-keys \
  -H "Authorization: $ACCESS_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"macrodroid-main-phone"}'
```

응답의 `api_key` 값은 이때만 확인할 수 있습니다.

### API Key 목록 조회

```bash
curl -sS http://127.0.0.1:8080/api-keys \
  -H "Authorization: $ACCESS_TOKEN"
```

### API Key 폐기

```bash
curl -sS -X DELETE http://127.0.0.1:8080/api-keys/1 \
  -H "Authorization: $ACCESS_TOKEN"
```

### JWT로 소비 기록 생성

```bash
curl -sS -X POST http://127.0.0.1:8080/budget/spending \
  -H "Authorization: $ACCESS_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"amount":12000,"merchant":"CU 역삼신웅점","transacted_at":"2026-03-03T12:20:00"}'
```

### API Key로 소비 기록 생성

```bash
curl -sS -X POST http://127.0.0.1:8080/budget/spending \
  -H "X-API-Key: $MACRODROID_USER_API_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"amount":12000,"merchant":"CU 역삼신웅점","transacted_at":"2026-03-03T12:20:00"}'
```
