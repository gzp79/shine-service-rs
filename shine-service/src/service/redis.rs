use bb8::{ManageConnection, Pool as BB8Pool, PooledConnection, RunError};

pub use bb8_redis::RedisConnectionManager;
pub use shine_macros::RedisJsonValue;

pub type RedisConnectionError = RunError<<RedisConnectionManager as ManageConnection>::Error>;
pub type RedisConnectionPool = BB8Pool<RedisConnectionManager>;
pub type RedisPooledConnection<'a> = PooledConnection<'a, RedisConnectionManager>;

pub async fn create_redis_pool(cns: &str) -> Result<RedisConnectionPool, RedisConnectionError> {
    let redis_manager = RedisConnectionManager::new(cns)?;
    let redis = bb8::Pool::builder()
        .max_size(10) // Set the maximum number of connections in the pool
        .build(redis_manager)
        .await?;

    {
        let client = &mut *redis.get().await?;
        let pong: String = redis::cmd("PING").query_async(client).await?;
        log::info!("Redis pong: {pong}");
    }

    Ok(redis)
}
