# tyange-cms-api (CMS 백엔드)

Rust + Poem 프레임워크. cargo 사용.

## 기술 스택
- Rust
- Poem (HTTP 프레임워크)
- SQLite + sqlx

## 연결 프로젝트
- 프론트엔드: `tyange-cms` (Nuxt 3 + Vue)
- API 변경 시 반드시 tyange-cms와 동기화 필요

## 현재 상태
- 운영 중
- 포트폴리오 DB를 단일 JSON(content TEXT) → 섹션 테이블(portfolio_section)로 분리 (2026-04-13)
  - portfolio (마스터): portfolio_id, slug, created_at
  - portfolio_section: section_id, portfolio_id, section_key, content(JSON), created_at, updated_at
  - 사용 중인 섹션: meta, identity, featured_projects, career
  - 섹션별 개별 업데이트 엔드포인트: PUT /portfolio/sections/:section_key
  - 기존 API 응답 형태(PortfolioResponse)는 하위 호환 유지
