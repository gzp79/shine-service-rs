use bb8::{ManageConnection, Pool as BB8Pool, RunError};
use bb8_redis::RedisConnectionManager;

pub use shine_macros::RedisJsonValue;

pub type RedisConnection = RedisConnectionManager;
pub type RedisConnectionError = RunError<<RedisConnection as ManageConnection>::Error>;
pub type RedisConnectionPool = BB8Pool<RedisConnection>;

pub async fn create_redis_pool(cns: &str) -> Result<RedisConnectionPool, RedisConnectionError> {
    let redis_manager = RedisConnectionManager::new(cns)?;
    let redis = bb8::Pool::builder()
        .max_size(10) // Set the maximum number of connections in the pool
        .build(redis_manager)
        .await?;

    Ok(redis)
}
