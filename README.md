# tyange-cms-api

Rust + Poem + SQLite로 만든 개인용 CMS API 서버입니다.
블로그 콘텐츠를 관리하고, 주간 예산/지출을 추적하기 위해 만든 백엔드이며 현재는 단일 운영자(개인) 사용 시나리오를 중심으로 구성되어 있습니다.

## 이 프로젝트의 목적

이 API는 크게 두 가지를 해결하기 위해 만들어졌습니다.

- 블로그 운영: 포스트/태그/이미지/포트폴리오 데이터를 CMS 형태로 관리
- 소비 추적: 주차 단위 예산 설정, 지출 기록, 주간 요약 및 예산 계획 계산

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

- `GET /budget/weekly-config` (JWT)
현재 주차 예산 설정 조회(없으면 기본값 생성).

- `POST /budget/set` (JWT)
현재 주차 예산/알림 임계값 저장(업서트).

- `PUT /budget/update/:config_id` (JWT)
특정 예산 설정(config_id) 수정.

- `POST /budget/plan` (JWT)
기간 총예산을 주차별로 분배 계산 후 저장.

- `POST /budget/card-excel/remaining-weekly-budget` (JWT)
카드 엑셀 업로드 기반 순지출/잔여예산/주간 버킷 계산.

- `GET /budget/spending`
주차별 소비 기록 목록 조회(`week=YYYY-Www`, 미지정 시 현재 주차).

- `POST /budget/spending` (JWT or API Key)
소비 기록 생성 및 주간 누적/남은 예산 계산.
응답의 `remaining`은 저장된 `projected_remaining - weekly_total` 기준이며, 소비 생성 시 `projected_remaining` 자체를 다시 쓰지는 않습니다.

- `PUT /budget/spending/:record_id`
소비 기록 수정.

- `DELETE /budget/spending/:record_id`
소비 기록 삭제.

- `GET /budget/weekly`
현재 주차 예산 요약(총지출/잔여/사용률/알림) 조회.
여기서 `remaining`은 `weekly_limit`이 아니라 저장된 `projected_remaining - total_spent` 의미입니다. `usage_rate`와 `alert`는 기존처럼 `weekly_limit` 기준을 유지합니다.

- `GET /budget/weeks`
예산이 등록된 주차 목록 조회(`weeks`, `min_week`, `max_week`).

- `GET /budget/weekly/:week_key`
특정 주차(`YYYY-Www`) 예산 요약 조회.

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
