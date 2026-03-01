extern crate hello_extension; // ensure linker includes the extension

use std::sync::atomic::Ordering;

use tuwunel_core::Result;

fn main() -> Result {
	let args = tuwunel::args::parse();
	let runtime = tuwunel::runtime::new(Some(&args))?;
	let server = tuwunel::Server::new(Some(&args), Some(runtime.handle()))?;
	tuwunel::exec(&server, runtime)?;

	#[cfg(unix)]
	if server.server.restarting.load(Ordering::Acquire) {
		tuwunel::restart::restart();
	}

	Ok(())
}
