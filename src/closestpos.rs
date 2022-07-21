use std::default::Default;
use std::error::Error;
use std::mem::MaybeUninit;
use std::os::raw::c_uint;
use std::os::raw::c_void;
use std::time::Duration;
use std::time::Instant;

use lazy_static::lazy_static;
use parking_lot::Condvar;
use parking_lot::Mutex;
use sm_ext::cell_t;
use sm_ext::native;
use sm_ext::vtable_override;
use sm_ext::CellArray;
use sm_ext::GameFrameHookId;
use sm_ext::HandleError;
use sm_ext::HandleId;
use sm_ext::HandleType;
use sm_ext::HandleTypeId;
use sm_ext::IExtension;
use sm_ext::IHandleSys;
use sm_ext::IHandleTypeDispatchAdapter;
use sm_ext::IHandleTypeDispatchPtr;
use sm_ext::IHandleTypeDispatchVtable;
use sm_ext::IPluginContext;
use sm_ext::IShareSys;
use sm_ext::IdentityToken;
use sm_ext::IdentityTokenPtr;
use sm_ext::RequestableInterface;
use sm_ext::SPError;
use sm_ext::TryFromPlugin;
use sm_ext::TryIntoPlugin;
use sm_ext::TypeAccess;

use crate::types_and_globals::*;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Point {
	pub coord: [f32; 3],
	pub idx:   i32,
}

#[link(name = "nanoflann_shim")]
extern "C" {
	fn nanoflann_shim_create_container(pts: *const Point, pts_size: usize) -> *mut c_void;
	fn nanoflann_shim_delete_container(object: *mut c_void);
	fn nanoflann_shim_get_nearest(object: *mut c_void, query_pt: *const f32) -> i32;
	fn nanoflann_shim_get_used_memory(object: *mut c_void) -> usize;
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct ClosestPos {
	id:   u32,
	pts:  *mut [Point],
	tree: *mut c_void, // KDTreeContainer
}

unsafe impl Send for ClosestPos {} // so we can store the pointers...

impl Drop for ClosestPos {
	fn drop(&mut self) {
		unsafe {
			let rebuilt = Box::from_raw(self.pts);
			drop(rebuilt);
			nanoflann_shim_delete_container(self.tree);
		};
	}
}

pub struct ClosestPosSizeTracker(u32);

impl<'ctx> TryIntoPlugin<'ctx> for ClosestPosSizeTracker {
	type Error = HandleError;
	fn try_into_plugin(self, ctx: &'ctx IPluginContext) -> Result<cell_t, Self::Error> {
		try_into_plugin_box(self, ctx, unsafe { &CLOSESTPOS_SIZETRACKER_TYPE })
	}
}

impl<'ctx> TryFromPlugin<'ctx> for &'ctx mut ClosestPosSizeTracker {
	type Error = HandleError;
	fn try_from_plugin(ctx: &'ctx IPluginContext, value: cell_t) -> Result<Self, Self::Error> {
		try_from_plugin_box(ctx, value, unsafe { &CLOSESTPOS_SIZETRACKER_TYPE })
	}
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[vtable_override]
unsafe extern fn get_dispatch_version(this: IHandleTypeDispatchPtr) -> u32 {
	<IHandleSys as RequestableInterface>::get_interface_version()
}

#[vtable_override]
unsafe extern fn on_handle_destroy(
	this: IHandleTypeDispatchPtr,
	ty: HandleTypeId,
	object: *mut c_void,
) {
	//
}

#[vtable_override]
unsafe extern fn get_handle_approx_size(
	this: IHandleTypeDispatchPtr,
	ty: HandleTypeId,
	object: *mut c_void,
	size: *mut c_uint,
) -> bool {
	let mut aaa = 0;

	{
		let lock = REGISTRY.lock();
		for x in lock.iter() {
			aaa += 20 + nanoflann_shim_get_used_memory(x.tree);
		}
	}

	*size = aaa as u32;
	aaa != 0
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////

static mut CLOSESTPOS_SIZETRACKER_TYPE: HandleType<ClosestPosSizeTracker> =
	unsafe { MaybeUninit::zeroed().assume_init() };

//////////////////////////////////////////////////////////////////////////////////////////////////////////////

// #[repr(C)]
// #[derive(Copy, Clone, Debug)]
// struct ToSP {
//     time_difference: f32,
//     vel3d_diference: f32,
//     vel2d_difference: f32,
//     replay_time_length: f32,
// }

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct FromSP {
	pos:      [f32; 3],
	// vel: [f32; 3],
	// vel_scale: f32, // = tickrate / stylesetting(speed) / stylesetting(timescale)
	replayid: u32, // = (track << 8) | style;
}

#[repr(C)]
struct SharedState {
	from_sp:             [FromSP; 64],
	// to_sp: [ToSP; 64],
	to_sp:               [i32; 64],
	from_sp_client_mask: i64,
	please_die:          bool,
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////

static mut UTTTT: Condvar = Condvar::new();
static mut FRICK: Mutex<SharedState> =
	Mutex::new(unsafe { std::mem::MaybeUninit::zeroed().assume_init() });
lazy_static! {
	static ref REGISTRY: Mutex<Vec<ClosestPos>> = Mutex::new(vec![]);
}
static mut FRAME_HOOK: Option<GameFrameHookId> = None;
static mut FRAME_COUNT: u32 = 0;
static mut THREAD: Option<std::thread::JoinHandle<()>> = None;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////

fn closestpos_thread() {
	let mut working_state: SharedState = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };

	loop {
		{
			let mut global_state = unsafe { FRICK.lock() };
			loop {
				if working_state.from_sp_client_mask != 0 {
					for i in 0..64 {
						if (working_state.from_sp_client_mask & (1 << i)) == 0 {
							continue;
						}
						global_state.to_sp[i] = working_state.to_sp[i];
					}

					working_state.from_sp_client_mask = 0;
				}

				if global_state.please_die {
					return;
				}
				unsafe { UTTTT.wait(&mut global_state) };
				if global_state.please_die {
					return;
				}

				working_state.from_sp = global_state.from_sp;
				working_state.from_sp_client_mask = global_state.from_sp_client_mask;
				global_state.from_sp_client_mask = 0;
				if working_state.from_sp_client_mask != 0 {
					break;
				}
			}
		}

		let registry = REGISTRY.lock();

		// collect every unique replayid to calc all clients for a replay id
		// at the same time so we keep kdtree structures in cache...

		let mut unique_replayids: [u32; 64] = [0; 64];
		let mut unique_count: usize = 0;

		for client_slot in 0..64 {
			if (working_state.from_sp_client_mask & (1 << client_slot)) == 0 {
				continue;
			}
			let replayid = working_state.from_sp[client_slot].replayid;
			if !unique_replayids
				.iter()
				.take(unique_count)
				.position(|v| *v == replayid)
				.is_some()
			{
				unique_replayids[unique_count] = replayid;
				unique_count += 1;
			}
		}
		for unique_idx in 0..unique_count {
			let replayid = unique_replayids[unique_idx];
			if let Some(idx) = registry.iter().position(|v| v.id == replayid) {
				for client_slot in 0..64 {
					if (working_state.from_sp_client_mask & (1 << client_slot)) == 0 {
						continue;
					}
					if working_state.from_sp[client_slot].replayid != replayid {
						continue;
					}
					let now = Instant::now();
					working_state.to_sp[client_slot] = unsafe {
						nanoflann_shim_get_nearest(
							registry[idx].tree,
							working_state.from_sp[client_slot].pos.as_ptr(),
						)
					};
					// println!(
					// 	"idx = {} | elapsed = {}s",
					// 	working_state.to_sp[client_slot],
					// 	now.elapsed().as_secs_f32()
					// );
				}
			} else {
				for client_slot in 0..64 {
					if working_state.from_sp[client_slot].replayid == replayid {
						working_state.to_sp[client_slot] = -1;
					}
				}
			}
		}

		// for i in 0..64 {
		//     if (working_state.from_sp_client_mask & (1 << i)) == 0 { continue; }
		//     // CALC CLOSESTPOS HERE
		//     let replayid = working_state.from_sp[i].replayid;
		//     if let Some(idx) = registry.iter()
		//         .position(|v| v.id == replayid) {
		//         if let Ok((_dist, elem)) = registry[idx].tree
		//             .nearest_one(&working_state.from_sp[i].pos, &squared_euclidean) {
		//             // println!("updating 2 {}", *elem);
		//             working_state.to_sp[i] = *elem;
		//         }
		//     }
		// }
	}
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub fn load(myself: &IExtension, sys: &IShareSys) -> Result<(), Box<dyn std::error::Error>> {
	let thread = std::thread::Builder::new()
		.name("ClosestPos Worker".to_string())
		.spawn(|| {
			println!("Hello, from ClosestPos worker thread!");
			closestpos_thread();
		})?;

	unsafe {
		THREAD = Some(thread);

		let handlesys = HANDLESYS.unwrap();
		let vtable = IHandleTypeDispatchVtable {
			GetDispatchVersion:  IHandleTypeDispatchAdapter::<i32>::get_dispatch_version,
			OnHandleDestroy:     on_handle_destroy,
			GetHandleApproxSize: get_handle_approx_size,
		};
		// "ClosestPosSizeTracker" gets cut off as "ClosestPosSizeTracke"...
		CLOSESTPOS_SIZETRACKER_TYPE =
			handlesys.create_type("ClosestPosSizeTrckr", None, myself.get_identity(), vtable)?;
		FRAME_HOOK = Some(SOURCEMOD.unwrap().add_game_frame_hook(on_game_frame));
	}

	Ok(())
}

pub fn unload() {
	unsafe {
		FRAME_HOOK = None;

		{
			let mut global_state = FRICK.lock();
			global_state.please_die = true;
		}
		UTTTT.notify_one();

		println!("[bhoptimer_helper] Trying to kill ClosestPos thread...");
		THREAD
			.take()
			.expect("Failed to take thread handles")
			.join()
			.unwrap();
		CLOSESTPOS_SIZETRACKER_TYPE = MaybeUninit::zeroed().assume_init();
	}
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub fn map_start(edict_list: *mut c_void, edict_count: i32, client_max: i32) {
	unsafe {
		FRAME_COUNT = 0;
	}
}

pub fn map_end() {}

extern "C" fn on_game_frame(_simulating: bool) {
	unsafe {
		FRAME_COUNT += 1;
	}
	const INTERVAL: u32 = 2;

	if (unsafe { FRAME_COUNT } % INTERVAL) == 0 {}
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[native]
pub fn BH_ClosestPos_SizeTracker(
	ctx: &IPluginContext,
) -> Result<ClosestPosSizeTracker, Box<dyn Error>> {
	Ok(ClosestPosSizeTracker(0))
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[native]
pub fn BH_ClosestPos_Update(
	ctx: &IPluginContext,
	stuff: cell_t,
	updated: &mut i64,
) -> Result<(), Box<dyn Error>> {
	let info = unsafe {
		std::mem::transmute::<*mut cell_t, &mut [FromSP; 64]>(
			// multi dim sourcepawn arrays have an indirection table...
			// https://github.com/alliedmodders/sourcepawn/blob/18cce5f84247302126b5b6292516752d8a6bd1a7/vm/plugin-context.cpp#L628
			(ctx.local_to_phys_addr(stuff)? as *mut cell_t).add(64),
		)
	};

	{
		let mut global_state = unsafe { FRICK.lock() };
		global_state.from_sp_client_mask |= *updated;

		for i in 0..64 {
			if (*updated & (1 << i)) != 0 {
				global_state.from_sp[i] = info[i];
			}
		}
	}

	unsafe {
		UTTTT.notify_one();
	}

	Ok(())
}

#[native]
pub fn BH_ClosestPos_Get(ctx: &IPluginContext, client: i32) -> Result<i32, Box<dyn Error>> {
	if client < 1 || client > 64 {
		return Err("invalid client index supplied..... die, nerd".into());
	}

	let info = {
		let global_state = unsafe { FRICK.lock() };
		global_state.to_sp[(client - 1) as usize]
	};

	Ok(info)
}

#[native]
pub fn BH_ClosestPos_Remove(ctx: &IPluginContext, replayid: u32) -> Result<(), Box<dyn Error>> {
	{
		let mut lock = REGISTRY.lock();
		if let Some(idx) = lock.iter().position(|v| v.id == replayid) {
			lock.remove(idx);
		}
	}

	Ok(())
}

#[native]
pub fn BH_ClosestPos_RemoveAll(ctx: &IPluginContext) -> Result<(), Box<dyn Error>> {
	{
		let mut lock = REGISTRY.lock();
		lock.clear();
	}

	Ok(())
}

#[native]
pub fn BH_ClosestPos_Register(
	ctx: &IPluginContext,
	replayid: u32,
	replay_time_length: f32,
	arraylist: CellArrayHandle,
	offset: i32,
	startidx: i32,
	count: i32,
) -> Result<bool, Box<dyn Error>> {
	if offset < 0 {
		return Err(format!("Offset must be 0 or greater (given {})", offset).into());
	}

	let offset = offset as usize;

	let size = (*arraylist.0).size() as i32;
	let mut count = std::cmp::min(count, size);

	if startidx < 0 || startidx > (size - 1) {
		return Err(format!(
			"startidx ({}) must be >=0 and less than the ArrayList size ({})",
			startidx, size
		)
		.into());
	}

	let startidx = startidx as usize;

	count = std::cmp::min(count, size);

	if count < 1 {
		return Err(format!("count must be 1 or greater (given {})", count).into());
	}

	let count = count as usize;
	let bs = arraylist.0.blocksize();
	let blk = arraylist.0.at(startidx);

	let mut positions: Vec<Point> = Vec::with_capacity(count);
	unsafe {
		positions.set_len(count);
	}

	for i in (0..count).rev() {
		unsafe {
			let pos = std::mem::transmute::<*mut cell_t, &[f32; 3]>(blk.add(i * bs + offset));
			positions[count - i - 1] = Point {
				coord: *pos,
				idx:   (i + startidx) as i32,
			};
		}
	}

	let (deduped, _) = positions
		.as_mut_slice()
		.partition_dedup_by(|a, b| a.coord == b.coord);

	let pts_len = deduped.len();
	let pts = positions.into_boxed_slice();
	let tree = unsafe { nanoflann_shim_create_container(pts.as_ptr(), pts_len) };

	let container = ClosestPos {
		id:   replayid,
		pts:  Box::into_raw(pts),
		tree: tree,
	};

	{
		let mut lock = REGISTRY.lock();
		lock.push(container);
		// if let Some(idx) = lock.iter().position(|v| v.id == replayid) {
		// 	lock[idx] = container;
		// } else {
		// 	lock.push(container);
		// }
	}

	Ok(true)
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
