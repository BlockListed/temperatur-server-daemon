use axum::{http::StatusCode, response::{IntoResponse, Response}};

pub struct ServerError {
    status: StatusCode,
    msg: String,
}

pub type ServerResult<T> = Result<T, ServerError>;

impl From<sqlx::Error> for ServerError {
    fn from(value: sqlx::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            msg: value.to_string(),
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        (self.status, self.msg).into_response()
    }
}