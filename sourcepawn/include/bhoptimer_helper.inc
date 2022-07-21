
#if defined _bhoptimer_helper_included
	#endinput
#endif
#define _bhoptimer_helper_included

#include "bhoptimer_helper_closest"
//#include "bhoptimer_helper_replay"

public Extension __ext_bhoptimer_helper =
{
	name = "bhoptimer_helper",
	file = "bhoptimer_helper.ext",
#if defined AUTOLOAD_EXTENSIONS
	autoload = 1,
#else
	autoload = 0,
#endif
#if defined REQUIRE_EXTENSIONS
	required = 1,
#else
	required = 0,
#endif
};

#if !defined REQUIRE_EXTENSIONS
public void __ext_bhoptimer_helper_SetNTVOptional()
{
	__ext_bhoptimer_helper_closest_SetNTVOptional();
	//__ext_bhoptimer_helper_replay_SetNTVOptional();
}
#endif
