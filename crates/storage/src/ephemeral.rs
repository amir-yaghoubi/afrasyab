use crate::DbPool;

pub async fn purge_expired(pool: &DbPool, table: &str) -> sqlx::Result<()> {
    let sql = format!("DELETE FROM {table} WHERE expires_at < datetime('now')");
    sqlx::query(&sql).execute(pool).await?;
    Ok(())
}

pub fn expires_in_secs(secs: u64) -> String {
    format!("datetime('now', '+{secs} seconds')")
}
