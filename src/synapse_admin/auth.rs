use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum_extra::{
	TypedHeader,
	headers::{Authorization, authorization::Bearer},
};
use http::request::Parts;
use ruma::{OwnedDeviceId, OwnedUserId};
use tuwunel_service::Services;

use crate::error::SynapseError;

/// Extractor that validates a Bearer token and checks that the user is a
/// server administrator.
pub(crate) struct AdminUser {
	#[allow(dead_code)]
	pub(crate) user_id: OwnedUserId,
	#[allow(dead_code)]
	pub(crate) device_id: OwnedDeviceId,
}

impl FromRequestParts<Arc<Services>> for AdminUser {
	type Rejection = SynapseError;

	async fn from_request_parts(
		parts: &mut Parts,
		services: &Arc<Services>,
	) -> Result<Self, Self::Rejection> {
		// Extract Bearer token from Authorization header
		let TypedHeader(Authorization(bearer)) =
			TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, services)
				.await
				.map_err(|_| SynapseError::unauthorized("Missing access token"))?;

		let token = bearer.token();

		// Resolve token to user + device
		let (user_id, device_id, _expires) = services
			.users
			.find_from_token(token)
			.await
			.map_err(|_| SynapseError::unauthorized("Invalid access token"))?;

		// Check admin status
		if !services.admin.user_is_admin(&user_id).await {
			return Err(SynapseError::forbidden(
				"You are not a server admin",
			));
		}

		Ok(Self { user_id, device_id })
	}
}
