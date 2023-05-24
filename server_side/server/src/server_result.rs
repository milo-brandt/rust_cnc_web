use std::error::Error;

use axum::response::IntoResponse;
use hyper::StatusCode;

pub type ServerResult<T> = Result<T, ServerError>;

/*
TODO: Could make this split off details, logging something and sending the rest to the user.
The right way to handle this is probably to make the endpoints slim - e.g. validate the input, then move on.

...but I guess there could be 500 errors in validation, so perhaps we really do need to handle a mixture.
 */
pub struct ServerError {
    status_code: StatusCode,
    message: String,
}

impl ServerError {
    pub fn bad_request(message: String) -> ServerError {
        ServerError {
            status_code: StatusCode::BAD_REQUEST,
            message
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        (self.status_code, self.message).into_response()
    }
}

impl<T> From<T> for ServerError
    where anyhow::Error: From<T>
{
    fn from(value: T) -> Self {
        let error = anyhow::Error::from(value);
        Self {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{}\n{}", error.to_string(), error.backtrace())
        }
    }
}