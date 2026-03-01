use std::sync::Arc;

use async_trait::async_trait;
use tuwunel_core::{
	Result,
	extension::{
		EventDecision, EventInterceptor, ExtensionApi, ExtensionEntry, HookContext,
	},
	matrix::pdu::{PduBuilder, PduEvent},
};
use tracing::{info, warn};

struct HelloExtension {
	api: Arc<dyn ExtensionApi>,
}

#[async_trait]
impl EventInterceptor for HelloExtension {
	fn name(&self) -> &str { "hello-extension" }

	async fn before_local_event(
		&self,
		ctx: &HookContext,
		builder: &mut PduBuilder,
	) -> Result<EventDecision> {
		info!(
			sender = %ctx.sender,
			room_id = %ctx.room_id,
			client_ip = ?ctx.client_ip(),
			device_id = ?ctx.device_id(),
			"[hello-extension] before_local_event"
		);

		// Check if the message body contains EXTENSION_BLOCK
		if let Ok(body) = serde_json::from_str::<serde_json::Value>(builder.content.get()) {
			if let Some(text) = body.get("body").and_then(|v| v.as_str()) {
				if text.contains("EXTENSION_BLOCK") {
					// Allow admins to bypass the block
					if self.api.user_is_admin(&ctx.sender).await {
						info!(
							sender = %ctx.sender,
							"[hello-extension] Admin bypassed EXTENSION_BLOCK"
						);
						return Ok(EventDecision::Allow);
					}

					warn!(
						sender = %ctx.sender,
						room_id = %ctx.room_id,
						"[hello-extension] Blocking event containing EXTENSION_BLOCK"
					);
					return Ok(EventDecision::Block(
						"Message blocked by hello-extension: contains EXTENSION_BLOCK".into(),
					));
				}
			}
		}

		Ok(EventDecision::Allow)
	}

	async fn after_event(&self, ctx: &HookContext, event: &PduEvent) -> Result {
		let members = self.api.get_room_members(&ctx.room_id).await;

		info!(
			event_id = %event.event_id,
			sender = %ctx.sender,
			room_id = %ctx.room_id,
			member_count = members.len(),
			"[hello-extension] after_event"
		);

		Ok(())
	}
}

inventory::submit! {
	ExtensionEntry {
		name: "hello-extension",
		create: |_config, api| {
			Ok(Box::new(HelloExtension { api }))
		},
	}
}
