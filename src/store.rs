use crate::models::{CreateThemagraph, Themagraph, UpdateThemagraph};
use crate::query::merge_links;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect(database_url).await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    async fn migrate(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS themagraphs (
                id TEXT PRIMARY KEY,
                body TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS themagraph_links (
                themagraph_id TEXT NOT NULL,
                link TEXT NOT NULL,
                PRIMARY KEY (themagraph_id, link),
                FOREIGN KEY (themagraph_id) REFERENCES themagraphs(id) ON DELETE CASCADE
            );
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_themagraphs(&self) -> Result<Vec<Themagraph>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, body, created_at, updated_at
            FROM themagraphs
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut themagraphs = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.try_get("id")?;
            themagraphs.push(self.hydrate_themagraph(id, row).await?);
        }
        Ok(themagraphs)
    }

    pub async fn get_themagraph(&self, id: &str) -> Result<Option<Themagraph>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, body, created_at, updated_at
            FROM themagraphs
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(self.hydrate_themagraph(id.to_owned(), row).await?)),
            None => Ok(None),
        }
    }

    pub async fn create_themagraph(
        &self,
        payload: CreateThemagraph,
    ) -> Result<Themagraph, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let links = merge_links(&payload.links, &payload.body);

        sqlx::query(
            r#"
            INSERT INTO themagraphs (id, body, created_at, updated_at)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&payload.body)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        self.replace_links(&id, &links).await?;
        self.get_themagraph(&id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn update_themagraph(
        &self,
        id: &str,
        payload: UpdateThemagraph,
    ) -> Result<Option<Themagraph>, sqlx::Error> {
        let updated_at = Utc::now().to_rfc3339();
        let links = merge_links(&payload.links, &payload.body);
        let result = sqlx::query(
            r#"
            UPDATE themagraphs
            SET body = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&payload.body)
        .bind(updated_at)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.replace_links(id, &links).await?;
        self.get_themagraph(id).await
    }

    pub async fn delete_themagraph(&self, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM themagraphs WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn replace_links(&self, id: &str, links: &[String]) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM themagraph_links WHERE themagraph_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        for link in links {
            sqlx::query(
                r#"
                INSERT INTO themagraph_links (themagraph_id, link)
                VALUES (?, ?)
                "#,
            )
            .bind(id)
            .bind(link)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn hydrate_themagraph(
        &self,
        id: String,
        row: sqlx::sqlite::SqliteRow,
    ) -> Result<Themagraph, sqlx::Error> {
        let links = sqlx::query(
            r#"
            SELECT link
            FROM themagraph_links
            WHERE themagraph_id = ?
            ORDER BY link COLLATE NOCASE ASC
            "#,
        )
        .bind(&id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|link_row| link_row.try_get("link"))
        .collect::<Result<Vec<String>, _>>()?;

        let created_at = parse_timestamp(row.try_get::<String, _>("created_at")?);
        let updated_at = parse_timestamp(row.try_get::<String, _>("updated_at")?);
        Ok(Themagraph {
            id,
            body: row.try_get("body")?,
            links,
            created_at,
            updated_at,
        })
    }
}

fn parse_timestamp(value: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}
