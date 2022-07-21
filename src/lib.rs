#![feature(const_maybe_uninit_zeroed)]
#![feature(maybe_uninit_uninit_array)]
#![feature(core_intrinsics)]
#![feature(abi_thiscall)]
#![feature(slice_partition_dedup)]
// TODO DISABLE
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(non_snake_case)]

mod closestpos;
//mod replay;
mod types_and_globals;

use std::ffi::c_void;

use sm_ext::register_natives;
use sm_ext::IExtension;
use sm_ext::IExtensionInterface;
use sm_ext::IShareSys;
use sm_ext::SMExtension;

use crate::types_and_globals::*;

#[derive(Default, SMExtension)]
#[extension(
	name = "bhoptimer_helper",
	description = "Helper for bhoptimer. ClosestPos, JSON, async replay loading, async sql, etc...",
	author = "rtldg",
	url = "https://github.com/srcwr/bhoptimer_helper",
	version = "1.0.0",
	tag = "BHOP"
)]
pub struct MyExtension {
	//arraylist_handle_type: Option<HandleType<ICellArray>>,
}

impl MyExtension {
	/// Helper to get the extension singleton from the global provided by sm-ext.
	/// This is implemented here rather than by the SMExtension derive to aid code completion.
	fn get() -> &'static Self {
		EXTENSION_GLOBAL.with(|ext| unsafe { &(*ext.borrow().unwrap()).delegate })
	}
}

//#[native]
//fn maps_folder_iter()...

impl IExtensionInterface for MyExtension {
	fn on_core_map_start(&mut self, edict_list: *mut c_void, edict_count: i32, client_max: i32) {
		types_and_globals::map_start(edict_list, edict_count, client_max);
		closestpos::map_start(edict_list, edict_count, client_max);
	}

	fn on_core_map_end(&mut self) {
		types_and_globals::map_end();
		closestpos::map_end();
	}

	fn on_extension_unload(&mut self) {
		closestpos::unload();
		//replay::unload();
		types_and_globals::unload();
	}

	fn on_extension_load(
		&mut self,
		myself: IExtension,
		sys: IShareSys,
		late: bool,
	) -> Result<(), Box<dyn std::error::Error>> {
		println!(
			">>> bhoptimer_helper extension loaded! me = {:?}, sys = {:?}, late = {:?}",
			myself, sys, late
		);

		types_and_globals::load(&myself, &sys)?;
		closestpos::load(&myself, &sys)?;
		//replay::load(&myself, &sys)?;

		sys.register_library(&myself, "bhoptimer_helper");

		register_natives!(&sys, &myself, [
			// ("ClosestPos2.ClosestPos2", closestpos::create),
			// ("ClosestPos2.Find", closestpos::find),
			("BH_ClosestPos_Update", closestpos::BH_ClosestPos_Update),
			("BH_ClosestPos_Get", closestpos::BH_ClosestPos_Get),
			("BH_ClosestPos_Register", closestpos::BH_ClosestPos_Register),
			("BH_ClosestPos_Remove", closestpos::BH_ClosestPos_Remove),
			(
				"BH_ClosestPos_RemoveAll",
				closestpos::BH_ClosestPos_RemoveAll
			),
			(
				"BH_ClosestPos_SizeTracker",
				closestpos::BH_ClosestPos_SizeTracker
			),
			//("BH_LoadReplayCache", replay::BH_LoadReplayCache),
			//("BH_LoadReplayCacheHandle", replay::BH_LoadReplayCacheHandle),
		]);

		Ok(())
	}
}
