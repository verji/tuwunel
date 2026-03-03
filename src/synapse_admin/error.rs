use axum::response::{IntoResponse, Response};
use http::StatusCode;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SynapseError {
	pub(crate) errcode: &'static str,
	pub(crate) error: String,
	#[serde(skip)]
	pub(crate) status: StatusCode,
}

impl SynapseError {
	pub(crate) fn not_found(msg: impl Into<String>) -> Self {
		Self {
			errcode: "M_NOT_FOUND",
			error: msg.into(),
			status: StatusCode::NOT_FOUND,
		}
	}

	pub(crate) fn forbidden(msg: impl Into<String>) -> Self {
		Self {
			errcode: "M_FORBIDDEN",
			error: msg.into(),
			status: StatusCode::FORBIDDEN,
		}
	}

	pub(crate) fn unauthorized(msg: impl Into<String>) -> Self {
		Self {
			errcode: "M_UNAUTHORIZED",
			error: msg.into(),
			status: StatusCode::UNAUTHORIZED,
		}
	}

	pub(crate) fn unknown(msg: impl Into<String>) -> Self {
		Self {
			errcode: "M_UNKNOWN",
			error: msg.into(),
			status: StatusCode::INTERNAL_SERVER_ERROR,
		}
	}
}

impl IntoResponse for SynapseError {
	fn into_response(self) -> Response {
		let status = self.status;
		let body = serde_json::to_string(&self).expect("SynapseError serialization cannot fail");
		(status, [("content-type", "application/json")], body).into_response()
	}
}

impl std::fmt::Display for SynapseError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}: {}", self.errcode, self.error)
	}
}
