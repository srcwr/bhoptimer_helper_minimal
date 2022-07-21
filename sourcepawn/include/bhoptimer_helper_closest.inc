
#if defined _bhoptimer_helper_closest_included
	#endinput
#endif
#define _bhoptimer_helper_closest_included

native void BH_ClosestPos_Update(any[][] stuff, int updated[2]);

native int BH_ClosestPos_Get(int client);
native void BH_ClosestPos_Register(int replayid, float replay_time_length, ArrayList input, int offset=0, int startidx=0, int count=2147483646);
native void BH_ClosestPos_Remove(int replayid);
native void BH_ClosestPos_RemoveAll();
native Handle BH_ClosestPos_SizeTracker();

stock void __ext_bhoptimer_helper_closest_SetNTVOptional()
{
	MarkNativeAsOptional("BH_ClosestPos_Update");
	MarkNativeAsOptional("BH_ClosestPos_Get");
	MarkNativeAsOptional("BH_ClosestPos_Register");
	MarkNativeAsOptional("BH_ClosestPos_Remove");
	MarkNativeAsOptional("BH_ClosestPos_RemoveAll");
	MarkNativeAsOptional("BH_ClosestPos_SizeTracker");
}
