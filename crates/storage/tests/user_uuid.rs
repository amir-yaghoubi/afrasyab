use afrasyab_storage::users::UsersRepo;

#[sqlx::test(migrations = "../../migrations")]
async fn user_row_decodes_text_uuid(pool: sqlx::SqlitePool) -> sqlx::Result<()> {
    let user = UsersRepo::new(&pool)
        .get_or_create_by_telegram_id(42)
        .await?;
    let loaded = UsersRepo::new(&pool)
        .get_by_id(user.id)
        .await?
        .expect("user row");
    assert_eq!(loaded.id, user.id);
    assert_eq!(loaded.telegram_user_id, 42);
    Ok(())
}
