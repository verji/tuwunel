use axum::extract::State;
use axum_client_ip::InsecureClientIp;
use futures::FutureExt;
use ruma::api::client::membership::ban_user;
use tuwunel_core::{
	Err, Result,
	extension::{EventOrigin, HookContext, HOOK_CTX},
};

use crate::Ruma;

/// # `POST /_matrix/client/r0/rooms/{roomId}/ban`
///
/// Tries to send a ban event into the room.
pub(crate) async fn ban_user_route(
	State(services): State<crate::State>,
	InsecureClientIp(client_ip): InsecureClientIp,
	body: Ruma<ban_user::v3::Request>,
) -> Result<ban_user::v3::Response> {
	let sender_user = body.sender_user();

	if sender_user == body.user_id {
		return Err!(Request(Forbidden("You cannot ban yourself.")));
	}

	let state_lock = services.state.mutex.lock(&body.room_id).await;

	let hook_ctx = HookContext {
		sender: sender_user.to_owned(),
		room_id: body.room_id.clone(),
		origin: EventOrigin::Local {
			device_id: body.sender_device.as_deref().map(ToOwned::to_owned),
			client_ip: Some(client_ip),
			is_appservice: body.appservice_info.is_some(),
		},
	};

	HOOK_CTX
		.scope(hook_ctx, async {
			services
				.membership
				.ban(&body.room_id, &body.user_id, body.reason.as_ref(), sender_user, &state_lock)
				.boxed()
				.await
		})
		.await?;

	drop(state_lock);

	Ok(ban_user::v3::Response::new())
}
