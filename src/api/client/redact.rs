use axum::extract::State;
use axum_client_ip::InsecureClientIp;
use ruma::{
	api::client::redact::redact_event, events::room::redaction::RoomRedactionEventContent,
};
use tuwunel_core::{
	Err, Result,
	extension::{EventOrigin, HookContext, HOOK_CTX},
	matrix::pdu::PduBuilder,
	warn,
};

use crate::Ruma;

/// # `PUT /_matrix/client/r0/rooms/{roomId}/redact/{eventId}/{txnId}`
///
/// Tries to send a redaction event into the room.
///
/// - TODO: Handle txn id
pub(crate) async fn redact_event_route(
	State(services): State<crate::State>,
	InsecureClientIp(client_ip): InsecureClientIp,
	body: Ruma<redact_event::v3::Request>,
) -> Result<redact_event::v3::Response> {
	let sender_user = body.sender_user();
	let sender_device = body.sender_device.as_deref();
	let appservice_info = body.appservice_info.as_ref();
	let body = &body.body;

	if services.config.disable_local_redactions
		&& !services.admin.user_is_admin(sender_user).await
	{
		warn!(
			%sender_user,
			event_id = %body.event_id,
			"Local redactions are disabled, non-admin user attempted to redact an event"
		);
		return Err!(Request(Forbidden("Redactions are disabled on this server.")));
	}

	let state_lock = services.state.mutex.lock(&body.room_id).await;

	let hook_ctx = HookContext {
		sender: sender_user.to_owned(),
		room_id: body.room_id.clone(),
		origin: EventOrigin::Local {
			device_id: sender_device.map(ToOwned::to_owned),
			client_ip: Some(client_ip),
			is_appservice: appservice_info.is_some(),
		},
	};

	let event_id = HOOK_CTX
		.scope(hook_ctx, async {
			services
				.timeline
				.build_and_append_pdu(
					PduBuilder {
						redacts: Some(body.event_id.clone()),
						..PduBuilder::timeline(&RoomRedactionEventContent {
							redacts: Some(body.event_id.clone()),
							reason: body.reason.clone(),
						})
					},
					sender_user,
					&body.room_id,
					&state_lock,
				)
				.await
		})
		.await?;

	drop(state_lock);

	Ok(redact_event::v3::Response { event_id })
}
