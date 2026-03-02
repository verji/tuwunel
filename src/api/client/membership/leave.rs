use axum::extract::State;
use axum_client_ip::InsecureClientIp;
use futures::FutureExt;
use ruma::api::client::membership::leave_room;
use tuwunel_core::{
	Result,
	extension::{EventOrigin, HookContext, HOOK_CTX},
};

use crate::Ruma;

/// # `POST /_matrix/client/v3/rooms/{roomId}/leave`
///
/// Tries to leave the sender user from a room.
///
/// - This should always work if the user is currently joined.
pub(crate) async fn leave_room_route(
	State(services): State<crate::State>,
	InsecureClientIp(client_ip): InsecureClientIp,
	body: Ruma<leave_room::v3::Request>,
) -> Result<leave_room::v3::Response> {
	let sender_user = body.sender_user();
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
				.leave(sender_user, &body.room_id, body.reason.clone(), false, &state_lock)
				.boxed()
				.await
		})
		.await?;

	if services.config.delete_rooms_after_leave {
		services
			.delete
			.delete_if_empty_local(&body.room_id, state_lock)
			.boxed()
			.await;
	}

	Ok(leave_room::v3::Response {})
}
