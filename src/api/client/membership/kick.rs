use axum::extract::State;
use axum_client_ip::InsecureClientIp;
use futures::FutureExt;
use ruma::api::client::membership::kick_user;
use tuwunel_core::{
	Err, Result,
	extension::{EventOrigin, HookContext, HOOK_CTX},
};

use crate::Ruma;

/// # `POST /_matrix/client/r0/rooms/{roomId}/kick`
///
/// Tries to send a kick event into the room.
pub(crate) async fn kick_user_route(
	State(services): State<crate::State>,
	InsecureClientIp(client_ip): InsecureClientIp,
	body: Ruma<kick_user::v3::Request>,
) -> Result<kick_user::v3::Response> {
	let sender_user = body.sender_user();

	if sender_user == body.user_id {
		return Err!(Request(Forbidden("You cannot kick yourself.")));
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
				.kick(
					&body.room_id,
					&body.user_id,
					body.reason.as_ref(),
					sender_user,
					&state_lock,
				)
				.boxed()
				.await
		})
		.await?;

	drop(state_lock);

	Ok(kick_user::v3::Response::new())
}
