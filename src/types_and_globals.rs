#![allow(non_upper_case_globals)]

use std::cell::Cell;
use std::ffi::c_void;
use std::mem::MaybeUninit;

use sm_ext::cell_t;
use sm_ext::native;
use sm_ext::CellArray;
use sm_ext::FileObject;
use sm_ext::HandleError;
use sm_ext::HandleId;
use sm_ext::HandleType;
use sm_ext::IExtension;
use sm_ext::IForwardManager;
use sm_ext::IHandleSys;
use sm_ext::IPluginContext;
use sm_ext::IShareSys;
use sm_ext::ISourceMod;
use sm_ext::IdentityTokenPtr;
use sm_ext::TryFromPlugin;
use sm_ext::TryIntoPlugin;
use sm_ext::TypeAccess;

#[derive(Debug)]
pub struct FileObjectHandle<'a>(pub &'a mut FileObject);

impl<'ctx> TryFromPlugin<'ctx> for FileObjectHandle<'_> {
	type Error = HandleError;
	fn try_from_plugin(ctx: &'ctx IPluginContext, value: cell_t) -> Result<Self, Self::Error> {
		Ok(Self(unsafe {
			&mut *(FILE_TYPE.read_handle(HandleId::from(value), ctx.get_identity())?
				as *mut FileObject)
		}))
	}
}

#[derive(Debug)]
pub struct CellArrayHandle<'a>(pub &'a mut CellArray);

impl<'ctx> TryFromPlugin<'ctx> for CellArrayHandle<'_> {
	type Error = HandleError;
	fn try_from_plugin(ctx: &'ctx IPluginContext, value: cell_t) -> Result<Self, Self::Error> {
		Ok(Self(unsafe {
			&mut *(ARRAYLIST_TYPE.read_handle(HandleId::from(value), ctx.get_identity())?
				as *mut CellArray)
		}))
	}
}

pub fn try_into_plugin_box<T>(
	object: T,
	ctx: &IPluginContext,
	asdf: &HandleType<T>,
) -> Result<cell_t, HandleError> {
	let object = Box::into_raw(Box::new(object)) as *mut c_void;
	let res = asdf.create_handle(object, ctx.get_identity(), None);
	match res {
		Ok(handleid) => Ok(handleid.into()),
		Err(e) => {
			unsafe { drop(Box::from_raw(object as *mut T)) };
			Err(e)
		},
	}
}

pub fn try_from_plugin_box<'ctx, T>(
	ctx: &'ctx IPluginContext,
	value: cell_t,
	asdf: &HandleType<T>,
) -> Result<&'ctx mut T, HandleError> {
	unsafe {
		let object = asdf.read_handle(HandleId::from(value), ctx.get_identity())?;
		let object = Box::from_raw(object as *mut T);
		Ok(Box::leak(object))
	}
}

pub static mut SOURCEMOD: Option<ISourceMod> = None;
pub static mut HANDLESYS: Option<IHandleSys> = None;
pub static mut SHARESYS: Option<IShareSys> = None;
pub static mut FORWARDSYS: Option<IForwardManager> = None;
pub static mut g_pCoreIdent: IdentityTokenPtr = std::ptr::null_mut();
pub static mut FILE_TYPE: HandleType<&mut FileObject> =
	unsafe { MaybeUninit::zeroed().assume_init() };
pub static mut ARRAYLIST_TYPE: HandleType<&mut CellArray> =
	unsafe { MaybeUninit::zeroed().assume_init() };
pub static mut CLIENT_MAX: i32 = 0;
//pub static mut FRAME_COUNT: u32 = 0;

pub fn map_start(edict_list: *mut c_void, edict_count: i32, client_max: i32) {
	unsafe {
		CLIENT_MAX = client_max;
		//FRAME_COUNT = 0;
	};
}

pub fn map_end() {}

pub fn load(myself: &IExtension, sys: &IShareSys) -> Result<(), Box<dyn std::error::Error>> {
	let handlesys: IHandleSys = sys.request_interface(&myself)?;
	let sourcemod: ISourceMod = sys.request_interface(&myself)?;
	let forwardsys: IForwardManager = sys.request_interface(&myself)?;
	let core_ident = handlesys.core_ident();

	unsafe {
		SOURCEMOD = Some(sourcemod);
		HANDLESYS = Some(handlesys);
		SHARESYS = Some(*sys);
		FORWARDSYS = Some(forwardsys);

		g_pCoreIdent = core_ident;

		FILE_TYPE = handlesys.faux_type(
			handlesys
				.find_type("File")
				.ok_or("Couldn't find the File type.")?,
			core_ident,
		)?;
		ARRAYLIST_TYPE = handlesys.faux_type(
			handlesys
				.find_type("CellArray")
				.ok_or("Couldn't find the CellArray (ArrayList) type.")?,
			core_ident,
		)?;
	};

	Ok(())
}

pub fn unload() {
	unsafe {
		FORWARDSYS = None;
		SOURCEMOD = None;
		HANDLESYS = None;
		SHARESYS = None;
	};
}
