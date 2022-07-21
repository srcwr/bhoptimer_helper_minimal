#include <sourcemod>
#include <bhoptimer_helper>

Handle gH_SizeTracker = null;
int gI_Ticks = 0;
float gF_PrevOrigin[MAXPLAYERS+1][3];


native int Shavit_GetBhopStyle(int client);
native int Shavit_GetClientTrack(int client);
native float Shavit_GetStyleSettingFloat(int style, const char[] key);


public void OnPluginStart()
{
	gH_SizeTracker = BH_ClosestPos_SizeTracker();
}

enum struct FromSP
{
	float pos[3];
	int replay_id;
}

public void OnGameFrame()
{
	if (++gI_Ticks & 2) return;

	FromSP stuff[64];
	int updated[2];

	for (int client = 1; client <= MaxClients; client++)
	{
		if (!IsClientConnected(client) || !IsClientInGame(client) || IsFakeClient(client) || !IsPlayerAlive(client))
		{
			continue; // add pause check...
		}

		int slot = client-1;

		GetClientAbsOrigin(client, stuff[slot].pos);
	
		if (stuff[slot].pos[0] == gF_PrevOrigin[client][0] && stuff[slot].pos[1] == gF_PrevOrigin[client][1] && stuff[slot].pos[2] == gF_PrevOrigin[client][2])
		{
			continue;
		}
		
		gF_PrevOrigin[client] = stuff[slot].pos;

		int style = Shavit_GetBhopStyle(client);
		stuff[slot].replay_id = (Shavit_GetClientTrack(client) << 8) | style;

		updated[slot/32] |= 1 << (slot % 32);
	}

	if (updated[0] || updated[1])
		BH_ClosestPos_Update(stuff, updated);
}

public void OnMapStart()
{
	BH_ClosestPos_RemoveAll();
	gI_Ticks = 0;
}