use tracing_subscriber::util::SubscriberInitExt;

use std::sync::Arc;
use std::{env::args, ops::DerefMut};
use tokio::sync::Mutex;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, Redirect},
    routing::{get, post},
    Router, Server,
};
use axum_client_ip::ClientIp;
use std::net::{IpAddr, SocketAddr};

use serde::Deserialize;

use chrono::{Duration, Utc};

use sqlx::{Connection, MySql, Pool};

use sqlx::types::chrono::DateTime;

struct SharedState {
    pub ip: Mutex<IpState>,
    pub db: DbState,
}

struct IpState {
    pub ip: Option<IpAddr>,
    pub expires: Option<DateTime<Utc>>,
}

struct DbState(Pool<MySql>);

async fn ip_update(ClientIp(client_ip): ClientIp, State(state): State<Arc<SharedState>>) {
    let mut ip_state_locked = state.ip.lock().await;
    let new_expires = Utc::now() + Duration::seconds(50);
    tracing::debug!(%new_expires, "Updating expire time!");
    ip_state_locked.expires = Some(new_expires);
    if std::mem::replace(&mut ip_state_locked.deref_mut().ip, Some(client_ip)) != Some(client_ip)
    {
        tracing::info!(ip = %client_ip ,"Updated IP address");
    }
}

async fn forward(
    State(s): State<Arc<SharedState>>,
) -> Result<Redirect, (StatusCode, Html<&'static str>)> {
    let ip_state_locked = s.ip.lock().await;
    let ip_str = match ip_state_locked.ip {
        Some(x) => {
            format!("{}", x)
        }
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Html("<h1>Raspberry ist wahrscheinlich Offline</h1>"),
            ))
        }
    };
    // Unwrap is fine, since it will be set at the same time as IP address
    if ip_state_locked.expires.unwrap() < Utc::now() {
        tracing::warn!(expires = %ip_state_locked.expires.unwrap(), "Raspberry seems to be offline, since IP expired.");
        return Err((
            StatusCode::NOT_FOUND,
            Html("<h1>Raspberry ist wahrscheinlich Offline</h1>"),
        ));
    }
    Ok(Redirect::temporary(
        (String::from("http://") + ip_str.as_str() + ":3000").as_str(),
    ))
}

#[derive(Deserialize)]
struct InsertQueryParams {
    temperatur: f64,
    kohlenstoff: i64,
    raum_id: i32,
}

async fn insert(
    Query(q): Query<InsertQueryParams>,
    State(s): State<Arc<SharedState>>,
) -> (StatusCode, String) {
    match sqlx::query!(
        "INSERT INTO temperaturmessung (comessung, temperature, raum_id) values (?, ?, ?)",
        q.kohlenstoff,
        q.temperatur,
        q.raum_id
    )
    .execute(s.db.0.acquire().await)
    .await
    {
        Ok(_) => {
            tracing::info!(co2 = q.kohlenstoff, t = q.temperatur, raum = q.raum_id, "Successfully entered data into database!");
            (StatusCode::OK, "Success".to_string())
        }
        Err(e) => {
            tracing::error!(%e, "Couldn't insert into database!");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e))
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::try_new("info").unwrap()),
        )
        .finish()
        .init();

    let ip = Mutex::new(IpState {
        ip: None,
        expires: None,
    });
    let db = DbState(
        Pool::<MySql>::connect(
            std::env::var("DATABASE_URL")
                .expect("Missing `DATABASE_URL`")
                .as_str(),
        )
        .await
        .expect("Couldn't connect to Database"),
    );
    let state = Arc::new(SharedState { ip, db });

    let app = Router::new()
        .route("/ip_update", post(ip_update))
        .route("/forward", get(forward))
        .route("/insert", post(insert))
        .with_state(state);

    Server::bind(&args().nth(1).expect("Give argument with port to bind to. Eg. 0.0.0.0:1420").parse().unwrap())
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
