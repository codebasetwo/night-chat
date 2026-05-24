use rand::distr::Alphanumeric;
use rand::{rng, RngExt};
use sqlx::{Executor, Transaction, Postgres};
use chrono:: {DateTime, Utc};
use uuid;

pub struct Token {
    hash: Vec<u8>,
    expiry: DateTime<Utc>,
    scope: String,
    user_id: uuid::Uuid,
    _plaintext: String,
}

impl Token {
    pub fn new(
        user_id: uuid::Uuid, 
        scope: &str,
        expiry: DateTime<Utc>,
    ) -> Self {
        let plaintext = Self::generate_token();
        let hash = Self::hash_token(&plaintext);
        Self {
            hash,
            expiry,
            scope: scope.to_string(),
            user_id,
            _plaintext: plaintext,
        }
    }

    fn generate_token() -> String {
        let rng = rng();
        rng.sample_iter(Alphanumeric)
            .take(25)
            .map(char::from)
            .collect()
    }

    // Hash token for storage (you should never store raw tokens)
    pub fn hash_token(token: &str) -> Vec<u8> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hasher.finalize().to_vec()
    }

    pub async fn insert_token(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<(), StoreTokenError> {
        let query = sqlx::query!(
            r#"
            INSERT INTO tokens (user_id, hash, expiry, scope)
            VALUES ($1, $2, $3, $4);
            "#,
            self.user_id,
            self.hash,
            self.expiry,
            self.scope,
        );
        transaction
            .execute(query)
            .await
            .map_err(StoreTokenError)?;
        Ok(())
    } 
}

#[derive(thiserror::Error, Debug)]
#[error("A database failure was encountered while trying to store a authentication token.")]
pub struct StoreTokenError(#[source] pub sqlx::Error);