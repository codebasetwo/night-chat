use secrecy::{SecretString};
use crate::utils::tokens::Token;

pub struct User{
    pub id: uuid::Uuid,
    pub first_name: String,
    pub email: String,
    pub password_hash: SecretString,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn get_user_from_token(
    pool: &sqlx::PgPool, 
    token: &str,
    token_scope: &str,
) -> Result<Option<User>, anyhow::Error> {
    let token_hash = Token::hash_token(token);
    let row = sqlx::query!(
        r#"
            SELECT users.id, users.first_name, users.email, users.password_hash, users.created_at
            FROM users
            INNER JOIN tokens
            ON users.id = tokens.user_id
            WHERE tokens.hash = $1
            AND tokens.scope = $2
            AND tokens.expiry > $3;
        "#,
        token_hash,
        token_scope,
        chrono::Utc::now(),
    )
    .fetch_optional(pool)
    .await?;
    // Map the row to a User object ONLY if the row exists
    let user = row.map(|r| User {
        id: r.id,
        first_name: r.first_name,
        email: r.email,
        password_hash: SecretString::new(r.password_hash),
        created_at: r.created_at,
    });
    Ok(user) 
}


pub async fn get_user_data(
    pool: &sqlx::PgPool,
    email: &str,
) -> Result<(uuid::Uuid,  SecretString), anyhow::Error> {
    let row = sqlx::query!(
        r#"
            SELECT id, password_hash
            FROM users
            WHERE email = $1;
        "#,
        email,
    )
    .fetch_one(pool)
    .await?;
    Ok((row.id, SecretString::new(row.password_hash)))
}