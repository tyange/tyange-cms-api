use poem::{Result, error::InternalServerError};
use sqlx::{Row, SqlitePool, query, query_scalar};

use crate::models::{
    PortfolioAbout, PortfolioDocument, PortfolioHero, PortfolioHighlightCard, PortfolioIdentity,
    PortfolioLink, PortfolioMetric, PortfolioProject, PortfolioWritingSection,
};

pub async fn init_db(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
          CREATE TABLE IF NOT EXISTS posts (
              post_id TEXT PRIMARY KEY,
              title TEXT,
              description TEXT,
              published_at DATETIME,
              tags TEXT,
              content TEXT,
              writer_id TEXT NOT NULL,
              status TEXT NOT NULL,
              created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
          )
          "#,
    )
    .execute(pool)
    .await
    .map_err(InternalServerError)?;

    sqlx::query(
        r#"
          CREATE TABLE IF NOT EXISTS images (
              image_id TEXT PRIMARY KEY,
              post_id TEXT,
              file_name TEXT NOT NULL,
              origin_name TEXT NOT NULL,
              file_path TEXT NOT NULL,
              mime_type TEXT NOT NULL,
              image_type TEXT NOT NULL,
              uploaded_at TEXT DEFAULT CURRENT_TIMESTAMP,
              FOREIGN KEY (post_id) REFERENCES posts(post_id)
          )
          "#,
    )
    .execute(pool)
    .await
    .map_err(InternalServerError)?;

    sqlx::query(
        r#"
          CREATE TABLE IF NOT EXISTS users (
              user_id TEXT PRIMARY KEY,
              password TEXT,
              user_role TEXT NOT NULL,
              auth_provider TEXT NOT NULL DEFAULT 'local',
              google_sub TEXT
          )
          "#,
    )
    .execute(pool)
    .await
    .map_err(InternalServerError)?;

    sqlx::query(
        r#"
          CREATE TABLE IF NOT EXISTS sections (
              section_id INTEGER PRIMARY KEY,
              section_type TEXT NOT NULL,
              content_data TEXT NOT NULL,
              order_index INTEGER NOT NULL,
              is_active BOOLEAN DEFAULT true,
              created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
              updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
          )
          "#,
    )
    .execute(pool)
    .await
    .map_err(InternalServerError)?;

    sqlx::query(
        r#"
          CREATE TABLE IF NOT EXISTS portfolio (
              portfolio_id INTEGER PRIMARY KEY,
              content TEXT NOT NULL,
              updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
          )
          "#,
    )
    .execute(pool)
    .await
    .map_err(InternalServerError)?;

    if !column_exists(pool, "portfolio", "slug")
        .await
        .map_err(InternalServerError)?
    {
        query("ALTER TABLE portfolio ADD COLUMN slug TEXT NOT NULL DEFAULT 'dev'")
            .execute(pool)
            .await
            .map_err(InternalServerError)?;
    }

    if !column_exists(pool, "portfolio", "created_at")
        .await
        .map_err(InternalServerError)?
    {
        query("ALTER TABLE portfolio ADD COLUMN created_at DATETIME DEFAULT CURRENT_TIMESTAMP")
            .execute(pool)
            .await
            .map_err(InternalServerError)?;
    }

    query("UPDATE portfolio SET slug = 'dev' WHERE slug IS NULL OR TRIM(slug) = ''")
        .execute(pool)
        .await
        .map_err(InternalServerError)?;

    query(
        "UPDATE portfolio SET created_at = updated_at WHERE created_at IS NULL OR TRIM(created_at) = ''",
    )
    .execute(pool)
    .await
    .map_err(InternalServerError)?;

    ensure_default_portfolio(pool)
        .await
        .map_err(InternalServerError)?;

    migrate_budget_periods(pool)
        .await
        .map_err(InternalServerError)?;
    migrate_spending_records(pool)
        .await
        .map_err(InternalServerError)?;
    migrate_users(pool).await.map_err(InternalServerError)?;
    migrate_user_matches(pool)
        .await
        .map_err(InternalServerError)?;
    migrate_match_messages(pool)
        .await
        .map_err(InternalServerError)?;
    migrate_api_keys(pool).await.map_err(InternalServerError)?;
    migrate_rss_push(pool).await.map_err(InternalServerError)?;

    Ok(())
}

async fn ensure_default_portfolio(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    let existing: Option<i64> =
        query_scalar("SELECT portfolio_id FROM portfolio WHERE slug = ? LIMIT 1")
            .bind("dev")
            .fetch_optional(pool)
            .await?;

    if existing.is_some() {
        return Ok(());
    }

    query(
        r#"
        INSERT INTO portfolio (slug, content, created_at, updated_at)
        VALUES (?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
    )
    .bind("dev")
    .bind(serde_json::to_string(&default_portfolio_document()).unwrap_or_default())
    .execute(pool)
    .await?;

    Ok(())
}

fn default_portfolio_document() -> PortfolioDocument {
    PortfolioDocument {
        slug: String::from("dev"),
        version: 1,
        identity: PortfolioIdentity {
            name: String::from("TYANGE"),
            role: String::from("프론트엔드 개발자"),
            location: String::from("서울, 대한민국"),
            availability: String::from("브랜딩과 제품 완성도가 중요한 작업을 선별해 진행합니다"),
            email: String::from("usun16@gmail.com"),
            github_url: String::from("https://github.com/tyange"),
            blog_url: String::from("https://blog.tyange.com"),
            velog_url: Some(String::from("https://velog.io/@tyange")),
        },
        hero: PortfolioHero {
            eyebrow: String::from("프론트엔드 개발자 / CMS 중심 사이드 프로젝트 / 서울"),
            headline: String::from("보이는 것 너머의 구조를 오래 다듬습니다."),
            summary: String::from(
                "블로그, CMS, API, 대시보드를 하나의 운영 흐름으로 연결하는 프론트엔드 작업을 합니다.",
            ),
            primary_cta: PortfolioLink {
                label: String::from("GitHub 보기"),
                url: String::from("https://github.com/tyange"),
            },
            secondary_cta: PortfolioLink {
                label: String::from("블로그 보기"),
                url: String::from("https://blog.tyange.com"),
            },
        },
        highlight_cards: vec![
            PortfolioHighlightCard {
                label: String::from("집중 영역"),
                title: String::from("콘텐츠 구조와 운영 흐름까지 포함한 프론트엔드 시스템 설계"),
            },
            PortfolioHighlightCard {
                label: String::from("기술 스택"),
                title: String::from("Next.js, Nuxt, Solid, Rust, Poem, Tailwind CSS, SQLite"),
            },
        ],
        metrics: Some(vec![
            PortfolioMetric {
                value: String::from("2"),
                unit: String::from("개사"),
                description: String::from("프론트엔드 개발자로 재직한 이력"),
            },
            PortfolioMetric {
                value: String::from("3+"),
                unit: String::from("년"),
                description: String::from("서비스 UI와 내부 도구를 설계하고 구현한 시간"),
            },
        ]),
        guiding_principle: String::from(
            "미니멀은 비워 두는 일이 아니라, 느슨한 부분을 끝까지 다듬고 난 뒤에 남는 결과라고 생각합니다.",
        ),
        featured_projects: vec![
            PortfolioProject {
                slug: String::from("tyange-blog"),
                title: String::from("tyange-blog"),
                period: String::from("Nuxt 4 / 콘텐츠 플랫폼"),
                summary: String::from(
                    "마크다운 발행, RSS, 태그 필터링, CMS 연동 재배포까지 이어지는 개인 블로그입니다.",
                ),
                stack: vec![
                    String::from("Nuxt 4"),
                    String::from("TypeScript"),
                    String::from("Tailwind CSS 4"),
                    String::from("Nuxt MDC"),
                    String::from("Pinia"),
                ],
                highlights: vec![
                    String::from(
                        "마크다운 포스트, 코드 블록, 태그 필터, RSS 피드를 직접 구성했습니다.",
                    ),
                    String::from(
                        "CMS 변경 후 블로그 재배포와 RSS 갱신이 이어지도록 운영 흐름을 연결했습니다.",
                    ),
                ],
                links: vec![
                    PortfolioLink {
                        label: String::from("저장소"),
                        url: String::from("https://github.com/tyange/tyange-blog"),
                    },
                    PortfolioLink {
                        label: String::from("서비스"),
                        url: String::from("https://blog.tyange.com"),
                    },
                ],
            },
            PortfolioProject {
                slug: String::from("tyange-cms-api"),
                title: String::from("tyange-cms-api"),
                period: String::from("Rust / Poem / 콘텐츠 인프라"),
                summary: String::from(
                    "포스트, 인증, 업로드, 알림, 예산, 포트폴리오 문서를 다루는 Rust API입니다.",
                ),
                stack: vec![
                    String::from("Rust"),
                    String::from("Poem"),
                    String::from("SQLx"),
                    String::from("SQLite"),
                    String::from("JWT"),
                ],
                highlights: vec![
                    String::from(
                        "JWT 인증, 이미지 업로드, 포스트 CRUD, 포트폴리오 CRUD를 직접 구현했습니다.",
                    ),
                    String::from(
                        "RSS poller, Web Push, 예산 API, API Key 기반 호출 흐름을 함께 운영합니다.",
                    ),
                ],
                links: vec![PortfolioLink {
                    label: String::from("저장소"),
                    url: String::from("https://github.com/tyange/tyange-cms-api"),
                }],
            },
            PortfolioProject {
                slug: String::from("tyange-cms"),
                title: String::from("tyange-cms"),
                period: String::from("Nuxt 4 / 내부 CMS"),
                summary: String::from(
                    "블로그 운영과 개인 관리 작업을 한 화면에서 처리하는 전용 CMS입니다.",
                ),
                stack: vec![
                    String::from("Nuxt 4"),
                    String::from("Vue 3"),
                    String::from("TypeScript"),
                    String::from("Tailwind CSS 4"),
                ],
                highlights: vec![
                    String::from(
                        "Google 로그인, 관리자 로그인, 포스트 CRUD, 이미지 업로드를 구현했습니다.",
                    ),
                    String::from(
                        "태그 조회, 예산 관리, 카드 사용내역 엑셀 업로드 흐름을 연결했습니다.",
                    ),
                ],
                links: vec![PortfolioLink {
                    label: String::from("저장소"),
                    url: String::from("https://github.com/tyange/tyange-cms"),
                }],
            },
            PortfolioProject {
                slug: String::from("tyange-dashboard"),
                title: String::from("tyange-dashboard"),
                period: String::from("Solid / 운영 대시보드"),
                summary: String::from(
                    "PWA와 Web Push를 실험하기 위해 만든 Solid 기반 대시보드로, 실제로 브라우저에서 Web Push를 수신할 수 있도록 구성했습니다.",
                ),
                stack: vec![
                    String::from("SolidJS"),
                    String::from("TypeScript"),
                    String::from("Vite"),
                    String::from("CMS API"),
                ],
                highlights: vec![
                    String::from(
                        "PWA, Service Worker, Web Push 구독/해제, RSS 구독 관리 화면을 구현했습니다.",
                    ),
                    String::from(
                        "Google 로그인, 예산 대시보드, 소비 기록, API Key 관리 화면을 연결했습니다.",
                    ),
                ],
                links: vec![PortfolioLink {
                    label: String::from("저장소"),
                    url: String::from("https://github.com/tyange/tyange-dashboard"),
                }],
            },
        ],
        about: PortfolioAbout {
            eyebrow: String::from("소개"),
            headline: String::from("화면의 완성도와 그 뒤의 구조가 함께 좋아지는 일을 선호합니다."),
            paragraphs: vec![
                String::from(
                    "단일 페이지보다 연결된 제품 흐름을 선호합니다. 블로그, CMS, API, 대시보드가 하나의 운영 경험처럼 이어져야 한다고 생각합니다.",
                ),
                String::from(
                    "화면 구현뿐 아니라 데이터 계약, 배포 흐름, 실제 운영 동선까지 같이 설계하는 편입니다.",
                ),
            ],
            services: vec![
                String::from("제품 화면과 콘텐츠 화면을 위한 프론트엔드 아키텍처 설계"),
                String::from("디자인 시스템을 고려한 UI 구현"),
                String::from("내부 툴 및 CMS 운영 화면 제작"),
                String::from("API 계약을 중심으로 한 프론트엔드 협업"),
            ],
            strengths: vec![
                String::from("거친 아이디어를 구조적인 화면 체계로 정리하는 일"),
                String::from("프론트엔드 완성도를 백엔드 현실과 연결하는 일"),
                String::from("개인 프로젝트를 처음부터 끝까지 밀도 있게 완성하는 일"),
            ],
        },
        writing: PortfolioWritingSection {
            eyebrow: String::from("기록"),
            title: String::from("dev 태그가 붙은 글"),
            description: String::from(
                "`/posts/search-with-tags?include=dev` 응답을 그대로 표시합니다.",
            ),
        },
        currently_building: None,
    }
}

async fn migrate_budget_periods(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    if !table_exists(pool, "budget_periods").await? {
        create_budget_periods_table(pool).await?;
        return Ok(());
    }

    if !column_exists(pool, "budget_periods", "snapshot_total_spent").await? {
        ensure_budget_period_indexes(pool).await?;
        return Ok(());
    }

    if table_exists(pool, "budget_periods_new").await? {
        query("DROP TABLE budget_periods_new").execute(pool).await?;
    }

    if column_exists(pool, "budget_periods", "snapshot_total_spent").await? {
        query(
            r#"
            CREATE TABLE budget_periods_new (
                budget_id INTEGER PRIMARY KEY AUTOINCREMENT,
                owner_user_id TEXT NOT NULL,
                total_budget INTEGER NOT NULL,
                from_date DATE NOT NULL,
                to_date DATE NOT NULL,
                alert_threshold REAL NOT NULL DEFAULT 0.85,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(pool)
        .await?;
        query(
            r#"
            INSERT INTO budget_periods_new (
                budget_id,
                owner_user_id,
                total_budget,
                from_date,
                to_date,
                alert_threshold,
                created_at,
                updated_at
            )
            SELECT
                budget_id,
                owner_user_id,
                total_budget,
                from_date,
                to_date,
                alert_threshold,
                created_at,
                updated_at
            FROM budget_periods
            "#,
        )
        .execute(pool)
        .await?;
        query("DROP TABLE budget_periods").execute(pool).await?;
        query("ALTER TABLE budget_periods_new RENAME TO budget_periods")
            .execute(pool)
            .await?;
    }

    ensure_budget_period_indexes(pool).await?;

    Ok(())
}

async fn migrate_spending_records(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    if !table_exists(pool, "spending_records").await? {
        create_spending_records_table(pool).await?;
        return Ok(());
    }

    let has_owner_user_id = column_exists(pool, "spending_records", "owner_user_id").await?;
    let has_source_type = column_exists(pool, "spending_records", "source_type").await?;
    let has_source_fingerprint =
        column_exists(pool, "spending_records", "source_fingerprint").await?;
    let has_week_key = column_exists(pool, "spending_records", "week_key").await?;

    if has_owner_user_id && has_source_type && has_source_fingerprint && !has_week_key {
        ensure_spending_record_indexes(pool).await?;
        return Ok(());
    }

    if table_exists(pool, "spending_records_new").await? {
        query("DROP TABLE spending_records_new")
            .execute(pool)
            .await?;
    }
    query(
        r#"
        CREATE TABLE spending_records_new (
            record_id INTEGER PRIMARY KEY AUTOINCREMENT,
            owner_user_id TEXT NOT NULL,
            amount INTEGER NOT NULL,
            merchant TEXT,
            transacted_at DATETIME NOT NULL,
            source_type TEXT,
            source_fingerprint TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    if has_owner_user_id {
        let copy_sql = match (has_source_type, has_source_fingerprint) {
            (true, true) => {
                r#"
                INSERT INTO spending_records_new
                    (record_id, owner_user_id, amount, merchant, transacted_at, source_type, source_fingerprint, created_at)
                SELECT
                    record_id,
                    COALESCE(owner_user_id, 'admin'),
                    amount,
                    merchant,
                    transacted_at,
                    source_type,
                    source_fingerprint,
                    created_at
                FROM spending_records
                "#
            }
            _ => {
                r#"
                INSERT INTO spending_records_new
                    (record_id, owner_user_id, amount, merchant, transacted_at, source_type, source_fingerprint, created_at)
                SELECT
                    record_id,
                    COALESCE(owner_user_id, 'admin'),
                    amount,
                    merchant,
                    transacted_at,
                    NULL,
                    NULL,
                    created_at
                FROM spending_records
                "#
            }
        };
        query(copy_sql).execute(pool).await?;
    } else {
        query(
            r#"
            INSERT INTO spending_records_new
                (record_id, owner_user_id, amount, merchant, transacted_at, source_type, source_fingerprint, created_at)
            SELECT
                record_id,
                'admin',
                amount,
                merchant,
                transacted_at,
                NULL,
                NULL,
                created_at
            FROM spending_records
            "#,
        )
        .execute(pool)
        .await?;
    }

    query("DROP TABLE spending_records").execute(pool).await?;
    query("ALTER TABLE spending_records_new RENAME TO spending_records")
        .execute(pool)
        .await?;
    ensure_spending_record_indexes(pool).await?;

    Ok(())
}

async fn create_budget_periods_table(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS budget_periods (
            budget_id INTEGER PRIMARY KEY AUTOINCREMENT,
            owner_user_id TEXT NOT NULL,
            total_budget INTEGER NOT NULL,
            from_date DATE NOT NULL,
            to_date DATE NOT NULL,
            alert_threshold REAL NOT NULL DEFAULT 0.85,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    ensure_budget_period_indexes(pool).await?;

    Ok(())
}

async fn create_spending_records_table(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS spending_records (
            record_id INTEGER PRIMARY KEY AUTOINCREMENT,
            owner_user_id TEXT NOT NULL,
            amount INTEGER NOT NULL,
            merchant TEXT,
            transacted_at DATETIME NOT NULL,
            source_type TEXT,
            source_fingerprint TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    ensure_spending_record_indexes(pool).await?;

    Ok(())
}

async fn ensure_budget_period_indexes(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_budget_periods_owner_updated_at
        ON budget_periods(owner_user_id, updated_at DESC, budget_id DESC)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn ensure_spending_record_indexes(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_spending_records_owner_transacted_at
        ON spending_records(owner_user_id, transacted_at)
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_spending_records_import_fingerprint
        ON spending_records(owner_user_id, source_type, source_fingerprint)
        WHERE source_type IS NOT NULL AND source_fingerprint IS NOT NULL
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn migrate_users(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    if !table_exists(pool, "users").await? {
        create_users_table(pool).await?;
        return Ok(());
    }

    let has_auth_provider = column_exists(pool, "users", "auth_provider").await?;
    let has_google_sub = column_exists(pool, "users", "google_sub").await?;
    let has_display_name = column_exists(pool, "users", "display_name").await?;
    let has_avatar_url = column_exists(pool, "users", "avatar_url").await?;
    let has_bio = column_exists(pool, "users", "bio").await?;

    if has_auth_provider && has_google_sub {
        if !has_display_name {
            query("ALTER TABLE users ADD COLUMN display_name TEXT")
                .execute(pool)
                .await?;
        }
        if !has_avatar_url {
            query("ALTER TABLE users ADD COLUMN avatar_url TEXT")
                .execute(pool)
                .await?;
        }
        if !has_bio {
            query("ALTER TABLE users ADD COLUMN bio TEXT")
                .execute(pool)
                .await?;
        }
        ensure_user_indexes(pool).await?;
        return Ok(());
    }

    query("DROP TABLE IF EXISTS users_new")
        .execute(pool)
        .await?;
    query(
        r#"
        CREATE TABLE users_new (
            user_id TEXT PRIMARY KEY,
            password TEXT,
            user_role TEXT NOT NULL,
            auth_provider TEXT NOT NULL DEFAULT 'local',
            google_sub TEXT,
            display_name TEXT,
            avatar_url TEXT,
            bio TEXT
        )
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        INSERT INTO users_new (user_id, password, user_role, auth_provider, google_sub, display_name, avatar_url, bio)
        SELECT user_id, password, user_role, 'local', NULL, NULL, NULL, NULL
        FROM users
        "#,
    )
    .execute(pool)
    .await?;

    query("DROP TABLE users").execute(pool).await?;
    query("ALTER TABLE users_new RENAME TO users")
        .execute(pool)
        .await?;
    ensure_user_indexes(pool).await?;

    Ok(())
}

async fn create_users_table(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            user_id TEXT PRIMARY KEY,
            password TEXT,
            user_role TEXT NOT NULL,
            auth_provider TEXT NOT NULL DEFAULT 'local',
            google_sub TEXT,
            display_name TEXT,
            avatar_url TEXT,
            bio TEXT
        )
        "#,
    )
    .execute(pool)
    .await?;
    ensure_user_indexes(pool).await?;

    Ok(())
}

async fn ensure_user_indexes(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_users_google_sub
        ON users(google_sub)
        WHERE google_sub IS NOT NULL
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn migrate_user_matches(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS user_matches (
            match_id INTEGER PRIMARY KEY AUTOINCREMENT,
            requester_user_id TEXT NOT NULL,
            target_user_id TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            responded_at DATETIME,
            closed_at DATETIME
        )
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_user_matches_requester_status
        ON user_matches(requester_user_id, status, created_at DESC, match_id DESC)
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_user_matches_target_status
        ON user_matches(target_user_id, status, created_at DESC, match_id DESC)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn migrate_match_messages(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS match_messages (
            message_id INTEGER PRIMARY KEY AUTOINCREMENT,
            match_id INTEGER NOT NULL,
            sender_user_id TEXT NOT NULL,
            receiver_user_id TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (match_id) REFERENCES user_matches(match_id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_match_messages_match_created_at
        ON match_messages(match_id, created_at ASC, message_id ASC)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn migrate_api_keys(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    if !table_exists(pool, "api_keys").await? {
        create_api_keys_table(pool).await?;
        return Ok(());
    }

    let has_key_lookup = column_exists(pool, "api_keys", "key_lookup").await?;
    let has_user_role = column_exists(pool, "api_keys", "user_role").await?;

    if has_key_lookup && has_user_role {
        query(
            r#"
            CREATE UNIQUE INDEX IF NOT EXISTS idx_api_keys_lookup
            ON api_keys(key_lookup)
            "#,
        )
        .execute(pool)
        .await?;
        query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_api_keys_user_id
            ON api_keys(user_id)
            "#,
        )
        .execute(pool)
        .await?;
        return Ok(());
    }

    query("DROP TABLE IF EXISTS api_keys_new")
        .execute(pool)
        .await?;
    query(
        r#"
        CREATE TABLE api_keys_new (
            api_key_id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            key_lookup TEXT NOT NULL,
            key_hash TEXT NOT NULL,
            user_role TEXT NOT NULL DEFAULT 'user',
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_used_at DATETIME,
            revoked_at DATETIME
        )
        "#,
    )
    .execute(pool)
    .await?;

    if has_key_lookup && !has_user_role {
        query(
            r#"
            INSERT INTO api_keys_new
                (api_key_id, user_id, name, key_lookup, key_hash, user_role, created_at, last_used_at, revoked_at)
            SELECT
                api_key_id,
                user_id,
                name,
                key_lookup,
                key_hash,
                'user',
                COALESCE(created_at, CURRENT_TIMESTAMP),
                last_used_at,
                revoked_at
            FROM api_keys
            "#,
        )
        .execute(pool)
        .await?;
    } else {
        query(
            r#"
            INSERT INTO api_keys_new
                (api_key_id, user_id, name, key_lookup, key_hash, user_role, created_at, last_used_at, revoked_at)
            SELECT
                api_key_id,
                user_id,
                name,
                lower(hex(randomblob(16))),
                key_hash,
                'user',
                COALESCE(created_at, CURRENT_TIMESTAMP),
                last_used_at,
                revoked_at
            FROM api_keys
            "#,
        )
        .execute(pool)
        .await?;
    }

    query("DROP TABLE api_keys").execute(pool).await?;
    query("ALTER TABLE api_keys_new RENAME TO api_keys")
        .execute(pool)
        .await?;
    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_api_keys_lookup
        ON api_keys(key_lookup)
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_api_keys_user_id
        ON api_keys(user_id)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn create_api_keys_table(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS api_keys (
            api_key_id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            key_lookup TEXT NOT NULL,
            key_hash TEXT NOT NULL,
            user_role TEXT NOT NULL DEFAULT 'user',
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_used_at DATETIME,
            revoked_at DATETIME
        )
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_api_keys_lookup
        ON api_keys(key_lookup)
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_api_keys_user_id
        ON api_keys(user_id)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn migrate_rss_push(pool: &SqlitePool) -> std::result::Result<(), sqlx::Error> {
    query(
        r#"
        CREATE TABLE IF NOT EXISTS rss_sources (
            source_id TEXT PRIMARY KEY,
            feed_url TEXT NOT NULL,
            normalized_feed_url TEXT NOT NULL UNIQUE,
            title TEXT,
            site_url TEXT,
            etag TEXT,
            last_modified TEXT,
            last_polled_at DATETIME,
            last_success_at DATETIME,
            last_error TEXT,
            consecutive_failures INTEGER NOT NULL DEFAULT 0,
            is_active BOOLEAN NOT NULL DEFAULT true,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_rss_sources_active
        ON rss_sources(is_active, updated_at DESC)
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE TABLE IF NOT EXISTS rss_feed_items (
            item_id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id TEXT NOT NULL,
            item_guid_hash TEXT NOT NULL,
            guid_or_link TEXT,
            title TEXT NOT NULL,
            link TEXT,
            published_at TEXT,
            detected_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (source_id) REFERENCES rss_sources(source_id)
        )
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_rss_feed_items_source_guid_hash
        ON rss_feed_items(source_id, item_guid_hash)
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE TABLE IF NOT EXISTS user_rss_subscriptions (
            subscription_id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            source_id TEXT NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (source_id) REFERENCES rss_sources(source_id)
        )
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_user_rss_subscriptions_user_source
        ON user_rss_subscriptions(user_id, source_id)
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_user_rss_subscriptions_source
        ON user_rss_subscriptions(source_id)
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE TABLE IF NOT EXISTS web_push_subscriptions (
            push_subscription_id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            endpoint TEXT NOT NULL UNIQUE,
            p256dh TEXT NOT NULL,
            auth TEXT NOT NULL,
            user_agent TEXT,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_success_at DATETIME,
            last_failure_at DATETIME,
            failure_count INTEGER NOT NULL DEFAULT 0,
            revoked_at DATETIME
        )
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_web_push_subscriptions_user_revoked
        ON web_push_subscriptions(user_id, revoked_at)
        "#,
    )
    .execute(pool)
    .await?;

    query(
        r#"
        CREATE TABLE IF NOT EXISTS push_delivery_logs (
            delivery_id INTEGER PRIMARY KEY AUTOINCREMENT,
            push_subscription_id INTEGER NOT NULL,
            item_id INTEGER NOT NULL,
            status TEXT NOT NULL,
            error_message TEXT,
            sent_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (push_subscription_id) REFERENCES web_push_subscriptions(push_subscription_id),
            FOREIGN KEY (item_id) REFERENCES rss_feed_items(item_id)
        )
        "#,
    )
    .execute(pool)
    .await?;
    query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_push_delivery_logs_subscription_item
        ON push_delivery_logs(push_subscription_id, item_id)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn table_exists(
    pool: &SqlitePool,
    table_name: &str,
) -> std::result::Result<bool, sqlx::Error> {
    let exists = query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;

    Ok(exists > 0)
}

async fn column_exists(
    pool: &SqlitePool,
    table_name: &str,
    column_name: &str,
) -> std::result::Result<bool, sqlx::Error> {
    let rows = query(&format!("PRAGMA table_info({})", table_name))
        .fetch_all(pool)
        .await?;

    Ok(rows
        .iter()
        .any(|row| row.get::<String, _>("name") == column_name))
}
