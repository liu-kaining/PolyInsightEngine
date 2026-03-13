use redis::AsyncCommands;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type RedisPool = Arc<RwLock<redis::aio::ConnectionManager>>;

pub async fn connect(url: &str) -> anyhow::Result<RedisPool> {
    let client = redis::Client::open(url)?;
    let conn = redis::aio::ConnectionManager::new(client).await?;
    Ok(Arc::new(RwLock::new(conn)))
}

pub async fn ping(pool: &RedisPool) -> anyhow::Result<()> {
    let mut conn = pool.write().await;
    redis::cmd("PING").query_async::<()>(&mut *conn).await?;
    Ok(())
}

pub async fn get_json<T: serde::de::DeserializeOwned>(
    pool: &RedisPool,
    key: &str,
) -> anyhow::Result<Option<T>> {
    let mut conn = pool.write().await;
    let raw: Option<String> = conn.get(key).await?;
    match raw {
        Some(s) => Ok(Some(serde_json::from_str(&s)?)),
        None => Ok(None),
    }
}

pub async fn set_json<T: serde::Serialize>(
    pool: &RedisPool,
    key: &str,
    value: &T,
    ttl_secs: Option<u64>,
) -> anyhow::Result<()> {
    let mut conn = pool.write().await;
    let s = serde_json::to_string(value)?;
    if let Some(ttl) = ttl_secs {
        conn.set_ex::<_, _, ()>(key, s, ttl).await?;
    } else {
        conn.set::<_, _, ()>(key, s).await?;
    }
    Ok(())
}

pub async fn xadd(pool: &RedisPool, stream: &str, field: &str, value: &str) -> anyhow::Result<()> {
    let mut conn = pool.write().await;
    conn.xadd::<_, _, _, _, ()>(stream, "*", &[(field, value)]).await?;
    Ok(())
}
